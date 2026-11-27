//! Silnik mostu: odczyt -> mapowanie -> wyjście, jedno wywołanie [`Bridge::tick`] na klatkę.
//! Port bieznia/engine.py; źródło odczytów i nagrywanie są po stronie wywołującego.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use super::speed::{Control, MappingConfig, SpeedMapper};

/// Historia prędkości do wykresu: ostatnia minuta.
pub const HISTORY_S: f64 = 60.0;
/// Najwięcej punktów historii (20 klatek/s -> do 1200 punktów na minutę).
pub const HISTORY_LEN: usize = 1300;
/// Kadencja z ostatnich tylu odczytów licznika kroków.
const STEPS_WINDOW: usize = 20;

/// Skąd przyszło włączenie albo wyłączenie sterowania (dopisek w dzienniku).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlSource {
    /// klawisz włączania sterowania (etykieta klawisza, np. „F8”, „]”)
    Hotkey(String),
    /// przycisk pada działający jak klawisz włączania (nazwa przycisku)
    PadButton(String),
    /// przełącznik na ekranie głównym; `keyboard` - Enter albo spacja zamiast kliknięcia
    WindowSwitch { keyboard: bool },
    /// kliknięcie przycisku na ekranie (nad grą)
    Bubble,
    /// Stop pod przyciskiem na ekranie poza Treningiem
    BubbleStop,
    /// Stop w Treningu (ekran główny albo przycisk na ekranie)
    TrainingStop,
    /// wybór poziomu Treningu włącza sterowanie (numer poziomu od 1)
    TrainingLevel(usize),
    /// polecenie bez wskazanego miejsca (komenda `control_toggle`, testy)
    Command,
}

impl ControlSource {
    /// Dopisek do dziennika, np. „klawisz F8”, „okno: przełącznik”.
    pub fn label(&self) -> String {
        match self {
            Self::Hotkey(key) => format!("klawisz {key}"),
            Self::PadButton(button) => format!("pad: przycisk {button}"),
            Self::WindowSwitch { keyboard: false } => "okno: przełącznik".into(),
            Self::WindowSwitch { keyboard: true } => "okno: przełącznik, z klawiatury".into(),
            Self::Bubble => "przycisk na ekranie".into(),
            Self::BubbleStop => "przycisk na ekranie: Stop".into(),
            Self::TrainingStop => "Trening: Stop".into(),
            Self::TrainingLevel(level) => format!("Trening: poziom {level}"),
            Self::Command => "polecenie".into(),
        }
    }
}

/// Linia dziennika przy zmianie stanu sterowania - zawsze ze źródłem w nawiasie.
pub fn control_log_line(enabled: bool, source: &ControlSource) -> String {
    let label = source.label();
    let label = if label.trim().is_empty() {
        "nieznane źródło".to_string()
    } else {
        label
    };
    format!(
        "Sterowanie {} ({label})",
        if enabled { "WŁĄCZONE" } else { "WYŁĄCZONE" }
    )
}

/// Co robimy z wyliczonym sterowaniem. Realizacje: [`DryOutput`] i klawiatura (moduł input).
pub trait Output {
    /// Ustawia stan wyjścia zgodnie ze sterowaniem (wywoływane co klatkę).
    fn apply(&mut self, control: &Control);
    /// Puszcza wszystko (pauza, błąd, wyjście z programu).
    fn release_all(&mut self);
}

/// Rodzaj wyjścia z ustawień ("dry" - podgląd, "keys" - klawiatura).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputKind {
    #[default]
    Dry,
    Keys,
}

impl OutputKind {
    /// Wartość z ustawień; nieznana -> "dry" (jak `normalize` w 0.5.x).
    pub fn from_setting(value: &str) -> Self {
        if value == "keys" {
            Self::Keys
        } else {
            Self::Dry
        }
    }

    /// Wartość do zapisu w ustawieniach.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dry => "dry",
            Self::Keys => "keys",
        }
    }
}

/// Wyjście „na sucho”: niczego nie naciska, tylko zapisuje zmiany stanu do logu.
#[derive(Debug, Default)]
pub struct DryOutput {
    state: (bool, bool),
}

fn py_bool(b: bool) -> &'static str {
    if b { "True" } else { "False" }
}

impl Output for DryOutput {
    fn apply(&mut self, control: &Control) {
        let state = (control.walk, control.run);
        if state != self.state {
            log::info!(
                "[dry] W={} Shift={} ({:.1} km/h)",
                py_bool(state.0),
                py_bool(state.1),
                control.speed
            );
            self.state = state;
        }
    }

