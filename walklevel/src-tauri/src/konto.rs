//! Konto online (bieznia/gui/ekrany/konto.py): zadania sieciowe poza pętlą i poza blokadą
//! stanu aplikacji, zegary wysyłki / synchronizacji (docs/synchro.md, co 5 min) / odświeżenia /
//! stanu na żywo, sprawdzanie potwierdzenia adresu e-mail, dane online do wycinka `online`.
//!
//! Kolejność blokad: najpierw `konto`, potem `app` - nigdy odwrotnie. Pętla blokuje tylko
//! `app`, więc zadanie czekające na serwer (do 15 s) jej nie zatrzymuje.

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;
use walklevel_core::account::{self, Account, Accounts, KontoError, KontoErrorKind, SyncTarget};
use walklevel_core::history::{History, now_unix};

use crate::stan::{App, Core, KontoWidok, lock};
use crate::widoki::*;

/// Konta wszystkich osób i osobne połączenie z historią (wysyłka i pobieranie sesji nie
/// czekają na pętlę i odwrotnie - SQLite w trybie WAL).
pub struct KontoCtx {
    pub accounts: Accounts,
    pub history: Option<History>,
}

impl KontoCtx {
    pub fn open(base_url: &str, data: &Path) -> Self {
        let history = History::open(&History::default_path(data))
            .map_err(|e| log::warn!("Konto: historia niedostępna: {e}"))
            .ok();
        Self {
            accounts: Accounts::open(base_url, data),
            history,
        }
    }

    /// Stan kont do pamięci aplikacji (bez sieci).
    pub fn snapshot(&mut self, people: &[String]) -> HashMap<String, KontoWidok> {
        let now = now_unix();
        let mut out = HashMap::new();
        for person in people {
            let acc = self.accounts.get(person);
            let pending = match (&self.history, acc.state.logged_in()) {
                (Some(h), true) => acc.pending_uploads(h, now).unwrap_or(0),
                _ => 0,
            };
            out.insert(
                person.clone(),
                KontoWidok {
                    state: acc.state.clone(),
                    pending,
                },
            );
        }
        out
    }
}

fn people_ids(app: &App) -> Vec<String> {
    app.store
        .as_ref()
        .map(|s| s.people().iter().map(|p| p.id.clone()).collect())
        .unwrap_or_default()
}

/// Akcje z `AccountAction` (kontrakt) - inne (np. synchronizacja) nie trafiają do `busy`.
const VISIBLE_ACTIONS: &[&str] = &[
    "login",
    "register",
    "reset",
    "logout",
    "delete",
    "password",
    "verify",
    "verified",
    "refresh",
    "upload",
    "profile",
    "terms",
    "export",
    "live",
    "invite",
    "answer",
    "removeFriend",
    "regCode",
    "person",
    "loadFriends",
    "loadStats",
    "loadInvitations",
    "loadRanking",
    "loadPeople",
    "deleteSession",
];

impl App {
    pub fn busy_key(person: &str, action: &str) -> String {
        format!("{person}:{action}")
    }

    pub fn visible_busy(action: &str) -> bool {
        VISIBLE_ACTIONS.contains(&action)
    }
}

/// Odświeża kopię stanu kont w aplikacji (po każdym zadaniu i przy starcie).
pub fn refresh_cache(core: &Core, ctx: &mut KontoCtx) {
    let people = people_ids(&lock(&core.0.app));
    let snap = ctx.snapshot(&people);
    let mut app = lock(&core.0.app);
    app.konta = snap;
    app.mark(&[SliceName::Account, SliceName::People]);
}

/// Zadanie konta osoby `person`. Ta sama akcja tej samej osoby w trakcie -> `busy`.
/// Działa w bieżącym wątku (wołać ze `spawn_blocking` albo z wątku roboczego).
pub fn job<T>(
    core: &Core,
    person: &str,
    action: &str,
    quiet: bool,
    work: impl FnOnce(&mut KontoCtx) -> account::Result<T>,
) -> CmdResult<T> {
    let key = App::busy_key(person, action);
    {
        let mut app = lock(&core.0.app);
        if !app.account_busy.insert(key.clone()) {
            return Err(CmdError::new(
                ErrorCode::Busy,
                crate::teksty::t("konto.blad.trwa", &[]),
            ));
        }
        app.mark(&[SliceName::Account]);
    }
    core.flush();
    let result = {
        let mut ctx = lock(&core.0.konto);
        let r = work(&mut ctx);
        refresh_cache(core, &mut ctx);
        r
    };
    {
        let mut app = lock(&core.0.app);
        app.account_busy.remove(&key);
        app.mark(&[SliceName::Account]);
        if let Err(e) = &result {
            let mine = app.person_id() == person;
            if e.kind == KontoErrorKind::LoggedOut {
                if mine {
                    app.online = OnlineView::default();
                }
                let name = app.person_name(person);
                app.toast(crate::teksty::t("konto.wygaslo", &[("osoba", &name)]));
            } else if !quiet && mine {
                app.account_message = e.message.clone();
            } else if quiet {
                log::info!("Konto ({action}): {}", e.message);
            }
        }
    }
    core.flush();
    result.map_err(CmdError::from)
}

