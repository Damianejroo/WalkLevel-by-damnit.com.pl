//! Konto online: logowanie, sejf na token, wysyłka i synchronizacja sesji - na wirtualnym
//! serwerze (port FakeServer z tests/test_konto.py, rozszerzony o trasę synchronizacji
//! z docs/synchro.md). Bez sieci i bez Menedżera poświadczeń.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, Once};

use serde_json::{Value, json};
use walklevel_core::account::{
    self, Account, Accounts, DELETED_ELSEWHERE, HttpReply, HttpRequest, KontoErrorKind,
    MemoryStore, SyncTarget, TokenStore, Transport, TransportError,
};
use walklevel_core::history::{self, History, Session};

const BASE: &str = "https://bieznia.example.pl";
const PASSWORD: &str = "dlugie-haslo-123";

// --- wirtualny serwer ----------------------------------------------------------------------

#[derive(Debug, Clone)]
struct User {
    id: String,
    email: String,
    name: String,
    password: String,
    verified: bool,
    terms: Value,
}

#[derive(Debug, Clone)]
struct Logged {
    method: String,
    path: String,
    body: Value,
    headers: Vec<(String, String)>,
}

#[derive(Default)]
struct State {
    users: BTreeMap<String, User>,
    tokens: HashMap<String, String>,
    /// (użytkownik, uuid) -> (sesja, numer zmiany)
    sessions: BTreeMap<(String, String), (Value, u64)>,
    /// (użytkownik, uuid) -> (usunięto, numer zmiany)
    tombstones: BTreeMap<(String, String), (f64, u64)>,
    seq: u64,
    requests: Vec<Logged>,
    too_old: bool,
    terms_current: String,
    /// `poziomy` w GET regulamin: od tej wersji serwer zapisuje pola gier ("" - jeszcze nie)
    terms_games: String,
    /// opis dla znajomych (PATCH profil `opis`)
    description: String,
    issued: u64,
    offline: bool,
    /// serwer bez trasy synchronizacji (404)
    no_sync: bool,
    /// kursory starsze niż ta zmiana -> 410 (nagrobki już skasowane)
    purged_before: u64,
    /// najwięcej zmian na stronę (prawdziwy serwer: limit 1-500)
    page_size: usize,
    /// te kursory -> 410 (jak `purged_before`, ale nowe kursory z tych samych zmian są dobre)
    expired: Vec<String>,
    /// tyle jeszcze stron synchronizacji, potem brak sieci (przerwane pobieranie)
    sync_pages_left: Option<usize>,
}

/// Zachowuje się jak PocketBase z trasami /api/bieznia/* z uzgodnionego kontraktu.
#[derive(Clone, Default)]
struct FakeServer(Arc<Mutex<State>>);

fn reply(url: &str, status: u16, body: Value) -> HttpReply {
    HttpReply {
        status,
        url: url.to_string(),
        body: if body.is_null() {
            Vec::new()
        } else {
            body.to_string().into_bytes()
        },
    }
}

fn error(url: &str, status: u16, message: &str) -> HttpReply {
    reply(url, status, json!({"message": message, "data": {}}))
}

impl FakeServer {
    fn new() -> Self {
        let server = Self::default();
        server.state().page_size = 500;
        server
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.0.lock().expect("stan serwera")
    }

    fn transport(&self) -> Arc<dyn Transport> {
        Arc::new(self.clone())
    }

    fn paths(&self) -> Vec<String> {
        self.state()
            .requests
            .iter()
            .map(|r| r.path.clone())
            .collect()
    }

    fn requests(&self) -> usize {
        self.state().requests.len()
    }

    fn session(&self, uuid: &str) -> Option<Value> {
        let st = self.state();
        st.sessions
            .iter()
            .find(|((_, u), _)| u == uuid)
            .map(|(_, (s, _))| s.clone())
    }

    fn session_count(&self) -> usize {
        self.state().sessions.len()
    }

    fn auth(st: &mut State, url: &str, uid: &str) -> HttpReply {
        st.issued += 1;
        let token = format!("tok-{uid}-{}", st.issued);
        st.tokens.insert(token.clone(), uid.to_string());
        let u = &st.users[uid];
        let record = json!({"id": u.id, "email": u.email, "name": u.name, "verified": u.verified});
        reply(url, 200, json!({"token": token, "record": record}))
    }

