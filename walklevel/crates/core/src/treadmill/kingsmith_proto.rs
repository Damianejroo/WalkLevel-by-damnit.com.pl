//! Protokół KingSmith (WalkingPad R2 / X21 / G1 / X23): kodowanie wiadomości i odbiór danych
//! (port bieznia/sterowniki/kingsmith/protokol.py i części odbiorczej sterownika).
//!
//! Na podstawie otwartego sterownika QZ (qdomyos-zwift, GPL-3.0). Wiadomość to tekst, np.
//! "servers getProp 1 2 7", który idzie tak: UTF-8 -> base64 -> podstawienie znaków tablicą
//! -> znak '\r' -> kawałki po 16 bajtów. Odpowiedzi wracają tak samo, np.
//! "props CurrentSpeed 3.5 spm 98".
//!
//! Bieżnie różnią się tablicą podstawień (QZ zna 7 wariantów). Nie zgadujemy: tablicę
//! wykrywamy z odpowiedzi bieżni, bo tylko właściwa daje sensowny tekst.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use base64::Engine;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use regex::Regex;

use super::kontrakt::{Capabilities, Counts, GattCharacteristic, GattService, Reading, py_split};

/// Alfabet base64 w kolejności, w której tablice podają zamienniki.
pub const PLAINTEXT: &[u8; 65] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=";

/// Tablice podstawień (nazwa, tablica) - kolejność jak w QZ.
pub const TABLES: [(&str, &[u8; 65]); 7] = [
    (
        "v1",
        b"SaCw4FGHIJqLhN+P9RVTU/WcY6ObDdefgEijklmnopQrsBuvMxXz1yA2t5078KZ3=",
    ),
    (
        "v2",
        b"ZaCw4FGHIJqLhN+P9RMTU/WcY6ObDdefgEijklmnopQrsBuvVxXz1yA2t5078KS3=",
    ),
    (
        "v3",
        b"0aCw4FGHIJqLhN+P9RVTU/WcY6ObDdefgEijklmnopQrsBuvMxXz1yA2t5Z78KS3=",
    ),
    (
        "v4",
        b"ZaCw4FGHIJqLhN9P+RVTU/WcY6ObDdefgEijklmnopQrsBuvMxXz1yA2t5078KS3=",
    ),
    (
        "v5",
        b"iaCw4FGHIJqLhN+P9RVTU/WcY6ObDdefgEZjklmnopQrsBuvMxXz1yA2t5078KS3=",
    ),
    (
        "v6",
        b"ZaCw4FGHIJqLhN+P8RVTU/WcY6ObDdefgEijklmnopQrsBuvMxXz1yA2t5079KS3=",
    ),
    (
        "v7",
        b"baCw4FGHIJqLhN+P9RVTU/WcY6OZDdefgEijklmnopQrsBuvMxXz1yA2t5078KS3=",
    ),
];

pub const END: u8 = b'\r';
pub const CHUNK: usize = 16;
pub const MAX_BUFFER: usize = 4096;

/// Pierwsze słowa odpowiedzi, które na pewno pochodzą od bieżni KingSmith.
pub const KNOWN_WORDS: [&str; 9] = [
    "props",
    "shake",
    "net",
    "get_dn",
    "get_pk",
    "time_posix",
    "version",
    "servers",
    "format",
];

/// Sekwencja startowa jak w QZ (bez niej bieżnia nie odpowiada na odczyty).
/// `{now}` zastępuje [`init_commands`] czasem uniksowym.
pub const INIT_COMMANDS: [&str; 6] = [
    "shake",
    "net",
    "get_dn",
    "get_pk",
    "time_posix {now}",
    "version",
];
pub const GET_PROPS: &str = "servers getProp 1 2 7 12 23 24 31";
/// Subskrypcja telemetrii oficjalnej aplikacji KS Fit (wg opisu protokołu z projektu
/// beltcontrol). Tylko odczyt. Na części modeli daje częstsze, częściowe ramki "props".
pub const TELEMETRY_PROPS: &str = "servers getProp 1 9 15 2 10 11 12 13 14";