/// Zadanie w tle bez wyniku (zegary, ciche odświeżenia).
pub fn background(
    core: &Core,
    person: String,
    action: &'static str,
    work: impl FnOnce(&mut KontoCtx) -> account::Result<()> + Send + 'static,
) {
    let core = core.clone();
    let _ = std::thread::Builder::new()
        .name(format!("konto-{action}"))
        .spawn(move || {
            let _ = job(&core, &person, action, true, work);
        });
}

fn current_person(core: &Core) -> String {
    lock(&core.0.app).person_id()
}

fn set_message(core: &Core, text: impl Into<String>) {
    let mut app = lock(&core.0.app);
    app.account_message = text.into();
    app.mark(&[SliceName::Account]);
}

fn account_view(core: &Core) -> AccountView {
    lock(&core.0.app).account_view()
}

fn not_logged_in() -> KontoError {
    KontoError::invalid(&crate::teksty::t("konto.blad.niezalogowany", &[]))
}

// --- zegary (z pętli co 5 klatek) ------------------------------------------------------------

pub fn tick(core: &Core) {
    let now = core.now();
    let (refresh, upload, sync, verify, live, person) = {
        let mut app = lock(&core.0.app);
        if !app.accounts_enabled {
            return;
        }
        let logged: Vec<String> = app
            .konta
            .iter()
            .filter(|(_, k)| k.state.logged_in())
            .map(|(p, _)| p.clone())
            .collect();
        let refresh = app.account_timers.refresh.due(now);
        let upload = app.account_timers.upload.due(now);
        let sync = app.account_timers.sync.due(now);
        let mut verify = false;
        if app.verify_until > now && now >= app.verify_at {
            app.verify_at = now + account::sync::VERIFY_POLL_S;
            verify = true;
        } else if app.verify_until > 0.0 && now >= app.verify_until {
            app.verify_until = 0.0;
            app.mark(&[SliceName::Account]);
        }
        let person = app.person_id();
        let mut live = None;
        let mine_live = app
            .konta
            .get(&person)
            .is_some_and(|k| k.state.logged_in() && k.state.live);
        if mine_live && app.account_timers.live.due(now) {
            let speed = app.bridge.state().control.speed;
            let stats = app.tracker.live(now);
            let current = app.tracker.current().map(|s| s.id.clone());
            match current {
                Some(sid) if speed > 0.05 => {
                    app.live_on = true;
                    live = Some(Some((speed, stats.steps.unwrap_or(0), stats.active_s, sid)));
                }
                _ if app.live_on => {
                    app.live_on = false;
                    live = Some(None);
                }
                _ => {}
            }
        }
        (
            refresh.then(|| logged.clone()),
            upload.then(|| logged.clone()),
            sync.then_some(logged),
            verify,
            live,
            person,
        )
    };
    for p in refresh.unwrap_or_default() {
        background(core, p.clone(), "refresh", move |ctx| {
            ctx.accounts.get(&p).refresh().map(|_| ())
        });
    }
    for p in upload.unwrap_or_default() {
        upload_person(core, p);
    }
    for p in sync.unwrap_or_default() {
        sync_person(core, p);
    }
    if verify {
        let c = core.clone();
        std::thread::spawn(move || {
            let _ = check_verified(&c, true);
        });
    }
    if let Some(live) = live {
        let p = person.clone();
        background(core, person, "live", move |ctx| {
            let acc = ctx.accounts.get(&p);
            match live {
                Some((kmh, steps, active, sid)) => acc.live(kmh, steps, active, &sid),
                None => acc.clear_live(),
            }
        });
    }
}

