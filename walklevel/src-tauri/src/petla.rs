//! Pętla sterowania co 50 ms (TICK_MS z 0.5.x) w osobnym wątku: zdarzenia bieżni, skróty
//! (włączanie, szybciej, wolniej - domyślnie F8 / F7 / F6), pad, most (mapowanie -> klawisze w grze), tryby sterujące taśmą, sesja
//! w historii, poziomy, popupy, konto, telemetria do okien.

use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use walklevel_core::input::{Binding, Hotkeys, Key, KeyboardOutput, SystemKeys};
use walklevel_core::mapping::{
    self, BeltCommand, Control, ControlSource, Direction, FrameInput, Held, KeyIdle, PadIdle,
    Reading, SteeringSettings,
};
use walklevel_core::treadmill::{self, Link, WorkerEvent};

use crate::stan::{App, Core, LOG_TAIL, lock};
use crate::teksty;
use crate::widoki::{GaugeMode, PadTelemetry, SliceName, Telemetry};

pub const TICK: Duration = Duration::from_millis(50);
/// Czekanie na przycisk pada przy przypisywaniu (s).
pub const PAD_CAPTURE_S: f64 = 10.0;

/// Klawiatura (SendInput) jako wyjście mostu.
pub struct Klawiatura(KeyboardOutput);

impl Klawiatura {
    pub fn new(walk: Key, run: Key) -> Self {
        Self(KeyboardOutput::new(walk, run))
    }
}

impl mapping::Output for Klawiatura {
    fn apply(&mut self, control: &Control) {
        self.0.set_keys(control.walk, control.run);
    }
    fn release_all(&mut self) {
        self.0.release();
    }
}

/// Nagrywanie odczytów do `nagrania/YYYYmmdd-HHMMSS.jsonl` (ustawienie „Nagrywaj”).
pub struct Nagrywanie {
    file: std::fs::File,
    start: f64,
    pub path: std::path::PathBuf,
}

impl Nagrywanie {
    pub fn open(folder: &Path, now: f64) -> std::io::Result<Self> {
        std::fs::create_dir_all(folder)?;
        let path = folder.join(format!(
            "{}.jsonl",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        ));
        let file = std::fs::File::create(&path)?;
        Ok(Self {
            file,
            start: now,
            path,
        })
    }

    fn write(&mut self, r: &Reading) {
        let line = json!({"t": ((r.t - self.start) * 1000.0).round() / 1000.0, "speed": r.speed_kmh, "steps": r.steps});
        let _ = writeln!(self.file, "{line}");
    }
}

fn held_mode(h: Held) -> GaugeMode {
    match h {
        Held::Idle => GaugeMode::Idle,
        Held::Walk => GaugeMode::Walk,
        Held::Run => GaugeMode::Run,
    }
}

/// Tryb Chodzenie: kolor wskaźnika, strefa i klucz podpisu (glowny.wskaznik.chodzenie.*).
fn treadmill_caption(run: bool, walk: bool, dry: bool) -> (GaugeMode, i32, &'static str) {
    match (run, walk, dry) {
        (true, _, true) => (GaugeMode::Run, 2, "glowny.wskaznik.chodzenie.proba_bieg"),
        (true, _, false) => (GaugeMode::Run, 2, "glowny.wskaznik.chodzenie.bieg"),
        (false, true, true) => (GaugeMode::Walk, 1, "glowny.wskaznik.chodzenie.proba"),
        (false, true, false) => (GaugeMode::Walk, 1, "glowny.wskaznik.chodzenie.chod"),
        (false, false, _) => (GaugeMode::Idle, 0, "glowny.wskaznik.chodzenie.stoisz"),
    }
}

/// Przycisk na ekranie: tyle sekund stanu „jedzie”/„stoi”, zanim zmieni się okno (Stop pod kółkiem).
pub const BUBBLE_MOVING_HOLD_S: f64 = 1.0;

impl App {
    pub fn steering_settings(&self) -> SteeringSettings {
        SteeringSettings {
            direction: Direction::from_setting(&self.s("direction")),
            belt_control: self.b("belt_control"),
            idle_speed: self.f("idle_speed"),
            walk_speed: self.f("walk_speed"),
            run_speed: self.f("run_speed"),
            key_hold_s: mapping::parse_key_hold(
                self.settings.get("key_hold_s").unwrap_or(&Value::Null),
            ),
            key_idle: KeyIdle::from_setting(&self.s("key_idle")),
            pad_steps: mapping::parse_pad_steps(
                self.settings.get("pad_steps").unwrap_or(&Value::Null),
            ),
            pad_idle: PadIdle::from_setting(&self.s("pad_idle")),
            training_levels: mapping::parse_training_levels(
                self.settings.get("training_levels").unwrap_or(&Value::Null),
            ),
            training_level: mapping::parse_training_level(
                self.settings.get("training_level").unwrap_or(&Value::Null),
            ),
        }
    }

