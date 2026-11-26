//! Klient serwera kont (PocketBase + trasy /api/bieznia/*) - port bieznia/konto/api.py.
//!
//! Bezpieczeństwo po stronie klienta (jak w 0.5.x):
//! - tylko HTTPS ze zwykłą weryfikacją certyfikatu (http wyłącznie do 127.0.0.1/localhost -
//!   serwer testowy na tym komputerze), przekierowanie na inny host przerywa żądanie,
//! - token tylko w nagłówku Authorization, nigdy w adresie; hasło nie jest nigdzie zapisywane,
//! - treści żądań i odpowiedzi nie trafiają do logów (tylko metoda, ścieżka, kod i czas),
//! - 401 = sesja wygasła -> [`KontoErrorKind::LoggedOut`] (token czyszczony, bez ponawiania),
//! - 426 = serwer wymaga nowszej aplikacji -> [`KontoErrorKind::UpdateRequired`],
//! - nagłówek X-Bieznia-Wersja w każdym żądaniu.
//!
//! Sieć jest BLOKUJĄCA (`reqwest::blocking`, jak aktualizacje i zgłoszenia): metody klienta
//! i konta wołamy z wątku roboczego (`spawn_blocking` albo osobny wątek), nigdy wewnątrz
//! zadania tokio. Transport to cecha [`Transport`] - testy podstawiają atrapę serwera.

use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{Map, Value, json};

use super::{KontoError, KontoErrorKind};

/// Limit czasu połączenia i odpowiedzi (sekundy).
pub const TIMEOUT_S: u64 = 15;
/// Najdłuższa odpowiedź serwera.
pub const MAX_RESPONSE: usize = 2 * 1024 * 1024;
/// Sesji w jednym żądaniu (limit treści serwera: 64 KB).
pub const BATCH: usize = 100;
/// Zakresy rankingu i statystyk (wartości jak na serwerze).
pub const RANKING_SCOPES: [&str; 4] = ["tydzien", "miesiac", "razem", "poziom"];

const BAD_FORMAT: &str = "Serwer odpowiedział w nieznanym formacie.";
const REDIRECTED: &str = "Serwer przekierował na inny adres - przerwano.";
const OFFLINE: &str = "Brak połączenia z serwerem kont - sprawdź internet.";

// --- transport -------------------------------------------------------------------------------

/// Żądanie HTTP przygotowane przez [`Client`].
#[derive(Debug, Clone, PartialEq)]
pub struct HttpRequest {
    /// "GET", "POST", "PUT", "PATCH", "DELETE"
    pub method: String,
    /// pełny adres (z parametrami zapytania)
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

impl HttpRequest {
    /// Wartość nagłówka (bez względu na wielkość liter nazwy).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Odpowiedź HTTP z dowolnym kodem (także błędu) - kod ocenia [`Client`].
#[derive(Debug, Clone, PartialEq)]
pub struct HttpReply {
    pub status: u16,
    /// adres po przekierowaniach
    pub url: String,
    /// treść (najwyżej [`MAX_RESPONSE`] + 1 bajtów)
    pub body: Vec<u8>,
}

/// Brak połączenia, przekroczony czas, błąd TLS itp. (szczegół tylko do logów).
#[derive(Debug, Clone, thiserror::Error)]
#[error("{0}")]
pub struct TransportError(pub String);

/// Wysyłka żądań - w aplikacji [`ReqwestTransport`], w testach atrapa serwera.
pub trait Transport: Send + Sync {
    fn send(&self, request: &HttpRequest) -> Result<HttpReply, TransportError>;
}

/// Prawdziwy transport: `reqwest::blocking` (rustls). Klient powstaje przy pierwszym
/// żądaniu (czyli już w wątku roboczym), więc samo utworzenie kont nie dotyka sieci.
#[derive(Default)]
pub struct ReqwestTransport {
    client: OnceLock<Result<reqwest::blocking::Client, String>>,
}

impl ReqwestTransport {
    pub fn new() -> Self {
        Self::default()
    }

