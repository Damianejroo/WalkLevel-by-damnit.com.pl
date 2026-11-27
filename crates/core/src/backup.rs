//! Kopia danych: eksport i import osób, profili gier i wyników (plik .bieznia = ZIP z JSON).
//! Port bieznia/kopia.py - ten sam format pliku (wersja 2), odczyt także wersji 1 z 0.3.x.
//!
//! Eksport z filtrami: które osoby i ich profile, ustawienia i profile, wyniki (od daty), logi.
//! Import: podgląd pliku ([`read`]), te same filtry, konflikt osoby: połącz albo dodaj jako nową.
//! Przed importem kopia obecnych danych (dane/kopie/przed-importem-...), do przywrócenia.
//! Plik z zewnątrz jest sprawdzany: rozmiar, nazwy w archiwum, ikony tylko PNG.
//!
//! Zawartość pliku:
//! - `manifest.json` - `format`, `wersja`, `aplikacja`, `utworzono`, `zawartosc{ustawienia, wyniki, logi}`,
//! - `osoby.json` - `{"osoby": [...]}` w formacie osoby.json (bez ustawień: profile tylko z id,
//!   nazwą i kolorem, pusty wygląd),
//! - `historia.jsonl` - wiersz = obiekt z kolumnami tabeli `sesje` (jak `json.dumps` w Pythonie),
//! - `ikony/<nazwa>.png`, `logi/<plik>.log` (końcówki logów).

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use chrono::{Local, NaiveDate, NaiveTime, TimeZone};
use regex::Regex;
use rusqlite::Connection;
use serde::Serialize;
use serde_json::{Map, Value};

// liczby z wartości JSON jak float()/int() w Pythonie (napis z liczbą też - jak SQLite REAL)
use crate::compat::{py_float as json_f64, py_int as json_i64};

use crate::history::{History, HistoryError, VISIBILITY, new_session};
use crate::model::Session;
use crate::store::{
    self, BACKUP_DIR, DEFAULT_GLYPH, ICONS_DIR, Person, Profile, Settings, Store, StoreError,
};
use crate::texts::t;

pub const FORMAT: &str = "bieznia-kopia";
/// 2: sesje z bieżnią, źródłem i widocznością, czasy *_utc_s
pub const FORMAT_VERSION: i64 = 2;
pub const SUFFIX: &str = ".bieznia";
/// Większych plików kopii nie otwieramy.
pub const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;
/// Z każdego logu trafia do kopii tylko końcówka.
pub const MAX_LOG_BYTES: u64 = 2_000_000;
pub const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";
/// Folder kopii sprzed importu (w dane/kopie) zaczyna się od tego.
pub const BEFORE_IMPORT: &str = "przed-importem-";
/// Ochrona przed „bombą ZIP”: więcej po rozpakowaniu wszystkich czytanych plików razem
/// (manifest, osoby, historia, ikony) to uszkodzona kopia.
const MAX_UNPACKED_BYTES: u64 = 256 * 1024 * 1024;

/// Jedyne nazwy dozwolone w archiwum (jak SAFE_NAME w 0.5.x).
/// Wzorce są stałe; gdyby się nie skompilowały, nic nie przechodzi (bezpieczniej).
static SAFE_NAME: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"^(manifest\.json|osoby\.json|historia\.jsonl|ikony/[\w-]+\.png|logi/[\w.-]+)$")
        .ok()
});

/// Id profilu, pod którym wolno zapisać ikonę `<id>.png` (bez ścieżek z pliku kopii).
static SAFE_ID: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"^[\w-]+$").ok());

fn matches(re: &LazyLock<Option<Regex>>, text: &str) -> bool {
    re.as_ref().is_some_and(|r| r.is_match(text))
}

