//! Stan kont zapisany lokalnie w dane/konto.json (bez tokenu i bez hasła) - port
//! `AccountState`, `load_states`, `save_states` z bieznia/konto/__init__.py.
//!
//! Format 2: `{"wersja": 2, "konta": [...]}` (wiele osób z kontem), format 1 ze starszej
//! wersji: sam obiekt jednego konta. Zapis `indent=1`, atomowo (konto.tmp -> konto.json),
//! tylko zalogowane konta z przypiętą osobą. Nieznane pola pomijamy przy odczycie.
//! 0.6.0 dopisuje `sync_cursor` (kursor synchronizacji z serwera, tylko gdy niepusty) -
//! 0.5.x tego pola nie zna i je pomija (zgubiony kursor = pełne pobranie, bez szkody).
//! 0.6.1 dopisuje `terms_games` (z `GET regulamin` pole `poziomy`: wersja regulaminu, od której
//! serwer zapisuje pola gier i pokazuje poziomy) i `resent_terms` (wersja, po której akceptacji
//! historia poszła jeszcze raz z grami) - oba tylko niepuste.
//! 0.6.2 dopisuje `live_status` (status dla znajomych: wszystko / chodze / niewidoczny; bez
//! pola - z `live`: true -> wszystko, false -> niewidoczny; `live` zostaje zgodne dla 0.5.x)
//! i `description` (opis dla znajomych, tylko niepusty).

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use serde::Serialize;
use serde_json::Value;

use super::{GAMES_VISIBILITY, LIVE_STATUS, PROFILE_VISIBILITY, VISIBILITY};

/// Plik stanu kont względem folderu danych.
pub const STATE_FILE: &str = "dane/konto.json";

/// Stan konta zapisany lokalnie (bez tokenu i bez hasła). Kolejność pól jak w Pythonie.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AccountState {
    pub user_id: String,
    pub name: String,
    pub email: String,
    pub verified: bool,
    /// lokalna osoba, której sesje idą na konto
    pub person: String,
    pub upload: bool,
    /// widoczność wysyłanych sesji (ukryte zawsze prywatne)
    pub visibility: String,
    /// stan na żywo dla znajomych
    pub live: bool,
    /// czy jestem na liście osób z kontem
    pub profile_visibility: String,
    /// wersja zaakceptowanego regulaminu
    pub terms: String,
    /// token w Menedżerze poświadczeń (inaczej tylko do zamknięcia)
    pub remember: bool,
    /// obowiązująca wersja regulaminu (z serwera)
    pub terms_current: String,
    /// kto widzi podział wyników na gry
    pub games_visibility: String,
    /// kursor synchronizacji z serwera (docs/synchro.md); pusty - pełne pobranie
    #[serde(skip_serializing_if = "String::is_empty")]
    pub sync_cursor: String,
    /// wersja regulaminu, od której serwer zapisuje pola gier (`poziomy` z `GET regulamin`);
    /// pusty - serwer jeszcze ich nie przyjmuje
    #[serde(skip_serializing_if = "String::is_empty")]
    pub terms_games: String,
    /// wersja regulaminu, po której akceptacji cała historia poszła jeszcze raz (z grami)
    #[serde(skip_serializing_if = "String::is_empty")]
    pub resent_terms: String,
    /// status dla znajomych ([`LIVE_STATUS`]); `live` = status inny niż „niewidoczny”
    /// („niewidoczny” nie trafia do pliku - odczyt bierze go z `live: false`, plik jak w 0.5.x)
    #[serde(skip_serializing_if = "is_invisible")]
    pub live_status: String,
    /// opis dla znajomych (z serwera, do 40 znaków)
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
}

impl Default for AccountState {
    fn default() -> Self {
        Self {
            user_id: String::new(),
            name: String::new(),
            email: String::new(),
            verified: false,
            person: String::new(),
            upload: true,
            visibility: "znajomi".into(),
            live: false,
            profile_visibility: "znajomi".into(),
            terms: String::new(),
            remember: true,
            terms_current: String::new(),
            games_visibility: "znajomi".into(),
            sync_cursor: String::new(),
            terms_games: String::new(),
            resent_terms: String::new(),
            live_status: "niewidoczny".into(),
            description: String::new(),
        }
    }
}