/// Start: przejęcie sterowania (ControlMode 1) i ruch (runState 1) - taśma rusza ze stałą
/// prędkością startową bieżni (X21: 1,0 km/h). Stop i pauza to to samo polecenie.
pub const CMD_CONTROL: &str = "props ControlMode 1";
pub const CMD_START: &str = "props runState 1";
pub const CMD_STOP: &str = "props runState 0";
pub const SPEED_MIN: f64 = 0.5;
pub const SPEED_MAX: f64 = 12.0;

/// Domyślne możliwości modułu - profil modelu (np. kingsmith-x21.json) je uzupełnia.
pub const DEFAULT_CAPS: Capabilities = Capabilities {
    speed_min: SPEED_MIN,
    speed_max: SPEED_MAX,
    speed_step: 0.1,
    start_speed: 1.0,
    start_delay_s: 0.0,
    acks_commands: false,
    pause: false,
    steps: true,
    incline: None,
    telemetry_s: 2.0,
    min_command_gap_s: 0.0,
    control: true,
};

/// Najdłuższa przerwa między ramkami z prędkością, przy której potwierdzenia odpytywania
/// jeszcze podtrzymują ostatnią prędkość (X21 wysyła ją co ok. 2 s).
pub const PROPS_TIMEOUT_S: f64 = 6.0;
/// Kody błędów od producenta (MIoT: -9999..-5000) = odmowa polecenia.
pub const VENDOR_ERRORS: std::ops::Range<i64> = -9999..-4999;

/// Dekoder base64 jak `base64.b64decode(..., validate=True)` w Pythonie 3.13: wymagane
/// poprawne dopełnienie, nadmiarowe bity ostatniego znaku pomijane.
const B64: GeneralPurpose = GeneralPurpose::new(
    &base64::alphabet::STANDARD,
    GeneralPurposeConfig::new()
        .with_decode_allow_trailing_bits(true)
        .with_decode_padding_mode(DecodePaddingMode::RequireCanonical),
);

static STRICT: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r#"^[A-Za-z0-9_.:+\-/,"=() ]+$"#).ok());

fn table(name: &str) -> Option<&'static [u8; 65]> {
    TABLES.iter().find(|(n, _)| *n == name).map(|(_, t)| *t)
}

/// Czy taka tablica istnieje.
pub fn is_table(name: &str) -> bool {
    table(name).is_some()
}

/// Nazwy wszystkich tablic w kolejności QZ.
pub fn table_names() -> Vec<&'static str> {
    TABLES.iter().map(|(n, _)| *n).collect()
}

fn static_name(name: &str) -> Option<&'static str> {
    TABLES.iter().find(|(n, _)| *n == name).map(|(n, _)| *n)
}

/// Polecenia startowe z bieżącym czasem uniksowym w `time_posix`.
pub fn init_commands(now_unix: i64) -> Vec<String> {
    INIT_COMMANDS
        .iter()
        .map(|c| c.replace("{now}", &now_unix.to_string()))
        .collect()
}

/// Prędkość w zakresie bieżni. Dokładnie 0 przechodzi bez zmian (test: 0 km/h bez stopu).
pub fn speed_command(kmh: f64) -> String {
    if kmh <= 0.0 {
        return "props CurrentSpeed 0".to_string();
    }
    let value = super::kontrakt::py_round1(SPEED_MAX.min(SPEED_MIN.max(kmh)));
    // jak `{value:g}` w Pythonie: liczba bez zbędnych zer, np. "1.5", "3"
    format!("props CurrentSpeed {value}")
}

/// Kolejność prób: X23 (KS-NACH-MXG) używa v7, reszta zwykle v1.
pub fn table_order(device_name: &str) -> Vec<&'static str> {
    let mut order = table_names();
    if device_name.to_uppercase().starts_with("KS-NACH-MXG") {
        order.retain(|n| *n != "v7");
        order.insert(0, "v7");
    }
    order
}

/// Gotowa wiadomość do wysłania (razem z kończącym '\r'). None dla nieznanej tablicy.
pub fn encode(text: &str, table_name: &str) -> Option<Vec<u8>> {
    let table = table(table_name)?;
    let plain = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    let mut out: Vec<u8> = plain
        .bytes()
        .map(|b| {
            PLAINTEXT
                .iter()
                .position(|p| *p == b)
                .map_or(b, |i| table[i])
        })
        .collect();
    out.push(END);
    Some(out)
}

/// Kawałki po `size` bajtów (zapis do charakterystyki).
pub fn chunks(data: &[u8], size: usize) -> impl Iterator<Item = &[u8]> {
    data.chunks(size.max(1))
}

