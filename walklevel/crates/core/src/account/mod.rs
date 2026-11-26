//! Konto online - OPCJONALNY dodatek (port bieznia/konto). Bez logowania aplikacja działa
//! w całości lokalnie, nic nie wysyła, a funkcji online w ogóle nie widać.
//!
//! Konto = osoba w aplikacji (jedno konto, jedna osoba; profile gier dalej dowolne).
//! Zalogowanych może być kilka osób naraz, obok osób bez konta ([`Accounts`]). Token każdej
//! osoby osobno w Menedżerze poświadczeń ([`vault`]) - albo tylko w pamięci, gdy „Zapamiętaj
//! logowanie” jest wyłączone. Stan kont (bez tokenu) w dane/konto.json ([`state`]).
//!
//! Na serwer idą wyłącznie sesje JEDNEJ osoby powiązanej z kontem - bez nazw osób i profili
//! gier, bez sesji z symulatora ([`upload`]). Sesje z innych komputerów tego konta przychodzą
//! synchronizacją ([`sync`], docs/synchro.md).
//!
//! Wszystkie metody sieciowe są BLOKUJĄCE (reqwest::blocking) - aplikacja woła je z wątku
//! roboczego (`spawn_blocking` / osobny wątek), nigdy wewnątrz zadania tokio. Czas (co ile
//! wysyłka, synchronizacja, odświeżenie) liczy aplikacja przez [`sync::AccountTimers`] -
//! moduł nie uruchamia własnych wątków.

pub mod api;
pub mod online;
pub mod state;
pub mod sync;
pub mod upload;
pub mod vault;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use regex::Regex;
use serde::Serialize;
use serde_json::Value;

use crate::history::{History, HistoryError};
use crate::py::truthy;
use crate::teksty::t;

pub use api::{
    Auth, ChangesPage, Client, HttpReply, HttpRequest, ProfilePatch, RANKING_SCOPES, RemoteSession,
    ReqwestTransport, Tombstone, Transport, TransportError, User, check_base_url, field_errors,
    friendly,
};
pub use state::{AccountState, STATE_FILE, load_state, load_states, save_state, save_states};
pub use sync::{AccountTimers, Every, LiveUpdate, NewGame, SyncReport, SyncTarget, live_update};
pub use upload::{DELETED_ELSEWHERE, UploadSession, game_name, session_payload, valid_game_id};
pub use vault::{MemoryStore, TARGET, TokenStore, default_store, vault_target};

use state::StateFile;

pub const VISIBILITY: [&str; 3] = ["prywatna", "znajomi", "publiczna"];
pub const PROFILE_VISIBILITY: [&str; 3] = ["ukryty", "znajomi", "publiczny"];
pub const GAMES_VISIBILITY: [&str; 3] = ["ja", "znajomi", "wszyscy"];
pub const GAME_NAME_MAX: usize = 40;
/// Sesja odrzucona przez serwer: ponowienie po dobie.
pub const REJECTED_RETRY_S: f64 = 24.0 * 3600.0;
/// Data regulaminu i polityki prywatności konta online (gdy serwer nie poda swojej).
pub const TERMS_VERSION: &str = "2026-11-25";
pub const NAME_MIN: usize = 3;
pub const NAME_MAX: usize = 24;
pub const PASSWORD_MIN: usize = 10;
pub const CODE_MIN: usize = 4;
/// Pseudonim jak na serwerze: łacinka z polskimi literami, cyfry, spacja, _, - i kropka
/// (bez cyrylicy: jej litery wyglądają jak łacińskie - podszywanie się).
pub const NAME_RE: &str = r"^[A-Za-z0-9 _.\-ĄĆĘŁŃÓŚŹŻąćęłńóśźż]+$";
pub const EMAIL_RE: &str = r"^[^@\s]{1,64}@[^@\s]{1,190}\.[^@\s]{2,24}$";
pub const RESERVED_NAMES: [&str; 6] = [
    "admin",
    "administrator",
    "moderator",
    "damnit",
    "bieznia",
    "bieżnia",
];
/// Komunikat po „Przypomnij hasło” (zawsze ten sam - nie zdradza, czy konto istnieje).
pub fn remind_message() -> String {
    t("konto.reset.wyslano", &[])
}

