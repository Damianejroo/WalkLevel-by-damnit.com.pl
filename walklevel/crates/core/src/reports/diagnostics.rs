//! Raport diagnostyczny Bluetooth - składanie tekstu (port bieznia/ble/diagnostics.py).
//!
//! Rozmowę z bieżnią (skan, połączenie, odczyty, powiadomienia) prowadzi moduł bieżni,
//! a wyniki wpisuje tu po kolei - tekst raportu jest taki sam jak w 0.5.x:
//! lista urządzeń z okolicy, usługi i charakterystyki bieżni, odczyty (zakresy prędkości,
//! firmware), surowe powiadomienia (hex) i próba rozmowy protokołem KingSmith.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::Local;

use super::py_float;

/// Ile sekund słuchamy powiadomień bieżni.
pub const LISTEN_S: f64 = 12.0;
/// Ile sekund trwa skan okolicy.
pub const SCAN_S: f64 = 8.0;

/// Urządzenie znalezione w skanie.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScannedDevice {
    pub address: String,
    pub rssi: i16,
    /// nazwa z ogłoszenia albo urządzenia; `None` (lub pusta) - "(bez nazwy)"
    pub name: Option<String>,
    pub service_uuids: Vec<String>,
    /// dane producenta: (identyfikator firmy, bajty) w kolejności z ogłoszenia
    pub manufacturer_data: Vec<(u16, Vec<u8>)>,
    /// wygląda na bieżnię (znany model albo usługa bieżni) - dopisek "<-- bieżnia?"
    pub treadmill: bool,
}

/// Ścieżka nowego raportu: `logs/diagnostyka-RRRRMMDD-GGMMSS.txt`.
pub fn report_path(logs: &Path) -> PathBuf {
    logs.join(format!(
        "diagnostyka-{}.txt",
        Local::now().format("%Y%m%d-%H%M%S")
    ))
}

