//! Osoby i profile gier (dane/osoby.json) - port bieznia/dane.py.
//!
//! Jeden plik osoby.json w folderze danych (zapis atomowy):
//! - wspólne: bieżnia, klawisze start/stop, szybciej i wolniej, logi - jedno dla wszystkich osób,
//! - osoby: nazwa, kolor, wygląd (motyw, duży tekst, przycisk na ekranie...) i profile gier,
//! - profil gry: nazwa, ikona (z gry albo z zestawu), kolor, ustawienia sterowania.
//!
//! Aplikacja widzi to jako jeden płaski słownik ustawień ([`Store::flat`]), złożony z części
//! wspólnej, wyglądu aktywnej osoby i ustawień jej aktywnego profilu. Zmiana klucza trafia tam,
//! gdzie ten klucz należy ([`Store::set`]).
//!
//! Wczytywanie jest wyrozumiałe jak w 0.5.x: brakujące pola dostają wartości domyślne,
//! nieznane klucze ustawień są pomijane, zły kolor zamienia się w Miętę.

pub mod names;
pub mod palette;
pub mod settings;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::py::truthy;

/// Słownik ustawień (klucz -> dowolna wartość JSON), jak `dict[str, Any]` w Pythonie.
pub type Settings = Map<String, Value>;

/// Nazwa pliku osób w folderze danych.
pub const STORE_FILE: &str = "osoby.json";
/// Folder ikon profili (PNG) w folderze danych.
pub const ICONS_DIR: &str = "ikony";
/// Folder kopii (uszkodzone pliki, ustawienia 0.2) w folderze danych.
pub const BACKUP_DIR: &str = "kopie";
/// Płaskie ustawienia z wersji 0.2 w folderze danych.
pub const OLD_SETTINGS: &str = "ustawienia.json";
/// Wersja formatu osoby.json.
pub const FORMAT_VERSION: i64 = 1;
/// Postać idąca (znak Segoe Fluent) - ikona profilu Domyślnego.
pub const DEFAULT_GLYPH: &str = "\u{e805}";

/// Klucze wspólne dla wszystkich osób.
pub const SHARED_KEYS: &[&str] = &[
    "device_address",
    "device_name",
    "belt_control",
    "quick_stop",
    "toggle_key",
    "faster_key",
    "slower_key",
    "toggle_pad",
    "simulator",
    "record",
    "detailed_logs",
    "last_version",
    "window_geometry",
];
/// Klucze wyglądu - osobne dla każdej osoby.
pub const LOOK_KEYS: &[&str] = &[
    "theme",
    "large_text",
    "always_on_top",
    "floating_button",
    "floating_pos",
    "show_person_switch",
    "show_profile_switch",
    "achievement_popups",
    "achievement_sound",
];
/// Klucze sterowania - osobne dla każdego profilu gry.
pub const PROFILE_KEYS: &[&str] = &[
    "direction",
    "output",
    "walk_key",
    "run_key",
    "stand_below",
    "run_on",
    "run_off",
    "idle_speed",
    "walk_speed",
    "run_speed",
    "pad_steps",
    "pad_idle",
    "training_levels",
    "training_level",
];

/// Gdzie należy klucz płaskich ustawień.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyScope {
    Shared,
    Look,
    Profile,
}

/// Do której części należy klucz; `None` - klucz tylko na czas działania (nie zapisywany).
pub fn key_scope(key: &str) -> Option<KeyScope> {
    if SHARED_KEYS.contains(&key) {
        Some(KeyScope::Shared)
    } else if LOOK_KEYS.contains(&key) {
        Some(KeyScope::Look)
    } else if PROFILE_KEYS.contains(&key) {
        Some(KeyScope::Profile)
    } else {
        None
    }
}

