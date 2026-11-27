//! Profile modeli bieżni - DANE (JSON), nie kod (port bieznia/sterowniki/profile.py
//! i bieznia/sterowniki/pobieranie.py).
//!
//! Profil mówi, jak rozpoznać model po nazwie Bluetooth, którym modułem z nim rozmawiać i czym
//! różni się od ustawień domyślnych modułu (zakres prędkości, odliczanie, pauza...). Wbudowane
//! są wkompilowane z assets/profile/, dodatkowe (np. pobrane) leżą w `<dane>/profile/`.
//! Profil nigdy nie zawiera kodu - nowe sterowniki przychodzą zwykłą aktualizacją aplikacji.
//!
//! Pobieranie: lista modeli (indeks.json) i pliki profili leżą w repozytorium aplikacji na
//! GitHubie. Pobieramy wyłącznie przez HTTPS z raw.githubusercontent.com, każdy plik
//! sprawdzamy sumą SHA-256 z listy, rozmiarem i pełną walidacją profilu.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::contract::{Capabilities, DriverKind};
use crate::compat::{is_space, truthy};

pub const FORMAT: i64 = 1;
/// Moduły, które profil może wskazać.
pub const DRIVERS: [&str; 2] = ["kingsmith", "ftms"];
pub const SOURCE_BUILTIN: &str = "wbudowany";
pub const SOURCE_USER: &str = "dodatkowy";
pub const SOURCE_DOWNLOADED: &str = "pobrany";

/// Wbudowane profile (nazwa pliku, treść) w kolejności nazw plików.
pub const BUILTIN: [(&str, &str); 5] = [
    (
        "kingsmith-x21.json",
        include_str!("../../assets/profile/kingsmith-x21.json"),
    ),
    (
        "kingsmith-x23.json",
        include_str!("../../assets/profile/kingsmith-x23.json"),
    ),
    (
        "kingsmith.json",
        include_str!("../../assets/profile/kingsmith.json"),
    ),
    (
        "urevo-spacewalk-5l.json",
        include_str!("../../assets/profile/urevo-spacewalk-5l.json"),
    ),
    (
        "urevo.json",
        include_str!("../../assets/profile/urevo.json"),
    ),
];
/// Lista wbudowanych profili z sumami (ta sama, co w repozytorium do pobierania).
pub const BUILTIN_INDEX: &str = include_str!("../../assets/profile/indeks.json");

/// Plik profilu nie spełnia formatu - pomijamy go (z wpisem w logu).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ProfileError(pub String);

fn err<T>(message: impl Into<String>) -> Result<T, ProfileError> {
    Err(ProfileError(message.into()))
}

/// Możliwości, które profil zmienia względem domyślnych modułu (None = bez zmian).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityOverrides {
    pub speed_min: Option<f64>,
    pub speed_max: Option<f64>,
    pub speed_step: Option<f64>,
    pub start_speed: Option<f64>,
    pub start_delay_s: Option<f64>,
    pub acks_commands: Option<bool>,
    pub pause: Option<bool>,
    pub steps: Option<bool>,
    pub telemetry_s: Option<f64>,
    pub min_command_gap_s: Option<f64>,
}

/// Pola liczbowe i ich dozwolone zakresy (jak _LIMITS w Pythonie).
const FLOAT_FIELDS: [(&str, f64, f64); 7] = [
    ("speed_min", 0.1, 30.0),
    ("speed_max", 0.5, 30.0),
    ("speed_step", 0.01, 1.0),
    ("start_speed", 0.1, 30.0),
    ("start_delay_s", 0.0, 15.0),
    ("telemetry_s", 0.1, 10.0),
    ("min_command_gap_s", 0.0, 5.0),
];
const BOOL_FIELDS: [&str; 3] = ["acks_commands", "pause", "steps"];

impl CapabilityOverrides {
    fn float_slot(&mut self, key: &str) -> Option<&mut Option<f64>> {
        Some(match key {
            "speed_min" => &mut self.speed_min,
            "speed_max" => &mut self.speed_max,
            "speed_step" => &mut self.speed_step,
            "start_speed" => &mut self.start_speed,
            "start_delay_s" => &mut self.start_delay_s,
            "telemetry_s" => &mut self.telemetry_s,
            "min_command_gap_s" => &mut self.min_command_gap_s,
            _ => return None,
        })
    }

    fn bool_slot(&mut self, key: &str) -> Option<&mut Option<bool>> {
        Some(match key {
            "acks_commands" => &mut self.acks_commands,
            "pause" => &mut self.pause,
            "steps" => &mut self.steps,
            _ => return None,
        })
    }