    pub fn belt(&self, command: BeltCommand) {
        let w = self.bieznia.worker();
        match command {
            BeltCommand::Start => w.belt(treadmill::BeltCommand::Start),
            BeltCommand::Stop => w.belt(treadmill::BeltCommand::Stop),
            BeltCommand::SetSpeed {
                kmh,
                start_if_stopped,
            } => w.set_speed(kmh, start_if_stopped),
        }
    }

    pub fn make_hotkeys(&mut self) {
        let bindings = vec![
            ("toggle".to_string(), Binding::from_key(&self.toggle_key)),
            (
                "sim_slower".to_string(),
                Binding::from_key(&self.slower_key),
            ),
            (
                "sim_faster".to_string(),
                Binding::from_key(&self.faster_key),
            ),
        ];
        self.hotkeys = match Hotkeys::new(bindings, SystemKeys) {
            Ok(h) => Some(h),
            Err(e) => {
                log::warn!("Skróty: {e}");
                None
            }
        };
    }

    /// Jedno miejsce dla przełącznika, skrótu, pada i przycisku (sterowanie.py set_control).
    /// `source` trafia do dziennika przy każdej zmianie (zgłoszenia „samo się wyłącza”).
    pub fn set_control(&mut self, on: bool, source: &ControlSource) {
        let was = self.bridge.state().enabled;
        self.bridge.set_enabled(on, source);
        walklevel_core::store::settings::save_enabled(
            on,
            &self
                .paths
                .data_files()
                .join(walklevel_core::store::settings::STATE_FILE),
        );
        self.belt_target = None;
        let settings = self.steering_settings();
        for cmd in self
            .steering
            .set_control(&settings, was, on, self.caps.control)
        {
            if cmd == BeltCommand::Start {
                let delay = self.caps.start_delay_s;
                self.hint_override = Some(if delay >= 2.0 {
                    teksty::t(
                        "tasma.uruchamiam_odliczanie",
                        &[("sekundy", &format!("{delay:.0}"))],
                    )
                } else {
                    teksty::t("tasma.uruchamiam", &[])
                });
            }
            self.belt(cmd);
        }
        self.mark(&[SliceName::Control, SliceName::Bubble]);
    }

    pub fn set_training_level(&mut self, level: usize) {
        let level = level.min(3);
        if self.settings.get("training_level").and_then(Value::as_u64) != Some(level as u64) {
            self.change("training_level", json!(level));
            log::info!("Trening: poziom {}", level + 1);
        }
        self.steering.forget_target();
        if !self.bridge.state().enabled {
            self.set_control(true, &ControlSource::TrainingLevel(level + 1));
        }
        self.mark(&[SliceName::Settings]);
    }

    pub fn change_training_level(&mut self, step: i64) {
        let level = mapping::parse_training_level(
            self.settings.get("training_level").unwrap_or(&Value::Null),
        );
        if let Some(next) = mapping::step_training_level(level, step) {
            self.set_training_level(next);
        }
    }

    pub fn training_stop(&mut self) {
        if self.bridge.state().enabled {
            self.set_control(false, &ControlSource::TrainingStop);
        } else {
            self.belt(BeltCommand::Stop);
        }
    }

    /// Stop z przycisku na ekranie (poza Treningiem): taśma staje zawsze - włączone sterowanie
    /// najpierw się wyłącza (inaczej klawisz albo gałka zaraz by ją znów uruchomiły).
    pub fn belt_stop(&mut self) {
        if self.bridge.state().enabled {
            self.set_control(false, &ControlSource::BubbleStop);
        }
        self.belt_target = None;
        self.belt(BeltCommand::Stop);
    }

    /// Przycisk na ekranie: taśma jedzie (dane na żywo, > 0,05 km/h). Zmiana stanu dopiero, gdy
    /// trzyma się [`BUBBLE_MOVING_HOLD_S`] - okno nie zmienia rozmiaru przy chwilowym odczycie.
    fn track_bubble_moving(&mut self, now: f64, live: bool, kmh: f64) {
        let moving = live && kmh > 0.05;
        if moving == self.bubble_moving {
            self.bubble_moving_since = None;
            return;
        }
        let since = *self.bubble_moving_since.get_or_insert(now);
        if now - since >= BUBBLE_MOVING_HOLD_S {
            self.bubble_moving = moving;
            self.bubble_moving_since = None;
            self.mark(&[SliceName::Bubble]);
        }
    }

