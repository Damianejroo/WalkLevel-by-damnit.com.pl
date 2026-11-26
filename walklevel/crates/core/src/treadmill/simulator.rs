//! Symulator bieżni jako pełnoprawny sterownik: ten sam kontrakt co prawdziwe bieżnie (port
//! bieznia/sterowniki/symulator.py).
//!
//! Dzięki temu całe sterowanie (start, stop, prędkość z pada, pilnowanie) działa i da się je
//! przetestować bez Bluetooth. Taśma rozpędza się płynnie, po starcie jest krótkie
//! odliczanie, kroki rosną z kadencją zależną od prędkości. F6 / F7 działają jak pilot
//! bieżni: zmieniają prędkość bez udziału aplikacji ([`SimulatorDriver::remote`]).
//!
//! Liczniki jak w prawdziwej bieżni (docs/analiza-dystans.md, R9): dystans co 10 m z
//! dokładnej całki prędkości, kroki z kadencją `40 + 14·v` na minutę (dopasowaną do Urevo:
//! krok 0,61 m przy 3 km/h) i czas ruchu w pełnych sekundach - ramki przechodzą tę samą
//! drogę ([`super::telemetry::Meter`]) co z bieżni.

use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use futures::FutureExt;
use futures::future::BoxFuture;

use super::kontrakt::{
    Capabilities, Clock, Counts, DeviceInfo, Driver, DriverError, DriverKind, FrameCallback,
    StartResult,
};
use super::telemetry::TelemetryFrame;
use super::watchdog::SpeedStats;
use crate::py::round_tenth;

pub const REPORT_EVERY_S: f64 = 0.75;
pub const ACCEL_KMH_PER_S: f64 = 1.0;
pub const REMOTE_STEP_KMH: f64 = 0.5;

/// Możliwości symulatora.
pub const CAPS: Capabilities = Capabilities {
    speed_min: 0.5,
    speed_max: 6.0,
    speed_step: 0.1,
    start_speed: 1.0,
    start_delay_s: 1.0,
    acks_commands: true,
    pause: true,
    steps: true,
    incline: None,
    telemetry_s: REPORT_EVERY_S,
    min_command_gap_s: 0.0,
    control: true,
};

/// Opis symulatora zapisywany przy sesji.
pub fn info() -> DeviceInfo {
    DeviceInfo {
        model: "Symulator bieżni".into(),
        driver: DriverKind::Simulator,
        profile: "symulator".into(),
        name: String::new(),
        firmware: String::new(),
    }
}

/// Stan taśmy symulatora (do podglądu i testów).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimulatorState {
    pub running: bool,
    pub target: f64,
    /// prędkość taśmy (bez zaokrąglenia)
    pub speed: f64,
    /// kroki (ułamkowe - w ramkach obcięte do całych)
    pub steps: f64,
    /// dokładny dystans (w ramkach co 10 m)
    pub distance_m: f64,
    /// czas jazdy taśmy (w ramkach pełne sekundy)
    pub moving_s: f64,
}

/// Kadencja symulatora (kroki/min) przy prędkości `kmh` - dopasowana do danych z Urevo.
pub fn cadence(kmh: f64) -> f64 {
    40.0 + 14.0 * kmh
}

struct State {
    belt: SimulatorState,
    last_t: Option<f64>,
    report_at: f64,
    belt_busy: u32,
    counts: Counts,
    speed_stats: SpeedStats,
}

/// Symulowana bieżnia (patrz opis modułu).
pub struct SimulatorDriver {
    on_frame: FrameCallback,
    clock: Clock,
    capabilities: Capabilities,
    info: DeviceInfo,
    state: Mutex<State>,
}