    /// Możliwości modułu poprawione profilem.
    pub fn apply(&self, base: &Capabilities) -> Capabilities {
        Capabilities {
            speed_min: self.speed_min.unwrap_or(base.speed_min),
            speed_max: self.speed_max.unwrap_or(base.speed_max),
            speed_step: self.speed_step.unwrap_or(base.speed_step),
            start_speed: self.start_speed.unwrap_or(base.start_speed),
            start_delay_s: self.start_delay_s.unwrap_or(base.start_delay_s),
            acks_commands: self.acks_commands.unwrap_or(base.acks_commands),
            pause: self.pause.unwrap_or(base.pause),
            steps: self.steps.unwrap_or(base.steps),
            telemetry_s: self.telemetry_s.unwrap_or(base.telemetry_s),
            min_command_gap_s: self.min_command_gap_s.unwrap_or(base.min_command_gap_s),
            ..*base
        }
    }

    /// Ustawione pola jako (klucz JSON, wartość), posortowane po kluczu.
    pub fn entries(&self) -> Vec<(&'static str, Value)> {
        let mut copy = *self;
        let mut out: Vec<(&'static str, Value)> = Vec::new();
        for (key, _, _) in FLOAT_FIELDS {
            if let Some(Some(v)) = copy.float_slot(key).map(|s| *s) {
                out.push((key, Value::from(v)));
            }
        }
        for key in BOOL_FIELDS {
            if let Some(Some(v)) = copy.bool_slot(key).map(|s| *s) {
                out.push((key, Value::from(v)));
            }
        }
        out.sort_by_key(|(k, _)| *k);
        out
    }
}

/// Profil modelu bieżni.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProfile {
    pub id: String,
    pub name: String,
    pub driver: DriverKind,
    /// początki nazw Bluetooth (wielkość liter bez znaczenia)
    #[serde(rename = "match")]
    pub match_prefixes: Vec<String>,
    pub capabilities: CapabilityOverrides,
    /// ustawienia modułu, np. tablica szyfru ("table": "v7") - tylko proste wartości
    pub options: BTreeMap<String, Value>,
    /// "wbudowany" | "dodatkowy" | "pobrany"
    pub source: String,
}

impl ModelProfile {
    /// Możliwości modułu poprawione tym profilem.
    pub fn apply(&self, base: &Capabilities) -> Capabilities {
        self.capabilities.apply(base)
    }

    /// Opcja tekstowa (np. "table").
    pub fn option_str(&self, key: &str) -> Option<&str> {
        self.options.get(key).and_then(Value::as_str)
    }

    /// Długość najdłuższego pasującego początku nazwy (0 = nie pasuje).
    pub fn matches(&self, device_name: &str) -> usize {
        let upper = device_name.to_uppercase();
        self.match_prefixes
            .iter()
            .filter(|m| upper.starts_with(&m.to_uppercase()))
            .map(|m| m.chars().count())
            .max()
            .unwrap_or(0)
    }
}

/// `repr()` wartości JSON w Pythonie (do komunikatów): None/True/False, teksty w apostrofach
/// (w cudzysłowie, gdy zawierają tylko apostrof), listy i słowniki ze spacją po przecinku.
fn py_repr(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => "None".into(),
        Some(Value::Bool(true)) => "True".into(),
        Some(Value::Bool(false)) => "False".into(),
        Some(Value::String(s)) => py_str_repr(s),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Array(items)) => {
            let parts: Vec<String> = items.iter().map(|v| py_repr(Some(v))).collect();
            format!("[{}]", parts.join(", "))
        }
        Some(Value::Object(map)) => {
            let parts: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{}: {}", py_str_repr(k), py_repr(Some(v))))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
    }
}