/// Odszyfrowuje jedną ramkę (bez '\r'). None, jeśli tablica nie pasuje.
pub fn decode(frame: &[u8], table_name: &str) -> Option<String> {
    let table = table(table_name)?;
    if frame.is_empty() {
        return None;
    }
    let mut plain = Vec::with_capacity(frame.len());
    for b in frame {
        let i = table.iter().position(|t| t == b)?;
        plain.push(PLAINTEXT[i]);
    }
    let raw = B64.decode(&plain).ok()?;
    if !raw.is_ascii() {
        return None;
    }
    let text = String::from_utf8(raw).ok()?;
    // X21 kończy odpowiedzi znakiem CR/LF wewnątrz zaszyfrowanej treści
    let text = text.trim_end_matches(['\r', '\n']);
    let printable = text
        .chars()
        .all(|c| c == '\t' || ('\x20'..='\x7e').contains(&c));
    printable.then(|| text.to_string())
}

/// Odpowiedź, która na pewno jest poprawnie odszyfrowana.
///
/// Zła tablica zmienia pojedyncze znaki (np. 'props ;urrentSpeed', 'shake 0?'), więc oprócz
/// znanego pierwszego słowa wymagamy samych „zwykłych” znaków.
pub fn is_known_reply(text: &str) -> bool {
    let Some(first) = py_split(text).next() else {
        return false;
    };
    let strict = STRICT.as_ref().is_some_and(|re| re.is_match(text));
    KNOWN_WORDS.contains(&first.to_lowercase().as_str()) && strict
}

/// Skleja powiadomienia BLE w pełne ramki zakończone '\r'.
#[derive(Debug, Clone, Default)]
pub struct FrameBuffer {
    buffer: Vec<u8>,
}

impl FrameBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Dokleja dane i zwraca pełne, niepuste ramki (bez '\r').
    pub fn feed(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
        self.buffer.extend_from_slice(data);
        if self.buffer.len() > MAX_BUFFER {
            // śmieci bez '\r' - nie rośnij w nieskończoność
            self.buffer.clear();
            return Vec::new();
        }
        let mut parts: Vec<Vec<u8>> = self
            .buffer
            .split(|b| *b == END)
            .map(<[u8]>::to_vec)
            .collect();
        self.buffer = parts.pop().unwrap_or_default();
        parts.into_iter().filter(|f| !f.is_empty()).collect()
    }
}

/// Odszyfrowany tekst i czy można mu ufać (wszystkie możliwe tablice dają ten sam tekst).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    pub text: String,
    pub trusted: bool,
}

/// Szyfrowanie z wykrywaniem tablicy na podstawie odpowiedzi bieżni.
///
/// Tablice różnią się tylko kilkoma znakami, więc jedna ramka często pasuje do kilku z nich.
/// Trzymamy zbiór kandydatów i zawężamy go każdą poprawną ramką. Tekst oznaczamy jako pewny
/// tylko wtedy, gdy wszyscy kandydaci odszyfrowują go identycznie - dzięki temu nigdy nie
/// podamy błędnej prędkości.
#[derive(Debug, Clone)]
pub struct Codec {
    candidates: Vec<&'static str>,
    table: &'static str,
}

impl Default for Codec {
    fn default() -> Self {
        Self::new(&[])
    }
}

impl Codec {
    /// Kolejność prób (nazwy tablic); pusta albo bez znanych nazw = wszystkie tablice.
    pub fn new(order: &[&str]) -> Self {
        let mut candidates: Vec<&'static str> =
            order.iter().filter_map(|n| static_name(n)).collect();
        if candidates.is_empty() {
            candidates = table_names();
        }
        let table = candidates.first().copied().unwrap_or("v1");
        Self { candidates, table }
    }

