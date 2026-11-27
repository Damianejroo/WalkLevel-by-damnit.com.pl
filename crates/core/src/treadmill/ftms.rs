//! Sterownik FTMS (standard Bluetooth Fitness Machine Service, np. Urevo SpaceWalk 5L) na
//! połączeniu GATT (port bieznia/sterowniki/ftms/sterownik.py).
//!
//! Dane co ok. 1 s (Treadmill Data), stan maszyny (Machine Status), polecenia przez Control
//! Point - każde z odpowiedzią. Stop z okna to pauza (bieżnia trzyma sesję), przy zamykaniu
//! aplikacji - pełny stop. Dziwactwa modeli (zakres, odliczanie) opisują profile JSON.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use futures::FutureExt;
use futures::future::BoxFuture;
use tokio::sync::{Notify, oneshot};

use super::contract::{
    Capabilities, Clock, Counts, DeviceInfo, Driver, DriverError, FrameCallback,
    GattCharacteristic, GattClient, GattService, StartResult,
};
use super::ftms_proto::{
    self as proto, CONTROL_POINT, FIRMWARE_REVISION, MACHINE_STATUS, RESULT_NOT_PERMITTED,
    RESULT_OK, SPEED_RANGE, StatusEvent, TREADMILL_DATA,
};
use super::watchdog::SpeedStats;

/// Urevo: pierwszy Start po Stopie bywa przyjęty, a taśma nie rusza.
pub const START_ATTEMPTS: u32 = 2;
pub const STOP_ATTEMPTS: u32 = 3;
/// Słaby sygnał: polecenie albo odpowiedź potrafi przepaść.
pub const SPEED_ATTEMPTS: u32 = 2;
pub const RAW_FRAMES_AFTER_CONNECT: u32 = 40;

/// Czasy oczekiwania (testy je skracają).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timing {
    /// każde polecenie dostaje odpowiedź - tyle na nią czekamy
    pub response_s: f64,
    /// Urevo: po Start taśma rusza po ok. 1-3 s (odliczanie 3-2-1)
    pub start_wait_s: f64,
    /// po przyjętej pauzie taśma hamuje kilka sekund, zanim bieżnia zgłosi „zatrzymana”
    pub stop_wait_s: f64,
    /// tyle, żeby było widać hamowanie w ramkach (co ok. 1 s)
    pub stop_slowing_s: f64,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            response_s: 1.5,
            start_wait_s: 4.0,
            stop_wait_s: 9.0,
            stop_slowing_s: 2.5,
        }
    }
}

fn secs(s: f64) -> Duration {
    Duration::try_from_secs_f64(s.max(0.0)).unwrap_or(Duration::ZERO)
}

/// Polecenie czekające na odpowiedź Control Point.
struct Pending {
    op: u8,
    reply: oneshot::Sender<u8>,
}

/// Bieżnia FTMS na połączeniu GATT (patrz opis modułu).
pub struct FtmsDriver {
    client: Arc<dyn GattClient>,
    on_frame: FrameCallback,
    clock: Clock,
    timing: Timing,
    info: Mutex<DeviceInfo>,
    rx: Mutex<proto::Telemetry>,
    data: Option<GattCharacteristic>,
    control: Option<GattCharacteristic>,
    status: Option<GattCharacteristic>,
    range: Option<GattCharacteristic>,
    firmware: Option<GattCharacteristic>,
    pending: Mutex<Option<Pending>>,
    command_lock: tokio::sync::Mutex<()>,
    speed_stats: Mutex<SpeedStats>,
    /// rośnie, gdy taśma ruszyła / stanęła (dla czekających na start i stop)
    started: AtomicU64,
    stopped: AtomicU64,
    changed: Notify,
    belt_busy: AtomicU32,
    raw_budget: AtomicU32,
}

/// Zdejmuje znacznik „start/stop w toku” także przy przerwaniu.
struct Busy<'a>(&'a AtomicU32);

