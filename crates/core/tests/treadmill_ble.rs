//! Sterowniki KingSmith i FTMS na wirtualnej bieżni oraz worker na atrapie Bluetooth (port
//! tests/test_ble.py). Bez radia i bez prawdziwej bieżni.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::FutureExt;
use futures::future::BoxFuture;
use tokio::sync::mpsc;
use walklevel_core::treadmill::kingsmith::{self, KingSmithDriver, Timing};
use walklevel_core::treadmill::kingsmith_proto as ks;
use walklevel_core::treadmill::worker::WorkerTiming;
use walklevel_core::treadmill::{
    Advertisement, BeltCommand, BeltOutcome, BleBackend, BleError, BleLink, BleWorker, Clock,
    DeviceInfo, Driver, DriverError, DriverKind, FrameCallback, FtmsDriver, GattCharacteristic,
    GattClient, GattService, Link, Notification, ProfileRegistry, StartResult, TelemetryFrame,
    WorkerEvent, ftms_proto as ftms, make_driver, monotonic_clock,
};

const BASE: &str = "-0000-1000-8000-00805f9b34fb";

fn ch(service: &str, uuid: &str, props: &[&str]) -> GattCharacteristic {
    GattCharacteristic {
        service_uuid: service.into(),
        uuid: uuid.into(),
        properties: props.iter().map(|p| p.to_string()).collect(),
    }
}

/// Wspólne dla wirtualnych bieżni: kanał powiadomień jak w btleplug.
#[derive(Default)]
struct Radio {
    tx: Mutex<Option<mpsc::UnboundedSender<Notification>>>,
}

impl Radio {
    fn notify(&self, uuid: &str, data: Vec<u8>) {
        if let Some(tx) = self.tx.lock().expect("radio").as_ref() {
            let _ = tx.send(Notification {
                characteristic_uuid: uuid.into(),
                data,
            });
        }
    }

    fn open(&self) -> mpsc::UnboundedReceiver<Notification> {
        let (tx, rx) = mpsc::unbounded_channel();
        *self.tx.lock().expect("radio") = Some(tx);
        rx
    }

    /// Zerwane połączenie: odbiornik dostaje None.
    fn drop_link(&self) {
        self.tx.lock().expect("radio").take();
    }
}

// --- wirtualny WalkingPad ------------------------------------------------------------------

#[derive(Clone)]
struct KsConfig {
    table: &'static str,
    prefix: &'static str,
    speed: f64,
    answer_empty: bool,
    extra_silent_channel: bool,
    running: bool,
    refuse_start: bool,
    /// tyle poleceń start/stop przepada bez śladu (jak w X21)
    drop_commands: u32,
    /// pierwsze polecenie prędkości przepada
    lose_first_speed: bool,
    /// zapis GATT się nie udaje (bieżnia zniknęła bez zgłoszenia rozłączenia)
    fail_writes: bool,
    /// po stopie prędkości kolejnych ramek z runState 1 (hamowanie X21C), potem runState 0
    braking: Vec<f64>,
    /// speed in miles per hour (`unit 1`)
    miles: bool,
}

impl Default for KsConfig {
    fn default() -> Self {
        Self {
            table: "v1",
            prefix: "0002",
            speed: 3.5,
            answer_empty: true,
            extra_silent_channel: false,
            running: true,
            refuse_start: false,
            drop_commands: 0,
            lose_first_speed: false,
            fail_writes: false,
            braking: Vec::new(),
            miles: false,
        }
    }
}

struct KsState {
    cfg: KsConfig,
    received: Vec<String>,
    buf: Vec<u8>,
    speed_lost: bool,
    braking_left: Vec<f64>,
}

/// Odszyfrowuje polecenia i odpowiada jak X21 (wg sterownika QZ).
struct FakeKs {
    services: Vec<GattService>,
    state: Mutex<KsState>,
    radio: Radio,
}

impl FakeKs {
    fn new(cfg: KsConfig) -> Arc<Self> {
        let service = |prefix: &str| {
            let uuid = format!("{prefix}1234{BASE}");
            GattService {
                characteristics: vec![
                    ch(
                        &uuid,
                        &format!("{prefix}fed7{BASE}"),
                        &["write-without-response", "write"],
                    ),
                    ch(&uuid, &format!("{prefix}fed8{BASE}"), &["notify"]),
                ],
                uuid,
            }
        };
        let mut services = vec![service(cfg.prefix)];
        if cfg.extra_silent_channel {
            services.insert(0, service("0000"));
        }
        Arc::new(Self {
            services,
            state: Mutex::new(KsState {
                cfg,
                received: Vec::new(),
                buf: Vec::new(),
                speed_lost: false,
                braking_left: Vec::new(),
            }),
            radio: Radio::default(),
        })
    }

    fn received(&self) -> Vec<String> {
        self.state.lock().expect("stan").received.clone()
    }

    fn running(&self) -> bool {
        self.state.lock().expect("stan").cfg.running
    }

    fn fail_writes(&self, fail: bool) {
        self.state.lock().expect("stan").cfg.fail_writes = fail;
    }

    fn count(&self, text: &str) -> usize {
        self.received().iter().filter(|t| *t == text).count()
    }

    fn reply(&self, s: &KsState, text: &str) {
        let notify = format!("{}fed8{BASE}", s.cfg.prefix);
        let data = ks::encode(text, s.cfg.table).expect("tablica");
        for chunk in ks::chunks(&data, ks::CHUNK) {
            self.radio.notify(&notify, chunk.to_vec());
        }
    }

    fn on_command(&self, s: &mut KsState, frame: &[u8]) {
        if frame.is_empty() {
            if s.cfg.answer_empty {
                self.reply(s, "format error");
            }
            return;
        }
        let Some(text) = ks::decode(frame, s.cfg.table) else {
            return; // zła tablica - prawdziwa bieżnia milczy
        };
        s.received.push(text.clone());
        let word = text.split_whitespace().next().unwrap_or("").to_string();
        if word == "servers" && !s.braking_left.is_empty() {
            let speed = s.braking_left.remove(0);
            if s.braking_left.is_empty() {
                s.cfg.running = false;
            }
            let reply = format!("props CurrentSpeed {speed} RunningSteps 120 runState 1");
            self.reply(s, &reply);
        } else if word == "servers" {
            let speed = if s.cfg.running { s.cfg.speed } else { 0.0 };
            let reply = format!(
                "props CurrentSpeed {speed} RunningSteps 120 runState {}{}",
                i32::from(s.cfg.running),
                if s.cfg.miles { " unit 1" } else { "" }
            );
            self.reply(s, &reply);
        } else if text == ks::CMD_CONTROL {
            self.reply(s, "props ControlMode 1");
        } else if (text == ks::CMD_START || text == ks::CMD_STOP) && s.cfg.drop_commands > 0 {
            s.cfg.drop_commands -= 1;
        } else if text == ks::CMD_START {
            if s.cfg.refuse_start {
                self.reply(s, "props Error ErrorCode -5000");
            } else {
                s.cfg.running = true;
                self.reply(s, "props runState 1");
            }
        } else if text == ks::CMD_STOP && !s.cfg.braking.is_empty() {
            s.braking_left = s.cfg.braking.clone(); // hamuje stopniami, runState 1 do końca
        } else if text == ks::CMD_STOP {
            s.cfg.running = false;
            self.reply(s, "props runState 0");
        } else if let Some(v) = text.strip_prefix("props CurrentSpeed ") {
            if s.cfg.lose_first_speed && !s.speed_lost {
                s.speed_lost = true; // przepada bez śladu
                return;
            }
            s.cfg.speed = v.parse().unwrap_or(s.cfg.speed);
        } else {
            self.reply(s, &format!("{word} ok"));
        }
    }
}

