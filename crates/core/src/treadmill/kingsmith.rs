//! Sterownik KingSmith (WalkingPad R2 / X21 / G1 / X23) na połączeniu GATT (port
//! bieznia/sterowniki/kingsmith/sterownik.py).
//!
//! Bieżnia nie potwierdza poleceń - czy posłuchała, widać w ramkach z danymi. Dlatego start
//! i stop sprawdzamy w danych i ponawiamy, a zadanej prędkości pilnuje [`SpeedGuard`].
//! Kodowanie i odbiór ramek: [`super::kingsmith_proto`].

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::FutureExt;
use futures::future::BoxFuture;
use tokio::sync::Notify;

use super::contract::{
    Capabilities, Clock, Counts, DeviceInfo, Driver, DriverError, FrameCallback, GattClient,
    StartResult,
};
use super::kingsmith_proto::{
    self as proto, CMD_CONTROL, CMD_START, CMD_STOP, Channel, GET_PROPS, PROPS_TIMEOUT_S,
    TELEMETRY_PROPS,
};
use super::watchdog::{BrakeWatch, Braking, SpeedGuard, SpeedStats};

pub const START_ATTEMPTS: u32 = 4;
pub const STOP_ATTEMPTS: u32 = 3;
/// Gdy bieżnia sama wysyła dane (subskrypcja KS Fit, co ok. 2 s), odpytujemy rzadziej.
pub const CALM_POLL_S: f64 = 2.0;
/// ControlMode i runState jedno po drugim - drugie potrafi przepaść.
pub const BUSY_POLL_S: f64 = 0.75;
/// Surowy zapis szesnastkowy tylko na początku połączenia (wykrywanie tablicy szyfru).
pub const RAW_FRAMES_AFTER_CONNECT: u32 = 40;
/// Ile czekamy na prędkość po przejściu na subskrypcję KS Fit, zanim wrócimy do listy z QZ.
pub const TELEMETRY_FALLBACK_S: f64 = 5.0;
/// Co ile sekund zapisać do logu, jak często bieżnia podaje prędkość.
pub const STATS_EVERY_S: f64 = 30.0;

/// Odstępy i czasy oczekiwania (testy je skracają).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timing {
    /// odstęp po każdym fragmencie i po całej wiadomości (jak KS Fit)
    pub fragment_gap_s: f64,
    pub line_gap_s: f64,
    /// czekanie na odpowiedź przy rozpoznawaniu
    pub reply_wait_s: f64,
    pub first_reply_s: f64,
    /// werdykt: pierwsza ramka co najmniej tyle po poleceniu, najpóźniej po `verdict_max_s`
    pub verdict_frame_s: f64,
    pub verdict_max_s: f64,
    pub control_gap_s: f64,
    /// X21 gubi polecenia wysłane tuż po jego własnej ramce z danymi
    pub quiet_after_push_s: f64,
    /// stop: prędkość nie spada od tylu sekund - ponowienie; po `brake_max_s` wciąż jedzie - alarm
    pub brake_no_drop_s: f64,
    pub brake_max_s: f64,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            fragment_gap_s: 0.03,
            line_gap_s: 0.06,
            reply_wait_s: 0.6,
            first_reply_s: 1.5,
            verdict_frame_s: 1.0,
            verdict_max_s: 2.6,
            control_gap_s: 0.4,
            quiet_after_push_s: 0.4,
            brake_no_drop_s: super::watchdog::BRAKE_NO_DROP_S,
            brake_max_s: super::watchdog::BRAKE_MAX_S,
        }
    }
}

fn secs(s: f64) -> Duration {
    Duration::try_from_secs_f64(s.max(0.0)).unwrap_or(Duration::ZERO)
}

/// Stan odbioru i sterowania (pod jednym zamkiem, nigdy przez `.await`).
struct Rx {
    t: proto::Telemetry,
    guard: SpeedGuard,
    unanswered_polls: u32,
    poll_command: &'static str,
    telemetry_since: f64,
    props_times: Vec<f64>,
    stats_at: f64,
    raw_budget: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Ok,
    Error,
    No,
}