static NAME: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(NAME_RE).ok());
static EMAIL: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(EMAIL_RE).ok());

// --- błędy -----------------------------------------------------------------------------------

/// Rodzaj błędu konta (do `CmdError.code` w aplikacji).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum KontoErrorKind {
    /// złe dane w formularzu (sprawdzone przed wysłaniem)
    Invalid,
    /// brak połączenia z serwerem
    Offline,
    /// token nieważny albo wygasł - konto zostało już wylogowane lokalnie
    LoggedOut,
    /// serwer nie przyjmuje tej wersji aplikacji (426)
    UpdateRequired,
    /// serwer odpowiedział błędem albo w nieznanym formacie
    Server,
    /// zapis lokalny (konto.json, historia)
    Io,
}

/// Błąd konta: komunikat dla użytkownika (po polsku, bez szczegółów technicznych), kod HTTP
/// (0 - bez odpowiedzi serwera) i podpowiedzi przy polach formularza.
#[derive(Debug, Clone, PartialEq, Serialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{message}")]
pub struct KontoError {
    pub kind: KontoErrorKind,
    pub message: String,
    pub status: u16,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, String>,
}

impl KontoError {
    pub fn new(kind: KontoErrorKind, message: &str, status: u16) -> Self {
        Self {
            kind,
            message: message.to_string(),
            status,
            fields: BTreeMap::new(),
        }
    }

    pub fn invalid(message: &str) -> Self {
        Self::new(KontoErrorKind::Invalid, message, 0)
    }

    /// Błąd przy polu formularza (`code`, `email`, `password`, `name`).
    pub fn invalid_field(field: &str, message: &str) -> Self {
        let mut error = Self::invalid(message);
        error.fields.insert(field.to_string(), message.to_string());
        error
    }

    pub fn server(message: &str, status: u16) -> Self {
        Self::new(KontoErrorKind::Server, message, status)
    }

    pub fn is_logged_out(&self) -> bool {
        self.kind == KontoErrorKind::LoggedOut
    }

    pub fn is_update_required(&self) -> bool {
        self.kind == KontoErrorKind::UpdateRequired
    }
}

impl From<HistoryError> for KontoError {
    fn from(e: HistoryError) -> Self {
        log::warn!("Konto: błąd historii: {e}");
        Self::new(KontoErrorKind::Io, &t("historia.niedostepna", &[]), 0)
    }
}

impl From<std::io::Error> for KontoError {
    fn from(e: std::io::Error) -> Self {
        log::warn!("Konto: błąd zapisu: {e}");
        Self::new(KontoErrorKind::Io, &t("konto.blad.zapis", &[]), 0)
    }
}

pub type Result<T> = std::result::Result<T, KontoError>;

// --- sprawdzanie pól -------------------------------------------------------------------------

/// Pusty tekst = poprawny; inaczej podpowiedź do pola.
pub fn check_name(name: &str) -> String {
    if name != name.trim() {
        return t("konto.blad.pseudonim_spacje", &[]);
    }
    let len = name.chars().count();
    if !(NAME_MIN..=NAME_MAX).contains(&len) {
        return t(
            "konto.blad.pseudonim_dlugosc",
            &[
                ("min", &NAME_MIN.to_string()),
                ("max", &NAME_MAX.to_string()),
            ],
        );
    }
    if !NAME.as_ref().is_some_and(|re| re.is_match(name)) {
        return t("konto.blad.pseudonim_znaki", &[]);
    }
    if RESERVED_NAMES.contains(&name.to_lowercase().as_str()) {
        return t("konto.blad.pseudonim_zarezerwowany", &[]);
    }
    String::new()
}

pub fn check_code(code: &str) -> String {
    if code.trim().chars().count() >= CODE_MIN {
        String::new()
    } else {
        t("konto.blad.kod", &[])
    }
}

