//! Stan aplikacji: moduły rdzenia złożone w jeden obiekt za `Mutex` (odpowiednik klasy `App`
//! z 0.5.x bez okna), wycinki z kontraktu (rozdz. 4) i ich emisja zdarzeniem `wl:stan`.
//!
//! Zasada: każda zmiana oznacza wycinek jako brudny; [`Core::flush`] buduje każdy brudny
//! wycinek raz i wysyła go poza blokadą stanu (rosnący `rev`). Komendy i pętla wołają
//! `flush` po swojej zmianie.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::{Map, Value, json};
use walklevel_core::account::{AccountState, AccountTimers};
use walklevel_core::config::Config;
use walklevel_core::history::{History, SessionSource, SessionTracker};
use walklevel_core::input::{Hotkeys, Key, PadReader, PadState};
use walklevel_core::levels::{Definitions, Status};
use walklevel_core::mapping::{BeltSteering, Bridge, DryOutput, MappingConfig, OutputKind};
use walklevel_core::paths::AppPaths;
use walklevel_core::store::{self, Store, palette};
use walklevel_core::treadmill::{
    Capabilities, Clock, DeviceInfo, FoundDevice, Link, ProfileRegistry,
};

use crate::okna::{Okno, UiSink};
use crate::popupy::Popupy;
use crate::teksty;
use crate::widoki::*;
use crate::zrodlo::Bieznia;

pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Co wysłać do okien po zmianie (poza blokadą stanu).
pub enum Wyjscie {
    Toast(Toast),
    Nav(NavRequest),
    Popups(PopupsView),
    Okno(Okno),
}

/// Ostatnie linie Dziennika (wl:log) - wspólne z odbiorcą logu.
pub const LOG_TAIL: usize = 500;

/// Uchwyt rdzenia dla komend (`tauri::State<Core>`) i pętli.
#[derive(Clone)]
pub struct Core(pub Arc<Shared>);

pub struct Shared {
    pub app: Mutex<App>,
    pub konto: Mutex<crate::konto::KontoCtx>,
    pub sink: Box<dyn UiSink>,
    pub handle: tauri::AppHandle,
    pub clock: Clock,
    rev: AtomicU64,
    flush_lock: Mutex<()>,
    /// nowe linie logu od odbiorcy (wysyłane z pętli)
    pub log_new: Mutex<Vec<String>>,
    pub log_tail: Mutex<VecDeque<String>>,
    pub main_visible: AtomicBool,
    pub quitting: AtomicBool,
}

impl Core {
    pub fn new(
        app: App,
        konto: crate::konto::KontoCtx,
        sink: Box<dyn UiSink>,
        handle: tauri::AppHandle,
        clock: Clock,
    ) -> Self {
        Self(Arc::new(Shared {
            app: Mutex::new(app),
            konto: Mutex::new(konto),
            sink,
            handle,
            clock,
            rev: AtomicU64::new(1),
            flush_lock: Mutex::new(()),
            log_new: Mutex::new(Vec::new()),
            log_tail: Mutex::new(VecDeque::with_capacity(LOG_TAIL)),
            main_visible: AtomicBool::new(false),
            quitting: AtomicBool::new(false),
        }))
    }

    pub fn now(&self) -> f64 {
        (self.0.clock)()
    }

    /// Zmiana stanu i od razu emisja skutków.
    pub fn with<R>(&self, f: impl FnOnce(&mut App) -> R) -> R {
        let r = f(&mut lock(&self.0.app));
        self.flush();
        r
    }

    /// Brudne wycinki i wyjścia do okien - poza blokadą stanu.
    pub fn flush(&self) {
        let _order = lock(&self.0.flush_lock);
        let (slices, out) = {
            let mut app = lock(&self.0.app);
            let dirty = std::mem::take(&mut app.dirty);
            let slices: Vec<(SliceName, Value)> =
                dirty.into_iter().map(|s| (s, app.slice_value(s))).collect();
            (slices, std::mem::take(&mut app.out))
        };
        for (slice, value) in slices {
            let rev = self.0.rev.fetch_add(1, Ordering::SeqCst) + 1;
            self.0.sink.slice(&SliceEvent { rev, slice, value });
        }
        for o in out {
            match o {
                Wyjscie::Toast(t) => self.0.sink.toast(&t),
                Wyjscie::Nav(n) => self.0.sink.navigate(&n),
                Wyjscie::Popups(p) => self.0.sink.popups(&p),
                Wyjscie::Okno(w) => self.0.sink.window(&w),
            }
        }
    }

    /// Pełny stan (`app_snapshot`): `rev` obecny - zdarzenia z większym numerem są nowsze.
    pub fn snapshot(&self) -> Value {
        let _order = lock(&self.0.flush_lock);
        let mut app = lock(&self.0.app);
        app.dirty.clear(); // wszystko i tak idzie w snapshot
        let mut map = Map::new();
        for s in SliceName::ALL {
            let key = serde_json::to_value(s)
                .ok()
                .and_then(|v| v.as_str().map(String::from));
            if let Some(key) = key {
                map.insert(key, app.slice_value(s));
            }
        }
        map.insert("rev".into(), json!(self.0.rev.load(Ordering::SeqCst)));
        map.insert("statics".into(), app.statics());
        let tail: Vec<String> = lock(&self.0.log_tail).iter().cloned().collect();
        map.insert("logTail".into(), json!(tail));
        Value::Object(map)
    }
}

/// Wiersz konta osoby w pamięci aplikacji (kopia stanu z `KontoCtx`, czytana bez sieci).
#[derive(Debug, Clone, Default)]
pub struct KontoWidok {
    pub state: AccountState,
    pub pending: u32,
}

