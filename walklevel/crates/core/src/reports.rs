//! Zgłoś problem: paczka ZIP z opisem i logami, maskowanie, wysyłka - port bieznia/zgloszenia.py.
//!
//! Co trafia do paczki (to samo pokazuje okno przed wysłaniem):
//! - opis problemu i opcjonalny kontakt wpisane przez użytkownika,
//! - wersja aplikacji, wersja Windows, ustawienia aplikacji (dane/ustawienia.json),
//! - logi aplikacji i Bluetooth z wybranego okresu (także starsze części, np. bluetooth.log.1),
//! - podsumowanie.txt: najważniejsze zdarzenia wyciągnięte z logów (polecenia, błędy, sesje),
//! - ostatni raport diagnostyczny (logs/diagnostyka-*.txt).
//!
//! Ścieżka profilu Windows (nazwa użytkownika) i adresy cudzych urządzeń Bluetooth są maskowane.
//! Nic nie jest wysyłane bez wywołania [`send_report`].

pub mod diagnostics;

use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{Datelike, Local, NaiveDateTime, TimeZone, Timelike};
use regex::Regex;
use serde::Serialize;
use serde_json::Value;

/// Nazwa aplikacji w temacie e-maila.
pub const APP_NAME: &str = "WalkLevel";
/// Adres kontaktowy autora (e-mail zamiast wysyłki).
pub const CONTACT: &str = "kontakt@damnit.com.pl";
/// Z każdego pliku logu bierzemy tylko koniec (bajty).
pub const MAX_LOG_BYTES: u64 = 1_000_000;
/// Domyślny okres logów w zgłoszeniu: ostatnie 6 godzin (sekundy).
pub const RECENT_S: f64 = 6.0 * 3600.0;
/// Okresy do wyboru w oknie: (sekundy, nazwa).
pub const PERIODS: [(u64, &str); 4] = [
    (3600, "1 godzina"),
    (6 * 3600, "6 godzin"),
    (24 * 3600, "24 godziny"),
    (7 * 86400, "7 dni"),
];
/// Limit serwera zgłoszeń (bajty paczki).
pub const MAX_REPORT_BYTES: u64 = 2 * 1024 * 1024;
/// Za duża paczka - kolejne próby z krótszymi końcówkami logów.
pub const TAIL_LIMITS: [u64; 3] = [MAX_LOG_BYTES, 300_000, 80_000];
/// Krótszy opis zgłoszenia nic nie mówi autorowi.
pub const REPORT_MIN_CHARS: usize = 5;
/// Czas na wysłanie zgłoszenia (sekundy).
pub const TIMEOUT_S: u64 = 30;
/// Nazwa pliku logu aplikacji (i jego starszych części), z którego powstaje podsumowanie.
pub const APP_LOG_PREFIX: &str = "bieznia.log";

const MASKED_MAC: &[u8] = b"XX:XX:XX:XX:XX:XX";
const MAX_SUMMARY_LINES: usize = 2000;

/// Linie, które trafiają do podsumowania - wyniki poleceń, połączenia, błędy.
static SUMMARY_LINE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r"Podsumowanie sesji|Start:|Stop:|Prędkość [0-9.]+:|nie przyjęła|nie zareagowała|Połączenie |rozłączyła|Telemetria|Bieżnia \d|WalkLevel \d|Zaktualizowano|WARNING|ERROR|CRITICAL",
    )
    .ok()
});

/// Błąd zgłoszenia. `Display` to gotowy komunikat dla użytkownika.
#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    /// Komunikat dla użytkownika (za duża paczka, odpowiedź serwera, brak internetu).
    #[error("{0}")]
    Message(String),
    /// Błąd dysku przy składaniu albo czytaniu paczki.
    #[error("Nie udało się przygotować paczki: {0}")]
    Io(#[from] io::Error),
}

impl ReportError {
    fn msg(text: impl Into<String>) -> Self {
        Self::Message(text.into())
    }
}