pub fn check_email(email: &str) -> String {
    if EMAIL.as_ref().is_some_and(|re| re.is_match(email.trim())) {
        String::new()
    } else {
        t("konto.blad.email", &[])
    }
}

pub fn check_password(password: &str) -> String {
    if password.chars().count() < PASSWORD_MIN {
        t(
            "konto.blad.haslo_krotkie",
            &[("liczba", &PASSWORD_MIN.to_string())],
        )
    } else {
        String::new()
    }
}

/// Zapis dla serwera: małe litery z myślnikami (36 znaków). Lokalnie 32 znaki hex.
pub fn canonical_uuid(session_id: &str) -> String {
    let h = session_id.replace('-', "").to_lowercase();
    if h.chars().count() != 32 || !h.is_ascii() {
        return session_id.to_string();
    }
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    )
}

/// Id sesji lokalnie (32 znaki hex) z uuid serwera; None - to nie jest uuid.
pub fn local_session_id(uuid: &str) -> Option<String> {
    let h = uuid.replace('-', "").to_lowercase();
    (h.len() == 32 && h.bytes().all(|b| b.is_ascii_hexdigit())).then_some(h)
}

// --- konto jednej osoby ----------------------------------------------------------------------

/// Gdzie konto zapisuje swój stan.
enum Saver {
    /// samodzielne konto: własny plik
    File(PathBuf),
    /// konto z [`Accounts`]: wspólny plik wszystkich osób
    Shared(Arc<StateFile>),
    /// konto tymczasowe (logowanie jako nowa osoba) - bez zapisu
    Nothing,
}

/// Konto: logowanie, wysyłka i pobieranie sesji, dane online. Metody sieciowe - z wątku
/// roboczego. Błąd [`KontoErrorKind::LoggedOut`] znaczy, że konto już się wylogowało lokalnie.
pub struct Account {
    pub state: AccountState,
    client: Client,
    store: Arc<dyn TokenStore>,
    saver: Saver,
    enabled: bool,
    /// serwer bez trasy synchronizacji (404) - do ponownego zalogowania tylko wysyłamy
    sync_missing: bool,
}

impl std::fmt::Debug for Account {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Account")
            .field("state", &self.state)
            .field("client", &self.client)
            .field("enabled", &self.enabled)
            .finish()
    }
}

impl Account {
    /// Samodzielne konto ze stanem z `state_path` (jak `Account(base_url, store, path)`).
    /// Pusty adres - konto wyłączone; zły adres - błąd.
    pub fn open(
        base_url: &str,
        store: Arc<dyn TokenStore>,
        state_path: &Path,
        transport: Arc<dyn Transport>,
    ) -> Result<Self> {
        let state = load_state(state_path);
        Self::build(
            base_url,
            store,
            Saver::File(state_path.to_path_buf()),
            transport,
            state,
            "",
        )
    }

    fn build(
        base_url: &str,
        store: Arc<dyn TokenStore>,
        saver: Saver,
        transport: Arc<dyn Transport>,
        state: AccountState,
        token: &str,
    ) -> Result<Self> {
        let enabled = !base_url.trim().is_empty();
        let token = if !token.is_empty() {
            token.to_string()
        } else if state.remember {
            store.get()
        } else {
            String::new()
        };
        let mut client = if enabled {
            Client::new(base_url, &token, transport)?
        } else {
            Client::disabled(transport)
        };
        client.set_token(&token);
        let mut account = Self {
            state,
            client,
            store,
            saver,
            enabled,
            sync_missing: false,
        };
        if account.state.logged_in() && account.client.token().is_empty() {
            // brak tokenu (bez zapamiętania albo inny użytkownik Windows)
            account.forget();
        }
        Ok(account)
    }

    /// Konto online w ogóle dostępne (adres serwera w config.toml).
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Klient do wywołań tras wprost (znajomi, ranking, osoby...). Po błędzie 401 klient sam
    /// czyści token - wtedy wołaj [`Account::handle`], żeby konto się wylogowało.
    pub fn client_mut(&mut self) -> &mut Client {
        &mut self.client
    }

