//! Telemetria bieżni: pełna ramka z każdego sterownika i jedna procedura liczników
//! (docs/analiza-dystans.md, rekomendacje R1-R6).
//!
//! Sterownik oddaje [`TelemetryFrame`] - prędkość i surowe liczniki bieżni (dystans, kroki,
//! czas). [`Meter`] zamienia je na wartości sesji niezależne od mapowania gry:
//!
//! - liczniki ([`Counter`]): przyrost, przepełnienie, zerowanie, pojedynczy błędny odczyt
//!   i skok (inna bieżnia, długa przerwa) - bez fantomowych kroków,
//! - dystans: całka prędkości po czasie, pilnowana licznikiem bieżni (±1 kwant, 10 m),
//! - czas ruchu: zegar ramek, gdy taśma jedzie; przerwy uzupełnia licznik czasu bieżni,
//! - kadencja: okno 15 s z próbek licznika kroków.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use super::kontrakt::{Capabilities, DriverKind};

/// Taśma uznawana za jadącą powyżej tej prędkości (km/h).
pub const MOVING_KMH: f64 = 0.05;
/// Okno kadencji (s).
pub const CADENCE_WINDOW_S: f64 = 15.0;
/// Najmniejsza rozpiętość próbek, przy której kadencja ma sens (X21 podaje kroki co 2 s).
pub const CADENCE_MIN_SPAN_S: f64 = 6.0;
/// Tyle sekund stania taśmy kasuje kadencję.
pub const CADENCE_STOPPED_S: f64 = 3.0;
/// Po takiej przerwie w danych liczniki zaczynają od nowego punktu odniesienia.
pub const COUNTERS_RESET_AFTER_S: f64 = 120.0;
/// Licznik dystansu stoi, choć taśma przejechała tyle metrów (albo 10 kwantów, jeśli więcej) -
/// bieżnia go nie liczy (np. stałe 0), dalej sama całka prędkości.
pub const DISTANCE_STUCK_M: f64 = 100.0;

/// Rodzaj ramki: dane z bieżni albo sam sygnał życia (KingSmith „servers 0”).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FrameKind {
    Data,
    /// Bieżnia żyje - odnawia świeżość danych, nie zmienia liczników ani kadencji.
    Heartbeat,
}

/// Jedna ramka z bieżni. `None` = pola nie ma w tej ramce (NIE zero).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryFrame {
    /// s, zegar monotoniczny w chwili odbioru
    pub t: f64,
    pub kind: FrameKind,
    /// km/h (KingSmith w jednostkach imperialnych - już przeliczone)
    pub speed_kmh: Option<f64>,
    /// surowe liczniki bieżni
    pub distance_m: Option<u64>,
    pub steps: Option<u64>,
    pub elapsed_s: Option<u64>,
}

impl TelemetryFrame {
    /// Ramka z danymi bez liczników.
    pub fn data(t: f64, speed_kmh: Option<f64>) -> Self {
        Self {
            t,
            kind: FrameKind::Data,
            speed_kmh,
            distance_m: None,
            steps: None,
            elapsed_s: None,
        }
    }

    /// Sygnał życia z ostatnią znaną prędkością (dla sterowania) i ostatnimi krokami (dla
    /// podglądu) - [`Meter`] bierze z niego tylko czas.
    pub fn heartbeat(t: f64, speed_kmh: Option<f64>, steps: Option<u64>) -> Self {
        Self {
            kind: FrameKind::Heartbeat,
            steps,
            ..Self::data(t, speed_kmh)
        }
    }

    pub fn is_data(&self) -> bool {
        self.kind == FrameKind::Data
    }
}

/// Licznik bieżni: zakres (przekręca się po `modulus`) i rozdzielczość.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CounterSpec {
    pub modulus: Option<u64>,
    pub quantum: u64,
}

/// Co wiemy o licznikach danej bieżni (z modułu sterownika i możliwości).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetrySpec {
    pub distance: Option<CounterSpec>,
    pub steps: Option<CounterSpec>,
    pub elapsed: Option<CounterSpec>,
    /// typowo co tyle sekund ramka z prędkością
    pub telemetry_s: f64,
    pub speed_max_kmh: f64,
}

