//! Bluetooth na runtime tokio: skanowanie, połączenie z zapamiętaną bieżnią z ponawianiem,
//! polecenia taśmy, symulator zamiennie z bieżnią (port bieznia/ble/worker.py).
//!
//! Interfejs wywołuje metody [`BleWorker`] (nie blokują) i odbiera [`WorkerEvent`] z kanału:
//! stan połączenia, wyniki skanu, opis bieżni, pełne ramki telemetrii, wyniki poleceń
//! taśmy i komunikaty o problemach (po polsku, docs/teksty.md). Worker nie zna modeli
//! bieżni - rozmawia ze sterownikiem przez kontrakt ([`Driver`]).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;

use super::ble::{BleBackend, BleError, BleLink, run_diagnostics};
use super::kontrakt::{
    Capabilities, Clock, DeviceInfo, Driver, DriverError, FrameCallback, GattClient, StartResult,
};
use super::profiles::ProfileRegistry;
use super::simulator::{self, SimulatorDriver};
use super::telemetry::{TelemetryFrame, TelemetrySpec};
use super::watchdog::SpeedStats;
use crate::teksty::t;

/// Tyle razy prędkość bez potwierdzenia idzie od nowa (co odpytanie).
pub const SPEED_ATTEMPTS: u32 = 3;
/// Tyle nieudanych odpytań z rzędu (np. zapis GATT do bieżni, której już nie ma) i łączymy
/// od nowa - w 0.5 każdy błąd odpytania kończył połączenie.
pub const POLL_FAILURES: u32 = 3;

/// Stan połączenia z bieżnią (wartości jak w 0.5, w JSON-ie camelCase).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Link {
    /// nic nie robimy
    Idle,
    /// skan albo szukanie zapamiętanej bieżni
    Searching,
    Connecting,
    /// połączono, rozpoznajemy bieżnię
    Connected,
    /// dane płyną
    Streaming,
    Retrying,
    BluetoothOff,
    Unsupported,
}

/// Urządzenie z listy skanu (bieżnie pierwsze). Pusta nazwa = urządzenie bez nazwy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundDevice {
    pub address: String,
    pub name: String,
    pub rssi: i16,
    pub treadmill: bool,
    /// czytelny model (`model_name`) - gdy równy nazwie, UI pisze „Bieżnia”
    pub model: String,
}

/// Polecenie taśmy z okna.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BeltCommand {
    Start,
    Stop,
}

/// Wynik polecenia taśmy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BeltOutcome {
    Started,
    Refused,
    Unknown,
    Unsupported,
    Stopped,
    NotConnected,
}

impl BeltOutcome {
    /// "started" | "refused" | "unknown" | "unsupported" | "stopped" | "not_connected"
    pub fn as_str(self) -> &'static str {
        match self {
            BeltOutcome::Started => "started",
            BeltOutcome::Refused => "refused",
            BeltOutcome::Unknown => "unknown",
            BeltOutcome::Unsupported => "unsupported",
            BeltOutcome::Stopped => "stopped",
            BeltOutcome::NotConnected => "not_connected",
        }
    }

    /// Komunikat dla użytkownika (docs/teksty.md, tasma.*).
    pub fn message(self) -> String {
        let key = match self {
            BeltOutcome::Started => "tasma.rusza",
            BeltOutcome::Stopped => "tasma.zatrzymana",
            BeltOutcome::Refused => "tasma.odmowa",
            BeltOutcome::Unknown => "tasma.bez_odpowiedzi",
            BeltOutcome::Unsupported => "tasma.nieobslugiwane",
            BeltOutcome::NotConnected => "tasma.brak_polaczenia",
        };
        t(key, &[])
    }
}

impl From<StartResult> for BeltOutcome {
    fn from(r: StartResult) -> Self {
        match r {
            StartResult::Started => BeltOutcome::Started,
            StartResult::Refused => BeltOutcome::Refused,
            StartResult::Unknown => BeltOutcome::Unknown,
            StartResult::Unsupported => BeltOutcome::Unsupported,
        }
    }
}

/// Komunikat: bieżnia nie potwierdziła zatrzymania (pokazać, nie tylko w logu).
pub fn stop_not_confirmed() -> String {
    t("ble.stop_niepotwierdzony", &[])
}

/// Komunikat: stop przy zamykaniu aplikacji nie zdążył się wysłać ([`BleWorker::stop_belt_now`]
/// zwróciło `false` przy jadącej taśmie) - powiadomienie systemowe.
pub fn stop_not_sent() -> String {
    t("ble.stop_nie_zdazyl", &[])
}

/// Raport diagnostyczny w toku, zapisany i nieudany (docs/teksty.md, dziennik.raport_*).
pub fn report_busy() -> String {
    t("dziennik.raport_trwa", &[])
}
pub fn report_done() -> String {
    t("dziennik.raport_gotowy", &[])
}
pub fn report_failed() -> String {
    t("dziennik.raport_blad", &[])
}

/// Komunikat: bieżnia nie przyjmuje prędkości.
pub fn speed_refused_text(kmh: f64) -> String {
    t(
        "ble.predkosc_nieprzyjeta",
        &[("kmh", &crate::teksty::kmh(kmh))],
    )
}