    pub fn store(&self) -> &Arc<dyn TokenStore> {
        &self.store
    }

    /// Wynik wywołania sieciowego: przy [`KontoErrorKind::LoggedOut`] konto się wylogowuje
    /// (bez pętli ponowień), błąd idzie dalej.
    pub fn handle<T>(&mut self, result: Result<T>) -> Result<T> {
        if let Err(e) = &result
            && e.is_logged_out()
            && self.state.logged_in()
        {
            log::info!(
                "Konto: token nieważny - wylogowano osobę {}",
                self.state.person
            );
            self.forget();
        }
        result
    }

    /// Zapamiętaj logowanie: token do sejfu (albo z niego usunięty - tylko w pamięci).
    pub fn set_remember(&mut self, on: bool) -> Result<()> {
        self.state.remember = on;
        if on && !self.client.token().is_empty() {
            self.write_vault(self.client.token().to_string().as_str());
        } else if !on {
            self.store.clear();
        }
        self.save()
    }

    fn write_vault(&self, token: &str) {
        if let Err(e) = self.store.set(token) {
            // jak „Menedżer poświadczeń niedostępny” w 0.5.x - logowanie tylko do zamknięcia
            log::warn!("Konto: nie udało się zapisać tokenu w Menedżerze poświadczeń: {e}");
        }
    }

    fn keep_token(&mut self, token: &str) {
        self.client.set_token(token);
        if self.state.remember {
            self.write_vault(token);
        } else {
            self.store.clear();
        }
    }

    // --- logowanie -----------------------------------------------------------------------

    /// Zalogowano (rejestracja, logowanie, odświeżenie): token i dane użytkownika. Inne konto
    /// niż dotąd kasuje kursor synchronizacji i znacznik ponownej wysyłki po regulaminie.
    pub fn signed_in(&mut self, auth: &Auth, person: &str) -> Result<User> {
        self.keep_token(&auth.token);
        if self.state.user_id != auth.user.id {
            self.state.sync_cursor.clear();
            self.state.resent_terms.clear();
            self.sync_missing = false;
        }
        self.state.user_id.clone_from(&auth.user.id);
        self.state.name.clone_from(&auth.user.name);
        self.state.email.clone_from(&auth.user.email);
        self.state.verified = auth.user.verified;
        if !person.is_empty() {
            self.state.person = person.to_string();
        }
        self.save()?;
        Ok(auth.user.clone())
    }

    /// Konto na kod zaproszenia - od razu zalogowane (rejestracja publiczna zamknięta).
    /// Złe pola - błąd [`KontoErrorKind::Invalid`] z `fields`, bez pytania serwera.
    pub fn register(
        &mut self,
        code: &str,
        email: &str,
        password: &str,
        name: &str,
        person: &str,
    ) -> Result<User> {
        let checks = [
            ("code", check_code(code)),
            ("email", check_email(email)),
            ("password", check_password(password)),
            ("name", check_name(name)),
        ];
        if let Some((field, problem)) = checks.iter().find(|(_, p)| !p.is_empty()) {
            return Err(KontoError::invalid_field(field, problem));
        }
        let mut version = self.client.terms_version(); // obowiązująca z serwera
        if version.is_empty() {
            version = TERMS_VERSION.to_string();
        }
        let auth = self
            .client
            .register(code, email.trim(), password, name, &version)?;
        self.state.terms = version;
        self.signed_in(&auth, person)
    }

    pub fn login(&mut self, email: &str, password: &str, person: &str) -> Result<User> {
        let auth = self.client.login(email.trim(), password)?;
        self.signed_in(&auth, person)
    }

    /// Wylogowanie: stan na żywo na serwerze czyszczony (bez znaczenia, czy się uda).
    pub fn logout(&mut self) {
        if self.state.live && !self.client.token().is_empty() {
            let _ = self.client.clear_live();
        }
        self.forget();
    }