impl Default for TelemetrySpec {
    /// Nieznana bieżnia: liczniki bez przepełnienia, dystans co 10 m.
    fn default() -> Self {
        Self {
            distance: Some(CounterSpec {
                modulus: None,
                quantum: 10,
            }),
            steps: Some(CounterSpec {
                modulus: None,
                quantum: 1,
            }),
            elapsed: Some(CounterSpec {
                modulus: None,
                quantum: 1,
            }),
            telemetry_s: 1.0,
            speed_max_kmh: 12.0,
        }
    }
}

impl TelemetrySpec {
    /// Liczniki modułu: FTMS - dystans uint24 (Urevo co 10 m), kroki i czas uint16;
    /// KingSmith i symulator - bez przepełnienia, dystans co 10 m.
    pub fn for_driver(kind: DriverKind, caps: &Capabilities) -> Self {
        let plain = |quantum| {
            Some(CounterSpec {
                modulus: None,
                quantum,
            })
        };
        let wrapping = |bits: u32, quantum| {
            Some(CounterSpec {
                modulus: Some(1u64 << bits),
                quantum,
            })
        };
        let (distance, steps, elapsed) = match kind {
            DriverKind::Ftms => (wrapping(24, 10), wrapping(16, 1), wrapping(16, 1)),
            DriverKind::KingSmith | DriverKind::Simulator => (plain(10), plain(1), plain(1)),
        };
        Self {
            distance,
            steps,
            elapsed,
            telemetry_s: caps.telemetry_s,
            speed_max_kmh: caps.speed_max,
        }
    }

    /// Dłuższy odstęp między ramkami z prędkością to przerwa w danych (nie całkujemy).
    pub fn gap_s(&self) -> f64 {
        3.0_f64.max(3.0 * self.telemetry_s)
    }
}

/// Jak szybko licznik może rosnąć: na sekundę i zapas na kwant odczytu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CounterRate {
    pub per_s: f64,
    pub slack: u64,
}

/// Kroki: najwyżej 4/s (240/min), zapas 5.
pub const STEPS_RATE: CounterRate = CounterRate {
    per_s: 4.0,
    slack: 5,
};
/// Czas: 1 s/s, zapas 2.
pub const ELAPSED_RATE: CounterRate = CounterRate {
    per_s: 1.0,
    slack: 2,
};

/// Dystans: najszybciej, jak taśma jedzie (m/s), bez zapasu.
pub fn distance_rate(speed_max_kmh: f64) -> CounterRate {
    CounterRate {
        per_s: speed_max_kmh.max(0.0) / 3.6,
        slack: 0,
    }
}

/// Jeden licznik bieżni: suma wiarygodnych przyrostów (docs/analiza-dystans.md, R2).
#[derive(Debug, Clone)]
pub struct Counter {
    name: &'static str,
    spec: CounterSpec,
    rate: CounterRate,
    last: Option<u64>,
    last_t: f64,
    pending: Option<u64>,
    total: u64,
}

impl Counter {
    pub fn new(name: &'static str, spec: CounterSpec, rate: CounterRate) -> Self {
        Self {
            name,
            spec,
            rate,
            last: None,
            last_t: 0.0,
            pending: None,
            total: 0,
        }
    }

    /// Suma przyrostów od utworzenia licznika.
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Jest punkt odniesienia (przyszła co najmniej jedna wartość).
    pub fn has_value(&self) -> bool {
        self.last.is_some()
    }

    /// Następna wartość będzie nowym punktem odniesienia (suma zostaje).
    pub fn forget(&mut self) {
        self.last = None;
        self.pending = None;
    }

    /// Największy wiarygodny przyrost w czasie `dt`.
    fn limit(&self, dt: f64) -> u64 {
        let growth = (self.rate.per_s * dt.max(1.0) * 1.25).ceil();
        // `as` nasyca wartości spoza zakresu - bez paniki przy dziwnym zegarze
        (growth as u64)
            .saturating_add(self.spec.quantum)
            .saturating_add(self.rate.slack)
    }