impl AccountState {
    /// Wylogowany stan osoby (np. konto do zalogowania).
    pub fn for_person(person: &str) -> Self {
        Self {
            person: person.to_string(),
            ..Self::default()
        }
    }

    pub fn logged_in(&self) -> bool {
        !self.user_id.is_empty()
    }

    /// Stan na żywo idzie do znajomych (status inny niż „niewidoczny”).
    pub fn shares_live(&self) -> bool {
        self.live_status != "niewidoczny"
    }

    /// Nowy status dla znajomych; `live` (pole 0.5.x) idzie za nim.
    pub fn set_live_status(&mut self, value: &str) {
        self.live_status = value.to_string();
        self.live = self.shares_live();
    }

    /// Serwer ma nowszy regulamin niż ten zaakceptowany - karta „zaakceptuj”.
    pub fn terms_outdated(&self) -> bool {
        !self.terms_current.is_empty() && self.terms != self.terms_current
    }

    /// Pola gier (tryb, gra, gra_publiczna) wychodzą dopiero, gdy serwer je przyjmuje
    /// (`terms_games` niepuste), a konto zaakceptowało tę wersję regulaminu albo nowszą
    /// (wersje to daty RRRR-MM-DD - porównanie napisów).
    pub fn games_allowed(&self) -> bool {
        !self.terms_games.is_empty() && !self.terms.is_empty() && self.terms >= self.terms_games
    }

    /// Historia jeszcze nie poszła z grami od wersji `terms_games` - pierwsza wysyłka wyśle ją
    /// raz jeszcze (raz na zmianę `poziomy` po stronie serwera).
    pub fn resend_due(&self) -> bool {
        self.games_allowed() && self.resent_terms < self.terms_games
    }

    /// Odczyt wyrozumiały (zły typ pola - wartość domyślna); złe `visibility`
    /// i `profile_visibility` -> "znajomi", jak w 0.5.x.
    pub fn from_value(raw: &Value) -> Self {
        let d = Self::default();
        let s = |key: &str, default: &str| match raw.get(key) {
            Some(Value::String(v)) => v.clone(),
            _ => default.to_string(),
        };
        let b = |key: &str, default: bool| raw.get(key).and_then(Value::as_bool).unwrap_or(default);
        let mut state = Self {
            user_id: s("user_id", &d.user_id),
            name: s("name", &d.name),
            email: s("email", &d.email),
            verified: b("verified", d.verified),
            person: s("person", &d.person),
            upload: b("upload", d.upload),
            visibility: s("visibility", &d.visibility),
            live: b("live", d.live),
            profile_visibility: s("profile_visibility", &d.profile_visibility),
            terms: s("terms", &d.terms),
            remember: b("remember", d.remember),
            terms_current: s("terms_current", &d.terms_current),
            games_visibility: s("games_visibility", &d.games_visibility),
            sync_cursor: s("sync_cursor", ""),
            terms_games: s("terms_games", ""),
            resent_terms: s("resent_terms", ""),
            live_status: s("live_status", ""),
            description: s("description", ""),
        };
        if !LIVE_STATUS.contains(&state.live_status.as_str()) {
            // konto.json sprzed 0.6.2: przełącznik „Na żywo dla znajomych”
            state.live_status = if state.live {
                "wszystko"
            } else {
                "niewidoczny"
            }
            .into();
        }
        state.live = state.shares_live();
        if !VISIBILITY.contains(&state.visibility.as_str()) {
            state.visibility = "znajomi".into();
        }
        if !PROFILE_VISIBILITY.contains(&state.profile_visibility.as_str()) {
            state.profile_visibility = "znajomi".into();
        }
        if !GAMES_VISIBILITY.contains(&state.games_visibility.as_str()) {
            // 0.5.x tu nie sprawdzało (zła wartość szła dalej na serwer) - poprawka
            state.games_visibility = "znajomi".into();
        }
        state
    }
}

fn is_invisible(status: &str) -> bool {
    status == "niewidoczny"
}