    /// Usunięcie wymaga hasła - sam token (np. skradziony) nie skasuje konta.
    pub fn delete_account(&mut self, password: &str) -> Result<()> {
        if password.is_empty() {
            return Err(KontoError::invalid_field(
                "password",
                "Wpisz hasło, żeby usunąć konto.",
            ));
        }
        let result = self.client.delete_account(password);
        self.handle(result)?;
        self.forget();
        Ok(())
    }

    /// Nowe hasło: serwer unieważnia stare tokeny, nowy z odpowiedzi zostaje.
    pub fn change_password(&mut self, old: &str, new: &str) -> Result<()> {
        let problem = check_password(new);
        if !problem.is_empty() {
            return Err(KontoError::invalid_field("password", &problem));
        }
        let result = self.client.change_password(old, new);
        let auth = self.handle(result)?;
        self.keep_token(&auth.token); // stary token przestał działać
        Ok(())
    }

    /// Przy starcie i co dobę: świeży token i ustawienia profilu z serwera.
    pub fn refresh(&mut self) -> Result<User> {
        let result = self.client.refresh();
        let auth = self.handle(result)?;
        let user = self.signed_in(&auth, "")?;
        // profil bez znaczenia dla odświeżenia (inny błąd pomijamy), ale 401 wylogowuje
        let result = self.client.my_profile();
        match self.handle(result) {
            Ok(profile) => {
                self.apply_profile(&profile);
                self.save()?;
            }
            Err(e) if e.is_logged_out() => return Err(e),
            Err(_) => {}
        }
        Ok(user)
    }

    /// Ustawienia konta z `GET /api/bieznia/profil` (bez zapisu).
    pub fn apply_profile(&mut self, profile: &Value) {
        let s = &mut self.state;
        if let Some(v) = profile.get("widocznosc_profilu").and_then(Value::as_str)
            && PROFILE_VISIBILITY.contains(&v)
        {
            s.profile_visibility = v.to_string();
        }
        if let Some(v) = profile.get("widocznosc_gier").and_then(Value::as_str)
            && GAMES_VISIBILITY.contains(&v)
        {
            s.games_visibility = v.to_string();
        }
        if let Some(v) = profile.get("regulamin_aktualny").filter(|v| truthy(v)) {
            s.terms_current = value_text(v);
        }
        if let Some(v) = profile.get("regulamin").filter(|v| truthy(v)) {
            s.terms = value_text(v);
        }
        if let Some(v) = profile.get("verified") {
            // po kliknięciu linku z maila
            s.verified = truthy(v);
        }
    }

    /// Czy adres jest już potwierdzony. Link z maila otwiera się w przeglądarce, więc
    /// aplikacja nie dostaje sygnału - musi zapytać serwer.
    pub fn check_verified(&mut self) -> Result<bool> {
        let result = self.client.my_profile();
        let profile = self.handle(result)?;
        if let Some(v) = profile.get("verified") {
            self.state.verified = truthy(v);
            self.save()?;
        }
        Ok(self.state.verified)
    }

    /// Link potwierdzający -> "wyslano" albo "potwierdzony" (wtedy adres oznaczony).
    pub fn send_verification(&mut self) -> Result<String> {
        let result = self.client.send_verification();
        let answer = self.handle(result)?;
        if answer == "potwierdzony" && !self.state.verified {
            self.state.verified = true;
            self.save()?;
        }
        Ok(answer)
    }

    /// Link do nowego hasła (działa też bez logowania); komunikat: [`REMIND_MESSAGE`].
    pub fn remind_password(&mut self, email: &str) -> Result<()> {
        self.client.remind_password(email)
    }

    /// Wylogowanie lokalne: token z sejfu i z pamięci, osoba i „zapamiętaj” zostają.
    pub fn forget(&mut self) {
        self.store.clear();
        self.client.set_token("");
        let person = std::mem::take(&mut self.state.person);
        let remember = self.state.remember;
        self.state = AccountState {
            person,
            remember,
            ..AccountState::default()
        };
        self.sync_missing = false;
        if let Err(e) = self.save() {
            log::warn!("Konto: nie udało się zapisać wylogowania: {e}");
        }
    }