impl<'a> Busy<'a> {
    fn new(counter: &'a AtomicU32) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self(counter)
    }
}

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl FtmsDriver {
    /// `caps` - możliwości z profilu (bez Control Point: `control = false`).
    pub fn new(
        client: Arc<dyn GattClient>,
        services: &[GattService],
        caps: Capabilities,
        info: DeviceInfo,
        on_frame: FrameCallback,
        clock: Clock,
    ) -> Self {
        let find = |uuid| proto::find_char(services, uuid).cloned();
        let control = find(CONTROL_POINT);
        let caps = Capabilities {
            control: caps.control && control.is_some(),
            ..caps
        };
        Self {
            client,
            on_frame,
            clock,
            timing: Timing::default(),
            info: Mutex::new(info),
            rx: Mutex::new(proto::Telemetry::new(caps)),
            data: find(TREADMILL_DATA),
            control,
            status: find(MACHINE_STATUS),
            range: find(SPEED_RANGE),
            firmware: find(FIRMWARE_REVISION),
            pending: Mutex::new(None),
            command_lock: tokio::sync::Mutex::new(()),
            speed_stats: Mutex::new(SpeedStats::default()),
            started: AtomicU64::new(0),
            stopped: AtomicU64::new(0),
            changed: Notify::new(),
            belt_busy: AtomicU32::new(0),
            raw_budget: AtomicU32::new(RAW_FRAMES_AFTER_CONNECT),
        }
    }

    /// Inne czasy oczekiwania (testy).
    pub fn with_timing(mut self, timing: Timing) -> Self {
        self.timing = timing;
        self
    }

    fn now(&self) -> f64 {
        (self.clock)()
    }

    fn last_speed(&self) -> f64 {
        lock(&self.rx).last_speed.map_or(0.0, |s| s.0)
    }

    /// Prędkość, którą bieżnia naprawdę pojedzie (zakres zgłoszony przez bieżnię).
    pub fn reachable_speed(&self, kmh: f64) -> f64 {
        lock(&self.rx).reachable_speed(kmh)
    }

    /// Polecenie i jego wynik (1 = przyjęte) albo None, gdy bieżnia nie odpowiedziała.
    async fn send(&self, payload: &[u8], what: &str) -> Result<Option<u8>, DriverError> {
        let Some(cp) = &self.control else {
            return Ok(None);
        };
        let Some(op) = payload.first().copied() else {
            return Ok(None);
        };
        let _one_at_a_time = self.command_lock.lock().await;
        let (reply, answer) = oneshot::channel();
        *lock(&self.pending) = Some(Pending { op, reply });
        lock(&self.rx).counts.commands += 1;
        log::debug!(">> FTMS {} ({what})", hex::encode(payload));
        let sent_at = self.now();
        let written = self.client.write(cp, payload, true).await;
        let result = match written {
            Ok(()) => tokio::time::timeout(secs(self.timing.response_s), answer)
                .await
                .ok()
                .and_then(Result::ok),
            Err(e) => {
                lock(&self.pending).take();
                return Err(e);
            }
        };
        lock(&self.pending).take();
        let waited_ms = ((self.now() - sent_at) * 1000.0).max(0.0);
        let Some(result) = result else {
            let (belt, speed) = {
                let rx = lock(&self.rx);
                (rx.running, rx.last_speed.map(|s| s.0))
            };
            log::warn!(
                "[ftms-no-reply] FTMS: brak odpowiedzi na {what} po {waited_ms:.0} ms (taśma {}, ostatnia prędkość {})",
                match belt {
                    Some(true) => "jedzie",
                    Some(false) => "stoi",
                    None => "nieznana",
                },
                speed.map_or_else(|| "?".to_string(), |v| format!("{v:.1} km/h"))
            );
            return Ok(None);
        };
        log::debug!(
            "<< FTMS CP {op:02x}: {} po {waited_ms:.0} ms ({what})",
            proto::result_name(result).map_or_else(|| format!("wynik {result}"), String::from)
        );
        let mut rx = lock(&self.rx);
        if result == RESULT_OK {
            rx.counts.acks += 1;
        } else {
            rx.counts.refused += 1;
            log::warn!(
                "FTMS: {what} - {}",
                proto::result_name(result).map_or_else(|| format!("wynik {result}"), String::from)
            );
        }
        Ok(Some(result))
    }

    async fn take_control(&self) -> Result<(), DriverError> {
        let got = self
            .send(&proto::encode_request_control(), "przejęcie kontroli")
            .await?;
        lock(&self.rx).has_control = got == Some(RESULT_OK);
        Ok(())
    }

    /// Polecenie z przejęciem kontroli, gdy bieżnia jej jeszcze nie oddała.
    async fn command(&self, payload: &[u8], what: &str) -> Result<Option<u8>, DriverError> {
        if !lock(&self.rx).has_control {
            self.take_control().await?;
        }
        let mut result = self.send(payload, what).await?;
        if result == Some(RESULT_NOT_PERMITTED) {
            self.take_control().await?;
            if lock(&self.rx).has_control {
                result = self.send(payload, what).await?;
            }
        }
        Ok(result)
    }

    /// Czeka, aż licznik `seq` przekroczy `start` (albo minie czas).
    async fn wait_seq(&self, seq: &AtomicU64, start: u64, timeout_s: f64) -> bool {
        let deadline = tokio::time::Instant::now() + secs(timeout_s);
        loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if seq.load(Ordering::SeqCst) > start {
                return true;
            }
            if tokio::time::timeout_at(deadline, notified).await.is_err() {
                return seq.load(Ordering::SeqCst) > start;
            }
        }
    }

    async fn start_inner(&self) -> Result<(), DriverError> {
        if let Some(range) = &self.range
            && let Ok(raw) = self.client.read(range).await
            && let Some((lo, hi, step)) = proto::decode_speed_range(&raw)
        {
            let mut rx = lock(&self.rx);
            rx.capabilities = rx.capabilities.with_range(lo, hi, step);
        }
        if let Some(fw) = &self.firmware
            && let Ok(raw) = self.client.read(fw).await
        {
            let firmware = proto::decode_firmware(&raw);
            if !firmware.is_empty() {
                lock(&self.info).firmware = firmware;
            }
        }
        {
            let caps = lock(&self.rx).capabilities;
            let info = lock(&self.info);
            log::info!(
                "FTMS: {}{}, prędkość {:.1}-{:.1} km/h co {:.1}",
                info.model,
                if info.firmware.is_empty() {
                    String::new()
                } else {
                    format!(" (firmware {})", info.firmware)
                },
                caps.speed_min,
                caps.speed_max,
                caps.speed_step
            );
        }
        if let Some(cp) = &self.control {
            self.client.subscribe(cp).await?;
        }
        if let Some(status) = &self.status {
            self.client.subscribe(status).await?;
        }
        match &self.data {
            Some(data) => self.client.subscribe(data).await,
            None => Err(DriverError::UnsupportedDevice),
        }
    }

    async fn start_belt_inner(&self) -> Result<StartResult, DriverError> {
        let began = self.now();
        for attempt in 1..=START_ATTEMPTS {
            let seq = self.started.load(Ordering::SeqCst);
            let Some(result) = self.command(&proto::encode_start(), "start").await? else {
                continue;
            };
            if result != RESULT_OK {
                return Ok(StartResult::Refused);
            }
            self.wait_seq(&self.started, seq, self.timing.start_wait_s)
                .await;
            if lock(&self.rx).running == Some(true) {
                log::info!(
                    "[belt-start-ok] Start: przyjęty po {:.1} s (próba {attempt})",
                    self.now() - began
                );
                return Ok(StartResult::Started);
            }
            log::info!("Start przyjęty, ale taśma nie ruszyła - ponawiam");
        }
        Ok(StartResult::Unknown)
    }

    /// Bezpieczeństwo przede wszystkim: dopóki bieżnia nie zgłosi, że stoi, ponawiamy -
    /// kolejne próby to już pełny stop (Urevo potrafi zgubić polecenie albo odpowiedź).
    async fn stop_belt_inner(&self, verify: bool) -> Result<bool, DriverError> {
        let began = self.now();
        let can_pause = lock(&self.rx).capabilities.pause;
        for attempt in 1..=STOP_ATTEMPTS {
            let seq = self.stopped.load(Ordering::SeqCst);
            let pause = verify && attempt == 1 && can_pause;
            let payload = if pause {
                proto::encode_pause()
            } else {
                proto::encode_stop()
            };
            let before = self.last_speed();
            let what = if pause { "pauza" } else { "stop" };
            let result = self.command(&payload, what).await?;
            if result == Some(RESULT_OK) && self.stopping(before, seq).await {
                log::info!(
                    "[belt-stop-ok] Stop: przyjęty po {:.1} s (próba {attempt})",
                    self.now() - began
                );
                return Ok(true);
            }
            log::warn!("Stop bez potwierdzenia bieżni - ponawiam pełnym stopem ({attempt})");
        }
        log::error!("Bieżnia nie potwierdziła stopu - zatrzymaj ją pilotem albo kluczem");
        Ok(false)
    }

    /// Po przyjętym stopie: czekamy na „zatrzymana”, dopóki taśma zwalnia.
    async fn stopping(&self, before: f64, seq: u64) -> bool {
        let start = self.now();
        while self.now() - start < self.timing.stop_wait_s {
            if self.wait_seq(&self.stopped, seq, 0.25).await
                || lock(&self.rx).running == Some(false)
            {
                return true;
            }
            if self.now() - start >= self.timing.stop_slowing_s {
                let speed = self.last_speed();
                if speed <= 0.0 {
                    // taśma stoi - zgłoszenie stanu przyjdzie za chwilę
                    lock(&self.rx).running = Some(false);
                    return true;
                }
                if speed >= before - 0.05 {
                    return false; // przyjęte, a taśma nie zwalnia - ponawiamy
                }
            }
        }
        false
    }

    /// Prędkość w zakresie bieżni - poza nim Urevo w ogóle nie odpowiada (0,5 i 0,6 km/h
    /// bez odpowiedzi), więc niżej niż minimum nie wysyłamy. Bez odpowiedzi - jeszcze raz.
    async fn set_speed_inner(&self, kmh: f64) -> Result<bool, DriverError> {
        let target = self.reachable_speed(kmh);
        let Ok(payload) = proto::encode_set_speed(target) else {
            return Ok(false);
        };
        lock(&self.speed_stats).sent += 1;
        let began = self.now();
        for attempt in 0..SPEED_ATTEMPTS {
            if attempt > 0 {
                lock(&self.speed_stats).resends += 1;
            }
            let what = format!("prędkość {target:.1}");
            match self.command(&payload, &what).await? {
                None => continue,
                Some(RESULT_OK) => {
                    let took = self.now() - began;
                    let mut stats = lock(&self.speed_stats);
                    stats.ok += 1;
                    stats.latencies.push(took);
                    return Ok(true);
                }
                Some(_) => break,
            }
        }
        lock(&self.speed_stats).failed += 1;
        Ok(false)
    }
}