impl GattClient for FakeKs {
    fn services(&self) -> Vec<GattService> {
        self.services.clone()
    }

    fn read<'a>(
        &'a self,
        _characteristic: &'a GattCharacteristic,
    ) -> BoxFuture<'a, Result<Vec<u8>, DriverError>> {
        async { Ok(Vec::new()) }.boxed()
    }

    fn write<'a>(
        &'a self,
        characteristic: &'a GattCharacteristic,
        data: &'a [u8],
        _with_response: bool,
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        let mut s = self.state.lock().expect("stan");
        if s.cfg.fail_writes {
            return async { Err(DriverError::Gatt("zapis nieudany".into())) }.boxed();
        }
        if characteristic.uuid.starts_with(s.cfg.prefix) {
            s.buf.extend_from_slice(data);
            while let Some(i) = s.buf.iter().position(|b| *b == b'\r') {
                let frame: Vec<u8> = s.buf.drain(..=i).collect();
                self.on_command(&mut s, &frame[..frame.len() - 1]);
            }
        }
        async { Ok(()) }.boxed()
    }

    fn subscribe<'a>(
        &'a self,
        _characteristic: &'a GattCharacteristic,
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        async { Ok(()) }.boxed()
    }
}

impl BleLink for FakeKs {
    fn notifications(
        &self,
    ) -> BoxFuture<'_, Result<mpsc::UnboundedReceiver<Notification>, BleError>> {
        let rx = self.radio.open();
        async { Ok(rx) }.boxed()
    }

    fn disconnect(&self) -> BoxFuture<'_, ()> {
        self.radio.drop_link();
        async {}.boxed()
    }
}

fn fast() -> Timing {
    Timing {
        fragment_gap_s: 0.0,
        line_gap_s: 0.0,
        reply_wait_s: 0.05,
        first_reply_s: 0.1,
        verdict_frame_s: 0.1,
        verdict_max_s: 0.5,
        control_gap_s: 0.0,
        quiet_after_push_s: 0.0,
        brake_no_drop_s: 0.4,
        brake_max_s: 2.0,
    }
}

fn info(model: &str, driver: DriverKind) -> DeviceInfo {
    DeviceInfo {
        model: model.into(),
        driver,
        profile: String::new(),
        name: model.into(),
        firmware: String::new(),
    }
}

/// Sterownik KingSmith z wirtualną bieżnią; powiadomienia płyną osobnym zadaniem.
struct KsRig {
    fake: Arc<FakeKs>,
    driver: Arc<KingSmithDriver>,
    frames: Arc<Mutex<Vec<TelemetryFrame>>>,
}

async fn ks_rig(cfg: KsConfig, clock: Clock) -> KsRig {
    let fake = FakeKs::new(cfg);
    let frames = Arc::new(Mutex::new(Vec::new()));
    let sink = frames.clone();
    let on_frame: FrameCallback = Arc::new(move |f| sink.lock().expect("ramki").push(f));
    let channels = ks::find_channels(&fake.services());
    let order = ks::table_order("KS-HDSY-X21C");
    let gatt: Arc<dyn GattClient> = fake.clone();
    let driver = Arc::new(
        KingSmithDriver::new(
            gatt,
            channels,
            &order,
            ks::DEFAULT_CAPS,
            info("KingSmith", DriverKind::KingSmith),
            on_frame,
            clock,
        )
        .with_timing(fast()),
    );
    let mut rx = fake.notifications().await.expect("powiadomienia");
    let pumped = driver.clone();
    tokio::spawn(async move {
        while let Some(n) = rx.recv().await {
            pumped.on_notification(&n.characteristic_uuid, &n.data);
        }
    });
    KsRig {
        fake,
        driver,
        frames,
    }
}