fn py_str_repr(text: &str) -> String {
    let quote = if text.contains('\'') && !text.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::from(quote);
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c == '\x7f' => out.push_str(&format!("\\x{:02x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

fn py_float_repr(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e16 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

/// Liczba JSON (bez true/false) jako f64.
fn json_number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        _ => None,
    }
}

/// Sprawdza profil i zwraca go albo błąd z opisem (jak parse_profile w Pythonie).
pub fn parse_profile(data: &Value, source: &str) -> Result<ModelProfile, ProfileError> {
    let Some(obj) = data.as_object() else {
        return err("profil musi być obiektem JSON");
    };
    let format_ok = match obj.get("format") {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64() == Some(FORMAT as f64),
        _ => false,
    };
    if !format_ok {
        return err(format!(
            "nieznany format profilu {}",
            py_repr(obj.get("format"))
        ));
    }
    let id = match obj.get("id") {
        Some(Value::String(s))
            if !s.is_empty() && {
                let bare: String = s.chars().filter(|c| *c != '-').collect();
                !bare.is_empty() && bare.chars().all(char::is_alphanumeric)
            } =>
        {
            s.clone()
        }
        _ => return err("brak poprawnego id (litery, cyfry, myślniki)"),
    };
    let name = match obj.get("name") {
        Some(Value::String(s)) if !s.trim_matches(is_space).is_empty() => {
            s.trim_matches(is_space).to_string()
        }
        _ => return err("brak nazwy modelu"),
    };
    let driver = match obj.get("driver").and_then(Value::as_str) {
        Some(d) if DRIVERS.contains(&d) => DriverKind::parse(d),
        _ => None,
    };
    let Some(driver) = driver else {
        return err(format!(
            "nieznany moduł {} (dozwolone: {})",
            py_repr(obj.get("driver")),
            DRIVERS.join(", ")
        ));
    };
    let match_prefixes = match obj.get("match") {
        None => Vec::new(),
        Some(Value::Array(items)) => {
            let mut out = Vec::new();
            for item in items {
                match item.as_str() {
                    Some(s) if !s.is_empty() => out.push(s.to_string()),
                    _ => return err("match: lista początków nazw Bluetooth"),
                }
            }
            out
        }
        Some(_) => return err("match: lista początków nazw Bluetooth"),
    };
    let mut caps = CapabilityOverrides::default();
    match obj.get("capabilities") {
        Some(Value::Object(map)) => {
            for (key, value) in map {
                parse_capability(&mut caps, key, value)?;
            }
        }
        Some(other) if truthy(other) => return err("capabilities: oczekiwano obiektu"),
        _ => {}
    }
    if caps.speed_min.unwrap_or(0.0) >= caps.speed_max.unwrap_or(99.0) {
        return err("speed_min musi być mniejsze od speed_max");
    }
    let mut options = BTreeMap::new();
    match obj.get("options") {
        Some(Value::Object(map)) => {
            for (key, value) in map {
                if !matches!(value, Value::String(_) | Value::Number(_) | Value::Bool(_)) {
                    return err("options: tylko proste wartości");
                }
                options.insert(key.clone(), value.clone());
            }
        }
        Some(other) if truthy(other) => return err("options: tylko proste wartości"),
        _ => {}
    }
    Ok(ModelProfile {
        id,
        name,
        driver,
        match_prefixes,
        capabilities: caps,
        options,
        source: source.to_string(),
    })
}

fn parse_capability(
    caps: &mut CapabilityOverrides,
    key: &str,
    value: &Value,
) -> Result<(), ProfileError> {
    if let Some(slot) = caps.bool_slot(key) {
        let Value::Bool(b) = value else {
            return err(format!("{key}: oczekiwano true/false"));
        };
        *slot = Some(*b);
        return Ok(());
    }
    let Some(&(_, lo, hi)) = FLOAT_FIELDS.iter().find(|(k, _, _)| *k == key) else {
        return err(format!("nieznana możliwość '{key}'"));
    };
    let Some(number) = json_number(value) else {
        return err(format!("{key}: oczekiwano liczby"));
    };
    if !(lo <= number && number <= hi) {
        return err(format!(
            "{key}: {value} poza zakresem {}-{}",
            py_float_repr(lo),
            py_float_repr(hi)
        ));
    }
    if let Some(slot) = caps.float_slot(key) {
        *slot = Some(number);
    }
    Ok(())
}

/// Profil z bajtów pliku JSON.
pub fn parse_profile_bytes(data: &[u8], source: &str) -> Result<ModelProfile, ProfileError> {
    let value: Value = serde_json::from_slice(data).map_err(|e| ProfileError(e.to_string()))?;
    parse_profile(&value, source)
}

/// Klucz sortowania nazw plików jak `sorted(Path)` w Pythonie: na Windows bez wielkości liter.
fn file_sort_key(name: &str) -> String {
    if cfg!(windows) {
        name.to_lowercase()
    } else {
        name.to_string()
    }
}

/// Pliki *.json z folderu (bez indeks.json), posortowane jak w Pythonie. Wzorzec `*.json` jak
/// glob w pathlib: pasuje też sama ".json", a na Windows wielkość liter nie ma znaczenia.
fn json_files(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut files: Vec<(String, PathBuf)> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter_map(|p| {
            let name = p.file_name()?.to_string_lossy().to_string();
            let matches = if cfg!(windows) {
                name.to_lowercase().ends_with(".json")
            } else {
                name.ends_with(".json")
            };
            (matches && name != INDEX).then_some((name, p))
        })
        .collect();
    files.sort_by_cached_key(|(name, _)| file_sort_key(name));
    files.into_iter().map(|(_, p)| p).collect()
}

fn merge(found: &mut Vec<ModelProfile>, profile: ModelProfile) {
    // ten sam id zastępuje wcześniejszy w tym samym miejscu (jak słownik w Pythonie)
    match found.iter_mut().find(|p| p.id == profile.id) {
        Some(slot) => *slot = profile,
        None => found.push(profile),
    }
}

/// Wszystkie znane profile: wbudowane + z folderu danych (ten sam id w danych zastępuje
/// wbudowany). Odpowiednik `all_profiles()` / `profile_for()` z Pythona.
#[derive(Debug, Clone)]
pub struct ProfileRegistry {
    user_dir: Option<PathBuf>,
    profiles: Vec<ModelProfile>,
}

impl ProfileRegistry {
    /// Tylko wbudowane profile (bez folderu danych).
    pub fn builtin() -> Self {
        let mut registry = Self {
            user_dir: None,
            profiles: Vec::new(),
        };
        registry.reload();
        registry
    }

    /// Wbudowane i dodatkowe z `user_dir` (zwykle `<folder danych>/profile`).
    pub fn load(user_dir: impl Into<PathBuf>) -> Self {
        let mut registry = Self {
            user_dir: Some(user_dir.into()),
            profiles: Vec::new(),
        };
        registry.reload();
        registry
    }

    /// Folder dodatkowych profili.
    pub fn user_dir(&self) -> Option<&Path> {
        self.user_dir.as_deref()
    }

    /// Wczytuje profile od nowa (po pobraniu, dodaniu albo usunięciu pliku).
    pub fn reload(&mut self) {
        let mut found = Vec::new();
        for (file, text) in BUILTIN {
            match parse_profile_bytes(text.as_bytes(), SOURCE_BUILTIN) {
                Ok(profile) => merge(&mut found, profile),
                Err(e) => log::warn!("Profil bieżni {file} pominięty: {e}"),
            }
        }
        if let Some(dir) = &self.user_dir {
            for path in json_files(dir) {
                let file = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let parsed = std::fs::read(&path)
                    .map_err(|e| ProfileError(e.to_string()))
                    .and_then(|data| parse_profile_bytes(&data, SOURCE_USER));
                match parsed {
                    Ok(profile) => merge(&mut found, profile),
                    Err(e) => log::warn!("Profil bieżni {file} pominięty: {e}"),
                }
            }
        }
        self.profiles = found;
    }

    pub fn all(&self) -> &[ModelProfile] {
        &self.profiles
    }

    /// Najlepiej pasujący profil (najdłuższy początek nazwy), opcjonalnie tylko dla modułu.
    pub fn profile_for(
        &self,
        device_name: &str,
        driver: Option<DriverKind>,
    ) -> Option<&ModelProfile> {
        let mut best: Option<(usize, &ModelProfile)> = None;
        for profile in &self.profiles {
            if driver.is_some_and(|d| d != profile.driver) {
                continue;
            }
            let score = profile.matches(device_name);
            if score > 0 && best.is_none_or(|(s, _)| score > s) {
                best = Some((score, profile));
            }
        }
        best.map(|(_, p)| p)
    }

    // --- pobieranie i pliki ------------------------------------------------------------

    fn saved_bytes(&self, profile_id: &str) -> Option<Vec<u8>> {
        let same_id = |raw: &[u8]| {
            serde_json::from_slice::<Value>(raw)
                .is_ok_and(|v| v.get("id").and_then(Value::as_str) == Some(profile_id))
        };
        if let Some(dir) = &self.user_dir {
            for path in json_files(dir) {
                if let Ok(raw) = std::fs::read(&path)
                    && same_id(&raw)
                {
                    return Some(raw);
                }
            }
        }
        BUILTIN
            .iter()
            .map(|(_, text)| text.as_bytes())
            .find(|raw| same_id(raw))
            .map(<[u8]>::to_vec)
    }

    fn require_user_dir(&self) -> Result<PathBuf, DownloadError> {
        self.user_dir.clone().ok_or(DownloadError::NoUserDir)
    }

    /// Nowe i zmienione profile z repozytorium do folderu dodatkowych profili. Wołać w wątku
    /// w tle (pobieranie blokuje). `base_url` - zwykle [`default_base_url`].
    pub fn download(
        &mut self,
        fetcher: &dyn Fetcher,
        base_url: &str,
    ) -> Result<DownloadResult, DownloadError> {
        let user_dir = self.require_user_dir()?;
        let entries = parse_index(&get(fetcher, &format!("{base_url}/{INDEX}"))?)?;
        let mut result = DownloadResult::default();
        for (name, digest) in entries {
            let data = get(fetcher, &format!("{base_url}/{name}"))?;
            if sha256(&data) != digest {
                log::warn!("Profil {name}: zła suma SHA-256 - odrzucony");
                result.rejected.push(name);
                continue;
            }
            let found = match parse_profile_bytes(&data, SOURCE_DOWNLOADED) {
                Ok(found) => found,
                Err(e) => {
                    log::warn!("Profil {name} odrzucony: {e}");
                    result.rejected.push(name);
                    continue;
                }
            };
            let current = self.saved_bytes(&found.id);
            if let Some(current) = &current
                && json_bytes_equal(current, &data)
            {
                result.same += 1;
                continue;
            }
            std::fs::create_dir_all(&user_dir).map_err(DownloadError::Write)?;
            std::fs::write(user_dir.join(format!("{}.json", found.id)), &data)
                .map_err(DownloadError::Write)?;
            if current.is_some() {
                result.updated.push(found.name);
            } else {
                result.added.push(found.name);
            }
        }
        self.reload();
        log::info!(
            "Profile bieżni: nowe {}, zmienione {}, bez zmian {}, odrzucone {}",
            result.added.len(),
            result.updated.len(),
            result.same,
            result.rejected.len()
        );
        Ok(result)
    }

    /// Profil z pliku (np. od autora albo innego użytkownika) - po pełnym sprawdzeniu.
    pub fn install_from_file(&mut self, path: &Path) -> Result<ModelProfile, DownloadError> {
        let user_dir = self.require_user_dir()?;
        let data = std::fs::read(path).map_err(|_| DownloadError::Unreadable)?;
        if data.len() > MAX_BYTES {
            return Err(DownloadError::FileTooLarge);
        }
        let found = parse_profile_bytes(&data, SOURCE_USER)
            .map_err(|e| DownloadError::InvalidProfile(e.0))?;
        std::fs::create_dir_all(&user_dir).map_err(DownloadError::Write)?;
        std::fs::write(user_dir.join(format!("{}.json", found.id)), &data)
            .map_err(DownloadError::Write)?;
        self.reload();
        log::info!("Dodano profil bieżni {} ({}) z pliku", found.name, found.id);
        Ok(found)
    }

    /// Usuwa dodatkowy profil (wbudowanego nie da się usunąć - wraca jego wersja).
    pub fn remove_user_profile(&mut self, profile_id: &str) -> bool {
        let mut removed = false;
        if let Some(dir) = &self.user_dir {
            for path in json_files(dir) {
                let same = std::fs::read(&path)
                    .ok()
                    .and_then(|raw| serde_json::from_slice::<Value>(&raw).ok())
                    .is_some_and(|v| v.get("id").and_then(Value::as_str) == Some(profile_id));
                if same && std::fs::remove_file(&path).is_ok() {
                    removed = true;
                }
            }
        }
        self.reload();
        removed
    }
}

// --- pobieranie ---------------------------------------------------------------------------

pub const REPOSITORY: &str = "Damianejroo/WalkLevel-by-damnit.com.pl";
pub const BRANCH: &str = "main";
pub const INDEX: &str = "indeks.json";
pub const INDEX_FORMAT: i64 = 1;
pub const ALLOWED_HOSTS: [&str; 1] = ["raw.githubusercontent.com"];
pub const MAX_BYTES: usize = 32 * 1024;
pub const TIMEOUT_S: u64 = 15;

/// Adres folderu profili w repozytorium (gałąź main).
pub fn default_base_url() -> String {
    format!("https://raw.githubusercontent.com/{REPOSITORY}/{BRANCH}/crates/core/assets/profile")
}

/// Błąd pobierania albo dodawania profilu - komunikat dla użytkownika.
#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("{}", crate::texts::t("modele.powod.adres", &[]))]
    UnexpectedAddress,
    #[error("{}", crate::texts::t("modele.powod.przekierowanie", &[]))]
    Redirected,
    #[error("{}", crate::texts::t("modele.powod.brak_listy", &[]))]
    NotPublished,
    #[error("{}", crate::texts::t("modele.powod.kod", &[("kod", &.0.to_string())]))]
    Status(u16),
    #[error("{}", crate::texts::t("modele.powod.internet", &[]))]
    Offline,
    #[error("{}", crate::texts::t("modele.powod.za_duzy", &[]))]
    TooLarge,
    #[error("{}", crate::texts::t("modele.powod.format", &[]))]
    BadIndex,
    #[error("{}", crate::texts::t("modele.powod.zly_wpis", &[]))]
    BadEntry,
    #[error("{}", crate::texts::t("modele.plik.nieczytelny", &[]))]
    Unreadable,
    #[error("{}", crate::texts::t("modele.plik.za_duzy", &[]))]
    FileTooLarge,
    #[error("{}", crate::texts::t("modele.plik.zly", &[("szczegoly", .0)]))]
    InvalidProfile(String),
    #[error("{}", crate::texts::t("modele.blad_folder", &[]))]
    NoUserDir,
    #[error("{}", crate::texts::t("modele.blad_zapisu", &[("szczegoly", &.0.to_string())]))]
    Write(std::io::Error),
}

/// Wynik pobierania profili.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadResult {
    /// nazwy nowych modeli
    pub added: Vec<String>,
    pub updated: Vec<String>,
    /// już były (wbudowane albo pobrane wcześniej)
    pub same: usize,
    /// pliki, które nie przeszły sprawdzenia
    pub rejected: Vec<String>,
}