    fn handle(&self, request: &HttpRequest) -> HttpReply {
        let url = request.url.clone();
        let rest = url.strip_prefix(BASE).unwrap_or("");
        let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
        let query: HashMap<String, String> = reqwest::Url::parse(&format!("http://x/?{query}"))
            .map(|u| u.query_pairs().into_owned().collect())
            .unwrap_or_default();
        let body: Value = request
            .body
            .as_ref()
            .and_then(|b| serde_json::from_slice(b).ok())
            .unwrap_or(Value::Null);
        let method = request.method.as_str();
        let mut st = self.state();
        st.requests.push(Logged {
            method: method.to_string(),
            path: path.to_string(),
            body: body.clone(),
            headers: request.headers.clone(),
        });
        if st.too_old {
            return error(&url, 426, "Zaktualizuj aplikację.");
        }
        let token = request.header("Authorization").unwrap_or("").to_string();
        let uid = st
            .tokens
            .get(&token)
            .cloned()
            .filter(|u| st.users.contains_key(u));
        let b = |k: &str| {
            body.get(k)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        if path == "/api/bieznia/rejestracja" {
            if b("kod") != "KOD-START" {
                return error(&url, 400, "Kod zaproszenia jest nieważny albo już użyty.");
            }
            let id = format!("u{}", st.users.len() + 1);
            let user = User {
                id: id.clone(),
                email: b("email"),
                name: b("name"),
                password: b("password"),
                verified: false,
                terms: body.get("regulamin").cloned().unwrap_or(Value::Null),
            };
            st.users.insert(id.clone(), user);
            return Self::auth(&mut st, &url, &id);
        }
        if path == "/api/bieznia/przypomnij-haslo" {
            return reply(&url, 200, json!({"stan": "wyslano"}));
        }
        if path == "/api/bieznia/regulamin" {
            return if st.terms_current.is_empty() {
                error(&url, 404, "nie ma")
            } else {
                reply(
                    &url,
                    200,
                    json!({"wersja": st.terms_current, "poziomy": st.terms_games}),
                )
            };
        }
        if path == "/api/collections/users/auth-with-password" {
            let found = st
                .users
                .values()
                .find(|u| u.email == b("identity") && u.password == b("password"))
                .map(|u| u.id.clone());
            return match found {
                Some(id) => Self::auth(&mut st, &url, &id),
                None => error(&url, 400, "Failed to authenticate."), // jak PocketBase
            };
        }
        let Some(uid) = uid else {
            return error(&url, 401, "brak tokenu");
        };
        if path == "/api/collections/users/auth-refresh" {
            return Self::auth(&mut st, &url, &uid);
        }
        let user = st.users[&uid].clone();
        if path == "/api/bieznia/konto" && method == "DELETE" {
            if b("password") != user.password {
                return error(&url, 400, "Nieprawidłowe hasło.");
            }
            st.users.remove(&uid);
            return reply(&url, 204, Value::Null);
        }
        if path == "/api/bieznia/haslo" {
            if b("oldPassword") != user.password {
                return error(&url, 400, "Nieprawidłowe obecne hasło.");
            }
            if let Some(u) = st.users.get_mut(&uid) {
                u.password = b("password");
            }
            st.tokens.clear(); // zmiana hasła unieważnia wszystkie tokeny
            return Self::auth(&mut st, &url, &uid);
        }
        if path == "/api/bieznia/profil" && method == "GET" {
            let mut mine = json!({
                "id": user.id, "email": user.email, "name": user.name, "verified": user.verified,
                "widocznosc_profilu": "publiczny", "widocznosc_gier": "znajomi",
                "opis": if st.description.is_empty() { Value::Null } else { json!(st.description) },
            });
            if !st.terms_current.is_empty() {
                mine["regulamin"] = user.terms.clone();
                mine["regulamin_aktualny"] = json!(st.terms_current);
            }
            return reply(&url, 200, mine);
        }
        if path == "/api/bieznia/profil" && method == "PATCH" {
            if let Some(opis) = body.get("opis").and_then(Value::as_str) {
                if opis.chars().count() > 40 {
                    return error(&url, 400, "Opis może mieć najwyżej 40 znaków.");
                }
                st.description = opis.to_string();
            }
            if let Some(terms) = body.get("regulamin") {
                if terms.as_str() != Some(st.terms_current.as_str()) {
                    return error(&url, 400, "Nieaktualna wersja regulaminu.");
                }
                if let Some(u) = st.users.get_mut(&uid) {
                    u.terms = terms.clone();
                }
            }
            if !b("name").is_empty()
                && let Some(u) = st.users.get_mut(&uid)
            {
                u.name = b("name");
            }
            let u = &st.users[&uid];
            return reply(
                &url,
                200,
                json!({"id": u.id, "email": u.email, "name": u.name, "verified": u.verified}),
            );
        }
        if path == "/api/bieznia/kod-rejestracji" {
            return reply(&url, 200, json!({"kod": "NOWY-1234", "wygasa_utc_s": 0}));
        }
        if let Some(other) = path.strip_prefix("/api/bieznia/profil/") {
            return reply(
                &url,
                200,
                json!({"id": other, "name": "Kot_Bury", "znajomy": true,
                       "bieznia_model": "Urevo SpaceWalk 5L",
                       "tydzien": {"kroki": 15200, "dystans_km": 10.1, "ruch_s": 9000, "sesje": 6}}),
            );
        }
        if path == "/api/bieznia/sesje" && method == "POST" {
            return Self::upload(&mut st, &url, &uid, &body);
        }
        if path == "/api/bieznia/sesje" && method == "GET" {
            return Self::changes(&mut st, &url, &uid, &query);
        }
        if let Some(uuid) = path.strip_prefix("/api/bieznia/sesje/")
            && method == "DELETE"
        {
            let key = (uid.clone(), uuid.to_string());
            if st.sessions.remove(&key).is_none() {
                return error(&url, 404, "Nie ma takiej sesji.");
            }
            st.seq += 1;
            let seq = st.seq;
            st.tombstones.insert(key, (1.7e9, seq));
            return reply(&url, 204, Value::Null);
        }
        if path == "/api/bieznia/wyslij-potwierdzenie" {
            let stan = if user.verified {
                "potwierdzony"
            } else {
                "wyslano"
            };
            return reply(&url, 200, json!({ "stan": stan }));
        }
        if path == "/api/bieznia/stan" {
            return reply(&url, 204, Value::Null);
        }
        if path == "/api/bieznia/znajomi" {
            return reply(
                &url,
                200,
                json!([
                    {"id": "u9", "name": "Kot_Bury", "na_zywo": {"kmh": 3.2, "kroki": 840},
                     "tydzien": {"kroki": 15200, "ruch_s": 9000}},
                    {"id": "u8", "name": "Szybka-Ania", "na_zywo": null,
                     "tydzien": {"kroki": 8100, "ruch_s": 4200}},
                ]),
            );
        }
        if path == "/api/bieznia/zaproszenia" {
            return reply(
                &url,
                200,
                json!({"przychodzace": [{"id": "z1", "osoba": {"id": "u5", "name": "Nowy_Gracz"}}],
                       "wyslane": [{"id": "z2", "osoba": {"id": "u8", "name": "Szybka-Ania"}}]}),
            );
        }
        if path == "/api/bieznia/statystyki" {
            return reply(
                &url,
                200,
                json!({"zakres": query.get("zakres"), "kroki": 12345, "dystans_km": 8.4,
                       "ruch_s": 7200, "gry": []}),
            );
        }
        if path == "/api/bieznia/ranking" {
            let places = json!([
                {"miejsce": 1, "name": "Kot_Bury", "kroki": 15200, "dystans_km": 10.1},
                {"miejsce": 2, "name": user.name, "kroki": 12345, "dystans_km": 8.4, "ja": true},
            ]);
            return reply(
                &url,
                200,
                json!({"zakres": query.get("zakres"), "miejsca": places}),
            );
        }
        if path == "/api/bieznia/osoby" {
            // stara wersja serwera zwraca samą listę
            return reply(
                &url,
                200,
                json!([{"id": "u7", "name": "Maraton_Max", "miejsce": 1}]),
            );
        }
        if path == "/api/bieznia/zaproszenie" && method == "POST" {
            if b("do").is_empty() && b("name").is_empty() {
                return error(&url, 400, "Podaj pseudonim osoby, którą chcesz zaprosić.");
            }
            if b("name").to_lowercase() == "nikt" {
                return error(&url, 404, "Nie ma takiej osoby.");
            }
            let who = if b("name").is_empty() {
                "Maraton_Max".to_string()
            } else {
                b("name")
            };
            return reply(
                &url,
                200,
                json!({"stan": "oczekuje", "osoba": {"id": "u7", "name": who}}),
            );
        }
        if path.starts_with("/api/bieznia/zaproszenie/")
            || path.starts_with("/api/bieznia/znajomi/")
        {
            return reply(
                &url,
                200,
                json!({"znajomy": {"id": "u9", "name": "Kot_Bury"}}),
            );
        }
        if path == "/api/bieznia/moje-dane" {
            return reply(
                &url,
                200,
                json!({"konto": {"name": user.name}, "sesje": []}),
            );
        }
        error(&url, 404, "nie ma")
    }

    fn upload(st: &mut State, url: &str, uid: &str, body: &Value) -> HttpReply {
        let sessions = body["sesje"].as_array().cloned().unwrap_or_default();
        assert!(sessions.len() <= 100, "paczka większa niż 100 sesji");
        let mut rejected = Vec::new();
        let mut skipped = Vec::new();
        let mut saved = 0;
        for s in sessions {
            let uuid = s["uuid"].as_str().unwrap_or("").to_string();
            let key = (uid.to_string(), uuid.clone());
            if uuid.len() != 36 || uuid != uuid.to_lowercase() {
                rejected.push(json!({"uuid": uuid, "powod": "zły format uuid"}));
            } else if s["kroki"].as_i64().unwrap_or(0) > 300_000 {
                rejected.push(json!({"uuid": uuid, "powod": "kroki poza zakresem"}));
            } else if st.tombstones.contains_key(&key) {
                rejected.push(json!({"uuid": uuid, "powod": DELETED_ELSEWHERE}));
            } else {
                let at = |v: &Value| v["zmieniono_utc_s"].as_f64().unwrap_or(0.0);
                if st
                    .sessions
                    .get(&key)
                    .is_some_and(|(old, _)| at(old) >= at(&s))
                {
                    // serwer nadpisuje tylko NOWSZĄ wersją (kontrakt HUB)
                    skipped.push(json!(uuid));
                    continue;
                }
                st.seq += 1;
                let seq = st.seq;
                let mut stored = s.clone();
                stored["osoba"] = json!(uid);
                st.sessions.insert(key, (stored, seq));
                saved += 1;
            }
        }
        reply(
            url,
            200,
            json!({"zapisane": saved, "pominiete": skipped, "odrzucone": rejected}),
        )
    }

    fn changes(st: &mut State, url: &str, uid: &str, query: &HashMap<String, String>) -> HttpReply {
        if st.no_sync {
            return error(url, 404, "nie ma");
        }
        let given = query.get("kursor").cloned().unwrap_or_default();
        let after = if given.is_empty() {
            0
        } else {
            match given.strip_prefix('c').and_then(|n| n.parse::<u64>().ok()) {
                Some(n) => n,
                None => return error(url, 400, "Nieprawidłowy kursor."),
            }
        };
        if !given.is_empty() && (after < st.purged_before || st.expired.contains(&given)) {
            return reply(
                url,
                410,
                json!({"message": "Kursor jest za stary - pobierz wszystko.", "data": {"pelna": true}}),
            );
        }
        let limit: usize = query
            .get("limit")
            .and_then(|l| l.parse().ok())
            .unwrap_or(500usize)
            .min(st.page_size);
        let mut items: Vec<(u64, Value, bool)> = Vec::new();
        for ((owner, _), (s, seq)) in &st.sessions {
            if owner == uid && *seq > after {
                items.push((*seq, s.clone(), false));
            }
        }
        for ((owner, uuid), (at, seq)) in &st.tombstones {
            if owner == uid && *seq > after {
                items.push((*seq, json!({"uuid": uuid, "usunieto_utc_s": at}), true));
            }
        }
        items.sort_by_key(|(seq, _, _)| *seq);
        let more = items.len() > limit;
        items.truncate(limit);
        let cursor = items
            .last()
            .map(|(seq, _, _)| format!("c{seq}"))
            .unwrap_or(given);
        let sesje: Vec<Value> = items.iter().filter(|i| !i.2).map(|i| i.1.clone()).collect();
        let usuniete: Vec<Value> = items.iter().filter(|i| i.2).map(|i| i.1.clone()).collect();
        reply(
            url,
            200,
            json!({"sesje": sesje, "usuniete": usuniete, "kursor": cursor, "wiecej": more}),
        )
    }
}

impl Transport for FakeServer {
    fn send(&self, request: &HttpRequest) -> Result<HttpReply, TransportError> {
        {
            let mut st = self.state();
            if st.offline {
                return Err(TransportError("brak sieci".into()));
            }
            let sync = request.method == "GET" && request.url.contains("/api/bieznia/sesje");
            if sync && let Some(left) = st.sync_pages_left.as_mut() {
                if *left == 0 {
                    return Err(TransportError("brak sieci".into()));
                }
                *left -= 1;
            }
        }
        Ok(self.handle(request))
    }
}

// --- pomocnicze ----------------------------------------------------------------------------

fn standalone(server: &FakeServer, dir: &Path) -> (Account, Arc<MemoryStore>) {
    let store = Arc::new(MemoryStore::new());
    let account = Account::open(
        BASE,
        store.clone(),
        &dir.join("konto.json"),
        server.transport(),
    )
    .expect("konto");
    (account, store)
}

fn signed_in(account: &mut Account, person: &str) {
    account
        .register("KOD-START", "ola@example.pl", PASSWORD, "Ola_77", person)
        .expect("rejestracja");
}

/// Krótka nazwa sesji w teście -> 32 znaki hex jak prawdziwe id.
fn hexid(sid: &str) -> String {
    if sid.len() == 32 {
        return sid.to_string();
    }
    let mut h = hex::encode(sid);
    h.push_str(&"0".repeat(32));
    h.truncate(32);
    h
}

fn uuid_of(sid: &str) -> String {
    account::canonical_uuid(&hexid(sid))
}

fn session(sid: &str, person: &str) -> Session {
    let mut s = history::new_session(
        &hexid(sid),
        person,
        "Ola",
        "g1",
        "Cyberpunk 2077",
        100.0,
        700.0,
    );
    s.steps = 900;
    s.distance_km = 0.6;
    s.active_s = 600.0;
    s
}

fn save(h: &History, mut s: Session) -> Session {
    h.save(&mut s).expect("zapis sesji");
    s
}

fn open_history(dir: &Path) -> History {
    History::open(&dir.join("h.db")).expect("historia")
}

fn now() -> f64 {
    history::now_unix()
}

type Vault = Arc<Mutex<HashMap<String, Arc<MemoryStore>>>>;

fn stores() -> (Vault, account::StoreFor) {
    let made: Vault = Arc::default();
    let inner = made.clone();
    let store_for: account::StoreFor = Box::new(move |person: &str| {
        let mut m = inner.lock().expect("sejf");
        let s = m
            .entry(person.to_string())
            .or_insert_with(|| Arc::new(MemoryStore::new()))
            .clone();
        s as Arc<dyn TokenStore>
    });
    (made, store_for)
}

fn vault_get(vault: &Vault, person: &str) -> String {
    vault
        .lock()
        .expect("sejf")
        .get(person)
        .map(|s| s.get())
        .unwrap_or_default()
}

fn accounts(server: &FakeServer, path: &Path) -> (Accounts, Vault) {
    let (vault, store_for) = stores();
    let accounts = Accounts::with_parts(BASE, path, server.transport(), store_for, None);
    (accounts, vault)
}

// --- logowanie, rejestracja, sejf --------------------------------------------------------------

#[test]
fn register_login_and_token_only_in_vault() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let (mut account, store) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    assert!(account.state.logged_in() && account.state.name == "Ola_77");
    assert_eq!(
        server.state().users["u1"].terms,
        json!(account::TERMS_VERSION)
    );
    let saved = std::fs::read_to_string(dir.path().join("konto.json")).expect("plik");
    assert!(!saved.contains("tok-") && !saved.contains("dlugie-haslo")); // ani token, ani hasło
    assert!(store.get().starts_with("tok-"));
    let last = server.state().requests.last().cloned().expect("żądanie");
    let version = last
        .headers
        .iter()
        .find(|(n, _)| n == "X-Bieznia-Wersja")
        .map(|(_, v)| v.clone());
    assert_eq!(version.as_deref(), Some(walklevel_core::version())); // wersja w każdym żądaniu
}