impl From<zip::result::ZipError> for ReportError {
    fn from(err: zip::result::ZipError) -> Self {
        match err {
            zip::result::ZipError::Io(err) => Self::Io(err),
            other => Self::Io(io::Error::other(other)),
        }
    }
}

/// Foldery, z których powstaje paczka, i folder na gotowe paczki.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportFolders {
    /// logi aplikacji (`*.log`, `*.log.1`…) i raporty `diagnostyka-*.txt`
    pub logs: PathBuf,
    /// dane użytkownika - z niego idzie tylko `ustawienia.json`
    pub data: PathBuf,
    /// gdzie zapisać `zgloszenie-RRRRMMDD-GGMMSS.zip` (w 0.5.x: dane/zgloszenia)
    pub out: PathBuf,
}

impl ReportFolders {
    /// Układ jak w 0.5.x: `root/logs`, `root/dane`, `root/dane/zgloszenia`.
    pub fn in_root(root: &Path) -> Self {
        Self {
            logs: root.join("logs"),
            data: root.join("dane"),
            out: root.join("dane").join("zgloszenia"),
        }
    }

    fn settings(&self) -> PathBuf {
        self.data.join("ustawienia.json")
    }
}

/// Dane wpisane w oknie i opis środowiska, które trafiają do `zgloszenie.json`.
#[derive(Debug, Clone, PartialEq)]
pub struct ReportInfo {
    pub description: String,
    pub contact: String,
    /// wersja aplikacji
    pub version: String,
    /// wersja systemu, np. "Windows-11-10.0.26300-SP0" (pole "windows")
    pub system: String,
    /// okres logów w sekundach (zwykle jeden z [`PERIODS`])
    pub period_s: f64,
}

#[derive(Serialize)]
struct InfoJson<'a> {
    opis: &'a str,
    kontakt: &'a str,
    wersja: &'a str,
    windows: &'a str,
    czas: String,
    okres_logow_h: f64,
}

/// Sprawdza opis przed wysłaniem. `Some(komunikat)` - opis za krótki (pokaż przy polu).
pub fn description_problem(description: &str) -> Option<String> {
    let length = py_strip(description).chars().count();
    if length >= REPORT_MIN_CHARS {
        return None;
    }
    if length == 0 {
        return Some("Opisz, co się stało - wystarczy jedno zdanie.".to_string());
    }
    let missing = REPORT_MIN_CHARS - length;
    let word = match missing {
        1 => "znak",
        2..=4 => "znaki",
        _ => "znaków",
    };
    Some(format!(
        "Opis jest za krótki - dopisz jeszcze {missing} {word}, np. co poszło nie tak."
    ))
}

/// Komunikat po udanej wysyłce (numer zgłoszenia z serwera albo pusty).
pub fn sent_message(number: &str) -> String {
    let done = if number.is_empty() {
        "Wysłano zgłoszenie".to_string()
    } else {
        format!("Wysłano zgłoszenie nr {number}")
    };
    format!("{done} - dziękuję! Autor dostał opis i logi.")
}

// --- logi z okresu ----------------------------------------------------------------------

fn now_s() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn mtime_s(path: &Path) -> io::Result<f64> {
    let modified = fs::metadata(path)?.modified()?;
    Ok(match modified.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs_f64(),
        Err(e) => -e.duration().as_secs_f64(),
    })
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Pliki z folderu (bez podfolderów) posortowane po nazwie bez rozróżniania wielkości liter
/// - jak glob i sortowanie ścieżek w Pythonie na Windows.
fn list_files(dir: &Path, matches: impl Fn(&str) -> bool) -> io::Result<Vec<PathBuf>> {
    let mut found: Vec<(String, PathBuf)> = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if entry.file_type()?.is_file() && matches(&name) {
            found.push((name, entry.path()));
        }
    }
    found.sort();
    Ok(found.into_iter().map(|(_, path)| path).collect())
}