/// Błąd kopii. [`BackupError::Invalid`] niesie komunikat dla użytkownika (CopyError w 0.5.x).
#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("{0}")]
    Invalid(String),
    #[error("Błąd pliku: {0}")]
    Io(#[from] io::Error),
    #[error("Błąd archiwum kopii: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("{0}")]
    Store(#[from] StoreError),
    #[error("{0}")]
    History(#[from] HistoryError),
    #[error("Kopia bazy historii: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("Błąd JSON: {0}")]
    Json(#[from] serde_json::Error),
}

fn invalid(message: &str) -> BackupError {
    BackupError::Invalid(message.to_string())
}

fn damaged<E: std::fmt::Display>(err: E) -> BackupError {
    log::warn!("Kopia: uszkodzony plik ({err})");
    invalid(&t("kopia.blad.uszkodzony", &[]))
}

// --- eksport ----------------------------------------------------------------------------------

/// Co zapisać w kopii.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportChoice {
    /// osoba -> wybrane id profili (`None` = wszystkie profile); osób spoza mapy nie ma w kopii
    pub people: BTreeMap<String, Option<HashSet<String>>>,
    /// ustawienia sterowania, wygląd i ikony profili
    pub settings: bool,
    pub results: bool,
    pub logs: bool,
    /// wyniki od tej chwili (start sesji, sekundy UTC); 0 = wszystkie
    pub since: f64,
}

impl ExportChoice {
    /// Wybór z domyślnymi polami jak w 0.5.x: ustawienia i wyniki, bez logów, wszystkie sesje.
    pub fn new(people: BTreeMap<String, Option<HashSet<String>>>) -> Self {
        Self {
            people,
            settings: true,
            results: true,
            logs: false,
            since: 0.0,
        }
    }
}

/// Początek zakresu wyników z kontraktu (`ExportRange`: `all` | `year` | `month`) dla
/// [`ExportChoice::since`]: lokalna północ 365 albo 30 dni temu (jak export_choice w 0.5.x),
/// inny zakres - wszystkie wyniki (0).
pub fn range_since(range: &str) -> f64 {
    range_since_on(range, Local::now().date_naive(), &Local)
}

fn range_since_on<Tz: TimeZone>(range: &str, today: NaiveDate, tz: &Tz) -> f64 {
    let days = match range {
        "year" => 365,
        "month" => 30,
        _ => return 0.0,
    };
    let Some(day) = today.checked_sub_days(chrono::Days::new(days)) else {
        return 0.0;
    };
    let midnight = day.and_time(NaiveTime::MIN);
    match tz.from_local_datetime(&midnight).earliest() {
        Some(t) => t.timestamp() as f64,
        // północ wypada w przeskoku zegara 00:00 -> 01:00 (rzadkie strefy): chwila zmiany
        // czasu, czyli 01:00 po zmianie (jak datetime.timestamp() dla fold=0)
        None => tz
            .from_local_datetime(&(midnight + chrono::Duration::hours(1)))
            .earliest()
            .map_or(0.0, |t| t.timestamp() as f64),
    }
}

/// Liczby do podsumowania eksportu (na żywo przy zaznaczaniu i po zapisie).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSummary {
    pub people: u32,
    pub profiles: u32,
    pub sessions: u32,
    /// rozmiar zapisanego pliku (0 w samym podsumowaniu)
    pub bytes: u64,
}

/// Osoba wybrana do kopii z wybranymi profilami.
struct Selected<'a> {
    person: &'a Person,
    profiles: Vec<&'a Profile>,
    /// wszystkie profile osoby - wtedy też sesje profili już usuniętych
    all_profiles: bool,
}

fn selected<'a>(store: &'a Store, choice: &ExportChoice) -> Vec<Selected<'a>> {
    store
        .people()
        .iter()
        .filter_map(|person| {
            let wanted = choice.people.get(&person.id)?;
            let profiles: Vec<&Profile> = person
                .profiles()
                .iter()
                .filter(|pr| wanted.as_ref().is_none_or(|w| w.contains(&pr.id)))
                .collect();
            let all_profiles = profiles.len() == person.profiles().len();
            Some(Selected {
                person,
                profiles,
                all_profiles,
            })
        })
        .collect()
}

fn selected_sessions(
    history: &History,
    since: f64,
    sel: &Selected,
) -> Result<Vec<Session>, HistoryError> {
    // Cała osoba: wszystkie jej sesje, także profili usuniętych wcześniej (w 0.5.x takie sesje
    // po cichu wypadały z pełnej kopii).
    if sel.all_profiles {
        return history.sessions(since, Some(&sel.person.id), None, None);
    }
    let ids: HashSet<String> = sel.profiles.iter().map(|p| p.id.clone()).collect();
    history.sessions(since, Some(&sel.person.id), Some(&ids), None)
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Ile osób, profili i sesji trafi do kopii (profile tylko z ustawieniami, sesje tylko z wynikami).
pub fn summarize(
    store: &Store,
    history: Option<&History>,
    choice: &ExportChoice,
) -> Result<ExportSummary, BackupError> {
    let people = selected(store, choice);
    let mut sessions = 0;
    if choice.results
        && let Some(history) = history
    {
        for sel in &people {
            sessions += selected_sessions(history, choice.since, sel)?.len();
        }
    }
    let profiles = if choice.settings {
        people.iter().map(|p| p.profiles.len()).sum()
    } else {
        0
    };
    Ok(ExportSummary {
        people: count(people.len()),
        profiles: count(profiles),
        sessions: count(sessions),
        bytes: 0,
    })
}

/// Osoba w formacie osoby.json (`P` - pełny profil albo skrót bez ustawień).
#[derive(Debug, Clone, Serialize)]
struct PersonJson<P> {
    id: String,
    #[serde(rename = "nazwa")]
    name: String,
    #[serde(rename = "kolor")]
    color: String,
    #[serde(rename = "wyglad")]
    look: Settings,
    #[serde(rename = "aktywny_profil")]
    active_profile: String,
    #[serde(rename = "profile")]
    profiles: Vec<P>,
}

impl<P> PersonJson<P> {
    fn with_profiles(person: &Person, profiles: Vec<P>) -> Self {
        Self {
            id: person.id.clone(),
            name: person.name.clone(),
            color: person.color.clone(),
            look: person.look.clone(),
            active_profile: person.active_profile_id().to_string(),
            profiles,
        }
    }
}

/// Profil w kopii: pełny albo (bez ustawień) tylko id, nazwa i kolor.
#[derive(Serialize)]
#[serde(untagged)]
enum ProfileOut<'a> {
    Full(&'a Profile),
    Short {
        id: &'a str,
        nazwa: &'a str,
        kolor: &'a str,
    },
}

#[derive(Serialize)]
struct PeopleFile<P> {
    osoby: Vec<PersonJson<P>>,
}

#[derive(Serialize)]
struct Manifest<'a> {
    format: &'a str,
    wersja: i64,
    aplikacja: &'a str,
    utworzono: String,
    zawartosc: Content,
}