#[test]
fn register_uses_terms_version_from_server() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    server.state().terms_current = "2026-11-26".into();
    server.state().terms_games = "2026-11-26".into();
    let (mut account, _) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    assert_eq!(account.state.terms, "2026-11-26");
    assert_eq!(server.state().users["u1"].terms, json!("2026-11-26"));
}

#[test]
fn bad_input_never_reaches_the_server() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let (mut account, _) = standalone(&server, dir.path());
    let e = account
        .register("KOD-START", "ola@example.pl", PASSWORD, "x", "p1")
        .expect_err("zły pseudonim");
    assert!(e.message.contains("Pseudonim") && e.kind == KontoErrorKind::Invalid);
    assert!(e.fields.contains_key("name"));
    let e = account
        .register("", "ola@example.pl", PASSWORD, "Ola_77", "p1")
        .expect_err("bez kodu");
    assert!(e.message.contains("kod") && e.fields.contains_key("code"));
    assert_eq!(server.requests(), 0);
}

#[test]
fn expired_token_logs_out_without_retry_loop() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let (mut account, store) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    server.state().tokens.clear(); // serwer unieważnił tokeny
    let before = server.requests();
    let e = account.refresh().expect_err("wygasło");
    assert!(e.is_logged_out());
    assert_eq!(server.requests(), before + 1);
    assert!(!account.state.logged_in() && store.get().is_empty());
    assert_eq!(account.state.person, "p1"); // osoba zostaje
}

#[test]
fn any_route_with_401_logs_the_account_out() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let (mut account, _) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    server.state().tokens.clear();
    let h = open_history(dir.path());
    save(&h, session("a", "p1"));
    let e = account.upload(&h, None, now()).expect_err("401");
    assert!(e.is_logged_out() && !account.state.logged_in());
    // bez tokenu nie pytamy serwera wcale
    let before = server.requests();
    assert_eq!(account.upload(&h, None, now()).ok(), Some(0));
    assert!(account.client_mut().friends().is_err());
    assert_eq!(server.requests(), before);
}

#[test]
fn delete_account_needs_password() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let (mut account, store) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    assert!(account.delete_account("").is_err());
    assert!(account.delete_account("zle-haslo-000").is_err());
    assert!(!server.state().users.is_empty() && account.state.logged_in());
    account.delete_account(PASSWORD).expect("usunięcie");
    assert!(server.state().users.is_empty() && !account.state.logged_in());
    assert!(store.get().is_empty());
}

#[test]
fn server_can_demand_newer_app() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let (mut account, _) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    server.state().too_old = true;
    let e = account.client_mut().friends().expect_err("426");
    assert!(e.is_update_required() && e.message.contains("Zaktualizuj"));
    assert!(account.state.logged_in()); // 426 nie wylogowuje
}

struct Elsewhere;

impl Transport for Elsewhere {
    fn send(&self, _r: &HttpRequest) -> Result<HttpReply, TransportError> {
        Ok(HttpReply {
            status: 200,
            url: "https://zly.example/api".into(),
            body: br#"{"token": "x"}"#.to_vec(),
        })
    }
}

struct Offline;

impl Transport for Offline {
    fn send(&self, _r: &HttpRequest) -> Result<HttpReply, TransportError> {
        Err(TransportError("brak sieci".into()))
    }
}

#[test]
fn only_https_and_no_redirect_to_other_hosts() {
    assert!(account::check_base_url("http://bieznia.example.pl").is_err());
    assert!(account::check_base_url("https://user:pass@bieznia.example.pl").is_err());
    assert_eq!(
        account::check_base_url("http://127.0.0.1:8090/")
            .ok()
            .as_deref(),
        Some("http://127.0.0.1:8090")
    );
    let mut client = account::Client::new(BASE, "", Arc::new(Elsewhere)).expect("klient");
    let e = client
        .login("a@b.pl", &"x".repeat(12))
        .expect_err("inny host");
    assert!(e.message.contains("bezpieczeństwa"));
    assert!(client.token().is_empty());
}

#[test]
fn no_connection_gives_friendly_message() {
    let mut client = account::Client::new(BASE, "", Arc::new(Offline)).expect("klient");
    let e = client
        .login("a@b.pl", &"x".repeat(12))
        .expect_err("offline");
    assert!(e.message.contains("internet") && e.kind == KontoErrorKind::Offline);
}

struct Capture(Mutex<Vec<String>>);

impl log::Log for Capture {
    fn enabled(&self, _m: &log::Metadata) -> bool {
        true
    }
    fn log(&self, record: &log::Record) {
        if let Ok(mut lines) = self.0.lock() {
            lines.push(record.args().to_string());
        }
    }
    fn flush(&self) {}
}

static LOGGER: Capture = Capture(Mutex::new(Vec::new()));
static LOG_INIT: Once = Once::new();

#[test]
fn request_bodies_and_tokens_are_not_logged() {
    LOG_INIT.call_once(|| {
        let _ = log::set_logger(&LOGGER);
        log::set_max_level(log::LevelFilter::Debug);
    });
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let (mut account, _) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    account.refresh().expect("odświeżenie");
    let text = LOGGER.0.lock().expect("logi").join("\n");
    assert!(!text.contains("dlugie-haslo") && !text.contains("tok-"));
    assert!(text.contains("rejestracja -> 200")); // tylko ścieżka i kod
    assert!(!format!("{:?}", account.client()).contains("tok-"));
}

#[test]
fn builtin_pocketbase_errors_are_polish_and_wrong_code_is_refused() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let (mut account, _) = standalone(&server, dir.path());
    let e = account
        .register("ZLY-KOD", "ola@example.pl", PASSWORD, "Ola_77", "p1")
        .expect_err("zły kod");
    assert!(e.message.contains("Kod zaproszenia") && !account.state.logged_in());
    signed_in(&mut account, "p1");
    let e = account
        .login("ola@example.pl", "zle-haslo-000", "p1")
        .expect_err("złe hasło");
    assert_eq!(e.message, "Nieprawidłowy e-mail lub hasło.");
    assert!(account.state.logged_in()); // nieudane logowanie nie wylogowuje
}

#[test]
fn change_password_keeps_session_with_new_token() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let (mut account, store) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    let old = store.get();
    account
        .change_password(PASSWORD, "nowe-dlugie-haslo-9")
        .expect("zmiana hasła");
    assert!(store.get() != old && account.client().token() == store.get());
    assert!(account.client_mut().friends().is_ok()); // nowy token działa
    let e = account
        .change_password("nowe-dlugie-haslo-9", "krotkie")
        .expect_err("za krótkie");
    assert!(e.message.contains("Hasło"));
}

#[test]
fn refresh_takes_profile_settings_from_server_and_verification() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let (mut account, _) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    account.refresh().expect("odświeżenie");
    assert_eq!(account.state.profile_visibility, "publiczny");
    assert_eq!(account.send_verification().ok().as_deref(), Some("wyslano"));
    assert_eq!(account.check_verified().ok(), Some(false));
    server.state().users.get_mut("u1").expect("u1").verified = true; // kliknięty link
    assert_eq!(account.check_verified().ok(), Some(true));
    assert_eq!(
        account.send_verification().ok().as_deref(),
        Some("potwierdzony")
    );
    account
        .remind_password("ktos@example.pl")
        .expect("przypomnienie");
    assert!(
        server
            .paths()
            .contains(&"/api/bieznia/przypomnij-haslo".to_string())
    );
}

#[test]
fn profile_settings_and_rename_go_to_server() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let (mut account, _) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    assert!(account.rename("x").is_err());
    account.rename("Ola Nowa").expect("pseudonim");
    assert_eq!(server.state().users["u1"].name, "Ola Nowa");
    assert_eq!(account.state.name, "Ola Nowa");
    account
        .set_profile_visibility("ukryty")
        .expect("widoczność");
    account.set_games_visibility("ja").expect("gry");
    assert!(account.set_games_visibility("nikt").is_err());
    let last = server.state().requests.last().cloned().expect("żądanie");
    assert_eq!(last.body, json!({"widocznosc_gier": "ja"}));
    let reread = account::load_state(&dir.path().join("konto.json"));
    assert_eq!(reread.profile_visibility, "ukryty");
    assert_eq!(reread.games_visibility, "ja");
}