/// Profile gier osoby, których nazwy wolno pokazać online.
fn shared_games(
    core: &Core,
    person: &str,
) -> (String, HashMap<String, String>, HashMap<String, bool>) {
    let app = lock(&core.0.app);
    let Some(p) = app
        .store
        .as_ref()
        .and_then(|s| s.people().iter().find(|p| p.id == person))
    else {
        return (String::new(), HashMap::new(), HashMap::new());
    };
    let names = p
        .profiles()
        .iter()
        .map(|g| (g.id.clone(), g.name.clone()))
        .collect();
    let online = p
        .profiles()
        .iter()
        .map(|g| (g.id.clone(), g.online))
        .collect();
    (p.name.clone(), names, online)
}

fn upload_with(
    ctx: &mut KontoCtx,
    person: &str,
    online: &HashMap<String, bool>,
) -> account::Result<u32> {
    let Some(history) = ctx.history.as_ref() else {
        return Ok(0);
    };
    let allowed = |profile: &str| online.get(profile).copied().unwrap_or(true);
    ctx.accounts
        .get(person)
        .upload(history, Some(&allowed), now_unix())
}

pub fn upload_person(core: &Core, person: String) {
    let (_, _, online) = shared_games(core, &person);
    let p = person.clone();
    background(core, person, "upload", move |ctx| {
        upload_with(ctx, &p, &online).map(|_| ())
    });
}

pub fn sync_person(core: &Core, person: String) {
    let (name, profiles, _) = shared_games(core, &person);
    let p = person.clone();
    let c = core.clone();
    background(core, person, "sync", move |ctx| {
        let Some(history) = ctx.history.as_ref() else {
            return Ok(());
        };
        let target = SyncTarget {
            person_name: &name,
            profiles: &profiles,
        };
        let report = ctx.accounts.get(&p).sync(history, &target, now_unix())?;
        if report.changed() {
            log::info!(
                "Konto: pobrano zmiany z serwera (zapisane {}, usunięte {})",
                report.saved,
                report.deleted
            );
            let mut app = lock(&c.0.app);
            app.levels_quiet = true; // bez popupów za historię z innego komputera
            app.mark(&[SliceName::History, SliceName::People]);
        }
        Ok(())
    });
}

// --- logowanie ---------------------------------------------------------------------------------

/// Po zalogowaniu: konto przypięte do osoby (nowej albo tej, dla której logowano).
fn signed_in(
    core: &Core,
    ctx: &mut KontoCtx,
    mut acc: Option<Account>,
    person: &str,
    user_id: &str,
) -> account::Result<()> {
    let taken = ctx
        .accounts
        .logged_in()
        .find(|a| a.state.user_id == user_id && a.state.person != person)
        .map(|a| a.state.person.clone());
    if let Some(taken) = taken {
        if !person.is_empty() {
            ctx.accounts.get(person).logout();
        }
        let name = lock(&core.0.app).person_name(&taken);
        return Err(KontoError::invalid(&crate::teksty::t(
            "konto.przypiete",
            &[("osoba", &name)],
        )));
    }
    let mut person = person.to_string();
    if let Some(acc) = acc.take() {
        // nowa osoba z konta online: pseudonim i kolejny kolor palety
        let new_id = {
            let mut app = lock(&core.0.app);
            let Some(store) = app.store.as_mut() else {
                return Err(not_logged_in());
            };
            let colors = walklevel_core::store::palette::COLORS.len();
            let color = walklevel_core::store::palette::COLORS[store.people().len() % colors].name;
            let id = store
                .add_person(&acc.state.name, color, None)
                .map_err(|e| KontoError::new(KontoErrorKind::Io, &e.to_string(), 0))?
                .id
                .clone();
            log::info!("Nowa osoba z konta online: {}", acc.state.name);
            app.apply_profile();
            id
        };
        ctx.accounts.attach(acc, &new_id)?;
        person = new_id;
    }
    let name = ctx.accounts.get(&person).state.name.clone();
    let now = core.now();
    let mut app = lock(&core.0.app);
    app.online = OnlineView::default();
    app.account_message.clear();
    app.toast(crate::teksty::t(
        "konto.zalogowano",
        &[("pseudonim", &name)],
    ));
    app.account_timers.signed_in(now);
    app.nav("online", "signedIn", None);
    Ok(())
}