/// Odpowiedź serwera: adres po przekierowaniach i treść (najwyżej `MAX_BYTES + 1` bajtów).
#[derive(Debug, Clone)]
pub struct Fetched {
    pub final_url: String,
    pub body: Vec<u8>,
}

/// Błąd pobrania jednego adresu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchError {
    /// odpowiedź HTTP z błędem (np. 404)
    Status(u16),
    /// brak połączenia, przekroczony czas
    Network,
}

/// Pobieranie przez HTTP (w aplikacji [`HttpFetcher`], w testach podróbka bez sieci).
pub trait Fetcher {
    fn fetch(&self, url: &str) -> Result<Fetched, FetchError>;
}

/// Pobieranie przez reqwest (blokujące - tylko poza wątkiem tokio, np. w spawn_blocking).
pub struct HttpFetcher {
    client: reqwest::blocking::Client,
    max_bytes: usize,
}

impl HttpFetcher {
    pub fn new() -> Result<Self, DownloadError> {
        Self::with_limit("Bieznia-profile", MAX_BYTES).ok_or(DownloadError::Offline)
    }

    /// Inny nagłówek User-Agent i limit treści (pakiety języków są większe od profili).
    pub fn with_limit(user_agent: &str, max_bytes: usize) -> Option<Self> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(user_agent)
            .timeout(Duration::from_secs(TIMEOUT_S))
            .build()
            .ok()?;
        Some(Self { client, max_bytes })
    }
}

