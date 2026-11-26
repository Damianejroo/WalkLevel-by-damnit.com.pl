//! Historia kroków: sesje osób i profili gier w lokalnej bazie SQLite (dane/historia.db).
//! Port bieznia/historia.py - ta sama baza (schemat 5), te same wyniki zapytań.
//!
//! Sesja trwa od pierwszego ruchu taśmy do 2 minut bez ruchu (albo zmiany osoby, profilu,
//! bieżni, trybu albo zamknięcia aplikacji) - patrz [`SessionTracker`]. Czasy w sekundach UTC,
//! dni i godziny liczone w strefie użytkownika. Wersja schematu w `PRAGMA user_version`;
//! starsze bazy (0.3.x bez wersji, 2, 3, 4) przechodzą migrację bez utraty sesji.

mod tracker;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Local, LocalResult, NaiveDate, NaiveTime, Offset, TimeDelta, TimeZone};
use rusqlite::types::{Value, ValueRef};
use rusqlite::{Connection, OptionalExtension, Row, ToSql};
use serde::{Deserialize, Serialize};

pub use crate::model::{Session, Totals};
pub use tracker::{Clock, LiveStats, SessionTracker};

/// Ścieżka bazy względem folderu danych (%LOCALAPPDATA%\Bieznia).
pub const DB_FILE: &str = "dane/historia.db";
/// Tyle sekund bez ruchu kończy sesję.
pub const IDLE_END_S: f64 = 120.0;
/// Co tyle sekund bieżąca sesja trafia na dysk.
pub const SAVE_EVERY_S: f64 = 60.0;
/// Krótsze sesje bez kroków nie trafiają do historii (przypadkowy ruch taśmy).
pub const MIN_SESSION_S: f64 = 10.0;
/// Podpis wyników usuniętej osoby.
pub const DELETED_PERSON: &str = "Usunięte osoby";
/// 3: wysłane i ukryte online; 4: tryb, osiągnięcia, poziom; 5: odrzucone.
pub const SCHEMA_VERSION: i64 = 5;
pub const VISIBILITY: [&str; 3] = ["prywatna", "znajomi", "publiczna"];

/// Kolumny tabeli `sesje` w kolejności pól [`Session`]. Wersja z 0.3.x miała pierwsze
/// dziesięć, a czasy nazywały się start / koniec.
const COLUMNS: [(&str, &str); 20] = [
    ("id", "TEXT PRIMARY KEY"),
    ("osoba", "TEXT NOT NULL"),
    ("osoba_nazwa", "TEXT NOT NULL"),
    ("profil", "TEXT NOT NULL"),
    ("profil_nazwa", "TEXT NOT NULL"),
    ("start_utc_s", "REAL NOT NULL"),
    ("koniec_utc_s", "REAL NOT NULL"),
    ("kroki", "INTEGER NOT NULL DEFAULT 0"),
    ("dystans_km", "REAL NOT NULL DEFAULT 0"),
    ("ruch_s", "REAL NOT NULL DEFAULT 0"),
    ("bieznia_model", "TEXT NOT NULL DEFAULT ''"),
    ("sterownik", "TEXT NOT NULL DEFAULT ''"),
    ("bieznia_profil", "TEXT NOT NULL DEFAULT ''"),
    ("firmware", "TEXT NOT NULL DEFAULT ''"),
    ("wersja_aplikacji", "TEXT NOT NULL DEFAULT ''"),
    ("zrodlo", "TEXT NOT NULL DEFAULT 'bieznia'"),
    ("widocznosc", "TEXT NOT NULL DEFAULT 'prywatna'"),
    ("strefa_min", "INTEGER NOT NULL DEFAULT 0"),
    ("zmieniono_utc_s", "REAL NOT NULL DEFAULT 0"),
    ("tryb", "TEXT NOT NULL DEFAULT 'gra'"), // gra albo trening (tryb Trening, bez gry)
];

static SELECT: LazyLock<String> = LazyLock::new(|| {
    let names: Vec<&str> = COLUMNS.iter().map(|(n, _)| *n).collect();
    format!("SELECT {} FROM sesje", names.join(", "))
});

static INSERT: LazyLock<String> = LazyLock::new(|| {
    let names: Vec<&str> = COLUMNS.iter().map(|(n, _)| *n).collect();
    let marks = vec!["?"; COLUMNS.len()].join(",");
    format!("INTO sesje ({}) VALUES ({marks})", names.join(", "))
});