fn is_rotated_log(lower_name: &str) -> bool {
    lower_name
        .strip_suffix(|c: char| c.is_ascii_digit())
        .is_some_and(|rest| rest.ends_with(".log."))
}

/// Bieżące pliki logu (`*.log`) i ich starsze części (`*.log.0`-`*.log.9`) zmienione w okresie.
pub fn recent_logs(logs: &Path, now: f64, period_s: f64) -> io::Result<Vec<PathBuf>> {
    if !logs.exists() {
        return Ok(Vec::new());
    }
    let mut files = list_files(logs, |name| name.ends_with(".log"))?;
    for old in list_files(logs, is_rotated_log)? {
        if now - mtime_s(&old)? <= period_s {
            files.push(old);
        }
    }
    Ok(files)
}

fn tail(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let size = file.seek(SeekFrom::End(0))?;
    file.seek(SeekFrom::Start(size.saturating_sub(limit)))?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;
    Ok(data)
}

/// Czas uniksowy z lokalnej daty "RRRR-MM-DD GG:MM:SS" (jak `strptime(...).timestamp()`).
fn local_timestamp(text: &str) -> Option<f64> {
    if text.get(17..19)?.parse::<u32>().ok()? > 59 {
        return None; // Python nie przyjmuje sekundy przestępnej
    }
    let naive = NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S").ok()?;
    if let Some(at) = Local.from_local_datetime(&naive).earliest() {
        return Some(at.timestamp() as f64);
    }
    // czas w luce zmiany na czas letni: przesunięcie sprzed zmiany (jak fold=0 w Pythonie)
    let before = naive - chrono::Duration::hours(3);
    let offset = Local.offset_from_local_datetime(&before).earliest()?;
    Some((naive.and_utc().timestamp() - i64::from(offset.local_minus_utc())) as f64)
}

/// Data na początku linii logu: "RRRR-MM-DD GG:MM:SS".
fn line_date(line: &[u8]) -> Option<&str> {
    let head = line.get(..19)?;
    let pattern = b"dddd-dd-dd dd:dd:dd";
    let ok = head.iter().zip(pattern).all(|(&b, &p)| {
        if p == b'd' {
            b.is_ascii_digit()
        } else {
            b == p
        }
    });
    if ok {
        std::str::from_utf8(head).ok()
    } else {
        None
    }
}

/// Końcówka logu (najwyżej `limit` bajtów) od chwili `since`. Linie bez daty (np. traceback)
/// idą za poprzednią. Stary log bez dat: cały koniec pliku, jeśli plik zmienił się w okresie.
pub fn tail_since(path: &Path, since: f64, limit: u64) -> io::Result<Vec<u8>> {
    let data = tail(path, limit)?;
    let mut dated = false;
    let mut start = 0usize;
    for line in data.split(|&b| b == b'\n') {
        if let Some(text) = line_date(line) {
            dated = true;
            if local_timestamp(text).is_some_and(|at| at >= since) {
                return Ok(data[start..].to_vec());
            }
        }
        start += line.len() + 1;
    }
    if dated {
        return Ok(Vec::new());
    }
    Ok(if mtime_s(path)? >= since {
        data
    } else {
        Vec::new()
    })
}

/// Białe znaki jak `str.isspace()` w Pythonie.
fn py_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

fn py_strip(text: &str) -> &str {
    text.trim_matches(py_space)
}

/// Podział na linie jak `str.splitlines()` w Pythonie.
fn py_splitlines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let breaks = matches!(
            c,
            '\n' | '\r'
                | '\u{0b}'
                | '\u{0c}'
                | '\u{1c}'
                | '\u{1d}'
                | '\u{1e}'
                | '\u{85}'
                | '\u{2028}'
                | '\u{2029}'
        );
        if !breaks {
            continue;
        }
        lines.push(&text[start..i]);
        let mut end = i + c.len_utf8();
        if c == '\r'
            && let Some(&(j, '\n')) = chars.peek()
        {
            chars.next();
            end = j + 1;
        }
        start = end;
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