/// WalkingPad na połączeniu GATT (patrz opis modułu).
pub struct KingSmithDriver {
    client: Arc<dyn GattClient>,
    channels: Vec<Channel>,
    on_frame: FrameCallback,
    clock: Clock,
    caps: Capabilities,
    info: DeviceInfo,
    timing: Timing,
    rx: Mutex<Rx>,
    write_lock: Arc<tokio::sync::Mutex<()>>,
    changed: Notify,
    belt_busy: AtomicU32,
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

impl KingSmithDriver {
    /// `order` - kolejność tablic szyfru ([`proto::Telemetry::order_for`]).
    pub fn new(
        client: Arc<dyn GattClient>,
        channels: Vec<Channel>,
        order: &[&str],
        caps: Capabilities,
        info: DeviceInfo,
        on_frame: FrameCallback,
        clock: Clock,
    ) -> Self {
        Self {
            client,
            channels,
            on_frame,
            clock,
            caps,
            info,
            timing: Timing::default(),
            rx: Mutex::new(Rx {
                t: proto::Telemetry::new(order),
                guard: SpeedGuard::new(),
                unanswered_polls: 0,
                poll_command: GET_PROPS,
                telemetry_since: 0.0,
                props_times: Vec::new(),
                stats_at: 0.0,
                raw_budget: RAW_FRAMES_AFTER_CONNECT,
            }),
            write_lock: Arc::new(tokio::sync::Mutex::new(())),
            changed: Notify::new(),
            belt_busy: AtomicU32::new(0),
        }
    }

    /// Inne odstępy (testy).
    pub fn with_timing(mut self, timing: Timing) -> Self {
        self.timing = timing;
        self
    }

    fn rx(&self) -> MutexGuard<'_, Rx> {
        self.rx
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn now(&self) -> f64 {
        (self.clock)()
    }

    /// Tablice szyfru, które wciąż pasują do odpowiedzi bieżni.
    pub fn table_candidates(&self) -> Vec<&'static str> {
        self.rx().t.codec.candidates().to_vec()
    }