async fn started(cfg: KsConfig, polls: u32) -> KsRig {
    let rig = ks_rig(cfg, monotonic_clock()).await;
    rig.driver.start().await.expect("start");
    for _ in 0..polls {
        rig.driver.poll().await.expect("odpytanie");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    rig
}

fn last_speed(rig: &KsRig) -> Option<f64> {
    rig.frames
        .lock()
        .expect("ramki")
        .last()
        .and_then(|f| f.speed_kmh)
}

#[tokio::test]
async fn x21_default_table_streams_speed() {
    let rig = started(KsConfig::default(), 1).await;
    assert_eq!(last_speed(&rig), Some(3.5));
    assert!(rig.driver.table_candidates().contains(&"v1"));
    assert_eq!(rig.fake.received()[..2], ["shake", "net"]);
    assert!(rig.fake.count(ks::GET_PROPS) >= 1);
    // pełna ramka: kroki z licznika bieżni
    assert!(
        rig.frames
            .lock()
            .expect("ramki")
            .iter()
            .any(|f| f.steps == Some(120))
    );
}

#[tokio::test]
async fn other_tables_are_detected_from_empty_message() {
    for table in ["v2", "v5", "v7"] {
        let rig = started(
            KsConfig {
                table,
                ..KsConfig::default()
            },
            1,
        )
        .await;
        assert!(rig.driver.table_candidates().contains(&table), "{table}");
        assert_eq!(last_speed(&rig), Some(3.5), "{table}");
    }
}

#[tokio::test]
async fn silent_treadmill_is_found_by_trying_tables() {
    let rig = started(
        KsConfig {
            table: "v3",
            answer_empty: false,
            ..KsConfig::default()
        },
        25,
    )
    .await;
    assert!(rig.driver.table_candidates().contains(&"v3"));
    assert_eq!(last_speed(&rig), Some(3.5));
}

#[tokio::test]
async fn driver_picks_the_channel_that_answers() {
    let rig = started(
        KsConfig {
            extra_silent_channel: true,
            ..KsConfig::default()
        },
        1,
    )
    .await;
    let active = rig.driver.active_channel().expect("kanał");
    assert!(active.starts_with("0002"), "{active}");
    assert_eq!(last_speed(&rig), Some(3.5));
}

#[tokio::test]
async fn ks_fit_telemetry_is_tried_first() {
    let rig = started(KsConfig::default(), 0).await;
    assert!(rig.fake.count(ks::TELEMETRY_PROPS) >= 1);
    assert_eq!(rig.driver.poll_command(), ks::TELEMETRY_PROPS);
}

/// Bieżnia co chwilę wysyła ramki z danymi (odpytywanie w tle, jak X21 co 2 s).
fn pusher(rig: &KsRig) -> tokio::task::JoinHandle<()> {
    let driver = rig.driver.clone();
    tokio::spawn(async move {
        loop {
            let _ = driver.poll().await;
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
}

#[tokio::test]
async fn start_belt_takes_control_and_starts() {
    let rig = started(
        KsConfig {
            running: false,
            ..KsConfig::default()
        },
        1,
    )
    .await;
    let result = rig.driver.start_belt().await.expect("start");
    assert_eq!(result, StartResult::Started);
    assert!(rig.fake.running());
    let received = rig.fake.received();
    let i = received
        .iter()
        .position(|t| t == ks::CMD_CONTROL)
        .expect("przejęcie sterowania");
    assert_eq!(received[i + 1], ks::CMD_START);
}

#[tokio::test]
async fn refused_start_is_retried() {
    let rig = started(
        KsConfig {
            running: false,
            refuse_start: true,
            ..KsConfig::default()
        },
        1,
    )
    .await;
    let result = rig.driver.start_belt().await.expect("start");
    assert_eq!(result, StartResult::Refused);
    assert_eq!(rig.driver.last_error(), Some(-5000));
    assert_eq!(
        rig.fake.count(ks::CMD_START),
        kingsmith::START_ATTEMPTS as usize
    );
    assert_eq!(
        rig.fake.count(ks::CMD_CONTROL),
        kingsmith::START_ATTEMPTS as usize
    );
}

#[tokio::test]
async fn stop_belt_stops() {
    let rig = started(KsConfig::default(), 1).await;
    let push = pusher(&rig);
    assert!(rig.driver.stop_belt(true).await.expect("stop"));
    push.abort();
    assert!(!rig.fake.running());
    assert_eq!(rig.driver.belt_running(), Some(false));
}

/// Zgłoszenie Velczera (KS-NACH-X21C): po stopie taśma hamuje 3,5 › 3,0 › 2,0 › 1,0 z runState 1
/// do końca - spadająca prędkość to potwierdzenie, bez ponawiania i bez alarmu.
#[tokio::test]
async fn braking_x21c_confirms_stop_without_resend() {
    let rig = started(
        KsConfig {
            braking: vec![3.5, 3.0, 2.0, 1.0, 0.0],
            ..KsConfig::default()
        },
        1,
    )
    .await;
    let push = pusher(&rig);
    assert!(rig.driver.stop_belt(true).await.expect("stop"));
    push.abort();
    assert!(!rig.fake.running());
    assert_eq!(rig.fake.count(ks::CMD_STOP), 1);
}

#[tokio::test]
async fn lost_start_is_resent_without_control_mode() {
    let rig = started(
        KsConfig {
            running: false,
            drop_commands: 1,
            ..KsConfig::default()
        },
        1,
    )
    .await;
    // z ramek: bieżnia już jest w trybie sterowania
    rig.fake
        .reply(&rig.fake.state.lock().expect("stan"), "props ControlMode 1");
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(rig.driver.control_mode(), Some(1));
    let push = pusher(&rig);
    let result = rig.driver.start_belt().await.expect("start");
    push.abort();
    assert_eq!(result, StartResult::Started);
    assert_eq!(rig.fake.count(ks::CMD_START), 2);
    assert_eq!(rig.fake.count(ks::CMD_CONTROL), 0);
}

#[tokio::test]
async fn running_belt_gets_no_start_command() {
    let rig = started(KsConfig::default(), 1).await;
    assert_eq!(rig.driver.belt_running(), Some(true));
    let result = rig.driver.start_belt().await.expect("start");
    assert_eq!(result, StartResult::Started);
    assert_eq!(rig.fake.count(ks::CMD_START), 0);
}

#[tokio::test]
async fn lost_stop_is_resent_and_quit_does_not_wait() {
    let rig = started(
        KsConfig {
            drop_commands: 1,
            ..KsConfig::default()
        },
        1,
    )
    .await;
    let push = pusher(&rig);
    assert!(rig.driver.stop_belt(true).await.expect("stop"));
    push.abort();
    assert!(!rig.fake.running());
    assert_eq!(rig.fake.count(ks::CMD_STOP), 2);
    let quit = started(
        KsConfig {
            drop_commands: 5,
            ..KsConfig::default()
        },
        1,
    )
    .await;
    assert!(quit.driver.stop_belt(false).await.expect("stop"));
    assert_eq!(quit.fake.count(ks::CMD_STOP), 1);
}

#[tokio::test]
async fn driver_sends_speed_and_resends_lost_speed_while_polling() {
    let rig = started(KsConfig::default(), 1).await;
    assert!(rig.driver.set_speed(2.5).await.expect("prędkość"));
    assert_eq!(
        rig.fake.received().last().map(String::as_str),
        Some("props CurrentSpeed 2.5")
    );
    // zegar sterownika ręczny - ponowienie po RESEND_S bez czekania
    let now = Arc::new(AtomicU64::new(100f64.to_bits()));
    let tick = now.clone();
    let clock: Clock = Arc::new(move || f64::from_bits(tick.load(Ordering::SeqCst)));
    let lossy = ks_rig(
        KsConfig {
            speed: 1.0,
            lose_first_speed: true,
            ..KsConfig::default()
        },
        clock,
    )
    .await;
    lossy.driver.start().await.expect("start");
    lossy.driver.poll().await.expect("odpytanie");
    tokio::time::sleep(Duration::from_millis(20)).await;
    lossy.driver.set_speed(2.5).await.expect("prędkość");
    for _ in 0..6 {
        let t = f64::from_bits(now.load(Ordering::SeqCst)) + 1.0;
        now.store(t.to_bits(), Ordering::SeqCst);
        lossy.driver.poll().await.expect("odpytanie");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let sent: Vec<String> = lossy
        .fake
        .received()
        .into_iter()
        .filter(|t| t.starts_with("props CurrentSpeed"))
        .collect();
    assert_eq!(sent.len(), 2, "{sent:?}");
    assert!(sent.iter().all(|t| t == "props CurrentSpeed 2.5"));
    assert_eq!(lossy.driver.belt_speed().map(|b| b.0), Some(2.5));
}

#[tokio::test]
async fn polling_calms_down_when_treadmill_pushes_data() {
    let rig = ks_rig(KsConfig::default(), monotonic_clock()).await;
    assert_eq!(rig.driver.poll_interval(), kingsmith::BUSY_POLL_S);
    rig.driver.start().await.expect("start");
    rig.driver.poll().await.expect("odpytanie");
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(rig.driver.poll_interval(), kingsmith::CALM_POLL_S);
}

// --- wirtualna bieżnia FTMS ---------------------------------------------------------------

struct FtmsState {
    speed: f64,
    running: bool,
    commands: Vec<Vec<u8>>,
    silent: u32,
}

struct FakeFtms {
    services: Vec<GattService>,
    state: Mutex<FtmsState>,
    radio: Radio,
}

impl FakeFtms {
    fn new(control: bool) -> Arc<Self> {
        let s = ftms::FTMS_SERVICE;
        let mut chars = vec![
            ch(s, ftms::TREADMILL_DATA, &["notify"]),
            ch(s, ftms::MACHINE_STATUS, &["notify"]),
            ch(s, ftms::SPEED_RANGE, &["read"]),
        ];
        if control {
            chars.push(ch(s, ftms::CONTROL_POINT, &["write", "indicate"]));
        }
        let info = ftms::DEVICE_INFO;
        Arc::new(Self {
            services: vec![
                GattService {
                    uuid: s.into(),
                    characteristics: chars,
                },
                GattService {
                    uuid: info.into(),
                    characteristics: vec![ch(info, ftms::FIRMWARE_REVISION, &["read"])],
                },
            ],
            state: Mutex::new(FtmsState {
                speed: 0.0,
                running: false,
                commands: Vec::new(),
                silent: 0,
            }),
            radio: Radio::default(),
        })
    }

    /// Ramka Treadmill Data jak z Urevo (0x259C): prędkość, dystans, czas, kroki.
    fn data(&self, speed: f64, distance: u32, elapsed: u16, steps: u16) {
        let mut raw = vec![0x9c, 0x25];
        raw.extend_from_slice(&((speed * 100.0).round() as u16).to_le_bytes());
        raw.extend_from_slice(&distance.to_le_bytes()[..3]);
        raw.extend_from_slice(&[0; 4 + 4]); // nachylenie, przewyższenie
        raw.extend_from_slice(&[0; 5]); // energia
        raw.push(0); // tętno
        raw.extend_from_slice(&elapsed.to_le_bytes());
        raw.extend_from_slice(&steps.to_le_bytes());
        raw.push(0);
        self.radio.notify(ftms::TREADMILL_DATA, raw);
    }
}

impl GattClient for FakeFtms {
    fn services(&self) -> Vec<GattService> {
        self.services.clone()
    }

    fn read<'a>(
        &'a self,
        characteristic: &'a GattCharacteristic,
    ) -> BoxFuture<'a, Result<Vec<u8>, DriverError>> {
        let value = match characteristic.uuid.as_str() {
            u if u == ftms::SPEED_RANGE => vec![0x64, 0x00, 0x58, 0x02, 0x0a, 0x00],
            u if u == ftms::FIRMWARE_REVISION => b"V90.08.15\x00".to_vec(),
            _ => Vec::new(),
        };
        async move { Ok(value) }.boxed()
    }

    fn write<'a>(
        &'a self,
        _characteristic: &'a GattCharacteristic,
        data: &'a [u8],
        _with_response: bool,
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        let mut s = self.state.lock().expect("stan");
        s.commands.push(data.to_vec());
        if s.silent > 0 {
            s.silent -= 1;
        } else {
            let op = data.first().copied().unwrap_or(0);
            self.radio.notify(
                ftms::CONTROL_POINT,
                vec![ftms::RESPONSE, op, ftms::RESULT_OK],
            );
            match op {
                ftms::OP_START => {
                    s.running = true;
                    s.speed = 1.0;
                    self.radio
                        .notify(ftms::MACHINE_STATUS, vec![ftms::STATUS_STARTED]);
                    self.data(1.0, 0, 0, 0);
                }
                ftms::OP_STOP_PAUSE => {
                    s.running = false;
                    s.speed = 0.0;
                    self.data(0.0, 0, 0, 0);
                    self.radio
                        .notify(ftms::MACHINE_STATUS, vec![ftms::STATUS_STOPPED, data[1]]);
                }
                ftms::OP_SET_SPEED => {
                    s.speed = f64::from(u16::from_le_bytes([data[1], data[2]])) / 100.0;
                }
                _ => {}
            }
        }
        async { Ok(()) }.boxed()
    }

    fn subscribe<'a>(
        &'a self,
        _characteristic: &'a GattCharacteristic,
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        async { Ok(()) }.boxed()
    }
}

impl BleLink for FakeFtms {
    fn notifications(
        &self,
    ) -> BoxFuture<'_, Result<mpsc::UnboundedReceiver<Notification>, BleError>> {
        let rx = self.radio.open();
        async { Ok(rx) }.boxed()
    }

    fn disconnect(&self) -> BoxFuture<'_, ()> {
        self.radio.drop_link();
        async {}.boxed()
    }
}