pub fn login(
    core: &Core,
    email: String,
    password: String,
    new_person: bool,
) -> CmdResult<AccountView> {
    let person = current_person(core);
    let c = core.clone();
    let key = person.clone();
    job(core, &key, "login", false, move |ctx| {
        if new_person {
            let mut acc = ctx.accounts.blank();
            let user = acc.login(&email, &password, "")?;
            signed_in(&c, ctx, Some(acc), "", &user.id)
        } else {
            let user = ctx
                .accounts
                .get(&person)
                .login(&email, &password, &person)?;
            signed_in(&c, ctx, None, &person, &user.id)
        }
    })?;
    Ok(account_view(core))
}

pub fn register(
    core: &Core,
    code: String,
    email: String,
    password: String,
    name: String,
    new_person: bool,
) -> CmdResult<AccountView> {
    let person = current_person(core);
    let c = core.clone();
    let key = person.clone();
    job(core, &key, "register", false, move |ctx| {
        if new_person {
            let mut acc = ctx.accounts.blank();
            let user = acc.register(&code, &email, &password, &name, "")?;
            signed_in(&c, ctx, Some(acc), "", &user.id)
        } else {
            let user = ctx
                .accounts
                .get(&person)
                .register(&code, &email, &password, &name, &person)?;
            signed_in(&c, ctx, None, &person, &user.id)
        }
    })?;
    // od razu link potwierdzający - bez niego nie ma resetu hasła
    let c = core.clone();
    std::thread::spawn(move || {
        let _ = send_verification(&c);
    });
    Ok(account_view(core))
}

pub fn remind(core: &Core, email: String) -> CmdResult<()> {
    let person = current_person(core);
    job(core, &person, "reset", false, move |ctx| {
        ctx.accounts.blank().remind_password(&email)
    })?;
    set_message(core, account::remind_message());
    core.flush();
    Ok(())
}

fn after_logout(core: &Core, text: &str) {
    let mut app = lock(&core.0.app);
    app.online = OnlineView::default();
    app.account_message.clear();
    app.toast(text);
    app.nav("settings", "settings", None);
    drop(app);
    core.flush();
}

pub fn logout(core: &Core) -> CmdResult<()> {
    let person = current_person(core);
    let p = person.clone();
    job(core, &person, "logout", false, move |ctx| {
        ctx.accounts.get(&p).logout();
        Ok(())
    })?;
    after_logout(core, &crate::teksty::t("konto.wylogowano", &[]));
    Ok(())
}

pub fn delete(core: &Core, password: String) -> CmdResult<()> {
    let person = current_person(core);
    let p = person.clone();
    job(core, &person, "delete", false, move |ctx| {
        ctx.accounts.get(&p).delete_account(&password)
    })?;
    after_logout(core, &crate::teksty::t("konto.usunieto", &[]));
    Ok(())
}

pub fn change_password(core: &Core, old: String, new: String) -> CmdResult<()> {
    let person = current_person(core);
    let p = person.clone();
    job(core, &person, "password", false, move |ctx| {
        ctx.accounts.get(&p).change_password(&old, &new)
    })?;
    set_message(core, crate::teksty::t("moje.haslo.zmienione", &[]));
    core.flush();
    Ok(())
}

pub fn send_verification(core: &Core) -> CmdResult<String> {
    let person = current_person(core);
    let p = person.clone();
    let answer = job(core, &person, "verify", false, move |ctx| {
        ctx.accounts.get(&p).send_verification()
    })?;
    let now = core.now();
    let mut app = lock(&core.0.app);
    if answer == "potwierdzony" {
        app.account_message = crate::teksty::t("konto.email.juz", &[]);
    } else {
        let email = app
            .konta
            .get(&person)
            .map(|k| k.state.email.clone())
            .unwrap_or_default();
        app.toast(crate::teksty::t(
            "konto.email.wyslano_toast",
            &[("email", &email)],
        ));
        app.account_message = crate::teksty::t("konto.email.wyslano", &[]);
        app.verify_until = now + account::sync::VERIFY_POLL_FOR_S;
        app.verify_at = now + account::sync::VERIFY_POLL_S;
    }
    app.mark(&[SliceName::Account]);
    drop(app);
    core.flush();
    Ok(answer)
}