    fn client(&self) -> Result<&reqwest::blocking::Client, TransportError> {
        self.client
            .get_or_init(|| {
                // przekierowanie tylko w obrębie tego samego hosta; na inny host - stop
                // (odpowiedź 3xx, którą Client odrzuca)
                let redirects = reqwest::redirect::Policy::custom(|attempt| {
                    let first = attempt.previous().first().and_then(|u| u.host_str());
                    if first != attempt.url().host_str() || attempt.previous().len() > 10 {
                        attempt.stop()
                    } else {
                        attempt.follow()
                    }
                });
                reqwest::blocking::Client::builder()
                    .timeout(Duration::from_secs(TIMEOUT_S))
                    .connect_timeout(Duration::from_secs(TIMEOUT_S))
                    .redirect(redirects)
                    .build()
                    .map_err(|e| e.to_string())
            })
            .as_ref()
            .map_err(|e| TransportError(e.clone()))
    }
}

impl Transport for ReqwestTransport {
    fn send(&self, request: &HttpRequest) -> Result<HttpReply, TransportError> {
        let client = self.client()?;
        let method = reqwest::Method::from_bytes(request.method.as_bytes())
            .map_err(|e| TransportError(e.to_string()))?;
        let mut builder = client.request(method, &request.url);
        for (name, value) in &request.headers {
            builder = builder.header(name.as_str(), value.as_str());
        }
        if let Some(body) = &request.body {
            builder = builder.body(body.clone());
        }
        let response = builder.send().map_err(|e| TransportError(e.to_string()))?;
        let status = response.status().as_u16();
        let url = response.url().to_string();
        let mut body = Vec::new();
        response
            .take(MAX_RESPONSE as u64 + 1)
            .read_to_end(&mut body)
            .map_err(|e| TransportError(e.to_string()))?;
        Ok(HttpReply { status, url, body })
    }
}

// --- adres i komunikaty ----------------------------------------------------------------------

/// Adres serwera: https (http tylko do 127.0.0.1/localhost). Zwraca adres bez spacji
/// i końcowych ukośników.
pub fn check_base_url(url: &str) -> Result<String, KontoError> {
    let url = url.trim().trim_end_matches('/');
    let https_only = || KontoError::invalid("Adres serwera kont musi zaczynać się od https://");
    let parsed = reqwest::Url::parse(url).map_err(|_| https_only())?;
    let host = parsed.host_str().unwrap_or("");
    let local = matches!(host, "127.0.0.1" | "localhost");
    let scheme = parsed.scheme();
    if host.is_empty() || !(scheme == "https" || (scheme == "http" && local)) {
        return Err(https_only());
    }
    if !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(KontoError::invalid("Niepoprawny adres serwera kont."));
    }
    Ok(url.to_string())
}

fn host_of(url: &str) -> Option<String> {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
}

fn standard_message(status: u16) -> Option<&'static str> {
    Some(match status {
        0 => OFFLINE,
        400 => "Serwer nie przyjął danych.",
        403 => "Brak dostępu - potwierdź adres e-mail (link w wiadomości).",
        404 => "Nie znaleziono.",
        413 => "Za dużo danych naraz.",
        429 => "Za dużo prób - odczekaj chwilę i spróbuj ponownie.",
        _ => return None,
    })
}

/// Komunikat z serwera albo nasz - nigdy surowy błąd. Trasy /api/bieznia/* mówią po polsku
/// (`{message}`); wbudowane trasy PocketBase po angielsku - te mapujemy po kodzie.
pub fn friendly(status: u16, body: &Value, path: &str) -> String {
    let mut body = body;
    if !path.starts_with("/api/bieznia/") {
        if status == 400 && path.ends_with("auth-with-password") {
            return "Nieprawidłowy e-mail lub hasło.".into();
        }
        if status == 429 {
            return "Za dużo prób - spróbuj za minutę.".into();
        }
        body = &Value::Null;
    } else if status == 404 {
        return "Ta funkcja będzie dostępna wkrótce.".into();
    }
    if let Some(message) = body.get("message").and_then(Value::as_str)
        && !message.trim().is_empty()
        && message.chars().count() < 200
    {
        return message.trim().to_string();
    }
    if status >= 500 {
        return "Serwer kont ma chwilowy problem - spróbuj później.".into();
    }
    match standard_message(status) {
        Some(m) => m.to_string(),
        None => format!("Serwer odpowiedział błędem {status}."),
    }
}

/// Błędy pól z PocketBase (`{data: {email: {message}}}`) - do podpowiedzi przy polach.
pub fn field_errors(payload: &Value) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if let Some(data) = payload.get("data").and_then(Value::as_object) {
        for (key, value) in data {
            if let Some(message) = value.get("message").and_then(Value::as_str) {
                out.insert(key.clone(), message.chars().take(160).collect());
            }
        }
    }
    out
}

