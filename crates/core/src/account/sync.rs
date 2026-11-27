//! Synchronizacja sesji z serwera (docs/sync.md) i zegary konta.
//!
//! `GET /api/bieznia/sesje?kursor=…&limit=500` oddaje zmiany konta rosnąco: sesje
//! i nagrobki (usunięte). Kursor jest nieprzezroczysty, per konto (`sync_cursor`
//! w dane/konto.json), zapisywany dopiero PO zapisaniu strony w historii; `wiecej` = od
//! razu pytać dalej. Kody:
//! - 404 - starszy serwer bez tej trasy: po cichu pomijamy (log raz), dalej tylko wysyłka,
//! - 400 - zły kursor: czyścimy i pobieramy wszystko od początku,
//! - 410 - kursor starszy niż nagrobki (90 dni): pełne pobranie, potem usunięcie wysłanych
//!   sesji, których serwer już nie zwrócił (niewysłane zostają i pójdą).
//!
//! Pobrane sesje należą do osoby powiązanej z kontem; konflikt rozstrzyga `zmieniono_utc_s`
//! (serwer wygrywa tylko nowszą wersją). Pobranych nie odsyłamy. Gra z serwera: lokalny profil
//! po `gra_id`, a gdy go nie ma - nowy profil z nazwą `gra` i tym samym id
//! ([`SyncReport::new_games`]; profil zakłada aplikacja w osoby.json).
//!
//! Moduł nie ma własnych wątków ani zegarów systemowych: aplikacja co klatkę pyta
//! [`AccountTimers`], czy już czas, i woła [`Account::sync`] / [`Account::upload`] z wątku
//! roboczego.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use super::upload::{game_name, valid_game_id};
use super::{Account, KontoErrorKind, RemoteSession, Result, VISIBILITY, local_session_id};
use crate::history::{History, Session};

/// Co ile sekund pobierać zmiany (oraz od razu po zalogowaniu i przy starcie).
pub const SYNC_EVERY_S: f64 = 300.0;
/// Account server without the sync route (404): asked again after this long.
pub const MISSING_SYNC_RETRY_S: f64 = 3600.0;
/// Sesji i nagrobków na stronę (największy limit serwera).
pub const SYNC_LIMIT: u32 = 500;
/// Bezpiecznik: najwyżej tyle stron w jednej synchronizacji (500 000 zmian).
pub const MAX_PAGES: u32 = 1000;
/// Po 410: świeżo zapisanych sesji nie usuwamy (serwer oddaje zmiany z opóźnieniem ~2 s).
pub const RECENT_KEEP_S: f64 = 120.0;
/// Nazwa profilu sesji z innego komputera, gdy gra nie jest znana lokalnie.
pub const OTHER_COMPUTER_GAME: &str = "gra z innego komputera";

/// Wysyłka sesji co tyle sekund (pierwsza [`FIRST_UPLOAD_S`] po starcie).
pub const UPLOAD_EVERY_S: f64 = 120.0;
pub const FIRST_UPLOAD_S: f64 = 20.0;
/// Stan na żywo co tyle sekund.
pub const LIVE_EVERY_S: f64 = 5.0;
/// Odświeżenie tokenu i profilu (token ważny 7 dni): przy starcie i co dobę.
pub const REFRESH_EVERY_S: f64 = 24.0 * 3600.0;
/// Po wysłaniu linku: co tyle sekund sprawdzać, czy kliknięty...
pub const VERIFY_POLL_S: f64 = 15.0;
/// ...przez tyle czasu.
pub const VERIFY_POLL_FOR_S: f64 = 600.0;
/// Zakładka Konto / powrót okna: najwyżej raz na tyle sekund.
pub const VERIFY_RECHECK_S: f64 = 10.0;

/// Lokalna osoba, do której trafiają pobrane sesje.
#[derive(Debug, Clone, Copy)]
pub struct SyncTarget<'a> {
    /// nazwa osoby powiązanej z kontem (osoba_nazwa pobranych sesji)
    pub person_name: &'a str,
    /// profile gier tej osoby: id -> nazwa (gra_id z serwera znany lokalnie)
    pub profiles: &'a HashMap<String, String>,
}

/// Gra z serwera bez lokalnego profilu - aplikacja zakłada profil osoby z tym id i nazwą.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewGame {
    /// `gra_id` z serwera (id profilu z innego komputera)
    pub id: String,
    pub name: String,
    /// `gra_publiczna` - przełącznik „Pokazuj tę grę na koncie online”
    pub public: bool,
}