/// Ten sam tekst co SCHEMA w Pythonie (sqlite_master w obu wersjach jest identyczny).
static SCHEMA: LazyLock<String> = LazyLock::new(|| {
    let cols: Vec<String> = COLUMNS.iter().map(|(n, k)| format!("{n} {k}")).collect();
    format!(
        "
CREATE TABLE IF NOT EXISTS sesje (
    {}
);
CREATE INDEX IF NOT EXISTS sesje_osoba_start ON sesje (osoba, start_utc_s);
CREATE TABLE IF NOT EXISTS wyslane (
    id TEXT PRIMARY KEY,
    zmieniono_utc_s REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS odrzucone (
    id TEXT PRIMARY KEY,
    zmieniono_utc_s REAL NOT NULL,
    powod TEXT NOT NULL,
    ponow_utc_s REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS ukryte_online (
    id TEXT PRIMARY KEY
);
CREATE TABLE IF NOT EXISTS osiagniecia (
    osoba TEXT NOT NULL,
    id TEXT NOT NULL,
    zdobyte_utc_s REAL NOT NULL,
    wersja INTEGER NOT NULL,
    PRIMARY KEY (osoba, id)
);
CREATE TABLE IF NOT EXISTS poziom_osoby (
    osoba TEXT PRIMARY KEY,
    poziom INTEGER NOT NULL
);
",
        cols.join(", ")
    )
});

/// Błędy historii.
#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("baza historii: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("folder historii: {0}")]
    Io(#[from] std::io::Error),
    #[error("nieprawidłowy czas: {0}")]
    Time(f64),
    #[error("nieprawidłowa liczba dni: {0}")]
    Days(i64),
}

pub type Result<T> = std::result::Result<T, HistoryError>;

/// Skąd sesja: bieżnia (model, sterownik, profil, firmware) albo symulator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSource {
    pub model: String,
    pub driver: String,
    pub profile: String,
    pub firmware: String,
    pub app_version: String,
    /// "bieznia" albo "symulator"
    pub source: String,
}

impl Default for SessionSource {
    fn default() -> Self {
        Self {
            model: String::new(),
            driver: String::new(),
            profile: String::new(),
            firmware: String::new(),
            app_version: String::new(),
            source: "bieznia".into(),
        }
    }
}

/// Po czym grupować kroki: profil gry albo osoba.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupKey {
    #[default]
    Profile,
    Person,
}

impl GroupKey {
    fn name(self, s: &Session) -> &str {
        match self {
            GroupKey::Profile => &s.profile_name,
            GroupKey::Person => &s.person_name,
        }
    }
}

/// Zakres ekranu historii: dziś, 7 dni, 30 dni, wszystko.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryRange {
    #[default]
    Day,
    Week,
    Month,
    All,
}

impl HistoryRange {
    /// Liczba dni zakresu (0 - wszystko).
    pub fn days(self) -> i64 {
        match self {
            HistoryRange::Day => 1,
            HistoryRange::Week => 7,
            HistoryRange::Month => 30,
            HistoryRange::All => 0,
        }
    }

    /// Początek zakresu: lokalna północ pierwszego dnia (0 dla "wszystko").
    pub fn since(self, now: f64) -> Result<f64> {
        self.since_in(&Local, now)
    }

    pub(crate) fn since_in<Tz: TimeZone>(self, tz: &Tz, now: f64) -> Result<f64> {
        let days = self.days();
        if days == 0 {
            return Ok(0.0);
        }
        let first = days_before(local_date(tz, now)?, days - 1)?;
        Ok(local_midnight(tz, first))
    }

    /// Ile dni pokazuje wykres "Kroki dziennie" dla tego zakresu.
    pub fn chart_days(self) -> i64 {
        match self {
            HistoryRange::All => 30,
            r if r.days() > 1 => r.days(),
            _ => 7,
        }
    }

    /// Dla "dziś" wykres jest godzinowy ([`History::by_hour`]).
    pub fn hourly(self) -> bool {
        self == HistoryRange::Day
    }
}

/// Krótki odcisk zawartości historii - czy widok trzeba zbudować od nowa.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryStamp {
    pub count: i64,
    pub steps: i64,
    /// najpóźniejszy koniec sesji
    pub last_end: f64,
}

/// Kroki w przedziałach (dzień "RRRR-MM-DD" albo godzina "HH") z podziałem na nazwy.
pub type StepSeries = Vec<(String, BTreeMap<String, i64>)>;

/// Bieżący czas uniksowy w sekundach.
pub fn now_unix() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// UUID wersji 7 jako 32 znaki hex (sortuje się po czasie, ponowna wysyłka nie dubluje).
pub fn uuid7() -> String {
    uuid::Uuid::now_v7().simple().to_string()
}

/// Strefa czasowa w chwili `at` (minuty od UTC) - dni liczone jak u użytkownika.
pub fn utc_offset_min(at: f64) -> i64 {
    to_local(&Local, at)
        .map(|d| i64::from(d.offset().fix().local_minus_utc()).div_euclid(60))
        .unwrap_or(0)
}

/// Nowa sesja z domyślnymi polami jak w Pythonie (bieżnia, prywatna, tryb gra).
pub fn new_session(
    id: &str,
    person: &str,
    person_name: &str,
    profile: &str,
    profile_name: &str,
    start: f64,
    end: f64,
) -> Session {
    Session {
        id: id.into(),
        person: person.into(),
        person_name: person_name.into(),
        profile: profile.into(),
        profile_name: profile_name.into(),
        start,
        end,
        steps: 0,
        distance_km: 0.0,
        active_s: 0.0,
        device_model: String::new(),
        driver: String::new(),
        device_profile: String::new(),
        firmware: String::new(),
        app_version: String::new(),
        source: "bieznia".into(),
        visibility: "prywatna".into(),
        tz_offset_min: 0,
        updated: 0.0,
        mode: "gra".into(),
    }
}