/// Domyślne nowych opcji wyglądu (gdy osoba ich jeszcze nie ustawiła).
pub fn look_defaults() -> Settings {
    let mut m = Settings::new();
    m.insert("show_person_switch".into(), Value::Bool(true));
    m.insert("show_profile_switch".into(), Value::Bool(true));
    m
}

/// Błąd zapisu danych.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Nie da się zapisać pliku {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Nie da się skopiować {from} do {to}: {source}")]
    Copy {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Nie da się zamienić danych na JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Nowy identyfikator osoby albo profilu: 10 znaków hex (z UUID v4).
pub fn new_id() -> String {
    uuid::Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(10)
        .collect()
}

/// Profil gry jednej osoby. Serializuje się do formatu osoby.json (polskie klucze).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Profile {
    pub id: String,
    #[serde(rename = "nazwa")]
    pub name: String,
    /// nazwa koloru z palety
    #[serde(rename = "kolor")]
    pub color: String,
    /// kolor dobrany z ikony gry (podpowiedź przy zmianie)
    #[serde(rename = "kolor_z_ikony")]
    pub icon_color: Option<String>,
    /// ikona z zestawu (znak czcionki Segoe Fluent)
    #[serde(rename = "glif")]
    pub glyph: Option<String>,
    /// ikona z gry: plik PNG w dane/ikony
    #[serde(rename = "ikona")]
    pub icon_file: Option<String>,
    pub exe: Option<String>,
    /// tylko klucze z [`PROFILE_KEYS`]
    #[serde(rename = "ustawienia")]
    pub settings: Settings,
    /// nazwa gry przy sesjach na koncie online (gdy konto pozwala)
    #[serde(rename = "gra_online")]
    pub online: bool,
}

impl Profile {
    /// Nowy profil z domyślnymi polami.
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            color: palette::DEFAULT.to_string(),
            icon_color: None,
            glyph: None,
            icon_file: None,
            exe: None,
            settings: Settings::new(),
            online: true,
        }
    }

    fn default_profile() -> Self {
        Self {
            glyph: Some(DEFAULT_GLYPH.to_string()),
            ..Self::new(new_id(), "Domyślny".into())
        }
    }

    /// Profil z obiektu JSON (wyrozumiale, jak Profile.from_dict w 0.5.x).
    pub fn from_json(d: &Map<String, Value>) -> Self {
        Self {
            id: str_or(d.get("id"), new_id),
            name: str_or(d.get("nazwa"), || "Profil".into()),
            color: palette::color(d.get("kolor").and_then(Value::as_str))
                .name
                .to_string(),
            icon_color: opt_string(d.get("kolor_z_ikony")),
            glyph: opt_string(d.get("glif")),
            icon_file: opt_string(d.get("ikona")),
            exe: opt_string(d.get("exe")),
            settings: filtered(d.get("ustawienia"), PROFILE_KEYS),
            online: d.get("gra_online").is_none_or(truthy),
        }
    }
}

/// Osoba: wygląd i profile gier. Zawsze ma co najmniej jeden profil.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Person {
    pub id: String,
    #[serde(rename = "nazwa")]
    pub name: String,
    /// nazwa koloru z palety
    #[serde(rename = "kolor")]
    pub color: String,
    /// tylko klucze z [`LOOK_KEYS`]
    #[serde(rename = "wyglad")]
    pub look: Settings,
    #[serde(rename = "aktywny_profil")]
    active_profile: String,
    /// niezmiennik: nigdy pusta
    #[serde(rename = "profile")]
    profiles: Vec<Profile>,
}

impl Person {
    fn new(id: String, name: String, color: String) -> Self {
        let first = Profile::default_profile();
        Self {
            id,
            name,
            color,
            look: Settings::new(),
            active_profile: first.id.clone(),
            profiles: vec![first],
        }
    }