    /// Nowy odczyt `raw` w chwili `t`. Zwraca przyrost doliczony do sumy.
    pub fn feed(&mut self, raw: u64, t: f64) -> u64 {
        let Some(last) = self.last else {
            self.last = Some(raw); // punkt odniesienia - liczniki sprzed połączenia nie liczą się
            self.last_t = t;
            return 0;
        };
        let limit = self.limit(t - self.last_t);
        self.last_t = t;
        let added = if raw >= last {
            // także koniec podejrzanego spadku: to był jednorazowy błąd odczytu
            self.pending = None;
            self.last = Some(raw);
            let d = raw - last;
            if d <= limit {
                d
            } else {
                log::info!(
                    "Licznik {}: skok {last} -> {raw} pominięty (inna bieżnia albo przerwa)",
                    self.name
                );
                0
            }
        } else if let Some(wrapped) = self
            .spec
            .modulus
            .filter(|m| last >= m.saturating_sub(limit))
            .and_then(|m| m.checked_sub(last))
            .map(|rest| rest.saturating_add(raw))
            .filter(|d| *d <= limit)
        {
            self.last = Some(raw); // przepełnienie
            self.pending = None;
            wrapped
        } else if raw <= limit {
            log::info!("Licznik {}: wyzerowany ({last} -> {raw})", self.name);
            self.last = Some(raw); // zerowanie do 0 i dalszy ruch
            self.pending = None;
            raw
        } else if let Some(pending) = self.pending.take() {
            log::info!("Licznik {}: nowy punkt odniesienia {raw}", self.name);
            self.last = Some(raw); // spadek potwierdzony drugim odczytem
            raw.saturating_sub(pending)
        } else {
            self.pending = Some(raw); // podejrzany spadek - czekamy na następny odczyt
            0
        };
        self.total = self.total.saturating_add(added);
        added
    }
}

/// Wartości narastające od utworzenia [`Meter`] (sesja liczy z nich przyrosty).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeterTotals {
    pub steps: u64,
    pub distance_m: f64,
    pub moving_s: f64,
}

/// Poprzednia ramka z prędkością i stan liczników w tej chwili.
#[derive(Debug, Clone, Copy)]
struct SpeedPoint {
    t: f64,
    kmh: f64,
    elapsed_total: u64,
    distance_total: u64,
}

/// Dystans, czas ruchu, kroki i kadencja z ramek jednej bieżni (R2-R6).
#[derive(Debug, Clone)]
pub struct Meter {
    spec: TelemetrySpec,
    distance: Counter,
    steps: Counter,
    elapsed: Counter,
    /// dystans w metrach: całka prędkości pilnowana licznikiem
    integral: f64,
    moving_s: f64,
    prev: Option<SpeedPoint>,
    last_seen: Option<f64>,
    last_data: Option<f64>,
    stopped_since: Option<f64>,
    cadence: VecDeque<(f64, u64)>,
    steps_seen: bool,
    distance_seen: bool,
    /// licznik dystansu stoi mimo jazdy - nie pilnuje już całki
    distance_stuck: bool,
    /// całka bez ograniczenia od ostatniego przyrostu licznika dystansu (m)
    since_counter_m: f64,
    /// dystans (po ograniczeniu) w chwili ostatniego przyrostu licznika
    at_counter_m: f64,
    /// całka w chwili pierwszej wartości licznika dystansu (jego zero)
    counter_base_m: f64,
}

impl Default for Meter {
    fn default() -> Self {
        Self::new(TelemetrySpec::default())
    }
}

impl Meter {
    pub fn new(spec: TelemetrySpec) -> Self {
        let off = CounterSpec {
            modulus: None,
            quantum: 0,
        };
        Self {
            distance: Counter::new(
                "dystansu",
                spec.distance.unwrap_or(off),
                distance_rate(spec.speed_max_kmh),
            ),
            steps: Counter::new("kroków", spec.steps.unwrap_or(off), STEPS_RATE),
            elapsed: Counter::new("czasu", spec.elapsed.unwrap_or(off), ELAPSED_RATE),
            spec,
            integral: 0.0,
            moving_s: 0.0,
            prev: None,
            last_seen: None,
            last_data: None,
            stopped_since: None,
            cadence: VecDeque::new(),
            steps_seen: false,
            distance_seen: false,
            distance_stuck: false,
            since_counter_m: 0.0,
            at_counter_m: 0.0,
            counter_base_m: 0.0,
        }
    }