#[test]
fn online_routes_return_server_data() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let (mut account, _) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    let c = account.client_mut();
    assert_eq!(c.friends().expect("znajomi")[0]["name"], "Kot_Bury");
    assert_eq!(
        c.invitations().expect("zaproszenia")["przychodzace"][0]["id"],
        "z1"
    );
    for scope in account::RANKING_SCOPES {
        assert_eq!(c.ranking(scope).expect("ranking")[1]["ja"], true);
        assert_eq!(c.stats(scope).expect("statystyki")["zakres"], scope);
    }
    assert!(c.ranking("rok").is_err());
    let people = c.people(" Mara ", 0).expect("osoby");
    assert_eq!(people["osoby"][0]["name"], "Maraton_Max");
    assert_eq!(people["strona"], 0); // jak w 0.5.x: strona z wywołania
    assert_eq!(
        c.profile("u9").expect("profil")["bieznia_model"],
        "Urevo SpaceWalk 5L"
    );
    assert_eq!(c.registration_code().expect("kod")["kod"], "NOWY-1234");
    assert_eq!(
        c.invite_by_name(" Kot Bury ").expect("zaproś")["osoba"]["name"],
        "Kot Bury"
    );
    assert!(c.invite_by_name("nikt").is_err());
    assert_eq!(c.invite_user("u7").expect("zaproś")["stan"], "oczekuje");
    c.answer_invitation("z1", true).expect("przyjmij");
    c.answer_invitation("z/1", false).expect("odrzuć");
    c.withdraw_invitation("z2").expect("wycofaj");
    c.remove_friend("u9").expect("usuń");
    let paths = server.paths();
    for p in [
        "/api/bieznia/zaproszenie/z1/przyjmij",
        "/api/bieznia/zaproszenie/z%2F1/odrzuc",
        "/api/bieznia/zaproszenie/z2/wycofaj",
        "/api/bieznia/znajomi/u9",
    ] {
        assert!(paths.contains(&p.to_string()), "{p}");
    }
    let sent = server
        .state()
        .requests
        .iter()
        .rev()
        .find(|r| r.path == "/api/bieznia/osoby")
        .cloned();
    assert!(sent.is_some());
    let file = account
        .export_my_data(dir.path(), "20261126-120000")
        .expect("eksport");
    assert!(file.ends_with("dane/moje-dane-konto-20261126-120000.json"));
    let text = std::fs::read_to_string(file).expect("plik");
    assert!(text.starts_with("{\n \"konto\""));
}

fn walk() -> account::LiveWalk {
    account::LiveWalk {
        kmh: 3.24,
        steps: 840,
        active_s: 600.7,
        session_id: "abc".into(),
        distance_km: 1.5249,
        mode: "gra".into(),
        game: Some(account::LiveGame {
            id: "0a1b2c3d4e".into(),
            name: "  World   of Warcraft ".into(),
            public: true,
        }),
    }
}

#[test]
fn live_state_only_when_not_invisible() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let (mut account, _) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    assert_eq!(account.state.live_status, "niewidoczny"); // domyślnie wyłączony
    let before = server.requests();
    account.live(&walk()).expect("na żywo");
    assert_eq!(server.requests(), before); // niewidoczny - nic nie idzie
    account.set_live_status("wszystko").expect("włącz");
    assert!(account.state.live);
    account.live(&walk()).expect("na żywo");
    let last = server.state().requests.last().cloned().expect("żądanie");
    assert_eq!(last.method, "PUT");
    // bez regulaminu z grami - tylko pola sprzed 0.6.2
    assert_eq!(
        last.body,
        json!({"kmh": 3.2, "kroki": 840, "ruch_s": 600, "sesja_uuid": "abc"})
    );
    assert!(account.set_live_status("zle").is_err());
    account.set_live_status("niewidoczny").expect("wyłącz");
    assert!(!account.state.live);
    let last = server.state().requests.last().cloned().expect("żądanie");
    assert_eq!(
        (last.method.as_str(), last.path.as_str()),
        ("DELETE", "/api/bieznia/stan")
    );
}

#[test]
fn live_body_has_distance_mode_and_game_only_with_consent_and_status() {
    let w = walk();
    let full = account::api::live_body(&w, true, true);
    assert_eq!(
        full,
        json!({"kmh": 3.2, "kroki": 840, "ruch_s": 600, "sesja_uuid": "abc",
               "dystans_km": 1.525, "tryb": "gra", "gra_id": "0a1b2c3d4e",
               "gra": "World of Warcraft", "gra_publiczna": true})
    );
    // „Pokazuj tylko, że chodzę”: tryb i dystans tak, gra nie
    let only = account::api::live_body(&w, true, false);
    assert_eq!(only["tryb"], "gra");
    assert!(only.get("gra_id").is_none() && only.get("gra").is_none());
    assert!(only.get("gra_publiczna").is_none());
    // bez zgody na regulamin z grami - nic nowego
    let old = account::api::live_body(&w, false, true);
    assert!(old.get("dystans_km").is_none() && old.get("tryb").is_none());
    // trening bez gry, dystans w zakresie 0..300, zły id gry pomijany
    let mut t = walk();
    t.mode = "trening".into();
    t.distance_km = 999.0;
    let tr = account::api::live_body(&t, true, true);
    assert_eq!(
        (tr["tryb"].as_str(), tr["dystans_km"].as_f64()),
        (Some("trening"), Some(300.0))
    );
    assert!(tr.get("gra_id").is_none());
    let mut bad = walk();
    bad.game = Some(account::LiveGame {
        id: "XYZ".into(),
        ..account::LiveGame::default()
    });
    assert!(
        account::api::live_body(&bad, true, true)
            .get("gra_id")
            .is_none()
    );
}

#[test]
fn live_status_migrates_from_live_switch_and_description_round_trips() {
    let st = account::AccountState::from_value(&json!({"user_id": "u1", "live": true}));
    assert_eq!((st.live_status.as_str(), st.live), ("wszystko", true));
    let st = account::AccountState::from_value(&json!({"user_id": "u1", "live": false}));
    assert_eq!((st.live_status.as_str(), st.live), ("niewidoczny", false));
    let st = account::AccountState::from_value(&json!({"live": true, "live_status": "chodze"}));
    assert_eq!((st.live_status.as_str(), st.live), ("chodze", true));
    let st =
        account::AccountState::from_value(&json!({"live": true, "live_status": "niewidoczny"}));
    assert!(!st.live); // status wygrywa, `live` dla 0.5.x idzie za nim
    let mut st = account::AccountState::default();
    st.set_live_status("chodze");
    st.description = "Do Mordoru".into();
    let v = serde_json::to_value(&st).expect("json");
    assert_eq!(
        (v["live_status"].as_str(), v["live"].as_bool()),
        (Some("chodze"), Some(true))
    );
    assert_eq!(account::AccountState::from_value(&v), st);
    let off = serde_json::to_value(account::AccountState::default()).expect("json");
    assert!(off.get("live_status").is_none()); // domyślny plik jak w 0.5.x
}

#[test]
fn description_is_patched_checked_and_read_back() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let (mut account, _) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    account
        .set_description("  Cel:   5 km dziennie ")
        .expect("opis");
    assert_eq!(account.state.description, "Cel: 5 km dziennie");
    assert_eq!(server.state().description, "Cel: 5 km dziennie");
    let long = "x".repeat(41);
    let before = server.requests();
    let e = account.set_description(&long).expect_err("za długi");
    assert_eq!(e.message, "Opis może mieć najwyżej 40 znaków.");
    assert_eq!(server.requests(), before); // nie poszło na serwer
    account.set_description("").expect("czyści");
    assert_eq!(server.state().description, "");
    server.state().description = "Z innego komputera".into();
    account.refresh().expect("odświeżenie");
    assert_eq!(account.state.description, "Z innego komputera");
}

// --- wysyłka -------------------------------------------------------------------------------

#[test]
fn upload_is_idempotent_and_sends_only_linked_person() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let h = open_history(dir.path());
    save(&h, session("a", "p1"));
    save(&h, session("b", "p1"));
    save(&h, session("c", "p2"));
    let mut sim = session("d", "p1");
    sim.source = "symulator".into();
    save(&h, sim);
    let (mut account, _) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    assert_eq!(account.pending_uploads(&h, now()).ok(), Some(2));
    assert_eq!(account.upload(&h, None, now()).ok(), Some(2));
    assert_eq!(server.session_count(), 2); // bez p2 i symulatora
    let sent = server.session(&uuid_of("a")).expect("sesja a");
    let text = sent.to_string();
    assert!(!text.contains("profil_nazwa") && !text.contains("Cyberpunk") && !text.contains("Ola"));
    assert_eq!(sent["widocznosc"], "znajomi");
    assert_eq!(account.upload(&h, None, now()).ok(), Some(0)); // nic nowego
    let mut changed = h.session(&hexid("a")).expect("baza").expect("sesja");
    changed.steps += 10;
    save(&h, changed); // sesja trwa dalej - zmieniona idzie jeszcze raz (upsert)
    assert_eq!(account.upload(&h, None, now()).ok(), Some(1));
}

#[test]
fn hidden_session_goes_private_and_visibility_change_resends() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let h = open_history(dir.path());
    save(&h, session("a", "p1"));
    save(&h, session("b", "p1"));
    let (mut account, _) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    account.upload(&h, None, now()).expect("wysyłka");
    h.set_hidden_online(&hexid("a"), true).expect("ukryj");
    assert_eq!(account.upload(&h, None, now()).ok(), Some(1));
    assert_eq!(
        server.session(&uuid_of("a")).expect("a")["widocznosc"],
        "prywatna"
    );
    let before = server.session(&uuid_of("b")).expect("b")["zmieniono_utc_s"]
        .as_f64()
        .expect("czas");
    let at = now() + 10.0;
    account
        .set_visibility(&h, "publiczna", at)
        .expect("widoczność");
    assert!(account.set_visibility(&h, "wszyscy", at).is_err());
    assert_eq!(account.upload(&h, None, now()).ok(), Some(2));
    // serwer nadpisuje tylko nowszą wersją - zmiana widoczności idzie z nowszym czasem
    let b = server.session(&uuid_of("b")).expect("b");
    assert!(b["zmieniono_utc_s"].as_f64().expect("czas") > before);
    assert_eq!(
        server.session(&uuid_of("b")).expect("b")["widocznosc"],
        "publiczna"
    );
    assert_eq!(
        server.session(&uuid_of("a")).expect("a")["widocznosc"],
        "prywatna"
    );
}

#[test]
fn no_upload_without_login_or_when_turned_off() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let h = open_history(dir.path());
    save(&h, session("a", "p1"));
    let (mut account, _) = standalone(&server, dir.path());
    assert_eq!(account.upload(&h, None, now()).ok(), Some(0));
    assert_eq!(server.requests(), 0);
    signed_in(&mut account, "p1");
    account.set_upload(false).expect("wyłącz");
    assert_eq!(account.upload(&h, None, now()).ok(), Some(0));
}