impl SimulatorDriver {
    /// `capabilities` - zwykle [`CAPS`] (testy skracają odliczanie).
    pub fn new(on_frame: FrameCallback, clock: Clock, capabilities: Capabilities) -> Self {
        Self {
            on_frame,
            clock,
            capabilities,
            info: info(),
            state: Mutex::new(State {
                belt: SimulatorState {
                    running: false,
                    target: 0.0,
                    speed: 0.0,
                    steps: 0.0,
                    distance_m: 0.0,
                    moving_s: 0.0,
                },
                last_t: None,
                report_at: f64::NEG_INFINITY,
                belt_busy: 0,
                counts: Counts::default(),
                speed_stats: SpeedStats::default(),
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Bieżący stan taśmy.
    pub fn state(&self) -> SimulatorState {
        self.lock().belt
    }

    /// Fizyka taśmy do chwili obecnej i ramka z danymi (co [`REPORT_EVERY_S`]).
    pub fn advance(&self) {
        let now = (self.clock)();
        let frame = {
            let mut s = self.lock();
            let dt = s.last_t.map_or(0.0, |last| (now - last).max(0.0));
            s.last_t = Some(now);
            let b = &mut s.belt;
            let goal = if b.running { b.target } else { 0.0 };
            let change = ACCEL_KMH_PER_S * dt;
            b.speed += (-change).max(change.min(goal - b.speed));
            if b.speed > 0.4 {
                b.steps += cadence(b.speed) / 60.0 * dt;
            }
            if b.speed > 0.0 {
                b.distance_m += b.speed / 3.6 * dt;
                b.moving_s += dt;
            }
            if now - s.report_at >= REPORT_EVERY_S - 1e-6 {
                s.report_at = now;
                s.counts.frames += 1;
                let b = &s.belt;
                Some(TelemetryFrame {
                    // licznik dystansu co 10 m, jak w bieżni
                    distance_m: Some((b.distance_m / 10.0).floor() as u64 * 10),
                    steps: Some(b.steps as u64),
                    elapsed_s: Some(b.moving_s as u64),
                    ..TelemetryFrame::data(now, Some(round_tenth(b.speed)))
                })
            } else {
                None
            }
        };
        if let Some(frame) = frame {
            (self.on_frame)(frame); // poza zamkiem - odbiorca może pytać sterownik o stan
        }
    }

    /// Pilot bieżni (F6/F7): prędkość zmienia sama bieżnia, nie aplikacja.
    pub fn remote(&self, steps: i32) {
        let mut s = self.lock();
        let b = &mut s.belt;
        let target = (if b.running { b.target } else { 0.0 }) + f64::from(steps) * REMOTE_STEP_KMH;
        if target <= 0.0 {
            b.running = false;
            b.target = 0.0;
            return;
        }
        b.running = true;
        b.target = self.capabilities.clamp(target);
    }
}

/// Zdejmuje znacznik „start w toku” także wtedy, gdy start zostanie przerwany.
struct BusyGuard<'a>(&'a SimulatorDriver);

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        let mut s = self.0.lock();
        s.belt_busy = s.belt_busy.saturating_sub(1);
    }
}

impl Driver for SimulatorDriver {
    fn protocol(&self) -> &'static str {
        "symulator"
    }

    fn capabilities(&self) -> Capabilities {
        self.capabilities
    }

    fn info(&self) -> DeviceInfo {
        self.info.clone()
    }

    fn counts(&self) -> Counts {
        self.lock().counts
    }

    fn speed_stats(&self) -> Option<SpeedStats> {
        Some(self.lock().speed_stats.clone())
    }