    /// Tablica, którą szyfrujemy wysyłane wiadomości.
    pub fn table(&self) -> &'static str {
        self.table
    }

    /// Tablice, które wciąż pasują do odpowiedzi bieżni.
    pub fn candidates(&self) -> &[&'static str] {
        &self.candidates
    }

    pub fn locked(&self) -> bool {
        self.candidates.len() == 1
    }

    pub fn describe(&self) -> String {
        if self.locked() {
            self.table.to_string()
        } else {
            self.candidates.join("/")
        }
    }

    pub fn encode(&self, text: &str) -> Vec<u8> {
        encode(text, self.table).unwrap_or_default()
    }

    pub fn decode(&mut self, frame: &[u8]) -> Option<Decoded> {
        let mut texts: Vec<(&'static str, Option<String>)> = self
            .candidates
            .iter()
            .map(|n| (*n, decode(frame, n)))
            .collect();
        let mut good = known_tables(&texts);
        if good.is_empty() {
            // Nic pewnego (albo bieżnia zmieniła zachowanie) - sprawdź wszystkie tablice.
            texts = TABLES.iter().map(|(n, _)| (*n, decode(frame, n))).collect();
            good = known_tables(&texts);
            if good.is_empty() {
                return None;
            }
        }
        self.candidates = good;
        if !self.candidates.contains(&self.table) {
            self.table = self.candidates[0];
        }
        let text_of = |name: &str| {
            texts
                .iter()
                .find(|(n, _)| *n == name)
                .and_then(|(_, t)| t.clone())
        };
        let first = text_of(self.candidates[0]);
        let trusted = self.candidates.iter().all(|n| text_of(n) == first);
        let text = text_of(self.table)?;
        Some(Decoded { text, trusted })
    }

    /// Bieżnia milczy - wysyłaj kolejną z pozostałych tablic.
    pub fn try_next_table(&mut self) {
        if !self.locked() {
            let i = self
                .candidates
                .iter()
                .position(|n| *n == self.table)
                .unwrap_or(0);
            self.table = self.candidates[(i + 1) % self.candidates.len()];
        }
    }
}