#[test]
fn uuid_goes_lowercase_with_dashes_and_rejected_do_not_loop() {
    assert_eq!(
        account::canonical_uuid("0192ABCD0000700080000000000000AA"),
        "0192abcd-0000-7000-8000-0000000000aa"
    );
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let h = open_history(dir.path());
    save(&h, session(&format!("{:032x}", 1), "p1"));
    let mut bad = session(&format!("{:032x}", 2), "p1");
    bad.steps = 999_999;
    save(&h, bad);
    let (mut account, _) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    assert_eq!(account.upload(&h, None, now()).ok(), Some(1)); // zła sesja nie wywraca paczki
    assert_eq!(account.upload(&h, None, now()).ok(), Some(0)); // i nie wraca w kółko
}

#[test]
fn upload_goes_in_batches_of_100() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let h = open_history(dir.path());
    for i in 0..250 {
        save(&h, session(&format!("{:032x}", i + 1), "p1"));
    }
    let (mut account, _) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    assert_eq!(account.upload(&h, None, now()).ok(), Some(250));
    let posts = server
        .state()
        .requests
        .iter()
        .filter(|r| r.method == "POST" && r.path == "/api/bieznia/sesje")
        .count();
    assert_eq!(posts, 3);
}

#[test]
fn rejected_session_is_retried_after_a_day() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let h = open_history(dir.path());
    let mut bad = session(&format!("{:032x}", 2), "p1");
    bad.steps = 999_999;
    let bad = save(&h, bad);
    let (mut account, _) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    let t = now();
    assert_eq!(account.upload(&h, None, t).ok(), Some(0));
    assert!(h.unsent("p1", t).expect("kolejka").is_empty()); // nie zaraz
    assert!(h.rejected().expect("odrzucone")[&bad.id].contains("zakresem"));
    let later = t + account::REJECTED_RETRY_S + 60.0;
    let ids: Vec<String> = h
        .unsent("p1", later)
        .expect("kolejka")
        .into_iter()
        .map(|s| s.id)
        .collect();
    assert_eq!(ids, vec![bad.id.clone()]); // po dobie
    let mut fixed = bad.clone();
    fixed.steps = 900;
    save(&h, fixed); // albo od razu, gdy sesja się zmieni
    assert_eq!(h.unsent("p1", now()).expect("kolejka").len(), 1);
    assert_eq!(account.upload(&h, None, now()).ok(), Some(1));
    assert!(h.rejected().expect("odrzucone").is_empty());
}

#[test]
fn walking_offline_uploads_everything_when_internet_returns() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let h = open_history(dir.path());
    let (mut account, _) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    server.state().offline = true;
    save(&h, session("a", "p1"));
    let b = save(&h, session("b", "p1"));
    let e = account.upload(&h, None, now()).expect_err("offline");
    assert_eq!(e.kind, KontoErrorKind::Offline);
    assert!(account.state.logged_in()); // brak sieci nie wylogowuje
    assert_eq!(h.unsent("p1", now()).expect("kolejka").len(), 2);
    let mut walking = b.clone(); // sesja trwała dalej, nadal bez sieci
    walking.steps = 1500;
    save(&h, walking);
    assert!(account.upload(&h, None, now()).is_err());
    server.state().offline = false;
    assert_eq!(account.upload(&h, None, now()).ok(), Some(2));
    assert_eq!(server.session(&uuid_of("b")).expect("b")["kroki"], 1500); // najnowszy stan
    assert_eq!(account.upload(&h, None, now()).ok(), Some(0));
}

#[test]
fn game_fields_go_only_after_accepting_current_terms_name_always() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let h = open_history(dir.path());
    let mut s1 = session("a", "p1");
    s1.profile = "aaaaaaaaaa".into();
    s1.profile_name = "  Cyberpunk   2077 ".into();
    save(&h, s1);
    let mut s2 = session("b", "p1");
    s2.profile = "bbbbbbbbbb".into();
    s2.profile_name = "Tajna gra".into();
    s2.mode = "trening".into();
    save(&h, s2);
    let (mut account, _) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    let shared = |pid: &str| pid == "aaaaaaaaaa";
    account.upload(&h, Some(&shared), now()).expect("wysyłka");
    let a = server.session(&uuid_of("a")).expect("a");
    for key in ["gra", "gra_id", "tryb", "gra_publiczna"] {
        assert!(a.get(key).is_none(), "{key} przed akceptacją regulaminu");
    }
    server.state().terms_current = "2026-11-26".into();
    server.state().terms_games = "2026-11-26".into();
    account.refresh().expect("odświeżenie");
    assert!(account.state.terms_outdated() && !account.state.games_allowed());
    // bez akceptacji nowej wersji dalej nic o grach
    let mut s3 = session("c", "p1");
    s3.profile = "aaaaaaaaaa".into();
    save(&h, s3);
    account.upload(&h, Some(&shared), now()).expect("wysyłka");
    assert!(
        server
            .session(&uuid_of("c"))
            .expect("c")
            .get("gra")
            .is_none()
    );
    account.accept_terms().expect("akceptacja");
    assert!(account.state.games_allowed());
    account
        .upload(&h, Some(&shared), now() + 10.0)
        .expect("wysyłka");
    let a = server.session(&uuid_of("a")).expect("a");
    assert_eq!(
        (a["gra"].clone(), a["gra_id"].clone(), a["tryb"].clone()),
        (json!("Cyberpunk 2077"), json!("aaaaaaaaaa"), json!("gra"))
    );
    assert_eq!(a["gra_publiczna"], true);
    let b = server.session(&uuid_of("b")).expect("b");
    // nazwa gry zawsze (widzi ją tylko właściciel), przełącznik profilu = gra_publiczna
    assert_eq!(
        (b["gra"].clone(), b["gra_id"].clone(), b["tryb"].clone()),
        (json!("Tajna gra"), json!("bbbbbbbbbb"), json!("trening"))
    );
    assert_eq!(b["gra_publiczna"], false);
}

#[test]
fn accepting_new_terms_resends_whole_history_once_newer_in_batches() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let h = open_history(dir.path());
    for i in 0..150 {
        let mut s = session(&format!("{:032x}", i + 1), "p1");
        s.profile = "aaaaaaaaaa".into();
        save(&h, s);
    }
    // sesja z innego komputera bez znanej gry - nie idzie jeszcze raz (nic tu o niej nowego)
    let mut other = session("obcy", "p1");
    other.profile = String::new();
    other.profile_name = "gra z innego komputera".into();
    other.updated = 50.0;
    h.upsert_from_server(&other, false).expect("pobrana");
    let (mut account, _) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    let shared = |_: &str| true;
    assert_eq!(account.upload(&h, Some(&shared), now()).ok(), Some(150));
    let before = server
        .session(&uuid_of(&format!("{:032x}", 1)))
        .expect("sesja")["zmieniono_utc_s"]
        .as_f64()
        .expect("czas");
    // serwer bez pól gier (`poziomy` puste) - nic jeszcze raz
    assert!(!account.state.games_allowed() && !account.state.resend_due());
    assert_eq!(account.upload(&h, Some(&shared), now()).ok(), Some(0));
    server.state().terms_current = "2026-11-26".into();
    server.state().terms_games = "2026-11-26".into();
    account.refresh().expect("odświeżenie");
    account.accept_terms().expect("akceptacja");
    server.state().requests.clear();
    let at = now() + 10.0;
    assert_eq!(account.upload(&h, Some(&shared), at).ok(), Some(150));
    let posts = server
        .state()
        .requests
        .iter()
        .filter(|r| r.method == "POST" && r.path == "/api/bieznia/sesje")
        .count();
    assert_eq!(posts, 2); // paczki po 100
    let s = server
        .session(&uuid_of(&format!("{:032x}", 1)))
        .expect("sesja");
    assert_eq!(s["zmieniono_utc_s"].as_f64(), Some(at));
    assert!(at > before && s["gra_id"] == "aaaaaaaaaa");
    assert!(server.session(&uuid_of("obcy")).is_none());
    assert_eq!(account.state.resent_terms, "2026-11-26");
    // raz na wersję regulaminu, także po ponownym otwarciu konta
    assert_eq!(account.upload(&h, Some(&shared), at + 10.0).ok(), Some(0));
    let saved = account::load_state(&dir.path().join("konto.json"));
    assert_eq!(saved.resent_terms, "2026-11-26");
}

// --- wiele osób ----------------------------------------------------------------------------

#[test]
fn each_person_has_own_account_and_vault_entry() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let path = dir.path().join("konto.json");
    let (vault, store_for) = stores();
    let mut accs = Accounts::with_parts(BASE, &path, server.transport(), store_for, None);
    accs.get("p1")
        .register("KOD-START", "a@example.pl", PASSWORD, "Ala", "p1")
        .expect("p1");
    accs.get("p2")
        .register("KOD-START", "b@example.pl", PASSWORD, "Bob", "p2")
        .expect("p2");
    let mut names: Vec<String> = accs.logged_in().map(|a| a.state.name.clone()).collect();
    names.sort();
    assert_eq!(names, ["Ala", "Bob"]);
    assert_ne!(vault_get(&vault, "p1"), vault_get(&vault, "p2")); // osobne tokeny
    let uid = accs.get("p2").state.user_id.clone();
    assert_eq!(accs.person_of(&uid), "p2");
    // to samo konto dla innej osoby - zajęte przez p2; dla p2 i obcego konta - wolne
    assert_eq!(accs.taken_by(&uid, "p1").as_deref(), Some("p2"));
    assert_eq!(accs.taken_by(&uid, "p2"), None);
    assert_eq!(accs.taken_by("obcy", "p1"), None);
    assert_eq!(account::load_states(&path).len(), 2);
    // po restarcie - te same sejfy
    let shared = vault.clone();
    let again_store: account::StoreFor = Box::new(move |person: &str| {
        shared.lock().expect("sejf")[person].clone() as Arc<dyn TokenStore>
    });
    let mut again = Accounts::with_parts(BASE, &path, server.transport(), again_store, None);
    let mut people: Vec<String> = again.logged_in().map(|a| a.state.person.clone()).collect();
    people.sort();
    assert_eq!(people, ["p1", "p2"]);
    again.forget_person("p1").expect("usunięcie osoby");
    let left: Vec<String> = again.logged_in().map(|a| a.state.person.clone()).collect();
    assert_eq!(left, ["p2"]);
    assert!(vault_get(&vault, "p1").is_empty());
    assert_eq!(account::load_states(&path).len(), 1);
}

