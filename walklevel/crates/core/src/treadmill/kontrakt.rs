//! Kontrakt sterownika bieżni - to, co rdzeń aplikacji wie o każdej bieżni (port
//! bieznia/sterowniki/kontrakt.py).
//!
//! Okno i wątek Bluetooth nie pytają o model, tylko o możliwości ([`Capabilities`]): zakres
//! prędkości, odliczanie przed ruszeniem, czy bieżnia potwierdza polecenia, czy zna pauzę,
//! czy podaje kroki. Każdy moduł (KingSmith, FTMS, symulator) spełnia ten sam kontrakt
//! ([`Driver`]).

use std::sync::{Arc, OnceLock};
use std::time::Instant;

use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};

pub use super::telemetry::TelemetryFrame;

/// Moduł sterownika. Nazwy tekstowe jak w Pythonie - trafiają do historii (`Session::driver`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DriverKind {
    #[serde(rename = "kingsmith")]
    KingSmith,
    #[serde(rename = "ftms")]
    Ftms,
    #[serde(rename = "symulator")]
    Simulator,
}

impl DriverKind {
    /// "kingsmith" | "ftms" | "symulator"
    pub fn as_str(self) -> &'static str {
        match self {
            DriverKind::KingSmith => "kingsmith",
            DriverKind::Ftms => "ftms",
            DriverKind::Simulator => "symulator",
        }
    }

    /// Odwrotność [`DriverKind::as_str`].
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "kingsmith" => Some(DriverKind::KingSmith),
            "ftms" => Some(DriverKind::Ftms),
            "symulator" => Some(DriverKind::Simulator),
            _ => None,
        }
    }
}

/// Co bieżnia potrafi - dane, nie kod. Część z profilu modelu, część z samej bieżni.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    /// km/h - najwolniej, jak taśma pojedzie
    pub speed_min: f64,
    pub speed_max: f64,
    pub speed_step: f64,
    /// z taką prędkością taśma rusza po starcie
    pub start_speed: f64,
    /// wbudowane odliczanie przed ruszeniem (nie da się wyłączyć)
    pub start_delay_s: f64,
    /// każde polecenie dostaje odpowiedź z wynikiem
    pub acks_commands: bool,
    /// pauza różna od stopu (bieżnia trzyma sesję)
    pub pause: bool,
    /// bieżnia podaje licznik kroków
    pub steps: bool,
    /// (od, do, co) w %
    pub incline: Option<(f64, f64, f64)>,
    /// typowo co tyle sekund przychodzi ramka z prędkością
    pub telemetry_s: f64,
    /// Najkrótszy odstęp między poleceniami prędkości: przy szybkim ruszaniu gałką liczy się
    /// tylko najnowsza prędkość (bieżnie z potwierdzeniem nie lubią serii).
    pub min_command_gap_s: f64,
    /// aplikacja może uruchamiać, zatrzymywać i zmieniać prędkość
    pub control: bool,
}

impl Capabilities {
    /// Wartości domyślne kontraktu (jak `Capabilities()` w Pythonie).
    pub const DEFAULT: Capabilities = Capabilities {
        speed_min: 0.5,
        speed_max: 6.0,
        speed_step: 0.1,
        start_speed: 1.0,
        start_delay_s: 0.0,
        acks_commands: false,
        pause: false,
        steps: true,
        incline: None,
        telemetry_s: 1.0,
        min_command_gap_s: 0.0,
        control: true,
    };

    /// Prędkość, którą taśma naprawdę pojedzie (0 = stop zostaje 0).
    pub fn clamp(&self, kmh: f64) -> f64 {
        if kmh <= 0.0 {
            return 0.0;
        }
        py_round1(self.speed_max.min(self.speed_min.max(kmh)))
    }

    /// Zakres zgłoszony przez samą bieżnię wygrywa z profilem (sprawdzone: Urevo).
    pub fn with_range(&self, lo: f64, hi: f64, step: f64) -> Capabilities {
        if !(0.0 < lo && lo < hi) {
            return *self;
        }
        Capabilities {
            speed_min: lo,
            speed_max: hi,
            speed_step: if step != 0.0 { step } else { self.speed_step },
            ..*self
        }
    }
}