    fn release_all(&mut self) {
        self.apply(&Control::idle());
    }
}

/// Jeden odczyt z bieżni.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    /// czas odczytu (zegar monotoniczny, sekundy)
    pub t: f64,
    /// prędkość taśmy (km/h)
    pub speed_kmh: f64,
    /// licznik kroków bieżni, jeśli go podaje
    pub steps: Option<i64>,
}

/// Stan do pokazania w interfejsie.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// sterowanie włączone (klawisze trafiają do wyjścia)
    pub enabled: bool,
    pub output: OutputKind,
    /// sterowanie z ostatniego odczytu
    pub control: Control,
    /// ostatni licznik kroków bieżni
    pub steps: Option<i64>,
    /// kroki na minutę
    pub cadence: Option<f64>,
    /// czy dane z bieżni są świeże
    pub live: bool,
    pub distance_km: f64,
    /// czas ruchu (sekundy)
    pub active_s: f64,
    /// (czas, km/h) z ostatniej minuty
    pub history: VecDeque<(f64, f64)>,
}

impl Snapshot {
    fn new(enabled: bool, output: OutputKind) -> Self {
        Self {
            enabled,
            output,
            control: Control::idle(),
            steps: None,
            cadence: None,
            live: false,
            distance_km: 0.0,
            active_s: 0.0,
            history: VecDeque::new(),
        }
    }
}

/// Most: odczyty bieżni -> sterowanie -> wyjście, z bezpiecznikiem nieświeżych danych.
pub struct Bridge {
    mapping: MappingConfig,
    mapper: SpeedMapper,
    output: Box<dyn Output + Send>,
    state: Snapshot,
    last_reading: Option<Reading>,
    last_tick: Option<f64>,
    steps_window: VecDeque<(f64, i64)>,
}

impl Bridge {
    /// Nowy most. `output` musi odpowiadać `kind` (np. [`DryOutput`] dla [`OutputKind::Dry`]).
    pub fn new(
        mapping: MappingConfig,
        kind: OutputKind,
        output: Box<dyn Output + Send>,
        enabled: bool,
    ) -> Self {
        Self {
            mapping,
            mapper: SpeedMapper::new(mapping),
            output,
            state: Snapshot::new(enabled, kind),
            last_reading: None,
            last_tick: None,
            steps_window: VecDeque::with_capacity(STEPS_WINDOW),
        }
    }

    /// Stan do pokazania w interfejsie.
    pub fn state(&self) -> &Snapshot {
        &self.state
    }

    /// Bieżące progi.
    pub fn mapping(&self) -> &MappingConfig {
        &self.mapping
    }

    /// Bieżący rodzaj wyjścia.
    pub fn output_kind(&self) -> OutputKind {
        self.state.output
    }

    /// Inny rodzaj wyjścia - stare najpierw puszcza wszystko. Ten sam rodzaj: nic się nie
    /// zmienia, a `output` przepada (jak `set_output` w 0.5.x - bez puszczania klawiszy).
    pub fn set_output(&mut self, kind: OutputKind, output: Box<dyn Output + Send>) {
        if kind == self.state.output {
            return;
        }
        self.replace_output(output);
        self.state.output = kind;
        log::info!(
            "Wyjście: {}",
            if kind == OutputKind::Keys {
                "klawiatura"
            } else {
                "podgląd"
            }
        );
    }

    /// To samo wyjście z nowymi klawiszami chodu/biegu (`set_keys` w 0.5.x) - stare
    /// klawisze są najpierw puszczane.
    pub fn replace_output(&mut self, output: Box<dyn Output + Send>) {
        self.output.release_all();
        self.output = output;
    }

    /// Nowe progi - mapowanie od nowa (bez biegu i hamowania).
    pub fn set_mapping(&mut self, mapping: MappingConfig) {
        self.mapping = mapping;
        self.mapper = SpeedMapper::new(mapping);
    }

    /// Włącza albo wyłącza sterowanie; wyłączenie puszcza klawisze. Każda zmiana trafia do
    /// dziennika razem ze źródłem ([`control_log_line`]) - zgłoszenia „samo się wyłącza”
    /// rozstrzyga dopiero to, skąd przyszło polecenie. Zwraca, czy stan się zmienił.
    pub fn set_enabled(&mut self, enabled: bool, source: &ControlSource) -> bool {
        if enabled == self.state.enabled {
            log::debug!(
                "Sterowanie już {} ({})",
                if enabled { "włączone" } else { "wyłączone" },
                source.label()
            );
            return false;
        }
        self.state.enabled = enabled;
        log::info!("{}", control_log_line(enabled, source));
        if !enabled {
            self.output.release_all();
        }
        true
    }