impl Fetcher for HttpFetcher {
    fn fetch(&self, url: &str) -> Result<Fetched, FetchError> {
        let response = self
            .client
            .get(url)
            .send()
            .map_err(|_| FetchError::Network)?;
        let status = response.status();
        if !status.is_success() {
            return Err(FetchError::Status(status.as_u16()));
        }
        let final_url = response.url().to_string();
        let mut body = Vec::new();
        response
            .take(self.max_bytes as u64 + 1)
            .read_to_end(&mut body)
            .map_err(|_| FetchError::Network)?;
        Ok(Fetched { final_url, body })
    }
}

fn allowed(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https" && u.host_str().is_some_and(|h| ALLOWED_HOSTS.contains(&h))
    })
}

fn get(fetcher: &dyn Fetcher, url: &str) -> Result<Vec<u8>, DownloadError> {
    if !allowed(url) {
        return Err(DownloadError::UnexpectedAddress);
    }
    let fetched = fetcher.fetch(url).map_err(|e| match e {
        FetchError::Status(404) => DownloadError::NotPublished,
        FetchError::Status(code) => DownloadError::Status(code),
        FetchError::Network => DownloadError::Offline,
    })?;
    if !allowed(&fetched.final_url) {
        return Err(DownloadError::Redirected);
    }
    if fetched.body.len() > MAX_BYTES {
        return Err(DownloadError::TooLarge);
    }
    Ok(fetched.body)
}