/// Gra wybrana w oknie wyboru, czekająca na zapis profilu.
#[derive(Debug, Clone)]
pub struct Wybor {
    pub exe: String,
    pub icon_file: Option<String>,
    pub icon_color: String,
    pub name: String,
}

pub struct App {
    pub paths: AppPaths,
    pub config: Config,
    pub defaults: Map<String, Value>,
    /// płaskie ustawienia aktywnej osoby i profilu (Store.flat + normalize)
    pub settings: Map<String, Value>,
    pub store: Option<Store>,
    pub legacy: Option<Map<String, Value>>,
    pub history: Option<History>,
    pub tracker: SessionTracker,
    pub defs: &'static Definitions,

    pub bridge: Bridge,
    pub steering: BeltSteering,
    pub toggle_key: Key,
    pub walk_key: Key,
    pub run_key: Key,
    pub hotkeys: Option<Hotkeys>,
    pub pad: PadReader,
    pub pad_state: Option<PadState>,
    pub pad_buttons: BTreeSet<String>,
    pub pad_capture_until: f64,
    pub key_capturing: Option<&'static str>,
    pub belt_target: Option<f64>,
    pub hint_override: Option<String>,
    /// najnowszy odczyt z ramek od poprzedniej klatki
    pub pending_reading: Option<walklevel_core::mapping::Reading>,

    pub bieznia: Bieznia,
    pub registry: ProfileRegistry,
    pub link: Link,
    pub link_text: String,
    pub device: Option<DeviceInfo>,
    pub caps: Capabilities,
    pub devices: Vec<FoundDevice>,
    pub scanning: bool,
    pub scan_text: String,
    pub auto_connect: bool,
    /// skan anulowany z karty stanu - jego wynik nie otwiera listy urządzeń
    pub scan_cancelled: bool,
    pub diagnostics: Diagnostics,
    pub recorder: Option<crate::petla::Nagrywanie>,

    pub levels: Option<Status>,
    pub levels_key: Option<String>,
    pub levels_quiet: bool,
    pub popups: Popupy,

    pub accounts_enabled: bool,
    pub konta: HashMap<String, KontoWidok>,
    /// trwające zadania konta: "osoba:akcja"
    pub account_busy: HashSet<String>,
    pub account_message: String,
    pub account_timers: AccountTimers,
    pub verify_until: f64,
    pub verify_at: f64,
    pub verify_checked_at: f64,
    pub live_on: bool,
    pub online: OnlineView,
    pub ranking_scope: String,

    pub update: UpdateView,
    pub report: ReportView,
    pub models: ModelsView,
    pub picks: HashMap<String, Wybor>,
    pub import_preview: Option<walklevel_core::backup::Preview>,

    pub first_run: bool,
    pub updated_from: Option<String>,
    pub window_focused: bool,
    pub bubble_dragging: bool,
    /// ostatnio wysłana karta statusu bieżni (żeby nie wysyłać jej 4 razy na sekundę)
    pub status_key: String,
    pub frame: u64,
    pub people_rev: u64,
    pub dirty: BTreeSet<SliceName>,
    pub out: Vec<Wyjscie>,
}

// --- ustawienia domyślne i normalizacja (app.py defaults_from / normalize) -----------------

fn section(config: &Config, name: &str) -> Map<String, Value> {
    config.section(name).cloned().unwrap_or_default()
}

fn get_or(m: &Map<String, Value>, key: &str, default: Value) -> Value {
    m.get(key).cloned().unwrap_or(default)
}

pub fn defaults_from(config: &Config) -> Map<String, Value> {
    let mapping = section(config, "mapping");
    let output = section(config, "output");
    let ble = section(config, "ble");
    let ui = section(config, "ui");
    let mut d = Map::new();
    let mut put = |k: &str, v: Value| {
        d.insert(k.into(), v);
    };
    put("stand_below", get_or(&mapping, "stand_below", json!(0.8)));
    put("run_on", get_or(&mapping, "run_on", json!(6.3)));
    put("run_off", get_or(&mapping, "run_off", json!(5.7)));
    put("quick_stop", get_or(&mapping, "quick_stop", json!(true)));
    put("output", get_or(&output, "type", json!("dry")));
    put("device_address", get_or(&ble, "device_address", json!("")));
    put("device_name", get_or(&ble, "device_name", json!("")));
    put("simulator", get_or(&ui, "simulator", json!(false)));
    put("record", get_or(&ui, "record", json!(false)));
    put("always_on_top", get_or(&ui, "always_on_top", json!(false)));
    put(
        "floating_button",
        get_or(&ui, "floating_button", json!(false)),
    );
    put("floating_pos", get_or(&ui, "floating_pos", Value::Null));
    put("theme", get_or(&ui, "theme", json!("system")));
    put("large_text", get_or(&ui, "large_text", json!(false)));
    put("detailed_logs", get_or(&ui, "detailed_logs", json!(true)));
    put("last_version", get_or(&ui, "last_version", json!("")));
    put("window_geometry", Value::Null);
    put("walk_key", get_or(&output, "walk_key", Value::Null));
    put("run_key", get_or(&output, "run_key", Value::Null));
    put("toggle_key", get_or(&output, "toggle_key", Value::Null));
    put("toggle_pad", get_or(&output, "toggle_pad", json!("")));
    put("belt_control", get_or(&ble, "belt_control", json!(true)));
    put("direction", get_or(&output, "direction", json!("keyboard")));
    put("idle_speed", get_or(&ble, "idle_speed", json!(1.0)));
    put("walk_speed", get_or(&ble, "walk_speed", json!(1.5)));
    put("run_speed", get_or(&ble, "run_speed", json!(2.5)));
    put("pad_steps", get_or(&ble, "pad_steps", Value::Null));
    put("pad_idle", get_or(&ble, "pad_idle", json!("stop")));
    put(
        "training_levels",
        get_or(&ble, "training_levels", Value::Null),
    );
    put("training_level", json!(0));
    put("show_person_switch", json!(true));
    put("show_profile_switch", json!(true));
    put(
        "achievement_popups",
        get_or(&ui, "achievement_popups", json!(true)),
    );
    put(
        "achievement_sound",
        get_or(&ui, "achievement_sound", json!(false)),
    );
    normalize(d)
}

fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|x| x != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

/// Wartości spoza zakresu (stary albo ręcznie zmieniony plik) -> bezpieczne domyślne.
pub fn normalize(mut s: Map<String, Value>) -> Map<String, Value> {
    use walklevel_core::mapping::{parse_pad_steps, parse_training_level, parse_training_levels};
    let steps = parse_pad_steps(s.get("pad_steps").unwrap_or(&Value::Null));
    s.insert("pad_steps".into(), json!(steps));
    let levels = parse_training_levels(s.get("training_levels").unwrap_or(&Value::Null));
    s.insert("training_levels".into(), json!(levels));
    let level = parse_training_level(s.get("training_level").unwrap_or(&Value::Null));
    s.insert("training_level".into(), json!(level));
    let one_of = |s: &mut Map<String, Value>, key: &str, allowed: &[&str], default: &str| {
        let ok = s
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|v| allowed.contains(&v));
        if !ok {
            s.insert(key.into(), json!(default));
        }
    };
    one_of(&mut s, "pad_idle", &["stop", "slow", "zero"], "stop");
    one_of(
        &mut s,
        "direction",
        &["keyboard", "gamepad", "treadmill", "training"],
        "keyboard",
    );
    one_of(&mut s, "output", &["dry", "keys"], "dry");
    one_of(&mut s, "theme", &["system", "light", "dark"], "system");
    if !s.get("toggle_pad").is_some_and(Value::is_string) {
        s.insert("toggle_pad".into(), json!(""));
    }
    for key in [
        "stand_below",
        "run_on",
        "run_off",
        "idle_speed",
        "walk_speed",
        "run_speed",
    ] {
        let v = s.get(key).and_then(Value::as_f64).unwrap_or(match key {
            "stand_below" => 0.8,
            "run_on" => 6.3,
            "run_off" => 5.7,
            "idle_speed" => 1.0,
            "walk_speed" => 1.5,
            _ => 2.5,
        });
        s.insert(key.into(), json!(v));
    }
    for key in [
        "quick_stop",
        "simulator",
        "record",
        "always_on_top",
        "floating_button",
        "large_text",
        "detailed_logs",
        "belt_control",
        "show_person_switch",
        "show_profile_switch",
        "achievement_popups",
        "achievement_sound",
    ] {
        if !s.get(key).is_some_and(Value::is_boolean) {
            let v = truthy(s.get(key));
            s.insert(key.into(), json!(v));
        }
    }
    s
}

// --- App ------------------------------------------------------------------------------------

impl App {
    pub fn b(&self, key: &str) -> bool {
        self.settings
            .get(key)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }
    pub fn f(&self, key: &str) -> f64 {
        self.settings
            .get(key)
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
    }
    pub fn s(&self, key: &str) -> String {
        self.settings
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    }

    pub fn mark(&mut self, slices: &[SliceName]) {
        self.dirty.extend(slices.iter().copied());
    }

    pub fn toast(&mut self, text: impl Into<String>) {
        let text = text.into();
        log::info!("Komunikat: {text}");
        self.out.push(Wyjscie::Toast(Toast {
            text,
            kind: ToastKind::Info,
        }));
    }

    pub fn toast_error(&mut self, text: impl Into<String>) {
        let text = text.into();
        log::info!("Komunikat: {text}");
        self.out.push(Wyjscie::Toast(Toast {
            text,
            kind: ToastKind::Error,
        }));
    }

    pub fn nav(
        &mut self,
        to: &'static str,
        reason: &'static str,
        account_mode: Option<&'static str>,
    ) {
        self.out.push(Wyjscie::Nav(NavRequest {
            to,
            reason,
            account_mode,
        }));
    }

    pub fn window(&mut self, w: Okno) {
        self.out.push(Wyjscie::Okno(w));
    }

    /// Ustawienia na dysk (przed konfiguratorem tylko w pamięci, plik 0.2 nietknięty).
    pub fn save(&mut self) {
        if let Some(store) = self.store.as_mut()
            && let Err(e) = store.update_flat(&self.settings)
        {
            log::warn!("Nie udało się zapisać ustawień: {e}");
        }
    }

    pub fn change(&mut self, key: &str, value: Value) {
        self.settings.insert(key.into(), value);
        self.save();
        self.mark(&[
            SliceName::Settings,
            SliceName::Control,
            SliceName::People,
            SliceName::Bubble,
        ]);
    }

    pub fn person_id(&self) -> String {
        self.store
            .as_ref()
            .map(|s| s.person().id.clone())
            .unwrap_or_default()
    }

    pub fn person_name(&self, person: &str) -> String {
        self.store
            .as_ref()
            .and_then(|s| s.people().iter().find(|p| p.id == person))
            .map_or_else(|| "?".into(), |p| p.name.clone())
    }

    pub fn profile_keys(&self) -> (Key, Key) {
        (
            Key::from_value(
                self.settings.get("walk_key").unwrap_or(&Value::Null),
                &Key::walk_default(),
            ),
            Key::from_value(
                self.settings.get("run_key").unwrap_or(&Value::Null),
                &Key::run_default(),
            ),
        )
    }