#[derive(Serialize)]
struct Content {
    ustawienia: bool,
    wyniki: bool,
    logi: bool,
}

/// Zapis kopii do pliku `out` (ZIP). `logs_dir` - folder logs/ (do zaznaczenia „logi”).
/// Archiwum powstaje obok jako `<out>.tmp` i dopiero gotowe zastępuje `out` - błąd w trakcie
/// nie zostawia uciętego pliku kopii (ani nie psuje poprzedniej kopii pod tą nazwą).
pub fn export(
    store: &Store,
    history: Option<&History>,
    choice: &ExportChoice,
    out: &Path,
    logs_dir: &Path,
) -> Result<ExportSummary, BackupError> {
    let people = selected(store, choice);
    if people.is_empty() {
        return Err(invalid(&t("kopia.blad.brak_osoby", &[])));
    }
    if !(choice.settings || choice.results || choice.logs) {
        return Err(invalid(&t("kopia.blad.brak_tresci", &[])));
    }
    let mut summary = summarize(store, history, choice)?;
    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir)?;
    }
    let mut tmp_name = out.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".tmp");
    let tmp = out.with_file_name(tmp_name);
    let written = write_archive(store, history, choice, &people, &tmp, logs_dir)
        .and_then(|()| fs::rename(&tmp, out).map_err(BackupError::from));
    if let Err(e) = written {
        // sprzątanie po nieudanym zapisie; brak pliku tymczasowego to nie błąd
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    summary.bytes = fs::metadata(out)?.len();
    log::info!(
        "Eksport: {} osób, {} profili, {} sesji -> {}",
        summary.people,
        summary.profiles,
        summary.sessions,
        out.file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default()
    );
    Ok(summary)
}

/// Zawartość kopii zapisana do pliku `path`.
fn write_archive(
    store: &Store,
    history: Option<&History>,
    choice: &ExportChoice,
    people: &[Selected],
    path: &Path,
    logs_dir: &Path,
) -> Result<(), BackupError> {
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(crate::reports::zip_time());
    let mut zip = zip::ZipWriter::new(File::create(path)?);

    let manifest = Manifest {
        format: FORMAT,
        wersja: FORMAT_VERSION,
        aplikacja: crate::paths::app_version(),
        utworzono: Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
        zawartosc: Content {
            ustawienia: choice.settings,
            wyniki: choice.results,
            logi: choice.logs,
        },
    };
    zip.start_file("manifest.json", options)?;
    zip.write_all(store::to_json_indent(&manifest, b" ")?.as_bytes())?;

    let icons_dir = store.icons_dir();
    let mut icons_written = HashSet::new();
    let mut people_out = Vec::new();
    for sel in people {
        let profiles = sel
            .profiles
            .iter()
            .map(|pr| {
                if choice.settings {
                    ProfileOut::Full(pr)
                } else {
                    ProfileOut::Short {
                        id: &pr.id,
                        nazwa: &pr.name,
                        kolor: &pr.color,
                    }
                }
            })
            .collect();
        let mut person = PersonJson::with_profiles(sel.person, profiles);
        if !choice.settings {
            person.look = Settings::new();
        }
        people_out.push(person);
        if !choice.settings {
            continue;
        }
        for pr in &sel.profiles {
            let Some(icon) = pr.icon_file.as_deref().filter(|f| !f.is_empty()) else {
                continue;
            };
            let src = icons_dir.join(icon);
            if !src.is_file() {
                continue;
            }
            let name = format!("ikony/{}", icon_key(icon));
            // nazwa, której odczyt by nie przyjął, albo ta sama ikona drugi raz - pomijamy
            if !matches(&SAFE_NAME, &name) {
                log::warn!("Eksport: pomijam ikonę o nazwie {icon}");
                continue;
            }
            if icons_written.insert(name.clone()) {
                zip.start_file(name, options)?;
                zip.write_all(&fs::read(&src)?)?;
            }
        }
    }
    zip.start_file("osoby.json", options)?;
    let people_file = PeopleFile { osoby: people_out };
    zip.write_all(store::to_json_indent(&people_file, b" ")?.as_bytes())?;

    if choice.results
        && let Some(history) = history
    {
        let mut text = String::new();
        for sel in people {
            for s in selected_sessions(history, choice.since, sel)? {
                text.push_str(&session_to_json(&s)?);
                text.push('\n');
            }
        }
        zip.start_file("historia.jsonl", options)?;
        zip.write_all(text.as_bytes())?;
    }

    if choice.logs && logs_dir.is_dir() {
        let mut logs: Vec<PathBuf> = fs::read_dir(logs_dir)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("log")))
            .collect();
        logs.sort_by_key(|p| p.to_string_lossy().to_lowercase());
        for file in logs {
            let name = file
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let name = format!("logi/{name}");
            // nazwa, której odczyt by nie przyjął (np. ze spacją), zepsułaby całą kopię
            if !matches(&SAFE_NAME, &name) {
                log::warn!("Eksport: pomijam log {name}");
                continue;
            }
            zip.start_file(name, options)?;
            zip.write_all(&tail(&file, MAX_LOG_BYTES)?)?;
        }
    }
    zip.finish()?.sync_all()?;
    Ok(())
}