/// Wynik synchronizacji.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    /// pobrane strony
    pub pages: u32,
    /// sesje zapisane albo zastąpione nowszą wersją z serwera
    pub saved: u32,
    /// sesje usunięte lokalnie (nagrobki, pełne pobranie po 410)
    pub deleted: u32,
    /// pobranie od początku (bez kursora, po 400 albo 410)
    pub full: bool,
    /// serwer bez trasy synchronizacji (404) - nic nie pobrano
    pub unsupported: bool,
    /// gry z pobranych sesji, których osoba nie ma lokalnie (bez powtórzeń, ostatnia nazwa)
    pub new_games: Vec<NewGame>,
}

impl SyncReport {
    /// Historia się zmieniła - przeliczyć poziomy i osiągnięcia (bez popupów za historię
    /// z innego komputera).
    pub fn changed(&self) -> bool {
        self.saved > 0 || self.deleted > 0
    }
}

/// Sesja z serwera w postaci lokalnej (osoba z konta). Profil: lokalny po `gra_id`; nieznany
/// `gra_id` z nazwą - ten sam id i nazwa z serwera (profil założy aplikacja,
/// [`SyncReport::new_games`]); bez tego - profil tej samej sesji w historii (`local`), sama
/// nazwa `gra`, a w ostateczności „gra z innego komputera”. Tryb z serwera; pusty (serwer
/// sprzed regulaminu z grami) - tryb lokalnej sesji albo "gra".
pub fn session_from_server(
    remote: &RemoteSession,
    id: &str,
    person: &str,
    target: &SyncTarget<'_>,
    local: Option<&Session>,
) -> Session {
    let game_id = valid_game_id(&remote.game_id).then_some(remote.game_id.as_str());
    let known = game_id.and_then(|g| target.profiles.get(g));
    let name = game_name(&remote.game);
    let (profile, profile_name) = match (game_id, known, local) {
        (Some(g), Some(local_name), _) => (g.to_string(), local_name.clone()),
        (Some(g), None, _) if !name.is_empty() => (g.to_string(), name),
        (_, _, Some(l)) if !l.profile.is_empty() => (l.profile.clone(), l.profile_name.clone()),
        (Some(g), None, _) => (g.to_string(), OTHER_COMPUTER_GAME.to_string()),
        (None, _, _) if !name.is_empty() => (String::new(), name),
        (None, _, _) => (String::new(), OTHER_COMPUTER_GAME.to_string()),
    };
    let visibility = if VISIBILITY.contains(&remote.visibility.as_str()) {
        remote.visibility.clone()
    } else {
        "prywatna".into()
    };
    let mode = match (remote.mode.as_str(), local) {
        ("trening", _) => "trening",
        ("", Some(l)) if l.mode == "trening" => "trening",
        _ => "gra",
    };
    Session {
        id: id.to_string(),
        person: person.to_string(),
        person_name: target.person_name.to_string(),
        profile,
        profile_name,
        start: remote.start,
        end: remote.end,
        steps: remote.steps,
        distance_km: remote.distance_km,
        active_s: remote.active_s,
        device_model: remote.device_model.clone(),
        driver: remote.driver.clone(),
        device_profile: remote.device_profile.clone(),
        firmware: remote.firmware.clone(),
        app_version: remote.app_version.clone(),
        // na serwer idą tylko sesje z bieżni (bez symulatora)
        source: "bieznia".into(),
        visibility,
        tz_offset_min: remote.tz_offset_min,
        updated: remote.updated,
        mode: mode.into(),
    }
}