/// Suma treści niezależna od końców linii (git na Windows potrafi zamienić LF na CRLF).
pub fn sha256(data: &[u8]) -> String {
    let mut normalized = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if data[i] == b'\r' && data.get(i + 1) == Some(&b'\n') {
            i += 1;
            continue;
        }
        normalized.push(data[i]);
        i += 1;
    }
    hex::encode(Sha256::digest(&normalized))
}

/// `str(x)` z Pythona dla wartości z listy modeli.
fn py_str(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => py_repr(Some(other)),
    }
}

fn parse_index(data: &[u8]) -> Result<Vec<(String, String)>, DownloadError> {
    let index: Value = serde_json::from_slice(data).map_err(|_| DownloadError::BadIndex)?;
    let obj = index.as_object().ok_or(DownloadError::BadIndex)?;
    let format_ok = match obj.get("format") {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64() == Some(INDEX_FORMAT as f64),
        _ => false,
    };
    if !format_ok {
        return Err(DownloadError::BadIndex);
    }
    // Python iteruje po `index["profile"]`: pusty obiekt albo pusty tekst = pusta lista
    let empty = Vec::new();
    let list = match obj.get("profile") {
        Some(Value::Array(list)) => list,
        Some(Value::Object(map)) if map.is_empty() => &empty,
        Some(Value::String(s)) if s.is_empty() => &empty,
        _ => return Err(DownloadError::BadIndex),
    };
    let mut entries = Vec::new();
    for entry in list {
        let entry = entry.as_object().ok_or(DownloadError::BadIndex)?;
        let file = entry.get("plik").ok_or(DownloadError::BadIndex)?;
        let digest = entry.get("sha256").ok_or(DownloadError::BadIndex)?;
        entries.push((py_str(file), py_str(digest).to_lowercase()));
    }
    for (name, digest) in &entries {
        let plain = !name.contains(['/', '\\', ':'])
            && Path::new(name)
                .file_name()
                .is_some_and(|n| n == name.as_str());
        let safe = name.ends_with(".json") && plain && name != INDEX;
        if !safe || digest.chars().count() != 64 {
            return Err(DownloadError::BadEntry);
        }
    }
    Ok(entries)
}