/// Najważniejsze linie z logów aplikacji (pliki `bieznia.log*`) od chwili `since`
/// - od najstarszych do najnowszych, z licznikami na górze.
pub fn summary_text(logs: &[PathBuf], since: f64) -> io::Result<String> {
    let mut app_logs = Vec::new();
    for path in logs {
        if file_name(path).starts_with(APP_LOG_PREFIX) {
            app_logs.push((mtime_s(path)?, path));
        }
    }
    app_logs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut lines: Vec<String> = Vec::new();
    for (_, path) in app_logs {
        let raw = tail_since(path, since, MAX_LOG_BYTES)?;
        let text = String::from_utf8_lossy(&raw);
        lines.extend(
            py_splitlines(&text)
                .into_iter()
                .filter(|line| SUMMARY_LINE.as_ref().is_some_and(|re| re.is_match(line)))
                .map(str::to_string),
        );
    }
    let count = |test: &dyn Fn(&str) -> bool| lines.iter().filter(|l| test(l)).count();
    let counts = [
        ("polecenia przyjęte", count(&|l| l.contains("przyjęt"))),
        (
            "bez reakcji",
            count(&|l| l.contains("nie przyjęła") || l.contains("nie zareagowała")),
        ),
        ("rozłączenia", count(&|l| l.contains("rozłączyła"))),
        (
            "błędy",
            count(&|l| l.contains(" ERROR ") || l.contains(" CRITICAL ")),
        ),
    ];
    let head = counts
        .iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect::<Vec<_>>()
        .join(" · ");
    let last = &lines[lines.len().saturating_sub(MAX_SUMMARY_LINES)..];
    Ok(format!("{head}\n\n{}\n", last.join("\n")))
}

/// Pliki do paczki: logi z okresu, najnowszy raport diagnostyczny i ustawienia.
pub fn report_files(folders: &ReportFolders, now: f64, period_s: f64) -> io::Result<Vec<PathBuf>> {
    let mut files = recent_logs(&folders.logs, now, period_s)?;
    if folders.logs.exists() {
        let reports = list_files(&folders.logs, |name| {
            name.starts_with("diagnostyka-") && name.ends_with(".txt")
        })?;
        if let Some(last) = reports.last() {
            files.push(last.clone());
        }
    }
    let settings = folders.settings();
    if settings.exists() {
        files.push(settings);
    }
    Ok(files)
}

// --- maskowanie -----------------------------------------------------------------------

fn normalize_mac(value: &str) -> String {
    py_strip(value).to_uppercase().replace('-', ":")
}

/// Prawda dla wartości, które w Pythonie są fałszywe (None, "", 0, False, [], {}).
fn py_falsy(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Bool(b) => !b,
        Value::Number(n) => n.as_f64() == Some(0.0),
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
    }
}

/// `str(value)` z Pythona dla wartości z JSON-a (listy i słowniki w zapisie JSON).
fn py_str(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        Value::String(s) => s.clone(),
        Value::Number(n) if n.is_f64() => n.as_f64().map(py_float).unwrap_or_default(),
        other => other.to_string(),
    }
}

