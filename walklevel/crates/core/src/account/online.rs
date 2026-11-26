//! Dane online z serwera (statystyki, znajomi, zaproszenia, ranking, ludzie, profil osoby,
//! kod rejestracji) jako typy wycinka `online` (docs/kontrakt.md, nazwy pól camelCase).
//!
//! Odpowiedzi serwera czytamy pobłażliwie: brak pola albo inny typ - wartość domyślna
//! (0, pusty tekst, `None`), nigdy błąd. Poziom spoza 1..=100 albo brak - `None` (interfejs
//! bierze wtedy poziom z aplikacji).

use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodTotals {
    pub steps: i64,
    pub distance_km: f64,
    pub active_s: f64,
    pub sessions: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameTotals {
    pub game: String,
    pub game_id: Option<String>,
    pub steps: i64,
    pub distance_km: f64,
    pub active_s: f64,
}

/// Pozostałe gry profilu osoby (`inne_gry`: bez nazw - wiersz „Inne gry (N)”).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OtherGames {
    pub steps: i64,
    pub distance_km: f64,
    pub active_s: f64,
    pub sessions: Option<i64>,
    /// ile gier
    pub games: u32,
}

/// Ile procent kont ma każde osiągnięcie (`GET osiagniecia/statystyki`). Poniżej progu kont
/// serwer nie podaje procentów (`accounts` = `None`, pusta mapa).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AchievementStats {
    pub accounts: Option<u32>,
    /// id osiągnięcia -> procent kont (0-100)
    pub percent: HashMap<String, f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineStats {
    #[serde(flatten)]
    pub totals: PeriodTotals,
    pub games: Option<Vec<GameTotals>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveState {
    pub kmh: f64,
    pub steps: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Friend {
    pub id: String,
    pub name: String,
    pub level: Option<u32>,
    pub live: Option<LiveState>,
    pub week: PeriodTotals,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvitationPerson {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Invitation {
    pub id: String,
    pub person: InvitationPerson,
    pub created_at: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Invitations {
    pub incoming: Vec<Invitation>,
    pub sent: Vec<Invitation>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RankingRow {
    pub place: u32,
    pub id: String,
    pub name: String,
    pub me: bool,
    pub level: Option<u32>,
    pub steps: i64,
    pub distance_km: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ranking {
    pub scope: String,
    pub rows: Vec<RankingRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeopleItem {
    pub id: String,
    pub name: String,
    pub level: Option<u32>,
    pub week_steps: i64,
    pub place: Option<u32>,
    pub friend: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeoplePage {
    pub page: u32,
    pub has_next: bool,
    pub people: Vec<PeopleItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EarnedOnline {
    pub id: String,
    pub earned_at: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlinePerson {
    pub id: String,
    pub name: String,
    pub me: bool,
    pub level: Option<u32>,
    pub xp: Option<i64>,
    pub achievements_count: Option<u32>,
    pub achievements: Option<Vec<EarnedOnline>>,
    pub since: Option<f64>,
    pub treadmill_model: Option<String>,
    pub friend: bool,
    pub week: Option<PeriodTotals>,
    pub total: Option<PeriodTotals>,
    pub games: Option<Vec<GameTotals>>,
    /// gry poza listą `games` (bez nazw); `None` - brak
    pub other_games: Option<OtherGames>,
    pub games_hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrationCode {
    pub code: String,
    pub valid_until: Option<f64>,
}

/// Odpowiedź na zaproszenie: `accepted` - zaproszenie z drugiej strony już czekało i od razu
/// jesteście znajomymi; `name` - pseudonim zaproszonej osoby.
#[derive(Debug, Clone, PartialEq)]
pub struct InviteAnswer {
    pub accepted: bool,
    pub name: String,
}

impl InviteAnswer {
    /// Stan dla interfejsu: "przyjeta" albo "oczekuje".
    pub fn state(&self) -> &'static str {
        if self.accepted {
            "przyjeta"
        } else {
            "oczekuje"
        }
    }
}

// Ściślejsze niż w api.rs: tekst tylko z napisu albo liczby, liczba tylko z liczby
// (tak czytał widok online od początku - inne typy dają wartość domyślną).

fn num(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

fn int(v: &Value, key: &str) -> i64 {
    num(v, key) as i64
}

fn text(v: &Value, key: &str) -> String {
    match v.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn flag(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn list(v: Option<&Value>) -> &[Value] {
    v.and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

fn level(v: &Value) -> Option<u32> {
    v.get("poziom")
        .and_then(Value::as_f64)
        .filter(|l| *l >= 1.0)
        .map(|l| (l as u32).min(100))
}

/// Sumy okresu (`kroki`, `dystans_km`, `ruch_s`, `sesje`); nie-obiekt - same zera.
pub fn period(v: Option<&Value>) -> PeriodTotals {
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

/// Podział na gry; nie-lista (także `null` - podział ukryty) - `None`. Nazwa gry: `gra`
/// (statystyki) albo `nazwa` (profil osoby).
pub fn games(v: Option<&Value>) -> Option<Vec<GameTotals>> {
    let list = v?.as_array()?;
    Some(
        list.iter()
            .filter(|g| g.is_object())
            .map(|g| GameTotals {
                game: Some(text(g, "gra"))
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| text(g, "nazwa"))
                    .trim()
                    .to_string(),
                game_id: Some(text(g, "gra_id")).filter(|s| !s.is_empty()),
                steps: int(g, "kroki"),
                distance_km: num(g, "dystans_km"),
                active_s: num(g, "ruch_s"),
            })
            .collect(),
    )
}

/// Pozostałe gry profilu (`inne_gry`); nie-obiekt albo zero gier - `None`.
pub fn other_games(v: Option<&Value>) -> Option<OtherGames> {
    let v = v.filter(|v| v.is_object())?;
    let totals = period(Some(v));
    let games = int(v, "gier").max(0) as u32;
    (games > 0).then_some(OtherGames {
        steps: totals.steps,
        distance_km: totals.distance_km,
        active_s: totals.active_s,
        sessions: totals.sessions,
        games,
    })
}

/// Procent kont z każdym osiągnięciem; liczby spoza 0-100 i inne typy pomijamy.
pub fn achievement_stats(v: &Value) -> AchievementStats {
    let percent = v
        .get("procent")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(id, p)| {
                    p.as_f64()
                        .filter(|p| (0.0..=100.0).contains(p))
                        .map(|p| (id.clone(), p))
                })
                .collect()
        })
        .unwrap_or_default();
    AchievementStats {
        accounts: v.get("kont").and_then(Value::as_f64).map(|n| n as u32),
        percent,
    }
}

/// Statystyki zalogowanej osoby (`GET statystyki`).
pub fn stats(v: &Value) -> OnlineStats {
    OnlineStats {
        totals: period(Some(v)),
        games: games(v.get("gry")),
    }
}

/// Lista znajomych.
pub fn friends(v: &Value) -> Vec<Friend> {
    list(Some(v))
        .iter()
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
}

fn invitation_list(v: Option<&Value>) -> Vec<Invitation> {
    list(v)
        .iter()
        .map(|i| {
            let who = i.get("osoba").unwrap_or(&Value::Null);
            let mut name = text(who, "name");
            if name.is_empty() {
                name = text(i, "name");
            }
            Invitation {
                id: text(i, "id"),
                person: InvitationPerson {
                    id: text(who, "id"),
                    name: if name.is_empty() { "?".into() } else { name },
                },
                created_at: i.get("utworzono_utc_s").and_then(Value::as_f64),
            }
        })
        .collect()
}

/// Zaproszenia przychodzące i wysłane.
pub fn invitations(v: &Value) -> Invitations {
    Invitations {
        incoming: invitation_list(v.get("przychodzace")),
        sent: invitation_list(v.get("wyslane")),
    }
}

/// Ranking w zakresie `scope`. Własny wiersz (`ja`) dostaje `my_id`; serwer nie odsyła id
/// pozostałych - znajomych rozpoznajemy po pseudonimie.
pub fn ranking(v: &Value, scope: &str, my_id: &str, friends: &[Friend]) -> Ranking {
    let known: HashMap<&str, &str> = friends
        .iter()
        .map(|f| (f.name.as_str(), f.id.as_str()))
        .collect();
    let rows = list(Some(v))
        .iter()
        .map(|r| {
            let me = flag(r, "ja");
            let name = text(r, "name");
            let mut id = if me { my_id.to_string() } else { text(r, "id") };
            if id.is_empty() {
                id = known
                    .get(name.as_str())
                    .map_or_else(String::new, |s| (*s).to_string());
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
        .collect();
    Ranking {
        scope: scope.into(),
        rows,
    }
}

/// Strona wyszukiwania ludzi.
pub fn people_page(v: &Value) -> PeoplePage {
    PeoplePage {
        page: v.get("strona").and_then(Value::as_u64).unwrap_or(1) as u32,
        has_next: flag(v, "nastepna"),
        people: list(v.get("osoby"))
            .iter()
            .map(|p| PeopleItem {
                id: text(p, "id"),
                name: text(p, "name"),
                level: level(p),
                week_steps: p.get("tydzien").map_or(0, |w| int(w, "kroki")),
                place: p.get("miejsce").and_then(Value::as_u64).map(|m| m as u32),
                friend: flag(p, "znajomy"),
            })
            .collect(),
    }
}

/// Profil osoby online; `me` - to profil zalogowanego użytkownika `my_id`.
pub fn person(v: &Value, my_id: &str) -> OnlinePerson {
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
    let totals = |key| {
        v.get(key)
            .filter(|t| t.is_object())
            .map(|t| period(Some(t)))
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
        friend: flag(v, "znajomy"),
        week: totals("tydzien"),
        total: totals("razem"),
        games: games(v.get("gry")),
        other_games: other_games(v.get("inne_gry")),
        games_hidden: v.get("gry").is_some_and(Value::is_null),
        id,
    }
}

/// Kod rejestracji dla znajomego (`wazny_do_utc_s`, starszy serwer: `wazny_do`).
pub fn registration_code(v: &Value) -> RegistrationCode {
    RegistrationCode {
        code: text(v, "kod"),
        valid_until: v
            .get("wazny_do_utc_s")
            .or_else(|| v.get("wazny_do"))
            .and_then(Value::as_f64),
    }
}

/// Odpowiedź serwera na zaproszenie (po pseudonimie albo z profilu).
pub fn invite_answer(v: &Value) -> InviteAnswer {
    InviteAnswer {
        accepted: text(v, "stan") == "przyjeta",
        name: v.get("osoba").map(|o| text(o, "name")).unwrap_or_default(),
    }
}