impl Account {
    /// Pobiera zmiany konta z serwera do historii (docs/sync.md). Bez logowania, bez osoby
    /// albo na serwerze bez tej trasy - nic nie robi. `now` - bieżący czas uniksowy.
    pub fn sync(
        &mut self,
        history: &History,
        target: &SyncTarget<'_>,
        now: f64,
    ) -> Result<SyncReport> {
        let mut report = SyncReport::default();
        let person = self.state.person.clone();
        if !self.enabled || !self.state.logged_in() || person.is_empty() {
            return Ok(report);
        }
        if now < self.sync_missing_until {
            report.unsupported = true;
            return Ok(report);
        }
        let mut cursor = self.state.sync_cursor.clone();
        report.full = cursor.is_empty();
        // An empty cursor in konto.json means a new login (possibly another account): what
        // was sent before may not be on this account, so it goes again and a later 410
        // does not delete it. The saved cursor also covers a restart before the first sync.
        let mut forget_sent = cursor.is_empty();
        let mut restarted = false;
        // po 410: usunąć wysłane, których serwer nie zwrócił. Do końca pełnego pobrania
        // w konto.json zostaje stary (przeterminowany) kursor - przerwane pobranie (brak
        // sieci, zamknięcie) skończy się kolejnym 410 i sprzątanie nie przepadnie.
        let mut recover = false;
        // ostatnia strona miała `wiecej = false` (serwer oddał wszystko)
        let mut complete = false;
        let mut seen: HashSet<String> = HashSet::new();
        loop {
            if report.pages >= MAX_PAGES {
                log::warn!("Konto: synchronizacja przerwana po {MAX_PAGES} stronach");
                break;
            }
            let result = self.client.changes(&cursor, SYNC_LIMIT);
            let page = match self.handle(result) {
                Ok(page) => page,
                Err(e) if e.kind == KontoErrorKind::Server && e.status == 404 => {
                    log::info!("Konto: serwer bez synchronizacji sesji - tylko wysyłka");
                    self.sync_missing_until = now + MISSING_SYNC_RETRY_S;
                    report.unsupported = true;
                    return Ok(report);
                }
                Err(e)
                    if e.kind == KontoErrorKind::Server
                        && matches!(e.status, 400 | 410)
                        && !cursor.is_empty()
                        && !restarted =>
                {
                    log::info!(
                        "Konto: kursor synchronizacji nieważny ({}) - pobieram wszystko",
                        e.status
                    );
                    restarted = true;
                    recover = e.status == 410;
                    cursor.clear();
                    if !recover {
                        self.state.sync_cursor.clear();
                        self.save()?;
                    }
                    report.full = true;
                    continue;
                }
                Err(e) => return Err(e),
            };
            if std::mem::take(&mut forget_sent) {
                history.forget_sent_of(&person)?;
            }
            report.pages += 1;
            for remote in &page.sessions {
                let Some(id) = local_session_id(&remote.uuid) else {
                    continue;
                };
                if recover {
                    seen.insert(id.clone());
                }
                let local = history.session(&id)?;
                if local.as_ref().is_some_and(|l| l.person != person) {
                    continue; // ta sama sesja u innej osoby (np. z kopii) - nie ruszamy
                }
                let s = session_from_server(remote, &id, &person, target, local.as_ref());
                // sesja ukryta na innym komputerze (prywatna mimo jawnej widoczności konta)
                // zostaje ukryta także tu - ponowna wysyłka stąd jej nie odsłoni
                let hidden = s.visibility == "prywatna" && self.state.visibility != "prywatna";
                if history.upsert_from_server(&s, hidden)? {
                    report.saved += 1;
                    let unknown = s.profile == remote.game_id
                        && !target.profiles.contains_key(&s.profile)
                        && !game_name(&remote.game).is_empty();
                    if unknown {
                        let game = NewGame {
                            id: s.profile.clone(),
                            name: s.profile_name.clone(),
                            public: remote.game_public,
                        };
                        match report.new_games.iter_mut().find(|g| g.id == game.id) {
                            Some(g) => *g = game,
                            None => report.new_games.push(game),
                        }
                    }
                }
            }
            for tomb in &page.deleted {
                let Some(id) = local_session_id(&tomb.uuid) else {
                    continue;
                };
                let local = history.session(&id)?;
                if local.as_ref().is_some_and(|l| l.person != person) {
                    continue;
                }
                if history.delete_from_server(&id)? {
                    report.deleted += 1;
                }
            }
            // kursor dopiero po zapisaniu strony; ten sam co wysłany - nic nowego
            if !recover && !page.cursor.is_empty() && page.cursor != self.state.sync_cursor {
                self.state.sync_cursor.clone_from(&page.cursor);
                self.save()?;
            }
            if !page.more {
                complete = true;
            }
            let stuck = page.cursor.is_empty() || page.cursor == cursor;
            if !page.cursor.is_empty() {
                cursor = page.cursor;
            }
            if complete || stuck {
                break;
            }
        }
        if recover {
            if !complete {
                // niepełna lista z serwera - nic nie usuwamy, następna synchronizacja
                // dostanie znów 410 i spróbuje od nowa
                log::warn!("Konto: pełne pobranie po 410 niedokończone - bez sprzątania");
                return Ok(report);
            }
            for (id, updated) in history.sent_ids(&person)? {
                if !seen.contains(&id)
                    && updated < now - RECENT_KEEP_S
                    && history.delete_from_server(&id)?
                {
                    report.deleted += 1;
                }
            }
            // sprzątanie zrobione - dopiero teraz nowy kursor (pusty przy pustym koncie)
            if cursor != self.state.sync_cursor {
                self.state.sync_cursor.clone_from(&cursor);
                self.save()?;
            }
        }
        if report.changed() {
            log::info!(
                "Konto: z serwera {} sesji, usunięte {} (stron {})",
                report.saved,
                report.deleted,
                report.pages
            );
        }
        Ok(report)
    }
}

// --- zegary ----------------------------------------------------------------------------------

/// Zadanie powtarzane co `period` sekund (czas monotoniczny aplikacji, w sekundach).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Every {
    pub period: f64,
    /// najbliższa chwila wykonania
    pub next: f64,
}

