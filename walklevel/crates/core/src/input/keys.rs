//! Klawisze sterowania (chód, bieg, start/stop) - port bieznia/klawisze.py.
//!
//! Klawisz to fizyczny kod skanowania + flaga „rozszerzony” (strzałki, prawy Ctrl/Alt). Kod
//! nie zależy od układu klawiatury i widzą go też gry z DirectInput.
//!
//! Klawisz włączania sterowania może być kombinacją Ctrl / Alt / Shift + klawisz. Niedozwolone
//! są skróty Windows (z klawiszem Windows, F12), systemowe (Alt+Tab, Alt+F4...), nakładek gier
//! (Steam Shift+Tab, NVIDIA Alt+Z) oraz Ctrl+Alt+litera, czyli AltGr z polskimi znakami.

use serde::{Deserialize, Serialize};

use crate::py::truthy;
use crate::teksty::t;
use serde_json::{Map, Value};

/// Modyfikator kombinacji. W JSON małymi literami: "ctrl", "alt", "shift", "win".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mod {
    Ctrl,
    Alt,
    Shift,
    /// Tylko do odrzucenia - skróty z klawiszem Windows są zarezerwowane.
    Win,
}

impl Mod {
    /// Kolejność zapisu i wyświetlania: Ctrl, Alt, Shift, Win.
    pub const ORDER: [Mod; 4] = [Mod::Ctrl, Mod::Alt, Mod::Shift, Mod::Win];

    /// Nazwa do pokazania („Ctrl”).
    pub fn label(self) -> &'static str {
        match self {
            Mod::Ctrl => "Ctrl",
            Mod::Alt => "Alt",
            Mod::Shift => "Shift",
            Mod::Win => "Win",
        }
    }

    /// Kod wirtualny stanu lewego albo prawego modyfikatora (do GetAsyncKeyState).
    pub fn vk(self) -> u32 {
        match self {
            Mod::Ctrl => 0x11,
            Mod::Alt => 0x12,
            Mod::Shift => 0x10,
            Mod::Win => 0x5B,
        }
    }

    /// Modyfikator z nazwy zapisanej w JSON ("ctrl"); inne nazwy - None.
    pub fn from_name(name: &str) -> Option<Mod> {
        match name {
            "ctrl" => Some(Mod::Ctrl),
            "alt" => Some(Mod::Alt),
            "shift" => Some(Mod::Shift),
            "win" => Some(Mod::Win),
            _ => None,
        }
    }
}

/// Modyfikatory bez powtórzeń w stałej kolejności Ctrl, Alt, Shift, Win.
pub fn order_mods(mods: impl IntoIterator<Item = Mod>) -> Vec<Mod> {
    let chosen: Vec<Mod> = mods.into_iter().collect();
    Mod::ORDER
        .into_iter()
        .filter(|m| chosen.contains(m))
        .collect()
}

/// Modyfikatory z flag zdarzenia klawiatury (np. `KeyboardEvent` z okna: ctrlKey, altKey,
/// shiftKey, metaKey), w stałej kolejności.
pub fn mods_from_flags(ctrl: bool, alt: bool, shift: bool, win: bool) -> Vec<Mod> {
    let flags = [ctrl, alt, shift, win];
    Mod::ORDER
        .into_iter()
        .zip(flags)
        .filter(|(_, on)| *on)
        .map(|(m, _)| m)
        .collect()
}

/// Klawisz: kod skanowania, flaga rozszerzonego, nazwa do pokazania, kod wirtualny
/// (potrzebny skrótom globalnym; 0 = nieznany, zapis sprzed 0.3) i modyfikatory.
///
/// Serializuje się jak w Pythonie (`Key.to_dict`): `mods` tylko przy kombinacji.
/// Do odczytu zapisanych ustawień służy pobłażliwe [`Key::from_value`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Key {
    pub scan: u16,
    pub extended: bool,
    pub name: String,
    #[serde(default)]
    pub vk: i64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mods: Vec<Mod>,
}

impl Key {
    /// Klawisz bez modyfikatorów.
    pub fn new(scan: u16, extended: bool, name: &str, vk: i64) -> Key {
        Key {
            scan,
            extended,
            name: name.to_string(),
            vk,
            mods: Vec::new(),
        }
    }

    /// Ten sam klawisz z modyfikatorami (kolejność zostaje taka, jak podana).
    pub fn with_mods(mut self, mods: Vec<Mod>) -> Key {
        self.mods = mods;
        self
    }