/// Równość jak `json.loads(a) == json.loads(b)` w Pythonie (1 == 1.0 == true).
fn json_equal(a: &Value, b: &Value) -> bool {
    let number = |v: &Value| match v {
        Value::Bool(b) => Some(f64::from(u8::from(*b))),
        Value::Number(n) => n.as_f64(),
        _ => None,
    };
    match (a, b) {
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| json_equal(p, q))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| json_equal(v, w)))
        }
        _ => match (number(a), number(b)) {
            (Some(x), Some(y)) => x == y,
            _ => a == b,
        },
    }
}

fn json_bytes_equal(a: &[u8], b: &[u8]) -> bool {
    match (
        serde_json::from_slice::<Value>(a),
        serde_json::from_slice::<Value>(b),
    ) {
        (Ok(x), Ok(y)) => json_equal(&x, &y),
        _ => false,
    }
}

/// Lista modeli dla plików (nazwa, treść) - jak build_index w Pythonie.
pub fn build_index(files: &[(String, Vec<u8>)]) -> Value {
    let mut sorted: Vec<&(String, Vec<u8>)> = files.iter().filter(|(n, _)| n != INDEX).collect();
    sorted.sort_by_cached_key(|(name, _)| file_sort_key(name));
    let profile: Vec<Value> = sorted
        .iter()
        .map(|(name, data)| serde_json::json!({"plik": name, "sha256": sha256(data)}))
        .collect();
    serde_json::json!({"format": INDEX_FORMAT, "profile": profile})
}