    fn start(&self) -> BoxFuture<'_, Result<(), DriverError>> {
        self.lock().last_t = Some((self.clock)());
        log::info!(
            "Symulator bieżni: {:.1}-{:.1} km/h",
            CAPS.speed_min,
            CAPS.speed_max
        );
        async { Ok(()) }.boxed()
    }

    fn poll(&self) -> BoxFuture<'_, Result<(), DriverError>> {
        self.advance();
        async { Ok(()) }.boxed()
    }

    fn poll_interval(&self) -> f64 {
        REPORT_EVERY_S
    }

    fn describe(&self) -> String {
        "Symulator bieżni".into()
    }

    fn start_belt(&self) -> BoxFuture<'_, Result<StartResult, DriverError>> {
        async move {
            {
                let mut s = self.lock();
                s.counts.commands += 1;
                if s.belt.running {
                    return Ok(StartResult::Started);
                }
                s.belt_busy += 1;
            }
            let _busy = BusyGuard(self);
            // odliczanie jak w bieżni
            let delay = Duration::try_from_secs_f64(self.capabilities.start_delay_s)
                .unwrap_or(Duration::ZERO);
            tokio::time::sleep(delay).await;
            let mut s = self.lock();
            s.belt.running = true;
            s.belt.target = s.belt.target.max(self.capabilities.start_speed);
            s.counts.acks += 1;
            Ok(StartResult::Started)
        }
        .boxed()
    }

    fn stop_belt(&self, _verify: bool) -> BoxFuture<'_, Result<bool, DriverError>> {
        {
            let mut s = self.lock();
            s.counts.commands += 1;
            s.counts.acks += 1;
            s.belt.running = false;
            s.belt.target = 0.0;
        }
        async { Ok(true) }.boxed()
    }

    fn set_speed(&self, kmh: f64) -> BoxFuture<'_, Result<bool, DriverError>> {
        {
            let mut s = self.lock();
            s.counts.commands += 1;
            s.counts.acks += 1;
            s.speed_stats.sent += 1;
            s.speed_stats.ok += 1;
            if s.belt.running {
                let clamped = self.capabilities.clamp(kmh);
                s.belt.target = if clamped != 0.0 {
                    clamped
                } else {
                    self.capabilities.speed_min
                };
            }
        }
        async { Ok(true) }.boxed()
    }

    fn belt_speed(&self) -> Option<(f64, f64)> {
        let s = self.lock();
        (s.report_at != f64::NEG_INFINITY).then(|| (round_tenth(s.belt.speed), s.report_at))
    }

    fn belt_running(&self) -> Option<bool> {
        Some(self.lock().belt.running)
    }

    fn busy(&self) -> bool {
        self.lock().belt_busy > 0
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    fn driver(clock: Arc<AtomicU64>, readings: Arc<Mutex<Vec<TelemetryFrame>>>) -> SimulatorDriver {
        let caps = Capabilities {
            start_delay_s: 0.0,
            ..CAPS
        };
        let sink: FrameCallback = Arc::new(move |r| readings.lock().expect("odczyty").push(r));
        let tick: Clock = Arc::new(move || f64::from_bits(clock.load(Ordering::SeqCst)));
        SimulatorDriver::new(sink, tick, caps)
    }

    #[tokio::test]
    async fn belt_ramps_up_and_stops() {
        let clock = Arc::new(AtomicU64::new(0f64.to_bits()));
        let readings = Arc::new(Mutex::new(Vec::new()));
        let sim = driver(clock.clone(), readings.clone());
        sim.start().await.expect("start");
        assert_eq!(sim.start_belt().await.expect("taśma"), StartResult::Started);
        assert!(sim.set_speed(2.5).await.expect("prędkość"));
        for i in 1..=6 {
            clock.store((f64::from(i)).to_bits(), Ordering::SeqCst);
            sim.advance();
        }
        assert_eq!(sim.belt_speed().map(|r| r.0), Some(2.5));
        sim.stop_belt(true).await.expect("stop");
        assert_eq!(sim.belt_running(), Some(false));
        assert!(!sim.busy());
        assert!(
            readings
                .lock()
                .expect("odczyty")
                .iter()
                .any(|r| r.steps.unwrap_or(0) > 0)
        );
    }

    /// Przypadki M1-M4 z docs/analiza-dystans.md: liczniki symulatora przez [`Meter`].
    #[tokio::test]
    async fn counters_are_realistic_m1_to_m4() {
        use crate::treadmill::telemetry::{Meter, TelemetrySpec};

        /// Przyrost kroków i dystansu przez `seconds` po rozpędzie do `kmh`.
        async fn walk(kmh: f64, seconds: u32) -> (u64, f64, Meter) {
            let clock = Arc::new(AtomicU64::new(0f64.to_bits()));
            let frames = Arc::new(Mutex::new(Vec::new()));
            let sink_frames = frames.clone();
            let sink: FrameCallback = Arc::new(move |f| sink_frames.lock().expect("ramki").push(f));
            let tick = clock.clone();
            let now: Clock = Arc::new(move || f64::from_bits(tick.load(Ordering::SeqCst)));
            let caps = Capabilities {
                start_delay_s: 0.0,
                speed_min: 0.1, // M3: taśma 0,3 km/h
                ..CAPS
            };
            let sim = SimulatorDriver::new(sink, now, caps);
            sim.start().await.expect("start");
            sim.start_belt().await.expect("taśma");
            sim.set_speed(kmh).await.expect("prędkość");
            let mut meter = Meter::new(TelemetrySpec::for_driver(DriverKind::Simulator, &CAPS));
            let mut t = 0.0;
            // rozpęd (po nim liczymy przyrosty)
            while t < 10.0 {
                t += REPORT_EVERY_S;
                clock.store(t.to_bits(), Ordering::SeqCst);
                sim.advance();
            }
            let warm = frames.lock().expect("ramki").len();
            for f in frames.lock().expect("ramki").iter() {
                meter.feed(f);
            }
            let before = meter.totals();
            let end = t + f64::from(seconds);
            while t < end - 1e-9 {
                t += REPORT_EVERY_S;
                clock.store(t.to_bits(), Ordering::SeqCst);
                sim.advance();
            }
            let all = frames.lock().expect("ramki").clone();
            for f in &all[warm..] {
                meter.feed(f);
            }
            let after = meter.totals();
            (
                after.steps - before.steps,
                after.distance_m - before.distance_m,
                meter,
            )
        }

        let (steps, d, meter) = walk(3.0, 60).await;
        assert!(steps.abs_diff(82) <= 1, "kroki {steps}");
        assert!((d - 50.0).abs() <= 0.1, "dystans {d}");
        assert!(meter.has_distance_counter());
        let (steps, d, _) = walk(1.0, 60).await;
        assert!(steps.abs_diff(54) <= 1, "kroki {steps}");
        let stride = d / steps as f64;
        assert!((stride - 0.31).abs() < 0.01, "krok {stride}");
        let (steps, _, _) = walk(0.3, 30).await;
        assert_eq!(steps, 0); // przy 0,3 km/h kroki nie rosną
        // M4: ten sam zegar - te same ramki
        let run = || async {
            let clock = Arc::new(AtomicU64::new(0f64.to_bits()));
            let frames = Arc::new(Mutex::new(Vec::new()));
            let sim = driver(clock.clone(), frames.clone());
            sim.start().await.expect("start");
            sim.start_belt().await.expect("taśma");
            sim.set_speed(4.0).await.expect("prędkość");
            for i in 1..=200 {
                clock.store((f64::from(i) * 0.75).to_bits(), Ordering::SeqCst);
                sim.advance();
            }
            frames.lock().expect("ramki").clone()
        };
        let (a, b) = (run().await, run().await);
        assert_eq!(a, b);
        assert!(a.iter().any(|f| f.distance_m.unwrap_or(0) >= 10));
    }

    #[test]
    fn remote_changes_speed_without_app() {
        let sim = driver(
            Arc::new(AtomicU64::new(0)),
            Arc::new(Mutex::new(Vec::new())),
        );
        sim.remote(2);
        assert_eq!((sim.state().running, sim.state().target), (true, 1.0));
        sim.remote(-5);
        assert_eq!((sim.state().running, sim.state().target), (false, 0.0));
    }
}