/// `repr(float)` z Pythona dla zwykłych wartości (3.0 -> "3.0", 2.5 -> "2.5").
pub(crate) fn py_float(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 1e16 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

/// Pole `key` słownika JSON jako `str(data.get(key) or "")`.
fn json_field(raw: &[u8], key: &str) -> String {
    let text = String::from_utf8_lossy(raw);
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(map)) => match map.get(key) {
            Some(value) if !py_falsy(value) => py_str(value),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

/// Adres wybranej bieżni z ustawień (`device_address`) - jego nie maskujemy,
/// przydaje się w diagnozie. Brak pliku albo zły JSON - pusty.
pub fn device_address(settings: &Path) -> String {
    match fs::read(settings) {
        Ok(raw) => normalize_mac(&json_field(&raw, "device_address")),
        Err(_) => String::new(),
    }
}

/// Folder profilu Windows (jak `Path.home()` w Pythonie).
pub fn home_dir() -> String {
    if let Ok(home) = std::env::var("USERPROFILE")
        && !home.is_empty()
    {
        return home;
    }
    match (std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH")) {
        (Ok(drive), Ok(path)) => format!("{drive}{path}"),
        _ => std::env::var("HOME").unwrap_or_default(),
    }
}

fn replace_bytes(data: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    if from.is_empty() {
        return data.to_vec();
    }
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if data[i..].starts_with(from) {
            out.extend_from_slice(to);
            i += from.len();
        } else {
            out.push(data[i]);
            i += 1;
        }
    }
    out
}

fn mac_char(b: u8) -> bool {
    b.is_ascii_hexdigit() || b == b':' || b == b'-'
}

/// Adres MAC "AA:BB:CC:DD:EE:FF" (separatory `:` albo `-`) od pozycji `i`, bez znaków
/// adresu tuż przed i tuż po (jak lookbehind/lookahead w Pythonie).
fn mac_at(data: &[u8], i: usize) -> bool {
    let Some(candidate) = data.get(i..i + 17) else {
        return false;
    };
    if i > 0 && mac_char(data[i - 1]) {
        return false;
    }
    if data.get(i + 17).is_some_and(|&b| mac_char(b)) {
        return false;
    }
    candidate.iter().enumerate().all(|(k, &b)| {
        if k % 3 == 2 {
            b == b':' || b == b'-'
        } else {
            b.is_ascii_hexdigit()
        }
    })
}

/// Bez ścieżki profilu Windows (nazwy użytkownika) i adresów cudzych urządzeń Bluetooth.
/// `keep_mac` - adres wybranej bieżni, który zostaje; `home` - folder profilu ([`home_dir`]).
pub fn scrub(data: &[u8], keep_mac: &str, home: &str) -> Vec<u8> {
    let mut out = data.to_vec();
    let variants = [
        home.replace('\\', "\\\\"),
        home.to_string(),
        home.replace('\\', "/"),
    ];
    let mut done: Vec<&str> = Vec::new();
    for variant in &variants {
        if variant.chars().count() > 3 && !done.contains(&variant.as_str()) {
            out = replace_bytes(&out, variant.as_bytes(), b"%USERPROFILE%");
            done.push(variant);
        }
    }
    let keep = normalize_mac(keep_mac);
    let mut masked = Vec::with_capacity(out.len());
    let mut i = 0;
    while i < out.len() {
        if mac_at(&out, i) {
            let found = &out[i..i + 17];
            let same = !keep.is_empty() && normalize_mac(&String::from_utf8_lossy(found)) == keep;
            masked.extend_from_slice(if same { found } else { MASKED_MAC });
            i += 17;
        } else {
            masked.push(out[i]);
            i += 1;
        }
    }
    masked
}

// --- paczka ZIP -------------------------------------------------------------------------

/// Zaokrąglenie do 1 miejsca jak `round(x, 1)` w Pythonie.
fn round1(value: f64) -> f64 {
    format!("{value:.1}").parse().unwrap_or(value)
}

fn zip_time() -> zip::DateTime {
    let now = Local::now();
    let year = u16::try_from(now.year()).unwrap_or(1980);
    zip::DateTime::from_date_and_time(
        year,
        now.month() as u8,
        now.day() as u8,
        now.hour() as u8,
        now.minute() as u8,
        now.second() as u8,
    )
    .unwrap_or_default()
}

/// Nazwa pliku w paczce: "logs/…" dla logów i raportów, "dane/ustawienia.json".
fn archive_name(folders: &ReportFolders, path: &Path) -> String {
    let folder = if path.parent() == Some(folders.data.as_path()) {
        "dane"
    } else {
        "logs"
    };
    format!("{folder}/{}", file_name(path))
}

/// Tworzy `zgloszenie-RRRRMMDD-GGMMSS.zip` w `folders.out` i zwraca jego ścieżkę. Logi bez
/// ścieżki profilu i cudzych adresów Bluetooth; za duża paczka - krótsze końcówki logów.
pub fn build_report(folders: &ReportFolders, info: &ReportInfo) -> Result<PathBuf, ReportError> {
    build_report_at(folders, info, now_s(), MAX_REPORT_BYTES, &home_dir())
}

fn build_report_at(
    folders: &ReportFolders,
    info: &ReportInfo,
    now: f64,
    max_bytes: u64,
    home: &str,
) -> Result<PathBuf, ReportError> {
    let stamp = Local::now();
    fs::create_dir_all(&folders.out)?;
    let path = folders
        .out
        .join(format!("zgloszenie-{}.zip", stamp.format("%Y%m%d-%H%M%S")));
    let json = InfoJson {
        opis: py_strip(&info.description),
        kontakt: py_strip(&info.contact),
        wersja: &info.version,
        windows: &info.system,
        czas: stamp.format("%Y-%m-%dT%H:%M:%S").to_string(),
        okres_logow_h: round1(info.period_s / 3600.0),
    };
    let info_json = serde_json::to_string_pretty(&json).map_err(io::Error::other)?;
    let keep = device_address(&folders.settings());
    let files = report_files(folders, now, info.period_s)?;
    let logs: Vec<PathBuf> = files
        .iter()
        .filter(|f| file_name(f).contains(".log"))
        .cloned()
        .collect();
    let since = now - info.period_s;
    let summary = if logs.is_empty() {
        None
    } else {
        Some(scrub(summary_text(&logs, since)?.as_bytes(), &keep, home))
    };
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip_time());
    for limit in TAIL_LIMITS {
        let mut zip = zip::ZipWriter::new(File::create(&path)?);
        zip.start_file("zgloszenie.json", options)?;
        zip.write_all(info_json.as_bytes())?;
        if let Some(summary) = &summary {
            zip.start_file("podsumowanie.txt", options)?;
            zip.write_all(summary)?;
        }
        for file in &files {
            let data = if file_name(file).contains(".log") {
                tail_since(file, since, limit)?
            } else {
                tail(file, limit)?
            };
            zip.start_file(archive_name(folders, file), options)?;
            zip.write_all(&scrub(&data, &keep, home))?;
        }
        zip.finish()?;
        if fs::metadata(&path)?.len() <= max_bytes {
            return Ok(path);
        }
    }
    fs::remove_file(&path)?;
    Err(ReportError::msg(
        "Paczka z logami jest za duża - usuń stare logi i spróbuj ponownie.",
    ))
}