    /// Osoba z obiektu JSON (wyrozumiale, jak Person.from_dict w 0.5.x).
    pub fn from_json(d: &Map<String, Value>) -> Self {
        let mut profiles: Vec<Profile> = match d.get("profile") {
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(Value::as_object)
                .map(Profile::from_json)
                .collect(),
            _ => Vec::new(),
        };
        if profiles.is_empty() {
            profiles.push(Profile::default_profile());
        }
        let first_id = profiles[0].id.clone();
        let mut active = str_or(d.get("aktywny_profil"), || first_id.clone());
        if profiles.iter().all(|p| p.id != active) {
            active = first_id;
        }
        Self {
            id: str_or(d.get("id"), new_id),
            name: str_or(d.get("nazwa"), || "Gracz".into()),
            color: palette::color(d.get("kolor").and_then(Value::as_str))
                .name
                .to_string(),
            look: filtered(d.get("wyglad"), LOOK_KEYS),
            active_profile: active,
            profiles,
        }
    }

    fn active_index(&self) -> usize {
        self.profiles
            .iter()
            .position(|p| p.id == self.active_profile)
            .unwrap_or(0)
    }

    /// Aktywny profil (gdy id aktywnego nie pasuje - pierwszy).
    pub fn profile(&self) -> &Profile {
        &self.profiles[self.active_index()]
    }

    fn profile_mut(&mut self) -> &mut Profile {
        let i = self.active_index();
        &mut self.profiles[i]
    }

    /// Wszystkie profile osoby (co najmniej jeden).
    pub fn profiles(&self) -> &[Profile] {
        &self.profiles
    }

    /// Id aktywnego profilu.
    pub fn active_profile_id(&self) -> &str {
        &self.active_profile
    }
}

/// Nowy profil gry ([`Store::add_profile`]).
#[derive(Debug, Clone)]
pub struct NewProfile {
    /// kolor z palety; brak - kolor z ikony, a bez niego Mięta
    pub color: Option<String>,
    pub icon_color: Option<String>,
    /// pomijany, gdy jest `icon_file`
    pub glyph: Option<String>,
    pub icon_file: Option<String>,
    pub exe: Option<String>,
    /// id profilu, z którego kopiowane są ustawienia sterowania
    pub copy_from: Option<String>,
    /// od razu ustaw jako aktywny
    pub activate: bool,
}

impl Default for NewProfile {
    fn default() -> Self {
        Self {
            color: None,
            icon_color: None,
            glyph: None,
            icon_file: None,
            exe: None,
            copy_from: None,
            activate: true,
        }
    }
}

/// Zmiany profilu ([`Store::update_profile`]); `None` - bez zmian.
/// Pola z `Option<Option<..>>`: `Some(None)` czyści wartość.
#[derive(Debug, Clone, Default)]
pub struct ProfileChanges {
    /// pusta albo z samych spacji - nazwa bez zmian
    pub name: Option<String>,
    /// nieznany kolor - Mięta
    pub color: Option<String>,
    pub icon_color: Option<Option<String>>,
    pub glyph: Option<Option<String>>,
    pub icon_file: Option<Option<String>>,
    pub exe: Option<Option<String>>,
    pub settings: Option<Settings>,
    pub online: Option<bool>,
}

/// Osoby, profile i ustawienia wspólne. Każda zmiana od razu trafia na dysk (atomowo).
///
/// Niezmiennik: zawsze co najmniej jedna osoba, a każda osoba ma co najmniej jeden profil.
#[derive(Debug, Clone)]
pub struct Store {
    path: PathBuf,
    shared: Settings,
    people: Vec<Person>,
    active_person: String,
    ask_at_start: bool,
}

#[derive(Serialize)]
struct StoreFile<'a> {
    wersja: i64,
    aktywna: &'a str,
    pytaj_przy_starcie: bool,
    wspolne: &'a Settings,
    osoby: &'a [Person],
}

impl Store {
    /// Ścieżka osoby.json w folderze danych.
    pub fn default_path(data_dir: &Path) -> PathBuf {
        data_dir.join(STORE_FILE)
    }

    // --- odczyt / zapis --------------------------------------------------------------