    /// Domyślny klawisz chodu: W.
    pub fn walk_default() -> Key {
        Key::new(0x11, false, "W", 0x57)
    }

    /// Domyślny klawisz biegu: lewy Shift.
    pub fn run_default() -> Key {
        Key::new(0x2A, false, "Shift", 0xA0)
    }

    /// Domyślny klawisz włączania sterowania: F8.
    pub fn toggle_default() -> Key {
        Key::new(0x42, false, "F8", 0x77)
    }

    /// Ten sam fizyczny klawisz (bez względu na nazwę, kod wirtualny i modyfikatory).
    pub fn same_key(&self, other: &Key) -> bool {
        (self.scan, self.extended) == (other.scan, other.extended)
    }

    /// Nazwa do pokazania z modyfikatorami, np. „Ctrl+Shift+F8”.
    pub fn label(&self) -> String {
        let mut parts: Vec<&str> = self.mods.iter().map(|m| m.label()).collect();
        parts.push(&self.name);
        parts.join("+")
    }

    /// Zapis do JSON jak `Key.to_dict` w Pythonie.
    pub fn to_value(&self) -> Value {
        let mut map = Map::new();
        map.insert("scan".into(), Value::from(self.scan));
        map.insert("extended".into(), Value::from(self.extended));
        map.insert("name".into(), Value::from(self.name.clone()));
        map.insert("vk".into(), Value::from(self.vk));
        if !self.mods.is_empty() {
            let mods = self
                .mods
                .iter()
                .map(|m| Value::from(mod_name(*m)))
                .collect();
            map.insert("mods".into(), Value::Array(mods));
        }
        Value::Object(map)
    }

    /// Odczyt zapisanego klawisza 1:1 z `Key.from_dict`: błędny albo brakujący - `default`.
    /// Modyfikatory po odczycie zawsze w kolejności Ctrl, Alt, Shift (nieznane pomijane).
    pub fn from_value(data: &Value, default: &Key) -> Key {
        parse_key(data).unwrap_or_else(|| default.clone())
    }
}

fn mod_name(m: Mod) -> &'static str {
    match m {
        Mod::Ctrl => "ctrl",
        Mod::Alt => "alt",
        Mod::Shift => "shift",
        Mod::Win => "win",
    }
}

fn parse_key(data: &Value) -> Option<Key> {
    let map = data.as_object()?;
    let scan = py_int(map.get("scan")?)?;
    if !(0 < scan && scan < 0x100) {
        return None;
    }
    let vk = match map.get("vk") {
        Some(v) if truthy(v) => py_int(v)?,
        _ => 0,
    };
    let names: Vec<String> = match map.get("mods") {
        Some(v) if truthy(v) => match v {
            Value::Array(items) => items
                .iter()
                .filter_map(|i| i.as_str().map(String::from))
                .collect(),
            Value::String(s) => s.chars().map(String::from).collect(),
            Value::Object(o) => o.keys().cloned().collect(),
            _ => return None, // liczba albo true - w Pythonie TypeError
        },
        _ => Vec::new(),
    };
    let mods = order_mods(names.iter().filter_map(|n| Mod::from_name(n)));
    let name = match map.get("name") {
        Some(v) if truthy(v) => py_str(v),
        _ => String::new(),
    };
    let extended = map.get("extended").is_some_and(truthy);
    Some(Key {
        scan: u16::try_from(scan).ok()?,
        extended,
        name,
        vk,
        mods,
    })
}

/// `int(value)` z Pythona: liczba (ułamek obcięty), napis z liczbą całkowitą, bool.
fn py_int(v: &Value) -> Option<i64> {
    match v {
        Value::Bool(b) => Some(i64::from(*b)),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(i)
            } else if n.as_u64().is_some() {
                Some(i64::MAX)
            } else {
                n.as_f64()
                    .filter(|f| f.is_finite())
                    .map(|f| f.trunc() as i64)
            }
        }
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

/// `str(value)` z Pythona dla wartości z JSON.
fn py_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Null => "None".into(),
        Value::Number(n) if n.is_f64() => py_float_repr(n.as_f64().unwrap_or(0.0)),
        other => other.to_string(),
    }
}