/// Zdarzenie dla interfejsu (kanał z [`BleWorker::spawn`]).
#[derive(Debug, Clone, PartialEq)]
pub enum WorkerEvent {
    Link {
        link: Link,
        text: String,
    },
    Devices(Vec<FoundDevice>),
    ScanFailed {
        link: Link,
        text: String,
    },
    /// co to za bieżnia, co potrafi i jakie ma liczniki (po połączeniu i przy zmianie)
    Device {
        /// adres, z którym połączono (po nazwie mógł się zmienić)
        address: String,
        /// w pudełku - najczęstsze zdarzenie (ramka) zostaje małe
        info: Box<DeviceInfo>,
        caps: Capabilities,
        spec: TelemetrySpec,
    },
    Frame(TelemetryFrame),
    Belt(BeltOutcome),
    /// problem do pokazania jako komunikat
    Problem(String),
    /// raport diagnostyczny: ścieżka albo komunikat dla użytkownika (szczegóły w logu)
    Report(Result<PathBuf, String>),
}

/// Czasy workera (testy je skracają).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorkerTiming {
    pub scan_s: f64,
    pub find_s: f64,
    pub connect_s: f64,
    pub retry_max_s: f64,
    /// odstęp kolejnych prób: `retry_step_s · próba`, najwyżej `retry_max_s`
    pub retry_step_s: f64,
    pub diagnostics_listen_s: f64,
}

impl Default for WorkerTiming {
    fn default() -> Self {
        Self {
            scan_s: 6.0,
            find_s: 10.0,
            connect_s: 20.0,
            retry_max_s: 20.0,
            retry_step_s: 2.0,
            diagnostics_listen_s: super::ble::LISTEN_S,
        }
    }
}