/// Kodowanie fragmentu ścieżki jak `urllib.parse.quote(text, safe="")`.
fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for &b in text.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

// --- dane z serwera --------------------------------------------------------------------------

/// Użytkownik z rekordu PocketBase.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    pub verified: bool,
}

impl User {
    pub fn from_record(record: &Value) -> Self {
        Self {
            id: text(record, "id"),
            name: text(record, "name"),
            email: text(record, "email"),
            verified: record.get("verified").is_some_and(truthy),
        }
    }
}

/// Token i użytkownik po zalogowaniu, rejestracji, odświeżeniu albo zmianie hasła.
#[derive(Debug, Clone, PartialEq)]
pub struct Auth {
    pub token: String,
    pub user: User,
}

/// Zmiany profilu (PATCH /api/bieznia/profil) - tylko podane pola.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProfilePatch {
    pub name: Option<String>,
    /// widoczność profilu: ukryty / znajomi / publiczny
    pub visibility: Option<String>,
    /// zaakceptowana wersja regulaminu
    pub terms: Option<String>,
    /// kto widzi podział na gry: ja / znajomi / wszyscy
    pub games_visibility: Option<String>,
}

/// Sesja z serwera (trasa synchronizacji, docs/synchro.md). `uuid` z kreskami.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RemoteSession {
    pub uuid: String,
    pub start: f64,
    pub end: f64,
    pub steps: i64,
    pub distance_km: f64,
    pub active_s: f64,
    pub device_model: String,
    pub driver: String,
    pub device_profile: String,
    pub firmware: String,
    pub app_version: String,
    pub source: String,
    pub visibility: String,
    pub tz_offset_min: i64,
    pub updated: f64,
    /// "" (przed regulaminem z grami = "gra"), "gra", "trening"
    pub mode: String,
    pub game_id: String,
    pub game: String,
}

impl RemoteSession {
    pub fn from_value(v: &Value) -> Self {
        Self {
            uuid: text(v, "uuid"),
            start: num(v, "start_utc_s"),
            end: num(v, "koniec_utc_s"),
            steps: num(v, "kroki") as i64,
            distance_km: num(v, "dystans_km"),
            active_s: num(v, "ruch_s"),
            device_model: text(v, "bieznia_model"),
            driver: text(v, "sterownik"),
            device_profile: text(v, "bieznia_profil"),
            firmware: text(v, "firmware"),
            app_version: text(v, "wersja_aplikacji"),
            source: text(v, "zrodlo"),
            visibility: text(v, "widocznosc"),
            tz_offset_min: num(v, "strefa_min") as i64,
            updated: num(v, "zmieniono_utc_s"),
            mode: text(v, "tryb"),
            game_id: text(v, "gra_id"),
            game: text(v, "gra"),
        }
    }
}

/// Nagrobek: sesja usunięta na serwerze.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tombstone {
    pub uuid: String,
    pub deleted_at: f64,
}

/// Jedna strona zmian z serwera.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChangesPage {
    pub sessions: Vec<RemoteSession>,
    pub deleted: Vec<Tombstone>,
    /// nieprzezroczysty kursor do następnego pytania
    pub cursor: String,
    /// od razu pytać dalej
    pub more: bool,
}