/// `repr(float)` z Pythona: najkrótszy zapis, notacja wykładnicza poniżej 1e-4 i od 1e16
/// („1e+16”, „1.5e-07”), liczby całkowite z „.0”.
fn py_float_repr(f: f64) -> String {
    let abs = f.abs();
    if f != 0.0 && !(1e-4..1e16).contains(&abs) {
        let sci = format!("{f:e}"); // np. "1.5e-7"
        let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
        let exp: i32 = exp.parse().unwrap_or(0);
        let sign = if exp < 0 { '-' } else { '+' };
        return format!("{mantissa}e{sign}{:02}", exp.abs());
    }
    if f.fract() == 0.0 {
        format!("{f:.1}")
    } else {
        f.to_string()
    }
}

/// Polskie nazwy klawiszy, których tekst nie mówi sam za siebie: (scan, rozszerzony) -> nazwa.
const NAMES: &[((u16, bool), &str)] = &[
    ((0x48, true), "Strzałka w górę"),
    ((0x50, true), "Strzałka w dół"),
    ((0x4B, true), "Strzałka w lewo"),
    ((0x4D, true), "Strzałka w prawo"),
    ((0x2A, false), "Shift"),
    ((0x36, false), "Prawy Shift"),
    ((0x1D, false), "Ctrl"),
    ((0x1D, true), "Prawy Ctrl"),
    ((0x38, false), "Alt"),
    ((0x38, true), "Prawy Alt"),
    ((0x39, false), "Spacja"),
    ((0x0F, false), "Tab"),
    ((0x3A, false), "Caps Lock"),
    ((0x1C, false), "Enter"),
    ((0x0E, false), "Backspace"),
    ((0x52, true), "Insert"),
    ((0x53, true), "Delete"),
    ((0x47, true), "Home"),
    ((0x4F, true), "End"),
    ((0x49, true), "Page Up"),
    ((0x51, true), "Page Down"),
];

/// F6/F7: skróty symulatora, Esc: anulowanie wyboru, F12: debuger Windows, Print Screen
/// (wartości: klucze powodów w docs/teksty.md).
const RESERVED: &[((u16, bool), &str)] = &[
    ((0x40, false), "klawisz.powod.f6"),
    ((0x41, false), "klawisz.powod.f7"),
    ((0x01, false), "klawisz.powod.esc"),
    ((0x58, false), "klawisz.powod.f12"),
    ((0x37, true), "klawisz.powod.printscreen"),
];

/// Kombinacje zajęte przez Windows, gry i nakładki: (modyfikatory, scan) -> klucz powodu.
const TAKEN_COMBOS: &[(&[Mod], u16, &str)] = &[
    (&[Mod::Alt], 0x0F, "klawisz.powod.alt_tab"),
    (&[Mod::Alt], 0x3E, "klawisz.powod.alt_f4"),
    (&[Mod::Alt], 0x01, "klawisz.powod.alt_esc"),
    (&[Mod::Alt], 0x39, "klawisz.powod.alt_spacja"),
    (&[Mod::Alt], 0x1C, "klawisz.powod.alt_enter"),
    (&[Mod::Alt], 0x2C, "klawisz.powod.alt_z"),
    (&[Mod::Alt], 0x3B, "klawisz.powod.alt_f1"),
    (&[Mod::Alt], 0x43, "klawisz.powod.alt_f9"),
    (&[Mod::Alt], 0x44, "klawisz.powod.alt_f10"),
    (&[Mod::Shift], 0x0F, "klawisz.powod.shift_tab"),
    (&[Mod::Ctrl], 0x01, "klawisz.powod.ctrl_esc"),
    (
        &[Mod::Ctrl, Mod::Shift],
        0x01,
        "klawisz.powod.ctrl_shift_esc",
    ),
    (&[Mod::Ctrl, Mod::Alt], 0x53, "klawisz.powod.ctrl_alt_del"),
    (&[Mod::Ctrl], 0x2E, "klawisz.powod.ctrl_c"),
    (&[Mod::Ctrl], 0x2F, "klawisz.powod.ctrl_v"),
    (&[Mod::Ctrl], 0x2D, "klawisz.powod.ctrl_x"),
    (&[Mod::Ctrl], 0x2C, "klawisz.powod.ctrl_z"),
    (&[Mod::Ctrl], 0x15, "klawisz.powod.ctrl_y"),
    (&[Mod::Ctrl], 0x1E, "klawisz.powod.ctrl_a"),
    (&[Mod::Ctrl], 0x1F, "klawisz.powod.ctrl_s"),
];

/// Litery, z których AltGr (= Ctrl+Alt) robi polskie znaki: ą ć ę ł ń ó ś ź ż.
const ALTGR_LETTERS: [u16; 9] = [0x1E, 0x2E, 0x12, 0x26, 0x31, 0x18, 0x1F, 0x2D, 0x2C];