impl Every {
    pub fn new(period: f64, first_at: f64) -> Self {
        Self {
            period,
            next: first_at,
        }
    }

    /// Czy już czas - jeśli tak, następne wykonanie za `period`.
    pub fn due(&mut self, now: f64) -> bool {
        if now >= self.next {
            self.next = now + self.period;
            true
        } else {
            false
        }
    }

    /// Wykonać przy najbliższym sprawdzeniu (np. „Wyślij teraz”, po zalogowaniu).
    pub fn soon(&mut self, now: f64) {
        self.next = now;
    }

    /// Właśnie wykonane poza kolejką - następne za pełny okres.
    pub fn postpone(&mut self, now: f64) {
        self.next = now + self.period;
    }
}

/// Zegary konta (jak w 0.5.x: wysyłka co 120 s, na żywo co 5 s, odświeżenie co dobę;
/// nowość: synchronizacja co 5 min). Aplikacja sprawdza je w pętli i uruchamia zadania
/// w wątku roboczym.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccountTimers {
    pub upload: Every,
    pub sync: Every,
    pub refresh: Every,
    pub live: Every,
}

impl AccountTimers {
    /// Zegary od startu aplikacji: odświeżenie i synchronizacja od razu, pierwsza wysyłka
    /// chwilę po starcie.
    pub fn new(now: f64) -> Self {
        Self {
            upload: Every::new(UPLOAD_EVERY_S, now + FIRST_UPLOAD_S),
            sync: Every::new(SYNC_EVERY_S, now),
            refresh: Every::new(REFRESH_EVERY_S, now),
            live: Every::new(LIVE_EVERY_S, now),
        }
    }

    /// Po zalogowaniu: wyniki od razu na konto i od razu pobranie z innych komputerów.
    pub fn signed_in(&mut self, now: f64) {
        self.upload.soon(now);
        self.sync.soon(now);
    }
}

/// Od tej prędkości (km/h) w trakcie sesji osoba „idzie” w stanie na żywo.
pub const LIVE_MIN_KMH: f64 = 0.05;

/// Gra bieżącej sesji w stanie na żywo (profil gry; tryb Trening - bez gry).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LiveGame {
    /// id profilu (10 znaków hex)
    pub id: String,
    pub name: String,
    /// przełącznik „Pokazuj tę grę na koncie online”
    pub public: bool,
}

/// Migawka stanu na żywo (co [`LIVE_EVERY_S`] pełny komplet, docs/sync.md).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LiveWalk {
    pub kmh: f64,
    pub steps: i64,
    pub active_s: f64,
    pub session_id: String,
    /// dystans BIEŻĄCEJ sesji (km)
    pub distance_km: f64,
    /// "gra" albo "trening"
    pub mode: String,
    pub game: Option<LiveGame>,
}

/// Stan na żywo do wysłania przy kolejnym tyknięciu zegara [`LIVE_EVERY_S`].
#[derive(Debug, Clone, PartialEq)]
pub enum LiveUpdate {
    Walking(LiveWalk),
    /// koniec ruchu - wyczyścić stan na serwerze (raz)
    Stopped,
}

/// Co wysłać teraz: ruch, gdy trwa sesja (`walk` - None bez sesji) i prędkość >
/// [`LIVE_MIN_KMH`]; po ruchu jeden raz [`LiveUpdate::Stopped`]; potem nic. `live_on` -
/// ostatnio wysłano ruch.
pub fn live_update(live_on: &mut bool, walk: Option<LiveWalk>) -> Option<LiveUpdate> {
    match walk {
        Some(walk) if walk.kmh > LIVE_MIN_KMH => {
            *live_on = true;
            Some(LiveUpdate::Walking(walk))
        }
        _ if *live_on => {
            *live_on = false;
            Some(LiveUpdate::Stopped)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timers_fire_at_start_after_login_and_periodically() {
        let mut t = AccountTimers::new(1000.0);
        assert!(t.refresh.due(1000.0) && t.sync.due(1000.0));
        assert!(!t.upload.due(1000.0)); // pierwsza wysyłka po 20 s
        assert!(t.upload.due(1020.0));
        assert!(!t.sync.due(1299.0) && t.sync.due(1300.0)); // co 5 min
        assert!(!t.refresh.due(1300.0));
        t.signed_in(1301.0);
        assert!(t.upload.due(1301.0) && t.sync.due(1301.0));
        assert!(!t.upload.due(1302.0));
        t.upload.postpone(1302.0);
        assert!(!t.upload.due(1421.0) && t.upload.due(1422.0));
        assert!(t.live.due(1000.0) && !t.live.due(1004.0) && t.live.due(1005.0));
    }
}