    /// Wczytanie osoby.json. `None`, gdy pliku nie ma (pierwsze uruchomienie - konfigurator),
    /// gdy nie ma w nim żadnej osoby albo gdy jest uszkodzony (wtedy kopia trafia do
    /// kopie/uszkodzony-<czas>.json).
    pub fn load(path: &Path) -> Option<Store> {
        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return None,
            Err(e) => {
                backup_corrupted(path, &e.to_string());
                return None;
            }
        };
        let data = match String::from_utf8(bytes)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str::<Value>(&t).map_err(|e| e.to_string()))
        {
            Ok(Value::Object(m)) => m,
            Ok(_) => {
                backup_corrupted(path, "to nie jest obiekt JSON");
                return None;
            }
            Err(e) => {
                backup_corrupted(path, &e);
                return None;
            }
        };
        Self::from_json(path, &data)
    }

    /// Dane z obiektu JSON w formacie osoby.json (`None`, gdy nie ma żadnej osoby).
    pub fn from_json(path: &Path, data: &Map<String, Value>) -> Option<Store> {
        let people: Vec<Person> = match data.get("osoby") {
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(Value::as_object)
                .map(Person::from_json)
                .collect(),
            _ => Vec::new(),
        };
        let first_id = people.first()?.id.clone();
        let mut active = str_or(data.get("aktywna"), || first_id.clone());
        if people.iter().all(|p| p.id != active) {
            active = first_id;
        }
        Some(Store {
            path: path.to_path_buf(),
            shared: filtered(data.get("wspolne"), SHARED_KEYS),
            people,
            active_person: active,
            ask_at_start: data.get("pytaj_przy_starcie").is_none_or(truthy),
        })
    }

    /// Całe dane w formacie osoby.json.
    pub fn to_json(&self) -> Result<Value, StoreError> {
        Ok(serde_json::to_value(self.file())?)
    }

    fn file(&self) -> StoreFile<'_> {
        StoreFile {
            wersja: FORMAT_VERSION,
            aktywna: &self.active_person,
            pytaj_przy_starcie: self.ask_at_start,
            wspolne: &self.shared,
            osoby: &self.people,
        }
    }

    /// Zapis na dysk: plik tymczasowy obok i podmiana (zamknięcie w trakcie nie psuje danych).
    pub fn save(&self) -> Result<(), StoreError> {
        let text = to_json_indent(&self.file(), b" ")?;
        write_atomic(&self.path, text.as_bytes())
    }

    /// Ścieżka pliku osoby.json.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn data_dir(&self) -> &Path {
        self.path.parent().unwrap_or(Path::new("."))
    }

    /// Folder ikon profili (dane/ikony).
    pub fn icons_dir(&self) -> PathBuf {
        self.data_dir().join(ICONS_DIR)
    }

    // --- tworzenie ------------------------------------------------------------------------

    /// Pierwsza osoba z profilem Domyślnym. `legacy` = płaskie ustawienia z 0.2 - każdy klucz
    /// trafia tam, gdzie należy (wspólne, wygląd osoby, profil Domyślny). Od razu zapisuje.
    pub fn create(
        path: &Path,
        person_name: &str,
        person_color: &str,
        legacy: Option<&Settings>,
    ) -> Result<Store, StoreError> {
        let empty = Settings::new();
        let legacy = legacy.unwrap_or(&empty);
        let pick = |keys: &[&str]| -> Settings {
            legacy
                .iter()
                .filter(|(k, _)| keys.contains(&k.as_str()))
                .map(clone_kv)
                .collect()
        };
        let mut person = Person::new(
            new_id(),
            person_name.to_string(),
            palette::color(Some(person_color)).name.to_string(),
        );
        person.look = pick(LOOK_KEYS);
        person.profiles[0].settings = pick(PROFILE_KEYS);
        let store = Store {
            path: path.to_path_buf(),
            shared: pick(SHARED_KEYS),
            active_person: person.id.clone(),
            people: vec![person],
            ask_at_start: true,
        };
        store.save()?;
        Ok(store)
    }

    // --- aktywna osoba i profil ----------------------------------------------------------

    fn active_index(&self) -> usize {
        self.people
            .iter()
            .position(|p| p.id == self.active_person)
            .unwrap_or(0)
    }

    /// Aktywna osoba (gdy id nie pasuje - pierwsza).
    pub fn person(&self) -> &Person {
        &self.people[self.active_index()]
    }

    fn person_mut(&mut self) -> &mut Person {
        let i = self.active_index();
        &mut self.people[i]
    }

    /// Aktywny profil aktywnej osoby.
    pub fn profile(&self) -> &Profile {
        self.person().profile()
    }

    /// Wszystkie osoby (co najmniej jedna).
    pub fn people(&self) -> &[Person] {
        &self.people
    }

    /// Id aktywnej osoby.
    pub fn active_person_id(&self) -> &str {
        &self.active_person
    }

    /// Ustawienia wspólne (tylko klucze z [`SHARED_KEYS`]).
    pub fn shared(&self) -> &Settings {
        &self.shared
    }

    /// Czy pytać o osobę przy starcie.
    pub fn ask_at_start(&self) -> bool {
        self.ask_at_start
    }

    /// Zmiana „pytaj o osobę przy starcie” (zapisuje).
    pub fn set_ask_at_start(&mut self, ask: bool) -> Result<(), StoreError> {
        self.ask_at_start = ask;
        self.save()
    }

    /// Płaskie ustawienia dla aplikacji: domyślne < wspólne < wygląd osoby < profil,
    /// plus `profile_color` i `person_color`.
    pub fn flat(&self, defaults: &Settings) -> Settings {
        let mut out = defaults.clone();
        let person = self.person();
        let profile = person.profile();
        for part in [
            &look_defaults(),
            &self.shared,
            &person.look,
            &profile.settings,
        ] {
            out.extend(part.iter().map(clone_kv));
        }
        out.insert("profile_color".into(), Value::String(profile.color.clone()));
        out.insert("person_color".into(), Value::String(person.color.clone()));
        out
    }

    fn put(&mut self, key: &str, value: Value) -> bool {
        match key_scope(key) {
            Some(KeyScope::Shared) => self.shared.insert(key.into(), value),
            Some(KeyScope::Look) => self.person_mut().look.insert(key.into(), value),
            Some(KeyScope::Profile) => self
                .person_mut()
                .profile_mut()
                .settings
                .insert(key.into(), value),
            None => return false,
        };
        true
    }

    /// Zmiana jednego klucza tam, gdzie należy (zapisuje). Klucze spoza zestawów
    /// (np. ścieżki na czas działania) są pomijane bez zapisu.
    pub fn set(&mut self, key: &str, value: Value) -> Result<(), StoreError> {
        if self.put(key, value) {
            self.save()
        } else {
            Ok(())
        }
    }

    /// Zapis płaskich ustawień aplikacji - każdy klucz tam, gdzie należy (jeden zapis).
    pub fn update_flat(&mut self, flat: &Settings) -> Result<(), StoreError> {
        for (key, value) in flat {
            self.put(key, value.clone());
        }
        self.save()
    }

    /// Przełączenie aktywnej osoby (nieznane id - bez zmian i bez zapisu).
    pub fn switch_person(&mut self, person_id: &str) -> Result<&Person, StoreError> {
        if self.people.iter().any(|p| p.id == person_id) {
            self.active_person = person_id.to_string();
            self.save()?;
        }
        Ok(self.person())
    }

    /// Przełączenie aktywnego profilu aktywnej osoby (nieznane id - bez zmian i bez zapisu).
    pub fn switch_profile(&mut self, profile_id: &str) -> Result<&Profile, StoreError> {
        if self.person().profiles.iter().any(|p| p.id == profile_id) {
            self.person_mut().active_profile = profile_id.to_string();
            self.save()?;
        }
        Ok(self.profile())
    }

    // --- osoby -----------------------------------------------------------------------------

    /// Nazwy wszystkich osób (do losowania wolnej nazwy).
    pub fn names(&self) -> Vec<String> {
        self.people.iter().map(|p| p.name.clone()).collect()
    }

    /// Nowa osoba, od razu aktywna (zapisuje). `copy_profiles_from` = id osoby, której wygląd
    /// i profile (z ikonami, pod nowymi id) dostaje nowa; bez tego - sam profil Domyślny.
    pub fn add_person(
        &mut self,
        name: &str,
        color: &str,
        copy_profiles_from: Option<&str>,
    ) -> Result<&Person, StoreError> {
        let name = match name.trim() {
            "" => "Gracz",
            n => n,
        };
        let mut person = Person::new(
            new_id(),
            name.to_string(),
            palette::color(Some(color)).name.to_string(),
        );
        let source = copy_profiles_from
            .and_then(|id| self.people.iter().find(|p| p.id == id))
            .map(|p| (p.look.clone(), p.profiles.clone()));
        if let Some((look, profiles)) = source {
            person.look = look;
            let copies: Vec<Profile> = profiles
                .iter()
                .map(|p| self.copy_profile(p, None))
                .collect();
            if !copies.is_empty() {
                person.active_profile = copies[0].id.clone();
                person.profiles = copies;
            }
        }
        self.active_person = person.id.clone();
        self.people.push(person);
        self.save()?;
        Ok(self.person())
    }

    /// Zmiana nazwy (pusta - bez zmian) i opcjonalnie koloru osoby (zapisuje).
    pub fn rename_person(
        &mut self,
        person_id: &str,
        name: &str,
        color: Option<&str>,
    ) -> Result<(), StoreError> {
        for p in self.people.iter_mut().filter(|p| p.id == person_id) {
            if !name.trim().is_empty() {
                p.name = name.trim().to_string();
            }
            if let Some(c) = color.filter(|c| !c.is_empty()) {
                p.color = palette::color(Some(c)).name.to_string();
            }
        }
        self.save()
    }

    /// Usunięcie osoby (zapisuje). Ostatniej osoby nie da się usunąć - wtedy `false`.
    pub fn remove_person(&mut self, person_id: &str) -> Result<bool, StoreError> {
        if self.people.len() <= 1 || self.people.iter().all(|p| p.id == person_id) {
            return Ok(false);
        }
        self.people.retain(|p| p.id != person_id);
        if self.active_person == person_id {
            self.active_person = self.people[0].id.clone();
        }
        self.save()?;
        Ok(true)
    }

    // --- profile gier --------------------------------------------------------------------

    fn copy_profile(&self, src: &Profile, name: Option<String>) -> Profile {
        let mut dup = src.clone();
        dup.id = new_id();
        if let Some(n) = name.filter(|n| !n.is_empty()) {
            dup.name = n;
        }
        // jak `if src.icon_file:` w 0.5.x - pusta nazwa zostaje bez kopiowania
        if let Some(icon) = src.icon_file.as_deref().filter(|f| !f.is_empty()) {
            dup.icon_file = self.copy_icon(icon, &dup.id);
        }
        dup
    }

    fn copy_icon(&self, icon_file: &str, new_profile_id: &str) -> Option<String> {
        let dir = self.icons_dir();
        let dst_name = format!("{new_profile_id}.png");
        match fs::copy(dir.join(icon_file), dir.join(&dst_name)) {
            Ok(_) => Some(dst_name),
            Err(e) => {
                log::warn!("Nie da się skopiować ikony {icon_file}: {e}");
                None
            }
        }
    }

    fn find_profile(&self, profile_id: &str) -> Option<&Profile> {
        self.person()
            .profiles
            .iter()
            .find(|p| p.id == profile_id)
            .or_else(|| {
                self.people
                    .iter()
                    .flat_map(|p| p.profiles.iter())
                    .find(|p| p.id == profile_id)
            })
    }

    /// Nowy profil gry aktywnej osoby (zapisuje). Pusta nazwa - „Profil”.
    pub fn add_profile(&mut self, name: &str, new: NewProfile) -> Result<&Profile, StoreError> {
        let settings = new
            .copy_from
            .as_deref()
            .and_then(|id| self.find_profile(id))
            .map(|p| p.settings.clone())
            .unwrap_or_default();
        let color = new
            .color
            .filter(|c| !c.is_empty())
            .or_else(|| new.icon_color.clone());
        let name = match name.trim() {
            "" => "Profil",
            n => n,
        };
        let profile = Profile {
            color: palette::color(color.as_deref()).name.to_string(),
            icon_color: new.icon_color,
            glyph: if new.icon_file.as_deref().is_some_and(|f| !f.is_empty()) {
                None
            } else {
                new.glyph
            },
            icon_file: new.icon_file,
            exe: new.exe,
            settings,
            ..Profile::new(new_id(), name.to_string())
        };
        let id = profile.id.clone();
        let person = self.person_mut();
        person.profiles.push(profile);
        if new.activate {
            person.active_profile = id.clone();
        }
        self.save()?;
        let person = self.person();
        Ok(person
            .profiles
            .iter()
            .find(|p| p.id == id)
            .unwrap_or(person.profile()))
    }

    /// Zmiana profilu aktywnej osoby (zapisuje).
    pub fn update_profile(
        &mut self,
        profile_id: &str,
        changes: ProfileChanges,
    ) -> Result<(), StoreError> {
        for p in self
            .person_mut()
            .profiles
            .iter_mut()
            .filter(|p| p.id == profile_id)
        {
            let ch = changes.clone();
            if let Some(name) = ch.name.filter(|n| !n.trim().is_empty()) {
                p.name = name.trim().to_string();
            }
            if let Some(color) = ch.color {
                p.color = palette::color(Some(&color)).name.to_string();
            }
            if let Some(v) = ch.icon_color {
                p.icon_color = v;
            }
            if let Some(v) = ch.glyph {
                p.glyph = v;
            }
            if let Some(v) = ch.icon_file {
                p.icon_file = v;
            }
            if let Some(v) = ch.exe {
                p.exe = v;
            }
            if let Some(v) = ch.settings {
                p.settings = v;
            }
            if let Some(v) = ch.online {
                p.online = v;
            }
        }
        self.save()
    }

    /// Kopia profilu aktywnej osoby „<nazwa> (kopia)” z ustawieniami i ikoną (zapisuje).
    /// `None`, gdy nie ma takiego profilu.
    pub fn duplicate_profile(&mut self, profile_id: &str) -> Result<Option<&Profile>, StoreError> {
        let Some(src) = self
            .person()
            .profiles
            .iter()
            .find(|p| p.id == profile_id)
            .cloned()
        else {
            return Ok(None);
        };
        let dup = self.copy_profile(&src, Some(format!("{} (kopia)", src.name)));
        let id = dup.id.clone();
        self.person_mut().profiles.push(dup);
        self.save()?;
        Ok(self.person().profiles.iter().find(|p| p.id == id))
    }

    /// Usunięcie profilu aktywnej osoby (zapisuje). Ostatniego profilu nie da się usunąć.
    pub fn remove_profile(&mut self, profile_id: &str) -> Result<bool, StoreError> {
        let person = self.person_mut();
        if person.profiles.len() <= 1 || person.profiles.iter().all(|p| p.id == profile_id) {
            return Ok(false);
        }
        person.profiles.retain(|p| p.id != profile_id);
        if person.active_profile == profile_id {
            person.active_profile = person.profiles[0].id.clone();
        }
        self.save()?;
        Ok(true)
    }
}

