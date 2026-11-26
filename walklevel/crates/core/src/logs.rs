//! Logi w plikach - port bieznia/logs.py.
//!
//! - logs/bieznia.log (+ konsola): poziom INFO i wyżej ze wszystkich modułów,
//! - logs/bluetooth.log: ruch Bluetooth (cel [`BLE_TARGET`] albo moduł `treadmill`); poziom DEBUG
//!   tylko przy włączonych szczegółowych logach ([`set_detailed`]), inaczej INFO,
//! - btleplug sam w sobie gada dużo - z niego tylko ostrzeżenia i błędy,
//! - panika (także w wątku) trafia do logu jako CRITICAL - exe nie ma konsoli.
//!
//! Pliki są zgodne z 0.5.x: ten sam format linii, rotacja co 2 MB z 3 starszymi częściami
//! (bieznia.log.1 ... .3), końce linii CRLF na Windows. Zgłoszenia czytają je po dacie z linii.

use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use log::{Level, LevelFilter, Log, Metadata, Record};

/// Główny plik logu w logs/.
pub const MAIN_FILE: &str = "bieznia.log";
/// Plik z rozmową z bieżnią w logs/.
pub const BLE_FILE: &str = "bluetooth.log";
/// Rozmiar, po którym plik jest obracany (jak `maxBytes` w Pythonie).
pub const MAX_BYTES: u64 = 2_000_000;
/// Ile starszych części zostaje (bieznia.log.1 ... .3).
pub const BACKUP_COUNT: u32 = 3;
/// Data w pliku - zgłoszenie bierze dokładnie wybrany okres.
pub const FILE_DATEFMT: &str = "%Y-%m-%d %H:%M:%S";
/// Data w konsoli i w podglądzie logu w oknie.
pub const CONSOLE_DATEFMT: &str = "%H:%M:%S";
/// Cel logów Bluetooth: `log::debug!(target: BLE_TARGET, ...)`.
pub const BLE_TARGET: &str = "bieznia.ble";
/// Cel wpisów o nieobsłużonych błędach (panika).
pub const CRASH_TARGET: &str = "bieznia.blad";

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
        if self.max_bytes > 0 && self.pos > 0 && self.pos + len >= self.max_bytes {
            // nieudane obrócenie (np. plik trzyma inny proces) nie gubi wpisu: piszemy dalej
            let _ = self.rollover();
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
            std::fs::rename(&self.path, &first)?;
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
    listener: RwLock<Option<Listener>>,
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
            listener: RwLock::new(None),
        })
    }

    /// Szczegółowe logi = surowa rozmowa z bieżnią (DEBUG) w bluetooth.log.
    pub fn set_detailed(&self, enabled: bool) {
        self.detailed.store(enabled, Ordering::Relaxed);
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
        level <= Level::Info
    }

    fn emit(&self, ns: i128, level: &str, info: bool, target: &str, msg: &str) {
        let (secs, msecs) = stamp(ns);
        let time = local_time(secs);
        if info {
            let _ = lock(&self.main).write_record(&main_line(&time, level, target, msg));
            if self.console {
                let _ = writeln!(io::stderr(), "{}", console_line(&time, level, target, msg));
            }
            if let Some(listener) = self
                .listener
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
            {
                listener(&format!("{}  {msg}", time.format(CONSOLE_DATEFMT)));
            }
        }
        if is_ble(target) {
            let _ = lock(&self.ble).write_record(&ble_line(&time, msecs, msg));
        }
    }

    /// Wpis CRITICAL o nieobsłużonym błędzie (z haka paniki).
    fn critical(&self, msg: &str) {
        self.emit(now_ns(), "CRITICAL", true, CRASH_TARGET, msg);
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
        let info = record.level() <= Level::Info;
        self.emit(
            now_ns(),
            level_name(record.level()),
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
            let head = match std::thread::current().name() {
                Some("main") | None => "Nieobsłużony błąd".to_string(),
                Some(name) => format!("Nieobsłużony błąd w wątku {name}"),
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
            .join("tests/fixtures/baza")
            .join(name);
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn lines_match_python() {
        for case in fixture("log_lines.json").as_array().unwrap() {
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
    }
}