/// Czy adres już potwierdzony (ręcznie, co 15 s po wysłaniu linku, po powrocie okna).
pub fn check_verified(core: &Core, force: bool) -> CmdResult<bool> {
    let person = current_person(core);
    let now = core.now();
    {
        let mut app = lock(&core.0.app);
        let state = app
            .konta
            .get(&person)
            .map(|k| k.state.clone())
            .unwrap_or_default();
        if !state.logged_in() || state.verified {
            app.verify_until = 0.0;
            return Ok(state.verified);
        }
        if !force && now - app.verify_checked_at < account::sync::VERIFY_RECHECK_S {
            return Ok(false);
        }
        app.verify_checked_at = now;
    }
    let p = person.clone();
    let verified = job(core, &person, "verified", true, move |ctx| {
        ctx.accounts.get(&p).check_verified()
    })?;
    if verified {
        let mut app = lock(&core.0.app);
        if app.verify_until > 0.0 || force {
            app.toast(crate::teksty::t("konto.email.potwierdzony", &[]));
        }
        app.verify_until = 0.0;
        app.mark(&[SliceName::Account]);
        drop(app);
        core.flush();
    }
    Ok(verified)
}

/// Prosta zmiana ustawienia konta wybranej osoby.
pub fn simple(
    core: &Core,
    action: &str,
    message: Option<&str>,
    work: impl FnOnce(&mut Account, Option<&History>) -> account::Result<()>,
) -> CmdResult<()> {
    let person = current_person(core);
    let p = person.clone();
    job(core, &person, action, false, move |ctx| {
        let history = ctx.history.as_ref();
        let acc = ctx.accounts.get(&p);
        if !acc.state.logged_in() {
            return Err(not_logged_in());
        }
        work(acc, history)
    })?;
    if let Some(m) = message {
        set_message(core, m);
        core.flush();
    }
    Ok(())
}

pub fn upload_now(core: &Core) -> CmdResult<u32> {
    let person = current_person(core);
    let (_, _, online) = shared_games(core, &person);
    let p = person.clone();
    let sent = job(core, &person, "upload", false, move |ctx| {
        upload_with(ctx, &p, &online)
    })?;
    let now = core.now();
    lock(&core.0.app).account_timers.upload.postpone(now);
    Ok(sent)
}

pub fn accept_terms(core: &Core) -> CmdResult<()> {
    simple(core, "terms", None, |acc, h| match h {
        Some(h) => acc.accept_terms(h),
        None => Err(KontoError::new(
            KontoErrorKind::Io,
            &crate::teksty::t("historia.niedostepna", &[]),
            0,
        )),
    })?;
    lock(&core.0.app).toast(crate::teksty::t("konto.regulamin.dziekujemy", &[]));
    core.flush();
    let _ = upload_now(core);
    Ok(())
}

pub fn export_my_data(core: &Core) -> CmdResult<String> {
    let person = current_person(core);
    let p = person.clone();
    let data = lock(&core.0.app).paths.data.clone();
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let path = job(core, &person, "export", false, move |ctx| {
        ctx.accounts.get(&p).export_my_data(&data, &stamp)
    })?;
    let text = path.to_string_lossy().into_owned();
    set_message(
        core,
        crate::teksty::t("moje.konto.pobrano", &[("plik", &text)]),
    );
    core.flush();
    Ok(text)
}

pub fn delete_session_online(core: &Core, person: String, session_id: String) {
    let p = person.clone();
    background(core, person, "deleteSession", move |ctx| {
        ctx.accounts.get(&p).delete_session(&session_id)
    });
}

// --- dane online ---------------------------------------------------------------------------

fn num(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

fn int(v: &Value, key: &str) -> i64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(0.0) as i64
}