#[test]
fn without_remember_login_lasts_until_closing() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let path = dir.path().join("konto.json");
    let (mut accs, vault) = accounts(&server, &path);
    let acc = accs.get("p1");
    acc.state.remember = false;
    acc.register("KOD-START", "a@example.pl", PASSWORD, "Ala", "p1")
        .expect("rejestracja");
    assert!(!acc.client().token().is_empty() && vault_get(&vault, "p1").is_empty());
    let (again, _) = accounts(&server, &path);
    assert_eq!(again.logged_in().count(), 0);
    // „Zapamiętaj” włączone później - token trafia do sejfu
    let acc = accs.get("p1");
    acc.set_remember(true).expect("zapamiętaj");
    assert!(vault_get(&vault, "p1").starts_with("tok-"));
}

#[test]
fn new_person_login_attaches_to_new_person() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let (mut accs, vault) = accounts(&server, &dir.path().join("k.json"));
    let mut temp = accs.blank();
    temp.register("KOD-START", "a@example.pl", PASSWORD, "Ala", "")
        .expect("rejestracja");
    assert!(!dir.path().join("k.json").exists()); // konto tymczasowe nic nie zapisuje
    let attached = accs.attach(temp, "nowa").expect("przypięcie");
    assert_eq!(attached.state.person, "nowa");
    let token = attached.client().token().to_string();
    assert_eq!(token, vault_get(&vault, "nowa"));
    assert_eq!(accs.get("nowa").client().token(), token);
    assert_eq!(account::load_states(&dir.path().join("k.json")).len(), 1);
}

#[test]
fn single_account_file_from_previous_version_moves_over() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let path = dir.path().join("konto.json");
    let old = json!({"user_id": "u1", "name": "Ala", "email": "a@example.pl", "person": "p1"});
    std::fs::write(&path, old.to_string()).expect("plik");
    let legacy = Arc::new(MemoryStore::new());
    legacy.set("tok-stary").expect("sejf");
    let (vault, store_for) = stores();
    let accs = Accounts::with_parts(
        BASE,
        &path,
        server.transport(),
        store_for,
        Some(legacy.clone() as Arc<dyn TokenStore>),
    );
    let names: Vec<String> = accs.logged_in().map(|a| a.state.name.clone()).collect();
    assert_eq!(names, ["Ala"]);
    assert_eq!(vault_get(&vault, "p1"), "tok-stary");
    assert!(legacy.get().is_empty());
}

#[test]
fn account_without_token_in_vault_counts_as_logged_out() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let path = dir.path().join("konto.json");
    let old = json!({"wersja": 2, "konta": [{"user_id": "u1", "name": "Ala", "person": "p1"}]});
    std::fs::write(&path, old.to_string()).expect("plik");
    let (accs, _) = accounts(&server, &path);
    assert_eq!(accs.logged_in().count(), 0);
}

#[test]
fn bad_or_empty_server_address_disables_accounts() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    for base in ["", "http://bieznia.example.pl"] {
        let (store_for_vault, store_for) = stores();
        drop(store_for_vault);
        let mut accs = Accounts::with_parts(
            base,
            &dir.path().join("k.json"),
            server.transport(),
            store_for,
            None,
        );
        assert!(!accs.enabled() && !accs.get("p1").enabled());
    }
}

// --- synchronizacja (docs/synchro.md) ------------------------------------------------------

struct Computer {
    _dir: tempfile::TempDir,
    data: PathBuf,
    history: History,
    accounts: Accounts,
    person: String,
    person_name: String,
    profiles: HashMap<String, String>,
}

impl Computer {
    fn new(server: &FakeServer, person: &str, name: &str) -> Self {
        let dir = tempfile::tempdir().expect("katalog");
        let data = dir.path().to_path_buf();
        let history = open_history(&data);
        let (vault, store_for) = stores();
        drop(vault);
        let accounts = Accounts::with_parts(
            BASE,
            &data.join(account::STATE_FILE),
            server.transport(),
            store_for,
            None,
        );
        Self {
            _dir: dir,
            data,
            history,
            accounts,
            person: person.to_string(),
            person_name: name.to_string(),
            profiles: HashMap::new(),
        }
    }

    fn account(&mut self) -> &mut Account {
        self.accounts.get(&self.person)
    }

    fn sync_at(&mut self, at: f64) -> account::SyncReport {
        let person = self.person.clone();
        let target = SyncTarget {
            person_name: &self.person_name,
            profiles: &self.profiles,
        };
        self.accounts
            .get(&person)
            .sync(&self.history, &target, at)
            .expect("synchronizacja")
    }

    fn sync(&mut self) -> account::SyncReport {
        self.sync_at(now())
    }

    fn upload(&mut self) -> u32 {
        let person = self.person.clone();
        self.accounts
            .get(&person)
            .upload(&self.history, None, now())
            .expect("wysyłka")
    }

    fn walk(&self, sid: &str, steps: i64) -> Session {
        let mut s = session(sid, &self.person);
        s.person_name.clone_from(&self.person_name);
        s.steps = steps;
        save(&self.history, s)
    }

    fn steps_of(&self, sid: &str) -> Option<i64> {
        self.history
            .session(&hexid(sid))
            .expect("baza")
            .map(|s| s.steps)
    }
}

fn two_computers() -> (FakeServer, Computer, Computer) {
    let server = FakeServer::new();
    let mut a = Computer::new(&server, "pa", "Ola");
    let mut b = Computer::new(&server, "pb", "Ola w pracy");
    let person_a = a.person.clone();
    a.account()
        .register("KOD-START", "ola@example.pl", PASSWORD, "Ola_77", &person_a)
        .expect("rejestracja");
    let person_b = b.person.clone();
    b.account()
        .login("ola@example.pl", PASSWORD, &person_b)
        .expect("logowanie");
    (server, a, b)
}

#[test]
fn two_computers_with_one_account_see_the_same_sessions() {
    let (server, mut a, mut b) = two_computers();
    a.walk("s1", 1000);
    a.walk("s2", 2000);
    assert_eq!(a.upload(), 2);
    assert_eq!(a.sync().saved, 0); // własne sesje - lokalna wersja ta sama

    // B pobiera sesje z A: osoba B, nazwa gry nieznana lokalnie
    let report = b.sync();
    assert!(report.full && report.saved == 2 && report.changed());
    let got = b.history.session(&hexid("s1")).expect("baza").expect("s1");
    assert_eq!(
        (got.person.as_str(), got.person_name.as_str()),
        ("pb", "Ola w pracy")
    );
    assert_eq!(
        (got.profile.as_str(), got.profile_name.as_str()),
        ("", "gra z innego komputera")
    );
    assert_eq!(
        (got.steps, got.mode.as_str(), got.source.as_str()),
        (1000, "gra", "bieznia")
    );
    // pobranych nie odsyłamy
    assert!(b.history.unsent("pb", now()).expect("kolejka").is_empty());
    let posts_before = server
        .paths()
        .iter()
        .filter(|p| p.as_str() == "/api/bieznia/sesje")
        .count();
    assert_eq!(b.upload(), 0);
    let posts_after = server
        .paths()
        .iter()
        .filter(|p| p.as_str() == "/api/bieznia/sesje")
        .count();
    assert_eq!(posts_before, posts_after);
    // nic nowego: ten sam kursor, nic nie zapisane
    let cursor = b.account().state.sync_cursor.clone();
    assert!(!cursor.is_empty());
    let again = b.sync();
    assert!(!again.full && !again.changed() && again.pages == 1);
    assert_eq!(b.account().state.sync_cursor, cursor);
    let file = std::fs::read_to_string(b.data.join(account::STATE_FILE)).expect("konto.json");
    assert!(file.contains("\"sync_cursor\"") && !file.contains("tok-"));

    // A zmienia sesję - B dostaje nową wersję
    let mut s1 = a.history.session(&hexid("s1")).expect("baza").expect("s1");
    s1.steps = 1500;
    save(&a.history, s1);
    assert_eq!(a.upload(), 1);
    assert_eq!(b.sync().saved, 1);
    assert_eq!(b.steps_of("s1"), Some(1500));

    // B dochodzi do sesji z B - A ją dostaje
    b.walk("s3", 3000);
    assert_eq!(b.upload(), 1);
    assert_eq!(a.sync().saved, 1);
    assert_eq!(a.steps_of("s3"), Some(3000));
    assert_eq!(
        a.history
            .session(&hexid("s3"))
            .expect("baza")
            .expect("s3")
            .person,
        "pa"
    );

    // B usuwa sesję (lokalnie i z konta) - A usuwa ją po nagrobku
    b.history.delete(&hexid("s3")).expect("usunięcie");
    b.account()
        .delete_session(&hexid("s3"))
        .expect("usunięcie z konta");
    let report = a.sync();
    assert_eq!((report.saved, report.deleted), (0, 1));
    assert_eq!(a.steps_of("s3"), None);
    assert_eq!(b.sync().deleted, 0); // B już jej nie ma
}

#[test]
fn stale_queue_of_deleted_session_is_removed_not_retried() {
    let (_server, mut a, mut b) = two_computers();
    a.walk("s2", 2000);
    assert_eq!(a.upload(), 1);
    b.sync();
    // B usuwa, A jeszcze o tym nie wie i wysyła zmienioną wersję (stara kolejka)
    b.history.delete(&hexid("s2")).expect("usunięcie");
    b.account()
        .delete_session(&hexid("s2"))
        .expect("usunięcie z konta");
    let mut s2 = a.history.session(&hexid("s2")).expect("baza").expect("s2");
    s2.steps = 2100;
    save(&a.history, s2);
    assert_eq!(a.upload(), 0);
    assert_eq!(a.steps_of("s2"), None); // usunięta lokalnie
    assert!(a.history.rejected().expect("odrzucone").is_empty()); // bez ponawiania
    assert!(
        a.history
            .unsent("pa", now() + 2.0 * account::REJECTED_RETRY_S)
            .expect("kolejka")
            .is_empty()
    );
    assert_eq!(a.sync().deleted, 0); // nagrobek - już jej nie ma
}