    /// Tablica, którą szyfrujemy wysyłane wiadomości.
    pub fn table(&self) -> &'static str {
        self.rx().t.codec.table()
    }

    /// Kanał (UUID usługi), którym bieżnia odpowiada.
    pub fn active_channel(&self) -> Option<String> {
        self.rx().t.active.clone()
    }

    /// Bieżące polecenie odpytywania (QZ albo subskrypcja KS Fit).
    pub fn poll_command(&self) -> &'static str {
        self.rx().poll_command
    }

    /// Ostatni błąd zgłoszony przez bieżnię (np. -5000 = odmowa).
    pub fn last_error(&self) -> Option<i64> {
        self.rx().t.last_error
    }

    /// Wynik z ramek: tryb sterowania (1 = aplikacja steruje).
    pub fn control_mode(&self) -> Option<i64> {
        self.rx().t.control_mode
    }

    // --- wysyłanie ----------------------------------------------------------

    /// Jedna wiadomość: szyfrowanie, kawałki po 16 bajtów, odstępy jak w KS Fit.
    pub async fn send(&self, text: &str) -> Result<(), DriverError> {
        let (data, targets) = {
            let mut rx = self.rx();
            let data = rx.t.codec.encode(text);
            let poll = text.starts_with("servers getProp");
            if poll {
                rx.t.counts.polls += 1;
            } else {
                rx.t.counts.commands += 1;
            }
            if rx.raw_budget > 0 {
                rx.raw_budget -= 1;
                log::debug!(
                    ">> {}  [{}] {}",
                    if text.is_empty() { "(pusta)" } else { text },
                    rx.t.codec.table(),
                    hex::encode(&data)
                );
            } else if !poll {
                log::debug!(">> {text}");
            }
            let active = rx.t.active.clone();
            let targets: Vec<Channel> = self
                .channels
                .iter()
                .filter(|ch| active.as_ref().is_none_or(|a| *a == ch.service_uuid))
                .cloned()
                .collect();
            (data, targets)
        };
        // całe wiadomości nie mogą się przeplatać
        let write = self.write_lock.clone().lock_owned().await;
        let client = self.client.clone();
        let timing = self.timing;
        // A cancelled caller (e.g. a speed send interrupted by Stop) must not leave half a
        // message on the treadmill, or the next command would be garbled.
        let job = tokio::spawn(async move {
            let _write = write;
            for channel in targets {
                let with_response = !channel
                    .write
                    .properties
                    .iter()
                    .any(|p| p == "write-without-response");
                for chunk in proto::chunks(&data, proto::CHUNK) {
                    client.write(&channel.write, chunk, with_response).await?;
                    tokio::time::sleep(secs(timing.fragment_gap_s)).await;
                }
                tokio::time::sleep(secs(timing.line_gap_s)).await;
            }
            Ok(())
        });
        job.await
            .unwrap_or_else(|e| Err(DriverError::Gatt(e.to_string())))
    }

    /// Wysyła wiadomość i czeka na odszyfrowaną odpowiedź najwyżej `timeout_s` (liczy się
    /// też odpowiedź, która przyszła jeszcze w trakcie wysyłania).
    async fn ask(&self, text: &str, timeout_s: f64) -> Result<bool, DriverError> {
        let start = self.rx().t.replies;
        self.send(text).await?;
        Ok(self.wait_until(timeout_s, |rx| rx.t.replies > start).await)
    }

    /// Czeka, aż warunek na stanie odbioru będzie spełniony (albo minie czas).
    async fn wait_until(&self, timeout_s: f64, done: impl Fn(&Rx) -> bool) -> bool {
        let deadline = tokio::time::Instant::now() + secs(timeout_s);
        loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if done(&self.rx()) {
                return true;
            }
            if tokio::time::timeout_at(deadline, notified).await.is_err() {
                return done(&self.rx());
            }
        }
    }

    /// Chwila ciszy po ramce z danymi - polecenia wysłane tuż po niej X21 często gubi.
    async fn quiet(&self) {
        let since_push = self.now() - self.rx().t.last_props_at;
        if (0.0..self.timing.quiet_after_push_s).contains(&since_push) {
            tokio::time::sleep(secs(self.timing.quiet_after_push_s - since_push)).await;
        }
    }

    /// Ok (runState == want), Error (odmowa) albo No (świeża ramka bez zmiany).
    async fn verdict(&self, sent_at: f64, want: i64) -> Verdict {
        let deadline = sent_at + self.timing.verdict_max_s;
        let frame_after = sent_at + self.timing.verdict_frame_s;
        let decide = |rx: &Rx| {
            if rx.t.run_state == Some(want) {
                Some(Verdict::Ok)
            } else if rx.t.last_error.is_some() {
                Some(Verdict::Error)
            } else if rx.t.last_props_at >= frame_after {
                Some(Verdict::No)
            } else {
                None
            }
        };
        loop {
            if let Some(v) = decide(&self.rx()) {
                return v;
            }
            let remaining = deadline - self.now();
            if remaining <= 0.0 {
                return Verdict::No;
            }
            self.wait_until(remaining, |rx| decide(rx).is_some()).await;
        }
    }

    async fn send_speed(&self, kmh: f64) -> Result<(), DriverError> {
        let command = proto::speed_command(kmh);
        self.quiet().await;
        self.send(&command).await?;
        log::info!(
            "Prędkość taśmy: {}",
            command.rsplit(' ').next().unwrap_or("")
        );
        Ok(())
    }

    fn log_stats(&self, rx: &mut Rx, now: f64) {
        if now - rx.stats_at < STATS_EVERY_S {
            return;
        }
        rx.stats_at = now;
        rx.props_times.retain(|t| now - t <= STATS_EVERY_S);
        if rx.props_times.len() < 2 {
            return;
        }
        let mut gaps: Vec<f64> = rx.props_times.windows(2).map(|w| w[1] - w[0]).collect();
        gaps.sort_by(f64::total_cmp);
        let typical = gaps.get(gaps.len() / 2).copied().unwrap_or(0.0);
        let worst = gaps.last().copied().unwrap_or(0.0);
        log::info!(
            "[telemetry-summary] Telemetria ({}): {} ramek z danymi w {:.0} s, typowo co {typical:.2} s, najdłużej \
             {worst:.2} s · zapytania {}, odpowiedzi {}, nieodszyfrowane {}",
            if rx.poll_command == TELEMETRY_PROPS {
                "KS Fit"
            } else {
                "QZ"
            },
            rx.props_times.len(),
            STATS_EVERY_S,
            rx.t.counts.polls,
            rx.t.counts.acks,
            rx.t.counts.undecoded,
        );
    }

    async fn start_inner(&self) -> Result<(), DriverError> {
        for channel in &self.channels {
            self.client.subscribe(&channel.notify).await?;
        }
        // Pusta wiadomość nie zależy od tablicy - odpowiedź („format error”) od razu mówi,
        // którą tablicą szyfruje ta bieżnia.
        self.ask("", self.timing.first_reply_s).await?;
        let now_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        for command in proto::init_commands(now_unix) {
            self.ask(&command, self.timing.reply_wait_s).await?;
        }
        // Odczyt zawiera znaki, którymi tablice się różnią - jeśli bieżnia milczy, od razu
        // próbujemy kolejnej z pozostałych tablic.
        let candidates = self.rx().t.codec.candidates().len();
        for _ in 0..candidates {
            if self.ask(GET_PROPS, self.timing.reply_wait_s * 2.0).await? {
                break;
            }
            self.rx().t.codec.try_next_table();
        }
        // Próba szybszej telemetrii (jak KS Fit). Gdy prędkość przestanie przychodzić,
        // poll() sam wróci do listy z QZ, która na X21 działa na pewno.
        {
            let now = self.now();
            let mut rx = self.rx();
            rx.poll_command = TELEMETRY_PROPS;
            rx.telemetry_since = now;
            rx.stats_at = now;
        }
        self.send(TELEMETRY_PROPS).await
    }

    async fn poll_inner(&self) -> Result<(), DriverError> {
        let command = {
            let now = self.now();
            let mut rx = self.rx();
            if rx.unanswered_polls >= 3 && !rx.t.codec.locked() {
                rx.t.codec.try_next_table();
                log::info!(
                    "Brak odpowiedzi - próbuję tablicy szyfrowania {}",
                    rx.t.codec.table()
                );
                rx.unanswered_polls = 0;
            }
            rx.unanswered_polls += 1;
            if rx.poll_command == TELEMETRY_PROPS
                && now - rx.telemetry_since > TELEMETRY_FALLBACK_S
                && now - rx.t.last_props_at > TELEMETRY_FALLBACK_S
            {
                log::warn!("Subskrypcja KS Fit nie daje prędkości - wracam do odpytywania z QZ");
                rx.poll_command = GET_PROPS;
            }
            self.log_stats(&mut rx, now);
            rx.poll_command
        };
        self.send(command).await?;
        if !self.busy() {
            let resend = {
                let now = self.now();
                let mut rx = self.rx();
                let belt = rx.t.belt_speed();
                let caps = self.caps;
                rx.guard.check(now, belt, |v| caps.clamp(v))
            };
            if let Some(kmh) = resend {
                let ok = self.send_speed(kmh).await.is_ok();
                let now = self.now();
                self.rx().guard.sent(ok, now);
            }
        }
        Ok(())
    }

    async fn start_belt_inner(&self) -> Result<StartResult, DriverError> {
        if self.rx().t.run_state == Some(1) {
            log::info!("Taśma już jedzie - nie wysyłam startu");
            return Ok(StartResult::Started);
        }
        let began = self.now();
        for attempt in 1..=START_ATTEMPTS {
            self.rx().t.last_error = None;
            self.quiet().await;
            // jak QZ: przejęcie sterowania tylko, gdy potrzebne
            if self.rx().t.control_mode != Some(1) {
                self.send(CMD_CONTROL).await?;
                tokio::time::sleep(secs(self.timing.control_gap_s)).await;
            }
            self.send(CMD_START).await?;
            match self.verdict(self.now(), 1).await {
                Verdict::Ok => {
                    log::info!(
                        "[belt-start-ok] Start: przyjęty po {:.1} s (próba {attempt})",
                        self.now() - began
                    );
                    return Ok(StartResult::Started);
                }
                Verdict::Error => {
                    let mut rx = self.rx();
                    log::warn!(
                        "Bieżnia odmówiła startu (kod {:?}), próba {attempt}",
                        rx.t.last_error
                    );
                    rx.t.control_mode = None; // następna próba z przejęciem sterowania
                }
                Verdict::No => {
                    log::info!(
                        "[belt-no-reaction] Bieżnia nie zareagowała na start (próba {attempt}) - ponawiam"
                    );
                }
            }
        }
        let refused = self.rx().t.last_error.is_some();
        Ok(if refused {
            StartResult::Refused
        } else {
            StartResult::Unknown // brak odpowiedzi - prędkość pokaże, czy taśma jedzie
        })
    }

    /// Stop. Potwierdzeniem jest runState 0 albo SPADAJĄCA prędkość: X21C hamuje stopniami
    /// (3,5 › 3,0 › 2,0 › 1,0) z runState 1 do końca - wtedy czekamy spokojnie ([`BrakeWatch`]).
    /// Stop idzie jeszcze raz, gdy prędkość nie spada od ok. 3 s; alarm (false) dopiero, gdy
    /// po ponowieniach dalej nie spada albo taśma po ok. 15 s wciąż jedzie.
    async fn stop_belt_inner(&self, verify: bool) -> Result<bool, DriverError> {
        let mut watch = BrakeWatch::new(self.now(), self.rx().t.belt_speed())
            .with_limits(self.timing.brake_no_drop_s, self.timing.brake_max_s);
        for attempt in 1..=STOP_ATTEMPTS {
            self.rx().t.last_error = None;
            self.send(CMD_STOP).await?;
            if !verify {
                log::info!("Wysłano stop taśmy");
                return Ok(true);
            }
            watch.resent(self.now());
            loop {
                let state = {
                    let now = self.now();
                    let rx = self.rx();
                    if let Some((kmh, at)) = rx.t.belt_speed() {
                        watch.feed(kmh, at);
                    }
                    if rx.t.last_error.is_some() {
                        Braking::Stuck // odmowa - ponawiamy jak przy braku reakcji
                    } else {
                        watch.state(now, rx.t.belt_running())
                    }
                };
                match state {
                    Braking::Stopped => {
                        log::info!(
                            "[belt-stop-ok] Stop: hamowanie {} (próba {attempt})",
                            watch.describe(self.now())
                        );
                        return Ok(true);
                    }
                    Braking::TooLong => {
                        log::warn!(
                            "Bieżnia nie potwierdziła stopu: {}",
                            watch.describe(self.now())
                        );
                        return Ok(false);
                    }
                    Braking::Stuck => break,
                    Braking::Slowing => {
                        let seen = self.rx().t.last_props_at;
                        self.wait_until(0.5, |rx| rx.t.last_props_at > seen).await;
                    }
                }
            }
            log::warn!(
                "Bieżnia nie zwalnia po stopie (próba {attempt}): {} - ponawiam",
                watch.describe(self.now())
            );
            self.quiet().await;
        }
        log::warn!(
            "Bieżnia nie potwierdziła stopu: {}",
            watch.describe(self.now())
        );
        Ok(false)
    }
}