/// Przepisuje do sesji dane bieżni (model, sterownik, profil, firmware, wersja, źródło).
pub fn apply_source(s: &mut Session, src: &SessionSource) {
    s.device_model.clone_from(&src.model);
    s.driver.clone_from(&src.driver);
    s.device_profile.clone_from(&src.profile);
    s.firmware.clone_from(&src.firmware);
    s.app_version.clone_from(&src.app_version);
    s.source.clone_from(&src.source);
}

/// Schemat bazy do [`SCHEMA_VERSION`]. Baza z 0.3.x (bez wersji) zachowuje wszystkie sesje.
fn migrate(db: &Connection) -> Result<()> {
    let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version >= SCHEMA_VERSION {
        return Ok(());
    }
    let exists = db
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'sesje'",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    let tx = db.unchecked_transaction()?;
    if exists {
        let mut have: HashSet<String> = {
            let mut st = tx.prepare("PRAGMA table_info(sesje)")?;
            st.query_map([], |r| r.get::<_, String>(1))?
                .collect::<rusqlite::Result<_>>()?
        };
        if have.contains("start") {
            tx.execute_batch("ALTER TABLE sesje RENAME COLUMN start TO start_utc_s")?;
            have.insert("start_utc_s".into());
        }
        if have.contains("koniec") {
            tx.execute_batch("ALTER TABLE sesje RENAME COLUMN koniec TO koniec_utc_s")?;
            have.insert("koniec_utc_s".into());
        }
        for (name, kind) in COLUMNS {
            if !have.contains(name) {
                tx.execute_batch(&format!("ALTER TABLE sesje ADD COLUMN {name} {kind}"))?;
            }
        }
        if version < 2 {
            tx.execute(
                "UPDATE sesje SET zmieniono_utc_s = koniec_utc_s WHERE zmieniono_utc_s = 0",
                [],
            )?;
        }
        log::info!("Historia: baza z wersji {version} przeniesiona do {SCHEMA_VERSION}");
    }
    tx.execute_batch(&SCHEMA)?;
    tx.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))?;
    tx.commit()?;
    Ok(())
}

/// Lokalna baza historii (jedno połączenie SQLite, tryb WAL).
#[derive(Debug)]
pub struct History {
    db: Connection,
}

