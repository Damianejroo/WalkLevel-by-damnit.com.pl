//! Konto online (bieznia/gui/ekrany/konto.py): zadania sieciowe poza pętlą i poza blokadą
//! stanu aplikacji, zegary wysyłki / synchronizacji (docs/synchro.md, co 5 min) / odświeżenia /
//! stanu na żywo, sprawdzanie potwierdzenia adresu e-mail, dane online do wycinka `online`.
//!
//! Kolejność blokad: najpierw `konto`, potem `app` - nigdy odwrotnie. Pętla blokuje tylko
//! `app`, więc zadanie czekające na serwer (do 15 s) jej nie zatrzymuje.

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;
use walklevel_core::account::{
    self, Account, Accounts, KontoError, KontoErrorKind, LiveUpdate, SyncTarget, online,
};
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

fn history_missing() -> KontoError {
    KontoError::new(
        KontoErrorKind::Io,
        &crate::teksty::t("historia.niedostepna", &[]),
        0,
    )
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
            let steps = stats.steps.unwrap_or(0);
            live = account::live_update(&mut app.live_on, current, speed, steps, stats.active_s);
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
                LiveUpdate::Walking {
                    kmh,
                    steps,
                    active_s,
                    session_id,
                } => acc.live(kmh, steps, active_s, &session_id),
                LiveUpdate::Stopped => acc.clear_live(),
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
        if !report.new_games.is_empty() {
            lock(&c.0.app).synced_profiles(&p, &report.new_games);
        }
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
    if let Some(taken) = ctx.accounts.taken_by(user_id, person) {
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
            let color = walklevel_core::store::palette::next_for(store.people().len()).name;
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
    simple(core, "terms", None, |acc, _| acc.accept_terms())?;
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

/// Id użytkownika konta wybranej osoby ("" - niezalogowana).
fn my_user_id(app: &App) -> String {
    app.konta
        .get(&app.person_id())
        .map(|k| k.state.user_id.clone())
        .unwrap_or_default()
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

/// Statystyki osiągnięć zmieniają się wolno - pobranie najwyżej raz na tyle sekund.
const ACHIEVEMENT_STATS_EVERY_S: f64 = 24.0 * 3600.0;

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
                |app, v| app.online.friends = Some(online::friends(v)),
            )?;
            load(
                core,
                "invitations",
                "loadInvitations",
                has_inv && !force,
                |a| a.client_mut().invitations(),
                |app, v| app.online.invitations = Some(online::invitations(v)),
            )?;
            load(
                core,
                "stats",
                "loadStats",
                has_stats && !force,
                |a| a.client_mut().stats("tydzien"),
                |app, v| app.online.stats = Some(online::stats(v)),
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
                    let friends = app.online.friends.as_deref().unwrap_or_default();
                    let ranking = online::ranking(v, &scope, &my_user_id(app), friends);
                    app.online.ranking = Some(ranking);
                },
            )
        }
        "people" => load(
            core,
            "people",
            "loadPeople",
            has_people && !force,
            move |a| a.client_mut().people(&query.search, query.page),
            |app, v| app.online.people = Some(online::people_page(v)),
        ),
        "achievements" => {
            // ekran Osiągnięcia: procent graczy z osiągnięciem - najwyżej raz na dobę
            let now = now_unix();
            let fresh = {
                let app = lock(&core.0.app);
                app.online.achievement_stats.is_some()
                    && now - app.achievement_stats_at < ACHIEVEMENT_STATS_EVERY_S
            };
            if fresh {
                return Ok(());
            }
            load(
                core,
                "achievementStats",
                "loadAchievementStats",
                true,
                |a| a.client_mut().achievement_stats(),
                move |app, v| {
                    app.online.achievement_stats = Some(online::achievement_stats(v));
                    app.achievement_stats_at = now;
                },
            )
        }
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
            app.online.person = Some(online::person(v, &my_user_id(app)));
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
    let answer = online::invite_answer(&v);
    let key = if answer.accepted {
        "znajomi.przyjeto"
    } else {
        "znajomi.zaproszono"
    };
    lock(&core.0.app).toast(crate::teksty::t(key, &[("pseudonim", &answer.name)]));
    core.flush();
    let _ = online_load(core, "friends", true);
    Ok(answer.state().into())
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
    let code = online::registration_code(&v);
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
        Some(h) => acc.set_visibility(h, &value, now_unix()),
        None => Err(history_missing()),
    })
}