impl Default for Capabilities {
    fn default() -> Self {
        Capabilities::DEFAULT
    }
}

/// Co to za bieżnia - zapisywane przy każdej sesji w historii.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    /// czytelna nazwa, np. "Urevo SpaceWalk 5L"
    pub model: String,
    pub driver: DriverKind,
    /// id profilu modelu albo ""
    pub profile: String,
    /// nazwa Bluetooth, np. "URTM054"
    pub name: String,
    pub firmware: String,
}

impl DeviceInfo {
    pub fn simulated(&self) -> bool {
        self.driver == DriverKind::Simulator
    }
}

/// Wynik startu taśmy: ruszyła / bieżnia odmówiła / brak odpowiedzi (prędkość pokaże).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StartResult {
    Started,
    Refused,
    Unknown,
    Unsupported,
}

impl StartResult {
    /// "started" | "refused" | "unknown" | "unsupported" (jak w Pythonie)
    pub fn as_str(self) -> &'static str {
        match self {
            StartResult::Started => "started",
            StartResult::Refused => "refused",
            StartResult::Unknown => "unknown",
            StartResult::Unsupported => "unsupported",
        }
    }
}

/// Odbiorca ramek telemetrii (wątek Bluetooth -> silnik sterowania i licznik sesji).
pub type FrameCallback = Arc<dyn Fn(TelemetryFrame) + Send + Sync>;

/// Zegar monotoniczny w sekundach (w testach podmieniany).
pub type Clock = Arc<dyn Fn() -> f64 + Send + Sync>;

/// Zegar monotoniczny procesu (odpowiednik `time.monotonic`).
pub fn monotonic_clock() -> Clock {
    static START: OnceLock<Instant> = OnceLock::new();
    let start = *START.get_or_init(Instant::now);
    Arc::new(move || start.elapsed().as_secs_f64())
}

/// Liczniki jednego połączenia (do podsumowania w logu). Moduł używa tych, które ma.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub commands: u64,
    pub acks: u64,
    pub refused: u64,
    pub frames: u64,
    pub undecoded: u64,
    pub polls: u64,
}

/// Błędy sterownika i połączenia.
#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    /// Urządzenie nie ma żadnej znanej usługi bieżni.
    #[error(
        "To urządzenie nie wygląda na bieżnię. Jeśli to bieżnia, zgłoś ją w Ustawienia > \
         Pomoc i diagnostyka > Zgłoś problem."
    )]
    UnsupportedDevice,
    /// Błąd Bluetooth (zapis, odczyt, subskrypcja).
    #[error("Problem z Bluetooth. Łączę ponownie… ({0})")]
    Gatt(String),
    /// Połączenie zerwane.
    #[error("Bieżnia się rozłączyła. Łączę ponownie…")]
    Disconnected,
}

/// Charakterystyka GATT (UUID małymi literami w pełnej postaci 128-bitowej).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GattCharacteristic {
    pub service_uuid: String,
    pub uuid: String,
    /// np. "read", "write", "write-without-response", "notify", "indicate"
    pub properties: Vec<String>,
}

/// Usługa GATT z charakterystykami.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GattService {
    pub uuid: String,
    pub characteristics: Vec<GattCharacteristic>,
}

/// Podzbiór klienta BLE, którego używają sterowniki (btleplug w aplikacji, wirtualna bieżnia
/// w testach). Powiadomienia z subskrybowanych charakterystyk worker przekazuje do
/// [`Driver::on_notification`].
pub trait GattClient: Send + Sync {
    fn services(&self) -> Vec<GattService>;

    fn read<'a>(
        &'a self,
        characteristic: &'a GattCharacteristic,
    ) -> BoxFuture<'a, Result<Vec<u8>, DriverError>>;

    fn write<'a>(
        &'a self,
        characteristic: &'a GattCharacteristic,
        data: &'a [u8],
        with_response: bool,
    ) -> BoxFuture<'a, Result<(), DriverError>>;

    fn subscribe<'a>(
        &'a self,
        characteristic: &'a GattCharacteristic,
    ) -> BoxFuture<'a, Result<(), DriverError>>;
}