    pub fn save(&self) -> Result<()> {
        match &self.saver {
            Saver::File(path) => save_state(&self.state, path)?,
            Saver::Shared(file) => file.put(&self.state)?,
            Saver::Nothing => {}
        }
        Ok(())
    }

    // --- ustawienia konta ----------------------------------------------------------------

    /// Akceptacja obowiązującego regulaminu. Najbliższa wysyłka ([`Account::upload`]) oznaczy
    /// całą historię do ponownej wysyłki - z grami i nowszym `zmieniono_utc_s`.
    pub fn accept_terms(&mut self) -> Result<()> {
        let version = self.state.terms_current.clone();
        if version.is_empty() {
            return Ok(());
        }
        let patch = ProfilePatch {
            terms: Some(version.clone()),
            ..ProfilePatch::default()
        };
        let result = self.client.update_profile(&patch);
        self.handle(result)?;
        self.state.terms = version;
        self.save()
    }

    pub fn set_games_visibility(&mut self, value: &str) -> Result<()> {
        if !GAMES_VISIBILITY.contains(&value) {
            return Err(KontoError::invalid(&t("wspolne.blad_nieznany", &[])));
        }
        let patch = ProfilePatch {
            games_visibility: Some(value.to_string()),
            ..ProfilePatch::default()
        };
        let result = self.client.update_profile(&patch);
        self.handle(result)?;
        self.state.games_visibility = value.to_string();
        self.save()
    }

    pub fn set_profile_visibility(&mut self, value: &str) -> Result<()> {
        if !PROFILE_VISIBILITY.contains(&value) {
            return Err(KontoError::invalid(&t("wspolne.blad_nieznany", &[])));
        }
        let patch = ProfilePatch {
            visibility: Some(value.to_string()),
            ..ProfilePatch::default()
        };
        let result = self.client.update_profile(&patch);
        self.handle(result)?;
        self.state.profile_visibility = value.to_string();
        self.save()
    }

    /// Nowy pseudonim (sprawdzony przed wysłaniem).
    pub fn rename(&mut self, name: &str) -> Result<()> {
        let problem = check_name(name);
        if !problem.is_empty() {
            return Err(KontoError::invalid_field("name", &problem));
        }
        let patch = ProfilePatch {
            name: Some(name.to_string()),
            ..ProfilePatch::default()
        };
        let result = self.client.update_profile(&patch);
        let user = self.handle(result)?;
        self.state.name = if user.name.is_empty() {
            name.to_string()
        } else {
            user.name
        };
        self.save()
    }

    /// Widoczność wysyłanych sesji; wszystkie sesje pójdą jeszcze raz - z nową widocznością.
    pub fn set_visibility(&mut self, history: &History, visibility: &str) -> Result<()> {
        if !VISIBILITY.contains(&visibility) {
            return Err(KontoError::invalid(&t("wspolne.blad_nieznany", &[])));
        }
        self.state.visibility = visibility.to_string();
        self.save()?;
        history.forget_sent()?;
        Ok(())
    }

    /// „Wysyłaj moje sesje” (lokalnie).
    pub fn set_upload(&mut self, on: bool) -> Result<()> {
        self.state.upload = on;
        self.save()
    }

    /// „Na żywo dla znajomych”; wyłączenie czyści stan na serwerze.
    pub fn set_live(&mut self, on: bool) -> Result<()> {
        self.state.live = on;
        self.save()?;
        if !on && self.state.logged_in() {
            let result = self.client.clear_live();
            self.handle(result)?;
        }
        Ok(())
    }

    /// Stan na żywo (co [`sync::LIVE_EVERY_S`], gdy osoba idzie i ma włączone „na żywo”).
    pub fn live(&mut self, kmh: f64, steps: i64, active_s: f64, session_id: &str) -> Result<()> {
        if self.state.logged_in() && self.state.live {
            let result = self.client.put_live(kmh, steps, active_s, session_id);
            self.handle(result)?;
        }
        Ok(())
    }

    pub fn clear_live(&mut self) -> Result<()> {
        if self.state.logged_in() {
            let result = self.client.clear_live();
            self.handle(result)?;
        }
        Ok(())
    }