// --- przejście z 0.2 -------------------------------------------------------------------------

/// Płaskie ustawienia 0.2 z dane/ustawienia.json (`None`, gdy to nowa instalacja).
pub fn legacy_settings(path: &Path) -> Option<Settings> {
    let text = fs::read_to_string(path).ok()?;
    match serde_json::from_str(&text).ok()? {
        Value::Object(m) => Some(m),
        _ => None,
    }
}

/// Kopia ustawień 0.2 przed migracją do kopie/ustawienia-0.2.json (oryginał zostaje -
/// stara wersja dalej go czyta). `None`, gdy pliku 0.2 nie ma.
pub fn backup_legacy(path: &Path) -> Result<Option<PathBuf>, StoreError> {
    if !path.exists() {
        return Ok(None);
    }
    let dir = path.parent().unwrap_or(Path::new(".")).join(BACKUP_DIR);
    fs::create_dir_all(&dir).map_err(|source| StoreError::Write {
        path: dir.clone(),
        source,
    })?;
    let dst = dir.join("ustawienia-0.2.json");
    if !dst.exists() {
        fs::copy(path, &dst).map_err(|source| StoreError::Copy {
            from: path.to_path_buf(),
            to: dst.clone(),
            source,
        })?;
    }
    Ok(Some(dst))
}