/// Ostatnie `max` bajtów pliku.
fn tail(path: &Path, max: u64) -> io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    if len > max {
        file.seek(SeekFrom::Start(len - max))?;
    }
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    Ok(buf)
}

/// Nazwa ikony w archiwum: `<nazwa pliku bez rozszerzenia>.png`.
fn icon_key(icon_file: &str) -> String {
    let stem = Path::new(icon_file)
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    format!("{stem}.png")
}

// --- sesje w historia.jsonl --------------------------------------------------------------

/// Wiersz historia.jsonl: kolumny tabeli `sesje` w ich kolejności (kontrakt z 0.5.x).
#[derive(Serialize)]
struct SessionRow<'a> {
    id: &'a str,
    osoba: &'a str,
    osoba_nazwa: &'a str,
    profil: &'a str,
    profil_nazwa: &'a str,
    start_utc_s: f64,
    koniec_utc_s: f64,
    kroki: i64,
    dystans_km: f64,
    ruch_s: f64,
    bieznia_model: &'a str,
    sterownik: &'a str,
    bieznia_profil: &'a str,
    firmware: &'a str,
    wersja_aplikacji: &'a str,
    zrodlo: &'a str,
    widocznosc: &'a str,
    strefa_min: i64,
    zmieniono_utc_s: f64,
    tryb: &'a str,
}

/// JSON jak `json.dumps(obj)` w Pythonie: separatory ", " i ": ", znaki spoza ASCII
/// jako \uXXXX, liczby zmiennoprzecinkowe jak `repr(float)`.
struct PyCompact;

impl serde_json::ser::Formatter for PyCompact {
    fn begin_array_value<W: ?Sized + Write>(&mut self, w: &mut W, first: bool) -> io::Result<()> {
        if first { Ok(()) } else { w.write_all(b", ") }
    }

    fn begin_object_key<W: ?Sized + Write>(&mut self, w: &mut W, first: bool) -> io::Result<()> {
        if first { Ok(()) } else { w.write_all(b", ") }
    }

    fn begin_object_value<W: ?Sized + Write>(&mut self, w: &mut W) -> io::Result<()> {
        w.write_all(b": ")
    }

    fn write_f64<W: ?Sized + Write>(&mut self, w: &mut W, value: f64) -> io::Result<()> {
        w.write_all(crate::reports::py_float(value).as_bytes())
    }

    fn write_string_fragment<W: ?Sized + Write>(
        &mut self,
        w: &mut W,
        fragment: &str,
    ) -> io::Result<()> {
        let mut units = [0u16; 2];
        for c in fragment.chars() {
            if (' '..='~').contains(&c) {
                w.write_all(&[c as u8])?;
            } else {
                for u in c.encode_utf16(&mut units) {
                    write!(w, "\\u{u:04x}")?;
                }
            }
        }
        Ok(())
    }
}

/// Jeden wiersz historia.jsonl (bez znaku końca linii), bajt w bajt jak w 0.5.x.
pub fn session_to_json(s: &Session) -> Result<String, serde_json::Error> {
    let row = SessionRow {
        id: &s.id,
        osoba: &s.person,
        osoba_nazwa: &s.person_name,
        profil: &s.profile,
        profil_nazwa: &s.profile_name,
        start_utc_s: s.start,
        koniec_utc_s: s.end,
        kroki: s.steps,
        dystans_km: s.distance_km,
        ruch_s: s.active_s,
        bieznia_model: &s.device_model,
        sterownik: &s.driver,
        bieznia_profil: &s.device_profile,
        firmware: &s.firmware,
        wersja_aplikacji: &s.app_version,
        zrodlo: &s.source,
        widocznosc: &s.visibility,
        strefa_min: s.tz_offset_min,
        zmieniono_utc_s: s.updated,
        tryb: &s.mode,
    };
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, PyCompact);
    row.serialize(&mut ser)?;
    // formatter pisze same znaki ASCII
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Tekst z wartości JSON (napis albo liczba); `None` = zły typ.
fn json_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) if n.is_f64() => n.as_f64().map(crate::reports::py_float),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// More steps than anyone walks in one session (24 h at 250 steps a minute is 360 000).
pub const MAX_SESSION_STEPS: i64 = 1_000_000;

