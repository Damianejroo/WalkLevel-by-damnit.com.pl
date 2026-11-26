//! Pad: wychylenie lewej gałki zamienione na skoki prędkości taśmy - port bieznia/pad.py.
//!
//! Odczyt bez fokusu (gra może być na wierzchu): DualSense i DualShock 4 wprost przez HID
//! (kabel i Bluetooth), XInput (pady Xbox i to, co Steam/DS4Windows udaje jako pad Xbox),
//! winmm (pozostałe pady). Program tylko czyta gałkę i przyciski, niczego nie wysyła do gry.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Gałka w spoczynku potrafi stać na kilku procentach...
pub const INNER_DEADZONE: f64 = 0.08;
/// ...a do końca rzadko dochodzi równe 100%.
pub const OUTER_DEADZONE: f64 = 0.95;
/// Bez pada szukamy go co tyle sekund, nie co klatkę.
pub const RESCAN_S: f64 = 2.0;

pub const STEPS_MIN: usize = 2;
pub const STEPS_MAX: usize = 5;
/// Skoki muszą różnić się co najmniej o tyle punktów procentowych.
pub const MIN_GAP: i64 = 5;

/// Skok: (wychylenie w %, prędkość km/h). Pierwszy zawsze od 0% = gałka puszczona.
pub type PadStep = (i64, f64);

/// Gotowe skoki dla 2-5 skoków (inna liczba - 4 skoki).
pub fn preset(count: usize) -> Vec<PadStep> {
    match count {
        2 => vec![(0, 1.0), (100, 2.5)],
        3 => vec![(0, 1.0), (50, 2.0), (100, 3.0)],
        5 => vec![(0, 1.0), (25, 1.5), (50, 2.5), (75, 3.5), (100, 4.5)],
        _ => vec![(0, 1.0), (25, 1.5), (50, 2.5), (100, 4.0)],
    }
}

/// Podpisy przycisków pada Sony pod wspólnymi nazwami (układ dół/prawo/lewo/góra).
const BUTTONS_SONY: &[(&str, &str)] = &[
    ("south", "Krzyżyk"),
    ("east", "Kółko"),
    ("west", "Kwadrat"),
    ("north", "Trójkąt"),
    ("l1", "L1"),
    ("r1", "R1"),
    ("l2", "L2"),
    ("r2", "R2"),
    ("select", "Create"),
    ("start", "Options"),
    ("l3", "L3 (wciśnięcie lewej gałki)"),
    ("r3", "R3 (wciśnięcie prawej gałki)"),
    ("home", "PS"),
    ("touchpad", "Panel dotykowy"),
    ("up", "Strzałka w górę"),
    ("down", "Strzałka w dół"),
    ("left", "Strzałka w lewo"),
    ("right", "Strzałka w prawo"),
];

/// Różnice podpisów na padzie Xbox (reszta jak u Sony).
const BUTTONS_XBOX: &[(&str, &str)] = &[
    ("south", "A"),
    ("east", "B"),
    ("west", "X"),
    ("north", "Y"),
    ("l1", "LB"),
    ("r1", "RB"),
    ("l2", "LT"),
    ("r2", "RT"),
    ("select", "View"),
    ("start", "Menu"),
    ("l3", "LS (wciśnięcie lewej gałki)"),
    ("r3", "RS (wciśnięcie prawej gałki)"),
    ("home", "Xbox"),
];