#[test]
fn local_newer_version_wins_over_older_from_server() {
    let (_server, mut a, mut b) = two_computers();
    a.walk("s1", 1000);
    a.upload();
    b.sync();
    // B zmienia lokalnie (nowsza), zanim A coś wyśle; serwer ma starszą
    let mut s1 = b.history.session(&hexid("s1")).expect("baza").expect("s1");
    s1.steps = 1200;
    save(&b.history, s1);
    // pełne pobranie od nowa (kursor wyczyszczony) - starsza wersja z serwera nie wygrywa
    b.account().state.sync_cursor.clear();
    assert_eq!(b.sync().saved, 0);
    assert_eq!(b.steps_of("s1"), Some(1200));
    assert_eq!(b.history.unsent("pb", now()).expect("kolejka").len(), 1); // pójdzie na serwer
    assert_eq!(b.upload(), 1);
    assert_eq!(a.sync().saved, 1);
    assert_eq!(a.steps_of("s1"), Some(1200));
}

#[test]
fn known_game_keeps_local_profile_and_unknown_named_game_becomes_profile() {
    let (server, mut a, mut b) = two_computers();
    server.state().terms_current = "2026-11-26".into();
    server.state().terms_games = "2026-11-26".into();
    for c in [&mut a, &mut b] {
        c.account().refresh().expect("odświeżenie");
        let person = c.person.clone();
        c.accounts.get(&person).accept_terms().expect("regulamin");
    }
    let mut s = session("g1", "pa");
    s.profile = "aaaaaaaaaa".into();
    s.profile_name = "Cyberpunk 2077".into();
    save(&a.history, s);
    let mut s = session("g2", "pa");
    s.profile = "cccccccccc".into();
    s.profile_name = "Wiedźmin 3".into();
    save(&a.history, s);
    let mut s = session("g3", "pa");
    s.mode = "trening".into();
    s.profile = "cccccccccc".into();
    s.profile_name = "Wiedźmin 3".into();
    save(&a.history, s);
    let mut s = session("g4", "pa");
    s.profile = "dddddddddd".into();
    s.profile_name = "Prywatna gra".into();
    save(&a.history, s);
    let shared = |pid: &str| pid != "dddddddddd";
    let person = a.person.clone();
    a.accounts
        .get(&person)
        .upload(&a.history, Some(&shared), now())
        .expect("wysyłka");
    b.profiles
        .insert("aaaaaaaaaa".into(), "Cyberpunk (laptop)".into());
    let report = b.sync();
    assert_eq!(report.saved, 4);
    let g = |sid: &str| {
        let s = b
            .history
            .session(&hexid(sid))
            .expect("baza")
            .expect("sesja");
        (s.profile, s.profile_name, s.mode)
    };
    let game =
        |id: &str, name: &str, mode: &str| (id.to_string(), name.to_string(), mode.to_string());
    assert_eq!(g("g1"), game("aaaaaaaaaa", "Cyberpunk (laptop)", "gra"));
    assert_eq!(g("g2"), game("cccccccccc", "Wiedźmin 3", "gra"));
    assert_eq!(g("g3"), game("cccccccccc", "Wiedźmin 3", "trening"));
    assert_eq!(g("g4"), game("dddddddddd", "Prywatna gra", "gra"));
    // nowe profile do założenia: bez powtórzeń, bez znanej gry, z przełącznikiem z serwera
    assert_eq!(
        report.new_games,
        vec![
            account::NewGame {
                id: "cccccccccc".into(),
                name: "Wiedźmin 3".into(),
                public: true,
            },
            account::NewGame {
                id: "dddddddddd".into(),
                name: "Prywatna gra".into(),
                public: false,
            },
        ]
    );
}

#[test]
fn terms_accepted_on_other_computer_also_resend_history_here() {
    let (server, mut a, mut b) = two_computers();
    let mut s = session("w1", "pb");
    s.profile = "eeeeeeeeee".into();
    s.profile_name = "Gra z pracy".into();
    save(&b.history, s);
    assert_eq!(b.upload(), 1);
    server.state().terms_current = "2026-11-26".into();
    server.state().terms_games = "2026-11-26".into();
    a.account().refresh().expect("odświeżenie");
    a.account().accept_terms().expect("regulamin");
    b.account().refresh().expect("odświeżenie"); // akceptacja przyszła z serwera
    assert!(b.account().state.games_allowed());
    let shared = |_: &str| true;
    let person = b.person.clone();
    let sent = b
        .accounts
        .get(&person)
        .upload(&b.history, Some(&shared), now() + 10.0)
        .expect("wysyłka");
    assert_eq!(sent, 1);
    let w = server.session(&uuid_of("w1")).expect("w1");
    assert_eq!(
        (w["gra"].clone(), w["gra_id"].clone()),
        (json!("Gra z pracy"), json!("eeeeeeeeee"))
    );
}

#[test]
fn session_from_server_game_and_mode_rules() {
    let profiles = HashMap::from([("aaaaaaaaaa".to_string(), "Cyberpunk".to_string())]);
    let target = SyncTarget {
        person_name: "Ola",
        profiles: &profiles,
    };
    let remote = |game_id: &str, game: &str, mode: &str| account::RemoteSession {
        uuid: uuid_of("r"),
        game_id: game_id.into(),
        game: game.into(),
        mode: mode.into(),
        visibility: "znajomi".into(),
        ..account::RemoteSession::default()
    };
    let mut local = session("r", "p1");
    local.profile = "bbbbbbbbbb".into();
    local.profile_name = "Lokalna".into();
    local.mode = "trening".into();
    let from = |r: &account::RemoteSession, l: Option<&Session>| {
        let s = account::sync::session_from_server(r, &hexid("r"), "p1", &target, l);
        (s.profile, s.profile_name, s.mode)
    };
    let x = |id: &str, name: &str, mode: &str| (id.to_string(), name.to_string(), mode.to_string());
    // po gra_id - lokalna nazwa profilu
    assert_eq!(
        from(&remote("aaaaaaaaaa", "Inna", "gra"), None),
        x("aaaaaaaaaa", "Cyberpunk", "gra")
    );
    // nieznany gra_id z nazwą - ten sam id, nazwa z serwera (pojedyncze spacje)
    assert_eq!(
        from(
            &remote("cccccccccc", " Nowa   gra ", "trening"),
            Some(&local)
        ),
        x("cccccccccc", "Nowa gra", "trening")
    );
    // bez gier z serwera (sprzed regulaminu): profil i tryb lokalnej sesji
    assert_eq!(
        from(&remote("", "", ""), Some(&local)),
        x("bbbbbbbbbb", "Lokalna", "trening")
    );
    // gra_id bez nazwy i bez lokalnej sesji - ten id, „gra z innego komputera”
    assert_eq!(
        from(&remote("cccccccccc", "", "gra"), None),
        x("cccccccccc", account::sync::OTHER_COMPUTER_GAME, "gra")
    );
    // zły format gra_id - jak brak
    assert_eq!(
        from(&remote("../zle", "Gra", ""), None),
        x("", "Gra", "gra")
    );
    assert_eq!(
        from(&remote("", "", ""), None),
        x("", account::sync::OTHER_COMPUTER_GAME, "gra")
    );
    // tryb z serwera wygrywa z lokalnym
    assert_eq!(from(&remote("", "", "gra"), Some(&local)).2, "gra");
}

#[test]
fn pages_follow_cursor_until_no_more() {
    let (server, mut a, mut b) = two_computers();
    for i in 0..5 {
        a.walk(&format!("p{i}"), 100 + i);
    }
    a.upload();
    server.state().page_size = 2;
    let report = b.sync();
    assert_eq!((report.pages, report.saved), (3, 5));
    let limits: Vec<String> = server
        .state()
        .requests
        .iter()
        .filter(|r| r.method == "GET" && r.path == "/api/bieznia/sesje")
        .map(|r| r.path.clone())
        .collect();
    assert_eq!(limits.len(), 3);
    assert_eq!(b.account().state.sync_cursor, "c5");
}

#[test]
fn old_server_without_sync_route_is_skipped_quietly() {
    let (server, mut a, mut b) = two_computers();
    server.state().no_sync = true;
    a.walk("s1", 1000);
    a.upload();
    let report = b.sync();
    assert!(report.unsupported && !report.changed());
    let before = server.requests();
    assert!(b.sync().unsupported);
    assert_eq!(server.requests(), before); // dalej nie pytamy
    assert_eq!(a.upload(), 0); // wysyłka działa dalej (nic nowego)
}

#[test]
fn bad_cursor_means_full_download() {
    let (_server, mut a, mut b) = two_computers();
    a.walk("s1", 1000);
    a.upload();
    b.account().state.sync_cursor = "zly-kursor".into();
    let report = b.sync();
    assert!(report.full && report.saved == 1);
    assert_eq!(b.account().state.sync_cursor, "c1");
}

#[test]
fn expired_cursor_downloads_all_and_drops_sent_sessions_missing_on_server() {
    let (server, mut a, mut b) = two_computers();
    a.walk("s1", 1000);
    a.walk("s2", 2000);
    a.upload();
    b.sync();
    // serwer usunął s2 dawno, nagrobek już skasowany (90 dni) - B tego nie widział
    {
        let mut st = server.state();
        st.sessions.retain(|(_, uuid), _| *uuid != uuid_of("s2"));
        st.purged_before = 1000;
    }
    b.walk("s9", 900); // niewysłana - zostaje
    let later = now() + 3600.0; // zapisane dawno (nie „przed chwilą”)
    let report = b.sync_at(later);
    assert!(report.full && report.deleted == 1);
    assert_eq!(b.steps_of("s2"), None);
    assert_eq!(b.steps_of("s1"), Some(1000));
    assert_eq!(b.steps_of("s9"), Some(900));
    assert_eq!(b.upload(), 1); // niewysłana idzie na serwer
}