/// Sesja z historia.jsonl dowolnej wersji kopii (brakujące pola - domyślne). Wersja 1 (0.3.x)
/// miała dziesięć pól, a czasy pod nazwami start / koniec. `None` - brak wymaganego pola
/// (pierwsze siedem) albo wartość złego typu, a non-finite number or steps out of range.
pub fn session_from_json(d: &Map<String, Value>) -> Option<Session> {
    // SQLite would store NaN as NULL, which the import silently drops
    let json_f64 = |v: &Value| json_f64(v).filter(|f| f.is_finite());
    let get = |name: &str| -> Option<&Value> {
        d.get(name).or_else(|| match name {
            "start_utc_s" => d.get("start"),
            "koniec_utc_s" => d.get("koniec"),
            _ => None,
        })
    };
    let text = |name: &str| get(name).and_then(json_text);
    let mut s = new_session(
        &text("id")?,
        &text("osoba")?,
        &text("osoba_nazwa")?,
        &text("profil")?,
        &text("profil_nazwa")?,
        json_f64(get("start_utc_s")?)?,
        json_f64(get("koniec_utc_s")?)?,
    );
    if let Some(v) = get("kroki") {
        s.steps = json_i64(v).filter(|n| (0..=MAX_SESSION_STEPS).contains(n))?;
    }
    if let Some(v) = get("dystans_km") {
        s.distance_km = json_f64(v)?;
    }
    if let Some(v) = get("ruch_s") {
        s.active_s = json_f64(v)?;
    }
    if let Some(v) = get("strefa_min") {
        s.tz_offset_min = json_i64(v)?;
    }
    if let Some(v) = get("zmieniono_utc_s") {
        s.updated = json_f64(v)?;
    }
    for (name, field) in [
        ("bieznia_model", &mut s.device_model),
        ("sterownik", &mut s.driver),
        ("bieznia_profil", &mut s.device_profile),
        ("firmware", &mut s.firmware),
        ("wersja_aplikacji", &mut s.app_version),
        ("zrodlo", &mut s.source),
        ("tryb", &mut s.mode),
    ] {
        if let Some(v) = get(name) {
            *field = json_text(v)?;
        }
    }
    // nieznana widoczność (także brak) - prywatna
    if let Some(v) = get("widocznosc") {
        s.visibility = json_text(v).unwrap_or_default();
    }
    if !VISIBILITY.contains(&s.visibility.as_str()) {
        s.visibility = "prywatna".into();
    }
    Some(s)
}

// --- odczyt pliku ---------------------------------------------------------------------------

/// Podgląd pliku kopii (bez zmian w danych).
#[derive(Debug, Clone, PartialEq)]
pub struct Preview {
    pub path: PathBuf,
    pub app_version: String,
    pub has_settings: bool,
    pub has_results: bool,
    pub people: Vec<Person>,
    pub sessions: Vec<Session>,
    /// `<nazwa>.png` -> zawartość (tylko prawdziwe PNG)
    pub icons: BTreeMap<String, Vec<u8>>,
}

impl Preview {
    /// Sesje jednej osoby z pliku.
    pub fn sessions_of(&self, person_id: &str) -> Vec<&Session> {
        self.sessions
            .iter()
            .filter(|s| s.person == person_id)
            .collect()
    }
}

/// Jeden plik z archiwum. `budget` - ile bajtów wolno jeszcze rozpakować (wspólny limit
/// dla całego pliku kopii, zmniejszany o każdy przeczytany plik).
fn entry<R: Read + Seek>(
    zip: &mut zip::ZipArchive<R>,
    name: &str,
    budget: &mut u64,
) -> Result<Vec<u8>, BackupError> {
    let file = zip.by_name(name).map_err(damaged)?;
    let mut buf = Vec::new();
    file.take(budget.saturating_add(1))
        .read_to_end(&mut buf)
        .map_err(damaged)?;
    let len = buf.len() as u64;
    if len > *budget {
        return Err(damaged(format!("{name}: za dużo danych po rozpakowaniu")));
    }
    *budget -= len;
    Ok(buf)
}

fn json_object(bytes: &[u8]) -> Result<Map<String, Value>, BackupError> {
    match serde_json::from_slice::<Value>(bytes).map_err(damaged)? {
        Value::Object(m) => Ok(m),
        _ => Err(damaged("to nie jest obiekt JSON")),
    }
}

/// Tekst wartości jak `str(x)` w Pythonie.
fn py_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Null => "None".into(),
        other => json_text(other).unwrap_or_else(|| other.to_string()),
    }
}