    pub fn apply_recording(&mut self) {
        self.recorder = None;
        if self.b("record") {
            match Nagrywanie::open(&self.paths.recordings(), crate::petla::now_s()) {
                Ok(r) => {
                    log::info!("Nagrywam do {}", r.path.display());
                    self.recorder = Some(r);
                }
                Err(e) => log::warn!("Nagrywanie: {e}"),
            }
        }
    }

    /// Połączenie wg ustawień: symulator, zapamiętana bieżnia albo skan (pierwszy start).
    pub fn connect_source(&mut self, first: bool) {
        let w = self.bieznia.worker();
        if self.b("simulator") {
            w.connect_simulator();
        } else if !self.s("device_address").is_empty() {
            w.connect(self.s("device_address"), self.s("device_name"));
        } else if first {
            self.start_scan();
        } else {
            w.disconnect();
        }
    }

    pub fn start_scan(&mut self) {
        self.devices.clear();
        self.scanning = true;
        self.scan_cancelled = false;
        self.scan_text = teksty::t("bieznia.lista.szukam", &[]);
        self.bieznia.worker().scan();
        self.mark(&[SliceName::Device]);
    }

    pub fn on_simulator(&mut self, on: bool) {
        self.change("simulator", json!(on));
        self.device = None;
        self.caps = treadmill::known_capabilities(&self.registry, &self.s("device_name"), on);
        self.bridge.reset_source();
        self.set_tracker_source();
        self.connect_source(false);
        log::info!("Symulator {}", if on { "włączony" } else { "wyłączony" });
        self.mark(&[SliceName::Device, SliceName::Control]);
    }

    fn handle_worker_event(&mut self, ev: WorkerEvent) {
        match ev {
            WorkerEvent::Link { link, text } => {
                self.link = link;
                self.link_text = text;
                if link != Link::Searching {
                    self.scanning = false;
                }
                self.mark(&[SliceName::Device]);
            }
            WorkerEvent::Devices(devices) => {
                self.scanning = false;
                let treadmills: Vec<_> = devices.iter().filter(|d| d.treadmill).cloned().collect();
                let others = devices.len() - treadmills.len();
                self.scan_text = if devices.is_empty() {
                    String::new()
                } else {
                    teksty::t(
                        "bieznia.lista.znalezione",
                        &[
                            ("liczba", &treadmills.len().to_string()),
                            ("razem", &others.to_string()),
                        ],
                    )
                };
                self.devices = devices;
                if std::mem::take(&mut self.scan_cancelled) {
                    self.auto_connect = false;
                } else if self.auto_connect && treadmills.len() == 1 {
                    self.auto_connect = false;
                    log::info!("Znaleziono jedną bieżnię - łączę automatycznie");
                    self.toast(teksty::t("bieznia.lista.automatycznie", &[]));
                    let d = &treadmills[0];
                    self.connect_device(d.address.clone(), d.name.clone());
                } else {
                    // lista bieżni otwiera się tylko z przycisku Szukaj bieżni (status_action),
                    // nigdy sama po skanie w tle (pierwszy start, zapomniana bieżnia)
                    self.auto_connect = false;
                }
                self.mark(&[SliceName::Device]);
            }
            WorkerEvent::ScanFailed { link, text } => {
                self.link = link;
                self.scanning = false;
                self.auto_connect = false;
                self.scan_text = text;
                self.mark(&[SliceName::Device]);
            }
            WorkerEvent::Device {
                info, caps, spec, ..
            } => {
                if info.simulated() != self.b("simulator") {
                    return; // spóźnione zdarzenie po przełączeniu symulatora
                }
                let extra = if caps.start_delay_s >= 1.0 {
                    format!(", rozruch ok. {:.0} s", caps.start_delay_s)
                } else {
                    String::new()
                };
                log::info!(
                    "Bieżnia: {}, {:.1}-{:.1} km/h{extra}",
                    info.model,
                    caps.speed_min,
                    caps.speed_max
                );
                self.device = Some(*info);
                self.caps = caps;
                self.tracker.set_spec(spec);
                self.set_tracker_source();
                self.mark(&[SliceName::Device, SliceName::Control]);
            }
            WorkerEvent::Frame(frame) => {
                if let Some(h) = self.history.as_ref()
                    && let Err(e) = self.tracker.feed(h, &frame)
                {
                    log::warn!("Historia kroków: {e}");
                }
                if let Some(speed) = frame.speed_kmh
                    && frame.is_data()
                {
                    let reading = Reading {
                        t: frame.t,
                        speed_kmh: speed,
                        steps: frame.steps.map(|s| s as i64),
                    };
                    if let Some(r) = self.recorder.as_mut() {
                        r.write(&reading);
                    }
                    self.pending_reading = Some(reading);
                }
            }
            WorkerEvent::Belt(outcome) => {
                log::info!("Taśma: {}", outcome.as_str());
                self.hint_override = Some(outcome.message());
                self.mark(&[SliceName::Control]);
            }
            WorkerEvent::Problem(text) => self.toast_error(text),
            WorkerEvent::Report(result) => {
                self.diagnostics.running = false;
                self.diagnostics.text = match result {
                    Ok(path) => {
                        log::info!("Raport diagnostyczny: {}", path.display());
                        walklevel_core::treadmill::worker::report_done()
                    }
                    Err(text) => text,
                };
                self.mark(&[SliceName::Device]);
            }
        }
    }

