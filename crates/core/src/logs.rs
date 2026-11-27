//! Logi w plikach - port bieznia/logs.py.
//!
//! - logs/bieznia.log (+ konsola): poziom INFO i wyżej ze wszystkich modułów, DEBUG spoza
//!   Bluetooth przy poziomie pliku DEBUG ([`set_file_level`], `[logs] level` w config.toml),
//! - logs/bluetooth.log: ruch Bluetooth (cel [`BLE_TARGET`] albo moduł `treadmill`); poziom DEBUG
//!   tylko przy włączonych szczegółowych logach ([`set_detailed`]), inaczej INFO,
//! - btleplug sam w sobie gada dużo - z niego tylko ostrzeżenia i błędy,
//! - panika (także w wątku) trafia do logu jako CRITICAL - exe nie ma konsoli.
//!
//! Pliki są zgodne z 0.5.x: ten sam format linii, rotacja co 2 MB z 3 starszymi częściami
//! (bieznia.log.1 ... .3), końce linii CRLF na Windows. Zgłoszenia czytają je po dacie z linii.
//!
//! Ważne zdarzenia zaczynają wiadomość stałym identyfikatorem w nawiasie (`[app-start] ...`):
//! podsumowanie zgłoszenia wybiera linie po nim i po poziomie, nie po treści zdania.

use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use log::{Level, LevelFilter, Log, Metadata, Record};
use regex::Regex;

/// Główny plik logu w logs/.
pub const MAIN_FILE: &str = "bieznia.log";
/// Plik z rozmową z bieżnią w logs/.
pub const BLE_FILE: &str = "bluetooth.log";
/// Rozmiar, po którym plik jest obracany (jak `maxBytes` w Pythonie).
pub const MAX_BYTES: u64 = 2_000_000;
/// Ile starszych części zostaje (bieznia.log.1 ... .3).
pub const BACKUP_COUNT: u32 = 3;
/// After a failed rotation (file held by another program) the next try waits this long.
pub const ROTATE_RETRY: Duration = Duration::from_secs(60);
/// Data w pliku - zgłoszenie bierze dokładnie wybrany okres.
pub const FILE_DATEFMT: &str = "%Y-%m-%d %H:%M:%S";
/// Data w konsoli i w podglądzie logu w oknie.
pub const CONSOLE_DATEFMT: &str = "%H:%M:%S";
/// Cel logów Bluetooth: `log::debug!(target: BLE_TARGET, ...)`.
pub const BLE_TARGET: &str = "bieznia.ble";
/// Cel wpisów o nieobsłużonych błędach (panika).
pub const CRASH_TARGET: &str = "bieznia.blad";

/// Level of bieznia.log unless config.toml says otherwise.
pub const DEFAULT_FILE_LEVEL: LevelFilter = LevelFilter::Info;
/// Event ids written by the app at start and exit (see [`previous_run_closed`]).
pub const APP_START_EVENT: &str = "[app-start]";
pub const APP_EXIT_EVENT: &str = "[app-exit]";

/// Moduł rdzenia, którego logi też trafiają do bluetooth.log.
const BLE_MODULE: &str = "walklevel_core::treadmill";
/// Biblioteki, z których logujemy tylko ostrzeżenia (jak `bleak` w Pythonie).
const QUIET_TARGETS: &[&str] = &["btleplug"];
#[cfg(windows)]
const NEWLINE: &str = "\r\n";
#[cfg(not(windows))]
const NEWLINE: &str = "\n";

/// Błędy przygotowania logów.
#[derive(Debug, thiserror::Error)]
pub enum LogError {
    #[error("Nie da się otworzyć logu {path}: {source}")]
    Open { path: PathBuf, source: io::Error },
    #[error("Inny logger jest już ustawiony w tym procesie")]
    OtherLogger,
}

/// Odbiorca wpisów INFO i wyżej w postaci "HH:MM:SS  wiadomość" (podgląd logu w oknie).
pub type Listener = Box<dyn Fn(&str) + Send + Sync>;

/// Plik obracany po przekroczeniu rozmiaru - zachowanie `RotatingFileHandler` z Pythona.
#[derive(Debug)]
pub struct RotatingFile {
    path: PathBuf,
    max_bytes: u64,
    backup_count: u32,
    file: Option<File>,
    pos: u64,
    /// no rotation before this moment (the last one failed)
    retry_at: Option<Instant>,
}

impl RotatingFile {
    /// Otwiera (albo tworzy) plik do dopisywania.
    pub fn open(path: impl Into<PathBuf>, max_bytes: u64, backup_count: u32) -> io::Result<Self> {
        let mut rotating = Self {
            path: path.into(),
            max_bytes,
            backup_count,
            file: None,
            pos: 0,
            retry_at: None,
        };
        rotating.reopen()?;
        Ok(rotating)
    }