/// Sprawdza i wczytuje plik kopii (bez zmian w danych). Zły plik - [`BackupError::Invalid`]
/// z komunikatem dla użytkownika.
pub fn read(path: &Path) -> Result<Preview, BackupError> {
    let missing = || invalid(&t("kopia.blad.brak_pliku", &[]));
    let meta = fs::metadata(path).map_err(|_| missing())?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return Err(missing());
    }
    let mut zip = zip::ZipArchive::new(File::open(path)?).map_err(damaged)?;
    let names: Vec<String> = zip
        .file_names()
        .map(|n| n.map(Cow::into_owned))
        .collect::<Result<_, _>>()
        .map_err(damaged)?;
    if names.iter().any(|n| !matches(&SAFE_NAME, n)) {
        return Err(invalid(&t("kopia.blad.nieznane_pliki", &[])));
    }
    let mut budget = MAX_UNPACKED_BYTES;
    let manifest = json_object(&entry(&mut zip, "manifest.json", &mut budget)?)?;
    if manifest.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(invalid(&t("kopia.blad.nie_kopia", &[])));
    }
    let version = match manifest.get("wersja") {
        None => 0,
        Some(v) => json_i64(v).ok_or_else(|| damaged("wersja nie jest liczbą"))?,
    };
    if version > FORMAT_VERSION {
        return Err(invalid(&t("kopia.blad.nowsza", &[])));
    }
    let raw_people = json_object(&entry(&mut zip, "osoby.json", &mut budget)?)?;
    let people = match raw_people.get("osoby") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_object)
            .map(Person::from_json)
            .collect(),
        _ => Vec::new(),
    };
    let mut sessions = Vec::new();
    if names.iter().any(|n| n == "historia.jsonl") {
        let bytes = entry(&mut zip, "historia.jsonl", &mut budget)?;
        let text = String::from_utf8(bytes).map_err(damaged)?;
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let Value::Object(d) = serde_json::from_str::<Value>(line).map_err(damaged)? else {
                return Err(damaged("wiersz historii nie jest obiektem"));
            };
            sessions.push(session_from_json(&d).ok_or_else(|| damaged("zła sesja"))?);
        }
    }
    let mut icons = BTreeMap::new();
    for name in names.iter().filter(|n| n.starts_with("ikony/")) {
        let data = entry(&mut zip, name, &mut budget)?;
        if data.starts_with(PNG_MAGIC) {
            icons.insert(name["ikony/".len()..].to_string(), data);
        }
    }
    let empty = Map::new();
    let content = match manifest.get("zawartosc") {
        Some(Value::Object(m)) => m,
        _ => &empty,
    };
    let flag = |key: &str| content.get(key).is_some_and(crate::compat::truthy);
    Ok(Preview {
        path: path.to_path_buf(),
        app_version: manifest
            .get("aplikacja")
            .map_or_else(|| "?".to_string(), py_str),
        has_settings: flag("ustawienia"),
        has_results: flag("wyniki"),
        people,
        sessions,
        icons,
    })
}

// --- import -----------------------------------------------------------------------------------

/// Co zaimportować z pliku.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportChoice {
    /// osoby z pliku do zaimportowania
    pub people: HashSet<String>,
    pub settings: bool,
    pub results: bool,
    /// osoba istnieje, ale dodać ją jako nową (zamiast łączyć)
    pub new_person: HashSet<String>,
    /// przy łączeniu nie zmieniać ustawień moich profili o tej samej nazwie
    pub keep_my_settings: bool,
}

impl ImportChoice {
    /// Wybór z domyślnymi polami jak w 0.5.x: ustawienia i wyniki, łączenie osób.
    pub fn new(people: HashSet<String>) -> Self {
        Self {
            people,
            settings: true,
            results: true,
            new_person: HashSet::new(),
            keep_my_settings: false,
        }
    }
}

/// Wynik importu (pola jak `ImportReport` w docs/contract.md).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub people_added: u32,
    pub people_merged: u32,
    pub profiles_added: u32,
    pub profiles_updated: u32,
    pub sessions_added: u32,
    /// folder kopii sprzed importu
    pub backup: Option<PathBuf>,
}

/// Folder kopii (dane/kopie) obok osoby.json.
fn backups_dir(store_path: &Path) -> PathBuf {
    store_path
        .parent()
        .unwrap_or(Path::new("."))
        .join(BACKUP_DIR)
}

/// Kopia danych sprzed importu: osoby.json, historia.db (SQLite backup API) i ikony.
pub fn backup_current(store: &Store, history_path: &Path) -> Result<PathBuf, BackupError> {
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let folder = backups_dir(store.path()).join(format!("{BEFORE_IMPORT}{stamp}"));
    fs::create_dir_all(&folder)?;
    store.save()?;
    fs::copy(store.path(), folder.join(store::STORE_FILE))?;
    if history_path.exists() {
        let src = Connection::open(history_path)?;
        src.backup(
            rusqlite::MAIN_DB,
            folder.join("historia.db"),
            None::<fn(rusqlite::backup::Progress)>,
        )?;
    }
    let icons = store.icons_dir();
    if icons.is_dir() {
        copy_tree(&icons, &folder.join(ICONS_DIR))?;
    }
    Ok(folder)
}

/// Najnowsza kopia sprzed importu w `data_files`/kopie (`data_files` = folder z osoby.json).
pub fn latest_backup(data_files: &Path) -> Option<PathBuf> {
    let dir = data_files.join(BACKUP_DIR);
    fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with(BEFORE_IMPORT))
        .map(|e| e.path())
        .max()
}