async fn ftms_rig(
    control: bool,
) -> (
    Arc<FakeFtms>,
    Arc<dyn Driver>,
    Arc<Mutex<Vec<TelemetryFrame>>>,
) {
    let fake = FakeFtms::new(control);
    let frames = Arc::new(Mutex::new(Vec::new()));
    let sink = frames.clone();
    let on_frame: FrameCallback = Arc::new(move |f| sink.lock().expect("ramki").push(f));
    let gatt: Arc<dyn GattClient> = fake.clone();
    let registry = ProfileRegistry::builtin();
    let driver = make_driver(gatt, "URTM054", &registry, on_frame, monotonic_clock())
        .expect("sterownik FTMS");
    let mut rx = fake.notifications().await.expect("powiadomienia");
    let pumped = driver.clone();
    tokio::spawn(async move {
        while let Some(n) = rx.recv().await {
            pumped.on_notification(&n.characteristic_uuid, &n.data);
        }
    });
    driver.start().await.expect("start");
    (fake, driver, frames)
}

#[tokio::test]
async fn ftms_driver_reads_range_firmware_and_full_frames() {
    let (fake, driver, frames) = ftms_rig(true).await;
    assert_eq!(driver.protocol(), "FTMS");
    assert_eq!(driver.info().model, "Urevo SpaceWalk 5L");
    assert_eq!(driver.info().firmware, "V90.08.15");
    let caps = driver.capabilities();
    assert_eq!((caps.speed_min, caps.speed_max), (1.0, 6.0));
    fake.data(2.5, 390, 763, 808);
    tokio::time::sleep(Duration::from_millis(20)).await;
    let last = *frames.lock().expect("ramki").last().expect("ramka");
    assert_eq!(
        (last.speed_kmh, last.distance_m, last.elapsed_s, last.steps),
        (Some(2.5), Some(390), Some(763), Some(808))
    );
    let spec = driver.telemetry_spec();
    assert_eq!(spec.steps.and_then(|s| s.modulus), Some(65536));
}