    pub fn spec(&self) -> &TelemetrySpec {
        &self.spec
    }

    /// Bieżnia podaje licznik kroków (przyszła co najmniej jedna ramka z krokami).
    pub fn has_steps(&self) -> bool {
        self.steps_seen
    }

    /// Dystans pilnowany licznikiem bieżni (inaczej sama całka prędkości).
    pub fn has_distance_counter(&self) -> bool {
        self.distance_seen && !self.distance_stuck
    }

    pub fn totals(&self) -> MeterTotals {
        MeterTotals {
            steps: self.steps.total(),
            distance_m: self.integral,
            moving_s: self.moving_s,
        }
    }

    /// Surowy przyrost licznika dystansu bieżni (m) - do diagnostyki.
    pub fn counter_distance_m(&self) -> u64 {
        self.distance.total()
    }

    /// Czas ostatniej ramki z danymi (zegar ramek).
    pub fn last_data_at(&self) -> Option<f64> {
        self.last_data
    }

    /// Nowa sesja: kadencja liczy się od nowa.
    pub fn clear_cadence(&mut self) {
        self.cadence.clear();
    }

    pub fn feed(&mut self, frame: &TelemetryFrame) {
        if let Some(seen) = self.last_seen
            && frame.t - seen > COUNTERS_RESET_AFTER_S
        {
            // długa przerwa: liczniki od nowa, bez doliczania tego, co było w przerwie
            self.distance.forget();
            self.steps.forget();
            self.elapsed.forget();
            self.prev = None;
            self.cadence.clear();
            self.stopped_since = None;
        }
        self.last_seen = Some(self.last_seen.map_or(frame.t, |s| s.max(frame.t)));
        if !frame.is_data() {
            return; // sygnał życia: tylko świeżość
        }
        self.last_data = Some(frame.t);
        let gap = self.spec.gap_s();
        if let (Some(raw), Some(_)) = (frame.steps, self.spec.steps) {
            self.steps.feed(raw, frame.t);
            self.steps_seen = true;
            if self
                .cadence
                .back()
                .is_some_and(|(t, _)| frame.t - t > gap || frame.t < *t)
            {
                self.cadence.clear();
            }
            self.cadence.push_back((frame.t, self.steps.total()));
            while self
                .cadence
                .front()
                .is_some_and(|(t, _)| frame.t - t > CADENCE_WINDOW_S)
            {
                self.cadence.pop_front();
            }
        }
        let mut distance_grew = false;
        if let (Some(raw), Some(_)) = (frame.distance_m, self.spec.distance) {
            if !self.distance_seen {
                // licznik pojawił się dopiero teraz - pilnuje dystansu od tej chwili
                self.counter_base_m = self.integral;
                self.at_counter_m = self.integral;
                self.since_counter_m = 0.0;
            }
            distance_grew = self.distance.feed(raw, frame.t) > 0;
            self.distance_seen = true;
        }
        if let (Some(raw), Some(_)) = (frame.elapsed_s, self.spec.elapsed) {
            self.elapsed.feed(raw, frame.t);
        }
        let mut after_gap = false;
        if let Some(kmh) = frame.speed_kmh {
            let kmh = kmh.max(0.0);
            if kmh > MOVING_KMH {
                self.stopped_since = None;
            } else if self.stopped_since.is_none() {
                self.stopped_since = Some(frame.t);
            }
            let point = SpeedPoint {
                t: frame.t,
                kmh,
                elapsed_total: self.elapsed.total(),
                distance_total: self.distance.total(),
            };
            match self.prev {
                // zegar się cofnął albo duplikat - odcinek pomijamy
                Some(p) if frame.t - p.t <= 0.0 => {}
                Some(p) if frame.t - p.t > gap => {
                    after_gap = true;
                    self.fill_gap(p, frame.t - p.t);
                    self.prev = Some(point);
                }
                Some(p) => {
                    let dt = frame.t - p.t;
                    self.integral += p.kmh / 3.6 * dt;
                    self.since_counter_m += p.kmh / 3.6 * dt;
                    if p.kmh > MOVING_KMH {
                        self.moving_s += dt;
                    }
                    self.prev = Some(point);
                }
                None => self.prev = Some(point),
            }
        }
        if self.distance_seen && !self.distance_stuck {
            let c = self.counter_base_m + self.distance.total() as f64;
            let q = self.spec.distance.map_or(0, |d| d.quantum) as f64;
            self.integral = self.integral.clamp((c - q).max(0.0), c + q);
            if after_gap {
                self.integral = self.integral.max(c);
            }
            if distance_grew {
                self.since_counter_m = 0.0;
                self.at_counter_m = self.integral;
            } else if self.since_counter_m > DISTANCE_STUCK_M.max(10.0 * q) {
                // licznik nie rośnie, a taśma jedzie: bez niego, z odzyskaniem ściętej całki
                log::warn!(
                    "Licznik dystansu bieżni stoi od {:.0} m jazdy - liczę dystans z prędkości",
                    self.since_counter_m
                );
                self.distance_stuck = true;
                self.integral = self.integral.max(self.at_counter_m + self.since_counter_m);
            }
        }
    }