/// Przywraca kopię sprzed importu (aplikacja wczytuje potem dane od nowa). Baza historii
/// jest nadpisywana przez SQLite backup API - otwarte połączenia widzą nową zawartość.
/// The database goes first: when it fails, osoby.json still matches the history.
pub fn restore(folder: &Path, store_path: &Path, history_path: &Path) -> Result<(), BackupError> {
    let people = fs::read(folder.join(store::STORE_FILE))?;
    let db = folder.join("historia.db");
    if db.exists() {
        let mut dst = Connection::open(history_path)?;
        dst.restore(
            rusqlite::MAIN_DB,
            &db,
            None::<fn(rusqlite::backup::Progress)>,
        )?;
    }
    store::write_atomic(store_path, &people)?;
    let icons = folder.join(ICONS_DIR);
    if icons.is_dir() {
        let target = store_path
            .parent()
            .unwrap_or(Path::new("."))
            .join(ICONS_DIR);
        copy_tree(&icons, &target)?;
    }
    log::info!(
        "Przywrócono kopię {}",
        folder
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default()
    );
    Ok(())
}

/// Kopia folderu z podfolderami (istniejące pliki są nadpisywane).
fn copy_tree(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for item in fs::read_dir(src)? {
        let item = item?;
        let to = dst.join(item.file_name());
        if item.file_type()?.is_dir() {
            copy_tree(&item.path(), &to)?;
        } else {
            fs::copy(item.path(), to)?;
        }
    }
    Ok(())
}

/// Zapisuje ikonę profilu z pliku kopii jako `<id profilu>.png`; `None` - brak ikony w kopii.
fn save_icon(
    preview: &Preview,
    icons_dir: &Path,
    icon_file: Option<&str>,
    profile_id: &str,
) -> Result<Option<String>, BackupError> {
    let Some(icon) = icon_file.filter(|f| !f.is_empty()) else {
        return Ok(None);
    };
    let Some(data) = preview.icons.get(&icon_key(icon)) else {
        return Ok(None);
    };
    // id pochodzi z pliku kopii - bez ścieżek poza folder ikon
    if !matches(&SAFE_ID, profile_id) {
        log::warn!("Import: pomijam ikonę profilu o nietypowym id");
        return Ok(None);
    }
    fs::create_dir_all(icons_dir)?;
    let name = format!("{profile_id}.png");
    fs::write(icons_dir.join(&name), data)?;
    Ok(Some(name))
}

/// Wolna nazwa: „Ala”, a gdy zajęta - „Ala 2”, „Ala 3”...
fn unique(name: &str, taken: &[String]) -> String {
    if !taken.iter().any(|t| t == name) {
        return name.to_string();
    }
    let mut i = 2;
    while taken.iter().any(|t| *t == format!("{name} {i}")) {
        i += 1;
    }
    format!("{name} {i}")
}

/// Import z podglądu: najpierw kopia obecnych danych ([`backup_current`]), potem osoby
/// (połączenie z istniejącą albo dodanie jako nowej), profile, ikony i sesje. Aktywna osoba
/// się nie zmienia. Zasady jak w 0.5.x:
/// - łączenie: profil o nowej nazwie jest dodawany (z nowym id, gdy id zajęte), profil o tej
///   samej nazwie dostaje ustawienia, kolor i ikonę z pliku (chyba że `keep_my_settings`),
/// - nowa osoba: wolna nazwa („Ala 2”); gdy jej id już jest - nowe id osoby, profili i sesji;
///   bez ustawień w pliku - sam profil Domyślny,
/// - sesje dochodzą tylko te, których jeszcze nie ma (po id).
pub fn apply_import(
    store: &mut Store,
    history: &History,
    preview: &Preview,
    choice: &ImportChoice,
    history_path: &Path,
) -> Result<ImportReport, BackupError> {
    let mut report = ImportReport {
        backup: Some(backup_current(store, history_path)?),
        ..ImportReport::default()
    };
    let icons_dir = store.icons_dir();
    let with_settings = choice.settings && preview.has_settings;
    let mut people: Vec<PersonJson<Profile>> = store
        .people()
        .iter()
        .map(|p| PersonJson::with_profiles(p, p.profiles().to_vec()))
        .collect();
    let mut imported: Vec<Session> = Vec::new();
    for src in &preview.people {
        if !choice.people.contains(&src.id) {
            continue;
        }
        let target = people.iter().rposition(|p| p.id == src.id);
        let sessions: Vec<Session> = match target {
            Some(ti) if !choice.new_person.contains(&src.id) => {
                report.people_merged += 1;
                if with_settings {
                    merge_profiles(
                        &mut people[ti].profiles,
                        src,
                        preview,
                        &icons_dir,
                        choice.keep_my_settings,
                        &mut report,
                    )?;
                }
                preview.sessions_of(&src.id).into_iter().cloned().collect()
            }
            _ => {
                let (person, sessions) =
                    new_person(src, preview, &people, &icons_dir, with_settings)?;
                report.people_added += 1;
                report.profiles_added += count(person.profiles.len());
                people.push(person);
                sessions
            }
        };
        if choice.results && preview.has_results {
            imported.extend(sessions);
        }
    }
    let mut data = match store.to_json()? {
        Value::Object(m) => m,
        _ => return Err(invalid(&t("kopia.blad.polaczenie", &[]))),
    };
    data.insert("osoby".into(), serde_json::to_value(&people)?);
    let Some(merged) = Store::from_json(store.path(), &data) else {
        return Err(invalid(&t("kopia.blad.polaczenie", &[])));
    };
    *store = merged;
    store.save()?;
    // one transaction, after the people are saved: no sessions of people missing from
    // osoby.json, and a failure adds none of them
    report.sessions_added = count(history.merge(&imported)?);
    log::info!(
        "Import: dodane osoby {}, połączone {}, profile +{}/~{}, sesje +{}",
        report.people_added,
        report.people_merged,
        report.profiles_added,
        report.profiles_updated,
        report.sessions_added
    );
    Ok(report)
}