/// Tekst pola (liczby i inne wartości jak `str()` w Pythonie; brak/null - pusty).
fn text(v: &Value, key: &str) -> String {
    match v.get(key) {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

fn num(v: &Value, key: &str) -> f64 {
    match v.get(key) {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(0.0),
        Some(Value::Bool(b)) => f64::from(u8::from(*b)),
        _ => 0.0,
    }
}

/// Prawdziwość wartości jak `bool()` w Pythonie.
pub(crate) fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

fn object_or_empty(v: Value) -> Value {
    if v.is_object() {
        v
    } else {
        Value::Object(Map::new())
    }
}

fn list_or_empty(v: Value) -> Value {
    if v.is_array() {
        v
    } else {
        Value::Array(Vec::new())
    }
}

// --- klient ----------------------------------------------------------------------------------

/// Klient serwera kont. Tani w klonowaniu (wspólny transport) - aplikacja może wziąć kopię
/// i pytać serwer bez trzymania blokady konta.
#[derive(Clone)]
pub struct Client {
    base: String,
    host: String,
    token: String,
    transport: Arc<dyn Transport>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // bez tokenu w logach i komunikatach
        f.debug_struct("Client")
            .field("base", &self.base)
            .field("token", &if self.token.is_empty() { "" } else { "***" })
            .finish()
    }
}

impl Client {
    pub fn new(
        base_url: &str,
        token: &str,
        transport: Arc<dyn Transport>,
    ) -> Result<Self, KontoError> {
        let base = check_base_url(base_url)?;
        let host = host_of(&base).unwrap_or_default();
        Ok(Self {
            base,
            host,
            token: token.to_string(),
            transport,
        })
    }

    /// Klient wyłączonego konta online (adres zastępczy jak w 0.5.x - nic na niego nie idzie,
    /// bo konto bez adresu serwera nie jest używane).
    pub(crate) fn disabled(transport: Arc<dyn Transport>) -> Self {
        Self {
            base: "https://127.0.0.1".into(),
            host: "127.0.0.1".into(),
            token: String::new(),
            transport,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn set_token(&mut self, token: &str) {
        self.token = token.to_string();
    }

    /// Jedno żądanie. `query` w adresie (nigdy token), treść jako JSON.
    pub fn call(
        &mut self,
        method: &str,
        path: &str,
        body: Option<&Value>,
        query: &[(&str, &str)],
        auth: bool,
    ) -> Result<Value, KontoError> {
        let mut url = reqwest::Url::parse(&format!("{}{path}", self.base))
            .map_err(|_| KontoError::invalid("Niepoprawny adres serwera kont."))?;
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query.iter().copied());
        }
        let version = crate::version();
        let mut headers = vec![
            ("Accept".to_string(), "application/json".to_string()),
            ("User-Agent".to_string(), format!("Bieznia/{version}")),
            ("X-Bieznia-Wersja".to_string(), version.to_string()),
        ];
        let data = match body {
            Some(b) => {
                headers.push(("Content-Type".into(), "application/json".into()));
                Some(serde_json::to_vec(b).map_err(|_| KontoError::server(BAD_FORMAT, 0))?)
            }
            None => None,
        };
        if auth {
            if self.token.is_empty() {
                return Err(KontoError::new(
                    KontoErrorKind::LoggedOut,
                    "Zaloguj się, żeby korzystać z funkcji online.",
                    401,
                ));
            }
            headers.push(("Authorization".into(), self.token.clone()));
        }
        let request = HttpRequest {
            method: method.to_string(),
            url: url.to_string(),
            headers,
            body: data,
        };
        let began = Instant::now();
        let reply = match self.transport.send(&request) {
            Ok(r) => r,
            Err(e) => {
                log::info!("Konto: {method} {path} -> brak połączenia ({e})");
                return Err(KontoError::new(KontoErrorKind::Offline, OFFLINE, 0));
            }
        };
        let status = reply.status;
        if (300..400).contains(&status) || host_of(&reply.url).as_deref() != Some(&self.host) {
            log::info!("Konto: {method} {path} -> {status} (przekierowanie)");
            return Err(KontoError::server(REDIRECTED, status));
        }
        if !(200..300).contains(&status) {
            log::info!("Konto: {method} {path} -> {status}");
            let raw = &reply.body[..reply.body.len().min(64 * 1024)];
            let payload: Value = if raw.is_empty() {
                Value::Null
            } else {
                serde_json::from_slice(raw).unwrap_or(Value::Null)
            };
            let message = friendly(status, &payload, path);
            if status == 401 {
                self.token.clear();
                return Err(KontoError::new(
                    KontoErrorKind::LoggedOut,
                    "Sesja wygasła - zaloguj się ponownie.",
                    401,
                ));
            }
            if status == 426 {
                return Err(KontoError::new(
                    KontoErrorKind::UpdateRequired,
                    &message,
                    426,
                ));
            }
            let mut error = KontoError::server(&message, status);
            error.fields = field_errors(&payload);
            return Err(error);
        }
        log::info!(
            "Konto: {method} {path} -> {status} ({:.1} s)",
            began.elapsed().as_secs_f64()
        );
        if reply.body.len() > MAX_RESPONSE {
            return Err(KontoError::server(
                "Odpowiedź serwera jest za duża - przerwano.",
                status,
            ));
        }
        if reply.body.is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_slice(&reply.body).map_err(|_| KontoError::server(BAD_FORMAT, status))
    }

    fn get(&mut self, path: &str, query: &[(&str, &str)]) -> Result<Value, KontoError> {
        self.call("GET", path, None, query, true)
    }

    fn post(&mut self, path: &str, body: Option<&Value>) -> Result<Value, KontoError> {
        self.call("POST", path, body, &[], true)
    }

    fn take_auth(&mut self, payload: &Value) -> Result<Auth, KontoError> {
        let token = match payload.get("token") {
            Some(t) if truthy(t) => match t {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            },
            _ => return Err(KontoError::server(BAD_FORMAT, 0)),
        };
        self.token.clone_from(&token);
        let record = payload.get("record").cloned().unwrap_or(Value::Null);
        Ok(Auth {
            token,
            user: User::from_record(&record),
        })
    }

    // --- konto ---------------------------------------------------------------------------

    /// Konto na kod zaproszenia (rejestracja publiczna zamknięta) - od razu zalogowane.
    /// Wersja zaakceptowanego regulaminu zapisuje się przy koncie.
    pub fn register(
        &mut self,
        code: &str,
        email: &str,
        password: &str,
        name: &str,
        terms: &str,
    ) -> Result<Auth, KontoError> {
        let body = json!({
            "kod": code.trim(),
            "email": email,
            "password": password,
            "passwordConfirm": password,
            "name": name,
            "regulamin": terms,
        });
        let payload = self.call("POST", "/api/bieznia/rejestracja", Some(&body), &[], false)?;
        self.take_auth(&payload)
    }

    pub fn login(&mut self, email: &str, password: &str) -> Result<Auth, KontoError> {
        let body = json!({"identity": email, "password": password});
        let payload = self.call(
            "POST",
            "/api/collections/users/auth-with-password",
            Some(&body),
            &[],
            false,
        )?;
        self.take_auth(&payload)
    }

    pub fn refresh(&mut self) -> Result<Auth, KontoError> {
        let payload = self.post("/api/collections/users/auth-refresh", None)?;
        self.take_auth(&payload)
    }

    /// Nowe hasło unieważnia wszystkie tokeny konta - zapisujemy nowy z odpowiedzi.
    pub fn change_password(&mut self, old: &str, new: &str) -> Result<Auth, KontoError> {
        let body = json!({"oldPassword": old, "password": new, "passwordConfirm": new});
        let payload = self.post("/api/bieznia/haslo", Some(&body))?;
        self.take_auth(&payload)
    }

    /// Hasło sprawdza serwer - sam token (np. skradziony) nie skasuje konta.
    pub fn delete_account(&mut self, password: &str) -> Result<(), KontoError> {
        let body = json!({"password": password});
        self.call("DELETE", "/api/bieznia/konto", Some(&body), &[], true)?;
        self.token.clear();
        Ok(())
    }

    pub fn my_profile(&mut self) -> Result<Value, KontoError> {
        Ok(object_or_empty(self.get("/api/bieznia/profil", &[])?))
    }

    /// Obowiązująca wersja regulaminu (trasa publiczna); "" gdy serwer jej jeszcze nie ma.
    pub fn terms_version(&mut self) -> String {
        match self.call("GET", "/api/bieznia/regulamin", None, &[], false) {
            Ok(v) => match v.get("wersja") {
                Some(w) if truthy(w) => match w {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                },
                _ => String::new(),
            },
            Err(_) => String::new(),
        }
    }

    /// Link potwierdzający na adres konta -> "wyslano" albo "potwierdzony" (już był).
    pub fn send_verification(&mut self) -> Result<String, KontoError> {
        let result = self.post("/api/bieznia/wyslij-potwierdzenie", None)?;
        Ok(match result.get("stan") {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Null) | None => "wyslano".into(),
            Some(other) => other.to_string(),
        })
    }

    /// Link do nowego hasła. Serwer zawsze odpowiada tak samo - nie zdradza, czy konto
    /// z tym adresem istnieje.
    pub fn remind_password(&mut self, email: &str) -> Result<(), KontoError> {
        let body = json!({"email": email.trim()});
        self.call(
            "POST",
            "/api/bieznia/przypomnij-haslo",
            Some(&body),
            &[],
            false,
        )?;
        Ok(())
    }

    // --- sesje ---------------------------------------------------------------------------

    /// Paczkami po [`BATCH`]. Zwraca odrzucone sesje {uuid: powód} - zła sesja nie wywraca
    /// paczki, a serwer zostawia nowszą wersję (wyższe zmieniono_utc_s).
    pub fn upload_sessions<T: Serialize>(
        &mut self,
        sessions: &[T],
    ) -> Result<HashMap<String, String>, KontoError> {
        let mut rejected = HashMap::new();
        for chunk in sessions.chunks(BATCH) {
            let body = json!({ "sesje": chunk });
            let result = self.post("/api/bieznia/sesje", Some(&body))?;
            if let Some(items) = result.get("odrzucone").and_then(Value::as_array) {
                for item in items.iter().filter(|i| i.is_object()) {
                    let reason: String = text(item, "powod").chars().take(120).collect();
                    rejected.insert(text(item, "uuid"), reason);
                }
            }
        }
        Ok(rejected)
    }

    /// Sesja usunięta z konta (404 = serwer jej nie miał - też dobrze).
    pub fn delete_session(&mut self, uuid: &str) -> Result<(), KontoError> {
        let path = format!("/api/bieznia/sesje/{}", quote(uuid));
        match self.call("DELETE", &path, None, &[], true) {
            Err(e) if e.status == 404 => Ok(()),
            other => other.map(|_| ()),
        }
    }

    /// Strona zmian od kursora (pusty kursor = od początku), docs/synchro.md. Błąd 400/404/410
    /// zwracany z kodem w `status` - decyduje synchronizacja.
    pub fn changes(&mut self, cursor: &str, limit: u32) -> Result<ChangesPage, KontoError> {
        let limit = limit.clamp(1, 500).to_string();
        let mut query = vec![("limit", limit.as_str())];
        if !cursor.is_empty() {
            query.insert(0, ("kursor", cursor));
        }
        let result = self.get("/api/bieznia/sesje", &query)?;
        if !result.is_object() {
            return Err(KontoError::server(BAD_FORMAT, 200));
        }
        let list = |key: &str| -> Vec<Value> {
            result
                .get(key)
                .and_then(Value::as_array)
                .map(|a| a.iter().filter(|v| v.is_object()).cloned().collect())
                .unwrap_or_default()
        };
        Ok(ChangesPage {
            sessions: list("sesje")
                .iter()
                .map(RemoteSession::from_value)
                .collect(),
            deleted: list("usuniete")
                .iter()
                .map(|v| Tombstone {
                    uuid: text(v, "uuid"),
                    deleted_at: num(v, "usunieto_utc_s"),
                })
                .collect(),
            cursor: text(&result, "kursor"),
            more: result.get("wiecej").is_some_and(truthy),
        })
    }

    /// Stan na żywo dla znajomych.
    pub fn put_live(
        &mut self,
        kmh: f64,
        steps: i64,
        active_s: f64,
        session_uuid: &str,
    ) -> Result<(), KontoError> {
        let body = json!({
            "kmh": (kmh * 10.0).round() / 10.0,
            "kroki": steps,
            "ruch_s": active_s.trunc() as i64,
            "sesja_uuid": session_uuid,
        });
        self.call("PUT", "/api/bieznia/stan", Some(&body), &[], true)?;
        Ok(())
    }

    pub fn clear_live(&mut self) -> Result<(), KontoError> {
        self.call("DELETE", "/api/bieznia/stan", None, &[], true)?;
        Ok(())
    }

    // --- znajomi, ranking, osoby ---------------------------------------------------------

    /// Zaproszenie do znajomych po pseudonimie -> {stan: oczekuje|przyjeta, osoba}.
    pub fn invite_by_name(&mut self, name: &str) -> Result<Value, KontoError> {
        let body = json!({"name": name.trim()});
        Ok(object_or_empty(
            self.post("/api/bieznia/zaproszenie", Some(&body))?,
        ))
    }

    pub fn invite_user(&mut self, user_id: &str) -> Result<Value, KontoError> {
        let body = json!({"do": user_id});
        Ok(object_or_empty(
            self.post("/api/bieznia/zaproszenie", Some(&body))?,
        ))
    }

    /// Odpowiedź na zaproszenie (przyjmij/odrzuć); nadawca - [`Client::withdraw_invitation`].
    pub fn answer_invitation(
        &mut self,
        invitation_id: &str,
        accept: bool,
    ) -> Result<(), KontoError> {
        let action = if accept { "przyjmij" } else { "odrzuc" };
        let path = format!("/api/bieznia/zaproszenie/{}/{action}", quote(invitation_id));
        self.post(&path, None)?;
        Ok(())
    }

    pub fn withdraw_invitation(&mut self, invitation_id: &str) -> Result<(), KontoError> {
        let path = format!("/api/bieznia/zaproszenie/{}/wycofaj", quote(invitation_id));
        self.post(&path, None)?;
        Ok(())
    }

    /// {"przychodzace": [{id, osoba: {id, name}, utworzono_utc_s}], "wyslane": [...]}.
    pub fn invitations(&mut self) -> Result<Value, KontoError> {
        Ok(object_or_empty(self.get("/api/bieznia/zaproszenia", &[])?))
    }

    pub fn friends(&mut self) -> Result<Value, KontoError> {
        Ok(list_or_empty(self.get("/api/bieznia/znajomi", &[])?))
    }

    pub fn remove_friend(&mut self, friend_id: &str) -> Result<(), KontoError> {
        let path = format!("/api/bieznia/znajomi/{}", quote(friend_id));
        self.call("DELETE", &path, None, &[], true)?;
        Ok(())
    }

    /// Miejsca rankingu (lista) w zakresie z [`RANKING_SCOPES`].
    pub fn ranking(&mut self, scope: &str) -> Result<Value, KontoError> {
        check_scope(scope)?;
        let result = self.get("/api/bieznia/ranking", &[("zakres", scope)])?;
        if result.is_array() {
            return Ok(result);
        }
        Ok(list_or_empty(
            result.get("miejsca").cloned().unwrap_or(Value::Null),
        ))
    }

    /// Statystyki konta w zakresie (kroki, dystans, czas, gry).
    pub fn stats(&mut self, scope: &str) -> Result<Value, KontoError> {
        check_scope(scope)?;
        Ok(object_or_empty(
            self.get("/api/bieznia/statystyki", &[("zakres", scope)])?,
        ))
    }

    /// Osoby z kontem (tylko profile publiczne): {"strona", "nastepna", "osoby"}.
    pub fn people(&mut self, search: &str, page: u32) -> Result<Value, KontoError> {
        let page_text = page.max(1).to_string();
        let search: String = search.trim().chars().take(24).collect();
        let mut query = vec![("strona", page_text.as_str())];
        if !search.is_empty() {
            query.push(("szukaj", search.as_str()));
        }
        let result = self.get("/api/bieznia/osoby", &query)?;
        if result.is_array() {
            // serwer może zwrócić samą listę
            return Ok(json!({"osoby": result, "strona": page, "nastepna": false}));
        }
        Ok(object_or_empty(result))
    }

    /// Profil innej osoby (albo własny - tak, jak widzą go inni).
    pub fn profile(&mut self, user_id: &str) -> Result<Value, KontoError> {
        let path = format!("/api/bieznia/profil/{}", quote(user_id));
        Ok(object_or_empty(self.get(&path, &[])?))
    }

    pub fn update_profile(&mut self, patch: &ProfilePatch) -> Result<User, KontoError> {
        let mut body = Map::new();
        let mut put = |key: &str, value: &Option<String>| {
            if let Some(v) = value {
                body.insert(key.to_string(), Value::String(v.clone()));
            }
        };
        put("name", &patch.name);
        put("widocznosc_profilu", &patch.visibility);
        put("regulamin", &patch.terms);
        put("widocznosc_gier", &patch.games_visibility);
        let result = self.call(
            "PATCH",
            "/api/bieznia/profil",
            Some(&Value::Object(body)),
            &[],
            true,
        )?;
        Ok(User::from_record(&object_or_empty(result)))
    }

    /// Kod dla nowej osoby („Zaproś do WalkLevel”): jednorazowy, 14 dni.
    pub fn registration_code(&mut self) -> Result<Value, KontoError> {
        Ok(object_or_empty(
            self.post("/api/bieznia/kod-rejestracji", None)?,
        ))
    }

    /// Wszystkie dane konta z serwera (eksport „Pobierz moje dane”).
    pub fn my_data(&mut self) -> Result<Value, KontoError> {
        self.get("/api/bieznia/moje-dane", &[])
    }
}

fn check_scope(scope: &str) -> Result<(), KontoError> {
    if RANKING_SCOPES.contains(&scope) {
        Ok(())
    } else {
        Err(KontoError::invalid("Nieznany zakres rankingu."))
    }
}