    /// Ścieżka bieżącego pliku.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Dopisuje jeden wpis (z końcem linii), wcześniej obraca plik, jeśli wpis by go przepełnił.
    pub fn write_record(&mut self, text: &str) -> io::Result<()> {
        // jak w Pythonie: pozycja w bajtach + długość wpisu w znakach z jednym "\n"
        let len = text.chars().count() as u64 + 1;
        let waiting = self.retry_at.is_some_and(|at| Instant::now() < at);
        if self.max_bytes > 0 && self.pos > 0 && self.pos + len >= self.max_bytes && !waiting {
            // nieudane obrócenie (np. plik trzyma inny proces) nie gubi wpisu: piszemy dalej
            self.retry_at = self
                .rollover()
                .is_err()
                .then(|| Instant::now() + ROTATE_RETRY);
        }
        if self.file.is_none() {
            self.reopen()?;
        }
        let mut data = text.replace('\n', NEWLINE);
        data.push_str(NEWLINE);
        let Some(file) = self.file.as_mut() else {
            return Ok(());
        };
        file.write_all(data.as_bytes())?;
        self.pos += data.len() as u64;
        Ok(())
    }

    fn reopen(&mut self) -> io::Result<()> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        self.pos = file.metadata()?.len();
        self.file = Some(file);
        Ok(())
    }

    fn rollover(&mut self) -> io::Result<()> {
        self.file = None;
        if self.backup_count > 0 {
            // the current file first: when another program holds it, older parts stay
            let moving = self.part(0);
            if let Err(e) = std::fs::rename(&self.path, &moving) {
                self.reopen()?;
                return Err(e);
            }
            for i in (1..self.backup_count).rev() {
                let src = self.part(i);
                let dst = self.part(i + 1);
                if src.exists() {
                    if dst.exists() {
                        std::fs::remove_file(&dst)?;
                    }
                    std::fs::rename(&src, &dst)?;
                }
            }
            let first = self.part(1);
            if first.exists() {
                std::fs::remove_file(&first)?;
            }
            std::fs::rename(&moving, &first)?;
        }
        self.reopen()
    }

    fn part(&self, index: u32) -> PathBuf {
        let mut name = OsString::from(self.path.as_os_str());
        name.push(format!(".{index}"));
        PathBuf::from(name)
    }
}

/// Logger rdzenia: dwa pliki, opcjonalnie konsola i podgląd w oknie.
pub struct Logger {
    main: Mutex<RotatingFile>,
    ble: Mutex<RotatingFile>,
    console: bool,
    detailed: AtomicBool,
    file_level: AtomicU8,
    listener: RwLock<Option<Listener>>,
}

fn level_code(level: LevelFilter) -> u8 {
    level as u8
}

fn level_from_code(code: u8) -> LevelFilter {
    LevelFilter::iter()
        .find(|l| *l as u8 == code)
        .unwrap_or(DEFAULT_FILE_LEVEL)
}

/// `info` or `debug` - level of bieznia.log from config.toml.
pub fn parse_level(text: &str) -> Option<LevelFilter> {
    match text.trim().to_ascii_lowercase().as_str() {
        "info" => Some(LevelFilter::Info),
        "debug" => Some(LevelFilter::Debug),
        _ => None,
    }
}

impl Logger {
    /// Otwiera logs/bieznia.log i logs/bluetooth.log w `log_dir` (tworzy folder).
    pub fn new(log_dir: &Path, console: bool, detailed: bool) -> Result<Self, LogError> {
        let open = |name: &str| {
            let path = log_dir.join(name);
            std::fs::create_dir_all(log_dir)
                .and_then(|_| RotatingFile::open(&path, MAX_BYTES, BACKUP_COUNT))
                .map_err(|source| LogError::Open { path, source })
        };
        Ok(Self {
            main: Mutex::new(open(MAIN_FILE)?),
            ble: Mutex::new(open(BLE_FILE)?),
            console,
            detailed: AtomicBool::new(detailed),
            file_level: AtomicU8::new(level_code(DEFAULT_FILE_LEVEL)),
            listener: RwLock::new(None),
        })
    }

    /// Szczegółowe logi = surowa rozmowa z bieżnią (DEBUG) w bluetooth.log.
    pub fn set_detailed(&self, enabled: bool) {
        self.detailed.store(enabled, Ordering::Relaxed);
    }

    /// Level of bieznia.log for entries outside Bluetooth. Errors, warnings and INFO always
    /// reach the file; Bluetooth DEBUG stays in bluetooth.log.
    pub fn set_file_level(&self, level: LevelFilter) {
        let level = level.max(LevelFilter::Info).min(LevelFilter::Debug);
        self.file_level.store(level_code(level), Ordering::Relaxed);
    }