/// Łączenie profili istniejącej osoby z profilami z pliku (po nazwie).
fn merge_profiles(
    target: &mut Vec<Profile>,
    src: &Person,
    preview: &Preview,
    icons_dir: &Path,
    keep_my_settings: bool,
    report: &mut ImportReport,
) -> Result<(), BackupError> {
    // nazwa -> indeks (przy powtórzonej nazwie ostatni), tylko profile sprzed importu
    let by_name: HashMap<String, usize> = target
        .iter()
        .enumerate()
        .map(|(i, p)| (p.name.clone(), i))
        .collect();
    for pr in src.profiles() {
        match by_name.get(&pr.name) {
            None => {
                let mut new = pr.clone();
                if target.iter().any(|x| x.id == pr.id) {
                    new.id = store::new_id();
                }
                new.icon_file = save_icon(preview, icons_dir, pr.icon_file.as_deref(), &new.id)?;
                target.push(new);
                report.profiles_added += 1;
            }
            Some(&i) if !keep_my_settings => {
                let mine = &mut target[i];
                mine.settings.clone_from(&pr.settings);
                mine.color.clone_from(&pr.color);
                mine.glyph.clone_from(&pr.glyph);
                if let Some(saved) =
                    save_icon(preview, icons_dir, pr.icon_file.as_deref(), &mine.id)?
                {
                    mine.icon_file = Some(saved);
                }
                report.profiles_updated += 1;
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// Osoba z pliku dodawana jako nowa (z jej sesjami po ewentualnej zmianie id).
fn new_person(
    src: &Person,
    preview: &Preview,
    people: &[PersonJson<Profile>],
    icons_dir: &Path,
    with_settings: bool,
) -> Result<(PersonJson<Profile>, Vec<Session>), BackupError> {
    let mut person = PersonJson::with_profiles(src, src.profiles().to_vec());
    let mut sessions: Vec<Session> = preview.sessions_of(&src.id).into_iter().cloned().collect();
    if people.iter().any(|p| p.id == person.id) {
        // ta sama osoba jako nowa - wszystko z nowymi id
        person.id = store::new_id();
        let mut remap = HashMap::new();
        for pr in &mut person.profiles {
            let id = store::new_id();
            remap.insert(std::mem::replace(&mut pr.id, id.clone()), id);
        }
        for s in &mut sessions {
            s.id = uuid::Uuid::new_v4().simple().to_string();
            s.person.clone_from(&person.id);
            if let Some(id) = remap.get(&s.profile) {
                s.profile.clone_from(id);
            }
        }
    }
    let taken: Vec<String> = people.iter().map(|p| p.name.clone()).collect();
    person.name = unique(&person.name, &taken);
    for s in &mut sessions {
        s.person_name.clone_from(&person.name);
    }
    if !with_settings {
        person.profiles = vec![Profile {
            glyph: Some(DEFAULT_GLYPH.to_string()),
            ..Profile::new(store::new_id(), "Domyślny".into())
        }];
        person.look = Settings::new();
    }
    for pr in &mut person.profiles {
        pr.icon_file = save_icon(preview, icons_dir, pr.icon_file.as_deref(), &pr.id)?;
    }
    if let Some(first) = person.profiles.first() {
        person.active_profile.clone_from(&first.id);
    }
    Ok((person, sessions))
}

/// Domyślna nazwa pliku kopii: bieznia-kopia-RRRR-MM-DD.bieznia.
pub fn default_name() -> String {
    format!("bieznia-kopia-{}{SUFFIX}", Local::now().format("%Y-%m-%d"))
}

/// Opis pliku w podglądzie importu.
pub fn describe(preview: &Preview) -> String {
    let profiles: usize = preview.people.iter().map(|p| p.profiles().len()).sum();
    format!(
        "z wersji {} · {} os. · {profiles} profili · {} sesji",
        preview.app_version,
        preview.people.len(),
        preview.sessions.len()
    )
}

#[cfg(test)]
mod tests;