    pub fn connect_device(&mut self, address: String, name: String) {
        self.settings
            .insert("device_address".into(), json!(address));
        self.settings.insert("device_name".into(), json!(name));
        if self.b("simulator") {
            self.settings.insert("simulator".into(), json!(false));
        }
        self.device = None;
        self.caps = treadmill::known_capabilities(&self.registry, &name, false);
        self.set_tracker_source();
        self.save();
        self.bieznia.worker().connect(address, name.clone());
        log::info!("Wybrano bieżnię {name}");
        self.mark(&[SliceName::Device, SliceName::Settings, SliceName::Control]);
    }

    fn read_pad(&mut self, now: f64, capturing_keys: bool) {
        let wanted = self.s("direction") == "gamepad"
            || !self.s("toggle_pad").is_empty()
            || self.pad_capture_until > 0.0;
        if !wanted {
            self.pad_state = None;
            self.pad_buttons.clear();
            return;
        }
        self.pad_state = self.pad.read();
        let buttons = self
            .pad_state
            .as_ref()
            .map(|p| p.buttons.clone())
            .unwrap_or_default();
        let pressed: std::collections::BTreeSet<String> =
            buttons.difference(&self.pad_buttons).cloned().collect();
        self.pad_buttons = buttons;
        if self.pad_capture_until > 0.0 {
            if let Some(button) = walklevel_core::input::pad::pick_button(&pressed) {
                self.set_toggle_pad(button);
            } else if now > self.pad_capture_until {
                self.pad_capture_until = 0.0;
                self.mark(&[SliceName::Control]);
            }
            return;
        }
        let toggle = self.s("toggle_pad");
        if !toggle.is_empty() && pressed.contains(&toggle) && !capturing_keys {
            let on = !self.bridge.state().enabled;
            let source = ControlSource::PadButton(self.pad_button_label());
            self.set_control(on, &source);
        }
    }

    pub fn set_toggle_pad(&mut self, button: String) {
        self.pad_capture_until = 0.0;
        self.change("toggle_pad", json!(button));
        let label = self.pad_button_label();
        log::info!(
            "Przycisk pada włączający sterowanie: {}",
            if button.is_empty() { "brak" } else { &button }
        );
        if button == "south" || button == "east" {
            self.toast(teksty::t(
                "ustawienia.wlaczanie.pad_ostrzezenie",
                &[("przycisk_pada", &label)],
            ));
        }
        self.mark(&[SliceName::Control]);
    }