    fn file_level(&self) -> LevelFilter {
        level_from_code(self.file_level.load(Ordering::Relaxed))
    }

    /// Ustawia (albo usuwa) odbiorcę wpisów do podglądu w oknie. Odbiorca nie może logować.
    pub fn set_listener(&self, listener: Option<Listener>) {
        *self.listener.write().unwrap_or_else(|e| e.into_inner()) = listener;
    }

    fn accepts(&self, level: Level, target: &str) -> bool {
        if QUIET_TARGETS.iter().any(|t| has_prefix(target, t)) {
            return level <= Level::Warn;
        }
        if is_ble(target) && self.detailed.load(Ordering::Relaxed) {
            return level <= Level::Debug;
        }
        level <= Level::Info || (!is_ble(target) && level <= self.file_level())
    }

    /// `main` - the entry goes to bieznia.log (and the console), `preview` - also to the
    /// log preview in the window.
    fn emit(&self, ns: i128, level: &str, main: bool, preview: bool, target: &str, msg: &str) {
        let (secs, msecs) = stamp(ns);
        let time = local_time(secs);
        if main {
            let _ = lock(&self.main).write_record(&main_line(&time, level, target, msg));
            if self.console {
                let _ = writeln!(io::stderr(), "{}", console_line(&time, level, target, msg));
            }
        }
        if preview
            && let Some(listener) = self
                .listener
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
        {
            let text = without_event(msg);
            listener(&format!("{}  {text}", time.format(CONSOLE_DATEFMT)));
        }
        if is_ble(target) {
            let _ = lock(&self.ble).write_record(&ble_line(&time, msecs, msg));
        }
    }

    /// Wpis CRITICAL o nieobsłużonym błędzie (z haka paniki).
    fn critical(&self, msg: &str) {
        self.emit(now_ns(), "CRITICAL", true, true, CRASH_TARGET, msg);
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.accepts(metadata.level(), metadata.target())
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let msg = record.args().to_string();
        let level = record.level();
        let info = level <= Level::Info;
        // Bluetooth DEBUG (frames, commands) only in bluetooth.log
        let main = info || !is_ble(record.target());
        self.emit(
            now_ns(),
            level_name(level),
            main,
            info,
            record.target(),
            &msg,
        );
    }

    fn flush(&self) {}
}

/// Logger procesu, przestawiany przez [`init`] (jak `force=True` w Pythonie).
static ACTIVE: RwLock<Option<Arc<Logger>>> = RwLock::new(None);
static INSTALLED: AtomicBool = AtomicBool::new(false);
static PANIC_HOOK: AtomicBool = AtomicBool::new(false);

struct Global;

static GLOBAL: Global = Global;

impl Log for Global {
    fn enabled(&self, metadata: &Metadata) -> bool {
        active().is_some_and(|l| l.enabled(metadata))
    }

    fn log(&self, record: &Record) {
        if let Some(logger) = active() {
            logger.log(record);
        }
    }

    fn flush(&self) {}
}

fn active() -> Option<Arc<Logger>> {
    ACTIVE.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Ustawia logi procesu w `data_dir`/logs (odpowiednik `setup_logging("bieznia")`).
/// Można wołać ponownie - kolejne wywołanie podmienia pliki i ustawienia.
/// `console` - wpisy także na stderr, `detailed` - DEBUG Bluetooth w bluetooth.log.
pub fn init(data_dir: &Path, console: bool, detailed: bool) -> Result<(), LogError> {
    let logger = Arc::new(Logger::new(&data_dir.join("logs"), console, detailed)?);
    *ACTIVE.write().unwrap_or_else(|e| e.into_inner()) = Some(logger);
    if !INSTALLED.load(Ordering::SeqCst) {
        log::set_logger(&GLOBAL).map_err(|_| LogError::OtherLogger)?;
        INSTALLED.store(true, Ordering::SeqCst);
    }
    log::set_max_level(LevelFilter::Debug);
    install_panic_hook();
    Ok(())
}

/// Włącza/wyłącza szczegółowe logi Bluetooth w loggerze procesu.
pub fn set_detailed(enabled: bool) {
    if let Some(logger) = active() {
        logger.set_detailed(enabled);
    }
}

/// Odbiorca wpisów do podglądu logu w oknie (None usuwa).
pub fn set_listener(listener: Option<Listener>) {
    if let Some(logger) = active() {
        logger.set_listener(listener);
    }
}

/// Level of bieznia.log in the process logger ([`Logger::set_file_level`]).
pub fn set_file_level(level: LevelFilter) {
    if let Some(logger) = active() {
        logger.set_file_level(level);
    }
}

/// How much of the end of bieznia.log is read to find the previous start and exit.
const PREVIOUS_RUN_TAIL: u64 = 256 * 1024;

/// Whether the previous run wrote its exit entry: `Some(false)` - the last
/// [`APP_START_EVENT`] has no [`APP_EXIT_EVENT`] after it (crash, killed process, power
/// cut). None - no start entry in the log (first run or a version before these entries).
/// Called before the new start entry is written.
pub fn previous_run_closed(log_dir: &Path) -> Option<bool> {
    let mut file = File::open(log_dir.join(MAIN_FILE)).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(PREVIOUS_RUN_TAIL)))
        .ok()?;
    let mut raw = Vec::new();
    file.read_to_end(&mut raw).ok()?;
    let text = String::from_utf8_lossy(&raw);
    let start = text.rfind(APP_START_EVENT)?;
    Some(text.rfind(APP_EXIT_EVENT).is_some_and(|exit| exit > start))
}

static EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}")
        .expect("e-mail pattern")
});
static URL_QUERY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(https?://[^\s?#)"'<>]*)\?[^\s)"'<>]*"#).expect("query pattern")
});
static CURSOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\b(kursor|cursor)=[^&\s)"'<>]+"#).expect("cursor pattern"));
static ID_ROUTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(/api/bieznia/(?:profil|znajomi|zaproszenie|sesje)/)[^/\s?#)"'<>]+"#)
        .expect("route pattern")
});
static USER_PATH: LazyLock<Regex> = LazyLock::new(|| {
    // a name with spaces only up to the next separator, otherwise up to the first space
    Regex::new(
        r#"(?i)\b[A-Z]:(?:\\{1,2}|/)Users(?:\\{1,2}|/)(?:[^\\/\r\n"':;<>|*?]+?(\\{1,2}|/)|[^\\/\s"':;<>|*?]+)"#,
    )
    .expect("user path pattern")
});

/// Text without personal data before it goes to a log: e-mail addresses, query strings of
/// addresses (sync cursor), account ids in API routes and the Windows user folder.
pub fn redact(text: &str) -> String {
    let text = EMAIL.replace_all(text, "<e-mail>");
    let text = URL_QUERY.replace_all(&text, "$1?…");
    let text = CURSOR.replace_all(&text, "$1=…");
    let text = ID_ROUTE.replace_all(&text, "$1:id");
    USER_PATH.replace_all(&text, "%USERPROFILE%$1").into_owned()
}

/// Target of entries sent by the interface.
pub const UI_TARGET: &str = "ui";
/// Longest message from the interface kept in the log (characters).
const UI_MESSAGE_MAX: usize = 600;

/// One entry from the interface: `kind` - `error` (window.onerror), `rejection` (unhandled
/// promise), `command` (a command failed without a command error, e.g. refused by the
/// permissions of the window), `command-error` (a command error nobody handled). Without
/// personal data, on one line, at most 20 entries a minute.
pub fn ui_error(window: &str, kind: &str, message: &str) {
    static THROTTLE: Mutex<Throttle> = Mutex::new(Throttle::new(20, Duration::from_secs(60)));
    let Some(dropped) = THROTTLE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .allow(Instant::now())
    else {
        return;
    };
    if dropped > 0 {
        log::warn!(target: UI_TARGET, "[ui-error] Pominięto {dropped} kolejnych błędów interfejsu");
    }
    let (level, kind) = match kind {
        "command-error" => (Level::Warn, kind),
        "rejection" | "command" => (Level::Error, kind),
        _ => (Level::Error, "error"),
    };
    let window: String = window
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(20)
        .collect();
    let one_line = message.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut text = redact(&one_line);
    if text.chars().count() > UI_MESSAGE_MAX {
        text = text.chars().take(UI_MESSAGE_MAX).collect::<String>() + "…";
    }
    log::log!(target: UI_TARGET, level, "[ui-error] Interfejs ({window}, {kind}): {text}");
}

/// At most `max` entries per `window`; the rest is counted and reported with the first entry
/// of the next window.
#[derive(Debug, Clone)]
pub struct Throttle {
    max: u32,
    window: Duration,
    started: Option<Instant>,
    used: u32,
    suppressed: u32,
}

impl Throttle {
    pub const fn new(max: u32, window: Duration) -> Self {
        Self {
            max,
            window,
            started: None,
            used: 0,
            suppressed: 0,
        }
    }

    /// `Some(n)` - log this entry, `n` entries were dropped before it; None - drop it.
    pub fn allow(&mut self, now: Instant) -> Option<u32> {
        let mut dropped = 0;
        if self
            .started
            .is_none_or(|at| now.saturating_duration_since(at) >= self.window)
        {
            dropped = std::mem::take(&mut self.suppressed);
            self.started = Some(now);
            self.used = 0;
        }
        if self.used < self.max {
            self.used += 1;
            Some(dropped)
        } else {
            self.suppressed += 1;
            None
        }
    }
}