    /// Przełącza sterowanie; zwraca nowy stan.
    pub fn toggle(&mut self, source: &ControlSource) -> bool {
        self.set_enabled(!self.state.enabled, source);
        self.state.enabled
    }

    /// Nowe (albo żadne) źródło odczytów: postać staje, dane przestają być świeże.
    pub fn reset_source(&mut self) {
        self.go_idle();
    }

    fn go_idle(&mut self) {
        self.mapper.reset();
        self.last_reading = None;
        self.state.control = Control::idle();
        self.state.live = false;
        self.state.cadence = None;
    }

    fn update_cadence(&mut self, reading: &Reading) {
        let Some(steps) = reading.steps else { return };
        if self.steps_window.len() >= STEPS_WINDOW {
            self.steps_window.pop_front();
        }
        self.steps_window.push_back((reading.t, steps));
        if let Some(&(first_t, first_steps)) = self.steps_window.front()
            && reading.t - first_t >= 3.0
            && steps >= first_steps
        {
            self.state.cadence = Some((steps - first_steps) as f64 / (reading.t - first_t) * 60.0);
        }
    }

    /// Jedna klatka z chwili `now`. `toggle` - wciśnięty skrót włączania; `reading` - nowy
    /// odczyt z bieżni od poprzedniej klatki (albo `None`).
    pub fn tick(&mut self, now: f64, toggle: bool, reading: Option<Reading>) -> &Snapshot {
        if toggle {
            self.toggle(&ControlSource::Command);
        }
        let dt = match self.last_tick {
            None => 0.0,
            Some(last) if now - last < 1.0 => now - last,
            Some(_) => 1.0,
        };
        self.last_tick = Some(now);

        if let Some(reading) = reading {
            self.last_reading = Some(reading);
            self.state.control = self.mapper.update(reading.speed_kmh, reading.t);
            self.state.live = true;
            self.state.steps = reading.steps;
            self.update_cadence(&reading);
        } else if let Some(last) = self.last_reading
            && now - last.t > self.mapping.stale_after_s
        {
            log::warn!(
                "Brak danych z bieżni od {:.1} s - zatrzymuję postać",
                now - last.t
            );
            self.go_idle();
        }

        let control = self.state.control;
        if control.walk {
            self.state.distance_km += control.speed * dt / 3600.0;
            self.state.active_s += dt;
        }
        let history = &mut self.state.history;
        if history.len() >= HISTORY_LEN {
            history.pop_front();
        }
        history.push_back((now, control.speed));
        while history.front().is_some_and(|&(t, _)| now - t > HISTORY_S) {
            history.pop_front();
        }

        if self.state.enabled {
            self.output.apply(&control);
        }
        &self.state
    }

