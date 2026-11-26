//! Wysyłka sesji na konto - port `session_payload` i `Account.upload` z bieznia/konto.
//!
//! Idą tylko sesje osoby powiązanej z kontem, z bieżni (bez symulatora), nowe albo zmienione
//! od wysyłki. Bez nazw osób i profili gier - nazwa gry tylko po akceptacji regulaminu
//! z grami i tylko dla profili z włączonym „Pokazuj tę grę na koncie online”. Paczki po 100;
//! odrzucone nie wracają w kółko, ale po dobie próbujemy jeszcze raz. Sesja usunięta na innym
//! komputerze (odrzucenie z [`DELETED_ELSEWHERE`]) znika też tutaj.

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use super::{Account, GAME_NAME_MAX, REJECTED_RETRY_S, Result, canonical_uuid};
use crate::history::{History, Session};

/// Powód odrzucenia sesji, którą ktoś usunął z konta na innym komputerze.
pub const DELETED_ELSEWHERE: &str = "Sesja została usunięta na innym komputerze.";

static GAME_ID: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"^[0-9a-f]{10}$").ok());

/// Sesja w postaci, w jakiej idzie na serwer (kolejność pól jak w 0.5.x).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UploadSession {
    pub start_utc_s: f64,
    pub koniec_utc_s: f64,
    pub kroki: i64,
    pub dystans_km: f64,
    pub ruch_s: f64,
    pub bieznia_model: String,
    pub sterownik: String,
    pub bieznia_profil: String,
    pub firmware: String,
    pub wersja_aplikacji: String,
    pub zrodlo: String,
    pub strefa_min: i64,
    pub zmieniono_utc_s: f64,
    pub uuid: String,
    pub widocznosc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tryb: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gra_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gra: Option<String>,
}

/// Sesja do wysłania.
///
/// `details` = Some(true) (zaakceptowana wersja regulaminu z grami i poziomami): tryb
/// (gra/trening) i stały identyfikator gry - zawsze, bo liczą się z nich osiągnięcia za gry.
/// `game` = true: do tego czytelna nazwa gry (przełącznik „Pokazuj tę grę na koncie online”).
/// `details` = None - jak `game`.
pub fn session_payload(
    s: &Session,
    visibility: &str,
    game: bool,
    details: Option<bool>,
) -> UploadSession {
    let details = details.unwrap_or(game);
    let mut out = UploadSession {
        start_utc_s: s.start,
        koniec_utc_s: s.end,
        kroki: s.steps,
        dystans_km: s.distance_km,
        ruch_s: s.active_s,
        bieznia_model: s.device_model.clone(),
        sterownik: s.driver.clone(),
        bieznia_profil: s.device_profile.clone(),
        firmware: s.firmware.clone(),
        wersja_aplikacji: s.app_version.clone(),
        zrodlo: s.source.clone(),
        strefa_min: s.tz_offset_min,
        zmieniono_utc_s: s.updated,
        uuid: canonical_uuid(&s.id),
        widocznosc: visibility.to_string(),
        tryb: None,
        gra_id: None,
        gra: None,
    };
    if details {
        let mode = if s.mode == "trening" {
            "trening"
        } else {
            "gra"
        };
        out.tryb = Some(mode.to_string());
        if GAME_ID.as_ref().is_some_and(|re| re.is_match(&s.profile)) {
            out.gra_id = Some(s.profile.clone());
            let name = s
                .profile_name
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            if game && !name.is_empty() {
                out.gra = Some(name.chars().take(GAME_NAME_MAX).collect());
            }
        }
    }
    out
}

fn same_reason(reason: &str, expected: &str) -> bool {
    reason.trim().trim_end_matches('.') == expected.trim_end_matches('.')
}

impl Account {
    /// Nowe i zmienione sesje powiązanej osoby. Przerwana wysyłka - dokończy następna (bez
    /// internetu: błąd, nic nie jest oznaczone). `shared_games(profil_id)`: czy nazwę tej gry
    /// wolno pokazać (przełącznik przy profilu). Zwraca liczbę przyjętych sesji.
    pub fn upload(
        &mut self,
        history: &History,
        shared_games: Option<&dyn Fn(&str) -> bool>,
        now: f64,
    ) -> Result<u32> {
        let state = &self.state;
        if !(state.logged_in() && state.upload && !state.person.is_empty()) {
            return Ok(0);
        }
        let pending = history.unsent(&state.person, now)?;
        if pending.is_empty() {
            return Ok(0);
        }
        let hidden = history.hidden_online()?;
        let games = state.games_allowed() && shared_games.is_some();
        let payload: Vec<UploadSession> = pending
            .iter()
            .map(|s| {
                let visibility = if hidden.contains(&s.id) {
                    "prywatna"
                } else {
                    state.visibility.as_str()
                };
                let game = games && shared_games.is_some_and(|f| f(&s.profile));
                session_payload(s, visibility, game, Some(games))
            })
            .collect();
        let result = self.client.upload_sessions(&payload);
        let rejected = self.handle(result)?;
        let mut accepted = Vec::new();
        let mut refused: Vec<(&Session, &str)> = Vec::new();
        let mut gone = 0_u32;
        for s in &pending {
            match rejected.get(&canonical_uuid(&s.id)) {
                None => accepted.push(s.clone()),
                Some(reason) if same_reason(reason, DELETED_ELSEWHERE) => {
                    // stara kolejka z innego komputera: sesji już nie ma na koncie - nie ponawiać
                    history.delete_from_server(&s.id)?;
                    gone += 1;
                    log::info!("Konto: sesja {} usunięta na innym komputerze", short(&s.id));
                }
                Some(reason) => refused.push((s, reason.as_str())),
            }
        }
        history.mark_sent(&accepted)?;
        // odrzucone: bez pętli wysyłek, ale ponowienie za dobę (serwer mógł zmienić zasady)
        history.mark_rejected(&refused, now + REJECTED_RETRY_S)?;
        for (s, reason) in &refused {
            log::warn!("Konto: serwer odrzucił sesję {}: {reason}", short(&s.id));
        }
        log::info!(
            "Konto: wysłano {} sesji (odrzucone {}, usunięte gdzie indziej {gone})",
            pending.len(),
            refused.len()
        );
        Ok(u32::try_from(accepted.len()).unwrap_or(u32::MAX))
    }

    /// Sesja usunięta lokalnie znika też z konta (404 - serwer jej nie miał).
    pub fn delete_session(&mut self, session_id: &str) -> Result<()> {
        if self.state.logged_in() {
            let result = self.client.delete_session(&canonical_uuid(session_id));
            self.handle(result)?;
        }
        Ok(())
    }

    /// Sesje czekające na wysyłkę (informacyjnie, `AccountView.pendingUploads`).
    pub fn pending_uploads(&self, history: &History, now: f64) -> Result<u32> {
        if !(self.state.logged_in() && !self.state.person.is_empty()) {
            return Ok(0);
        }
        let n = history.unsent(&self.state.person, now)?.len();
        Ok(u32::try_from(n).unwrap_or(u32::MAX))
    }
}

fn short(id: &str) -> &str {
    id.get(..13).unwrap_or(id)
}