/// Sterownik jednej bieżni. Metody biorą `&self` (stan wewnątrz pod zamkiem), żeby wątek
/// Bluetooth mógł podawać powiadomienia i pytać o stan, gdy start albo stop jest w toku:
///
/// - `start()`: subskrypcje i rozpoznanie bieżni; potem `capabilities()` i `info()` są pewne,
/// - `poll()`: co `poll_interval()` - odpytanie i pilnowanie zadanej prędkości,
/// - `start_belt()` / `stop_belt()`: z ponawianiem do skutku albo do limitu prób,
/// - `set_speed()`: prędkość docelowa; sterownik sam pilnuje, żeby taśma jej posłuchała,
/// - `belt_speed()` / `belt_running()`: ostatni stan z danych bieżni (None = nie wiadomo).
pub trait Driver: Send + Sync {
    /// "KingSmith" | "FTMS" | "symulator"
    fn protocol(&self) -> &'static str;

    fn capabilities(&self) -> Capabilities;

    fn info(&self) -> DeviceInfo;

    fn counts(&self) -> Counts;

    /// Liczniki poleceń prędkości (jeśli moduł je prowadzi).
    fn speed_stats(&self) -> Option<super::watchdog::SpeedStats> {
        None
    }

    fn start(&self) -> BoxFuture<'_, Result<(), DriverError>>;

    fn poll(&self) -> BoxFuture<'_, Result<(), DriverError>>;

    /// Co ile sekund wołać `poll()`.
    fn poll_interval(&self) -> f64;

    fn describe(&self) -> String;

    fn start_belt(&self) -> BoxFuture<'_, Result<StartResult, DriverError>>;

    /// `verify = false` przy zamykaniu aplikacji (pełny stop zamiast pauzy). Wynik: czy
    /// bieżnia potwierdziła zatrzymanie (bez sprawdzania - zawsze `true`).
    fn stop_belt(&self, verify: bool) -> BoxFuture<'_, Result<bool, DriverError>>;

    /// Liczniki tej bieżni (zakresy, rozdzielczość) dla [`super::telemetry::Meter`].
    fn telemetry_spec(&self) -> super::telemetry::TelemetrySpec {
        super::telemetry::TelemetrySpec::for_driver(self.info().driver, &self.capabilities())
    }

    fn set_speed(&self, kmh: f64) -> BoxFuture<'_, Result<bool, DriverError>>;

    /// Prędkość z ostatniej ramki i czas tej ramki (zegar sterownika).
    fn belt_speed(&self) -> Option<(f64, f64)>;

    fn belt_running(&self) -> Option<bool>;

    /// Start albo stop w toku - wtedy prędkości nie pilnujemy.
    fn busy(&self) -> bool;

    /// Powiadomienie z charakterystyki (UUID małymi literami).
    fn on_notification(&self, _characteristic_uuid: &str, _data: &[u8]) {}
}

/// `round(x, 1)` jak w Pythonie (zaokrąglenie dziesiętne, remis do parzystej).
pub(crate) fn py_round1(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    format!("{x:.1}").parse().unwrap_or(x)
}

/// Białe znaki jak w `str.split()` Pythona (Rust nie liczy do nich \x1c-\x1f).
pub(crate) fn py_is_space(c: char) -> bool {
    c.is_whitespace() || ('\x1c'..='\x1f').contains(&c)
}

/// `str.split()` bez argumentów.
pub(crate) fn py_split(text: &str) -> impl Iterator<Item = &str> {
    text.split(py_is_space).filter(|part| !part.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn py_round_is_decimal_half_even() {
        assert_eq!(py_round1(0.25), 0.2);
        assert_eq!(py_round1(0.35), 0.3);
        assert_eq!(py_round1(1.05), 1.1);
        assert_eq!(py_round1(0.15), 0.1);
        assert_eq!(py_round1(2.45), 2.5);
    }

    #[test]
    fn driver_kind_names() {
        for kind in [
            DriverKind::KingSmith,
            DriverKind::Ftms,
            DriverKind::Simulator,
        ] {
            assert_eq!(DriverKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(DriverKind::parse("python"), None);
    }
}