impl Driver for KingSmithDriver {
    fn protocol(&self) -> &'static str {
        "KingSmith"
    }

    fn capabilities(&self) -> Capabilities {
        self.caps
    }

    fn info(&self) -> DeviceInfo {
        let firmware = self.rx().t.firmware.clone();
        DeviceInfo {
            firmware,
            ..self.info.clone()
        }
    }

    fn counts(&self) -> Counts {
        self.rx().t.counts
    }

    fn speed_stats(&self) -> Option<SpeedStats> {
        Some(self.rx().guard.stats.clone())
    }

    fn start(&self) -> BoxFuture<'_, Result<(), DriverError>> {
        self.start_inner().boxed()
    }

    fn poll(&self) -> BoxFuture<'_, Result<(), DriverError>> {
        self.poll_inner().boxed()
    }

    fn poll_interval(&self) -> f64 {
        let now = self.now();
        let rx = self.rx();
        let pushing = now - rx.t.last_props_at < PROPS_TIMEOUT_S / 2.0;
        if rx.poll_command == TELEMETRY_PROPS && pushing {
            CALM_POLL_S
        } else {
            BUSY_POLL_S
        }
    }

    fn describe(&self) -> String {
        let rx = self.rx();
        let channel =
            rx.t.active
                .as_deref()
                .map_or("?", |a| a.get(..8).unwrap_or(a));
        format!(
            "{} (KingSmith, usługa {channel}, tablica {})",
            self.info.model,
            rx.t.codec.describe()
        )
    }

    fn start_belt(&self) -> BoxFuture<'_, Result<StartResult, DriverError>> {
        async move {
            let _busy = Busy::new(&self.belt_busy);
            self.start_belt_inner().await
        }
        .boxed()
    }

    fn stop_belt(&self, verify: bool) -> BoxFuture<'_, Result<bool, DriverError>> {
        self.rx().guard.reset();
        async move {
            let _busy = Busy::new(&self.belt_busy);
            self.stop_belt_inner(verify).await
        }
        .boxed()
    }

    /// Prędkość docelowa - wysyłana od razu, a w poll() pilnowana (X21 gubi polecenia).
    fn set_speed(&self, kmh: f64) -> BoxFuture<'_, Result<bool, DriverError>> {
        async move {
            let kmh = {
                let now = self.now();
                let mut rx = self.rx();
                let belt = rx.t.belt_speed();
                // bieżnia w milach: polecenie i pilnowanie w jej jednostkach
                let native = rx.t.native_speed(kmh);
                rx.guard.set(native, now, belt)
            };
            let sent = self.send_speed(kmh).await;
            let now = self.now();
            self.rx().guard.sent(sent.is_ok(), now);
            sent.map(|()| true)
        }
        .boxed()
    }

    /// In km/h like the other drivers; treadmill units stay inside [`SpeedGuard`].
    fn belt_speed(&self) -> Option<(f64, f64)> {
        let rx = self.rx();
        rx.t.belt_speed().map(|(_, at)| (rx.t.speed_kmh(), at))
    }

    fn belt_running(&self) -> Option<bool> {
        self.rx().t.belt_running()
    }

    fn busy(&self) -> bool {
        self.belt_busy.load(Ordering::SeqCst) > 0
    }

    fn on_notification(&self, characteristic_uuid: &str, data: &[u8]) {
        let Some(channel) = self
            .channels
            .iter()
            .find(|ch| ch.notify.uuid.eq_ignore_ascii_case(characteristic_uuid))
        else {
            return;
        };
        let now = self.now();
        let frames = {
            let mut rx = self.rx();
            if rx.raw_budget > 0 {
                rx.raw_budget -= 1;
                log::debug!("<< [{}] {}", channel.label(), hex::encode(data));
            }
            let replies = rx.t.replies;
            let frames = rx.t.feed(&channel.service_uuid, data, now);
            if rx.t.replies > replies {
                rx.unanswered_polls = 0;
            }
            for frame in &frames {
                if frame.is_data() {
                    rx.props_times.push(now);
                }
            }
            frames
        };
        self.changed.notify_waiters();
        for frame in frames {
            (self.on_frame)(frame); // poza zamkiem - odbiorca może pytać sterownik o stan
        }
    }
}