#[tokio::test]
async fn ftms_driver_starts_sets_speed_and_pauses() {
    let (fake, driver, _) = ftms_rig(true).await;
    assert_eq!(
        driver.start_belt().await.expect("start"),
        StartResult::Started
    );
    assert!(driver.set_speed(2.5).await.expect("prędkość"));
    assert_eq!(fake.state.lock().expect("stan").speed, 2.5);
    // poniżej minimum bieżni wysyłamy minimum
    assert!(driver.set_speed(0.5).await.expect("prędkość"));
    assert_eq!(fake.state.lock().expect("stan").speed, 1.0);
    assert!(driver.stop_belt(true).await.expect("stop"));
    let commands = fake.state.lock().expect("stan").commands.clone();
    assert_eq!(commands[0], ftms::encode_request_control());
    assert_eq!(commands.last(), Some(&ftms::encode_pause()));
    assert_eq!(driver.belt_running(), Some(false));
    let stats = driver.speed_stats().expect("statystyki");
    assert_eq!((stats.sent, stats.ok), (2, 2));
}

#[tokio::test]
async fn ftms_without_control_point_cannot_steer() {
    let (_, driver, _) = ftms_rig(false).await;
    assert!(!driver.capabilities().control);
    assert!(!driver.set_speed(2.0).await.expect("prędkość"));
}

#[tokio::test]
async fn ftms_driver_alone() {
    // sterownik zbudowany bez rozpoznawania (np. profil bez Control Point)
    let fake = FakeFtms::new(true);
    let gatt: Arc<dyn GattClient> = fake.clone();
    let caps = ftms::DEFAULT_CAPS;
    let driver = FtmsDriver::new(
        gatt,
        &fake.services(),
        caps,
        info("Bieżnia FTMS", DriverKind::Ftms),
        Arc::new(|_| {}),
        monotonic_clock(),
    );
    assert_eq!(driver.reachable_speed(9.0), 6.0);
    assert_eq!(driver.describe(), "Bieżnia FTMS (FTMS)");
}

#[tokio::test]
async fn unknown_device_is_unsupported() {
    struct Headphones;
    impl GattClient for Headphones {
        fn services(&self) -> Vec<GattService> {
            let s = format!("0000180f{BASE}");
            vec![GattService {
                characteristics: vec![ch(&s, &format!("00002a19{BASE}"), &["read"])],
                uuid: s,
            }]
        }
        fn read<'a>(
            &'a self,
            _c: &'a GattCharacteristic,
        ) -> BoxFuture<'a, Result<Vec<u8>, DriverError>> {
            async { Ok(Vec::new()) }.boxed()
        }
        fn write<'a>(
            &'a self,
            _c: &'a GattCharacteristic,
            _d: &'a [u8],
            _r: bool,
        ) -> BoxFuture<'a, Result<(), DriverError>> {
            async { Ok(()) }.boxed()
        }
        fn subscribe<'a>(
            &'a self,
            _c: &'a GattCharacteristic,
        ) -> BoxFuture<'a, Result<(), DriverError>> {
            async { Ok(()) }.boxed()
        }
    }
    let registry = ProfileRegistry::builtin();
    let result = make_driver(
        Arc::new(Headphones),
        "JBL",
        &registry,
        Arc::new(|_| {}),
        monotonic_clock(),
    );
    assert!(matches!(result, Err(DriverError::UnsupportedDevice)));
}

// --- worker na atrapie Bluetooth -----------------------------------------------------------

/// Atrapa radia: skan z listą urządzeń i połączenie z wirtualną bieżnią.
struct FakeBackend {
    ads: Vec<Advertisement>,
    link: Arc<dyn BleLink>,
    connects: AtomicU64,
    scan_fails: bool,
}

impl FakeBackend {
    fn new(ads: Vec<Advertisement>, link: Arc<dyn BleLink>) -> Self {
        Self {
            ads,
            link,
            connects: AtomicU64::new(0),
            scan_fails: false,
        }
    }
}

impl BleBackend for FakeBackend {
    fn scan(&self, _duration: Duration) -> BoxFuture<'_, Result<Vec<Advertisement>, BleError>> {
        let ads = self.ads.clone();
        let fails = self.scan_fails;
        async move {
            if fails {
                Err(BleError::Other("skan przerwany".into()))
            } else {
                Ok(ads)
            }
        }
        .boxed()
    }

    fn find(
        &self,
        address: String,
        name: String,
        _timeout: Duration,
    ) -> BoxFuture<'_, Result<Option<Advertisement>, BleError>> {
        let found = walklevel_core::treadmill::ble::pick(&self.ads, &address, &name).cloned();
        async move { Ok(found) }.boxed()
    }

    fn connect(
        &self,
        _address: String,
        _timeout: Duration,
    ) -> BoxFuture<'_, Result<Arc<dyn BleLink>, BleError>> {
        self.connects.fetch_add(1, Ordering::SeqCst);
        let link = self.link.clone();
        async move { Ok(link) }.boxed()
    }
}

fn ad(address: &str, name: Option<&str>, rssi: i16) -> Advertisement {
    Advertisement {
        address: address.into(),
        name: name.map(String::from),
        rssi: Some(rssi),
        ..Advertisement::default()
    }
}

fn fast_worker() -> WorkerTiming {
    WorkerTiming {
        scan_s: 0.0,
        find_s: 0.1,
        connect_s: 1.0,
        retry_max_s: 0.2,
        retry_step_s: 0.05,
        diagnostics_listen_s: 0.2,
    }
}

/// Następne zdarzenie spełniające warunek (inne po drodze pomijamy).
async fn wait_for(
    events: &mut mpsc::UnboundedReceiver<WorkerEvent>,
    seen: &mut Vec<WorkerEvent>,
    want: impl Fn(&WorkerEvent) -> bool,
) -> WorkerEvent {
    let found = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(e) = events.recv().await {
            seen.push(e.clone());
            if want(&e) {
                return Some(e);
            }
        }
        None
    })
    .await;
    match found {
        Ok(Some(e)) => e,
        _ => panic!("brak zdarzenia; były: {seen:?}"),
    }
}