fn text(v: &Value, key: &str) -> String {
    match v.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn level(v: &Value) -> Option<u32> {
    v.get("poziom")
        .and_then(Value::as_f64)
        .filter(|l| *l >= 1.0)
        .map(|l| (l as u32).min(100))
}

fn period(v: Option<&Value>) -> PeriodTotals {
    let Some(v) = v.filter(|v| v.is_object()) else {
        return PeriodTotals::default();
    };
    PeriodTotals {
        steps: int(v, "kroki"),
        distance_km: num(v, "dystans_km"),
        active_s: num(v, "ruch_s"),
        sessions: v.get("sesje").and_then(Value::as_f64).map(|s| s as i64),
    }
}

fn games(v: Option<&Value>) -> Option<Vec<GameTotals>> {
    let list = v?.as_array()?;
    Some(
        list.iter()
            .filter(|g| g.is_object())
            .map(|g| GameTotals {
                game: text(g, "gra").trim().to_string(),
                game_id: Some(text(g, "gra_id")).filter(|s| !s.is_empty()),
                steps: int(g, "kroki"),
                distance_km: num(g, "dystans_km"),
                active_s: num(g, "ruch_s"),
            })
            .collect(),
    )
}

fn friends(v: &Value) -> Vec<Friend> {
    v.as_array()
        .map(|l| {
            l.iter()
                .filter(|f| f.is_object())
                .map(|f| Friend {
                    id: text(f, "id"),
                    name: text(f, "name"),
                    level: level(f),
                    live: f
                        .get("na_zywo")
                        .filter(|l| l.is_object())
                        .map(|l| LiveState {
                            kmh: num(l, "kmh"),
                            steps: int(l, "kroki"),
                        }),
                    week: period(f.get("tydzien")),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn invitation_list(v: Option<&Value>) -> Vec<Invitation> {
    v.and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .map(|i| {
                    let who = i.get("osoba").cloned().unwrap_or(Value::Null);
                    let mut name = text(&who, "name");
                    if name.is_empty() {
                        name = text(i, "name");
                    }
                    Invitation {
                        id: text(i, "id"),
                        person: InvitationPerson {
                            id: text(&who, "id"),
                            name: if name.is_empty() { "?".into() } else { name },
                        },
                        created_at: i.get("utworzono_utc_s").and_then(Value::as_f64),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn ranking(v: &Value, scope: &str, my_id: &str, known: &HashMap<String, String>) -> Ranking {
    let rows = v
        .as_array()
        .map(|l| {
            l.iter()
                .map(|r| {
                    let me = r.get("ja").and_then(Value::as_bool).unwrap_or(false);
                    let name = text(r, "name");
                    let mut id = if me { my_id.to_string() } else { text(r, "id") };
                    if id.is_empty() {
                        // serwer nie odsyła id w rankingu - znajomych po pseudonimie
                        id = known.get(&name).cloned().unwrap_or_default();
                    }
                    RankingRow {
                        place: int(r, "miejsce") as u32,
                        id,
                        name,
                        me,
                        level: level(r),
                        steps: int(r, "kroki"),
                        distance_km: num(r, "dystans_km"),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    Ranking {
        scope: scope.into(),
        rows,
    }
}

fn people_page(v: &Value) -> PeoplePage {
    PeoplePage {
        page: v.get("strona").and_then(Value::as_u64).unwrap_or(1) as u32,
        has_next: v.get("nastepna").and_then(Value::as_bool).unwrap_or(false),
        people: v
            .get("osoby")
            .and_then(Value::as_array)
            .map(|l| {
                l.iter()
                    .map(|p| PeopleItem {
                        id: text(p, "id"),
                        name: text(p, "name"),
                        level: level(p),
                        week_steps: p.get("tydzien").map_or(0, |w| int(w, "kroki")),
                        place: p.get("miejsce").and_then(Value::as_u64).map(|m| m as u32),
                        friend: p.get("znajomy").and_then(Value::as_bool).unwrap_or(false),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn online_person(v: &Value, my_id: &str) -> OnlinePerson {
    let id = text(v, "id");
    let achievements = v
        .get("osiagniecia_lista")
        .and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .map(|a| EarnedOnline {
                    id: text(a, "id"),
                    earned_at: num(a, "zdobyte_utc_s"),
                })
                .collect()
        });
    let count = match v.get("osiagniecia") {
        Some(Value::Number(n)) => n.as_f64().map(|n| n as u32),
        Some(Value::Array(a)) => Some(a.len() as u32),
        _ => None,
    };
    OnlinePerson {
        me: !id.is_empty() && id == my_id,
        name: text(v, "name"),
        level: level(v),
        xp: v.get("xp").and_then(Value::as_f64).map(|x| x as i64),
        achievements_count: count,
        achievements,
        since: v.get("od_utc_s").and_then(Value::as_f64),
        treadmill_model: Some(text(v, "bieznia_model")).filter(|s| !s.is_empty()),
        friend: v.get("znajomy").and_then(Value::as_bool).unwrap_or(false),
        week: v
            .get("tydzien")
            .filter(|t| t.is_object())
            .map(|t| period(Some(t))),
        total: v
            .get("razem")
            .filter(|t| t.is_object())
            .map(|t| period(Some(t))),
        games: games(v.get("gry")),
        games_hidden: v.get("gry").is_some_and(Value::is_null),
        id,
    }
}

fn set_loading(core: &Core, key: &str, on: bool) {
    let mut app = lock(&core.0.app);
    app.online.loading.retain(|k| k != key);
    if on {
        app.online.loading.push(key.into());
    }
    app.mark(&[SliceName::Online]);
}

/// Jedno pobranie danych online wybranej osoby do wycinka `online`.
fn load(
    core: &Core,
    key: &'static str,
    action: &str,
    quiet: bool,
    fetch: impl FnOnce(&mut Account) -> account::Result<Value>,
    apply: impl FnOnce(&mut App, &Value),
) -> CmdResult<()> {
    let person = current_person(core);
    if !quiet {
        set_loading(core, key, true);
    }
    let p = person.clone();
    let result = job(core, &person, action, quiet, move |ctx| {
        let acc = ctx.accounts.get(&p);
        if !acc.state.logged_in() {
            return Err(not_logged_in());
        }
        let r = fetch(acc);
        acc.handle(r)
    });
    set_loading(core, key, false);
    let value = result?;
    let mut app = lock(&core.0.app);
    if app.person_id() == person {
        app.online.person_id = person;
        apply(&mut app, &value);
        app.mark(&[SliceName::Online]);
    }
    drop(app);
    core.flush();
    Ok(())
}

pub fn online_load(core: &Core, tab: &str, force: bool) -> CmdResult<()> {
    let (has_friends, has_stats, has_inv, has_rank, has_people, scope, query) = {
        let app = lock(&core.0.app);
        let o = &app.online;
        (
            o.friends.is_some(),
            o.stats.is_some(),
            o.invitations.is_some(),
            o.ranking.is_some(),
            o.people.is_some(),
            app.ranking_scope.clone(),
            o.people_query.clone(),
        )
    };
    match tab {
        "friends" => {
            load(
                core,
                "friends",
                "loadFriends",
                has_friends && !force,
                |a| a.client_mut().friends(),
                |app, v| app.online.friends = Some(friends(v)),
            )?;
            load(
                core,
                "invitations",
                "loadInvitations",
                has_inv && !force,
                |a| a.client_mut().invitations(),
                |app, v| {
                    app.online.invitations = Some(Invitations {
                        incoming: invitation_list(v.get("przychodzace")),
                        sent: invitation_list(v.get("wyslane")),
                    });
                },
            )?;
            load(
                core,
                "stats",
                "loadStats",
                has_stats && !force,
                |a| a.client_mut().stats("tydzien"),
                |app, v| {
                    app.online.stats = Some(OnlineStats {
                        totals: period(Some(v)),
                        games: games(v.get("gry")),
                    });
                },
            )
        }
        "ranking" => {
            let s = scope.clone();
            load(
                core,
                "ranking",
                "loadRanking",
                has_rank && !force,
                move |a| a.client_mut().ranking(&s),
                move |app, v| {
                    let my = app
                        .konta
                        .get(&app.person_id())
                        .map(|k| k.state.user_id.clone())
                        .unwrap_or_default();
                    let known: HashMap<String, String> = app
                        .online
                        .friends
                        .iter()
                        .flatten()
                        .map(|f| (f.name.clone(), f.id.clone()))
                        .collect();
                    app.online.ranking = Some(ranking(v, &scope, &my, &known));
                },
            )
        }
        "people" => load(
            core,
            "people",
            "loadPeople",
            has_people && !force,
            move |a| a.client_mut().people(&query.search, query.page),
            |app, v| app.online.people = Some(people_page(v)),
        ),
        "me" => Ok(()),
        _ => Err(CmdError::unexpected("Nieznana zakładka.")),
    }
}

pub fn ranking_scope(core: &Core, scope: String) -> CmdResult<()> {
    if !account::RANKING_SCOPES.contains(&scope.as_str()) {
        return Err(CmdError::unexpected("Nieznany zakres rankingu."));
    }
    lock(&core.0.app).ranking_scope = scope;
    online_load(core, "ranking", true)
}

pub fn people_search(core: &Core, search: String, page: u32) -> CmdResult<()> {
    {
        let mut app = lock(&core.0.app);
        app.online.people_query = PeopleQuery {
            search: search.chars().take(24).collect(),
            page: page.max(1),
        };
        app.mark(&[SliceName::Online]);
    }
    online_load(core, "people", true)
}

pub fn person_open(core: &Core, user_id: String) -> CmdResult<OnlinePerson> {
    let id = user_id.clone();
    load(
        core,
        "person",
        "person",
        false,
        move |a| a.client_mut().profile(&id),
        |app, v| {
            let my = app
                .konta
                .get(&app.person_id())
                .map(|k| k.state.user_id.clone())
                .unwrap_or_default();
            app.online.person = Some(online_person(v, &my));
            app.nav("onlinePerson", "personOpened", None);
        },
    )?;
    lock(&core.0.app).online.person.clone().ok_or_else(|| {
        CmdError::new(
            ErrorCode::NotFound,
            crate::teksty::t("profil_online.brak_osoby", &[]),
        )
    })
}

fn invite(
    core: &Core,
    fetch: impl FnOnce(&mut Account) -> account::Result<Value>,
) -> CmdResult<String> {
    let person = current_person(core);
    let p = person.clone();
    let v = job(core, &person, "invite", false, move |ctx| {
        let acc = ctx.accounts.get(&p);
        let r = fetch(acc);
        acc.handle(r)
    })?;
    let who = v.get("osoba").map(|o| text(o, "name")).unwrap_or_default();
    let state = text(&v, "stan");
    lock(&core.0.app).toast(if state == "przyjeta" {
        crate::teksty::t("znajomi.przyjeto", &[("pseudonim", &who)])
    } else {
        crate::teksty::t("znajomi.zaproszono", &[("pseudonim", &who)])
    });
    core.flush();
    let _ = online_load(core, "friends", true);
    Ok(if state == "przyjeta" {
        state
    } else {
        "oczekuje".into()
    })
}

pub fn invite_name(core: &Core, name: String) -> CmdResult<String> {
    invite(core, move |a| a.client_mut().invite_by_name(&name))
}

pub fn invite_user(core: &Core, user_id: String) -> CmdResult<String> {
    invite(core, move |a| a.client_mut().invite_user(&user_id))
}

pub fn friends_action(
    core: &Core,
    action: &str,
    toast: &str,
    work: impl FnOnce(&mut Account) -> account::Result<()>,
) -> CmdResult<()> {
    let person = current_person(core);
    let p = person.clone();
    job(core, &person, action, false, move |ctx| {
        let acc = ctx.accounts.get(&p);
        let r = work(acc);
        acc.handle(r)
    })?;
    lock(&core.0.app).toast(toast);
    core.flush();
    let _ = online_load(core, "friends", true);
    Ok(())
}

pub fn registration_code(core: &Core) -> CmdResult<RegistrationCode> {
    let person = current_person(core);
    let p = person.clone();
    let v = job(core, &person, "regCode", false, move |ctx| {
        let acc = ctx.accounts.get(&p);
        let r = acc.client_mut().registration_code();
        acc.handle(r)
    })?;
    let code = RegistrationCode {
        code: text(&v, "kod"),
        valid_until: v
            .get("wazny_do_utc_s")
            .or_else(|| v.get("wazny_do"))
            .and_then(Value::as_f64),
    };
    let mut app = lock(&core.0.app);
    app.online.reg_code = Some(code.clone());
    app.mark(&[SliceName::Online]);
    drop(app);
    core.flush();
    Ok(code)
}

/// Przy starcie: kopia stanu kont i ciche odświeżenie zalogowanych (zegary w pętli).
pub fn start(core: &Core) {
    let c = core.clone();
    std::thread::spawn(move || {
        let mut ctx = lock(&c.0.konto);
        refresh_cache(&c, &mut ctx);
        drop(ctx);
        c.flush();
    });
}

/// Osoba usunięta z aplikacji - konto zostaje na serwerze, tu wylogowane.
pub fn forget_person(core: &Core, person: &str) {
    let mut ctx = lock(&core.0.konto);
    if let Err(e) = ctx.accounts.forget_person(person) {
        log::warn!("Konto: {e}");
    }
}

/// Ustawienie konta zmieniające widoczność sesji (`forget_sent` w historii).
pub fn set_visibility(core: &Core, value: String) -> CmdResult<()> {
    simple(core, "profile", None, move |acc, h| match h {
        Some(h) => acc.set_visibility(h, &value),
        None => Err(KontoError::new(
            KontoErrorKind::Io,
            &crate::teksty::t("historia.niedostepna", &[]),
            0,
        )),
    })
}