    /// Koniec pracy: puszcza wszystko. Źródło zamyka wywołujący.
    pub fn shutdown(&mut self) {
        self.output.release_all();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Default)]
    struct Spy {
        applied: Vec<bool>,
        released: usize,
    }

    struct SpyOutput(Arc<Mutex<Spy>>);

    impl Output for SpyOutput {
        fn apply(&mut self, control: &Control) {
            if let Ok(mut s) = self.0.lock() {
                s.applied.push(control.walk);
            }
        }
        fn release_all(&mut self) {
            if let Ok(mut s) = self.0.lock() {
                s.released += 1;
            }
        }
    }

    fn make(enabled: bool) -> (Bridge, Arc<Mutex<Spy>>) {
        let spy = Arc::new(Mutex::new(Spy::default()));
        let output = Box::new(SpyOutput(spy.clone()));
        let bridge = Bridge::new(MappingConfig::default(), OutputKind::Dry, output, enabled);
        (bridge, spy)
    }

    fn reading(t: f64, speed_kmh: f64, steps: Option<i64>) -> Option<Reading> {
        Some(Reading {
            t,
            speed_kmh,
            steps,
        })
    }

    #[test]
    fn disabled_bridge_never_touches_output() {
        let (mut bridge, spy) = make(false);
        bridge.tick(0.0, false, reading(0.0, 4.0, None));
        assert!(spy.lock().expect("spy").applied.is_empty());
    }

    #[test]
    fn stale_data_stops_the_character() {
        let (mut bridge, spy) = make(true);
        assert!(
            bridge
                .tick(0.0, false, reading(0.0, 4.0, None))
                .control
                .walk
        );
        assert!(bridge.tick(1.0, false, None).control.walk); // jeszcze świeże
        let state = bridge.tick(4.0, false, None); // dłużej niż stale_after_s (3 s)
        assert!(!state.control.walk && !state.live);
        assert_eq!(spy.lock().expect("spy").applied.last(), Some(&false));
    }

    #[test]
    fn every_control_change_is_logged_with_its_source() {
        let sources = [
            ControlSource::Hotkey("]".into()),
            ControlSource::Hotkey(String::new()),
            ControlSource::PadButton("Panel dotykowy".into()),
            ControlSource::WindowSwitch { keyboard: false },
            ControlSource::WindowSwitch { keyboard: true },
            ControlSource::Bubble,
            ControlSource::BubbleStop,
            ControlSource::TrainingStop,
            ControlSource::TrainingLevel(2),
            ControlSource::Command,
        ];
        for source in &sources {
            for on in [true, false] {
                let line = control_log_line(on, source);
                let (head, tail) = line.split_once(" (").expect("źródło w nawiasie");
                assert_eq!(
                    head,
                    if on {
                        "Sterowanie WŁĄCZONE"
                    } else {
                        "Sterowanie WYŁĄCZONE"
                    }
                );
                assert!(tail.ends_with(')') && tail.len() > 3, "{line}");
            }
        }
        assert_eq!(
            control_log_line(false, &ControlSource::Hotkey("]".into())),
            "Sterowanie WYŁĄCZONE (klawisz ])"
        );
        assert_eq!(
            control_log_line(true, &ControlSource::TrainingLevel(3)),
            "Sterowanie WŁĄCZONE (Trening: poziom 3)"
        );
        let (mut bridge, _) = make(false);
        assert!(bridge.set_enabled(true, &ControlSource::Bubble));
        assert!(!bridge.set_enabled(true, &ControlSource::Bubble)); // bez zmiany - bez wpisu info
    }

    #[test]
    fn toggle_releases_keys() {
        let (mut bridge, spy) = make(true);
        bridge.tick(0.0, true, None);
        assert!(!bridge.state().enabled);
        assert_eq!(spy.lock().expect("spy").released, 1);
    }

    #[test]
    fn distance_time_and_cadence() {
        let (mut bridge, _) = make(true);
        for i in 0..41_i64 {
            // 10 s co 0,25 s przy 6 km/h i 2 krokach na sekundę
            let t = i as f64 * 0.25;
            bridge.tick(t, false, reading(t, 6.0, Some(i / 2)));
        }
        let state = bridge.state();
        assert!((state.distance_km - 6.0 * 10.0 / 3600.0).abs() < 0.001);
        assert!((state.active_s - 10.0).abs() < 0.01);
        assert!(state.cadence.is_some_and(|c| 100.0 < c && c < 140.0));
    }

    #[test]
    fn history_keeps_last_minute() {
        let (mut bridge, _) = make(true);
        for i in 0..200 {
            bridge.tick(i as f64 * 0.5, false, None);
        }
        let h = &bridge.state().history;
        let (first, last) = (
            h.front().expect("historia").0,
            h.back().expect("historia").0,
        );
        assert!(last - first <= 60.0);
    }

    #[test]
    fn output_change_releases_old_output() {
        let (mut bridge, spy) = make(true);
        bridge.set_output(OutputKind::Keys, Box::new(DryOutput::default()));
        assert_eq!(bridge.output_kind(), OutputKind::Keys);
        assert_eq!(spy.lock().expect("spy").released, 1);
        bridge.shutdown();
    }

    #[test]
    fn same_output_kind_keeps_keys_pressed() {
        let (mut bridge, spy) = make(true);
        bridge.tick(0.0, false, reading(0.0, 4.0, None));
        bridge.set_output(OutputKind::Dry, Box::new(DryOutput::default()));
        assert_eq!(spy.lock().expect("spy").released, 0); // nic nie puszczone
        bridge.tick(0.1, false, None);
        assert_eq!(spy.lock().expect("spy").applied.len(), 2); // nadal stare wyjście
        bridge.replace_output(Box::new(DryOutput::default())); // nowe klawisze
        assert_eq!(spy.lock().expect("spy").released, 1);
    }

    #[test]
    fn output_kind_from_setting() {
        assert_eq!(OutputKind::from_setting("keys"), OutputKind::Keys);
        assert_eq!(OutputKind::from_setting("pad"), OutputKind::Dry);
        assert_eq!(OutputKind::Keys.as_str(), "keys");
    }
}