fn known_tables(texts: &[(&'static str, Option<String>)]) -> Vec<&'static str> {
    texts
        .iter()
        .filter(|(_, t)| {
            t.as_deref()
                .is_some_and(|t| !t.is_empty() && is_known_reply(t))
        })
        .map(|(n, _)| *n)
        .collect()
}

/// 'props CurrentSpeed 3.5 spm 98 ...' -> {'CurrentSpeed': 3.5, 'spm': 98.0}.
///
/// Jak w QZ: pary klucz-wartość, 'Error' kończy listę, wartości tekstowe pomijamy. Ten sam
/// klucz drugi raz nadpisuje pierwszy.
pub fn parse_props(text: &str) -> BTreeMap<String, f64> {
    let parts: Vec<&str> = py_split(text).collect();
    let mut props = BTreeMap::new();
    if parts.first() != Some(&"props") {
        return props;
    }
    let mut i = 1;
    while i + 1 < parts.len() {
        let (key, value) = (parts[i], parts[i + 1]);
        if key == "Error" {
            break;
        }
        if let Some(number) = py_float(value) {
            props.insert(key.to_string(), number);
        }
        i += 2;
    }
    props
}

/// `float(tekst)` z Pythona: także "inf", "nan", "1e3", ".5" i podkreślenia między cyframi.
fn py_float(text: &str) -> Option<f64> {
    let cleaned = if text.contains('_') {
        let bytes = text.as_bytes();
        let ok = bytes.iter().enumerate().all(|(i, b)| {
            *b != b'_'
                || (i > 0
                    && i + 1 < bytes.len()
                    && bytes[i - 1].is_ascii_digit()
                    && bytes[i + 1].is_ascii_digit())
        });
        if !ok {
            return None;
        }
        text.replace('_', "")
    } else {
        text.to_string()
    };
    let lower = cleaned.to_ascii_lowercase();
    let body = lower.trim_start_matches(['+', '-']);
    let special = matches!(body, "inf" | "infinity" | "nan");
    if !special
        && !body
            .bytes()
            .all(|b| b.is_ascii_digit() || b == b'.' || b == b'e' || b == b'+' || b == b'-')
    {
        return None;
    }
    cleaned.parse().ok()
}

// --- usługi GATT ---------------------------------------------------------------------------

/// Kanał KingSmith: usługa z charakterystykami zapisu (fed7) i powiadomień (fed8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Channel {
    pub service_uuid: String,
    pub write: GattCharacteristic,
    pub notify: GattCharacteristic,
}

impl Channel {
    /// Krótka etykieta do logu (pierwsze 8 znaków UUID usługi).
    pub fn label(&self) -> &str {
        self.service_uuid.get(..8).unwrap_or(&self.service_uuid)
    }
}

/// UUID KingSmith: 0000fed7-…, 0001fed7-…, 0002fed7-… (zależnie od wersji firmware X21).
fn short_uuid(uuid: &str) -> Option<&'static str> {
    let lower = uuid.to_lowercase();
    let rest = lower.strip_prefix("000")?;
    let mut chars = rest.chars();
    if !matches!(chars.next(), Some('0'..='2')) {
        return None;
    }
    let tail = chars.as_str();
    ["fed7", "fed8", "1234"]
        .into_iter()
        .find(|s| tail.strip_prefix(s) == Some("-0000-1000-8000-00805f9b34fb"))
}

/// Usługa bieżni KingSmith (xxxx1234-…), do rozpoznawania w wynikach skanowania.
pub fn is_kingsmith_service(uuid: &str) -> bool {
    short_uuid(uuid) == Some("1234")
}

/// Kanały KingSmith w kolejności preferencji usług (0002, 0001, 0000, reszta).
pub fn find_channels(services: &[GattService]) -> Vec<Channel> {
    let mut channels = Vec::new();
    for service in services {
        let mut write = None;
        let mut notify = None;
        for c in &service.characteristics {
            match short_uuid(&c.uuid) {
                Some("fed7") => write = Some(c.clone()),
                Some("fed8") => notify = Some(c.clone()),
                _ => {}
            }
        }
        if let (Some(write), Some(notify)) = (write, notify) {
            channels.push(Channel {
                service_uuid: service.uuid.to_lowercase(),
                write,
                notify,
            });
        }
    }
    let rank = |ch: &Channel| {
        let prefix = ch.service_uuid.get(..4).unwrap_or("");
        ["0002", "0001", "0000"]
            .iter()
            .position(|p| *p == prefix)
            .unwrap_or(9)
    };
    channels.sort_by_key(rank);
    channels
}

// --- odbiór danych --------------------------------------------------------------------------

/// Część odbiorcza sterownika KingSmith: ramki -> odczyty i stan taśmy.
///
/// Sterownik (połączenie BLE) podaje tu każde powiadomienie z czasem z zegara monotonicznego;
/// wynik to odczyty dla silnika i stan potrzebny do startu, stopu i pilnowania prędkości.
#[derive(Debug, Clone)]
pub struct Telemetry {
    pub codec: Codec,
    pub last_speed: f64,
    pub last_steps: Option<i64>,
    /// czas ostatniej ramki z danymi (`f64::NEG_INFINITY` = jeszcze żadnej)
    pub last_props_at: f64,
    pub run_state: Option<i64>,
    pub control_mode: Option<i64>,
    pub last_error: Option<i64>,
    /// wersja z odpowiedzi "version ..." (pusta, dopóki nie przyjdzie)
    pub firmware: String,
    pub counts: Counts,
    /// rośnie przy każdej ramce, która rozstrzyga o stanie (dla oczekujących na start/stop)
    pub state_changes: u64,
    /// rośnie przy każdej odszyfrowanej ramce (bieżnia żyje)
    pub replies: u64,
    /// czas ostatniej odszyfrowanej ramki (0 = jeszcze żadnej, jak `last_reply` w Pythonie)
    pub last_reply_at: f64,
    /// kanał (UUID usługi), z którego przyszła pierwsza odszyfrowana ramka - potem polecenia
    /// idą tylko nim (None = wysyłaj wszystkimi)
    pub active: Option<String>,
    buffers: BTreeMap<String, FrameBuffer>,
}

impl Telemetry {
    /// `order` - kolejność tablic (np. z [`table_order`] i opcji profilu "table").
    pub fn new(order: &[&str]) -> Self {
        Self {
            codec: Codec::new(order),
            last_speed: 0.0,
            last_steps: None,
            last_props_at: f64::NEG_INFINITY,
            run_state: None,
            control_mode: None,
            last_error: None,
            firmware: String::new(),
            counts: Counts::default(),
            state_changes: 0,
            replies: 0,
            last_reply_at: 0.0,
            active: None,
            buffers: BTreeMap::new(),
        }
    }

    /// Kolejność tablic dla modelu: X23 najpierw v7, opcja profilu "table" na początek.
    pub fn order_for(device_name: &str, profile_table: Option<&str>) -> Vec<&'static str> {
        let mut order = table_order(device_name);
        if let Some(name) = profile_table.and_then(static_name)
            && order.contains(&name)
        {
            order.retain(|n| *n != name);
            order.insert(0, name);
        }
        order
    }

    /// Czy taśma jedzie (runState z ramek); None, gdy jeszcze nie wiadomo.
    pub fn belt_running(&self) -> Option<bool> {
        self.run_state.map(|s| s == 1)
    }

    /// Prędkość z ostatniej ramki z danymi i czas tej ramki.
    pub fn belt_speed(&self) -> Option<(f64, f64)> {
        (self.last_props_at != f64::NEG_INFINITY).then_some((self.last_speed, self.last_props_at))
    }

    /// Powiadomienie z kanału (`channel` - UUID usługi; każdy kanał ma własny bufor ramek).
    pub fn feed(&mut self, channel: &str, data: &[u8], now: f64) -> Vec<Reading> {
        let frames = self
            .buffers
            .entry(channel.to_string())
            .or_default()
            .feed(data);
        let mut readings = Vec::new();
        for frame in frames {
            let Some(decoded) = self.codec.decode(&frame) else {
                self.counts.undecoded += 1;
                log::debug!("   (nie da się odszyfrować ramki) {}", hex::encode(&frame));
                continue;
            };
            self.counts.frames += 1;
            if decoded.text.starts_with("servers") {
                self.counts.acks += 1; // odpowiedź na zapytanie - liczona, nie zapisywana
            } else {
                log::debug!(
                    "   {}{}",
                    decoded.text,
                    if decoded.trusted { "" } else { "  (niepewne)" }
                );
            }
            if self.active.is_none() {
                self.active = Some(channel.to_string());
                log::info!(
                    "Bieżnia odpowiada (usługa {channel}, tablica {})",
                    self.codec.describe()
                );
            }
            self.replies += 1;
            self.last_reply_at = now;
            if decoded.trusted {
                // niepewny tekst mógłby mieć przekłamaną cyfrę prędkości
                readings.extend(self.handle(&decoded.text, now));
            }
        }
        readings
    }

    /// Jedna pewna odpowiedź bieżni: stan taśmy i odczyt (jeśli coś niesie).
    pub fn handle(&mut self, text: &str, now: f64) -> Option<Reading> {
        self.track_state(text);
        let props = parse_props(text);
        if props.is_empty() {
            // X21 wysyła prędkość sam co ok. 2 s, a na nasze odpytywanie odpowiada krótkim
            // "servers 0". To potwierdzenie = bieżnia żyje, prędkość bez zmian. Bez świeżej
            // ramki z prędkością (PROPS_TIMEOUT_S) nic nie oddajemy - wtedy bezpiecznik
            // w silniku zatrzyma postać.
            if text.starts_with("servers") && now - self.last_props_at < PROPS_TIMEOUT_S {
                return Some(self.reading());
            }
            return None;
        }
        self.last_props_at = now;
        self.state_changes += 1; // każda ramka rozstrzyga, czy polecenie zadziałało
        if let Some(speed) = props.get("CurrentSpeed") {
            self.last_speed = speed.max(0.0);
        }
        if props.get("runState") == Some(&0.0) {
            self.last_speed = 0.0; // taśma zatrzymana
        }
        if let Some(steps) = props.get("RunningSteps").and_then(|s| py_int(*s)) {
            self.last_steps = Some(steps);
        }
        // Każda ramka z danymi = żywa bieżnia, nawet jeśli prędkość się nie zmieniła.
        Some(self.reading())
    }

    fn reading(&self) -> Reading {
        Reading {
            speed_kmh: self.last_speed,
            steps: self.last_steps,
        }
    }

    /// Stan taśmy i odpowiedzi na polecenia (start/stop, odmowa).
    fn track_state(&mut self, text: &str) {
        let parts: Vec<&str> = py_split(text).collect();
        if parts.len() == 2 && parts[0] == "version" && self.firmware.is_empty() {
            self.firmware = parts[1].to_string();
        }
        if parts.len() >= 3 && parts[0] == "props" && parts[1] == "Error" {
            if let Some(code) = parts.last().and_then(|p| py_int_text(p)) {
                self.last_error = Some(code);
                if !VENDOR_ERRORS.contains(&code) {
                    log::info!("Bieżnia zgłasza błąd {code} (nie odmowa)");
                }
            }
            self.state_changes += 1;
            return;
        }
        let props = parse_props(text);
        if let Some(state) = props.get("runState") {
            self.run_state = py_int(*state).or(self.run_state);
            self.state_changes += 1;
        }
        if let Some(mode) = props.get("ControlMode") {
            self.control_mode = py_int(*mode).or(self.control_mode);
            self.state_changes += 1;
        }
        if props.get("CurrentSpeed").copied().unwrap_or(0.0) > 0.0 && self.run_state.is_none() {
            self.run_state = Some(1);
        }
    }
}