fn secs(s: f64) -> Duration {
    Duration::try_from_secs_f64(s.max(0.0)).unwrap_or(Duration::ZERO)
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Liczniki jednego połączenia - na koniec jedna linia podsumowania w logu.
#[derive(Debug, Clone, Default)]
pub struct SessionStats {
    pub began: f64,
    pub starts: u32,
    pub starts_ok: u32,
    pub stops: u32,
}

impl SessionStats {
    /// Linia podsumowania (jak `log_summary` w Pythonie).
    pub fn summary(
        &self,
        now: f64,
        counts: &super::kontrakt::Counts,
        speed: Option<&SpeedStats>,
    ) -> String {
        let empty = SpeedStats::default();
        let speed = speed.unwrap_or(&empty);
        let mut lat = speed.latencies.clone();
        lat.sort_by(f64::total_cmp);
        let typical = lat
            .get(lat.len() / 2)
            .map_or("-".to_string(), |v| format!("{v:.1} s"));
        let worst = lat.last().map_or("-".to_string(), |v| format!("{v:.1} s"));
        format!(
            "Podsumowanie sesji: {:.0} min · prędkość: wysłana {}, przyjęta {}, bez skutku {}, \
             ponowienia {}, czas do zmiany typowo {typical}, najdłużej {worst} · start {}/{} · \
             stop {} · ramki {}, zapytania {}, nieodszyfrowane {}",
            (now - self.began) / 60.0,
            speed.sent,
            speed.ok,
            speed.failed,
            speed.resends,
            self.starts_ok,
            self.starts,
            self.stops,
            counts.frames,
            counts.polls,
            counts.undecoded,
        )
    }
}

/// Prośba okna o prędkość - liczy się tylko najnowsza.
#[derive(Debug, Clone, Copy, Default)]
struct Requests {
    target: Option<f64>,
    auto_start: bool,
}

/// Wspólne dla workera i zadania połączenia.
struct Ctx {
    events: mpsc::UnboundedSender<WorkerEvent>,
    registry: ProfileRegistry,
    clock: Clock,
    timing: WorkerTiming,
    requests: Mutex<Requests>,
    /// dane płyną, polecenia taśmy mają dokąd iść
    streaming: Mutex<bool>,
}

impl Ctx {
    fn emit(&self, event: WorkerEvent) {
        let _ = self.events.send(event); // odbiorca zamknięty = aplikacja się zamyka
    }

    fn link(&self, link: Link, text: impl Into<String>) {
        self.emit(WorkerEvent::Link {
            link,
            text: text.into(),
        });
    }

    fn now(&self) -> f64 {
        (self.clock)()
    }

    fn on_frame(self: &Arc<Self>) -> FrameCallback {
        let events = self.events.clone();
        Arc::new(move |frame| {
            let _ = events.send(WorkerEvent::Frame(frame));
        })
    }
}

/// Polecenia dla trwającego połączenia.
enum SessionCmd {
    Belt(BeltCommand),
    Speed,
    SpeedDue,
    StopNow(oneshot::Sender<bool>),
    SimRemote(i32),
    /// wynik zadania taśmy (id zadania, wynik)
    Started(u64, Result<StartResult, String>),
    Stopped(u64, Result<bool, String>),
}

enum Cmd {
    Scan,
    Connect { address: String, name: String },
    Simulator,
    Disconnect,
    Diagnose(PathBuf),
    Session(SessionCmd),
    Speed { kmh: f64, start_if_stopped: bool },
    Shutdown(oneshot::Sender<()>),
}

/// Uchwyt workera. Metody wysyłają polecenie i wracają od razu.
#[derive(Clone)]
pub struct BleWorker {
    tx: mpsc::UnboundedSender<Cmd>,
}

impl BleWorker {
    /// Uruchamia workera na runtime `runtime`; zdarzenia przychodzą z odbiornika.
    pub fn spawn(
        runtime: &tokio::runtime::Handle,
        backend: Arc<dyn BleBackend>,
        registry: ProfileRegistry,
        clock: Clock,
    ) -> (Self, mpsc::UnboundedReceiver<WorkerEvent>) {
        Self::spawn_with(runtime, backend, registry, clock, WorkerTiming::default())
    }

    /// Jak [`BleWorker::spawn`], z własnymi czasami (testy).
    pub fn spawn_with(
        runtime: &tokio::runtime::Handle,
        backend: Arc<dyn BleBackend>,
        registry: ProfileRegistry,
        clock: Clock,
        timing: WorkerTiming,
    ) -> (Self, mpsc::UnboundedReceiver<WorkerEvent>) {
        let (events, events_rx) = mpsc::unbounded_channel();
        let (tx, rx) = mpsc::unbounded_channel();
        let ctx = Arc::new(Ctx {
            events,
            registry,
            clock,
            timing,
            requests: Mutex::new(Requests::default()),
            streaming: Mutex::new(false),
        });
        let worker = Worker {
            backend,
            ctx,
            session: None,
            target: None,
            simulator: false,
            rssi: HashMap::new(),
        };
        runtime.spawn(worker.run(rx));
        (Self { tx }, events_rx)
    }

    fn send(&self, cmd: Cmd) {
        let _ = self.tx.send(cmd); // worker zakończony - nie ma komu przekazać
    }

    pub fn scan(&self) {
        self.send(Cmd::Scan);
    }

    pub fn connect(&self, address: impl Into<String>, name: impl Into<String>) {
        self.send(Cmd::Connect {
            address: address.into(),
            name: name.into(),
        });
    }

    /// Symulator bieżni zamiast Bluetooth - ten sam kontrakt i to samo sterowanie.
    pub fn connect_simulator(&self) {
        self.send(Cmd::Simulator);
    }

    /// Pilot symulatora (F6/F7): prędkość zmienia sama bieżnia.
    pub fn sim_remote(&self, steps: i32) {
        self.send(Cmd::Session(SessionCmd::SimRemote(steps)));
    }

    pub fn disconnect(&self) {
        self.send(Cmd::Disconnect);
    }

    pub fn diagnose(&self, path: PathBuf) {
        self.send(Cmd::Diagnose(path));
    }

    /// Start albo stop taśmy. Wynik przychodzi jako [`WorkerEvent::Belt`].
    pub fn belt(&self, command: BeltCommand) {
        self.send(Cmd::Session(SessionCmd::Belt(command)));
    }

    /// Prędkość docelowa taśmy. Liczy się tylko najnowsza - starsze prośby przepadają.
    /// `start_if_stopped`: stojącą taśmę najpierw uruchom (pad: ruch gałki budzi bieżnię).
    pub fn set_speed(&self, kmh: f64, start_if_stopped: bool) {
        self.send(Cmd::Speed {
            kmh: super::kontrakt::py_round1(kmh),
            start_if_stopped,
        });
    }

    /// Stop taśmy z czekaniem (zamykanie aplikacji). False, gdy nie było połączenia, taśma
    /// stała albo stop nie zdążył się wysłać.
    pub async fn stop_belt_now(&self, timeout: Duration) -> bool {
        let (reply, answer) = oneshot::channel();
        self.send(Cmd::Session(SessionCmd::StopNow(reply)));
        match tokio::time::timeout(timeout, answer).await {
            Ok(Ok(sent)) => sent,
            _ => {
                log::warn!("Stop taśmy nie zdążył się wysłać");
                false
            }
        }
    }

    /// Rozłącza i kończy workera.
    pub async fn shutdown(&self, timeout: Duration) {
        let (reply, done) = oneshot::channel();
        self.send(Cmd::Shutdown(reply));
        if tokio::time::timeout(timeout, done).await.is_err() {
            log::warn!("Bluetooth nie zamknął się na czas");
        }
    }
}

/// Trwające połączenie (bieżnia albo symulator).
struct Session {
    tx: mpsc::UnboundedSender<SessionCmd>,
    close: watch::Sender<bool>,
    handle: JoinHandle<()>,
}

struct Worker {
    backend: Arc<dyn BleBackend>,
    ctx: Arc<Ctx>,
    session: Option<Session>,
    /// zapamiętana bieżnia trwającego połączenia (adres, nazwa)
    target: Option<(String, String)>,
    /// trwa symulator (wraca po skanie i raporcie diagnostycznym jak bieżnia)
    simulator: bool,
    rssi: HashMap<String, i16>,
}

/// Co wznowić po skanie albo raporcie diagnostycznym.
enum Resume {
    Device(String, String),
    Simulator,
}

/// Komunikat i stan połączenia dla błędu (docs/teksty.md, ble.powod.*).
pub fn friendly_error(error: &BleError) -> (Link, String) {
    let link = match error {
        BleError::BluetoothOff => Link::BluetoothOff,
        BleError::Driver(DriverError::UnsupportedDevice) => Link::Unsupported,
        _ => Link::Retrying,
    };
    (link, error.to_string())
}

impl Worker {
    async fn run(mut self, mut rx: mpsc::UnboundedReceiver<Cmd>) {
        while let Some(cmd) = rx.recv().await {
            match cmd {
                Cmd::Scan => self.scan().await,
                Cmd::Connect { address, name } => self.start_session(address, name).await,
                Cmd::Simulator => self.start_simulator().await,
                Cmd::Disconnect => self.stop_session(true).await,
                Cmd::Diagnose(path) => self.diagnose(path).await,
                Cmd::Speed {
                    kmh,
                    start_if_stopped,
                } => {
                    {
                        let mut r = lock(&self.ctx.requests);
                        r.target = Some(kmh);
                        r.auto_start = start_if_stopped;
                    }
                    self.to_session(SessionCmd::Speed);
                }
                Cmd::Session(cmd) => self.to_session(cmd),
                Cmd::Shutdown(done) => {
                    self.stop_session(true).await;
                    let _ = done.send(());
                    return;
                }
            }
        }
        self.stop_session(true).await;
    }

    /// Polecenie dla połączenia; bez danych z bieżni polecenia taśmy nie mają dokąd iść.
    fn to_session(&self, cmd: SessionCmd) {
        let streaming = *lock(&self.ctx.streaming);
        let session = self.session.as_ref().filter(|_| streaming);
        match (session, cmd) {
            (Some(s), cmd) => {
                if let Err(mpsc::error::SendError(cmd)) = s.tx.send(cmd) {
                    self.not_delivered(cmd);
                }
            }
            (None, cmd) => self.not_delivered(cmd),
        }
    }

    fn not_delivered(&self, cmd: SessionCmd) {
        match cmd {
            SessionCmd::Belt(_) => self.ctx.emit(WorkerEvent::Belt(BeltOutcome::NotConnected)),
            SessionCmd::StopNow(reply) => {
                let _ = reply.send(false);
            }
            _ => {}
        }
    }

    async fn stop_session(&mut self, forget: bool) {
        if forget {
            self.target = None;
            self.simulator = false;
        }
        let Some(session) = self.session.take() else {
            return;
        };
        let _ = session.close.send(true);
        let mut handle = session.handle;
        if tokio::time::timeout(Duration::from_secs(3), &mut handle)
            .await
            .is_err()
        {
            handle.abort();
        }
        *lock(&self.ctx.streaming) = false;
        self.ctx.link(Link::Idle, t("ble.rozlaczono", &[]));
    }

    async fn start_session(&mut self, address: String, name: String) {
        self.stop_session(true).await;
        self.target = Some((address.clone(), name.clone()));
        let (tx, rx) = mpsc::unbounded_channel();
        let (close, closed) = watch::channel(false);
        let ctx = self.ctx.clone();
        let backend = self.backend.clone();
        let rssi = self.rssi.get(&address).copied();
        let session_tx = tx.clone();
        let handle = tokio::spawn(async move {
            session_loop(ctx, backend, address, name, rssi, rx, session_tx, closed).await;
        });
        self.session = Some(Session { tx, close, handle });
    }

    async fn start_simulator(&mut self) {
        self.stop_session(true).await;
        self.simulator = true;
        let (tx, rx) = mpsc::unbounded_channel();
        let (close, closed) = watch::channel(false);
        let ctx = self.ctx.clone();
        let session_tx = tx.clone();
        let handle = tokio::spawn(async move {
            let driver = Arc::new(SimulatorDriver::new(
                ctx.on_frame(),
                ctx.clock.clone(),
                simulator::CAPS,
            ));
            let mut closed = closed;
            let mut rx = rx;
            let never = std::future::pending::<()>();
            let as_driver: Arc<dyn Driver> = driver.clone();
            if let Err(e) = drive(
                &ctx,
                as_driver,
                Some(driver),
                "",
                &mut rx,
                session_tx,
                &mut closed,
                never,
            )
            .await
            {
                log::warn!("Symulator: {e}");
            }
        });
        self.session = Some(Session { tx, close, handle });
    }

    /// Wstrzymuje połączenie na czas skanu albo raportu; zwraca, co potem wznowić.
    async fn pause_session(&mut self) -> Option<Resume> {
        let resume = if self.simulator {
            Some(Resume::Simulator)
        } else {
            self.target
                .clone()
                .map(|(address, name)| Resume::Device(address, name))
        };
        self.stop_session(false).await;
        resume
    }

    /// Wznawia wstrzymane połączenie; bez niego - stan `link` z tekstem.
    async fn resume_or(&mut self, resume: Option<Resume>, link: Link, text: String) {
        match resume {
            Some(Resume::Device(address, name)) => self.start_session(address, name).await,
            // w 0.5.x symulator po skanie albo raporcie zostawał rozłączony (taśma „niepołączona”)
            Some(Resume::Simulator) => self.start_simulator().await,
            None => self.ctx.link(link, text),
        }
    }

    async fn resume(&mut self, resume: Option<Resume>, idle_text: String) {
        self.resume_or(resume, Link::Idle, idle_text).await;
    }

    async fn scan(&mut self) {
        let resume = self.pause_session().await;
        self.ctx
            .link(Link::Searching, t("glowny.status.skanowanie.tytul", &[]));
        let found = match self.backend.scan(secs(self.ctx.timing.scan_s)).await {
            Ok(found) => found,
            Err(e) => {
                let (link, text) = friendly_error(&e);
                log::warn!("Skan: {e}");
                self.ctx.emit(WorkerEvent::ScanFailed {
                    link,
                    text: text.clone(),
                });
                // wstrzymane połączenie wraca (w 0.5 zostawało rozłączone po nieudanym skanie)
                self.resume_or(resume, link, text).await;
                return;
            }
        };
        let registry = &self.ctx.registry;
        let mut devices: Vec<FoundDevice> = found
            .iter()
            .map(|a| {
                let name = a.name.clone().unwrap_or_default();
                FoundDevice {
                    address: a.address.clone(),
                    treadmill: super::looks_like_treadmill(
                        registry,
                        a.name.as_deref(),
                        &a.services,
                    ),
                    model: super::model_name(registry, &name),
                    name,
                    rssi: a.rssi.unwrap_or(i16::MIN),
                }
            })
            .collect();
        devices.sort_by_key(|d| (!d.treadmill, d.name.is_empty(), std::cmp::Reverse(d.rssi)));
        self.rssi = devices
            .iter()
            .map(|d| (d.address.clone(), d.rssi))
            .collect();
        log::info!(
            "Skan: {} urządzeń, w tym bieżni: {}",
            devices.len(),
            devices.iter().filter(|d| d.treadmill).count()
        );
        self.ctx.emit(WorkerEvent::Devices(devices));
        self.resume(resume, t("ble.wybierz_z_listy", &[])).await;
    }

    async fn diagnose(&mut self, path: PathBuf) {
        let resume = self.pause_session().await;
        self.ctx.link(Link::Searching, report_busy());
        let address = match &resume {
            Some(Resume::Device(address, _)) => Some(address.clone()),
            _ => None,
        };
        let result = run_diagnostics(
            self.backend.as_ref(),
            &self.ctx.registry,
            &path,
            address,
            self.ctx.clock.clone(),
            self.ctx.timing.diagnostics_listen_s,
        )
        .await;
        let report = result.map_err(|e| {
            log::error!("Raport diagnostyczny nie powiódł się: {e}");
            report_failed()
        });
        let idle = if report.is_ok() {
            report_done()
        } else {
            report_failed()
        };
        self.ctx.emit(WorkerEvent::Report(report));
        self.resume(resume, idle).await;
    }
}

/// Kończy się, gdy okno zamknie połączenie (albo worker zniknie).
async fn wait_closed(closed: &mut watch::Receiver<bool>) {
    let _ = closed.wait_for(|c| *c).await;
}

/// Czeka na `future` albo zamknięcie połączenia (wtedy None).
async fn unless_closed<F: Future>(
    closed: &mut watch::Receiver<bool>,
    future: F,
) -> Option<F::Output> {
    if *closed.borrow() {
        return None;
    }
    tokio::select! {
        out = future => Some(out),
        () = wait_closed(closed) => None,
    }
}

#[allow(clippy::too_many_arguments)]
async fn session_loop(
    ctx: Arc<Ctx>,
    backend: Arc<dyn BleBackend>,
    address: String,
    name: String,
    rssi: Option<i16>,
    mut rx: mpsc::UnboundedReceiver<SessionCmd>,
    session_tx: mpsc::UnboundedSender<SessionCmd>,
    mut closed: watch::Receiver<bool>,
) {
    let model = super::model_name(&ctx.registry, &name);
    let mut attempt: u32 = 0;
    loop {
        let result = connect_once(
            &ctx,
            backend.as_ref(),
            &address,
            &name,
            &model,
            rssi,
            &mut rx,
            &session_tx,
            &mut closed,
            &mut attempt,
        )
        .await;
        *lock(&ctx.streaming) = false;
        match result {
            None => return, // zamknięte z okna
            Some(Ok(())) => {
                log::warn!("Bieżnia się rozłączyła");
                ctx.link(Link::Retrying, DriverError::Disconnected.to_string());
            }
            Some(Err(e)) => {
                let (link, text) = friendly_error(&e);
                log::warn!("Połączenie: {e}");
                ctx.link(link, text);
                if link == Link::Unsupported {
                    return;
                }
            }
        }
        attempt += 1;
        let wait = ctx
            .timing
            .retry_max_s
            .min(ctx.timing.retry_step_s * f64::from(attempt));
        if unless_closed(&mut closed, tokio::time::sleep(secs(wait)))
            .await
            .is_none()
        {
            return;
        }
    }
}

/// Jedno połączenie: szukanie, łączenie, sterownik, dane. None - zamknięte z okna,
/// Ok - bieżnia się rozłączyła.
#[allow(clippy::too_many_arguments)]
async fn connect_once(
    ctx: &Arc<Ctx>,
    backend: &dyn BleBackend,
    address: &str,
    name: &str,
    model: &str,
    rssi: Option<i16>,
    rx: &mut mpsc::UnboundedReceiver<SessionCmd>,
    session_tx: &mpsc::UnboundedSender<SessionCmd>,
    closed: &mut watch::Receiver<bool>,
    attempt: &mut u32,
) -> Option<Result<(), BleError>> {
    ctx.link(
        Link::Searching,
        t("glowny.status.szukanie.tytul", &[("model", model)]),
    );
    let found = unless_closed(
        closed,
        backend.find(address.into(), name.into(), secs(ctx.timing.find_s)),
    )
    .await?;
    let found = match found {
        Ok(Some(found)) => found,
        Ok(None) => return Some(Err(BleError::NotFound)),
        Err(e) => return Some(Err(e)),
    };
    if !found.address.eq_ignore_ascii_case(address) {
        log::info!(
            "Bieżnia {name} widoczna pod innym adresem: {}",
            found.address
        );
    }
    ctx.link(
        Link::Connecting,
        t("glowny.status.laczenie.tytul", &[("model", model)]),
    );
    let link = match unless_closed(
        closed,
        backend.connect(found.address.clone(), secs(ctx.timing.connect_s)),
    )
    .await?
    {
        Ok(link) => link,
        Err(e) => return Some(Err(e)),
    };
    let result = run_link(
        ctx,
        &link,
        &found.address,
        name,
        rssi.or(found.rssi),
        rx,
        session_tx,
        closed,
        attempt,
    )
    .await;
    link.disconnect().await;
    result
}

#[allow(clippy::too_many_arguments)]
async fn run_link(
    ctx: &Arc<Ctx>,
    link: &Arc<dyn BleLink>,
    address: &str,
    name: &str,
    rssi: Option<i16>,
    rx: &mut mpsc::UnboundedReceiver<SessionCmd>,
    session_tx: &mpsc::UnboundedSender<SessionCmd>,
    closed: &mut watch::Receiver<bool>,
    attempt: &mut u32,
) -> Option<Result<(), BleError>> {
    let mut notes = match link.notifications().await {
        Ok(notes) => notes,
        Err(e) => return Some(Err(e)),
    };
    let gatt: Arc<dyn GattClient> = link.clone();
    let driver =
        match super::make_driver(gatt, name, &ctx.registry, ctx.on_frame(), ctx.clock.clone()) {
            Ok(driver) => driver,
            Err(e) => return Some(Err(e.into())),
        };
    log::info!(
        "Połączenie {}: {name}, sygnał {} dBm",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        rssi.map_or("?".to_string(), |r| r.to_string())
    );
    *attempt = 0;
    // powiadomienia do sterownika osobnym zadaniem - start sterownika czeka na odpowiedzi
    let (lost_tx, lost_rx) = oneshot::channel::<()>();
    let pumped = driver.clone();
    let pump = tokio::spawn(async move {
        while let Some(n) = notes.recv().await {
            pumped.on_notification(&n.characteristic_uuid, &n.data);
        }
        let _ = lost_tx.send(());
    });
    let lost = async {
        let _ = lost_rx.await;
    };
    let result = drive(
        ctx,
        driver,
        None,
        address,
        rx,
        session_tx.clone(),
        closed,
        lost,
    )
    .await;
    pump.abort();
    match result {
        Ok(true) => Some(Ok(())),
        Ok(false) => None,
        Err(e) => Some(Err(e.into())),
    }
}

/// Zadanie taśmy w toku (start albo stop).
struct BeltJob {
    id: u64,
    kind: BeltCommand,
    handle: JoinHandle<()>,
}

/// Wspólne dla bieżni i symulatora: start sterownika, polecenia, pilnowanie, podsumowanie.
/// Ok(true) - bieżnia się rozłączyła, Ok(false) - zamknięte z okna.
#[allow(clippy::too_many_arguments)]
async fn drive(
    ctx: &Arc<Ctx>,
    driver: Arc<dyn Driver>,
    sim: Option<Arc<SimulatorDriver>>,
    address: &str,
    rx: &mut mpsc::UnboundedReceiver<SessionCmd>,
    session_tx: mpsc::UnboundedSender<SessionCmd>,
    closed: &mut watch::Receiver<bool>,
    lost: impl Future<Output = ()>,
) -> Result<bool, DriverError> {
    tokio::pin!(lost);
    let mut stream = Stream::new(ctx.clone(), driver.clone(), sim, session_tx);
    ctx.link(
        Link::Connected,
        t(
            "glowny.status.polaczono.tytul",
            &[("model", &driver.info().model)],
        ),
    );
    let started = tokio::select! {
        r = driver.start() => Some(r),
        () = wait_closed(closed) => None,
        () = &mut lost => return Ok(true),
    };
    match started {
        None => return Ok(false),
        Some(Err(e)) => return Err(e),
        Some(Ok(())) => {}
    }
    log::info!("Bieżnia: {}", driver.describe());
    stream.announce(address);
    ctx.link(
        Link::Streaming,
        t(
            "glowny.status.polaczono.tytul",
            &[("model", &driver.info().model)],
        ),
    );
    // polecenia sprzed połączenia nie dotyczą tej taśmy
    while rx.try_recv().is_ok() {}
    *lock(&ctx.streaming) = true;
    let result = stream.run(address, rx, closed, lost).await;
    *lock(&ctx.streaming) = false;
    stream.finish();
    result
}

/// Pętla połączenia: odpytywanie, polecenia taśmy i prędkość (port `_stream`).
struct Stream {
    ctx: Arc<Ctx>,
    driver: Arc<dyn Driver>,
    sim: Option<Arc<SimulatorDriver>>,
    tx: mpsc::UnboundedSender<SessionCmd>,
    caps: Capabilities,
    belt: Option<BeltJob>,
    next_job: u64,
    sent_speed: Option<f64>,
    sent_at: f64,
    speed_later: bool,
    speed_failures: u32,
    stats: SessionStats,
}

impl Stream {
    fn new(
        ctx: Arc<Ctx>,
        driver: Arc<dyn Driver>,
        sim: Option<Arc<SimulatorDriver>>,
        tx: mpsc::UnboundedSender<SessionCmd>,
    ) -> Self {
        let caps = driver.capabilities();
        let stats = SessionStats {
            began: ctx.now(),
            ..SessionStats::default()
        };
        Self {
            ctx,
            driver,
            sim,
            tx,
            caps,
            belt: None,
            next_job: 0,
            sent_speed: None,
            sent_at: 0.0,
            speed_later: false,
            speed_failures: 0,
            stats,
        }
    }

    fn announce(&mut self, address: &str) {
        self.caps = self.driver.capabilities();
        self.ctx.emit(WorkerEvent::Device {
            address: address.to_string(),
            info: Box::new(self.driver.info()),
            caps: self.caps,
            spec: self.driver.telemetry_spec(),
        });
    }

    async fn run(
        &mut self,
        address: &str,
        rx: &mut mpsc::UnboundedReceiver<SessionCmd>,
        closed: &mut watch::Receiver<bool>,
        lost: impl Future<Output = ()>,
    ) -> Result<bool, DriverError> {
        tokio::pin!(lost);
        let mut next_poll = tokio::time::Instant::now();
        let mut failures = 0;
        loop {
            if tokio::time::Instant::now() >= next_poll {
                next_poll = tokio::time::Instant::now() + secs(self.driver.poll_interval());
                match self.driver.poll().await {
                    Ok(()) => failures = 0,
                    Err(DriverError::Disconnected) => return Ok(true),
                    Err(e) => {
                        log::warn!("Odpytywanie bieżni: {e}");
                        failures += 1;
                        if failures >= POLL_FAILURES {
                            return Err(e); // połączenie martwe, choć nikt nie zgłosił rozłączenia
                        }
                    }
                }
                if self.driver.capabilities() != self.caps {
                    // bieżnia potrafi zdradzić więcej w trakcie (np. Urevo: kroki w danych)
                    self.announce(address);
                }
                self.check_speed().await;
            }
            tokio::select! {
                cmd = rx.recv() => match cmd {
                    Some(cmd) => self.handle(cmd).await,
                    None => return Ok(false),
                },
                () = wait_closed(closed) => return Ok(false),
                () = &mut lost => return Ok(true),
                () = tokio::time::sleep_until(next_poll) => {}
            }
        }
    }

    fn finish(&mut self) {
        if let Some(job) = self.belt.take() {
            job.handle.abort();
        }
        let line = self.stats.summary(
            self.ctx.now(),
            &self.driver.counts(),
            self.driver.speed_stats().as_ref(),
        );
        log::info!("{line}");
    }

    fn busy(&self, kind: BeltCommand) -> bool {
        // zadanie trwa, dopóki pętla nie przyjmie jego wyniku
        self.belt.as_ref().is_some_and(|job| job.kind == kind)
    }

    fn requests(&self) -> Requests {
        *lock(&self.ctx.requests)
    }

    /// Pad wychylony, a taśma stoi (albo właśnie ją zatrzymujemy) - trzeba ją uruchomić.
    fn needs_start(&self) -> bool {
        let r = self.requests();
        if !r.auto_start || r.target.unwrap_or(0.0) <= 0.0 || self.busy(BeltCommand::Start) {
            return false;
        }
        self.busy(BeltCommand::Stop) || self.driver.belt_running() == Some(false)
    }

    fn cancel_belt(&mut self) {
        if let Some(job) = self.belt.take() {
            job.handle.abort();
        }
    }

    fn begin_start(&mut self) {
        if self.busy(BeltCommand::Start) {
            return; // start już w toku
        }
        self.cancel_belt(); // sprawdzanie stopu przerwane nowym startem
        self.stats.starts += 1;
        self.next_job += 1;
        let id = self.next_job;
        let driver = self.driver.clone();
        let tx = self.tx.clone();
        let handle = tokio::spawn(async move {
            let result = driver.start_belt().await.map_err(|e| e.to_string());
            let _ = tx.send(SessionCmd::Started(id, result));
        });
        self.belt = Some(BeltJob {
            id,
            kind: BeltCommand::Start,
            handle,
        });
    }

    fn begin_stop(&mut self) {
        self.cancel_belt(); // stop przerywa start w toku
        self.sent_speed = None;
        {
            // stop kasuje prośby o prędkość i samo-uruchamianie
            let mut r = lock(&self.ctx.requests);
            r.target = None;
            r.auto_start = false;
        }
        self.stats.stops += 1;
        self.next_job += 1;
        let id = self.next_job;
        let driver = self.driver.clone();
        let tx = self.tx.clone();
        let handle = tokio::spawn(async move {
            let result = driver.stop_belt(true).await.map_err(|e| e.to_string());
            let _ = tx.send(SessionCmd::Stopped(id, result));
        });
        self.belt = Some(BeltJob {
            id,
            kind: BeltCommand::Stop,
            handle,
        });
    }

    fn job_done(&mut self, id: u64) -> bool {
        if self.belt.as_ref().is_some_and(|job| job.id == id) {
            self.belt = None;
            true
        } else {
            false // wynik przerwanego zadania
        }
    }

    async fn handle(&mut self, cmd: SessionCmd) {
        match cmd {
            SessionCmd::Speed => {
                if self.needs_start() {
                    self.begin_start();
                } else if !self.busy(BeltCommand::Start) {
                    // w trakcie startu prędkość ustawi wynik startu
                    self.apply_speed().await;
                }
            }
            SessionCmd::SpeedDue => {
                self.speed_later = false;
                if !self.busy(BeltCommand::Start) {
                    self.apply_speed().await;
                }
            }
            SessionCmd::Belt(BeltCommand::Start) => self.begin_start(),
            SessionCmd::Belt(BeltCommand::Stop) => self.begin_stop(),
            SessionCmd::StopNow(reply) => {
                self.cancel_belt();
                let sent = if self.driver.belt_running() == Some(false) {
                    false // taśma stoi - nic nie wysyłamy
                } else {
                    // zamykanie: pełny stop bez czekania na potwierdzenie
                    self.driver.stop_belt(false).await.is_ok()
                };
                let _ = reply.send(sent);
            }
            SessionCmd::SimRemote(steps) => {
                if let Some(sim) = &self.sim {
                    sim.remote(steps);
                }
            }
            SessionCmd::Started(id, result) => {
                if !self.job_done(id) {
                    return;
                }
                let outcome = match result {
                    Ok(r) => BeltOutcome::from(r),
                    Err(e) => {
                        log::warn!("Start taśmy: {e}");
                        BeltOutcome::Unknown
                    }
                };
                if outcome == BeltOutcome::Started {
                    self.stats.starts_ok += 1;
                }
                // po starcie taśma jedzie z prędkością startową bieżni - docelowa jeszcze raz
                self.sent_speed = None;
                self.sent_at = 0.0;
                if matches!(outcome, BeltOutcome::Started | BeltOutcome::Unknown) {
                    self.apply_speed().await;
                }
                self.ctx.emit(WorkerEvent::Belt(outcome));
            }
            SessionCmd::Stopped(id, result) => {
                if !self.job_done(id) {
                    return;
                }
                match result {
                    Ok(true) => {}
                    Ok(false) => self.ctx.emit(WorkerEvent::Problem(stop_not_confirmed())),
                    Err(e) => {
                        log::warn!("Stop taśmy: {e}");
                        self.ctx.emit(WorkerEvent::Problem(stop_not_confirmed()));
                    }
                }
                self.ctx.emit(WorkerEvent::Belt(BeltOutcome::Stopped));
            }
        }
    }

    async fn apply_speed(&mut self) {
        let Some(target) = self.requests().target else {
            return;
        };
        if Some(target) == self.sent_speed {
            return;
        }
        let wait = self.caps.min_command_gap_s - (self.ctx.now() - self.sent_at);
        if wait > 0.0 {
            // za szybko po poprzednim - wyślemy najnowszą prędkość za chwilę
            if !self.speed_later {
                self.speed_later = true;
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(secs(wait)).await;
                    let _ = tx.send(SessionCmd::SpeedDue);
                });
            }
            return;
        }
        self.sent_at = self.ctx.now();
        if matches!(self.driver.set_speed(target).await, Ok(true)) {
            self.sent_speed = Some(target);
            self.speed_failures = 0;
            return;
        }
        self.speed_failures += 1;
        if self.speed_failures >= SPEED_ATTEMPTS {
            // dalej nie walczymy - nowa prośba spróbuje
            log::warn!("Bieżnia nie przyjęła {target:.1} km/h po {SPEED_ATTEMPTS} próbach");
            self.ctx
                .emit(WorkerEvent::Problem(speed_refused_text(target)));
            self.sent_speed = Some(target);
            self.speed_failures = 0;
        }
    }

    /// Co odpytanie: stojąca taśma przy wychylonej gałce rusza, a prędkość, której bieżnia
    /// nie przyjęła, idzie jeszcze raz. Czy taśma posłuchała, pilnuje już sam sterownik.
    async fn check_speed(&mut self) {
        if self.needs_start() {
            // np. taśmę zatrzymał pilot albo zgubiony start
            log::info!(
                "Taśma stoi, a pad chce {:.1} km/h - uruchamiam",
                self.requests().target.unwrap_or(0.0)
            );
            self.begin_start();
            return;
        }
        if self.belt.is_some() {
            return;
        }
        let target = self.requests().target;
        if target.is_some() && target != self.sent_speed && self.sent_at > 0.0 {
            self.apply_speed().await; // poprzednia próba nie dostała potwierdzenia
        }
    }
}