    /// Progi mapowania z ustawień (zły zestaw - zostaje poprzedni, jak w 0.5.x).
    pub fn apply_mapping(&mut self) {
        let mut m = section(&self.config, "mapping");
        for key in ["stand_below", "run_on", "run_off", "quick_stop"] {
            if let Some(v) = self.settings.get(key) {
                m.insert(key.into(), v.clone());
            }
        }
        match MappingConfig::from_value(&Value::Object(m)) {
            Ok(mapping) => {
                if mapping != *self.bridge.mapping() {
                    self.bridge.set_mapping(mapping);
                }
            }
            Err(e) => log::warn!("Progi: {e}"),
        }
    }

    pub fn max_speed(&self) -> f64 {
        self.bridge.mapping().max_speed
    }

    pub fn set_tracker_owner(&mut self) {
        let (Some(store), Some(history)) = (self.store.as_ref(), self.history.as_ref()) else {
            return;
        };
        let (person, profile) = (store.person(), store.profile());
        if let Err(e) = self.tracker.set_owner(
            history,
            &person.id,
            &person.name,
            &profile.id,
            &profile.name,
        ) {
            log::warn!("Historia kroków: {e}");
        }
    }

    pub fn set_tracker_source(&mut self) {
        let Some(history) = self.history.as_ref() else {
            return;
        };
        let simulated = self.b("simulator");
        let src = match &self.device {
            Some(d) => SessionSource {
                model: d.model.clone(),
                driver: d.driver.as_str().into(),
                profile: d.profile.clone(),
                firmware: d.firmware.clone(),
                app_version: walklevel_core::paths::app_version().into(),
                source: if d.simulated() {
                    "symulator".into()
                } else {
                    "bieznia".into()
                },
            },
            None => SessionSource {
                model: if simulated {
                    "Symulator bieżni".into()
                } else {
                    walklevel_core::treadmill::model_name(&self.registry, &self.s("device_name"))
                },
                driver: if simulated {
                    "symulator".into()
                } else {
                    String::new()
                },
                profile: String::new(),
                firmware: String::new(),
                app_version: walklevel_core::paths::app_version().into(),
                source: if simulated {
                    "symulator".into()
                } else {
                    "bieznia".into()
                },
            },
        };
        if let Err(e) = self.tracker.set_source(history, src) {
            log::warn!("Historia kroków: {e}");
        }
    }

    /// Po zmianie osoby albo profilu: ustawienia, klawisze, tryb, wygląd, historia.
    pub fn apply_profile(&mut self) {
        let Some(store) = self.store.as_ref() else {
            return;
        };
        let flat = normalize(store.flat(&self.defaults));
        let before_top = self.b("always_on_top");
        let before_bubble = self.b("floating_button");
        self.settings = flat;
        let (walk, run) = self.profile_keys();
        self.walk_key = walk;
        self.run_key = run;
        self.apply_keys_output();
        self.apply_mapping();
        if self.b("always_on_top") != before_top {
            self.window(Okno::MainAlwaysOnTop(self.b("always_on_top")));
        }
        if self.b("floating_button") != before_bubble {
            let pos = self.floating_pos();
            self.window(Okno::BubbleVisible(self.b("floating_button"), pos));
        }
        self.apply_direction();
        self.set_tracker_owner();
        let Some(store) = self.store.as_ref() else {
            return;
        };
        let (person, profile) = (store.person().name.clone(), store.profile().name.clone());
        log::info!("Osoba {person}, profil {profile} ({})", self.s("direction"));
        self.levels_key = None; // inna osoba - poziomy od nowa
        self.people_rev += 1;
        self.mark(&SliceName::ALL);
    }