// --- wysyłka ----------------------------------------------------------------------------

/// Pola formularza w kolejności jak w 0.5.x: wersja, opis, kontakt, (okres).
fn form_fields(
    description: &str,
    contact: &str,
    period_s: Option<f64>,
    version: &str,
) -> Vec<(&'static str, String)> {
    let mut fields = vec![
        ("wersja", version.to_string()),
        ("opis", py_strip(description).chars().take(4000).collect()),
        ("kontakt", py_strip(contact).chars().take(200).collect()),
    ];
    if let Some(period) = period_s.filter(|p| *p > 0.0) {
        let hours = (period / 3600.0).round_ties_even().max(1.0);
        fields.push(("okres", format!("{hours:.0}")));
    }
    fields
}

/// Treść multipart/form-data: pola tekstowe i plik "paczka" (application/zip).
fn multipart_body(
    fields: &[(&str, String)],
    file_name: &str,
    file: &[u8],
    boundary: &str,
) -> (Vec<u8>, String) {
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"paczka\"; filename=\"{file_name}\"\r\nContent-Type: application/zip\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(file);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (body, format!("multipart/form-data; boundary={boundary}"))
}

/// Komunikat dla odpowiedzi serwera z błędem (kod HTTP i treść odpowiedzi).
fn http_error(status: u16, body: &[u8]) -> ReportError {
    let message = json_field(body, "message");
    let or = |default: String| {
        if message.is_empty() {
            default
        } else {
            message.clone()
        }
    };
    ReportError::Message(match status {
        413 => "Paczka jest za duża dla serwera - zapisz ją na dysk.".to_string(),
        429 => or("Za dużo zgłoszeń naraz - spróbuj za godzinę albo zapisz paczkę.".to_string()),
        503 => format!(
            "{} Zapisz paczkę na dysk i wyślij ją e-mailem na {CONTACT}.",
            or("Serwer nie wysłał teraz maila.".to_string())
        ),
        _ => or(format!("Serwer odrzucił zgłoszenie (kod {status}).")),
    })
}