/// Same modyfikatory (Shift, Ctrl, Alt, Win) - nie mogą być klawiszem kombinacji.
const MODIFIER_SCANS: [(u16, bool); 8] = [
    (0x2A, false),
    (0x36, false),
    (0x1D, false),
    (0x1D, true),
    (0x38, false),
    (0x38, true),
    (0x5B, true),
    (0x5C, true),
];

/// Litery i cyfry (Shift + taki klawisz pisze w czacie zamiast działać jak skrót).
fn is_typing_key(scan: u16) -> bool {
    matches!(scan, 0x02..0x0C | 0x10..0x1A | 0x1E..0x27 | 0x2C..0x33)
}

fn lookup<'a>(table: &'a [((u16, bool), &'a str)], key: (u16, bool)) -> Option<&'a str> {
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// Polska nazwa klawisza z tabeli (strzałki, Shift, Spacja...), jeśli jest.
pub fn polish_name(scan: u16, extended: bool) -> Option<&'static str> {
    lookup(NAMES, (scan, extended))
}

/// Czy kombinacja nie koliduje z Windows, grami albo pisaniem. None = w porządku.
pub fn combo_problem(key: &Key) -> Option<String> {
    if key.mods.contains(&Mod::Win) {
        return Some(t("klawisz.blad.win", &[]));
    }
    if key.mods.is_empty() {
        return None;
    }
    if MODIFIER_SCANS.contains(&(key.scan, key.extended)) {
        return Some(t("klawisz.blad.pojedynczy", &[]));
    }
    let taken = TAKEN_COMBOS
        .iter()
        .find(|(mods, scan, _)| *mods == key.mods.as_slice() && *scan == key.scan);
    if let Some((_, _, reason)) = taken {
        return Some(t("klawisz.blad.zajety", &[("powod", &t(reason, &[]))]));
    }
    let only_ctrl_alt = order_mods(key.mods.iter().copied()) == [Mod::Ctrl, Mod::Alt];
    if only_ctrl_alt && ALTGR_LETTERS.contains(&key.scan) && !key.extended {
        return Some(t("klawisz.blad.altgr", &[("klawisz", &key.label())]));
    }
    if key.mods == [Mod::Shift] && is_typing_key(key.scan) && !key.extended {
        return Some(t("klawisz.blad.shift_litera", &[]));
    }
    None
}

/// Klawisz z kodu podanego przez Windows (bit 0x100 = klawisz rozszerzony, jak
/// `nativeScanCode` w Qt). Kod 0 - None.
pub fn key_from_native(native_scan: u32, fallback_name: &str, vk: i64) -> Option<Key> {
    let scan = (native_scan & 0xFF) as u16;
    if scan == 0 {
        return None;
    }
    let extended = native_scan & 0x100 != 0;
    let name = match polish_name(scan, extended) {
        Some(n) => n.to_string(),
        None if !fallback_name.trim().is_empty() => fallback_name.trim().to_string(),
        None => t("klawisz.kod", &[("liczba", &format!("{scan:#04x}"))]),
    };
    Some(Key::new(scan, extended, &name, vk))
}

/// Opis problemu z wybranymi klawiszami albo None, gdy wszystko w porządku.
pub fn problem(walk: &Key, run: &Key, toggle: &Key) -> Option<String> {
    for key in [walk, run, toggle] {
        if let Some(reason) = lookup(RESERVED, (key.scan, key.extended)) {
            return Some(t("klawisz.blad.ogolny", &[("powod", &t(reason, &[]))]));
        }
    }
    if walk.same_key(run) {
        return Some(t("klawisz.blad.te_same", &[]));
    }
    if toggle.same_key(walk) || toggle.same_key(run) {
        // aplikacja sama naciska klawisze chodu i biegu - skrót wyłączałby sam siebie
        return Some(t("klawisz.blad.wlaczanie_koliduje", &[]));
    }
    if toggle.vk == 0 {
        return Some(t("klawisz.blad.wlaczanie_niedozwolony", &[]));
    }
    if !walk.mods.is_empty() || !run.mods.is_empty() {
        return Some(t("klawisz.blad.chod_z_modyfikatorem", &[]));
    }
    combo_problem(toggle)
}

/// Fizyczne klawisze po `KeyboardEvent.code` z okna aplikacji: (code, scan, rozszerzony,
/// kod wirtualny, nazwa). Okno (WebView2) nie podaje kodów skanowania, tylko `code`, który
/// opisuje to samo położenie klawisza niezależnie od układu.
const CODES: &[(&str, u16, bool, i64, &str)] = &[
    ("Escape", 0x01, false, 0x1B, "Esc"),
    ("Digit1", 0x02, false, 0x31, "1"),
    ("Digit2", 0x03, false, 0x32, "2"),
    ("Digit3", 0x04, false, 0x33, "3"),
    ("Digit4", 0x05, false, 0x34, "4"),
    ("Digit5", 0x06, false, 0x35, "5"),
    ("Digit6", 0x07, false, 0x36, "6"),
    ("Digit7", 0x08, false, 0x37, "7"),
    ("Digit8", 0x09, false, 0x38, "8"),
    ("Digit9", 0x0A, false, 0x39, "9"),
    ("Digit0", 0x0B, false, 0x30, "0"),
    ("Minus", 0x0C, false, 0xBD, "-"),
    ("Equal", 0x0D, false, 0xBB, "="),
    ("Backspace", 0x0E, false, 0x08, "Backspace"),
    ("Tab", 0x0F, false, 0x09, "Tab"),
    ("KeyQ", 0x10, false, 0x51, "Q"),
    ("KeyW", 0x11, false, 0x57, "W"),
    ("KeyE", 0x12, false, 0x45, "E"),
    ("KeyR", 0x13, false, 0x52, "R"),
    ("KeyT", 0x14, false, 0x54, "T"),
    ("KeyY", 0x15, false, 0x59, "Y"),
    ("KeyU", 0x16, false, 0x55, "U"),
    ("KeyI", 0x17, false, 0x49, "I"),
    ("KeyO", 0x18, false, 0x4F, "O"),
    ("KeyP", 0x19, false, 0x50, "P"),
    ("BracketLeft", 0x1A, false, 0xDB, "["),
    ("BracketRight", 0x1B, false, 0xDD, "]"),
    ("Enter", 0x1C, false, 0x0D, "Enter"),
    ("ControlLeft", 0x1D, false, 0xA2, "Ctrl"),
    ("KeyA", 0x1E, false, 0x41, "A"),
    ("KeyS", 0x1F, false, 0x53, "S"),
    ("KeyD", 0x20, false, 0x44, "D"),
    ("KeyF", 0x21, false, 0x46, "F"),
    ("KeyG", 0x22, false, 0x47, "G"),
    ("KeyH", 0x23, false, 0x48, "H"),
    ("KeyJ", 0x24, false, 0x4A, "J"),
    ("KeyK", 0x25, false, 0x4B, "K"),
    ("KeyL", 0x26, false, 0x4C, "L"),
    ("Semicolon", 0x27, false, 0xBA, ";"),
    ("Quote", 0x28, false, 0xDE, "'"),
    ("Backquote", 0x29, false, 0xC0, "`"),
    ("ShiftLeft", 0x2A, false, 0xA0, "Shift"),
    ("Backslash", 0x2B, false, 0xDC, "\\"),
    ("KeyZ", 0x2C, false, 0x5A, "Z"),
    ("KeyX", 0x2D, false, 0x58, "X"),
    ("KeyC", 0x2E, false, 0x43, "C"),
    ("KeyV", 0x2F, false, 0x56, "V"),
    ("KeyB", 0x30, false, 0x42, "B"),
    ("KeyN", 0x31, false, 0x4E, "N"),
    ("KeyM", 0x32, false, 0x4D, "M"),
    ("Comma", 0x33, false, 0xBC, ","),
    ("Period", 0x34, false, 0xBE, "."),
    ("Slash", 0x35, false, 0xBF, "/"),
    ("ShiftRight", 0x36, false, 0xA1, "Prawy Shift"),
    ("NumpadMultiply", 0x37, false, 0x6A, "Num *"),
    ("AltLeft", 0x38, false, 0xA4, "Alt"),
    ("Space", 0x39, false, 0x20, "Spacja"),
    ("CapsLock", 0x3A, false, 0x14, "Caps Lock"),
    ("F1", 0x3B, false, 0x70, "F1"),
    ("F2", 0x3C, false, 0x71, "F2"),
    ("F3", 0x3D, false, 0x72, "F3"),
    ("F4", 0x3E, false, 0x73, "F4"),
    ("F5", 0x3F, false, 0x74, "F5"),
    ("F6", 0x40, false, 0x75, "F6"),
    ("F7", 0x41, false, 0x76, "F7"),
    ("F8", 0x42, false, 0x77, "F8"),
    ("F9", 0x43, false, 0x78, "F9"),
    ("F10", 0x44, false, 0x79, "F10"),
    ("Pause", 0x45, false, 0x13, "Pause"),
    ("NumLock", 0x45, true, 0x90, "Num Lock"),
    ("ScrollLock", 0x46, false, 0x91, "Scroll Lock"),
    ("Numpad7", 0x47, false, 0x67, "Num 7"),
    ("Numpad8", 0x48, false, 0x68, "Num 8"),
    ("Numpad9", 0x49, false, 0x69, "Num 9"),
    ("NumpadSubtract", 0x4A, false, 0x6D, "Num -"),
    ("Numpad4", 0x4B, false, 0x64, "Num 4"),
    ("Numpad5", 0x4C, false, 0x65, "Num 5"),
    ("Numpad6", 0x4D, false, 0x66, "Num 6"),
    ("NumpadAdd", 0x4E, false, 0x6B, "Num +"),
    ("Numpad1", 0x4F, false, 0x61, "Num 1"),
    ("Numpad2", 0x50, false, 0x62, "Num 2"),
    ("Numpad3", 0x51, false, 0x63, "Num 3"),
    ("Numpad0", 0x52, false, 0x60, "Num 0"),
    ("NumpadDecimal", 0x53, false, 0x6E, "Num ,"),
    ("IntlBackslash", 0x56, false, 0xE2, "<"),
    ("F11", 0x57, false, 0x7A, "F11"),
    ("F12", 0x58, false, 0x7B, "F12"),
    ("F13", 0x64, false, 0x7C, "F13"),
    ("F14", 0x65, false, 0x7D, "F14"),
    ("F15", 0x66, false, 0x7E, "F15"),
    ("F16", 0x67, false, 0x7F, "F16"),
    ("F17", 0x68, false, 0x80, "F17"),
    ("F18", 0x69, false, 0x81, "F18"),
    ("F19", 0x6A, false, 0x82, "F19"),
    ("F20", 0x6B, false, 0x83, "F20"),
    ("F21", 0x6C, false, 0x84, "F21"),
    ("F22", 0x6D, false, 0x85, "F22"),
    ("F23", 0x6E, false, 0x86, "F23"),
    ("F24", 0x76, false, 0x87, "F24"),
    ("NumpadEnter", 0x1C, true, 0x0D, "Num Enter"),
    ("ControlRight", 0x1D, true, 0xA3, "Prawy Ctrl"),
    ("NumpadDivide", 0x35, true, 0x6F, "Num /"),
    ("PrintScreen", 0x37, true, 0x2C, "Print Screen"),
    ("AltRight", 0x38, true, 0xA5, "Prawy Alt"),
    ("Home", 0x47, true, 0x24, "Home"),
    ("ArrowUp", 0x48, true, 0x26, "Strzałka w górę"),
    ("PageUp", 0x49, true, 0x21, "Page Up"),
    ("ArrowLeft", 0x4B, true, 0x25, "Strzałka w lewo"),
    ("ArrowRight", 0x4D, true, 0x27, "Strzałka w prawo"),
    ("End", 0x4F, true, 0x23, "End"),
    ("ArrowDown", 0x50, true, 0x28, "Strzałka w dół"),
    ("PageDown", 0x51, true, 0x22, "Page Down"),
    ("Insert", 0x52, true, 0x2D, "Insert"),
    ("Delete", 0x53, true, 0x2E, "Delete"),
    ("MetaLeft", 0x5B, true, 0x5B, "Win"),
    ("MetaRight", 0x5C, true, 0x5C, "Prawy Win"),
    ("ContextMenu", 0x5D, true, 0x5D, "Menu"),
];

/// Klawisz z `KeyboardEvent.code` i `KeyboardEvent.key` z okna aplikacji (zamiennik
/// `nativeScanCode` z Qt). Nazwa: polska z tabeli, inaczej znak z `key_text` wielką literą
/// (np. „W”, „Ś” w układzie polskim), inaczej nazwa z tabeli. Nieznany `code` - None.
pub fn key_from_code(code: &str, key_text: &str) -> Option<Key> {
    let &(_, scan, extended, vk, table_name) = CODES.iter().find(|(c, ..)| *c == code)?;
    let mut chars = key_text.chars();
    let printable = match (chars.next(), chars.next()) {
        (Some(c), None) if !c.is_control() && !c.is_whitespace() => {
            Some(c.to_uppercase().collect::<String>())
        }
        _ => None,
    };
    let name = match (polish_name(scan, extended), printable) {
        (Some(n), _) => n.to_string(),
        (None, Some(text)) => text,
        (None, None) => table_name.to_string(),
    };
    Some(Key::new(scan, extended, &name, vk))
}

/// Czy `KeyboardEvent.code` to sam modyfikator (przy wyborze kombinacji czekamy wtedy na
/// właściwy klawisz).
pub fn is_modifier_code(code: &str) -> bool {
    matches!(
        code,
        "ControlLeft"
            | "ControlRight"
            | "ShiftLeft"
            | "ShiftRight"
            | "AltLeft"
            | "AltRight"
            | "MetaLeft"
            | "MetaRight"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combo(scan: u16, name: &str, vk: i64, mods: &[Mod]) -> Key {
        Key::new(scan, false, name, vk).with_mods(mods.to_vec())
    }

    #[test]
    fn native_scan_codes_map_to_named_keys() {
        assert_eq!(
            key_from_native(0x148, "Up", 0),
            Some(Key::new(0x48, true, "Strzałka w górę", 0))
        );
        assert_eq!(
            key_from_native(0x11, "W", 0),
            Some(Key::new(0x11, false, "W", 0))
        );
        assert_eq!(
            key_from_native(0x1D, "Ctrl", 0),
            Some(Key::new(0x1D, false, "Ctrl", 0))
        );
        assert_eq!(
            key_from_native(0x11D, "Ctrl", 0),
            Some(Key::new(0x1D, true, "Prawy Ctrl", 0))
        );
        assert_eq!(key_from_native(0x00, "?", 0), None);
        assert_eq!(
            key_from_native(0x42, " ", 0).map(|k| k.name),
            Some("Klawisz nr 0x42".into())
        );
    }

    #[test]
    fn problems_are_explained() {
        let (walk, run, toggle) = (
            Key::walk_default(),
            Key::run_default(),
            Key::toggle_default(),
        );
        assert_eq!(problem(&walk, &run, &toggle), None);
        assert!(
            problem(&walk, &walk, &toggle)
                .unwrap_or_default()
                .contains("różne")
        );
        assert!(
            problem(&Key::new(0x40, false, "F6", 0), &run, &toggle)
                .unwrap_or_default()
                .contains("F6")
        );
        let f8 = Key::new(0x42, false, "F8", 0);
        assert!(
            problem(&f8, &run, &toggle)
                .unwrap_or_default()
                .contains("Klawisz włączania")
        );
        assert!(
            problem(&walk, &run, &Key::new(0x11, false, "W", 0x57))
                .unwrap_or_default()
                .contains("Klawisz włączania")
        );
        let f9 = Key::new(0x43, false, "F9", 0x78);
        assert_eq!(problem(&f8, &run, &f9), None);
        assert!(
            problem(&walk, &run, &Key::new(0x43, false, "F9", 0))
                .unwrap_or_default()
                .contains("inny")
        );
    }

    #[test]
    fn saved_keys_round_trip_and_fallback() {
        let up = Key::new(0x48, true, "Strzałka w górę", 0);
        assert_eq!(Key::from_value(&up.to_value(), &Key::walk_default()), up);
        assert_eq!(
            Key::from_value(&Value::Null, &Key::walk_default()),
            Key::walk_default()
        );
        let bad = serde_json::json!({"scan": 9999});
        assert_eq!(
            Key::from_value(&bad, &Key::run_default()),
            Key::run_default()
        );
        let toggle = Key::toggle_default();
        assert_eq!(
            Key::from_value(&toggle.to_value(), &Key::walk_default()),
            toggle
        );
        let old = serde_json::json!({"scan": 0x11, "name": "W"});
        assert_eq!(
            Key::from_value(&old, &Key::run_default()),
            Key::new(0x11, false, "W", 0)
        );
    }

    #[test]
    fn toggle_combinations_allowed() {
        let (walk, run) = (Key::walk_default(), Key::run_default());
        for key in [
            combo(0x42, "F8", 0x77, &[Mod::Ctrl, Mod::Shift]),
            combo(0x42, "F8", 0x77, &[Mod::Ctrl]),
            combo(0x42, "F8", 0x77, &[Mod::Alt]),
            combo(0x42, "F8", 0x77, &[Mod::Shift]),
            combo(0x30, "B", 0x42, &[Mod::Ctrl, Mod::Shift]),
            combo(0x22, "G", 0x47, &[Mod::Ctrl, Mod::Alt]),
        ] {
            assert_eq!(problem(&walk, &run, &key), None, "{}", key.label());
        }
        assert_eq!(
            combo(0x42, "F8", 0x77, &[Mod::Ctrl, Mod::Shift]).label(),
            "Ctrl+Shift+F8"
        );
    }

    #[test]
    fn toggle_combinations_that_collide_are_rejected() {
        let (walk, run) = (Key::walk_default(), Key::run_default());
        let cases = [
            (combo(0x42, "F8", 0x77, &[Mod::Win]), "Windows"),
            (combo(0x0F, "Tab", 0x09, &[Mod::Alt]), "Alt+Tab"),
            (combo(0x3E, "F4", 0x73, &[Mod::Alt]), "Alt+F4"),
            (combo(0x0F, "Tab", 0x09, &[Mod::Shift]), "Steam"),
            (combo(0x2C, "Z", 0x5A, &[Mod::Alt]), "NVIDIA"),
            (combo(0x01, "Esc", 0x1B, &[Mod::Ctrl, Mod::Shift]), "Esc"),
            (combo(0x1F, "S", 0x53, &[Mod::Ctrl, Mod::Alt]), "AltGr"),
            (combo(0x12, "E", 0x45, &[Mod::Ctrl, Mod::Alt]), "AltGr"),
            (combo(0x10, "Q", 0x51, &[Mod::Shift]), "wielką literę"),
            (combo(0x2E, "C", 0x43, &[Mod::Ctrl]), "kopiuje"),
            (Key::new(0x58, false, "F12", 0x7B), "F12"),
            (combo(0x1D, "Ctrl", 0xA2, &[Mod::Shift]), "tylko z Ctrl"),
        ];
        for (key, reason) in cases {
            let found = problem(&walk, &run, &key);
            assert!(
                found.as_deref().is_some_and(|f| f.contains(reason)),
                "{} {found:?}",
                key.label()
            );
        }
        assert!(problem(&walk, &run, &combo(0x11, "W", 0x57, &[Mod::Ctrl])).is_some());
    }

    #[test]
    fn combination_survives_saving() {
        let key = combo(0x42, "F8", 0x77, &[Mod::Shift, Mod::Ctrl]);
        assert_eq!(key.mods, [Mod::Shift, Mod::Ctrl]);
        let saved = Key::from_value(&key.to_value(), &Key::toggle_default());
        assert_eq!(saved.mods, [Mod::Ctrl, Mod::Shift]);
        let plain = Key::toggle_default().to_value();
        assert!(plain.get("mods").is_none());
        // serde daje ten sam zapis co to_value (aplikacja może zapisywać przez serde)
        assert_eq!(
            serde_json::to_value(Key::toggle_default()).ok(),
            Some(plain)
        );
    }

    #[test]
    fn window_key_codes_map_to_scan_codes() {
        assert_eq!(key_from_code("KeyW", "w"), Some(Key::walk_default()));
        assert_eq!(
            key_from_code("ShiftLeft", "Shift"),
            Some(Key::run_default())
        );
        assert_eq!(key_from_code("F8", "F8"), Some(Key::toggle_default()));
        assert_eq!(
            key_from_code("ArrowUp", "ArrowUp"),
            Some(Key::new(0x48, true, "Strzałka w górę", 0x26))
        );
        assert_eq!(key_from_code("KeyS", "ś").map(|k| k.name), Some("Ś".into()));
        assert_eq!(
            key_from_code("Space", " ").map(|k| k.name),
            Some("Spacja".into())
        );
        assert_eq!(key_from_code("Nieznany", "x"), None);
        assert!(is_modifier_code("AltRight") && !is_modifier_code("KeyA"));
        assert_eq!(
            mods_from_flags(true, false, true, false),
            [Mod::Ctrl, Mod::Shift]
        );
        // każdy kod z tabeli tłumaczy się z powrotem przez key_from_native na ten sam klawisz
        for (code, scan, ext, vk, _) in CODES {
            let k = key_from_code(code, "");
            let native = u32::from(*scan) | if *ext { 0x100 } else { 0 };
            let n = key_from_native(
                native,
                k.as_ref().map(|k| k.name.as_str()).unwrap_or(""),
                *vk,
            );
            assert_eq!(k, n, "{code}");
        }
    }
}