    pub fn floating_pos(&self) -> Option<[f64; 2]> {
        let a = self.settings.get("floating_pos")?.as_array()?;
        Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?])
    }

    /// Wyjście mostu: klawiatura albo podgląd (z bieżącymi klawiszami chodu i biegu).
    pub fn apply_keys_output(&mut self) {
        let kind = self.output_kind();
        let out: Box<dyn walklevel_core::mapping::Output + Send> = match kind {
            OutputKind::Keys => Box::new(crate::petla::Klawiatura::new(
                self.walk_key.clone(),
                self.run_key.clone(),
            )),
            OutputKind::Dry => Box::new(DryOutput::default()),
        };
        if kind == self.bridge.output_kind() {
            self.bridge.replace_output(out);
        } else {
            self.bridge.set_output(kind, out);
        }
    }

    /// Klawisze/pad/trening -> bieżnia: aplikacja niczego nie naciska.
    pub fn output_kind(&self) -> OutputKind {
        let dir = walklevel_core::mapping::Direction::from_setting(&self.s("direction"));
        dir.bridge_output(OutputKind::from_setting(&self.s("output")))
    }

    pub fn apply_direction(&mut self) {
        let kind = self.output_kind();
        if kind != self.bridge.output_kind() {
            self.apply_keys_output();
        }
        self.belt_target = None;
        self.steering.forget_target();
        self.hint_override = None;
        self.mark(&[SliceName::Control, SliceName::Settings]);
    }

    pub fn belt_commands_possible(&self) -> bool {
        self.b("belt_control") && self.caps.control
    }

    // --- wycinki --------------------------------------------------------------------------

    pub fn slice_value(&mut self, s: SliceName) -> Value {
        let v = match s {
            SliceName::App => serde_json::to_value(self.app_info()),
            SliceName::Settings => serde_json::to_value(self.settings_view()),
            SliceName::People => serde_json::to_value(self.people_view()),
            SliceName::Device => serde_json::to_value(self.device_view()),
            SliceName::Control => serde_json::to_value(self.control_view()),
            SliceName::Levels => serde_json::to_value(self.levels_view()),
            SliceName::History => serde_json::to_value(self.history_stamp()),
            SliceName::Account => serde_json::to_value(self.account_view()),
            SliceName::Online => serde_json::to_value(&self.online),
            SliceName::Update => serde_json::to_value(&self.update),
            SliceName::Report => serde_json::to_value(&self.report),
            SliceName::Models => serde_json::to_value(&self.models),
            SliceName::Bubble => serde_json::to_value(self.bubble_view()),
        };
        v.unwrap_or(Value::Null)
    }

    pub fn app_info(&self) -> AppInfo {
        let latest = walklevel_core::backup::latest_backup(&self.paths.data_files())
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
        let version = walklevel_core::paths::app_version().to_string();
        AppInfo {
            title: format!("WalkLevel {} by damnit.com.pl", env!("CARGO_PKG_VERSION")),
            version,
            installed: walklevel_core::paths::is_installed(),
            data_dir: self.paths.data.to_string_lossy().into_owned(),
            first_run: self.store.is_none(),
            legacy02: self.legacy.is_some(),
            updated_from: self.updated_from.clone(),
            latest_backup: latest,
            accounts_enabled: self.accounts_enabled,
            report_endpoint: self
                .config
                .reports_url()
                .is_some_and(|u| !u.trim().is_empty()),
            window_focused: self.window_focused,
        }
    }

    pub fn key_binding(key: &Key) -> KeyBinding {
        KeyBinding {
            scan: key.scan,
            extended: key.extended,
            name: key.name.clone(),
            vk: key.vk,
            mods: key
                .mods
                .iter()
                .filter_map(|m| serde_json::to_value(m).ok()?.as_str().map(String::from))
                .collect(),
            label: key.label(),
        }
    }

    pub fn settings_view(&self) -> Settings {
        let steps: Vec<(u32, f64)> = walklevel_core::mapping::parse_pad_steps(
            self.settings.get("pad_steps").unwrap_or(&Value::Null),
        );
        let levels = walklevel_core::mapping::parse_training_levels(
            self.settings.get("training_levels").unwrap_or(&Value::Null),
        );
        let profile_color = self
            .store
            .as_ref()
            .map_or(palette::DEFAULT.to_string(), |s| s.profile().color.clone());
        let person_color = self
            .store
            .as_ref()
            .map_or(palette::DEFAULT.to_string(), |s| s.person().color.clone());
        Settings {
            device_address: self.s("device_address"),
            device_name: self.s("device_name"),
            belt_control: self.b("belt_control"),
            quick_stop: self.b("quick_stop"),
            toggle_key: Self::key_binding(&self.toggle_key),
            toggle_pad: self.s("toggle_pad"),
            simulator: self.b("simulator"),
            record: self.b("record"),
            detailed_logs: self.b("detailed_logs"),
            theme: self.s("theme"),
            large_text: self.b("large_text"),
            always_on_top: self.b("always_on_top"),
            floating_button: self.b("floating_button"),
            show_person_switch: self.b("show_person_switch"),
            show_profile_switch: self.b("show_profile_switch"),
            achievement_popups: self.b("achievement_popups"),
            achievement_sound: self.b("achievement_sound"),
            direction: self.s("direction"),
            output: self.s("output"),
            walk_key: Self::key_binding(&self.walk_key),
            run_key: Self::key_binding(&self.run_key),
            stand_below: self.f("stand_below"),
            run_on: self.f("run_on"),
            run_off: self.f("run_off"),
            idle_speed: self.f("idle_speed"),
            walk_speed: self.f("walk_speed"),
            run_speed: self.f("run_speed"),
            pad_steps: steps,
            pad_idle: self.s("pad_idle"),
            training_levels: levels,
            training_level: walklevel_core::mapping::parse_training_level(
                self.settings.get("training_level").unwrap_or(&Value::Null),
            ),
            profile_color,
            person_color,
        }
    }

    pub fn icon_ref(&self, profile: &store::Profile) -> IconRef {
        let url = profile
            .icon_file
            .as_deref()
            .filter(|f| !f.is_empty())
            .map(|file| {
                let path = self.paths.data_files().join(store::ICONS_DIR).join(file);
                let mtime = std::fs::metadata(&path)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs());
                format!("http://ikona.localhost/{file}?v={mtime}")
            });
        IconRef {
            color: profile.color.clone(),
            glyph: if url.is_some() {
                None
            } else {
                profile.glyph.clone()
            },
            url,
        }
    }

    fn profile_summary(&self, profile: &store::Profile) -> ProfileSummary {
        let mut flat = self.defaults.clone();
        for (k, v) in &profile.settings {
            flat.insert(k.clone(), v.clone());
        }
        let flat = normalize(flat);
        let steps =
            walklevel_core::mapping::parse_pad_steps(flat.get("pad_steps").unwrap_or(&Value::Null));
        let levels = walklevel_core::mapping::parse_training_levels(
            flat.get("training_levels").unwrap_or(&Value::Null),
        );
        let f = |k: &str| flat.get(k).and_then(Value::as_f64).unwrap_or(0.0);
        ProfileSummary {
            direction: flat
                .get("direction")
                .and_then(Value::as_str)
                .unwrap_or("keyboard")
                .into(),
            walk_speed: f("walk_speed"),
            run_speed: f("run_speed"),
            pad_step_count: steps.len(),
            pad_max_kmh: steps.iter().map(|s| s.1).fold(0.0, f64::max),
            training_min: levels.iter().copied().fold(f64::INFINITY, f64::min),
            training_max: levels.iter().copied().fold(0.0, f64::max),
            stand_below: f("stand_below"),
        }
    }

    pub fn today_since() -> f64 {
        walklevel_core::history::HistoryRange::Day
            .since(walklevel_core::history::now_unix())
            .unwrap_or(0.0)
    }

    pub fn people_view(&self) -> PeopleView {
        let Some(store) = self.store.as_ref() else {
            return PeopleView {
                ready: false,
                active_person_id: String::new(),
                active_profile_id: String::new(),
                ask_at_start: true,
                people: Vec::new(),
            };
        };
        let since = Self::today_since();
        let people = store
            .people()
            .iter()
            .map(|p| {
                let today = self
                    .history
                    .as_ref()
                    .and_then(|h| h.totals(since, Some(&p.id)).ok())
                    .map_or(0, |t| t.steps);
                let konto = self.konta.get(&p.id).filter(|k| k.state.logged_in());
                PersonView {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    color: p.color.clone(),
                    active_profile_id: p.active_profile_id().to_string(),
                    profiles: p
                        .profiles()
                        .iter()
                        .map(|g| GameProfileView {
                            id: g.id.clone(),
                            name: g.name.clone(),
                            color: g.color.clone(),
                            icon_color: g.icon_color.clone(),
                            icon: self.icon_ref(g),
                            exe: g.exe.clone(),
                            online: g.online,
                            summary: self.profile_summary(g),
                        })
                        .collect(),
                    today_steps: today,
                    has_account: konto.is_some(),
                    account_name: konto.map(|k| k.state.name.clone()),
                }
            })
            .collect();
        PeopleView {
            ready: true,
            active_person_id: store.person().id.clone(),
            active_profile_id: store.profile().id.clone(),
            ask_at_start: store.ask_at_start(),
            people,
        }
    }

    fn device_model_name(&self) -> String {
        match &self.device {
            Some(d) if !d.simulated() => d.model.clone(),
            _ => {
                let m =
                    walklevel_core::treadmill::model_name(&self.registry, &self.s("device_name"));
                if m.is_empty() { "bieżnia".into() } else { m }
            }
        }
    }

    pub fn link_status(&self) -> LinkStatus {
        let remembered = !self.s("device_address").is_empty();
        let model = self.device_model_name();
        let hk = section(&self.config, "hotkeys");
        let key = |k: &str, d: &str| hk.get(k).and_then(Value::as_str).unwrap_or(d).to_string();
        let slower = key("sim_slower", "F6");
        let faster = key("sim_faster", "F7");
        let min_kmh = teksty::kmh(self.caps.speed_min);
        let max_kmh = teksty::kmh(self.caps.speed_max);
        let vars: [(&str, &str); 5] = [
            ("model", &model),
            ("klawisz_wolniej", &slower),
            ("klawisz_szybciej", &faster),
            ("min_kmh", &min_kmh),
            ("max_kmh", &max_kmh),
        ];
        let st = |tone: &'static str, busy, state: &str, action: &'static str, primary| {
            let (title, sub, action_label) = teksty::status_texts(state, &vars);
            LinkStatus {
                tone,
                busy,
                title,
                sub,
                action,
                action_label,
                primary,
            }
        };
        if self.b("simulator") {
            return st("accent", false, "symulator", "simulatorOff", false);
        }
        match self.link {
            Link::Streaming if !self.bridge.state().live => {
                st("run", true, "czekam", "disconnect", false)
            }
            Link::Streaming => st("accent", false, "polaczono", "disconnect", false),
            // trwa raport diagnostyczny albo inny komunikat pracownika - tytuł z jego tekstu
            Link::Searching
                if self.link_text == walklevel_core::treadmill::worker::report_busy() =>
            {
                LinkStatus {
                    title: self.link_text.clone(),
                    ..st("accent", true, "szukanie", "cancel", false)
                }
            }
            Link::Searching if remembered => st("accent", true, "szukanie", "cancel", false),
            Link::Searching => st("accent", true, "skanowanie", "cancel", false),
            Link::Connecting | Link::Connected => st("accent", true, "laczenie", "cancel", false),
            // podtytuł: powód z pracownika (ble.powod.*)
            Link::Retrying => LinkStatus {
                sub: self.link_text.clone(),
                ..st("run", true, "ponawiam", "disconnect", false)
            },
            Link::BluetoothOff => st("danger", false, "bt_wylaczony", "retry", true),
            Link::Unsupported => st("danger", false, "nieobslugiwane", "scan", true),
            Link::Idle if remembered => st("muted", false, "rozlaczona", "retry", true),
            Link::Idle => st("muted", false, "brak", "scan", true),
        }
    }

    fn device_label(&self) -> String {
        let caps = &self.caps;
        let min_kmh = teksty::kmh(caps.speed_min);
        let max_kmh = teksty::kmh(caps.speed_max);
        let speeds = [("min_kmh", min_kmh.as_str()), ("max_kmh", max_kmh.as_str())];
        if self.b("simulator") {
            return teksty::t("ustawienia.bieznia.symulator_opis", &speeds);
        }
        let name = self.s("device_name");
        if self.s("device_address").is_empty() {
            return teksty::t("ustawienia.bieznia.brak", &[]);
        }
        let model = match &self.device {
            Some(d) => d.model.clone(),
            None => walklevel_core::treadmill::model_name(&self.registry, &name),
        };
        let model = if model.is_empty() { name } else { model };
        // druga linia: „Taśma 1,0–6,0 km/h · start ok. 3 s · oprogramowanie 1.2”, części tylko gdy są
        let full = teksty::t("ustawienia.bieznia.opis", &[]);
        let second = full.split_once('\n').map_or("", |(_, b)| b);
        let firmware = self
            .device
            .as_ref()
            .map(|d| d.firmware.clone())
            .unwrap_or_default();
        let seconds = format!("{:.0}", caps.start_delay_s);
        let parts: Vec<String> = second
            .split(" · ")
            .filter(|p| {
                (!p.contains("{sekundy}") || caps.start_delay_s >= 1.0)
                    && (!p.contains("{wersja_bt}") || !firmware.is_empty())
            })
            .map(|p| {
                p.replace("{min_kmh}", &min_kmh)
                    .replace("{max_kmh}", &max_kmh)
                    .replace("{sekundy}", &seconds)
                    .replace("{wersja_bt}", &firmware)
            })
            .collect();
        format!("{model}\n{}", parts.join(" · "))
    }

    pub fn device_view(&self) -> DeviceView {
        let treadmills = self.devices.iter().filter(|d| d.treadmill).count();
        let mut devices: Vec<crate::widoki::FoundDevice> = self
            .devices
            .iter()
            .filter(|d| d.treadmill)
            .chain(self.devices.iter().filter(|d| !d.treadmill).take(12))
            .map(|d| crate::widoki::FoundDevice {
                address: d.address.clone(),
                name: d.name.clone(),
                rssi: d.rssi,
                treadmill: d.treadmill,
                model: d.model.clone(),
            })
            .collect();
        devices.truncate(treadmills + 12);
        DeviceView {
            link: link_str(self.link),
            link_text: self.link_text.clone(),
            status: self.link_status(),
            device: self.device.as_ref().map(|d| DeviceInfoView {
                model: d.model.clone(),
                driver: d.driver.as_str().into(),
                profile: d.profile.clone(),
                name: d.name.clone(),
                firmware: d.firmware.clone(),
                simulated: d.simulated(),
            }),
            caps: self.caps,
            remembered: !self.s("device_address").is_empty(),
            label: self.device_label(),
            scanning: self.scanning,
            scan_text: self.scan_text.clone(),
            devices,
            bluetooth_off: self.link == Link::BluetoothOff,
            diagnostics: self.diagnostics.clone(),
        }
    }

    pub fn pad_button_label(&self) -> String {
        let button = self.s("toggle_pad");
        if button.is_empty() {
            return String::new();
        }
        let name = self
            .pad_state
            .as_ref()
            .map(|p| p.name.clone())
            .or_else(|| self.pad.name());
        walklevel_core::input::button_label(&button, name.as_deref())
    }

    pub fn control_view(&self) -> ControlView {
        let direction = self.s("direction");
        let hk = section(&self.config, "hotkeys");
        let key = |k: &str, d: &str| hk.get(k).and_then(Value::as_str).unwrap_or(d).to_string();
        let pad_button = self.pad_button_label();
        let hint = self.hint_override.clone().unwrap_or_else(|| {
            teksty::output_hint(
                &direction,
                &self.s("output"),
                &self.walk_key.name,
                &self.run_key.name,
                &key("sim_slower", "F6"),
                &key("sim_faster", "F7"),
            )
        });
        ControlView {
            enabled: self.bridge.state().enabled,
            sets_speed: walklevel_core::mapping::Direction::from_setting(&direction).sets_speed(),
            subtitle: teksty::control_subtitle(
                &direction,
                self.b("belt_control"),
                &pad_button,
                &self.walk_key.name,
                &key("sim_slower", "F6"),
                &key("sim_faster", "F7"),
            ),
            direction,
            hint,
            toggle_label: self.toggle_key.label(),
            walk_name: self.walk_key.name.clone(),
            run_name: self.run_key.name.clone(),
            pad_button_label: pad_button,
            pad_capturing: self.pad_capture_until > 0.0,
            key_capturing: self.key_capturing,
            belt_commands: self.belt_commands_possible(),
        }
    }

    pub fn levels_view(&self) -> Option<LevelsView> {
        let status = self.levels.as_ref()?;
        let earned = status.progress.iter().filter(|p| p.earned()).count();
        Some(LevelsView {
            person_id: self.person_id(),
            km: status.km,
            xp: status.xp,
            level: status.level,
            max_level: self.defs.max_level(),
            level_floor_xp: status.level_floor,
            level_ceiling_xp: status.level_ceiling,
            level_fraction: status.level_fraction,
            km_to_next_level: status.km_to_next_level,
            rank: (&status.rank).into(),
            next_rank: status.next_rank.as_ref().map(Into::into),
            earned_count: earned as u32,
            total: status.progress.len() as u32,
            progress: status
                .progress
                .iter()
                .map(|p| AchievementProgress {
                    id: p.achievement.id.clone(),
                    earned_at: p.earned_at,
                    current: p.current,
                    target: p.target,
                    fraction: p.fraction(),
                })
                .collect(),
        })
    }

    pub fn history_stamp(&self) -> HistoryStampView {
        let Some(h) = self.history.as_ref() else {
            return HistoryStampView {
                available: false,
                stamp: String::new(),
            };
        };
        let v = h.version().ok();
        let day = chrono::Local::now().format("%Y-%m-%d");
        HistoryStampView {
            available: true,
            stamp: format!(
                "{}-{}-{}-{day}-{}",
                v.map_or(0, |v| v.count),
                v.map_or(0, |v| v.steps),
                v.map_or(0.0, |v| v.last_end),
                self.people_rev
            ),
        }
    }

    pub fn account_view(&self) -> AccountView {
        let person = self.person_id();
        let k = self
            .konta
            .get(&person)
            .cloned()
            .unwrap_or_else(|| KontoWidok {
                state: AccountState::for_person(&person),
                pending: 0,
            });
        let s = &k.state;
        let prefix = format!("{person}:");
        AccountView {
            enabled: self.accounts_enabled,
            person_id: person.clone(),
            logged_in: s.logged_in(),
            user_id: s.user_id.clone(),
            name: s.name.clone(),
            email: s.email.clone(),
            verified: s.verified,
            upload: s.upload,
            visibility: s.visibility.clone(),
            live: s.live,
            profile_visibility: s.profile_visibility.clone(),
            games_visibility: s.games_visibility.clone(),
            terms: s.terms.clone(),
            terms_current: s.terms_current.clone(),
            terms_outdated: s.terms_outdated(),
            games_allowed: s.games_allowed(),
            remember: s.remember,
            terms_url: self
                .config
                .terms_url()
                .unwrap_or(walklevel_core::config::DEFAULT_TERMS_URL)
                .to_string(),
            busy: self
                .account_busy
                .iter()
                .filter_map(|b| b.strip_prefix(&prefix))
                .filter(|a| App::visible_busy(a))
                .map(String::from)
                .collect(),
            message: self.account_message.clone(),
            verify_polling: self.verify_until > 0.0,
            pending_uploads: k.pending,
        }
    }

    pub fn bubble_view(&self) -> BubbleView {
        BubbleView {
            visible: self.b("floating_button"),
            enabled: self.bridge.state().enabled,
            theme: self.s("theme"),
            accent: self
                .store
                .as_ref()
                .map_or(palette::DEFAULT.to_string(), |s| s.profile().color.clone()),
        }
    }

    pub fn statics(&self) -> Value {
        let hk = section(&self.config, "hotkeys");
        let key = |k: &str, d: &str| hk.get(k).and_then(Value::as_str).unwrap_or(d).to_string();
        let palette: Vec<Value> = palette::COLORS
            .iter()
            .map(|c| json!({"name": c.name, "hue": c.hue, "dark": c.dark, "light": c.light}))
            .collect();
        let glyphs: Vec<Value> = [
            ("Pad", '\u{e7fc}'),
            ("Tarcza", '\u{ea18}'),
            ("Ogień", '\u{ecad}'),
            ("Gwiazda", '\u{e734}'),
            ("Auto", '\u{e804}'),
            ("Samolot", '\u{e709}'),
            ("Świat", '\u{e774}'),
            ("Flaga", '\u{e7c1}'),
            ("Robot", '\u{e99a}'),
            ("Klocek", '\u{ea86}'),
            ("Iskry", '\u{f4a5}'),
            ("Serce", '\u{eb51}'),
        ]
        .iter()
        .map(|(n, c)| json!({"name": n, "code": c.to_string()}))
        .collect();
        let mut presets = Map::new();
        for n in 2..=5 {
            presets.insert(n.to_string(), json!(walklevel_core::mapping::pad_preset(n)));
        }
        let periods: Vec<Value> = walklevel_core::reports::PERIODS
            .iter()
            .map(|(s, _)| {
                let key = match s {
                    3600 => "zgloszenie.okres.1h",
                    21600 => "zgloszenie.okres.6h",
                    86400 => "zgloszenie.okres.24h",
                    _ => "zgloszenie.okres.7d",
                };
                json!({"seconds": s, "label": teksty::t(key, &[])})
            })
            .collect();
        let defs = self.defs;
        let streak_km = defs
            .achievements
            .iter()
            .find(|a| a.kind == "seria")
            .and_then(|a| a.param("km_dziennie"))
            .unwrap_or(2.0);
        let achievements: Vec<Value> = defs
            .achievements
            .iter()
            .map(|a| {
                json!({
                    "id": a.id, "kind": a.kind, "rarity": a.rarity, "mark": a.mark, "name": a.name,
                    "flavor": a.flavor, "condition": a.condition,
                    "km": if a.kind == "dystans" { a.km() } else { None },
                    "params": a.params,
                })
            })
            .collect();
        use walklevel_core::account as acc;
        json!({
            "appName": "WalkLevel",
            "author": "damnit.com.pl Damian Skrzypiec",
            "homepage": "https://damnit.com.pl",
            "contact": walklevel_core::reports::CONTACT,
            "palette": palette,
            "defaultColor": palette::DEFAULT,
            "runAltColor": palette::run_alt().name,
            "glyphs": glyphs,
            "defaultGlyph": store::DEFAULT_GLYPH,
            "modeNames": {"keyboard": "Klawisze", "gamepad": "Pad", "treadmill": "Chodzenie", "training": "Trening"},
            "maxSpeed": self.max_speed(),
            "hysteresis": 0.6,
            "thresholdGap": 0.8,
            "hotkeys": {"simSlower": key("sim_slower", "F6"), "simFaster": key("sim_faster", "F7")},
            "padPresets": presets,
            "padMinGap": walklevel_core::input::pad::MIN_GAP,
            "trainingRange": [0.5, 20.0],
            "padSpeedRange": [0.5, 12.0],
            "reportPeriods": periods,
            "reportDefaultS": walklevel_core::reports::RECENT_S,
            "reportMinChars": walklevel_core::reports::REPORT_MIN_CHARS,
            "account": {
                "nameMin": acc::NAME_MIN, "nameMax": acc::NAME_MAX, "nameRe": acc::NAME_RE,
                "reservedNames": acc::RESERVED_NAMES, "emailRe": acc::EMAIL_RE,
                "passwordMin": acc::PASSWORD_MIN, "codeMin": acc::CODE_MIN,
                "gameNameMax": acc::GAME_NAME_MAX,
            },
            "definitions": {
                "version": defs.version, "xpPerKm": defs.xp_per_km, "rarityXp": defs.rarity_xp,
                "rarities": defs.rarities, "levelsXp": defs.levels, "ranks": defs.ranks.iter().map(RankView::from).collect::<Vec<_>>(),
                "achievements": achievements,
                "rules": {"restDaysPer7": 1, "minKmPerStreakDay": streak_km},
            },
            "driverNames": {"kingsmith": "KingSmith", "ftms": "Bluetooth FTMS"},
        })
    }
}

pub fn link_str(l: Link) -> &'static str {
    match l {
        Link::Idle => "idle",
        Link::Searching => "searching",
        Link::Connecting => "connecting",
        Link::Connected => "connected",
        Link::Streaming => "streaming",
        Link::Retrying => "retrying",
        Link::BluetoothOff => "bluetoothOff",
        Link::Unsupported => "unsupported",
    }
}