    /// Jedna klatka. Zwraca telemetrię do wysłania.
    pub fn tick(&mut self, now: f64) -> Telemetry {
        for ev in self.bieznia.events() {
            self.handle_worker_event(ev);
        }
        let capturing = self.key_capturing.is_some();
        self.read_pad(now, capturing);
        let actions = self.hotkeys.as_mut().map(|h| h.poll()).unwrap_or_default();
        let direction = self.s("direction");
        for (action, step) in [("sim_faster", 1), ("sim_slower", -1)] {
            if actions.iter().any(|a| a == action) && !capturing {
                if direction == "training" {
                    self.change_training_level(step);
                } else if self.b("simulator") {
                    self.bieznia.worker().sim_remote(step as i32);
                }
            }
        }
        if actions.iter().any(|a| a == "toggle") && !capturing {
            let on = !self.bridge.state().enabled;
            let source = ControlSource::Hotkey(self.toggle_key.label());
            self.set_control(on, &source);
        }
        let reading = self.pending_reading.take();
        let state = self.bridge.tick(now, false, reading).clone();
        self.track_bubble_moving(now, state.live, state.control.speed);

        let settings = self.steering_settings();
        let key_down = |k: &Key| {
            let vk = if k.vk > 0 {
                k.vk as u32
            } else {
                walklevel_core::input::system::vk_from_scan(k.scan, k.extended)
            };
            walklevel_core::input::system::is_key_down(vk)
        };
        let deflection = self.pad_state.as_ref().map_or(0.0, |p| p.deflection());
        let frame = self.steering.frame(
            &settings,
            &FrameInput {
                now,
                enabled: state.enabled,
                capturing,
                walk_down: direction == "keyboard" && key_down(&self.walk_key),
                run_down: direction == "keyboard" && key_down(&self.run_key),
                deflection,
                belt_kmh: state
                    .live
                    .then(|| state.history.back().map(|&(_, kmh)| kmh))
                    .flatten(),
                belt_range: (self.caps.speed_min, self.caps.speed_max),
            },
        );
        if let Some(cmd) = frame.command {
            if let BeltCommand::SetSpeed { kmh, .. } = cmd {
                self.belt_target = Some(kmh);
            }
            self.belt(cmd);
        }

        // podpis i kolor wskaźnika (app.py tick)
        let off = teksty::t(
            "glowny.wskaznik.wylaczone",
            &[("klawisz_wlaczania", &self.toggle_key.label())],
        );
        let c = state.control;
        let offline = !state.live;
        let target = frame.target;
        let (mut mode, active, caption) = if offline && direction == "treadmill" {
            let text = if self.link == Link::Streaming {
                "glowny.wskaznik.czekam"
            } else {
                "glowny.wskaznik.brak_danych"
            };
            (GaugeMode::Offline, -1, teksty::t(text, &[]))
        } else if !state.enabled {
            (GaugeMode::Idle, -1, off)
        } else {
            let kmh = teksty::kmh(target);
            let (w, r) = (self.walk_key.name.as_str(), self.run_key.name.as_str());
            let vars = [
                ("kmh", kmh.as_str()),
                ("klawisz_chodu", w),
                ("klawisz_biegu", r),
            ];
            match direction.as_str() {
                "gamepad" => {
                    let idle = frame.pad_step == 0 && settings.pad_idle != PadIdle::Slow;
                    let belt = if frame.pad_stopped || (idle && settings.pad_idle == PadIdle::Stop)
                    {
                        teksty::t("glowny.wskaznik.tasma_stoi", &[])
                    } else {
                        teksty::t("glowny.wskaznik.tasma_kmh", &vars)
                    };
                    let caption = match &self.pad_state {
                        None => teksty::t("glowny.wskaznik.pad.brak", &[("tasma", &belt)]),
                        Some(_) => teksty::t(
                            "glowny.wskaznik.pad.stan",
                            &[
                                ("procent", &format!("{}", (deflection * 100.0).round())),
                                ("stopien", &(frame.pad_step + 1).to_string()),
                                ("tasma", &belt),
                            ],
                        ),
                    };
                    (held_mode(frame.held), frame.pad_step as i32, caption)
                }
                "training" => (
                    held_mode(frame.held),
                    settings.training_level as i32,
                    teksty::t(
                        "glowny.wskaznik.trening",
                        &[
                            ("poziom", &(settings.training_level + 1).to_string()),
                            ("kmh", &kmh),
                        ],
                    ),
                ),
                "keyboard" => {
                    let (zone, key) = match frame.held {
                        Held::Run => (2, "glowny.wskaznik.klawisze.bieg"),
                        Held::Walk => (1, "glowny.wskaznik.klawisze.chod"),
                        Held::Idle if frame.pad_stopped => (0, "glowny.wskaznik.klawisze.stoi"),
                        Held::Idle => (0, "glowny.wskaznik.klawisze.bez"),
                    };
                    (held_mode(frame.held), zone, teksty::t(key, &vars))
                }
                _ => {
                    let (m, zone, key) =
                        treadmill_caption(c.run, c.walk, self.s("output") == "dry");
                    (m, zone, teksty::t(key, &vars))
                }
            }
        };
        if offline {
            mode = GaugeMode::Offline;
        }

        self.frame += 1;
        if self.frame.is_multiple_of(5) {
            if let Some(h) = self.history.as_ref() {
                let session_mode = Direction::from_setting(&direction).session_mode();
                if let Err(e) = self.tracker.set_mode(h, session_mode) {
                    log::warn!("Historia kroków: {e}");
                }
            }
            // karta statusu: świeżość danych zmienia kolor i podpis - tylko przy zmianie
            let status = serde_json::to_string(&self.link_status()).unwrap_or_default();
            if status != self.status_key {
                self.status_key = status;
                self.mark(&[SliceName::Device]);
            }
        }
        if self.frame.is_multiple_of(20)
            && let Some(h) = self.history.as_ref()
        {
            let before = self.tracker.current().is_some();
            if let Err(e) = self.tracker.tick(h) {
                log::warn!("Historia kroków: {e}");
            }
            if before != self.tracker.current().is_some() {
                self.mark(&[SliceName::History, SliceName::People]);
            }
        }
        if self.frame % 40 == 1 {
            self.levels_tick();
        }
        let (changed, added) = self.popups.tick(crate::petla::now_s());
        if changed {
            let view = self.popups.view();
            if added {
                self.window(crate::okna::Okno::PopupShow);
            }
            if view.items.is_empty() {
                self.window(crate::okna::Okno::PopupHide);
            }
            self.out.push(crate::stan::Wyjscie::Popups(view));
        }

        let live = self.tracker.live(now);
        let sets_speed = Direction::from_setting(&direction).sets_speed();
        Telemetry {
            t_ms: (now * 1000.0) as u64,
            speed_kmh: c.speed,
            live: state.live,
            enabled: state.enabled,
            mode,
            active,
            caption,
            steps: live.steps,
            cadence: live.cadence,
            distance_km: live.distance_km,
            active_s: live.active_s,
            belt_target_kmh: if sets_speed { self.belt_target } else { None },
            pad: (direction == "gamepad").then(|| PadTelemetry {
                name: self.pad_state.as_ref().map(|p| p.name.clone()),
                deflection,
                step: frame.pad_step as u32,
                status_text: teksty::pad_status(
                    self.pad_state.as_ref().map(|p| p.name.as_str()),
                    deflection,
                    frame.pad_step,
                ),
            }),
        }
    }
}