// --- pomocnicze ------------------------------------------------------------------------------

/// Zapis atomowy: plik .tmp obok i podmiana (rename na Windows zastępuje istniejący plik).
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    let err = |source| StoreError::Write {
        path: path.to_path_buf(),
        source,
    };
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir).map_err(err)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes).map_err(err)?;
    fs::rename(&tmp, path).map_err(err)
}

/// JSON z wcięciem jak `json.dumps(indent=..., ensure_ascii=False)` w Pythonie.
pub(crate) fn to_json_indent<T: Serialize>(value: &T, indent: &[u8]) -> Result<String, StoreError> {
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(indent);
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    value.serialize(&mut ser)?;
    // serde_json pisze zawsze poprawny UTF-8
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn backup_corrupted(path: &Path, reason: &str) {
    log::warn!(
        "Nie da się odczytać {} ({reason}) - zapisuję kopię i zaczynam od nowa",
        path.display()
    );
    let dir = path.parent().unwrap_or(Path::new(".")).join(BACKUP_DIR);
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let copied = fs::create_dir_all(&dir)
        .and_then(|()| fs::copy(path, dir.join(format!("uszkodzony-{secs}.json"))));
    if let Err(e) = copied {
        log::warn!("Kopia uszkodzonego pliku się nie udała: {e}");
    }
}

fn clone_kv((k, v): (&String, &Value)) -> (String, Value) {
    (k.clone(), v.clone())
}

/// Tekst wartości jak `str(x)` w Pythonie (dla tekstu, liczb i prawda/fałsz).
fn py_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Null => "None".into(),
        other => other.to_string(),
    }
}

/// `str(d.get(k) or domyślna)`.
fn str_or(v: Option<&Value>, default: impl FnOnce() -> String) -> String {
    match v {
        Some(v) if truthy(v) => py_str(v),
        _ => default(),
    }
}

/// Pole tekstowe, które może być puste (null).
fn opt_string(v: Option<&Value>) -> Option<String> {
    match v {
        None | Some(Value::Null) => None,
        Some(v) => Some(py_str(v)),
    }
}

/// `{k: v for k, v in dict(d.get(k) or {}).items() if k in keys}`.
fn filtered(v: Option<&Value>, keys: &[&str]) -> Settings {
    match v {
        Some(Value::Object(m)) => m
            .iter()
            .filter(|(k, _)| keys.contains(&k.as_str()))
            .map(clone_kv)
            .collect(),
        _ => Settings::new(),
    }
}

#[cfg(test)]
mod tests;