/// Kolejność przy przypisywaniu przycisku (gdy naraz kilka): najpierw rzadko używane w grach.
pub fn button_order() -> Vec<String> {
    let mut order: Vec<String> = ["touchpad", "select", "home", "l3", "r3", "start"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    order.extend((1..=32).map(|i| format!("b{i}")));
    let rest = [
        "up", "down", "left", "right", "l1", "r1", "l2", "r2", "west", "north", "east", "south",
    ];
    order.extend(rest.iter().map(|s| s.to_string()));
    order
}

/// Przycisk do przypisania spośród właśnie wciśniętych (według [`button_order`]).
pub fn pick_button(pressed: &BTreeSet<String>) -> Option<String> {
    button_order()
        .into_iter()
        .find(|b| pressed.contains(b))
        .or_else(|| pressed.iter().next().cloned())
}

/// Podpis przycisku jak na padzie (Krzyżyk na DualSense, A na padzie Xbox). Bez nazwy pada
/// - podpisy Sony.
pub fn button_label(button: &str, pad_name: Option<&str>) -> String {
    if let Some(num) = button.strip_prefix('b')
        && !num.is_empty()
        && num.chars().all(|c| c.is_ascii_digit())
    {
        return crate::teksty::t("pad.przycisk_nr", &[("liczba", num)]);
    }
    let sony = pad_name.is_none_or(|n| {
        ["DualSense", "DualShock", "PlayStation"]
            .iter()
            .any(|w| n.contains(w))
    });
    let find =
        |table: &[(&str, &'static str)]| table.iter().find(|(k, _)| *k == button).map(|(_, v)| *v);
    let label = if sony {
        find(BUTTONS_SONY)
    } else {
        find(BUTTONS_XBOX).or_else(|| find(BUTTONS_SONY))
    };
    label.map_or_else(|| button.to_string(), str::to_string)
}

/// Stan pada w jednej chwili.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PadState {
    pub name: String,
    /// -1..1, w prawo dodatnie
    pub x: f64,
    /// -1..1, do przodu dodatnie
    pub y: f64,
    /// wciśnięte teraz (nazwy jak "south", "touchpad", "b5")
    pub buttons: BTreeSet<String>,
}

impl PadState {
    pub fn new(name: &str, x: f64, y: f64) -> PadState {
        PadState {
            name: name.to_string(),
            x,
            y,
            buttons: BTreeSet::new(),
        }
    }

    /// Wychylenie 0..1 w dowolną stronę - postać idzie także w bok i do tyłu.
    pub fn deflection(&self) -> f64 {
        let r = self.x.hypot(self.y);
        if r <= INNER_DEADZONE {
            return 0.0;
        }
        ((r - INNER_DEADZONE) / (OUTER_DEADZONE - INNER_DEADZONE)).min(1.0)
    }
}

// --- skoki ---------------------------------------------------------------------

/// Skoki z ustawień (`pad_steps`, lista par [wychylenie %, km/h]) sprawdzone jak w Pythonie;
/// błędne - gotowe 4 skoki.
pub fn parse_steps(data: &Value) -> Vec<PadStep> {
    try_parse_steps(data).unwrap_or_else(|| preset(4))
}

fn try_parse_steps(data: &Value) -> Option<Vec<PadStep>> {
    let items: Vec<Value> = match data {
        Value::Array(a) => a.clone(),
        Value::Object(o) => o.keys().map(|k| Value::String(k.clone())).collect(),
        Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
        _ => return None,
    };
    let mut steps = Vec::with_capacity(items.len());
    for item in &items {
        let (at, speed) = match item {
            Value::Array(pair) if pair.len() == 2 => (pair[0].clone(), pair[1].clone()),
            Value::String(s) if s.chars().count() == 2 => {
                let mut c = s.chars().map(|c| Value::String(c.to_string()));
                (c.next()?, c.next()?)
            }
            Value::Object(o) if o.len() == 2 => {
                let mut k = o.keys().map(|k| Value::String(k.clone()));
                (k.next()?, k.next()?)
            }
            _ => return None,
        };
        steps.push((py_int(&at)?, round1(py_float(&speed)?)));
    }
    if !(STEPS_MIN..=STEPS_MAX).contains(&steps.len()) || steps[0].0 != 0 {
        return None;
    }
    for pair in steps.windows(2) {
        let (a, b) = (pair[0].0, pair[1].0);
        // saturating: w Pythonie liczby nie mają zakresu (np. -1e30 po int() nie przepełnia)
        if b.saturating_sub(a) < MIN_GAP || b > 100 {
            return None;
        }
    }
    if steps
        .iter()
        .any(|&(_, speed)| !(0.5..=12.0).contains(&speed))
    {
        return None;
    }
    Some(steps)
}

/// `int(value)` z Pythona (bez napisów z ułamkiem).
fn py_int(v: &Value) -> Option<i64> {
    match v {
        Value::Bool(b) => Some(i64::from(*b)),
        Value::Number(n) => n.as_i64().or_else(|| {
            n.as_f64()
                .filter(|f| f.is_finite())
                .map(|f| f.trunc() as i64)
        }),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

/// `float(value)` z Pythona.
fn py_float(v: &Value) -> Option<f64> {
    match v {
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

/// `round(x, 1)` z Pythona: poprawne zaokrąglenie dokładnej wartości (remis do parzystej).
fn round1(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    format!("{x:.1}").parse().unwrap_or(x)
}

/// Który skok teraz obowiązuje. Przy cofaniu gałki mały zapas, żeby skok nie migał.
#[derive(Debug, Clone, Default)]
pub struct StepPicker {
    pub index: usize,
}

impl StepPicker {
    pub const HYSTERESIS: f64 = 0.04;

    pub fn new() -> StepPicker {
        StepPicker::default()
    }

    /// Numer obowiązującego skoku (od 0) dla wychylenia 0..1.
    pub fn update(&mut self, steps: &[PadStep], deflection: f64) -> usize {
        if steps.is_empty() {
            self.index = 0;
            return 0;
        }
        self.index = self.index.min(steps.len() - 1);
        let highest = |margin: f64| {
            steps
                .iter()
                .enumerate()
                .filter(|(_, (at, _))| deflection >= *at as f64 / 100.0 - margin)
                .map(|(i, _)| i)
                .max()
                .unwrap_or(0)
        };
        let up = highest(0.0);
        if up >= self.index {
            self.index = up;
        } else {
            self.index = self.index.min(highest(Self::HYSTERESIS));
        }
        self.index
    }
}

// --- raporty pada ------------------------------------------------------------------

/// Lewa gałka z raportu DualSense/DualShock 4 (USB i oba tryby Bluetooth).
pub fn parse_sony(report: &[u8]) -> Option<(f64, f64)> {
    let (lx, ly) = match (report.first()?, report.len()) {
        (0x01, n) if n >= 3 => (report[1], report[2]), // USB albo Bluetooth w trybie podstawowym
        (0x31, n) if n >= 4 => (report[2], report[3]), // DualSense, Bluetooth, pełny raport
        (0x11, n) if n >= 5 => (report[3], report[4]), // DualShock 4, Bluetooth, pełny raport
        _ => return None,
    };
    let x = ((f64::from(lx) - 127.5) / 127.5).clamp(-1.0, 1.0);
    let y = ((127.5 - f64::from(ly)) / 127.5).clamp(-1.0, 1.0); // w raporcie oś Y rośnie w dół
    Some((x, y))
}

const HAT: [&[&str]; 8] = [
    &["up"],
    &["up", "right"],
    &["right"],
    &["down", "right"],
    &["down"],
    &["down", "left"],
    &["left"],
    &["up", "left"],
];
const SONY_B0: [(u8, &str); 4] = [
    (0x10, "west"),
    (0x20, "south"),
    (0x40, "east"),
    (0x80, "north"),
];
const SONY_B1: [(u8, &str); 8] = [
    (0x01, "l1"),
    (0x02, "r1"),
    (0x04, "l2"),
    (0x08, "r2"),
    (0x10, "select"),
    (0x20, "start"),
    (0x40, "l3"),
    (0x80, "r3"),
];
const SONY_B2: [(u8, &str); 2] = [(0x01, "home"), (0x02, "touchpad")];

/// Przyciski z raportu DualSense/DualShock 4. DualSense USB i Bluetooth (pełny raport) mają
/// przyciski za spustami, DualShock 4 i podstawowy raport Bluetooth - zaraz za gałkami.
pub fn parse_sony_buttons(report: &[u8], dualsense: bool) -> BTreeSet<String> {
    let mut pressed = BTreeSet::new();
    let start = match report.first() {
        Some(0x01) if dualsense && report.len() > 20 => 8,
        Some(0x01) => 5,
        Some(0x31) => 9,
        Some(0x11) => 8,
        _ => return pressed,
    };
    if report.len() < start + 3 {
        return pressed;
    }
    let (b0, b1, b2) = (report[start], report[start + 1], report[start + 2]);
    if let Some(dirs) = HAT.get(usize::from(b0 & 0x0F)) {
        pressed.extend(dirs.iter().map(|d| d.to_string()));
    }
    let bits = |byte: u8, table: &[(u8, &str)]| -> Vec<String> {
        table
            .iter()
            .filter(|(bit, _)| byte & bit != 0)
            .map(|(_, n)| n.to_string())
            .collect()
    };
    pressed.extend(bits(b0, &SONY_B0));
    pressed.extend(bits(b1, &SONY_B1));
    pressed.extend(bits(b2, &SONY_B2));
    pressed
}

const XINPUT_BITS: [(u16, &str); 14] = [
    (0x0001, "up"),
    (0x0002, "down"),
    (0x0004, "left"),
    (0x0008, "right"),
    (0x0010, "start"),
    (0x0020, "select"),
    (0x0040, "l3"),
    (0x0080, "r3"),
    (0x0100, "l1"),
    (0x0200, "r1"),
    (0x1000, "south"),
    (0x2000, "east"),
    (0x4000, "west"),
    (0x8000, "north"),
];

/// Przyciski pada XInput (`wButtons`, spusty wciśnięte powyżej połowy).
pub fn xinput_buttons(buttons: u16, left_trigger: u8, right_trigger: u8) -> BTreeSet<String> {
    let mut pressed: BTreeSet<String> = XINPUT_BITS
        .iter()
        .filter(|(bit, _)| buttons & bit != 0)
        .map(|(_, n)| n.to_string())
        .collect();
    if left_trigger > 128 {
        pressed.insert("l2".into());
    }
    if right_trigger > 128 {
        pressed.insert("r2".into());
    }
    pressed
}

/// Oś joysticka winmm (lo..hi) na -1..1.
pub fn axis(value: u32, lo: u32, hi: u32) -> f64 {
    if hi <= lo {
        return 0.0;
    }
    ((f64::from(value) - f64::from(lo)) / (f64::from(hi) - f64::from(lo)) * 2.0 - 1.0)
        .clamp(-1.0, 1.0)
}

// --- czytanie pada -------------------------------------------------------------------

/// Źródło stanu jednego pada (XInput, winmm, HID; w testach - udawany pad).
pub trait PadSource: Send {
    fn name(&self) -> &str;
    /// Stan teraz; None = pad odłączony albo nie odpowiada.
    fn read(&mut self) -> Option<PadState>;
    /// Zatrzymuje wątek odczytu (tylko HID), gdy źródło nie będzie używane.
    fn stop(&mut self) {}
}

/// Szukanie padów: zwraca podłączone źródła w kolejności pierwszeństwa.
pub type PadFinder = Arc<dyn Fn() -> Vec<Box<dyn PadSource>> + Send + Sync>;

/// Podłączone pady: Sony przez HID, potem XInput, potem pozostałe przez winmm. Poza Windows
/// - pusta lista.
pub fn discover() -> Vec<Box<dyn PadSource>> {
    #[cfg(windows)]
    {
        super::pad_win::discover()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

type Found = Arc<Mutex<Option<Vec<Box<dyn PadSource>>>>>;

/// Pierwszy podłączony pad. Bez pada sprawdza co [`RESCAN_S`], nie co klatkę.
///
/// `background = true`: szukanie (10-40 ms: SetupAPI, winmm) idzie w osobnym wątku, żeby nie
/// przycinać okna; wynik odbiera następne [`PadReader::read`].
pub struct PadReader {
    find: PadFinder,
    clock: Box<dyn Fn() -> f64 + Send>,
    background: bool,
    source: Option<Box<dyn PadSource>>,
    next_scan: f64,
    found: Found,
    scanning: Arc<AtomicBool>,
}

impl PadReader {
    /// Prawdziwe pady, szukanie w tle (jak w oknie aplikacji).
    pub fn system() -> PadReader {
        PadReader::new(
            Arc::new(discover),
            Box::new(super::system::monotonic_s),
            true,
        )
    }

    /// Czytnik z własnym szukaniem i zegarem (sekundy, monotoniczny) - do testów.
    pub fn new(find: PadFinder, clock: Box<dyn Fn() -> f64 + Send>, background: bool) -> PadReader {
        PadReader {
            find,
            clock,
            background,
            source: None,
            next_scan: 0.0,
            found: Arc::new(Mutex::new(None)),
            scanning: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Nazwa używanego pada albo None.
    pub fn name(&self) -> Option<String> {
        self.source.as_ref().map(|s| s.name().to_string())
    }

    /// Stan pada teraz (wywoływane co klatkę). None - brak pada.
    pub fn read(&mut self) -> Option<PadState> {
        if let Some(source) = self.source.as_mut() {
            if let Some(state) = source.read() {
                return Some(state);
            }
            log::info!("Pad odłączony: {}", source.name());
            self.source = None;
        }
        let found = self.found.lock().ok().and_then(|mut f| f.take());
        if let Some(found) = found {
            return self.choose(found);
        }
        let now = (self.clock)();
        if now < self.next_scan || self.scanning.load(Ordering::SeqCst) {
            return None;
        }
        self.next_scan = now + RESCAN_S;
        if self.background {
            self.scanning.store(true, Ordering::SeqCst);
            let (find, found, scanning) =
                (self.find.clone(), self.found.clone(), self.scanning.clone());
            let spawned = std::thread::Builder::new()
                .name("pad-szukanie".into())
                .spawn(move || {
                    // Jak w Pythonie: błąd szukania = pusta lista, a `scanning` zawsze wraca na
                    // false (inaczej po jednym błędzie pad nie byłby już nigdy szukany).
                    let list = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| find()))
                        .unwrap_or_else(|_| {
                            log::error!("Szukanie pada nie powiodło się");
                            Vec::new()
                        });
                    if let Ok(mut slot) = found.lock() {
                        *slot = Some(list);
                    }
                    scanning.store(false, Ordering::SeqCst);
                });
            if let Err(e) = spawned {
                log::error!("Szukanie pada nie powiodło się: {e}");
                self.scanning.store(false, Ordering::SeqCst);
            }
            return None;
        }
        let list = (self.find)();
        self.choose(list)
    }

    fn choose(&mut self, found: Vec<Box<dyn PadSource>>) -> Option<PadState> {
        let mut chosen = None;
        for mut source in found {
            let state = if chosen.is_none() {
                source.read()
            } else {
                None
            };
            match state {
                Some(state) => {
                    log::info!("Pad: {}", source.name());
                    self.source = Some(source);
                    chosen = Some(state);
                }
                None => source.stop(), // nieużywany - nie trzymamy wątku
            }
        }
        chosen
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn deflection_has_dead_zones_and_works_in_any_direction() {
        assert_eq!(PadState::new("p", 0.05, 0.03).deflection(), 0.0);
        assert_eq!(PadState::new("p", 0.0, 0.96).deflection(), 1.0);
        assert_eq!(PadState::new("p", 0.0, -0.96).deflection(), 1.0);
        let half = PadState::new("p", 0.0, (INNER_DEADZONE + OUTER_DEADZONE) / 2.0).deflection();
        assert!((half - 0.5).abs() < 1e-9);
    }

    #[test]
    fn saved_steps_are_validated() {
        let j = |v: Value| parse_steps(&v);
        assert_eq!(
            j(serde_json::json!([
                [0, 1.0],
                [25, 1.5],
                [50, 2.5],
                [100, 4.0]
            ])),
            preset(4)
        );
        assert_eq!(
            j(serde_json::json!([[0, 1], [60, 3]])),
            vec![(0, 1.0), (60, 3.0)]
        );
        for bad in [
            serde_json::json!(null),
            serde_json::json!([[0, 1.0]]),
            serde_json::json!([[0, 1.0], [0, 1.0], [0, 1.0], [0, 1.0], [0, 1.0], [0, 1.0]]),
            serde_json::json!([[10, 1.0], [50, 2.0]]),
            serde_json::json!([[0, 1.0], [52, 2.0], [50, 3.0]]),
            serde_json::json!([[0, 1.0], [50, 20.0]]),
            serde_json::json!("zepsute"),
        ] {
            assert_eq!(parse_steps(&bad), preset(4), "{bad}");
        }
        for n in 2..=5 {
            let steps = preset(n);
            assert_eq!(parse_steps(&serde_json::to_value(&steps).unwrap()), steps);
        }
    }

    #[test]
    fn step_picker_follows_stick_with_hysteresis_on_the_way_back() {
        let steps = preset(4);
        let mut p = StepPicker::new();
        for (d, want) in [
            (0.0, 0),
            (0.24, 0),
            (0.30, 1),
            (0.55, 2),
            (1.0, 3),
            (0.98, 3),
            (0.90, 2),
            (0.48, 2),
            (0.40, 1),
            (0.0, 0),
        ] {
            assert_eq!(p.update(&steps, d), want, "{d}");
        }
        p.update(&steps, 1.0);
        assert_eq!(p.update(&preset(2), 1.0), 1);
    }

    struct FakeSource {
        name: String,
        state: Arc<Mutex<Option<PadState>>>,
    }

    impl PadSource for FakeSource {
        fn name(&self) -> &str {
            &self.name
        }
        fn read(&mut self) -> Option<PadState> {
            self.state.lock().unwrap().clone()
        }
    }

    type Shared = Arc<Mutex<Option<PadState>>>;

    fn fake(name: &str) -> (Box<dyn PadSource>, Shared) {
        let state = Arc::new(Mutex::new(Some(PadState::new(name, 0.0, 0.0))));
        (
            Box::new(FakeSource {
                name: name.into(),
                state: state.clone(),
            }),
            state,
        )
    }

    #[test]
    fn reader_finds_pad_rescans_rarely_and_survives_unplugging() {
        let clock = Arc::new(Mutex::new(0.0f64));
        let plugged: Arc<Mutex<Option<Shared>>> = Arc::new(Mutex::new(None));
        let scans = Arc::new(Mutex::new(Vec::new()));
        let (c, p, s) = (clock.clone(), plugged.clone(), scans.clone());
        let find: PadFinder = Arc::new(move || {
            s.lock().unwrap().push(*c.lock().unwrap());
            match p.lock().unwrap().clone() {
                Some(state) => vec![Box::new(FakeSource {
                    name: "DualSense".into(),
                    state,
                }) as Box<dyn PadSource>],
                None => Vec::new(),
            }
        });
        let c2 = clock.clone();
        let mut reader = PadReader::new(find, Box::new(move || *c2.lock().unwrap()), false);
        assert!(reader.read().is_none());
        *clock.lock().unwrap() = 1.0;
        assert!(reader.read().is_none());
        assert_eq!(*scans.lock().unwrap(), vec![0.0]);
        let (_, state) = fake("DualSense");
        *plugged.lock().unwrap() = Some(state.clone());
        *clock.lock().unwrap() = 2.0;
        assert!(reader.read().is_some());
        assert_eq!(reader.name().as_deref(), Some("DualSense"));
        *state.lock().unwrap() = None;
        *plugged.lock().unwrap() = None;
        assert!(reader.read().is_none());
        assert_eq!(reader.name(), None);
    }

    #[test]
    fn background_scan_does_not_block_and_delivers_pad() {
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let rx = Mutex::new(rx);
        let find: PadFinder = Arc::new(move || {
            let _ = rx.lock().unwrap().recv_timeout(Duration::from_secs(2));
            vec![fake("DualSense (Bluetooth)").0]
        });
        let mut reader = PadReader::new(find, Box::new(crate::input::system::monotonic_s), true);
        let t0 = Instant::now();
        assert!(reader.read().is_none());
        assert!(t0.elapsed() < Duration::from_millis(50));
        tx.send(()).unwrap();
        let mut state = None;
        for _ in 0..100 {
            state = reader.read();
            if state.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(state.is_some());
        assert_eq!(reader.name().as_deref(), Some("DualSense (Bluetooth)"));
    }

    #[test]
    fn failed_background_scan_is_retried_later() {
        let calls = Arc::new(Mutex::new(0));
        let c = calls.clone();
        let find: PadFinder = Arc::new(move || {
            let n = {
                let mut n = c.lock().unwrap();
                *n += 1;
                *n
            };
            if n == 1 {
                panic!("błąd szukania (test)");
            }
            vec![fake("DualSense (USB)").0]
        });
        let clock = Arc::new(Mutex::new(0.0f64));
        let c2 = clock.clone();
        let mut reader = PadReader::new(find, Box::new(move || *c2.lock().unwrap()), true);
        let wait = |reader: &mut PadReader| {
            for _ in 0..200 {
                if let Some(s) = reader.read() {
                    return Some(s);
                }
                if !reader.scanning.load(Ordering::SeqCst) && reader.found.lock().unwrap().is_none()
                {
                    return None;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            None
        };
        assert!(wait(&mut reader).is_none());
        assert!(!reader.scanning.load(Ordering::SeqCst));
        *clock.lock().unwrap() = RESCAN_S;
        let mut state = None;
        for _ in 0..200 {
            state = reader.read();
            if state.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(state.is_some());
        assert_eq!(*calls.lock().unwrap(), 2);
    }

    #[test]
    fn real_pad_discovery_does_not_crash() {
        for mut source in discover() {
            if let Some(state) = source.read() {
                assert!((-1.0..=1.0).contains(&state.x));
            }
            source.stop();
        }
    }

    #[test]
    fn sony_reports_usb_and_bluetooth() {
        let mut rest = vec![0x31, 0xd1, 0x81, 0x7d, 0x7e, 0x7f, 0, 0];
        rest.extend([0u8; 70]);
        let (x, y) = parse_sony(&rest).unwrap();
        assert!(x.abs() < 0.02 && y.abs() < 0.03);
        let mut fwd = vec![0x31, 0x00, 128, 0];
        fwd.extend([0u8; 74]);
        assert_eq!(parse_sony(&fwd).unwrap().1, 1.0);
        let mut usb = vec![0x01, 255, 128];
        usb.extend([0u8; 61]);
        assert_eq!(parse_sony(&usb).unwrap().0, 1.0);
        let mut ds4 = vec![0x11, 0xC0, 0x00, 0, 255];
        ds4.extend([0u8; 73]);
        assert_eq!(parse_sony(&ds4), Some((-1.0, -1.0)));
        assert_eq!(parse_sony(&[0x05, 1, 2, 3]), None);
        assert_eq!(parse_sony(&[]), None);
    }

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn sony_buttons_in_every_report_layout() {
        let mut usb = vec![0u8; 64];
        (usb[0], usb[8], usb[10]) = (0x01, 0x08 | 0x20, 0x02);
        assert_eq!(parse_sony_buttons(&usb, true), set(&["south", "touchpad"]));
        let mut bt = vec![0u8; 78];
        (bt[0], bt[9], bt[10]) = (0x31, 0x08, 0x10);
        assert_eq!(parse_sony_buttons(&bt, true), set(&["select"]));
        let mut ds4 = vec![0u8; 64];
        (ds4[0], ds4[5], ds4[6]) = (0x01, 0x80, 0x01);
        assert_eq!(parse_sony_buttons(&ds4, false), set(&["up", "north", "l1"]));
        let short = [0x01, 128, 128, 128, 128, 0x08 | 0x40, 0x00, 0x01, 0, 0];
        assert_eq!(parse_sony_buttons(&short, true), set(&["east", "home"]));
        assert!(parse_sony_buttons(&[], true).is_empty());
    }

    #[test]
    fn xinput_buttons_and_labels() {
        assert_eq!(
            xinput_buttons(0x1000 | 0x0020, 200, 10),
            set(&["south", "select", "l2"])
        );
        assert_eq!(button_label("south", Some("DualSense (USB)")), "Krzyżyk");
        assert_eq!(button_label("south", Some("Pad Xbox (XInput 1)")), "A");
        assert_eq!(button_label("touchpad", None), "Panel dotykowy");
        assert_eq!(button_label("b5", Some("Gamepad")), "Przycisk 5");
    }

    #[test]
    fn assigned_button_prefers_rarely_used_ones() {
        assert_eq!(
            pick_button(&set(&["south", "touchpad"])).as_deref(),
            Some("touchpad")
        );
        assert_eq!(pick_button(&set(&["b3", "l1"])).as_deref(), Some("b3"));
        assert_eq!(pick_button(&set(&["zzz", "aaa"])).as_deref(), Some("aaa"));
        assert_eq!(pick_button(&BTreeSet::new()), None);
    }
}