/// Panika w dowolnym wątku trafia do logu jako CRITICAL; potem działa poprzedni hak.
pub fn install_panic_hook() {
    if PANIC_HOOK.swap(true, Ordering::SeqCst) {
        return;
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(logger) = active() {
            let payload = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_default();
            let place = info
                .location()
                .map(|l| format!(" ({l})"))
                .unwrap_or_default();
            // jak w Pythonie: główny wątek bez nazwy, inne z nazwą albo "?"
            let head = match std::thread::current().name() {
                Some("main") => "Nieobsłużony błąd".to_string(),
                name => format!("Nieobsłużony błąd w wątku {}", name.unwrap_or("?")),
            };
            let trace = std::backtrace::Backtrace::force_capture();
            logger.critical(format!("{head}\n{payload}{place}\n{trace}").trim_end());
        }
        previous(info);
    }));
}

fn lock(file: &Mutex<RotatingFile>) -> MutexGuard<'_, RotatingFile> {
    file.lock().unwrap_or_else(|e| e.into_inner())
}

/// The message without its event id - the log preview in the window is for people.
fn without_event(msg: &str) -> &str {
    msg.strip_prefix('[')
        .and_then(|rest| rest.split_once("] "))
        .filter(|(event, _)| {
            event.contains('-')
                && event
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        })
        .map_or(msg, |(_, text)| text)
}

fn has_prefix(target: &str, prefix: &str) -> bool {
    target == prefix
        || target
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('.') || rest.starts_with("::"))
}

fn is_ble(target: &str) -> bool {
    has_prefix(target, BLE_TARGET) || has_prefix(target, BLE_MODULE)
}

/// Nazwy poziomów jak w Pythonie (`%(levelname)s`).
fn level_name(level: Level) -> &'static str {
    match level {
        Level::Error => "ERROR",
        Level::Warn => "WARNING",
        Level::Info => "INFO",
        Level::Debug | Level::Trace => "DEBUG",
    }
}

fn now_ns() -> i128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as i128)
        .unwrap_or(0)
}

/// Sekundy i milisekundy wpisu liczone jak w `LogRecord` (Python 3.13).
fn stamp(ns: i128) -> (i64, u32) {
    let created = ns as f64 / 1e9;
    let secs = created.floor() as i64;
    let mut msecs = (ns.rem_euclid(1_000_000_000) / 1_000_000) as u32;
    if msecs == 999 && i128::from(secs) != ns.div_euclid(1_000_000_000) {
        msecs = 0;
    }
    (secs, msecs)
}

fn local_time(secs: i64) -> NaiveDateTime {
    match Local.timestamp_opt(secs, 0).earliest() {
        Some(time) => time.naive_local(),
        None => utc_time(secs),
    }
}

fn utc_time(secs: i64) -> NaiveDateTime {
    DateTime::<Utc>::from_timestamp(secs, 0)
        .unwrap_or_default()
        .naive_utc()
}

/// "%(asctime)s %(levelname)-7s %(name)s: %(message)s" z datą [`FILE_DATEFMT`].
fn main_line(time: &NaiveDateTime, level: &str, name: &str, msg: &str) -> String {
    format!("{} {level:<7} {name}: {msg}", time.format(FILE_DATEFMT))
}

/// Ten sam format w konsoli, data [`CONSOLE_DATEFMT`].
fn console_line(time: &NaiveDateTime, level: &str, name: &str, msg: &str) -> String {
    format!("{} {level:<7} {name}: {msg}", time.format(CONSOLE_DATEFMT))
}