/// Zegar ścienny w sekundach od startu (czasy popupów).
pub fn now_s() -> f64 {
    walklevel_core::input::system::monotonic_s()
}

/// Wątek pętli: działa zawsze, także przy zminimalizowanym oknie.
pub fn spawn(core: Core) {
    let _ = std::thread::Builder::new()
        .name("petla".into())
        .spawn(move || {
            let mut frame: u64 = 0;
            let mut next = Instant::now();
            loop {
                if core.0.quitting.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
                let now = core.now();
                let telemetry = lock(&core.0.app).tick(now);
                frame += 1;
                let visible = core
                    .0
                    .main_visible
                    .load(std::sync::atomic::Ordering::Relaxed);
                if visible || frame.is_multiple_of(5) {
                    core.0.sink.telemetry(&telemetry);
                }
                // linie Dziennika
                let lines = std::mem::take(&mut *lock(&core.0.log_new));
                if !lines.is_empty() {
                    let mut tail = lock(&core.0.log_tail);
                    for line in &lines {
                        if tail.len() >= LOG_TAIL {
                            tail.pop_front();
                        }
                        tail.push_back(line.clone());
                    }
                    drop(tail);
                    for line in &lines {
                        core.0.sink.log_line(line);
                    }
                }
                if frame.is_multiple_of(5) {
                    crate::konto::tick(&core);
                }
                core.flush();
                next += TICK;
                let now_i = Instant::now();
                if next > now_i {
                    std::thread::sleep(next - now_i);
                } else {
                    next = now_i;
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn treadmill_caption_trial_mode_says_game_gets_nothing() {
        let vars = [("klawisz_chodu", "W"), ("klawisz_biegu", "Shift")];
        let text = |run, walk, dry| teksty::t(treadmill_caption(run, walk, dry).2, &vars);
        assert_eq!(
            text(false, true, true),
            "Chód · tryb próbny, gra nic nie dostaje"
        );
        assert_eq!(
            text(true, true, true),
            "Bieg · tryb próbny, gra nic nie dostaje"
        );
        assert_eq!(text(false, true, false), "Chód · gra dostaje W");
        assert_eq!(text(true, true, false), "Bieg · gra dostaje Shift + W");
        assert_eq!(text(false, false, true), "Stoisz");
    }
}