fn read_json(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Pierwsze konto z pliku (jak `load_state` w 0.5.x); brak pliku - stan pusty.
pub fn load_state(path: &Path) -> AccountState {
    let Some(raw) = read_json(path) else {
        return AccountState::default();
    };
    match raw.get("konta") {
        Some(list) => match list.as_array().and_then(|l| l.first()) {
            Some(first) if first.is_object() => AccountState::from_value(first),
            _ => AccountState::default(),
        },
        None if raw.is_object() => AccountState::from_value(&raw),
        None => AccountState::default(),
    }
}

/// Wszystkie konta z pliku (format 2: lista; format 1: jedno), tylko zalogowane
/// z przypiętą osobą.
pub fn load_states(path: &Path) -> Vec<AccountState> {
    let Some(raw) = read_json(path) else {
        return Vec::new();
    };
    let items: Vec<Value> = match raw.get("konta") {
        Some(Value::Array(list)) => list.clone(),
        Some(_) => Vec::new(),
        None => vec![raw],
    };
    items
        .iter()
        .filter(|r| r.is_object())
        .map(AccountState::from_value)
        .filter(|s| s.logged_in() && !s.person.is_empty())
        .collect()
}

#[derive(Serialize)]
struct StateFileData<'a> {
    wersja: u32,
    konta: &'a [AccountState],
}

/// Treść pliku konto.json (wcięcie 1 spacja, bez ucieczek dla polskich liter - jak
/// `json.dumps(..., ensure_ascii=False, indent=1)`).
pub fn states_text(states: &[AccountState]) -> String {
    let data = StateFileData {
        wersja: 2,
        konta: states,
    };
    let mut out = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b" ");
    let mut ser = serde_json::Serializer::with_formatter(&mut out, formatter);
    if data.serialize(&mut ser).is_err() {
        return String::new();
    }
    String::from_utf8(out).unwrap_or_default()
}

/// Zapis atomowy: konto.tmp, potem podmiana konto.json.
pub fn save_states(states: &[AccountState], path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(states_text(states).as_bytes())?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

/// Jedno konto do zapisu (format 1 nie powstaje - zawsze format 2).
pub fn save_state(state: &AccountState, path: &Path) -> io::Result<()> {
    if state.logged_in() {
        save_states(std::slice::from_ref(state), path)
    } else {
        save_states(&[], path)
    }
}

/// Wspólny plik kont wielu osób: każde konto po zmianie wpisuje tu swój stan i zapisuje
/// całość (jak `saver` -> `Accounts.save` w 0.5.x).
#[derive(Debug)]
pub(crate) struct StateFile {
    path: PathBuf,
    inner: Mutex<StateFileInner>,
}

#[derive(Debug, Default)]
struct StateFileInner {
    states: Vec<AccountState>,
    /// bez zapisu pliku, zanim wszystkie konta są wczytane
    loading: bool,
}

impl StateFile {
    pub(crate) fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            inner: Mutex::new(StateFileInner {
                states: Vec::new(),
                loading: true,
            }),
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn finish_loading(&self) {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .loading = false;
    }

    /// Stan jednego konta (po osobie): zalogowane - wpis, wylogowane - usunięcie wpisu.
    pub(crate) fn put(&self, state: &AccountState) -> io::Result<()> {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let at = inner.states.iter().position(|s| s.person == state.person);
        match (at, state.logged_in() && !state.person.is_empty()) {
            (Some(i), true) => inner.states[i] = state.clone(),
            (None, true) => inner.states.push(state.clone()),
            (Some(i), false) => {
                inner.states.remove(i);
            }
            (None, false) => {}
        }
        Self::write(&self.path, &inner)
    }

    /// Cała lista kont (np. po usunięciu osoby).
    pub(crate) fn replace(&self, states: Vec<AccountState>) -> io::Result<()> {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        inner.states = states
            .into_iter()
            .filter(|s| s.logged_in() && !s.person.is_empty())
            .collect();
        Self::write(&self.path, &inner)
    }

    fn write(path: &Path, inner: &StateFileInner) -> io::Result<()> {
        if inner.loading {
            return Ok(());
        }
        save_states(&inner.states, path)
    }
}