/// "%(asctime)s.%(msecs)03d %(message)s" - bluetooth.log.
fn ble_line(time: &NaiveDateTime, msecs: u32, msg: &str) -> String {
    format!("{}.{msecs:03} {msg}", time.format(FILE_DATEFMT))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::fs;

    fn fixture(name: &str) -> Value {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/base")
            .join(name);
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn lines_match_python() {
        for case in fixture("log-lines.json").as_array().unwrap() {
            let ns = i128::from(case["ns"].as_u64().unwrap());
            let (secs, msecs) = stamp(ns);
            let time = utc_time(secs);
            let level = case["level"].as_str().unwrap();
            let name = case["name"].as_str().unwrap();
            let msg = case["msg"].as_str().unwrap();
            assert_eq!(
                main_line(&time, level, name, msg),
                case["main"].as_str().unwrap()
            );
            assert_eq!(ble_line(&time, msecs, msg), case["ble"].as_str().unwrap());
            assert_eq!(
                console_line(&time, level, name, msg),
                case["console"].as_str().unwrap()
            );
        }
    }

    #[test]
    fn level_names_like_python() {
        assert_eq!(level_name(Level::Warn), "WARNING");
        assert_eq!(level_name(Level::Error), "ERROR");
        assert_eq!(level_name(Level::Info), "INFO");
        assert_eq!(level_name(Level::Debug), "DEBUG");
    }

    #[cfg(windows)]
    #[test]
    fn rotation_matches_python() {
        let data = fixture("rotation.json");
        let messages: Vec<&str> = data["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m.as_str().unwrap())
            .collect();
        for case in data["cases"].as_array().unwrap() {
            let tmp = tempfile::tempdir().unwrap();
            let max_bytes = case["max_bytes"].as_u64().unwrap();
            let backups = case["backup_count"].as_u64().unwrap() as u32;
            let mut file =
                RotatingFile::open(tmp.path().join("test.log"), max_bytes, backups).unwrap();
            for msg in &messages {
                file.write_record(msg).unwrap();
            }
            drop(file);
            let expected = case["files"].as_object().unwrap();
            let mut found = 0;
            for entry in fs::read_dir(tmp.path()).unwrap() {
                let entry = entry.unwrap();
                let name = entry.file_name().into_string().unwrap();
                let text = fs::read_to_string(entry.path()).unwrap();
                assert_eq!(
                    Some(text.as_str()),
                    expected[&name].as_str(),
                    "{max_bytes} {name}"
                );
                found += 1;
            }
            assert_eq!(found, expected.len());
        }
    }

    #[test]
    fn rotation_appends_to_existing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.log");
        fs::write(&path, "x".repeat(95)).unwrap();
        let mut file = RotatingFile::open(&path, 100, 3).unwrap();
        file.write_record("abc").unwrap(); // 95 + 4 < 100: zostaje
        file.write_record("d").unwrap(); // pozycja >= 98: obrót
        assert!(
            fs::read_to_string(file.part(1))
                .unwrap()
                .ends_with(&format!("abc{NEWLINE}"))
        );
        assert_eq!(fs::read_to_string(&path).unwrap().trim_end(), "d");
    }

    #[cfg(windows)]
    #[test]
    fn failed_rotation_keeps_older_parts_and_waits() {
        use std::os::windows::fs::OpenOptionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.log");
        let mut file = RotatingFile::open(&path, 100, 3).unwrap();
        for i in 1..=3 {
            fs::write(file.part(i), format!("part {i}")).unwrap();
        }
        fs::write(&path, "x".repeat(95)).unwrap();
        file.reopen().unwrap();
        // another program keeps the log open without letting it be renamed
        const SHARE_READ_WRITE: u32 = 0x1 | 0x2;
        let holder = OpenOptions::new()
            .read(true)
            .share_mode(SHARE_READ_WRITE)
            .open(&path)
            .unwrap();
        file.write_record("abc").unwrap();
        file.write_record("def").unwrap();
        for i in 1..=3 {
            assert_eq!(
                fs::read_to_string(file.part(i)).unwrap(),
                format!("part {i}")
            );
        }
        assert!(file.retry_at.is_some());
        drop(holder);
        assert!(
            fs::read_to_string(&path)
                .unwrap()
                .ends_with(&format!("def{NEWLINE}"))
        );
    }

    fn record(logger: &Logger, level: Level, target: &str, msg: &str) {
        logger.log(
            &Record::builder()
                .level(level)
                .target(target)
                .args(format_args!("{msg}"))
                .build(),
        );
    }

    fn read(dir: &Path, name: &str) -> String {
        fs::read_to_string(dir.join(name)).unwrap()
    }

    #[test]
    fn levels_and_bluetooth_filter() {
        let tmp = tempfile::tempdir().unwrap();
        let logger = Logger::new(tmp.path(), false, true).unwrap();
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = seen.clone();
        logger.set_listener(Some(Box::new(move |line| {
            sink.lock().unwrap().push(line.to_string())
        })));

        record(
            &logger,
            Level::Info,
            "walklevel_core::history",
            "zapis sesji",
        );
        record(
            &logger,
            Level::Debug,
            "walklevel_core::history",
            "szczegół historii",
        );
        record(&logger, Level::Debug, BLE_TARGET, "surowe 02 53");
        record(
            &logger,
            Level::Info,
            "walklevel_core::treadmill::kingsmith",
            "połączono",
        );
        record(
            &logger,
            Level::Info,
            "btleplug::winrtble",
            "gadatliwy btleplug",
        );
        record(&logger, Level::Warn, "btleplug", "ostrzeżenie btleplug");
        record(&logger, Level::Trace, BLE_TARGET, "trace");
        logger.set_detailed(false);
        record(&logger, Level::Debug, BLE_TARGET, "po wyłączeniu");
        record(&logger, Level::Warn, "bieznia.blezzz", "nie bluetooth");

        let main = read(tmp.path(), MAIN_FILE);
        let ble = read(tmp.path(), BLE_FILE);
        assert!(main.contains(" INFO    walklevel_core::history: zapis sesji"));
        assert!(!main.contains("szczegół historii"));
        assert!(!main.contains("surowe 02 53"));
        assert!(main.contains("INFO    walklevel_core::treadmill::kingsmith: połączono"));
        assert!(!main.contains("gadatliwy"));
        assert!(main.contains("WARNING btleplug: ostrzeżenie btleplug"));
        assert!(main.contains("WARNING bieznia.blezzz: nie bluetooth"));
        assert!(ble.contains(" surowe 02 53"));
        assert!(ble.contains(" połączono"));
        assert!(!ble.contains("trace"));
        assert!(!ble.contains("po wyłączeniu"));
        assert!(!ble.contains("historii") && !ble.contains("btleplug") && !ble.contains("blezzz"));
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 4);
        assert!(
            seen[0].ends_with("  zapis sesji") && seen[0].len() == "12:00:00  zapis sesji".len()
        );
        // linia pliku zaczyna się datą, po której zgłoszenia wybierają okres
        let first = main.lines().next().unwrap();
        assert!(
            NaiveDateTime::parse_from_str(&first[..19], FILE_DATEFMT).is_ok(),
            "{first}"
        );
        let first_ble = ble.lines().next().unwrap();
        assert_eq!(&first_ble[19..20], ".");
    }

    #[test]
    fn debug_outside_bluetooth_follows_file_level() {
        let tmp = tempfile::tempdir().unwrap();
        let logger = Logger::new(tmp.path(), false, true).unwrap();
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = seen.clone();
        logger.set_listener(Some(Box::new(move |line| {
            sink.lock().unwrap().push(line.to_string())
        })));
        record(&logger, Level::Debug, "walklevel_lib::stan", "przed");
        record(
            &logger,
            Level::Info,
            "walklevel_lib",
            "[app-start] WalkLevel 0.6.3",
        );
        logger.set_file_level(LevelFilter::Debug);
        record(&logger, Level::Debug, "walklevel_lib::stan", "po");
        record(&logger, Level::Debug, BLE_TARGET, "ramka 02 53");
        record(&logger, Level::Debug, "btleplug", "gadatliwy");
        logger.set_file_level(LevelFilter::Error); // never below INFO
        record(&logger, Level::Info, "walklevel_lib::stan", "info zostaje");

        let main = read(tmp.path(), MAIN_FILE);
        assert!(!main.contains("przed"));
        assert!(main.contains("DEBUG   walklevel_lib::stan: po"));
        assert!(!main.contains("ramka") && !main.contains("gadatliwy"));
        assert!(main.contains("info zostaje"));
        assert!(read(tmp.path(), BLE_FILE).contains(" ramka 02 53"));
        assert!(main.contains("INFO    walklevel_lib: [app-start] WalkLevel 0.6.3"));
        // the preview in the window: without DEBUG and without event ids
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        assert!(seen[0].ends_with("  WalkLevel 0.6.3"), "{seen:?}");
        assert_eq!(without_event("[Not an id] x"), "[Not an id] x");
        assert_eq!(without_event("[dry] W=True"), "[dry] W=True");
        assert_eq!(parse_level(" Debug "), Some(LevelFilter::Debug));
        assert_eq!(parse_level("info"), Some(LevelFilter::Info));
        assert_eq!(parse_level("trace"), None);
    }

    #[test]
    fn redact_hides_personal_data() {
        let cases = [
            (
                "Wysłaliśmy link na jan.kowalski+test@example.com.pl",
                "Wysłaliśmy link na <e-mail>",
            ),
            (
                "error sending request for url (https://srv.example/api/bieznia/sesje?kursor=abc123def&limit=500)",
                "error sending request for url (https://srv.example/api/bieznia/sesje?…)",
            ),
            ("kursor=r0k1a2b3 limit=5", "kursor=… limit=5"),
            (
                "GET /api/bieznia/profil/u8a7b6c5d4e3f21",
                "GET /api/bieznia/profil/:id",
            ),
            (
                "POST /api/bieznia/zaproszenie/abc123/wycofaj",
                "POST /api/bieznia/zaproszenie/:id/wycofaj",
            ),
            (
                "DELETE /api/bieznia/znajomi/x1y2z3",
                "DELETE /api/bieznia/znajomi/:id",
            ),
            ("PUT /api/bieznia/stan", "PUT /api/bieznia/stan"),
            (
                r"dane: C:\Users\Jan Nowak\AppData\Local\Bieznia",
                r"dane: %USERPROFILE%\AppData\Local\Bieznia",
            ),
            (
                r#"{"path": "C:\\Users\\jan\\gry\\wow.exe"}"#,
                r#"{"path": "%USERPROFILE%\\gry\\wow.exe"}"#,
            ),
            ("d:/users/ola/x.log", "%USERPROFILE%/x.log"),
            ("Prędkość 2.5: przyjęta", "Prędkość 2.5: przyjęta"),
            (
                "svelte@5.57.3 i walklevel@0.6.3",
                "svelte@5.57.3 i walklevel@0.6.3",
            ),
        ];
        for (input, want) in cases {
            assert_eq!(redact(input), want, "{input}");
        }
    }

    #[test]
    fn previous_run_closed_reads_last_start_and_exit() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(previous_run_closed(tmp.path()), None);
        let path = tmp.path().join(MAIN_FILE);
        fs::write(
            &path,
            "2026-11-26 10:00:00 INFO    bieznia: WalkLevel 0.6.2\n",
        )
        .unwrap();
        assert_eq!(previous_run_closed(tmp.path()), None);
        let start = format!(
            "2026-11-27 10:00:00 INFO    walklevel_lib: {APP_START_EVENT} WalkLevel 0.6.3\n"
        );
        let exit = format!(
            "2026-11-27 11:00:00 INFO    walklevel_lib: {APP_EXIT_EVENT} Zamknięcie aplikacji: zamknięcie okna\n"
        );
        fs::write(&path, format!("{start}{exit}")).unwrap();
        assert_eq!(previous_run_closed(tmp.path()), Some(true));
        fs::write(&path, format!("{start}{exit}{start}")).unwrap();
        assert_eq!(previous_run_closed(tmp.path()), Some(false));
    }

    #[test]
    fn throttle_counts_dropped_entries() {
        let mut throttle = Throttle::new(2, Duration::from_secs(60));
        let t0 = Instant::now();
        assert_eq!(throttle.allow(t0), Some(0));
        assert_eq!(throttle.allow(t0), Some(0));
        assert_eq!(throttle.allow(t0 + Duration::from_secs(1)), None);
        assert_eq!(throttle.allow(t0 + Duration::from_secs(2)), None);
        assert_eq!(throttle.allow(t0 + Duration::from_secs(61)), Some(2));
        assert_eq!(throttle.allow(t0 + Duration::from_secs(62)), Some(0));
    }

    #[test]
    fn global_logger_and_panic_hook() {
        let tmp = tempfile::tempdir().unwrap();
        init(tmp.path(), false, true).unwrap();
        log::info!(target: "bieznia", "Start aplikacji");
        log::debug!(target: BLE_TARGET, "ramka");
        set_detailed(false);
        log::debug!(target: BLE_TARGET, "ukryta ramka");
        let worker = std::thread::Builder::new()
            .name("pracownik".into())
            .spawn(|| panic!("coś poszło nie tak"))
            .unwrap();
        assert!(worker.join().is_err());

        let logs = tmp.path().join("logs");
        let main = read(&logs, MAIN_FILE);
        assert!(main.contains("INFO    bieznia: Start aplikacji"));
        assert!(main.contains("CRITICAL bieznia.blad: Nieobsłużony błąd w wątku pracownik"));
        assert!(main.contains("coś poszło nie tak"));
        let ble = read(&logs, BLE_FILE);
        assert!(ble.contains(" ramka") && !ble.contains("ukryta"));

        // ponowne init przestawia pliki (jak force=True)
        let other = tempfile::tempdir().unwrap();
        init(other.path(), false, true).unwrap();
        log::warn!(target: "bieznia", "drugi folder");
        assert!(read(&other.path().join("logs"), MAIN_FILE).contains("drugi folder"));
        assert!(!read(&logs, MAIN_FILE).contains("drugi folder"));

        // errors of the interface: one line, without personal data, kind from a short list
        ui_error(
            "main",
            "command",
            "account_set_live: Command account_set_live not allowed by ACL\n  ola@example.com",
        );
        ui_error("popup", "command-error", "code offline");
        ui_error("main\n", "cokolwiek", "TypeError: x is undefined");
        let main = read(&other.path().join("logs"), MAIN_FILE);
        assert!(main.contains(
            "ERROR   ui: [ui-error] Interfejs (main, command): account_set_live: Command account_set_live not allowed by ACL <e-mail>"
        ));
        assert!(
            main.contains("WARNING ui: [ui-error] Interfejs (popup, command-error): code offline")
        );
        assert!(main.contains("ERROR   ui: [ui-error] Interfejs (main, error): TypeError"));

        // bez nazwy wątku: "w wątku ?" jak threading.excepthook w Pythonie
        assert!(std::thread::spawn(|| panic!("bez nazwy")).join().is_err());
        let main = read(&other.path().join("logs"), MAIN_FILE);
        assert!(main.contains("CRITICAL bieznia.blad: Nieobsłużony błąd w wątku ?"));

        // zamyka pliki logu, żeby folder tymczasowy dał się usunąć (Windows)
        *ACTIVE.write().unwrap() = None;
    }
}