/// Zapisuje indeks.json dla profili z folderu (po zmianie profili w repozytorium).
pub fn write_index(directory: &Path) -> std::io::Result<PathBuf> {
    let mut files = Vec::new();
    for path in json_files(directory) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        files.push((name, std::fs::read(&path)?));
    }
    let index = build_index(&files);
    let mut out = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b" ");
    let mut serializer = serde_json::Serializer::with_formatter(&mut out, formatter);
    index
        .serialize(&mut serializer)
        .map_err(std::io::Error::other)?;
    out.push(b'\n');
    let path = directory.join(INDEX);
    std::fs::write(&path, out)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builtin_profiles_are_valid_and_match_models() {
        let registry = ProfileRegistry::builtin();
        let ids: Vec<&str> = registry.all().iter().map(|p| p.id.as_str()).collect();
        for id in ["kingsmith-x21", "urevo-spacewalk-5l", "kingsmith"] {
            assert!(ids.contains(&id));
        }
        let id = |name: &str| registry.profile_for(name, None).map(|p| p.id.as_str());
        assert_eq!(id("URTM054"), Some("urevo-spacewalk-5l"));
        assert_eq!(id("URTM061"), Some("urevo"));
        assert_eq!(id("KS-HDSY-X21C"), Some("kingsmith-x21"));
        let x23 = registry.profile_for("KS-NACH-MXG", None).expect("X23");
        assert_eq!(x23.option_str("table"), Some("v7"));
        assert_eq!(id("Mi Band"), None);
    }

    #[test]
    fn bad_profiles_are_rejected() {
        let bad = [
            json!({"format": 2, "id": "a", "name": "A", "driver": "ftms"}),
            json!({"format": 1, "id": "a b", "name": "A", "driver": "ftms"}),
            json!({"format": 1, "id": "a", "name": "A", "driver": "python"}),
            json!({"format": 1, "id": "a", "name": "A", "driver": "ftms", "capabilities": {"exec": 1}}),
            json!({"format": 1, "id": "a", "name": "A", "driver": "ftms", "capabilities": {"speed_max": 99}}),
            json!({"format": 1, "id": "a", "name": "A", "driver": "ftms", "options": {"x": [1]}}),
            json!({"format": 1, "id": "a", "name": "A", "driver": "ftms",
                   "capabilities": {"speed_min": 5, "speed_max": 2}}),
        ];
        for data in bad {
            assert!(parse_profile(&data, SOURCE_BUILTIN).is_err(), "{data}");
        }
    }

    #[test]
    fn user_profile_overrides_builtin() {
        let dir = tempfile::tempdir().expect("folder");
        std::fs::write(
            dir.path().join("moj.json"),
            r#"{"format": 1, "id": "urevo-spacewalk-5l", "name": "Mój 5L", "driver": "ftms",
                "match": ["URTM054"], "capabilities": {"speed_max": 4.0}}"#,
        )
        .expect("zapis");
        std::fs::write(dir.path().join("zepsuty.json"), "{nie json").expect("zapis");
        let registry = ProfileRegistry::load(dir.path());
        let found = registry.profile_for("URTM054", None).expect("profil");
        assert_eq!(
            (found.name.as_str(), found.source.as_str()),
            ("Mój 5L", SOURCE_USER)
        );
        assert_eq!(found.apply(&Capabilities::DEFAULT).speed_max, 4.0);
    }

    #[test]
    fn index_file_round_trip() {
        let dir = tempfile::tempdir().expect("folder");
        for (name, text) in BUILTIN {
            std::fs::write(dir.path().join(name), text).expect("zapis");
        }
        let path = write_index(dir.path()).expect("indeks");
        let written = std::fs::read_to_string(path).expect("odczyt");
        assert_eq!(
            written.replace("\r\n", "\n"),
            BUILTIN_INDEX.replace("\r\n", "\n")
        );
    }

    /// Lista modeli jak `_parse_index` w Pythonie (fixtura z tools/fixtures).
    #[test]
    fn index_parsing_matches_python() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/protocols/profiles.json"
        );
        let data: Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("fixtura")).expect("JSON");
        let cases = data["index"].as_array().expect("index");
        assert!(cases.len() > 30);
        for case in cases {
            let raw = hex::decode(case["hex"].as_str().expect("hex")).expect("hex");
            let actual = match parse_index(&raw) {
                Ok(entries) => json!({"entries": entries}),
                Err(e) => json!({"error": e.to_string()}),
            };
            let mut expected = case.clone();
            if let Some(obj) = expected.as_object_mut() {
                obj.remove("hex");
                // komunikat z 0.5.2 w brzmieniu 0.6.0 (docs/texts.md, modele.*)
                if let Some(Value::String(e)) = obj.get_mut("error") {
                    *e = crate::texts::from_052(e, &["modele."]);
                }
            }
            assert_eq!(actual, expected, "{}", String::from_utf8_lossy(&raw));
        }
    }

    #[test]
    fn python_equality_of_json() {
        assert!(json_equal(
            &json!({"a": 1, "b": [true]}),
            &json!({"b": [1.0], "a": 1.0})
        ));
        assert!(!json_equal(&json!({"a": 1}), &json!({"a": "1"})));
        assert_eq!(sha256(b"a\r\nb"), sha256(b"a\nb"));
    }
}