/// `int(liczba)` - obcięcie w stronę zera. Nieskończoność i NaN: None (Python rzuca wyjątek
/// i stan zostaje bez zmian - nie udajemy, że taśma stoi).
fn py_int(value: f64) -> Option<i64> {
    value.is_finite().then(|| value.trunc() as i64)
}

/// `int(tekst)` z Pythona: liczba całkowita, opcjonalny znak, podkreślenia między cyframi.
/// Poza zakresem i64 - wartość skrajna (Python ma dowolnie duże liczby; i tak to nie odmowa).
fn py_int_text(text: &str) -> Option<i64> {
    let digits = text.trim_start_matches(['+', '-']);
    if digits.is_empty()
        || digits.starts_with('_')
        || digits.ends_with('_')
        || digits.contains("__")
    {
        return None;
    }
    if !digits.bytes().all(|b| b.is_ascii_digit() || b == b'_') {
        return None;
    }
    let cleaned = text.replace('_', "");
    match cleaned.parse() {
        Ok(value) => Some(value),
        // jeden znak przed cyframi (dwa, np. "+-5", Python odrzuca)
        Err(_) if text.len() - digits.len() <= 1 => Some(if text.starts_with('-') {
            i64::MIN
        } else {
            i64::MAX
        }),
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_are_permutations_of_base64_alphabet() {
        for (_, table) in TABLES {
            let mut a = table.to_vec();
            let mut b = PLAINTEXT.to_vec();
            a.sort_unstable();
            b.sort_unstable();
            assert_eq!(a, b);
        }
    }

    #[test]
    fn encode_decode_round_trip() {
        for name in table_names() {
            let text = "props CurrentSpeed 3.5 spm 98";
            let data = encode(text, name).expect("tablica");
            assert_eq!(data.last(), Some(&END));
            assert_eq!(decode(&data[..data.len() - 1], name).as_deref(), Some(text));
        }
    }

    #[test]
    fn known_vector_matches_qz_algorithm() {
        let v1 = table("v1").expect("v1");
        let mut expected: Vec<u8> = b"c2hha2U="
            .iter()
            .map(|c| v1[PLAINTEXT.iter().position(|p| p == c).expect("znak")])
            .collect();
        expected.push(END);
        assert_eq!(encode("shake", "v1"), Some(expected));
    }

    #[test]
    fn codec_narrows_to_the_right_table_and_never_trusts_wrong_numbers() {
        let replies = [
            "format error",
            "shake 00",
            "props CurrentSpeed 3.5 RunningSteps 120 runState 1",
            "props ControlMode 1 spm 0",
            "version 0014",
        ];
        for name in table_names() {
            let mut codec = Codec::default();
            for reply in replies {
                let data = encode(reply, name).expect("tablica");
                let decoded = codec.decode(&data[..data.len() - 1]).expect("odpowiedź");
                if decoded.trusted {
                    assert_eq!(decoded.text, reply);
                }
                assert!(codec.candidates().contains(&name));
            }
        }
    }

    #[test]
    fn wrong_table_artifacts_are_rejected() {
        assert!(!is_known_reply("props ;urrentSpeed 3.5"));
        assert!(!is_known_reply("shake 0?"));
        assert!(is_known_reply("props CurrentSpeed 3.5"));
    }

    #[test]
    fn frame_buffer_joins_chunks_and_drops_garbage() {
        let mut fb = FrameBuffer::new();
        let msg = encode("props CurrentSpeed 4.5 RunningSteps 1200", "v1").expect("v1");
        let parts: Vec<&[u8]> = chunks(&msg, CHUNK).collect();
        assert!(parts.len() > 1);
        let frames: Vec<Vec<u8>> = parts.iter().flat_map(|p| fb.feed(p)).collect();
        assert_eq!(frames, vec![msg[..msg.len() - 1].to_vec()]);
        let mut fb = FrameBuffer::new();
        assert!(fb.feed(&vec![b'x'; MAX_BUFFER + 1]).is_empty());
        assert_eq!(fb.feed(b"ab\r"), vec![b"ab".to_vec()]);
    }

    #[test]
    fn parse_props_like_qz() {
        let text = "props CurrentSpeed 3.5 mcu_version 1.2.3 spm 98 goal x Error -5000";
        let props = parse_props(text);
        assert_eq!(props.len(), 2);
        assert_eq!(props["CurrentSpeed"], 3.5);
        assert_eq!(props["spm"], 98.0);
        assert!(parse_props("shake 00").is_empty());
    }

    #[test]
    fn x23_prefers_table_v7_and_speed_commands() {
        assert_eq!(table_order("KS-NACH-MXG-1234")[0], "v7");
        assert_eq!(table_order("KS-HDSY-X21C")[0], "v1");
        assert_eq!(speed_command(1.5), "props CurrentSpeed 1.5");
        assert_eq!(speed_command(3.0), "props CurrentSpeed 3");
        assert_eq!(speed_command(0.2), "props CurrentSpeed 0.5");
        assert_eq!(speed_command(0.0), "props CurrentSpeed 0");
        assert_eq!(speed_command(30.0), "props CurrentSpeed 12");
    }

    #[test]
    fn channels_prefer_newer_services() {
        let ch = |service: &str, uuid: &str| GattCharacteristic {
            service_uuid: service.into(),
            uuid: uuid.into(),
            properties: vec![],
        };
        let service = |prefix: &str| {
            let uuid = format!("{prefix}1234-0000-1000-8000-00805f9b34fb");
            GattService {
                characteristics: vec![
                    ch(&uuid, &format!("{prefix}fed7-0000-1000-8000-00805f9b34fb")),
                    ch(&uuid, &format!("{prefix}fed8-0000-1000-8000-00805F9B34FB")),
                ],
                uuid,
            }
        };
        let found = find_channels(&[service("0000"), service("0002"), service("0001")]);
        let labels: Vec<&str> = found.iter().map(Channel::label).collect();
        assert_eq!(labels, ["00021234", "00011234", "00001234"]);
        assert!(is_kingsmith_service("00011234-0000-1000-8000-00805F9B34FB"));
        assert!(!is_kingsmith_service(
            "00031234-0000-1000-8000-00805f9b34fb"
        ));
    }

    #[test]
    fn py_numbers() {
        assert_eq!(py_float("1_0"), Some(10.0));
        assert_eq!(py_float("1__0"), None);
        assert_eq!(py_float("inf"), Some(f64::INFINITY));
        assert_eq!(py_float("0x10"), None);
        assert_eq!(py_int_text("-5000"), Some(-5000));
        assert_eq!(py_int_text("1.5"), None);
        assert_eq!(py_int_text("+5_000"), Some(5000));
        assert_eq!(py_int_text("+-5"), None);
        assert_eq!(py_int_text("99999999999999999999"), Some(i64::MAX));
        assert_eq!(py_int_text("-99999999999999999999"), Some(i64::MIN));
        assert_eq!(py_int(f64::INFINITY), None);
        assert_eq!(py_int(-7.5), Some(-7));
    }

    #[test]
    fn telemetry_keeps_state_on_garbage_numbers_and_tracks_active_channel() {
        let mut t = Telemetry::new(&["v1"]);
        t.handle("props runState 1 ControlMode 1", 1.0);
        t.handle("props runState inf ControlMode nan", 2.0);
        assert_eq!((t.run_state, t.control_mode), (Some(1), Some(1)));
        t.handle("props Error 99999999999999999999", 3.0);
        assert_eq!(t.last_error, Some(i64::MAX)); // błąd, ale nie odmowa producenta
        let msg = encode("version 0014", "v1").expect("v1");
        t.feed("00011234", &msg, 4.0);
        t.feed("00001234", &msg, 5.0);
        assert_eq!(t.active.as_deref(), Some("00011234"));
        assert_eq!((t.last_reply_at, t.firmware.as_str()), (5.0, "0014"));
    }
}