    /// Czas ruchu z przerwy w danych: z licznika czasu bieżni albo z przyrostu dystansu.
    fn fill_gap(&mut self, before: SpeedPoint, dt: f64) {
        if self.elapsed.has_value() {
            let ran = self.elapsed.total().saturating_sub(before.elapsed_total) as f64;
            self.moving_s += dt.min(ran);
        } else if before.kmh > MOVING_KMH && self.distance.has_value() {
            let ran = self.distance.total().saturating_sub(before.distance_total) as f64;
            self.moving_s += dt.min(ran / (before.kmh / 3.6));
        }
    }

    /// Kadencja (kroki/min) w chwili `now` (zegar ramek); None, gdy taśma stoi, dane nie są
    /// świeże albo próbek jest za mało.
    pub fn cadence(&self, now: f64) -> Option<f64> {
        let fresh = self.last_data.is_some_and(|t| now - t <= self.spec.gap_s());
        let stopped = self
            .stopped_since
            .is_some_and(|t| now - t >= CADENCE_STOPPED_S);
        if !fresh || stopped {
            return None;
        }
        let (t_new, s_new) = *self.cadence.back()?;
        let (t_old, s_old) = *self.cadence.front()?;
        let span = t_new - t_old;
        (span >= CADENCE_MIN_SPAN_S).then(|| (s_new.saturating_sub(s_old)) as f64 / span * 60.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steps_counter() -> Counter {
        Counter::new(
            "kroków",
            CounterSpec {
                modulus: None,
                quantum: 1,
            },
            STEPS_RATE,
        )
    }

    fn run(counter: &mut Counter, samples: &[(f64, u64)]) -> u64 {
        for (t, raw) in samples {
            counter.feed(*raw, *t);
        }
        counter.total()
    }

    #[test]
    fn counters_like_analysis_c1_to_c8() {
        let cases: [(&[(f64, u64)], u64); 7] = [
            (&[(0.0, 808)], 0),
            (&[(0.0, 808), (1.0, 809), (2.0, 811)], 3),
            (&[(0.0, 5000), (1.0, 3)], 3),
            (&[(0.0, 5000), (1.0, 4990), (2.0, 5002)], 2),
            (&[(0.0, 5000), (1.0, 4000), (2.0, 4001)], 1),
            (&[(0.0, 1000), (1.0, 9000), (2.0, 9002)], 2),
            (&[(0.0, 2400), (50.0, 2460)], 60),
        ];
        for (i, (samples, want)) in cases.iter().enumerate() {
            assert_eq!(run(&mut steps_counter(), samples), *want, "przypadek {i}");
        }
    }

    #[test]
    fn counter_wraps_around_modulus_c4_c10() {
        let mut steps = Counter::new(
            "kroków",
            CounterSpec {
                modulus: Some(65536),
                quantum: 1,
            },
            STEPS_RATE,
        );
        assert_eq!(run(&mut steps, &[(0.0, 65530), (2.0, 4)]), 10);
        let mut distance = Counter::new(
            "dystansu",
            CounterSpec {
                modulus: Some(1 << 24),
                quantum: 10,
            },
            distance_rate(6.0),
        );
        assert_eq!(run(&mut distance, &[(0.0, 16_777_210), (1.0, 5)]), 11);
    }

    #[test]
    fn distance_and_time_counters_c11_c12() {
        let mut distance = Counter::new(
            "dystansu",
            CounterSpec {
                modulus: Some(1 << 24),
                quantum: 10,
            },
            distance_rate(6.0),
        );
        let samples = [(0.0, 390), (1.0, 390), (2.0, 400), (3.0, 400)];
        assert_eq!(run(&mut distance, &samples), 10);
        let mut elapsed = Counter::new(
            "czasu",
            CounterSpec {
                modulus: Some(65536),
                quantum: 1,
            },
            ELAPSED_RATE,
        );
        let samples = [(0.0, 763), (1.0, 764), (2.0, 764), (3.0, 765)];
        assert_eq!(run(&mut elapsed, &samples), 2);
    }

    /// C13: ramki bez kroków nie zerują licznika - pierwsza wartość to punkt odniesienia.
    #[test]
    fn steps_appear_later_c13() {
        let mut meter = Meter::default();
        for i in 0..10 {
            meter.feed(&TelemetryFrame::data(f64::from(i), Some(2.0)));
        }
        assert!(!meter.has_steps());
        for (t, raw) in [(10.0, 120), (11.0, 121)] {
            meter.feed(&TelemetryFrame {
                steps: Some(raw),
                ..TelemetryFrame::data(t, Some(2.0))
            });
        }
        assert!(meter.has_steps());
        assert_eq!(meter.totals().steps, 1);
    }

    /// Bieżnia z polem dystansu, które zawsze ma 0: po 100 m jazdy dystans z samej całki,
    /// bez utraty metrów ściętych przez licznik.
    #[test]
    fn stuck_distance_counter_falls_back_to_speed() {
        let mut meter = Meter::default();
        for i in 0..=200 {
            meter.feed(&TelemetryFrame {
                distance_m: Some(0),
                ..TelemetryFrame::data(f64::from(i), Some(3.6))
            });
        }
        assert!(!meter.has_distance_counter());
        assert!((meter.totals().distance_m - 200.0).abs() < 1e-9);
    }

    /// Licznik dystansu pojawia się w ramkach dopiero po 50 m: dystans się nie cofa.
    #[test]
    fn late_distance_counter_keeps_earlier_distance() {
        let mut meter = Meter::default();
        for i in 0..=50 {
            meter.feed(&TelemetryFrame::data(f64::from(i), Some(3.6)));
        }
        for i in 51..=80 {
            let counted = (f64::from(i - 51) / 10.0).floor() as u64 * 10;
            meter.feed(&TelemetryFrame {
                distance_m: Some(1000 + counted),
                ..TelemetryFrame::data(f64::from(i), Some(3.6))
            });
        }
        assert!(meter.has_distance_counter());
        let d = meter.totals().distance_m;
        assert!((d - 80.0).abs() <= 10.0, "dystans {d}");
        assert!(d >= 70.0);
    }

    #[test]
    fn spec_per_driver() {
        let ftms = TelemetrySpec::for_driver(DriverKind::Ftms, &Capabilities::DEFAULT);
        assert_eq!(ftms.steps.and_then(|s| s.modulus), Some(65536));
        assert_eq!(ftms.distance.and_then(|s| s.modulus), Some(1 << 24));
        let ks = TelemetrySpec::for_driver(
            DriverKind::KingSmith,
            &super::super::kingsmith_proto::DEFAULT_CAPS,
        );
        assert_eq!(ks.gap_s(), 6.0);
        assert_eq!(ftms.gap_s(), 3.0);
    }
}