    /// Dane konta z serwera do `dane/moje-dane-konto-YYYYmmdd-HHMMSS.json` w folderze danych
    /// (`stamp` - "YYYYmmdd-HHMMSS" czasu lokalnego). Zwraca ścieżkę pliku.
    pub fn export_my_data(&mut self, data_dir: &Path, stamp: &str) -> Result<PathBuf> {
        let result = self.client.my_data();
        let data = self.handle(result)?;
        let folder = data_dir.join("dane");
        std::fs::create_dir_all(&folder)?;
        let target = folder.join(format!("moje-dane-konto-{stamp}.json"));
        let mut out = Vec::new();
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b" ");
        let mut ser = serde_json::Serializer::with_formatter(&mut out, formatter);
        data.serialize(&mut ser)
            .map_err(|_| KontoError::server(&t("konto.blad.format", &[]), 0))?;
        std::fs::write(&target, out)?;
        Ok(target)
    }
}

fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

// --- konta wszystkich osób -------------------------------------------------------------------

/// Sejf dla osoby (w aplikacji: Menedżer poświadczeń z celem `Bieznia/konto/<osoba>`).
pub type StoreFor = Box<dyn Fn(&str) -> Arc<dyn TokenStore> + Send + Sync>;

/// Konta wszystkich osób: osoba -> konto (jedno na osobę). Zapis w dane/konto.json.
pub struct Accounts {
    base_url: String,
    enabled: bool,
    file: Arc<StateFile>,
    transport: Arc<dyn Transport>,
    store_for: StoreFor,
    items: Vec<Account>,
}

impl Accounts {
    /// Konta aplikacji: dane/konto.json w folderze danych, tokeny w Menedżerze poświadczeń,
    /// prawdziwa sieć. Pusty albo zły adres serwera - konto online wyłączone.
    pub fn open(base_url: &str, data_dir: &Path) -> Self {
        let legacy = default_store(TARGET);
        Self::with_parts(
            base_url,
            &data_dir.join(STATE_FILE),
            Arc::new(ReqwestTransport::new()),
            Box::new(|person: &str| default_store(&vault_target(person))),
            Some(legacy),
        )
    }

    /// Konta z podanych części (testy: atrapa serwera i sejfy w pamięci). `legacy` - sejf
    /// wersji z jednym kontem (`Bieznia/konto`), z którego token przenosi się do osoby.
    pub fn with_parts(
        base_url: &str,
        path: &Path,
        transport: Arc<dyn Transport>,
        store_for: StoreFor,
        legacy: Option<Arc<dyn TokenStore>>,
    ) -> Self {
        let mut base = base_url.trim().to_string();
        if !base.is_empty()
            && let Err(e) = check_base_url(&base)
        {
            log::warn!("Konto online wyłączone: {e}");
            base.clear();
        }
        let file = Arc::new(StateFile::new(path));
        let mut accounts = Self {
            enabled: !base.is_empty(),
            base_url: base,
            file,
            transport,
            store_for,
            items: Vec::new(),
        };
        let mut loaded = Vec::new();
        for state in load_states(path) {
            let store = (accounts.store_for)(&state.person);
            if state.remember
                && store.get().is_empty()
                && let Some(legacy) = &legacy
            {
                // wersja z jednym kontem: token pod wspólną nazwą
                let token = legacy.get();
                if !token.is_empty() {
                    match store.set(&token) {
                        Ok(()) => legacy.clear(),
                        Err(e) => log::warn!("Konto: nie udało się przenieść tokenu: {e}"),
                    }
                }
            }
            match accounts.make(state, Some(store), "") {
                Ok(account) if account.state.logged_in() => {
                    loaded.push(account.state.clone());
                    accounts.items.push(account);
                }
                Ok(_) => {}
                Err(e) => log::warn!("Konto: nie wczytano konta: {e}"),
            }
        }
        // stan po wczytaniu, bez zapisu (plik zmieni się przy pierwszej zmianie)
        let _ = accounts.file.replace(loaded);
        accounts.file.finish_loading();
        accounts
    }