/// Bajty jako "aa bb cc" (jak `bytes.hex(" ")`).
fn hex_spaced(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn short_uuid(uuid: &str) -> String {
    uuid.chars().take(8).collect()
}

/// Odczyt jako tekst, gdy to drukowalne ASCII: "  'tekst'" (jak w 0.5.x), inaczej pusty.
fn shown_text(value: &[u8]) -> String {
    if value.is_empty() || !value.is_ascii() {
        return String::new();
    }
    let text: String = value.iter().map(|&b| char::from(b)).collect();
    // białe znaki ASCII jak w str.strip() w Pythonie
    let stripped =
        text.trim_matches(|c: char| matches!(c, ' ' | '\t'..='\r' | '\u{1c}'..='\u{1f}'));
    if stripped.chars().all(|c| (' '..='~').contains(&c)) {
        format!("  '{stripped}'")
    } else {
        String::new()
    }
}

/// Raport składany linia po linii. Linie z czasem mają przedrostek sekund od początku.
#[derive(Debug)]
pub struct DiagnosticReport {
    lines: Vec<String>,
    started: Instant,
}

impl DiagnosticReport {
    /// Nagłówek: wersja aplikacji i data, wersja Windows i biblioteki Bluetooth
    /// (`stack`, np. "btleplug 0.13.5"), pusta linia.
    pub fn new(version: &str, system: &str, stack: &str) -> Self {
        let mut report = Self {
            lines: Vec::new(),
            started: Instant::now(),
        };
        report.line(format!(
            "Raport WalkLevel {version} - {}",
            Local::now().format("%Y-%m-%d %H:%M:%S")
        ));
        report.line(format!("Windows {system} | {stack}"));
        report.blank();
        report
    }

    /// Dowolna linia.
    pub fn line(&mut self, text: impl Into<String>) {
        self.lines.push(text.into());
    }

    /// Pusta linia.
    pub fn blank(&mut self) {
        self.lines.push(String::new());
    }

    /// Linia z czasem od początku raportu, np. "   1.25s tekst".
    pub fn stamped(&mut self, text: &str) {
        let secs = self.started.elapsed().as_secs_f64();
        self.line(format!("{secs:7.2}s {text}"));
    }

    /// "== Skan (8 s) ==" - przed skanem.
    pub fn scan_started(&mut self) {
        self.line(format!("== Skan ({SCAN_S:.0} s) =="));
    }

    /// Lista urządzeń z okolicy - od najsilniejszego sygnału, z usługami i danymi producenta.
    pub fn devices(&mut self, devices: &[ScannedDevice]) {
        let mut sorted: Vec<&ScannedDevice> = devices.iter().collect();
        sorted.sort_by_key(|d| std::cmp::Reverse(d.rssi));
        for dev in sorted {
            let name = dev
                .name
                .as_deref()
                .filter(|n| !n.is_empty())
                .unwrap_or("(bez nazwy)");
            let mark = if dev.treadmill { "  <-- bieżnia?" } else { "" };
            self.line(format!("{}  {:4} dBm  {name}{mark}", dev.address, dev.rssi));
            if !dev.service_uuids.is_empty() {
                self.line(format!("    usługi: {}", dev.service_uuids.join(", ")));
            }
            for (company, data) in &dev.manufacturer_data {
                self.line(format!(
                    "    producent 0x{company:04x}: {}",
                    hex_spaced(data)
                ));
            }
        }
    }

    /// Skan nie znalazł bieżni - koniec raportu.
    pub fn no_treadmill(&mut self) {
        self.line("Nie znaleziono bieżni - brak połączenia do zbadania.");
    }

    /// "== Połączenie z ADRES ==".
    pub fn connecting(&mut self, address: &str) {
        self.line(format!("== Połączenie z {address} =="));
    }

    /// Urządzenie zniknęło przed połączeniem - koniec raportu.
    pub fn device_gone(&mut self) {
        self.line("Urządzenie zniknęło przed połączeniem.");
    }

    /// Usługa GATT.
    pub fn service(&mut self, uuid: &str, description: &str) {
        self.line(format!("usługa {uuid}  {description}"));
    }

    /// Charakterystyka usługi z właściwościami (read, write, notify…).
    pub fn characteristic(&mut self, uuid: &str, properties: &[&str], description: &str) {
        self.line(format!(
            "    char {uuid}  [{}]  {description}",
            properties.join(", ")
        ));
    }

    /// Nagłówek sekcji odczytów (poprzedzony pustą linią).
    pub fn reads_started(&mut self) {
        self.blank();
        self.line("== Odczyt charakterystyk (zakresy, firmware - do profilu modelu) ==");
    }

    /// Odczytana wartość charakterystyki (hex i tekst, jeśli drukowalny).
    pub fn read_value(&mut self, uuid: &str, value: &[u8]) {
        self.line(format!(
            "    {}  {}{}",
            short_uuid(uuid),
            hex_spaced(value),
            shown_text(value)
        ));
    }

    /// Odczyt się nie udał (`error_kind` - rodzaj błędu, np. "Timeout").
    pub fn read_failed(&mut self, uuid: &str, error_kind: &str) {
        self.line(format!(
            "    {}  nie da się odczytać: {error_kind}",
            short_uuid(uuid)
        ));
    }

    /// Nagłówek sekcji powiadomień (poprzedzony pustą linią).
    pub fn notifications_started(&mut self, listen_s: f64) {
        self.blank();
        self.line(format!("== Powiadomienia przez {listen_s:.0} s =="));
    }

    /// Nie da się subskrybować charakterystyki.
    pub fn subscribe_failed(&mut self, uuid: &str, error: &str) {
        self.line(format!("    nie da się subskrybować {uuid}: {error}"));
    }

    /// Surowe powiadomienie: "  0.52s << 00002acd  04 00 a0 00".
    pub fn notification(&mut self, uuid: &str, data: &[u8]) {
        self.stamped(&format!("<< {}  {}", short_uuid(uuid), hex_spaced(data)));
    }

    /// Kanały protokołu KingSmith znalezione w usługach.
    pub fn kingsmith_channels(&mut self, labels: &[&str]) {
        self.line(format!("Protokół KingSmith: kanały {}", labels.join(", ")));
    }

    /// Tekst odebrany protokołem KingSmith.
    pub fn kingsmith_text(&mut self, text: &str) {
        self.stamped(&format!("tekst: {text}"));
    }

    /// Odczyt prędkości i kroków (kroki `None` - bieżnia ich nie podaje).
    pub fn reading(&mut self, speed_kmh: f64, steps: Option<i64>) {
        let steps = steps.map_or_else(|| "None".to_string(), |s| s.to_string());
        self.stamped(&format!(
            "ODCZYT prędkość={} km/h kroki={steps}",
            py_float(speed_kmh)
        ));
    }

    /// Podsumowanie sterownika po próbie rozmowy.
    pub fn result(&mut self, describe: &str) {
        self.line(format!("Wynik: {describe}"));
    }

    /// Błąd, który przerwał diagnostykę (`kind` - rodzaj, `message` - opis).
    pub fn error(&mut self, kind: &str, message: &str) {
        self.line(format!("BŁĄD: {kind}: {message}"));
    }

    /// Cały tekst raportu (linie zakończone znakiem nowej linii).
    pub fn text(&self) -> String {
        format!("{}\n", self.lines.join("\n"))
    }

    /// Zapisuje raport (UTF-8), tworząc folder. Zwraca ścieżkę.
    pub fn save(&self, path: &Path) -> io::Result<PathBuf> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.text())?;
        Ok(path.to_path_buf())
    }
}