fn is_link(link: Link) -> impl Fn(&WorkerEvent) -> bool {
    move |e| matches!(e, WorkerEvent::Link { link: l, .. } if *l == link)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn worker_scans_connects_streams_and_reconnects() {
    let fake = FakeKs::new(KsConfig {
        prefix: "0001",
        ..KsConfig::default()
    });
    let backend = Arc::new(FakeBackend::new(
        vec![
            ad("11:22:33:44:55:66", Some("JBL Flip 5"), -40),
            ad("AA:BB:CC:DD:EE:FF", Some("KS-HDSY-X21C"), -70),
            ad("00:00:00:00:00:01", None, -30),
        ],
        fake.clone(),
    ));
    let (worker, mut events) = BleWorker::spawn_with(
        &tokio::runtime::Handle::current(),
        backend.clone(),
        ProfileRegistry::builtin(),
        monotonic_clock(),
        fast_worker(),
    );
    let mut seen = Vec::new();
    // bez połączenia polecenia taśmy nie mają dokąd iść
    worker.belt(BeltCommand::Start);
    wait_for(&mut events, &mut seen, |e| {
        *e == WorkerEvent::Belt(BeltOutcome::NotConnected)
    })
    .await;
    assert!(!worker.stop_belt_now(Duration::from_secs(1)).await);
    worker.scan();
    let WorkerEvent::Devices(devices) = wait_for(&mut events, &mut seen, |e| {
        matches!(e, WorkerEvent::Devices(_))
    })
    .await
    else {
        unreachable!()
    };
    let names: Vec<&str> = devices.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["KS-HDSY-X21C", "JBL Flip 5", ""]); // bieżnie pierwsze, bez nazwy na końcu
    assert!(devices[0].treadmill);
    assert_eq!(devices[0].model, "KingSmith WalkingPad X21");
    wait_for(&mut events, &mut seen, is_link(Link::Idle)).await;
    // adres zmieniony - zapamiętana bieżnia znaleziona po nazwie
    worker.connect("AA:BB:CC:DD:EE:00", "KS-HDSY-X21C");
    let device = wait_for(&mut events, &mut seen, |e| {
        matches!(e, WorkerEvent::Device { .. })
    })
    .await;
    let WorkerEvent::Device { address, info, .. } = device else {
        unreachable!()
    };
    assert_eq!(address, "AA:BB:CC:DD:EE:FF");
    assert_eq!(info.model, "KingSmith WalkingPad X21");
    wait_for(&mut events, &mut seen, is_link(Link::Streaming)).await;
    wait_for(
        &mut events,
        &mut seen,
        |e| matches!(e, WorkerEvent::Frame(f) if f.speed_kmh == Some(3.5)),
    )
    .await;
    let order: Vec<Link> = seen
        .iter()
        .filter_map(|e| match e {
            WorkerEvent::Link { link, .. } => Some(*link),
            _ => None,
        })
        .collect();
    let tail = &order[order.len() - 4..];
    assert_eq!(
        tail,
        [
            Link::Searching,
            Link::Connecting,
            Link::Connected,
            Link::Streaming
        ]
    );
    // zerwane połączenie: ponawianie i znowu dane
    fake.radio.drop_link();
    wait_for(&mut events, &mut seen, is_link(Link::Retrying)).await;
    wait_for(&mut events, &mut seen, is_link(Link::Streaming)).await;
    assert!(backend.connects.load(Ordering::SeqCst) >= 2);
    // stop przy zamykaniu
    assert!(worker.stop_belt_now(Duration::from_secs(5)).await);
    assert!(!fake.running());
    worker.disconnect();
    wait_for(&mut events, &mut seen, is_link(Link::Idle)).await;
    worker.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn worker_reports_unsupported_device_and_stops_trying() {
    let ftms_like = FakeFtms::new(true);
    // urządzenie bez usług bieżni
    struct Plain(Arc<FakeFtms>);
    impl GattClient for Plain {
        fn services(&self) -> Vec<GattService> {
            Vec::new()
        }
        fn read<'a>(
            &'a self,
            c: &'a GattCharacteristic,
        ) -> BoxFuture<'a, Result<Vec<u8>, DriverError>> {
            self.0.read(c)
        }
        fn write<'a>(
            &'a self,
            c: &'a GattCharacteristic,
            d: &'a [u8],
            r: bool,
        ) -> BoxFuture<'a, Result<(), DriverError>> {
            self.0.write(c, d, r)
        }
        fn subscribe<'a>(
            &'a self,
            c: &'a GattCharacteristic,
        ) -> BoxFuture<'a, Result<(), DriverError>> {
            self.0.subscribe(c)
        }
    }
    impl BleLink for Plain {
        fn notifications(
            &self,
        ) -> BoxFuture<'_, Result<mpsc::UnboundedReceiver<Notification>, BleError>> {
            self.0.notifications()
        }
        fn disconnect(&self) -> BoxFuture<'_, ()> {
            self.0.disconnect()
        }
    }
    let backend = Arc::new(FakeBackend::new(
        vec![ad("AA:00:00:00:00:01", Some("JBL"), -50)],
        Arc::new(Plain(ftms_like)),
    ));
    let (worker, mut events) = BleWorker::spawn_with(
        &tokio::runtime::Handle::current(),
        backend.clone(),
        ProfileRegistry::builtin(),
        monotonic_clock(),
        fast_worker(),
    );
    let mut seen = Vec::new();
    worker.connect("AA:00:00:00:00:01", "JBL");
    let event = wait_for(&mut events, &mut seen, is_link(Link::Unsupported)).await;
    let WorkerEvent::Link { text, .. } = event else {
        unreachable!()
    };
    assert!(
        text.starts_with("To urządzenie nie wygląda na bieżnię."),
        "{text}"
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(backend.connects.load(Ordering::SeqCst), 1);
    worker.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn worker_drives_simulator_like_a_treadmill() {
    let backend = Arc::new(FakeBackend::new(
        Vec::new(),
        FakeKs::new(KsConfig::default()),
    ));
    let (worker, mut events) = BleWorker::spawn_with(
        &tokio::runtime::Handle::current(),
        backend,
        ProfileRegistry::builtin(),
        monotonic_clock(),
        fast_worker(),
    );
    let mut seen = Vec::new();
    worker.connect_simulator();
    let device = wait_for(&mut events, &mut seen, |e| {
        matches!(e, WorkerEvent::Device { .. })
    })
    .await;
    let WorkerEvent::Device { info, spec, .. } = device else {
        unreachable!()
    };
    assert!(info.simulated());
    assert_eq!(spec.telemetry_s, 0.75);
    wait_for(&mut events, &mut seen, is_link(Link::Streaming)).await;
    worker.set_speed(2.0, true); // gałka budzi stojącą taśmę
    wait_for(&mut events, &mut seen, |e| {
        *e == WorkerEvent::Belt(BeltOutcome::Started)
    })
    .await;
    wait_for(
        &mut events,
        &mut seen,
        |e| matches!(e, WorkerEvent::Frame(f) if f.speed_kmh.unwrap_or(0.0) >= 1.5),
    )
    .await;
    worker.sim_remote(1);
    worker.belt(BeltCommand::Stop);
    wait_for(&mut events, &mut seen, |e| {
        *e == WorkerEvent::Belt(BeltOutcome::Stopped)
    })
    .await;
    worker.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn diagnostics_report_lists_devices_and_raw_notifications() {
    let fake = FakeFtms::new(true);
    let backend = FakeBackend::new(
        vec![ad("AA:BB:CC:00:11:22", Some("URTM054"), -60)],
        fake.clone(),
    );
    let dir = std::env::temp_dir().join(format!("walklevel-raport-{}", std::process::id()));
    let path = dir.join("raport.txt");
    let pusher = {
        let fake = fake.clone();
        tokio::spawn(async move {
            loop {
                fake.data(2.5, 10, 5, 7);
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
    };
    let written = walklevel_core::treadmill::ble::run_diagnostics(
        &backend,
        &ProfileRegistry::builtin(),
        &path,
        None,
        monotonic_clock(),
        0.2,
    )
    .await
    .expect("raport");
    pusher.abort();
    let text = std::fs::read_to_string(&written).expect("plik");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(text.contains("<-- bieżnia?"), "{text}");
    assert!(text.contains("== Połączenie z AA:BB:CC:00:11:22 =="));
    assert!(text.contains("'V90.08.15'"));
    assert!(text.contains("<< 00002acd"));
}

/// Bieżnia znika bez zgłoszenia rozłączenia (zapisy GATT się nie udają): po kilku nieudanych
/// odpytaniach worker łączy od nowa, jak 0.5 przy błędzie odpytania.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn worker_reconnects_when_polling_keeps_failing() {
    let fake = FakeKs::new(KsConfig::default());
    let backend = Arc::new(FakeBackend::new(
        vec![ad("AA:BB:CC:DD:EE:FF", Some("KS-HDSY-X21C"), -60)],
        fake.clone(),
    ));
    let (worker, mut events) = BleWorker::spawn_with(
        &tokio::runtime::Handle::current(),
        backend.clone(),
        ProfileRegistry::builtin(),
        monotonic_clock(),
        fast_worker(),
    );
    let mut seen = Vec::new();
    worker.connect("AA:BB:CC:DD:EE:FF", "KS-HDSY-X21C");
    wait_for(&mut events, &mut seen, is_link(Link::Streaming)).await;
    fake.fail_writes(true);
    let event = wait_for(&mut events, &mut seen, is_link(Link::Retrying)).await;
    let WorkerEvent::Link { text, .. } = event else {
        unreachable!()
    };
    assert!(
        text.starts_with("Problem z Bluetooth. Łączę ponownie…"),
        "{text}"
    );
    fake.fail_writes(false);
    wait_for(&mut events, &mut seen, is_link(Link::Streaming)).await;
    assert!(backend.connects.load(Ordering::SeqCst) >= 2);
    worker.shutdown(Duration::from_secs(2)).await;
}

/// Nieudany skan nie zostawia zapamiętanej bieżni rozłączonej.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_scan_resumes_the_paused_connection() {
    let fake = FakeKs::new(KsConfig::default());
    let backend = Arc::new(FakeBackend {
        scan_fails: true,
        ..FakeBackend::new(
            vec![ad("AA:BB:CC:DD:EE:FF", Some("KS-HDSY-X21C"), -60)],
            fake.clone(),
        )
    });
    let (worker, mut events) = BleWorker::spawn_with(
        &tokio::runtime::Handle::current(),
        backend.clone(),
        ProfileRegistry::builtin(),
        monotonic_clock(),
        fast_worker(),
    );
    let mut seen = Vec::new();
    worker.connect("AA:BB:CC:DD:EE:FF", "KS-HDSY-X21C");
    wait_for(&mut events, &mut seen, is_link(Link::Streaming)).await;
    worker.scan();
    let failed = wait_for(&mut events, &mut seen, |e| {
        matches!(e, WorkerEvent::ScanFailed { .. })
    })
    .await;
    assert_eq!(
        failed,
        WorkerEvent::ScanFailed {
            link: Link::Retrying,
            text: "Problem z Bluetooth. Łączę ponownie… (skan przerwany)".into()
        }
    );
    wait_for(&mut events, &mut seen, is_link(Link::Streaming)).await;
    assert_eq!(backend.connects.load(Ordering::SeqCst), 2);
    worker.shutdown(Duration::from_secs(2)).await;
}

/// Stop z okna kasuje prośbę o prędkość i budzenie taśmy - taśma zostaje zatrzymana, choć
/// odpytywanie dalej widzi stojącą taśmę (port test_stop_cancels_wake_so_belt_stays_stopped).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_cancels_wake_so_belt_stays_stopped() {
    let backend = Arc::new(FakeBackend::new(
        Vec::new(),
        FakeKs::new(KsConfig::default()),
    ));
    let (worker, mut events) = BleWorker::spawn_with(
        &tokio::runtime::Handle::current(),
        backend,
        ProfileRegistry::builtin(),
        monotonic_clock(),
        fast_worker(),
    );
    let mut seen = Vec::new();
    worker.connect_simulator();
    wait_for(&mut events, &mut seen, is_link(Link::Streaming)).await;
    worker.set_speed(2.0, true);
    wait_for(&mut events, &mut seen, |e| {
        *e == WorkerEvent::Belt(BeltOutcome::Started)
    })
    .await;
    worker.belt(BeltCommand::Stop);
    wait_for(&mut events, &mut seen, |e| {
        *e == WorkerEvent::Belt(BeltOutcome::Stopped)
    })
    .await;
    // kilka odpytań symulatora (co 0,75 s): taśma stoi i nikt jej nie budzi
    let quiet = tokio::time::timeout(Duration::from_millis(2500), async {
        while let Some(e) = events.recv().await {
            if matches!(e, WorkerEvent::Belt(_)) {
                return Some(e);
            }
        }
        None
    })
    .await;
    assert!(quiet.is_err(), "nieoczekiwane polecenie taśmy: {quiet:?}");
    assert!(!worker.stop_belt_now(Duration::from_secs(1)).await); // taśma już stoi
    worker.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn simulator_comes_back_after_scan_and_diagnostics() {
    let fake = FakeKs::new(KsConfig::default());
    let backend = Arc::new(FakeBackend::new(
        vec![ad("11:22:33:44:55:66", Some("JBL Flip 5"), -40)],
        fake.clone(),
    ));
    let (worker, mut events) = BleWorker::spawn_with(
        &tokio::runtime::Handle::current(),
        backend,
        ProfileRegistry::builtin(),
        monotonic_clock(),
        fast_worker(),
    );
    let mut seen = Vec::new();
    worker.connect_simulator();
    wait_for(&mut events, &mut seen, is_link(Link::Streaming)).await;
    // skan przy włączonym symulatorze: lista urządzeń, potem symulator znowu nadaje
    worker.scan();
    wait_for(&mut events, &mut seen, |e| {
        matches!(e, WorkerEvent::Devices(_))
    })
    .await;
    wait_for(&mut events, &mut seen, is_link(Link::Streaming)).await;
    // raport diagnostyczny: tak samo (w 0.5.x taśma zostawała „niepołączona”)
    let dir = tempfile::tempdir().unwrap();
    worker.diagnose(dir.path().join("raport.txt"));
    wait_for(&mut events, &mut seen, |e| {
        matches!(e, WorkerEvent::Report(_))
    })
    .await;
    wait_for(&mut events, &mut seen, is_link(Link::Streaming)).await;
    worker.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test]
async fn kingsmith_reports_belt_speed_in_kmh_on_imperial_treadmill() {
    let rig = started(
        KsConfig {
            speed: 2.0,
            miles: true,
            ..KsConfig::default()
        },
        1,
    )
    .await;
    let kmh = rig.driver.belt_speed().map(|b| b.0);
    assert_eq!(kmh, Some(3.22), "2.0 mph is 3.22 km/h");
}

fn sim_worker() -> (BleWorker, mpsc::UnboundedReceiver<WorkerEvent>) {
    let backend = Arc::new(FakeBackend::new(
        Vec::new(),
        FakeKs::new(KsConfig::default()),
    ));
    BleWorker::spawn_with(
        &tokio::runtime::Handle::current(),
        backend,
        ProfileRegistry::builtin(),
        monotonic_clock(),
        fast_worker(),
    )
}

/// True when the belt reports Started within `within`.
async fn belt_starts_within(
    events: &mut mpsc::UnboundedReceiver<WorkerEvent>,
    within: Duration,
) -> bool {
    tokio::time::timeout(within, async {
        while let Some(e) = events.recv().await {
            if e == WorkerEvent::Belt(BeltOutcome::Started) {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_while_disconnected_keeps_belt_stopped_after_reconnect() {
    let (worker, mut events) = sim_worker();
    let mut seen = Vec::new();
    worker.set_speed(2.0, true);
    worker.belt(BeltCommand::Stop);
    wait_for(&mut events, &mut seen, |e| {
        *e == WorkerEvent::Belt(BeltOutcome::NotConnected)
    })
    .await;
    worker.connect_simulator();
    wait_for(&mut events, &mut seen, is_link(Link::Streaming)).await;
    assert!(
        !belt_starts_within(&mut events, Duration::from_secs(3)).await,
        "belt started by itself after reconnecting"
    );
    worker.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_disconnect_forgets_speed_request() {
    let (worker, mut events) = sim_worker();
    let mut seen = Vec::new();
    worker.connect_simulator();
    wait_for(&mut events, &mut seen, is_link(Link::Streaming)).await;
    worker.set_speed(2.0, true);
    wait_for(&mut events, &mut seen, |e| {
        *e == WorkerEvent::Belt(BeltOutcome::Started)
    })
    .await;
    worker.disconnect();
    wait_for(&mut events, &mut seen, is_link(Link::Idle)).await;
    worker.connect_simulator();
    wait_for(&mut events, &mut seen, is_link(Link::Streaming)).await;
    assert!(
        !belt_starts_within(&mut events, Duration::from_secs(3)).await,
        "belt started by itself after reconnecting"
    );
    worker.shutdown(Duration::from_secs(2)).await;
}

/// FTMS worker connected to a running belt.
async fn ftms_worker_running(
    fake: &Arc<FakeFtms>,
) -> (
    BleWorker,
    mpsc::UnboundedReceiver<WorkerEvent>,
    Vec<WorkerEvent>,
) {
    let backend = Arc::new(FakeBackend::new(
        vec![ad("AA:00:00:00:00:05", Some("URTM054"), -50)],
        fake.clone(),
    ));
    let (worker, mut events) = BleWorker::spawn_with(
        &tokio::runtime::Handle::current(),
        backend,
        ProfileRegistry::builtin(),
        monotonic_clock(),
        fast_worker(),
    );
    let mut seen = Vec::new();
    worker.connect("AA:00:00:00:00:05", "URTM054");
    wait_for(&mut events, &mut seen, is_link(Link::Streaming)).await;
    worker.belt(BeltCommand::Start);
    wait_for(&mut events, &mut seen, |e| {
        *e == WorkerEvent::Belt(BeltOutcome::Started)
    })
    .await;
    // past the minimum gap between FTMS commands
    tokio::time::sleep(Duration::from_millis(1500)).await;
    (worker, events, seen)
}

fn ftms_ops(fake: &FakeFtms) -> Vec<u8> {
    let s = fake.state.lock().expect("stan");
    s.commands
        .iter()
        .filter_map(|c| c.first().copied())
        .collect()
}

/// Time until the treadmill receives `op` after its first `skip` commands.
async fn time_until_op(fake: &FakeFtms, skip: usize, op: u8, limit: Duration) -> Duration {
    let t0 = std::time::Instant::now();
    while t0.elapsed() < limit {
        if ftms_ops(fake).iter().skip(skip).any(|o| *o == op) {
            return t0.elapsed();
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    t0.elapsed()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_goes_out_at_once_while_speed_waits_for_lost_answers() {
    let fake = FakeFtms::new(true);
    let (worker, _events, _seen) = ftms_worker_running(&fake).await;
    fake.state.lock().expect("stan").silent = 2; // both speed attempts lose their answer
    worker.set_speed(3.0, false);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let before = ftms_ops(&fake).len();
    worker.belt(BeltCommand::Stop);
    let took = time_until_op(&fake, before, ftms::OP_STOP_PAUSE, Duration::from_secs(10)).await;
    assert!(
        took < Duration::from_millis(300),
        "stop reached the treadmill after {took:?}"
    );
    worker.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_now_on_quit_is_not_delayed_by_unanswered_speed() {
    let fake = FakeFtms::new(true);
    let (worker, _events, _seen) = ftms_worker_running(&fake).await;
    fake.state.lock().expect("stan").silent = 1;
    worker.set_speed(3.0, false);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(worker.stop_belt_now(Duration::from_secs(1)).await);
    assert!(!fake.state.lock().expect("stan").running);
    worker.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stop_now_during_start_sends_stop() {
    let fake = FakeFtms::new(true);
    let (worker, mut events, mut seen) = ftms_worker_running(&fake).await;
    worker.belt(BeltCommand::Stop);
    wait_for(&mut events, &mut seen, |e| {
        *e == WorkerEvent::Belt(BeltOutcome::Stopped)
    })
    .await;
    // the start reaches the treadmill, but its answer is lost
    fake.state.lock().expect("stan").silent = 1;
    let before = ftms_ops(&fake).len();
    worker.belt(BeltCommand::Start);
    time_until_op(&fake, before, ftms::OP_START, Duration::from_secs(2)).await;
    let before = ftms_ops(&fake).len();
    assert!(worker.stop_belt_now(Duration::from_secs(2)).await);
    assert!(
        ftms_ops(&fake)
            .iter()
            .skip(before)
            .any(|o| *o == ftms::OP_STOP_PAUSE),
        "no stop after a start that may have reached the treadmill"
    );
    worker.shutdown(Duration::from_secs(2)).await;
}