impl History {
    /// Otwiera (albo tworzy) bazę pod `path` i przenosi ją do bieżącego schematu.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        Self::init(Connection::open(path)?)
    }

    /// Baza w pamięci (testy, podgląd bez zapisu na dysk).
    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    /// Ścieżka bazy w folderze danych.
    pub fn default_path(data_dir: &Path) -> PathBuf {
        data_dir.join("dane").join("historia.db")
    }

    fn init(db: Connection) -> Result<Self> {
        migrate(&db)?;
        db.query_row("PRAGMA journal_mode=WAL", [], |r| r.get::<_, String>(0))?;
        Ok(Self { db })
    }

    /// Zapisuje (albo zastępuje) sesję; ustawia `updated` na bieżący czas.
    pub fn save(&self, s: &mut Session) -> Result<()> {
        s.updated = now_unix();
        self.db.execute(
            &format!("INSERT OR REPLACE {}", *INSERT),
            bind(s).as_slice(),
        )?;
        Ok(())
    }

    /// Import: dodaje sesje, których jeszcze nie ma (po id). Zwraca liczbę nowych.
    pub fn merge(&self, sessions: &[Session]) -> Result<usize> {
        let before = self.count()?;
        let tx = self.db.unchecked_transaction()?;
        {
            let mut st = tx.prepare(&format!("INSERT OR IGNORE {}", *INSERT))?;
            for s in sessions {
                st.execute(bind(s).as_slice())?;
            }
        }
        tx.commit()?;
        Ok(self.count()?.saturating_sub(before))
    }

    // --- konto online (opcjonalne) ------------------------------------------------------

    /// Sesje osoby powiązanej z kontem, których serwer jeszcze nie ma albo które się
    /// zmieniły od wysyłki. Bez sesji z symulatora. Odrzucona przez serwer czeka do terminu
    /// ponowienia, chyba że sama się zmieniła. Od najstarszej.
    pub fn unsent(&self, person: &str, now: f64) -> Result<Vec<Session>> {
        let sql = format!(
            "{} WHERE osoba = ? AND zrodlo = 'bieznia' AND NOT EXISTS (\
             SELECT 1 FROM wyslane w WHERE w.id = sesje.id \
             AND w.zmieniono_utc_s >= sesje.zmieniono_utc_s) AND NOT EXISTS (\
             SELECT 1 FROM odrzucone o WHERE o.id = sesje.id \
             AND o.zmieniono_utc_s >= sesje.zmieniono_utc_s AND o.ponow_utc_s > ?) \
             ORDER BY start_utc_s",
            *SELECT
        );
        self.query_sessions(&sql, &[Value::from(person.to_owned()), Value::Real(now)])
    }

    /// Odrzucone przez serwer (sesja, powód): bez pętli wysyłek, ale z ponowieniem po
    /// `retry_at`. Powód skracany do 200 znaków.
    pub fn mark_rejected(&self, rejected: &[(&Session, &str)], retry_at: f64) -> Result<()> {
        let tx = self.db.unchecked_transaction()?;
        {
            let mut st = tx.prepare("INSERT OR REPLACE INTO odrzucone VALUES (?, ?, ?, ?)")?;
            for (s, reason) in rejected {
                let reason: String = reason.chars().take(200).collect();
                st.execute((&s.id, s.updated, reason, retry_at))?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Odrzucone sesje: id -> powód.
    pub fn rejected(&self) -> Result<HashMap<String, String>> {
        let mut st = self.db.prepare("SELECT id, powod FROM odrzucone")?;
        let rows = st.query_map([], |r| Ok((text(r, 0)?, text(r, 1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Serwer przyjął te sesje (w wersji z ich `updated`).
    pub fn mark_sent(&self, sessions: &[Session]) -> Result<()> {
        let tx = self.db.unchecked_transaction()?;
        {
            let mut sent = tx.prepare("INSERT OR REPLACE INTO wyslane VALUES (?, ?)")?;
            let mut unreject = tx.prepare("DELETE FROM odrzucone WHERE id = ?")?;
            for s in sessions {
                sent.execute((&s.id, s.updated))?;
            }
            for s in sessions {
                unreject.execute([&s.id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Inne konto, wylogowanie albo zmiana widoczności - wszystko pójdzie od nowa.
    pub fn forget_sent(&self) -> Result<()> {
        let tx = self.db.unchecked_transaction()?;
        tx.execute("DELETE FROM wyslane", [])?;
        tx.execute("DELETE FROM odrzucone", [])?;
        tx.commit()?;
        Ok(())
    }

    /// Sesje osoby z profilem gry (z bieżni) do ponownej wysyłki w nowszej wersji:
    /// `zmieniono_utc_s` = `now`, więc [`History::unsent`] znów je zwróci, a serwer, który
    /// zostawia nowszą wersję, je nadpisze (nowy regulamin, przełącznik „Pokazuj tę grę”).
    /// `profile` - tylko sesje tego profilu. Sesje bez profilu (pobrane z innego komputera
    /// bez znanej gry) zostają: tu nie ma o nich nic nowego, a nowszy czas nadpisałby na
    /// serwerze pełniejszą wersję z ich komputera. Zwraca liczbę oznaczonych.
    pub fn mark_for_resend(&self, person: &str, profile: Option<&str>, now: f64) -> Result<usize> {
        let n = self.db.execute(
            "UPDATE sesje SET zmieniono_utc_s = ?1 WHERE osoba = ?2 AND zrodlo = 'bieznia' \
             AND profil != '' AND (?3 IS NULL OR profil = ?3) AND zmieniono_utc_s < ?1",
            (now, person, profile),
        )?;
        Ok(n)
    }

    /// Wszystkie sesje osoby z bieżni do ponownej wysyłki w nowszej wersji (zmiana widoczności
    /// konta dotyczy każdej sesji, także pobranych z innego komputera). Zwraca liczbę oznaczonych.
    pub fn mark_all_for_resend(&self, person: &str, now: f64) -> Result<usize> {
        let n = self.db.execute(
            "UPDATE sesje SET zmieniono_utc_s = ?1 WHERE osoba = ?2 AND zrodlo = 'bieznia' \
             AND zmieniono_utc_s < ?1",
            (now, person),
        )?;
        Ok(n)
    }

    /// Id sesji ukrytych online.
    pub fn hidden_online(&self) -> Result<HashSet<String>> {
        let mut st = self.db.prepare("SELECT id FROM ukryte_online")?;
        let rows = st.query_map([], |r| text(r, 0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Ukryta sesja zostaje u Ciebie, a online jest prywatna (nikt jej nie widzi).
    /// Zmiana wymusza ponowną wysyłkę tej sesji w nowszej wersji (`zmieniono_utc_s` = teraz -
    /// serwer nadpisuje tylko nowszą).
    pub fn set_hidden_online(&self, session_id: &str, hidden: bool) -> Result<()> {
        let tx = self.db.unchecked_transaction()?;
        if hidden {
            tx.execute(
                "INSERT OR IGNORE INTO ukryte_online VALUES (?)",
                [session_id],
            )?;
        } else {
            tx.execute("DELETE FROM ukryte_online WHERE id = ?", [session_id])?;
        }
        tx.execute("DELETE FROM wyslane WHERE id = ?", [session_id])?;
        tx.execute(
            "UPDATE sesje SET zmieniono_utc_s = MAX(zmieniono_utc_s, ?) WHERE id = ?",
            (now_unix(), session_id),
        )?;
        tx.commit()?;
        Ok(())
    }

    // --- poziomy i osiągnięcia ----------------------------------------------------------

    /// Zdobyte osiągnięcia osoby (id -> czas zdobycia). Raz zapisanych nie usuwamy.
    pub fn earned(&self, person: &str) -> Result<HashMap<String, f64>> {
        let mut st = self
            .db
            .prepare("SELECT id, zdobyte_utc_s FROM osiagniecia WHERE osoba = ?")?;
        let rows = st.query_map([person], |r| Ok((text(r, 0)?, real(r, 1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Dopisuje zdobyte osiągnięcia (już zapisane zostają bez zmian).
    pub fn add_earned(
        &self,
        person: &str,
        items: &HashMap<String, f64>,
        version: i64,
    ) -> Result<()> {
        let tx = self.db.unchecked_transaction()?;
        {
            let mut st = tx.prepare("INSERT OR IGNORE INTO osiagniecia VALUES (?, ?, ?, ?)")?;
            for (id, at) in items {
                st.execute((person, id, at, version))?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Ostatni poziom pokazany osobie (None - jeszcze nigdy: pierwsze liczenie z historii).
    pub fn shown_level(&self, person: &str) -> Result<Option<i64>> {
        Ok(self
            .db
            .query_row(
                "SELECT poziom FROM poziom_osoby WHERE osoba = ?",
                [person],
                |r| int(r, 0),
            )
            .optional()?)
    }

    pub fn set_shown_level(&self, person: &str, level: i64) -> Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO poziom_osoby VALUES (?, ?)",
            (person, level),
        )?;
        Ok(())
    }

    pub fn schema_version(&self) -> Result<i64> {
        Ok(self.db.query_row("PRAGMA user_version", [], |r| r.get(0))?)
    }

    /// Odcisk zawartości (liczba sesji, kroki, ostatni koniec).
    pub fn version(&self) -> Result<HistoryStamp> {
        Ok(self.db.query_row(
            "SELECT COUNT(*), COALESCE(SUM(kroki), 0), COALESCE(MAX(koniec_utc_s), 0) FROM sesje",
            [],
            |r| {
                Ok(HistoryStamp {
                    count: int(r, 0)?,
                    steps: int(r, 1)?,
                    last_end: real(r, 2)?,
                })
            },
        )?)
    }

    pub fn count(&self) -> Result<usize> {
        let n: i64 = self
            .db
            .query_row("SELECT COUNT(*) FROM sesje", [], |r| r.get(0))?;
        Ok(usize::try_from(n).unwrap_or(0))
    }

    /// Sesje od `since` (start), opcjonalnie jednej osoby, od najnowszej. `limit` (Some(0)
    /// to bez limitu) działa przed filtrem `profiles` - jak w Pythonie.
    pub fn sessions(
        &self,
        since: f64,
        person: Option<&str>,
        profiles: Option<&HashSet<String>>,
        limit: Option<usize>,
    ) -> Result<Vec<Session>> {
        let mut sql = format!("{} WHERE start_utc_s >= ?", *SELECT);
        let mut args = vec![Value::Real(since)];
        if let Some(p) = person {
            sql.push_str(" AND osoba = ?");
            args.push(Value::from(p.to_owned()));
        }
        sql.push_str(" ORDER BY start_utc_s DESC");
        if let Some(n) = limit.filter(|n| *n > 0) {
            sql.push_str(&format!(" LIMIT {n}"));
        }
        let mut out = self.query_sessions(&sql, &args)?;
        if let Some(profiles) = profiles {
            out.retain(|s| profiles.contains(&s.profile));
        }
        Ok(out)
    }

    /// Suma kroków, dystansu i czasu ruchu od `since`.
    pub fn totals(&self, since: f64, person: Option<&str>) -> Result<Totals> {
        Ok(sum(&self.sessions(since, person, None, None)?))
    }

    /// Kroki dziennie z ostatnich `days` dni (do dziś włącznie): [(dzień, {nazwa: kroki})].
    /// Nazwy w dniu posortowane alfabetycznie (w Pythonie: kolejność sesji od najnowszej).
    pub fn by_day(
        &self,
        days: i64,
        person: Option<&str>,
        key: GroupKey,
        now: f64,
    ) -> Result<StepSeries> {
        self.by_day_in(&Local, days, person, key, now)
    }

    pub(crate) fn by_day_in<Tz: TimeZone>(
        &self,
        tz: &Tz,
        days: i64,
        person: Option<&str>,
        key: GroupKey,
        now: f64,
    ) -> Result<StepSeries> {
        if days <= 0 {
            return Ok(Vec::new());
        }
        let first = days_before(local_date(tz, now)?, days - 1)?;
        let since = local_midnight(tz, first);
        let mut out: StepSeries = first
            .iter_days()
            .take(usize::try_from(days).unwrap_or(usize::MAX))
            .map(|d| (iso(d), BTreeMap::new()))
            .collect();
        let index: HashMap<String, usize> = out
            .iter()
            .enumerate()
            .map(|(i, (d, _))| (d.clone(), i))
            .collect();
        for s in self.sessions(since, person, None, None)? {
            let Ok(day) = local_date(tz, s.start) else {
                continue;
            };
            if let Some(&i) = index.get(&iso(day)) {
                *out[i].1.entry(key.name(&s).to_owned()).or_insert(0) += s.steps;
            }
        }
        Ok(out)
    }

    /// Kroki dzisiaj godzina po godzinie: [("HH", {nazwa: kroki})]. Sesja przez kilka
    /// godzin dzieli kroki proporcjonalnie do czasu w każdej godzinie.
    pub fn by_hour(&self, person: Option<&str>, key: GroupKey, now: f64) -> Result<StepSeries> {
        self.by_hour_in(&Local, person, key, now)
    }

    pub(crate) fn by_hour_in<Tz: TimeZone>(
        &self,
        tz: &Tz,
        person: Option<&str>,
        key: GroupKey,
        now: f64,
    ) -> Result<StepSeries> {
        let day_start = local_midnight(tz, local_date(tz, now)?);
        let mut out: Vec<BTreeMap<String, i64>> = vec![BTreeMap::new(); 24];
        for s in self.sessions(day_start - 86400.0, person, None, None)? {
            let start = s.start.max(day_start);
            let end = s.end.max(s.start);
            let span = (end - s.start).max(1.0);
            for (hour, parts) in out.iter_mut().enumerate() {
                let h0 = day_start + hour as f64 * 3600.0;
                let overlap = end.min(h0 + 3600.0) - start.max(h0);
                if overlap <= 0.0 {
                    continue;
                }
                let share = if end - s.start < 1.0 {
                    s.steps
                } else {
                    // round() w Pythonie: połówki do parzystej
                    (s.steps as f64 * overlap / span).round_ties_even() as i64
                };
                if share != 0 {
                    *parts.entry(key.name(&s).to_owned()).or_insert(0) += share;
                }
            }
        }
        Ok(out
            .into_iter()
            .enumerate()
            .map(|(hour, parts)| (format!("{hour:02}"), parts))
            .collect())
    }

    /// [(nazwa, Totals)] od `since`, od największej liczby kroków (przy remisie kolejność
    /// pierwszego wystąpienia wśród sesji od najnowszej).
    pub fn by_group(
        &self,
        since: f64,
        person: Option<&str>,
        key: GroupKey,
    ) -> Result<Vec<(String, Totals)>> {
        let mut groups: Vec<(String, Vec<Session>)> = Vec::new();
        let mut index: HashMap<String, usize> = HashMap::new();
        for s in self.sessions(since, person, None, None)? {
            let name = key.name(&s).to_owned();
            let i = *index.entry(name.clone()).or_insert_with(|| {
                groups.push((name, Vec::new()));
                groups.len() - 1
            });
            groups[i].1.push(s);
        }
        let mut rows: Vec<(String, Totals)> =
            groups.into_iter().map(|(n, ss)| (n, sum(&ss))).collect();
        rows.sort_by_key(|(_, t)| std::cmp::Reverse(t.steps));
        Ok(rows)
    }

    /// Jedna sesja po id.
    pub fn session(&self, session_id: &str) -> Result<Option<Session>> {
        Ok(self
            .db
            .query_row(&format!("{} WHERE id = ?", *SELECT), [session_id], from_row)
            .optional()?)
    }

    /// Usuwa sesję z historii (i ślady wysyłki - nic po niej nie zostaje).
    pub fn delete(&self, session_id: &str) -> Result<()> {
        let tx = self.db.unchecked_transaction()?;
        tx.execute("DELETE FROM sesje WHERE id = ?", [session_id])?;
        tx.execute("DELETE FROM wyslane WHERE id = ?", [session_id])?;
        tx.execute("DELETE FROM ukryte_online WHERE id = ?", [session_id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn was_sent(&self, session_id: &str) -> Result<bool> {
        Ok(self
            .db
            .query_row("SELECT 1 FROM wyslane WHERE id = ?", [session_id], |_| {
                Ok(())
            })
            .optional()?
            .is_some())
    }

    /// Usunięta osoba: jej wyniki zostają, podpisane jako „Usunięte osoby”.
    pub fn forget_person(&self, person: &str) -> Result<()> {
        self.db.execute(
            "UPDATE sesje SET osoba_nazwa = ? WHERE osoba = ?",
            (DELETED_PERSON, person),
        )?;
        Ok(())
    }

    /// Czyści historię (sesje, osiągnięcia, poziom) jednej osoby albo wszystkich (None).
    pub fn clear(&self, person: Option<&str>) -> Result<()> {
        let tx = self.db.unchecked_transaction()?;
        match person {
            None => {
                tx.execute("DELETE FROM sesje", [])?;
                tx.execute("DELETE FROM osiagniecia", [])?;
                tx.execute("DELETE FROM poziom_osoby", [])?;
            }
            Some(p) => {
                tx.execute("DELETE FROM sesje WHERE osoba = ?", [p])?;
                tx.execute("DELETE FROM osiagniecia WHERE osoba = ?", [p])?;
                tx.execute("DELETE FROM poziom_osoby WHERE osoba = ?", [p])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn query_sessions(&self, sql: &str, args: &[Value]) -> Result<Vec<Session>> {
        let mut st = self.db.prepare(sql)?;
        let rows = st.query_map(rusqlite::params_from_iter(args), from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    #[cfg(test)]
    pub(crate) fn conn(&self) -> &Connection {
        &self.db
    }
}

fn sum(ss: &[Session]) -> Totals {
    Totals {
        steps: ss.iter().map(|s| s.steps).sum(),
        distance_km: py_sum(ss.iter().map(|s| s.distance_km)),
        active_s: py_sum(ss.iter().map(|s| s.active_s)),
    }
}

/// Suma liczb jak `sum()` w Pythonie 3.12+ (algorytm Neumaiera) - wynik co do bitu ten sam.
/// Zwykłe dodawanie po kolei różni się od Pythona na ostatnich bitach.
fn py_sum(xs: impl IntoIterator<Item = f64>) -> f64 {
    let (mut total, mut c) = (0.0_f64, 0.0_f64);
    for x in xs {
        let t = total + x;
        if total.abs() >= x.abs() {
            c += (total - t) + x;
        } else {
            c += (x - t) + total;
        }
        total = t;
    }
    if c != 0.0 && c.is_finite() {
        total += c;
    }
    total
}

fn bind(s: &Session) -> [&dyn ToSql; 20] {
    [
        &s.id,
        &s.person,
        &s.person_name,
        &s.profile,
        &s.profile_name,
        &s.start,
        &s.end,
        &s.steps,
        &s.distance_km,
        &s.active_s,
        &s.device_model,
        &s.driver,
        &s.device_profile,
        &s.firmware,
        &s.app_version,
        &s.source,
        &s.visibility,
        &s.tz_offset_min,
        &s.updated,
        &s.mode,
    ]
}

fn from_row(r: &Row) -> rusqlite::Result<Session> {
    Ok(Session {
        id: text(r, 0)?,
        person: text(r, 1)?,
        person_name: text(r, 2)?,
        profile: text(r, 3)?,
        profile_name: text(r, 4)?,
        start: real(r, 5)?,
        end: real(r, 6)?,
        steps: int(r, 7)?,
        distance_km: real(r, 8)?,
        active_s: real(r, 9)?,
        device_model: text(r, 10)?,
        driver: text(r, 11)?,
        device_profile: text(r, 12)?,
        firmware: text(r, 13)?,
        app_version: text(r, 14)?,
        source: text(r, 15)?,
        visibility: text(r, 16)?,
        tz_offset_min: int(r, 17)?,
        updated: real(r, 18)?,
        mode: text(r, 19)?,
    })
}

// SQLite ma typy dynamiczne - odczyt wyrozumiały, żeby żadna stara sesja nie przepadła.

fn text(r: &Row, i: usize) -> rusqlite::Result<String> {
    Ok(match r.get_ref(i)? {
        ValueRef::Text(t) | ValueRef::Blob(t) => String::from_utf8_lossy(t).into_owned(),
        ValueRef::Integer(n) => n.to_string(),
        ValueRef::Real(f) => f.to_string(),
        ValueRef::Null => String::new(),
    })
}

fn real(r: &Row, i: usize) -> rusqlite::Result<f64> {
    Ok(match r.get_ref(i)? {
        ValueRef::Real(f) => f,
        ValueRef::Integer(n) => n as f64,
        ValueRef::Text(t) => std::str::from_utf8(t)
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0.0),
        ValueRef::Null | ValueRef::Blob(_) => 0.0,
    })
}

fn int(r: &Row, i: usize) -> rusqlite::Result<i64> {
    Ok(match r.get_ref(i)? {
        ValueRef::Integer(n) => n,
        ValueRef::Real(f) => f as i64,
        ValueRef::Text(t) => std::str::from_utf8(t)
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0),
        ValueRef::Null | ValueRef::Blob(_) => 0,
    })
}

// --- czas lokalny (jak datetime.fromtimestamp w Pythonie) ---------------------------------

/// Jak datetime.fromtimestamp: mikrosekundy zaokrąglane połówkami do parzystej
/// (23:59:59.9999996 to już następny dzień).
fn to_local<Tz: TimeZone>(tz: &Tz, ts: f64) -> Result<DateTime<Tz>> {
    if !ts.is_finite() {
        return Err(HistoryError::Time(ts));
    }
    let mut secs = ts.trunc();
    let mut us = ((ts - secs) * 1e6).round_ties_even();
    if us >= 1e6 {
        us -= 1e6;
        secs += 1.0;
    } else if us < 0.0 {
        us += 1e6;
        secs -= 1.0;
    }
    DateTime::from_timestamp(secs as i64, (us as u32).min(999_999) * 1000)
        .map(|d| d.with_timezone(tz))
        .ok_or(HistoryError::Time(ts))
}

fn local_date<Tz: TimeZone>(tz: &Tz, ts: f64) -> Result<NaiveDate> {
    Ok(to_local(tz, ts)?.date_naive())
}

/// Lokalna północ dnia jako czas uniksowy (jak datetime.combine(d, min.time()).timestamp():
/// przy podwójnej godzinie wcześniejsza, przy brakującej - przesunięcie sprzed zmiany,
/// czyli chwila samej zmiany czasu; port local_to_seconds z CPythona dla fold=0).
fn local_midnight<Tz: TimeZone>(tz: &Tz, day: NaiveDate) -> f64 {
    let midnight = day.and_time(NaiveTime::MIN);
    match tz.from_local_datetime(&midnight) {
        LocalResult::Single(d) | LocalResult::Ambiguous(d, _) => d.timestamp() as f64,
        LocalResult::None => {
            let t = midnight.and_utc().timestamp();
            let offset = |u: i64| {
                DateTime::from_timestamp(u, 0).map_or(0, |d| {
                    i64::from(
                        tz.offset_from_utc_datetime(&d.naive_utc())
                            .fix()
                            .local_minus_utc(),
                    )
                })
            };
            let u1 = t - offset(t);
            let u2 = t - offset(u1);
            u1.max(u2) as f64
        }
    }
}

/// `n` dni przed `day` (ujemne `n` - po), bez paniki przy wyjściu poza zakres dat.
fn days_before(day: NaiveDate, n: i64) -> Result<NaiveDate> {
    TimeDelta::try_days(n)
        .and_then(|d| day.checked_sub_signed(d))
        .ok_or(HistoryError::Days(n))
}

fn iso(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

// --- synchronizacja z serwerem (konto online, docs/synchro.md) ------------------------------

impl History {
    /// Sesja pobrana z serwera: zastępuje lokalną, gdy jej `updated` (zmieniono_utc_s z
    /// serwera) jest większe; przy równości albo starszej zostaje lokalna. Zapisana wersja
    /// jest oznaczona jako wysłana (pobranych nie odsyłamy) i znika z odrzuconych. Czas
    /// `updated` zostaje ten z serwera (inaczej niż w [`History::save`]). `hide_online` -
    /// zapisana wersja trafia też do ukrytych online (bez wymuszania ponownej wysyłki, inaczej
    /// niż [`History::set_hidden_online`]). Zwraca, czy historia się zmieniła.
    pub fn upsert_from_server(&self, s: &Session, hide_online: bool) -> Result<bool> {
        let tx = self.db.unchecked_transaction()?;
        let local: Option<f64> = tx
            .query_row(
                "SELECT zmieniono_utc_s FROM sesje WHERE id = ?",
                [&s.id],
                |r| real(r, 0),
            )
            .optional()?;
        let newer = local.is_none_or(|at| s.updated > at);
        if newer {
            tx.execute(
                &format!("INSERT OR REPLACE {}", *INSERT),
                bind(s).as_slice(),
            )?;
            tx.execute("DELETE FROM odrzucone WHERE id = ?", [&s.id])?;
            if hide_online {
                tx.execute("INSERT OR IGNORE INTO ukryte_online VALUES (?)", [&s.id])?;
            }
        }
        if newer || local == Some(s.updated) {
            // ta wersja jest już na serwerze - nie wysyłać jej z powrotem
            tx.execute(
                "INSERT INTO wyslane VALUES (?, ?) ON CONFLICT(id) DO UPDATE SET \
                 zmieniono_utc_s = MAX(zmieniono_utc_s, excluded.zmieniono_utc_s)",
                (&s.id, s.updated),
            )?;
        }
        tx.commit()?;
        Ok(newer)
    }

    /// Nagrobek z serwera (sesja usunięta na innym komputerze): usuwa sesję z historii,
    /// kolejki wysyłki, odrzuconych i ukrytych online. Zwraca, czy sesja tu była.
    pub fn delete_from_server(&self, session_id: &str) -> Result<bool> {
        let tx = self.db.unchecked_transaction()?;
        let removed = tx.execute("DELETE FROM sesje WHERE id = ?", [session_id])?;
        tx.execute("DELETE FROM wyslane WHERE id = ?", [session_id])?;
        tx.execute("DELETE FROM odrzucone WHERE id = ?", [session_id])?;
        tx.execute("DELETE FROM ukryte_online WHERE id = ?", [session_id])?;
        tx.commit()?;
        Ok(removed > 0)
    }

    /// Wysłane sesje osoby w bieżącej wersji: id -> `updated` sesji w historii (do pełnego
    /// pobrania po 410: czego serwer już nie ma, a było wysłane - usunąć). Sesja zmieniona
    /// po wysyłce liczy się jako niewysłana - tu jej nie ma (zostaje i pójdzie na serwer).
    pub fn sent_ids(&self, person: &str) -> Result<HashMap<String, f64>> {
        let mut st = self.db.prepare(
            "SELECT s.id, s.zmieniono_utc_s FROM sesje s JOIN wyslane w ON w.id = s.id \
             WHERE s.osoba = ? AND w.zmieniono_utc_s >= s.zmieniono_utc_s",
        )?;
        let rows = st.query_map([person], |r| Ok((text(r, 0)?, real(r, 1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}