fn is_https(endpoint: &str) -> bool {
    endpoint
        .split_once(':')
        .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("https"))
}

fn network_error() -> ReportError {
    ReportError::msg("Nie udało się wysłać - sprawdź internet albo zapisz paczkę.")
}

fn post(
    endpoint: &str,
    body: Vec<u8>,
    content_type: &str,
    version: &str,
) -> Result<String, ReportError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_S))
        .build()
        .map_err(|_| network_error())?;
    let response = client
        .post(endpoint)
        .header(reqwest::header::CONTENT_TYPE, content_type)
        .header(reqwest::header::USER_AGENT, format!("Bieznia/{version}"))
        .body(body)
        .send()
        .map_err(|_| network_error())?;
    let status = response.status();
    let raw = response.bytes().map_err(|_| network_error())?;
    if !status.is_success() {
        return Err(http_error(status.as_u16(), &raw));
    }
    Ok(json_field(&raw, "id"))
}

/// Wysyła paczkę metodą POST (multipart) na serwer zgłoszeń (`[zgloszenia] adres` z config).
/// Zwraca numer zgłoszenia (pusty, gdy serwer go nie podał).
///
/// Serwer przekazuje opis i paczkę mailem do autora i ich nie przechowuje. `period_s`: okres
/// logów - idzie osobnym polem "okres" (godziny), bo serwer nie rozpakowuje paczki.
/// Wywołanie blokujące (do 30 s) - w aplikacji uruchamiać poza wątkiem interfejsu.
pub fn send_report(
    report: &Path,
    endpoint: &str,
    description: &str,
    contact: &str,
    period_s: Option<f64>,
    version: &str,
) -> Result<String, ReportError> {
    let fields = form_fields(description, contact, period_s, version);
    if !is_https(endpoint) {
        return Err(ReportError::msg(
            "Adres zgłoszeń musi zaczynać się od https://",
        ));
    }
    let file = fs::read(report)?;
    let boundary = uuid::Uuid::new_v4().simple().to_string();
    let (body, content_type) = multipart_body(&fields, &file_name(report), &file, &boundary);
    post(endpoint, body, &content_type, version)
}

// --- e-mail i folder ----------------------------------------------------------------------

/// Kodowanie jak `urllib.parse.quote(text, safe="")`.
fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for &b in text.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Link `mailto:` do autora z tematem i treścią (opis + nazwa paczki do dołączenia).
/// Otwiera go aplikacja (np. opener w Tauri).
pub fn mailto_link(description: &str, report_name: &str, version: &str) -> String {
    let subject = format!("{APP_NAME} {version} - zgłoszenie problemu");
    let body = format!(
        "{}\n\nPaczka z logami do dołączenia: {report_name}\n(folder otworzył się obok - przeciągnij plik do tej wiadomości)\n",
        py_strip(description)
    );
    format!(
        "mailto:{CONTACT}?subject={}&body={}",
        percent_encode(&subject),
        percent_encode(&body)
    )
}

/// Pokazuje paczkę w Eksploratorze (zapis na dysk zamiast wysyłki). Poza Windows nic nie robi.
pub fn show_in_folder(report: &Path) -> io::Result<()> {
    if cfg!(windows) {
        let full = std::path::absolute(report)?;
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", full.display()))
            .spawn()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