impl Driver for FtmsDriver {
    fn protocol(&self) -> &'static str {
        "FTMS"
    }

    fn capabilities(&self) -> Capabilities {
        lock(&self.rx).capabilities
    }

    fn info(&self) -> DeviceInfo {
        lock(&self.info).clone()
    }

    fn counts(&self) -> Counts {
        lock(&self.rx).counts
    }

    fn speed_stats(&self) -> Option<SpeedStats> {
        Some(lock(&self.speed_stats).clone())
    }

    fn start(&self) -> BoxFuture<'_, Result<(), DriverError>> {
        self.start_inner().boxed()
    }

    fn poll(&self) -> BoxFuture<'_, Result<(), DriverError>> {
        async { Ok(()) }.boxed() // bieżnia sama wysyła dane co ok. 1 s
    }

    fn poll_interval(&self) -> f64 {
        1.0
    }

    fn describe(&self) -> String {
        format!("{} (FTMS)", lock(&self.info).model)
    }

    fn start_belt(&self) -> BoxFuture<'_, Result<StartResult, DriverError>> {
        async move {
            if lock(&self.rx).running == Some(true) {
                return Ok(StartResult::Started);
            }
            let _busy = Busy::new(&self.belt_busy);
            self.start_belt_inner().await
        }
        .boxed()
    }

    fn stop_belt(&self, verify: bool) -> BoxFuture<'_, Result<bool, DriverError>> {
        async move {
            let _busy = Busy::new(&self.belt_busy);
            self.stop_belt_inner(verify).await
        }
        .boxed()
    }

    fn set_speed(&self, kmh: f64) -> BoxFuture<'_, Result<bool, DriverError>> {
        self.set_speed_inner(kmh).boxed()
    }

    fn belt_speed(&self) -> Option<(f64, f64)> {
        lock(&self.rx).last_speed
    }

    fn belt_running(&self) -> Option<bool> {
        lock(&self.rx).running
    }

    fn busy(&self) -> bool {
        self.belt_busy.load(Ordering::SeqCst) > 0
    }

    fn on_notification(&self, characteristic_uuid: &str, data: &[u8]) {
        let is = |c: &Option<GattCharacteristic>| {
            c.as_ref()
                .is_some_and(|c| c.uuid.eq_ignore_ascii_case(characteristic_uuid))
        };
        if is(&self.data) {
            // budżet surowych ramek (wyścig o jedną linię logu nie szkodzi)
            if self.raw_budget.load(Ordering::SeqCst) > 0 {
                self.raw_budget.fetch_sub(1, Ordering::SeqCst);
                log::debug!("<< FTMS {}", hex::encode(data));
            }
            let now = self.now();
            let frame = lock(&self.rx).on_data(data, now);
            if let Some(frame) = frame {
                if frame.speed_kmh.unwrap_or(0.0) > 0.0 {
                    self.started.fetch_add(1, Ordering::SeqCst);
                }
                self.changed.notify_waiters();
                (self.on_frame)(frame);
            }
        } else if is(&self.status) {
            let event = lock(&self.rx).on_status(data);
            match event {
                Some(StatusEvent::Started) => {
                    self.started.fetch_add(1, Ordering::SeqCst);
                }
                Some(StatusEvent::Stopped { .. }) => {
                    self.stopped.fetch_add(1, Ordering::SeqCst);
                }
                _ => {}
            }
            self.changed.notify_waiters();
        } else if is(&self.control)
            && let Some((op, result)) = proto::parse_response(data)
        {
            let mut pending = lock(&self.pending);
            if pending.as_ref().is_some_and(|p| p.op == op)
                && let Some(p) = pending.take()
            {
                let _ = p.reply.send(result); // czekający mógł już zrezygnować
            }
        }
    }
}