    fn make(
        &self,
        state: AccountState,
        store: Option<Arc<dyn TokenStore>>,
        token: &str,
    ) -> Result<Account> {
        let store = store.unwrap_or_else(|| (self.store_for)(&state.person));
        Account::build(
            &self.base_url,
            store,
            Saver::Shared(self.file.clone()),
            self.transport.clone(),
            state,
            token,
        )
    }

    /// Konto online dostępne (adres serwera niepusty i poprawny).
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn path(&self) -> &Path {
        self.file.path()
    }

    /// Konto osoby (wylogowane, jeśli osoba jeszcze nie ma konta).
    pub fn get(&mut self, person: &str) -> &mut Account {
        if let Some(i) = self.items.iter().position(|a| a.state.person == person) {
            return &mut self.items[i];
        }
        let account = match self.make(AccountState::for_person(person), None, "") {
            Ok(a) => a,
            // adres sprawdzony w with_parts - tu tylko awaryjnie: konto wyłączone
            Err(_) => self.disabled_account(person),
        };
        self.items.push(account);
        let last = self.items.len() - 1;
        &mut self.items[last]
    }

    fn disabled_account(&self, person: &str) -> Account {
        Account {
            state: AccountState::for_person(person),
            client: Client::disabled(self.transport.clone()),
            store: Arc::new(MemoryStore::new()),
            saver: Saver::Nothing,
            enabled: false,
            sync_missing: false,
        }
    }

    /// Konto tymczasowe (logowanie jako nowa osoba) - przypinane do osoby po sukcesie.
    pub fn blank(&self) -> Account {
        let base = if self.enabled {
            self.base_url.as_str()
        } else {
            ""
        };
        match Account::build(
            base,
            Arc::new(MemoryStore::new()),
            Saver::Nothing,
            self.transport.clone(),
            AccountState::default(),
            "",
        ) {
            Ok(a) => a,
            Err(_) => self.disabled_account(""),
        }
    }

    /// Zalogowane konto tymczasowe -> konto osoby (token do jej miejsca w sejfie).
    pub fn attach(&mut self, account: Account, person: &str) -> Result<&mut Account> {
        let token = account.client.token().to_string();
        let mut state = account.state;
        state.person = person.to_string();
        let mut attached = self.make(state, None, &token)?;
        attached.keep_token(&token);
        self.items.retain(|a| a.state.person != person);
        self.items.push(attached);
        self.save()?;
        let last = self.items.len() - 1;
        Ok(&mut self.items[last])
    }

    /// Zalogowane konta (do wysyłki i synchronizacji każdej osoby jej kontem).
    pub fn logged_in(&self) -> impl Iterator<Item = &Account> {
        self.items.iter().filter(|a| a.state.logged_in())
    }

    /// Osoba, do której przypięte jest konto użytkownika ("" - żadna).
    pub fn person_of(&self, user_id: &str) -> String {
        self.logged_in()
            .find(|a| a.state.user_id == user_id)
            .map(|a| a.state.person.clone())
            .unwrap_or_default()
    }

    /// Inna osoba niż `person`, do której jest już przypięte konto `user_id` (jedno konto,
    /// jedna osoba) - logowanie dla `person` trzeba wtedy odrzucić.
    pub fn taken_by(&self, user_id: &str, person: &str) -> Option<String> {
        self.logged_in()
            .find(|a| a.state.user_id == user_id && a.state.person != person)
            .map(|a| a.state.person.clone())
    }

    /// Osoba usunięta z aplikacji - jej konto zostaje na serwerze, tu wylogowane.
    pub fn forget_person(&mut self, person: &str) -> Result<()> {
        if let Some(i) = self.items.iter().position(|a| a.state.person == person) {
            let account = self.items.remove(i);
            if account.state.logged_in() {
                account.store.clear();
            }
        }
        self.save()
    }

    pub fn save(&self) -> Result<()> {
        let states = self.logged_in().map(|a| a.state.clone()).collect();
        self.file.replace(states)?;
        Ok(())
    }
}