#[test]
fn just_sent_sessions_survive_expired_cursor_recovery() {
    let (server, mut a, mut b) = two_computers();
    a.walk("s1", 1000);
    a.upload();
    b.sync();
    b.walk("s5", 500);
    b.upload();
    {
        let mut st = server.state();
        st.sessions.retain(|(_, uuid), _| *uuid != uuid_of("s5")); // jeszcze nie oddana
        st.purged_before = 1000;
    }
    let report = b.sync();
    assert_eq!(report.deleted, 0);
    assert_eq!(b.steps_of("s5"), Some(500));
}

#[test]
fn logout_and_other_account_clear_the_cursor() {
    let (server, mut a, mut b) = two_computers();
    a.walk("s1", 1000);
    a.upload();
    b.sync();
    assert!(!b.account().state.sync_cursor.is_empty());
    b.account().logout();
    assert!(b.account().state.sync_cursor.is_empty());
    let file = std::fs::read_to_string(b.data.join(account::STATE_FILE)).expect("konto.json");
    assert!(!file.contains("sync_cursor"));
    // sync bez logowania - nic
    let before = server.requests();
    assert_eq!(b.sync(), account::SyncReport::default());
    assert_eq!(server.requests(), before);
    // logowanie innym kontem też zaczyna od zera
    let person_b = b.person.clone();
    b.account()
        .login("ola@example.pl", PASSWORD, &person_b)
        .expect("logowanie");
    b.sync();
    let other = b.account();
    other
        .register(
            "KOD-START",
            "inny@example.pl",
            PASSWORD,
            "Inny_1",
            &person_b,
        )
        .expect("inne konto");
    assert!(other.state.sync_cursor.is_empty());
}

#[test]
fn sync_with_expired_token_logs_out() {
    let (server, _a, mut b) = two_computers();
    server.state().tokens.clear();
    let person = b.person.clone();
    let target = SyncTarget {
        person_name: "x",
        profiles: &HashMap::new(),
    };
    let e = b
        .accounts
        .get(&person)
        .sync(&b.history, &target, now())
        .expect_err("401");
    assert!(e.is_logged_out());
    assert!(!b.account().state.logged_in());
}

#[test]
fn sessions_of_other_local_person_are_not_touched() {
    let (_server, mut a, mut b) = two_computers();
    a.walk("s1", 1000);
    a.upload();
    // ta sama sesja u innej osoby na B (np. z kopii danych)
    let mut copy = session("s1", "ktos-inny");
    copy.steps = 7;
    save(&b.history, copy);
    assert_eq!(b.sync().saved, 0);
    let kept = b.history.session(&hexid("s1")).expect("baza").expect("s1");
    assert_eq!((kept.person.as_str(), kept.steps), ("ktos-inny", 7));
}

#[test]
fn interrupted_recovery_after_expired_cursor_is_finished_next_time() {
    let (server, mut a, mut b) = two_computers();
    for i in 0..4 {
        a.walk(&format!("r{i}"), 100 + i);
    }
    a.upload();
    b.sync();
    let stale = b.account().state.sync_cursor.clone();
    {
        let mut st = server.state();
        st.sessions.retain(|(_, uuid), _| *uuid != uuid_of("r3"));
        st.expired = vec![stale.clone()];
        st.page_size = 2;
        st.sync_pages_left = Some(2); // 410 i pierwsza strona, druga już bez sieci
    }
    let later = now() + 3600.0;
    let person = b.person.clone();
    let target = SyncTarget {
        person_name: "Ola w pracy",
        profiles: &HashMap::new(),
    };
    let e = b
        .accounts
        .get(&person)
        .sync(&b.history, &target, later)
        .expect_err("bez sieci");
    assert_eq!(e.kind, KontoErrorKind::Offline);
    // nic nie usunięte, stary kursor został - następna synchronizacja znów dostanie 410
    assert_eq!(b.steps_of("r3"), Some(103));
    assert_eq!(b.account().state.sync_cursor, stale);
    server.state().sync_pages_left = None;
    let report = b.sync_at(later);
    assert!(report.full && report.deleted == 1);
    assert_eq!(b.steps_of("r3"), None);
    assert_eq!(b.steps_of("r0"), Some(100));
    assert_ne!(b.account().state.sync_cursor, stale);
    // dalej zwykłe pobieranie od nowego kursora
    let again = b.sync_at(later);
    assert!(!again.full && !again.changed());
}

#[test]
fn session_changed_after_sending_survives_expired_cursor_recovery() {
    let (server, mut a, mut b) = two_computers();
    a.walk("s1", 1000);
    a.upload();
    b.sync();
    // B zmienia pobraną sesję (jeszcze nie wysłana), a serwer jej już nie ma
    let mut s1 = b.history.session(&hexid("s1")).expect("baza").expect("s1");
    s1.steps = 1100;
    save(&b.history, s1);
    {
        let mut st = server.state();
        st.sessions.clear();
        st.purged_before = 1000;
    }
    let report = b.sync_at(now() + 3600.0);
    assert_eq!(report.deleted, 0);
    assert_eq!(b.steps_of("s1"), Some(1100)); // niewysłana wersja zostaje i pójdzie
    assert_eq!(b.upload(), 1);
}

#[test]
fn session_hidden_on_one_computer_stays_hidden_on_the_other() {
    let (server, mut a, mut b) = two_computers();
    a.walk("s1", 1000);
    a.walk("s2", 2000);
    a.history
        .set_hidden_online(&hexid("s1"), true)
        .expect("ukryj");
    a.upload();
    assert_eq!(b.sync().saved, 2);
    let hidden = b.history.hidden_online().expect("ukryte");
    assert!(hidden.contains(&hexid("s1")) && !hidden.contains(&hexid("s2")));
    assert!(b.history.unsent("pb", now()).expect("kolejka").is_empty()); // bez odsyłania
    // zmiana widoczności na B wysyła wszystko jeszcze raz - ukryta zostaje prywatna
    let person = b.person.clone();
    b.accounts
        .get(&person)
        .set_visibility(&b.history, "publiczna", now() + 10.0)
        .expect("widoczność");
    assert_eq!(b.upload(), 2);
    assert_eq!(
        server.session(&uuid_of("s1")).expect("s1")["widocznosc"],
        "prywatna"
    );
    assert_eq!(
        server.session(&uuid_of("s2")).expect("s2")["widocznosc"],
        "publiczna"
    );
}

#[test]
fn games_need_server_levels_version_and_accepted_terms_not_older() {
    let mut st = account::AccountState {
        terms: "2026-11-26".into(),
        ..account::AccountState::default()
    };
    assert!(!st.games_allowed()); // serwer jeszcze nie przyjmuje pól gier
    st.terms_games = "2026-11-26".into();
    assert!(st.games_allowed() && st.resend_due());
    st.terms = "2026-11-25".into();
    assert!(!st.games_allowed()); // zaakceptowana starsza wersja
    st.terms = "2027-01-10".into();
    assert!(st.games_allowed()); // nowsza też wystarcza
    st.resent_terms = "2027-01-10".into();
    assert!(!st.resend_due()); // historia już poszła z grami
    // zapis i odczyt z konto.json
    let v = serde_json::to_value(&st).expect("json");
    assert_eq!(v["terms_games"], "2026-11-26");
    assert_eq!(account::AccountState::from_value(&v), st);
}

#[test]
fn levels_version_comes_from_terms_route_on_register_and_refresh() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    server.state().terms_current = "2026-11-26".into();
    let (mut account, _) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    assert_eq!(account.state.terms_games, "");
    assert!(!account.state.games_allowed());
    server.state().terms_games = "2026-11-26".into();
    account.refresh().expect("odświeżenie");
    assert_eq!(account.state.terms_games, "2026-11-26");
    assert!(account.state.games_allowed());
    // logowanie na drugim komputerze: od razu, bez czekania na odświeżenie
    let dir2 = tempfile::tempdir().expect("katalog");
    let (mut other, _) = standalone(&server, dir2.path());
    other
        .login("ola@example.pl", PASSWORD, "p2")
        .expect("logowanie");
    assert_eq!(other.state.terms_games, "2026-11-26");
    assert_eq!(other.state.terms, "2026-11-26");
    assert!(other.state.games_allowed());
}

#[test]
fn skipped_sessions_count_as_sent_without_loop() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let h = open_history(dir.path());
    let s = save(&h, session("a", "p1"));
    let (mut account, _) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    assert_eq!(account.upload(&h, None, now()).ok(), Some(1));
    // na serwerze nowsza wersja z innego komputera; tu ta sama sesja wraca do kolejki
    let uid = server.state().users.keys().next().cloned().expect("konto");
    let key = (uid, uuid_of("a"));
    server.state().sessions.get_mut(&key).expect("sesja").0["zmieniono_utc_s"] =
        json!(s.updated + 100.0);
    h.forget_sent().expect("kolejka");
    assert_eq!(account.upload(&h, None, now()).ok(), Some(1)); // pominięta = wysłana
    assert_eq!(account.upload(&h, None, now()).ok(), Some(0)); // bez pętli
    assert_eq!(
        server.session(&uuid_of("a")).expect("a")["zmieniono_utc_s"],
        json!(s.updated + 100.0)
    );
}

#[test]
fn hiding_one_session_online_sends_it_newer() {
    let dir = tempfile::tempdir().expect("katalog");
    let server = FakeServer::new();
    let h = open_history(dir.path());
    let s = save(&h, session("a", "p1"));
    let (mut account, _) = standalone(&server, dir.path());
    signed_in(&mut account, "p1");
    account.upload(&h, None, now()).expect("wysyłka");
    h.set_hidden_online(&hexid("a"), true).expect("ukryj");
    assert_eq!(account.upload(&h, None, now()).ok(), Some(1));
    let a = server.session(&uuid_of("a")).expect("a");
    assert_eq!(a["widocznosc"], "prywatna");
    assert!(a["zmieniono_utc_s"].as_f64().expect("czas") > s.updated);
}
