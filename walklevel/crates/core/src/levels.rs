//! Poziomy, rangi i osiągnięcia WalkLevel liczone z historii sesji - port bieznia/poziomy.
//!
//! Definicje (progi, teksty, tabela 100 poziomów) są w `assets/definicje.json` - od 0.6.0
//! wersja 2 (docs/definicje-v2.json, docs/analiza-szlak.md), format pliku ten sam co w 0.5.x,
//! wspólny dla aplikacji i serwera. Tu tylko liczenie: czyste funkcje bez interfejsu i bez bazy.
//!
//! Zasady (te same co na serwerze):
//! - liczą się sesje z bieżni, bez symulatora;
//! - dzień sesji to lokalny dzień jej POCZĄTKU (strefa zapisana przy sesji);
//! - seria: dzień liczy się od 2 km, jeden dzień wolny na 7 nie przerywa serii;
//! - XP = 100 za km + mała premia za każde zdobyte osiągnięcie;
//! - raz zdobytego osiągnięcia nie odbieramy (pilnuje tego zapis w historii, nie ta funkcja).
//!
//! Użycie: `let defs = Definitions::builtin()?; let walks = walks_from_sessions(&sesje);
//! let status = evaluate(defs, &walks, &EvalOptions::default());`

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::model::Session;

/// Definicje wbudowane w program (wersja 2, kopia docs/definicje-v2.json).
pub const BUILTIN_DEFINITIONS: &str = include_str!("../assets/definicje.json");

/// Sekundy w dobie.
pub const DAY_S: f64 = 86400.0;
/// Tryb sesji: gra (liczy się do osiągnięć gier).
pub const GAME_MODE: &str = "gra";
/// Tryb sesji: trening (bez gry).
pub const TRAINING_MODE: &str = "trening";

/// Średnio szybciej to nie chód na bieżni (km/h).
pub const MAX_KMH: f64 = 20.0;
/// Najwięcej kroków na minutę.
pub const MAX_STEPS_PER_MIN: f64 = 250.0;
/// Rozsądna długość kroku w metrach (od, do).
pub const STEP_M: (f64, f64) = (0.3, 1.3);
/// Ponad 8 h ruchu w dniu lokalnym nic się nie liczy.
pub const DAY_MOTION_CAP_S: f64 = 8.0 * 3600.0;

/// Tolerancja progu (sumy kilometrów typu 10 x 0,1).
const EPS: f64 = 1e-9;

/// Błąd wczytania definicji poziomów.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum LevelsError {
    #[error("nie udało się odczytać definicji poziomów {path}: {message}")]
    Read { path: String, message: String },
    #[error("błędny plik definicji poziomów: {0}")]
    Parse(String),
    #[error("niepoprawne definicje poziomów: {0}")]
    Invalid(String),
}

// --- definicje ---------------------------------------------------------------------------------

/// Osiągnięcie z definicji. `params` to pozostałe pola z JSON (np. `km`, `dni`, `od`, `do`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Achievement {
    pub id: String,
    /// dystans, sesja_km, dzien_kroki, seria, gra_pierwsza, gry_km, gra_km, trening,
    /// start_miedzy, znajomi, polecenia (nieznany typ: zawsze bez postępu)
    pub kind: String,
    pub rarity: String,
    /// znak na odznace (rysowany w interfejsie)
    pub mark: String,
    pub name: String,
    /// klimat: zdanie ze świata gry albo książki
    pub flavor: String,
    /// suchy warunek
    pub condition: String,
    pub params: Map<String, Value>,
}

impl Achievement {
    /// Próg szlaku (dystans łącznie) - tylko dla osiągnięć typu `dystans`.
    pub fn km(&self) -> Option<f64> {
        if self.kind == "dystans" {
            self.param("km")
        } else {
            None
        }
    }

    /// Parametr liczbowy z definicji.
    pub fn param(&self, key: &str) -> Option<f64> {
        self.params.get(key).and_then(Value::as_f64)
    }

    fn param_or(&self, key: &str, default: f64) -> f64 {
        self.param(key).unwrap_or(default)
    }
}

/// Ranga - nazwa gracza od danego poziomu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rank {
    /// od którego poziomu
    pub level: u32,
    pub name: String,
    pub about: String,
}

/// Wczytane definicje poziomów, rang i osiągnięć.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Definitions {
    pub version: i64,
    pub description: String,
    pub xp_per_km: i64,
    /// premia XP za zdobyte osiągnięcie danej rzadkości
    pub rarity_xp: BTreeMap<String, i64>,
    /// rzadkości od najczęstszej
    pub rarities: Vec<String>,
    /// XP potrzebne do poziomu n (indeks n - 1), rosnąco
    pub levels: Vec<i64>,
    pub ranks: Vec<Rank>,
    pub achievements: Vec<Achievement>,
}

// Liczby całkowite czytane jako f64 i obcinane jak `int()` w Pythonie - plik z `2.0`
// zamiast `2` wczytuje się tak samo jak w 0.5.x.
#[derive(Deserialize)]
struct RawDefinitions {
    wersja: f64,
    #[serde(default)]
    opis: String,
    xp_za_km: f64,
    xp_za_rzadkosc: BTreeMap<String, f64>,
    rzadkosci: Vec<String>,
    poziomy_xp: Vec<f64>,
    rangi: Vec<RawRank>,
    osiagniecia: Vec<RawAchievement>,
}

#[derive(Deserialize)]
struct RawRank {
    od_poziomu: f64,
    nazwa: String,
    opis: String,
}

#[derive(Deserialize)]
struct RawAchievement {
    id: String,
    typ: String,
    rzadkosc: String,
    znak: String,
    nazwa: String,
    klimat: String,
    warunek: String,
    #[serde(flatten)]
    params: Map<String, Value>,
}

/// Wymagane parametry liczbowe znanych typów osiągnięć.
fn required_params(kind: &str) -> &'static [&'static str] {
    match kind {
        "dystans" | "sesja_km" | "gra_km" => &["km"],
        "dzien_kroki" => &["kroki"],
        "seria" => &["dni"],
        "gry_km" => &["gier", "km"],
        "start_miedzy" => &["od", "do"],
        "znajomi" | "polecenia" => &["ile"],
        _ => &[],
    }
}

impl Definitions {
    /// Definicje wbudowane w program (wczytane raz).
    pub fn builtin() -> Result<&'static Definitions, LevelsError> {
        static BUILTIN: OnceLock<Result<Definitions, LevelsError>> = OnceLock::new();
        BUILTIN
            .get_or_init(|| Definitions::from_json(BUILTIN_DEFINITIONS))
            .as_ref()
            .map_err(Clone::clone)
    }

    /// Definicje z innego pliku (np. nowsza wersja pobrana z serwera).
    pub fn load_file(path: &Path) -> Result<Definitions, LevelsError> {
        let text = std::fs::read_to_string(path).map_err(|e| LevelsError::Read {
            path: path.display().to_string(),
            message: e.to_string(),
        })?;
        Definitions::from_json(&text)
    }

    /// Definicje z tekstu JSON w formacie definicje.json. Nieznane pola są pomijane,
    /// a dodatkowe pola osiągnięć trafiają do `params` - nowsza wersja pliku się wczyta.
    pub fn from_json(text: &str) -> Result<Definitions, LevelsError> {
        let raw: RawDefinitions =
            serde_json::from_str(text).map_err(|e| LevelsError::Parse(e.to_string()))?;
        let levels: Vec<i64> = raw.poziomy_xp.iter().map(|&x| x as i64).collect();
        let xp_per_km = raw.xp_za_km as i64;
        if levels.is_empty() {
            return Err(LevelsError::Invalid("pusta tabela poziomów".into()));
        }
        if levels.windows(2).any(|w| w[0] > w[1]) {
            return Err(LevelsError::Invalid("progi poziomów nie rosną".into()));
        }
        if raw.rangi.is_empty() {
            return Err(LevelsError::Invalid("brak rang".into()));
        }
        if xp_per_km <= 0 {
            return Err(LevelsError::Invalid("xp_za_km musi być dodatnie".into()));
        }
        let mut seen = std::collections::HashSet::new();
        for a in &raw.osiagniecia {
            if !seen.insert(a.id.as_str()) {
                return Err(LevelsError::Invalid(format!(
                    "powtórzone osiągnięcie {}",
                    a.id
                )));
            }
            for key in required_params(&a.typ) {
                if a.params.get(*key).and_then(Value::as_f64).is_none() {
                    return Err(LevelsError::Invalid(format!(
                        "osiągnięcie {} bez liczby w polu {key}",
                        a.id
                    )));
                }
            }
        }
        Ok(Definitions {
            version: raw.wersja as i64,
            description: raw.opis,
            xp_per_km,
            rarity_xp: raw
                .xp_za_rzadkosc
                .into_iter()
                .map(|(k, v)| (k, v as i64))
                .collect(),
            rarities: raw.rzadkosci,
            levels,
            ranks: raw
                .rangi
                .into_iter()
                .map(|r| Rank {
                    // ujemny próg działa w rank/next_rank tak samo jak 0
                    level: r.od_poziomu as u32,
                    name: r.nazwa,
                    about: r.opis,
                })
                .collect(),
            achievements: raw
                .osiagniecia
                .into_iter()
                .map(|a| Achievement {
                    id: a.id,
                    kind: a.typ,
                    rarity: a.rzadkosc,
                    mark: a.znak,
                    name: a.nazwa,
                    flavor: a.klimat,
                    condition: a.warunek,
                    params: a.params,
                })
                .collect(),
        })
    }

    /// Osiągnięcie o danym id.
    pub fn by_id(&self, id: &str) -> Option<&Achievement> {
        self.achievements.iter().find(|a| a.id == id)
    }

    /// Przystanki szlaku (osiągnięcia typu dystans) od najbliższego.
    pub fn trail(&self) -> Vec<&Achievement> {
        let mut out: Vec<&Achievement> = self
            .achievements
            .iter()
            .filter(|a| a.km().is_some())
            .collect();
        out.sort_by(|a, b| cmp_f64(a.km().unwrap_or(0.0), b.km().unwrap_or(0.0)));
        out
    }

    /// Najwyższy poziom.
    pub fn max_level(&self) -> u32 {
        u32::try_from(self.levels.len()).unwrap_or(u32::MAX)
    }

    /// Poziom dla danego XP (co najmniej 1).
    pub fn level_of(&self, xp: f64) -> u32 {
        let reached = self
            .levels
            .iter()
            .filter(|&&need| need as f64 <= xp)
            .count();
        u32::try_from(reached).unwrap_or(u32::MAX).max(1)
    }

    /// XP potrzebne do poziomu (poziom przycinany do 1..=max).
    pub fn xp_for(&self, level: u32) -> i64 {
        let level = level.min(self.max_level()).max(1) as usize;
        self.levels.get(level - 1).copied().unwrap_or(0)
    }

    /// Ranga na danym poziomie.
    pub fn rank(&self, level: u32) -> Rank {
        let mut current = self.ranks.first();
        for r in &self.ranks {
            if r.level <= level {
                current = Some(r);
            }
        }
        current.cloned().unwrap_or(Rank {
            level: 1,
            name: String::new(),
            about: String::new(),
        })
    }

    /// Następna ranga powyżej poziomu (None - już najwyższa).
    pub fn next_rank(&self, level: u32) -> Option<Rank> {
        self.ranks.iter().find(|r| r.level > level).cloned()
    }

    /// Najmniej km dziennie, żeby dzień liczył się do serii.
    fn min_km_streak_day(&self) -> f64 {
        self.achievements
            .iter()
            .filter(|a| a.kind == "seria")
            .map(|a| a.param_or("km_dziennie", 2.0))
            .reduce(f64::max)
            .unwrap_or(2.0)
    }
}

// --- sesje -> przejścia --------------------------------------------------------------------------

/// To, czego potrzebuje liczenie, z jednej sesji historii.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Walk {
    /// czasy uniksowe UTC
    pub start: f64,
    pub end: f64,
    pub km: f64,
    pub steps: i64,
    /// identyfikator profilu (gry)
    pub game: String,
    /// "gra" albo "trening"
    pub mode: String,
    pub tz_offset_min: i64,
}

impl Walk {
    /// Lokalny dzień początku sesji (liczba dni od 1970-01-01).
    pub fn day(&self) -> i64 {
        local_day(self.start, self.tz_offset_min)
    }

    /// Lokalna godzina początku sesji (0-23).
    pub fn hour(&self) -> i64 {
        let local = self.start + (self.tz_offset_min * 60) as f64;
        py_floor_div(py_mod(local, DAY_S), 3600.0) as i64
    }
}

fn local_day(t: f64, tz_offset_min: i64) -> i64 {
    py_floor_div(t + (tz_offset_min * 60) as f64, DAY_S) as i64
}

/// Dzielenie całkowite liczb zmiennoprzecinkowych jak `//` w Pythonie (ten sam wynik bit w bit).
fn py_floor_div(a: f64, b: f64) -> f64 {
    let m = a % b;
    let mut div = (a - m) / b;
    if m != 0.0 && ((b < 0.0) != (m < 0.0)) {
        div -= 1.0;
    }
    if div != 0.0 {
        let floor = div.floor();
        if div - floor > 0.5 {
            floor + 1.0
        } else {
            floor
        }
    } else {
        0.0_f64.copysign(a / b)
    }
}

/// Reszta z dzielenia jak `%` w Pythonie (znak dzielnika).
fn py_mod(a: f64, b: f64) -> f64 {
    let m = a % b;
    if m != 0.0 {
        if (b < 0.0) != (m < 0.0) { m + b } else { m }
    } else {
        0.0_f64.copysign(b)
    }
}

fn cmp_f64(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}

/// Te same zasady co na serwerze: z bieżni, z ruchem, w ludzkim tempie. Bieżnia, która nie
/// liczy kroków (0), nie jest sprawdzana pod kątem kroków.
pub fn reliable(s: &Session) -> bool {
    if s.source != "bieznia" {
        return false;
    }
    let (active, km, steps) = (s.active_s, s.distance_km, s.steps);
    if active <= 0.0 || km < 0.0 {
        return false;
    }
    if km / (active / 3600.0) > MAX_KMH {
        return false;
    }
    if steps > 0 {
        if steps as f64 / (active / 60.0) > MAX_STEPS_PER_MIN {
            return false;
        }
        let step = km * 1000.0 / steps as f64;
        if km > 0.0 && !(STEP_M.0 <= step && step <= STEP_M.1) {
            return false;
        }
    }
    true
}

/// Sesje historii -> lista do liczenia, tak samo jak na serwerze: tylko wiarygodne sesje
/// z bieżni, sesja zaczynająca się przed końcem poprzedniej jest pomijana, a ruch ponad 8 h
/// w dniu lokalnym nie daje dystansu ani kroków (proporcjonalnie). Wynik od najwcześniej
/// zakończonej.
pub fn walks_from_sessions<'a>(sessions: impl IntoIterator<Item = &'a Session>) -> Vec<Walk> {
    let mut ordered: Vec<&Session> = sessions.into_iter().filter(|s| reliable(s)).collect();
    ordered.sort_by(|a, b| cmp_f64(a.start, b.start).then(cmp_f64(a.end, b.end)));
    let mut out = Vec::with_capacity(ordered.len());
    let mut last_end = f64::NEG_INFINITY;
    let mut used: HashMap<i64, f64> = HashMap::new();
    for s in ordered {
        if s.start < last_end {
            continue;
        }
        last_end = s.end;
        let tz = s.tz_offset_min;
        let used_day = used.entry(local_day(s.start, tz)).or_insert(0.0);
        let allowed = (DAY_MOTION_CAP_S - *used_day).max(0.0);
        let active = s.active_s;
        let share = if active <= allowed {
            1.0
        } else {
            allowed / active
        };
        *used_day += active;
        out.push(Walk {
            start: s.start,
            end: s.end,
            km: s.distance_km.max(0.0) * share,
            steps: (s.steps.max(0) as f64 * share) as i64,
            game: s.profile.clone(),
            mode: if s.mode.is_empty() {
                GAME_MODE.to_string()
            } else {
                s.mode.clone()
            },
            tz_offset_min: tz,
        });
    }
    out.sort_by(|a, b| cmp_f64(a.end, b.end).then(cmp_f64(a.start, b.start)));
    out
}

// --- liczenie ---------------------------------------------------------------------------------

/// Postęp jednego osiągnięcia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub achievement: Achievement,
    /// koniec sesji, w której padł próg (UTC); None - jeszcze nie zdobyte
    pub earned_at: Option<f64>,
    pub current: f64,
    pub target: f64,
}

impl Progress {
    /// Postęp bez zdobycia (cel 1).
    pub fn new(achievement: Achievement) -> Progress {
        Progress {
            achievement,
            earned_at: None,
            current: 0.0,
            target: 1.0,
        }
    }

    /// Czy osiągnięcie jest zdobyte.
    pub fn earned(&self) -> bool {
        self.earned_at.is_some()
    }

    /// Ułamek drogi do celu 0..1 (zdobyte: 1).
    pub fn fraction(&self) -> f64 {
        if self.earned() {
            1.0
        } else if self.target > 0.0 {
            (self.current / self.target).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

/// Stan gracza policzony z jego sesji.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// km łącznie (po regułach wiarygodności)
    pub km: f64,
    pub xp: i64,
    pub level: u32,
    pub rank: Rank,
    pub next_rank: Option<Rank>,
    /// XP początku bieżącego poziomu
    pub level_floor: i64,
    /// XP następnego poziomu (na najwyższym: równe level_floor)
    pub level_ceiling: i64,
    /// ułamek drogi do następnego poziomu 0..1
    pub level_fraction: f64,
    pub km_to_next_level: f64,
    /// postęp wszystkich osiągnięć w kolejności definicji
    pub progress: Vec<Progress>,
}

impl Status {
    /// Postęp osiągnięcia o danym id.
    pub fn get(&self, id: &str) -> Option<&Progress> {
        self.progress.iter().find(|p| p.achievement.id == id)
    }

    /// Zdobyte, od najnowszego.
    pub fn earned(&self) -> Vec<&Progress> {
        let mut got: Vec<&Progress> = self.progress.iter().filter(|p| p.earned()).collect();
        got.sort_by(|a, b| cmp_f64(b.earned_at.unwrap_or(0.0), a.earned_at.unwrap_or(0.0)));
        got
    }

    /// Niezdobyte najbliżej celu (największy ułamek), najwyżej `limit`.
    pub fn closest(&self, limit: usize) -> Vec<&Progress> {
        let mut left: Vec<&Progress> = self
            .progress
            .iter()
            .filter(|p| !p.earned() && p.current > 0.0)
            .collect();
        left.sort_by(|a, b| cmp_f64(b.fraction(), a.fraction()));
        left.truncate(limit);
        left
    }
}

/// Ustawienia liczenia poza samymi sesjami.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EvalOptions {
    /// liczba znajomych na koncie
    pub friends: u32,
    /// liczba poleceń
    pub referrals: u32,
    /// zapisane wcześniej zdobycze: id -> czas (nie odbieramy ich, nawet gdy sesja zniknie)
    pub earned_before: HashMap<String, f64>,
    /// teraz (UTC); None - zegar systemowy
    pub now: Option<f64>,
    /// ile dni wolnych na 7 nie przerywa serii
    pub rest_per_week: usize,
}

impl Default for EvalOptions {
    fn default() -> Self {
        EvalOptions {
            friends: 0,
            referrals: 0,
            earned_before: HashMap::new(),
            now: None,
            rest_per_week: 1,
        }
    }
}

/// Teraz w sekundach uniksowych.
pub fn now_unix() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// (czas, wartość narastająca) -> czas pierwszego przekroczenia progu.
fn first_crossing(values: &[(f64, f64)], target: f64) -> Option<f64> {
    values
        .iter()
        .find(|(_, v)| *v >= target - EPS)
        .map(|(t, _)| *t)
}

/// Długość serii (dni kalendarzowe od jej początku) na każdy zaliczony dzień oraz początek
/// serii, która trwa jeszcze po `last_day` (None - przerwana).
///
/// Dzień wolny przerywa serię, gdy jest drugi z rzędu albo gdy w ostatnich 7 dniach serii
/// był już inny wolny dzień (najwyżej `rest_per_week` wolnych na 7 dni).
fn streaks(
    day_done: &BTreeMap<i64, f64>,
    last_day: i64,
    rest_per_week: usize,
) -> (BTreeMap<i64, i64>, Option<i64>) {
    let mut out = BTreeMap::new();
    let (Some(&first), Some(&last_done)) = (day_done.keys().next(), day_done.keys().next_back())
    else {
        return (out, None);
    };
    let mut start: Option<i64> = None;
    let mut rests: Vec<i64> = Vec::new();
    for d in first..=last_day.max(last_done) {
        if day_done.contains_key(&d) {
            let s = *start.get_or_insert_with(|| {
                rests.clear();
                d
            });
            out.insert(d, d - s + 1);
            continue;
        }
        if start.is_none() {
            continue;
        }
        let recent = rests.iter().filter(|&&r| r > d - 7).count();
        if rests.contains(&(d - 1)) || recent >= rest_per_week {
            start = None;
            rests.clear();
        } else {
            rests.push(d);
        }
    }
    (out, start)
}

/// Stan gracza z jego przejść (z [`walks_from_sessions`], posortowanych po końcu).
pub fn evaluate(defs: &Definitions, walks: &[Walk], opts: &EvalOptions) -> Status {
    let now = opts.now.unwrap_or_else(now_unix);

    let mut total = 0.0_f64;
    let mut cumulative: Vec<(f64, f64)> = Vec::new();
    let mut best_session: Vec<(f64, f64)> = Vec::new();
    let mut day_steps: HashMap<i64, i64> = HashMap::new();
    let mut day_steps_at: Vec<(f64, f64)> = Vec::new();
    let mut day_km: HashMap<i64, f64> = HashMap::new();
    let mut day_done_at: BTreeMap<i64, f64> = BTreeMap::new();
    let mut game_km: HashMap<&str, f64> = HashMap::new();
    let mut best_game: Vec<(f64, f64)> = Vec::new();
    // seria (czas, liczba gier z co najmniej `km`) dla każdego osiągnięcia gry_km
    let mut games_over: HashMap<&str, Vec<(f64, f64)>> = HashMap::new();
    let mut first_game: Option<f64> = None;
    let mut first_training: Option<f64> = None;
    let mut windows: HashMap<&str, f64> = HashMap::new();
    let mut best_km = 0.0_f64;
    let mut best_day = 0_i64;
    let games_kinds: Vec<&Achievement> = defs
        .achievements
        .iter()
        .filter(|a| a.kind == "gry_km")
        .collect();
    let time_windows: Vec<(&Achievement, i64, i64)> = defs
        .achievements
        .iter()
        .filter(|a| a.kind == "start_miedzy")
        .map(|a| {
            (
                a,
                a.param_or("od", 0.0) as i64,
                a.param_or("do", 0.0) as i64,
            )
        })
        .collect();
    let min_km_day = defs.min_km_streak_day();

    for w in walks {
        let day = w.day();
        total += w.km;
        cumulative.push((w.end, total));
        best_km = best_km.max(w.km);
        best_session.push((w.end, best_km));
        let steps = day_steps.entry(day).or_insert(0);
        *steps += w.steps;
        best_day = best_day.max(*steps);
        day_steps_at.push((w.end, best_day as f64));
        let km_today = day_km.entry(day).or_insert(0.0);
        *km_today += w.km;
        if *km_today >= min_km_day {
            day_done_at.entry(day).or_insert(w.end);
        }
        if w.mode == TRAINING_MODE {
            first_training.get_or_insert(w.end);
        } else {
            first_game.get_or_insert(w.end);
            *game_km.entry(w.game.as_str()).or_insert(0.0) += w.km;
            let best = game_km.values().copied().reduce(f64::max).unwrap_or(0.0);
            best_game.push((w.end, best));
            for a in &games_kinds {
                let need = a.param_or("km", 0.0);
                let count = game_km.values().filter(|&&v| v >= need).count();
                games_over
                    .entry(a.id.as_str())
                    .or_default()
                    .push((w.end, count as f64));
            }
        }
        let hour = w.hour();
        for (a, lo, hi) in &time_windows {
            let inside = if lo < hi {
                *lo <= hour && hour < *hi
            } else {
                hour >= *lo || hour < *hi
            };
            if inside {
                windows.entry(a.id.as_str()).or_insert(w.end);
            }
        }
    }

    // dzisiejszy dzień jeszcze trwa, więc nie jest dniem wolnym: seria trwa, jeśli przetrwała
    // do wczoraj
    let today = local_day(now, walks.last().map_or(0, |w| w.tz_offset_min));
    let (streak, open_start) = streaks(&day_done_at, today - 1, opts.rest_per_week);
    let alive = match (open_start, day_done_at.keys().next_back()) {
        (Some(_), Some(last)) => streak.get(last).copied().unwrap_or(0),
        _ => 0,
    };
    let best_game_km = game_km.values().copied().reduce(f64::max).unwrap_or(0.0);

    let mut progress = Vec::with_capacity(defs.achievements.len());
    for a in &defs.achievements {
        let mut p = Progress::new(a.clone());
        match a.kind.as_str() {
            "dystans" => {
                (p.target, p.current) = (a.param_or("km", 0.0), total);
                p.earned_at = first_crossing(&cumulative, p.target);
            }
            "sesja_km" => {
                (p.target, p.current) = (a.param_or("km", 0.0), best_km);
                p.earned_at = first_crossing(&best_session, p.target);
            }
            "dzien_kroki" => {
                (p.target, p.current) = (a.param_or("kroki", 0.0), best_day as f64);
                p.earned_at = first_crossing(&day_steps_at, p.target);
            }
            "seria" => {
                (p.target, p.current) = (a.param_or("dni", 0.0), alive as f64);
                p.earned_at = streak
                    .iter()
                    .filter(|&(_, &n)| n as f64 >= p.target)
                    .filter_map(|(d, _)| day_done_at.get(d).copied())
                    .reduce(f64::min);
            }
            "gra_pierwsza" => {
                p.current = if first_game.is_some() { 1.0 } else { 0.0 };
                p.earned_at = first_game;
            }
            "trening" => {
                p.current = if first_training.is_some() { 1.0 } else { 0.0 };
                p.earned_at = first_training;
            }
            "gry_km" => {
                let series = games_over
                    .get(a.id.as_str())
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                p.target = a.param_or("gier", 0.0);
                p.current = series.last().map_or(0.0, |(_, v)| *v);
                p.earned_at = first_crossing(series, p.target);
            }
            "gra_km" => {
                (p.target, p.current) = (a.param_or("km", 0.0), best_game_km);
                p.earned_at = first_crossing(&best_game, p.target);
            }
            "start_miedzy" => {
                p.earned_at = windows.get(a.id.as_str()).copied();
                p.current = if p.earned_at.is_some() { 1.0 } else { 0.0 };
            }
            "znajomi" => {
                (p.target, p.current) = (a.param_or("ile", 0.0), f64::from(opts.friends));
                p.earned_at = (p.current >= p.target).then_some(now);
            }
            "polecenia" => {
                (p.target, p.current) = (a.param_or("ile", 0.0), f64::from(opts.referrals));
                p.earned_at = (p.current >= p.target).then_some(now);
            }
            _ => {}
        }
        if let Some(&before) = opts.earned_before.get(&a.id) {
            // nie odbieramy
            p.earned_at = Some(p.earned_at.map_or(before, |at| at.min(before)));
        }
        progress.push(p);
    }

    let bonus: i64 = progress
        .iter()
        .filter(|p| p.earned())
        .map(|p| {
            defs.rarity_xp
                .get(&p.achievement.rarity)
                .copied()
                .unwrap_or(0)
        })
        .sum();
    let xp = (total * defs.xp_per_km as f64) as i64 + bonus;
    let level = defs.level_of(xp as f64);
    let level_floor = defs.xp_for(level);
    let level_ceiling = if level >= defs.max_level() {
        level_floor
    } else {
        defs.xp_for(level + 1)
    };
    let span = level_ceiling - level_floor;
    let level_fraction = if span <= 0 {
        1.0
    } else {
        ((xp - level_floor) as f64 / span as f64).clamp(0.0, 1.0)
    };
    let km_to_next_level = ((level_ceiling - xp) as f64 / defs.xp_per_km as f64).max(0.0);
    Status {
        km: total,
        xp,
        level,
        rank: defs.rank(level),
        next_rank: defs.next_rank(level),
        level_floor,
        level_ceiling,
        level_fraction,
        km_to_next_level,
        progress,
    }
}

// --- kolejka popupów ----------------------------------------------------------------------------

/// Najwięcej popupów osiągnięć z jednego przeliczenia (próg zbiorczego popupu z 0.5.x).
pub const MAX_POPUPS_AT_ONCE: usize = 4;

/// Nowe osiągnięcia z jednego przeliczenia rozdzielone na popupy.
#[derive(Debug, Clone, PartialEq)]
pub struct PopupQueue<'a> {
    /// pokazywane pojedynczo, od najstarszego
    pub single: Vec<&'a Progress>,
    /// reszta do jednego zbiorczego popupu „+N kolejnych”, od najstarszego (puste - bez niego)
    pub grouped: Vec<&'a Progress>,
}

impl PopupQueue<'_> {
    /// Ile popupów osiągnięć to daje (pojedyncze + ewentualny zbiorczy).
    pub fn popups(&self) -> usize {
        self.single.len() + usize::from(!self.grouped.is_empty())
    }
}

/// Kolejka popupów dla nowych osiągnięć (np. kilka progów szlaku w jednej sesji).
///
/// Kolejność od najstarszego (`earned_at`, brak czasu jak 0 w 0.5.x; przy równym czasie
/// kolejność definicji). Do `max` nowych - każde osobno. Więcej - najwyżej `max` popupów
/// razem: `max - 1` najrzadszych osobno (przy równej rzadkości starsze), reszta w jednym
/// zbiorczym. Popup awansu dokłada wywołujący, poza tym limitem.
pub fn popup_queue<'a>(
    defs: &Definitions,
    new: impl IntoIterator<Item = &'a Progress>,
    max: usize,
) -> PopupQueue<'a> {
    let order = |p: &Progress| {
        defs.achievements
            .iter()
            .position(|a| a.id == p.achievement.id)
            .unwrap_or(usize::MAX)
    };
    let mut all: Vec<&Progress> = new.into_iter().collect();
    all.sort_by(|a, b| {
        cmp_f64(a.earned_at.unwrap_or(0.0), b.earned_at.unwrap_or(0.0))
            .then_with(|| order(a).cmp(&order(b)))
    });
    if all.len() <= max {
        return PopupQueue {
            single: all,
            grouped: Vec::new(),
        };
    }
    // nieznana rzadkość liczy się jak najczęstsza
    let tier = |p: &Progress| {
        defs.rarities
            .iter()
            .position(|r| *r == p.achievement.rarity)
            .map_or(0, |i| i + 1)
    };
    let mut by_rarity: Vec<usize> = (0..all.len()).collect();
    // sortowanie stabilne: przy równej rzadkości zostaje kolejność od najstarszego
    by_rarity.sort_by_key(|&i| std::cmp::Reverse(tier(all[i])));
    let mut picked = vec![false; all.len()];
    for &i in by_rarity.iter().take(max.saturating_sub(1)) {
        picked[i] = true;
    }
    let (single, grouped): (Vec<_>, Vec<_>) =
        all.into_iter().zip(picked).partition(|&(_, chosen)| chosen);
    PopupQueue {
        single: single.into_iter().map(|(p, _)| p).collect(),
        grouped: grouped.into_iter().map(|(p, _)| p).collect(),
    }
}

#[cfg(test)]
mod tests {
    //! Port tests/test_poziomy.py (bez testu migracji bazy 0.3 - to moduł history).

    use super::*;

    const DAY: f64 = 86400.0;
    const T0: f64 = 1_767_225_600.0; // 2026-01-01 00:00 UTC

    fn defs() -> &'static Definitions {
        Definitions::builtin().expect("wbudowane definicje")
    }

    fn walk_full(day: i64, km: f64, steps: i64, game: &str, mode: &str, hour: i64) -> Walk {
        let start = T0 + day as f64 * DAY + hour as f64 * 3600.0;
        Walk {
            start,
            end: start + 3600.0,
            km,
            steps,
            game: game.into(),
            mode: mode.into(),
            tz_offset_min: 0,
        }
    }

    fn walk(day: i64, km: f64) -> Walk {
        walk_full(day, km, 0, "g1", "gra", 18)
    }

    fn at(now: f64) -> EvalOptions {
        EvalOptions {
            now: Some(now),
            ..EvalOptions::default()
        }
    }

    fn p<'a>(s: &'a Status, id: &str) -> &'a Progress {
        s.get(id).expect("osiągnięcie")
    }

    fn sess(i: u32, start: f64, end: f64, steps: i64, km: f64, active: f64) -> Session {
        Session {
            id: format!("s{i}"),
            person: "p".into(),
            person_name: "P".into(),
            profile: "g".into(),
            profile_name: "G".into(),
            start,
            end,
            steps,
            distance_km: km,
            active_s: active,
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

    #[test]
    fn definitions_are_consistent() {
        let d = defs();
        assert_eq!(d.max_level(), 100);
        assert!(d.levels.windows(2).all(|w| w[0] < w[1]) && d.levels[0] == 0);
        let ranks: Vec<u32> = d.ranks.iter().map(|r| r.level).collect();
        assert!(ranks.windows(2).all(|w| w[0] < w[1]));
        assert_eq!((ranks[0], ranks[ranks.len() - 1]), (1, 100));
        let kinds = [
            "dystans",
            "sesja_km",
            "dzien_kroki",
            "seria",
            "gra_pierwsza",
            "gry_km",
            "gra_km",
            "trening",
            "start_miedzy",
            "znajomi",
            "polecenia",
        ];
        for a in &d.achievements {
            assert!(kinds.contains(&a.kind.as_str()), "{}", a.id);
            assert!(d.rarities.contains(&a.rarity), "{}", a.id);
            assert!(!a.name.is_empty() && !a.flavor.is_empty() && !a.condition.is_empty());
            assert!(
                !a.flavor.contains("eś ") && !a.flavor.ends_with("eś."),
                "{}",
                a.id
            );
        }
        let trail: Vec<f64> = d.trail().iter().filter_map(|a| a.km()).collect();
        assert!(trail.windows(2).all(|w| w[0] <= w[1]));
        let max_gap = trail
            .windows(2)
            .filter(|w| w[1] <= 2865.0)
            .map(|w| w[1] - w[0])
            .fold(0.0, f64::max);
        assert!(max_gap <= 150.0); // do Mordoru przystanek co ok. 30 dni przy 5 km dziennie
    }

    #[test]
    fn level_100_after_about_five_years_at_5_km_a_day() {
        let d = defs();
        let xp_km_only = (5 * 365 * 5 * d.xp_per_km) as f64;
        let need = d.xp_for(100) as f64;
        assert!(0.85 * need < xp_km_only && xp_km_only < 1.1 * need);
    }

    #[test]
    fn distance_thresholds_and_time_of_unlock() {
        let d = defs();
        let walks = [walk(0, 1.5), walk(1, 4.5), walk(2, 0.3)];
        let s = evaluate(d, &walks, &at(T0 + 3.0 * DAY));
        assert_eq!(p(&s, "km1").earned_at, Some(walks[0].end));
        assert_eq!(p(&s, "km2").earned_at, Some(walks[1].end)); // 1,5 + 4,5 = 6 km
        assert_eq!(p(&s, "km57").earned_at, Some(walks[1].end));
        assert!(!p(&s, "km113").earned());
        assert!((s.km - 6.3).abs() < 1e-9);
        let bonus: i64 = s
            .earned()
            .iter()
            .map(|p| d.rarity_xp[&p.achievement.rarity])
            .sum();
        assert_eq!(s.xp, (6.3_f64 * 100.0) as i64 + bonus);
        assert_eq!(s.level, d.level_of(s.xp as f64));
    }

    #[test]
    fn simulator_sessions_do_not_count() {
        let real = sess(1, T0, T0 + 1800.0, 4000, 3.0, 1800.0);
        let mut sim = sess(2, T0, T0 + 600.0, 1000, 30.0, 600.0);
        sim.source = "symulator".into();
        let walks = walks_from_sessions(&[real, sim]);
        assert_eq!(walks.iter().map(|w| w.km).collect::<Vec<_>>(), [3.0]);
    }

    #[test]
    fn streak_allows_one_rest_day_in_seven() {
        // dni 0-7 z przerwą w dniu 3: seria 8 dni kalendarzowych
        let walks: Vec<Walk> = (0..8).filter(|&i| i != 3).map(|i| walk(i, 2.5)).collect();
        let s = evaluate(defs(), &walks, &at(T0 + 7.0 * DAY + 3600.0));
        assert!(p(&s, "seria3").earned());
        assert!(p(&s, "seria7").earned());
        assert_eq!(p(&s, "seria7").current, 8.0);
    }

    #[test]
    fn two_rest_days_in_a_row_break_the_streak() {
        let walks: Vec<Walk> = [0, 1, 2, 5, 6, 7, 8]
            .iter()
            .map(|&i| walk(i, 2.5))
            .collect();
        let s = evaluate(defs(), &walks, &at(T0 + 8.0 * DAY + 3600.0));
        assert!(!p(&s, "seria7").earned());
        assert!(p(&s, "seria3").earned());
        assert_eq!(p(&s, "seria7").current, 4.0); // od dnia 5
    }

    #[test]
    fn short_day_does_not_count_and_old_streak_is_not_current() {
        let mut walks: Vec<Walk> = (0..4).map(|i| walk(i, 2.5)).collect();
        walks.push(walk(4, 1.0));
        let s = evaluate(defs(), &walks, &at(T0 + 20.0 * DAY));
        assert!(p(&s, "seria3").earned());
        assert_eq!(p(&s, "seria7").current, 0.0); // seria dawno przerwana
    }

    #[test]
    fn daily_steps_session_distance_and_time_windows() {
        let walks = [
            walk_full(0, 3.2, 4000, "g1", "gra", 5),
            walk_full(0, 1.0, 3500, "g1", "gra", 20),
            walk_full(1, 1.0, 100, "g1", "gra", 23),
        ];
        let s = evaluate(defs(), &walks, &at(T0 + 2.0 * DAY));
        assert_eq!(p(&s, "d7").earned_at, Some(walks[1].end)); // 7 500 kroków w dniu 0
        assert_eq!(p(&s, "s3").earned_at, Some(walks[0].end));
        assert_eq!(p(&s, "rano").earned_at, Some(walks[0].end));
        assert_eq!(p(&s, "noc").earned_at, Some(walks[2].end));
    }

    #[test]
    fn games_variety_and_training() {
        let mut walks = vec![
            walk_full(0, 10.0, 0, "a", "gra", 18),
            walk_full(1, 10.0, 0, "b", "gra", 18),
            walk_full(2, 9.0, 0, "c", "gra", 18),
            walk_full(3, 5.0, 0, "c", "trening", 18),
        ];
        let s = evaluate(defs(), &walks, &at(T0 + 4.0 * DAY));
        assert_eq!(p(&s, "gra1").earned_at, Some(walks[0].end));
        assert!(!p(&s, "gry3").earned()); // trening nie liczy się do gry
        assert_eq!(p(&s, "gry3").current, 2.0);
        assert_eq!(p(&s, "trening").earned_at, Some(walks[3].end));
        walks.push(walk_full(4, 1.0, 0, "c", "gra", 18));
        let s2 = evaluate(defs(), &walks, &at(T0 + 5.0 * DAY));
        assert!(p(&s2, "gry3").earned());
    }

    #[test]
    fn earned_achievements_are_never_taken_away() {
        let d = defs();
        let mut opts = at(T0);
        opts.earned_before.insert("km1".into(), T0);
        let s = evaluate(d, &[], &opts);
        assert_eq!(p(&s, "km1").earned_at, Some(T0));
        let rarity = &d.by_id("km1").expect("km1").rarity;
        assert_eq!(s.xp, d.rarity_xp[rarity]);
    }

    #[test]
    fn ranks_cover_levels() {
        let d = defs();
        assert_eq!(d.rank(1).name, d.ranks[0].name);
        assert_eq!(d.rank(100).name, d.ranks[d.ranks.len() - 1].name);
        assert_eq!(d.next_rank(100), None);
        assert_eq!(d.next_rank(1).as_ref(), Some(&d.ranks[1]));
    }

    #[test]
    fn trail_rarity_only_grows_with_distance() {
        let d = defs();
        let tiers: Vec<usize> = d
            .trail()
            .iter()
            .map(|a| {
                d.rarities
                    .iter()
                    .position(|r| *r == a.rarity)
                    .expect("rzadkość")
            })
            .collect();
        assert!(tiers.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn unreliable_sessions_do_not_count_like_on_the_server() {
        let ok = sess(1, T0, T0 + 3600.0, 6000, 4.5, 3600.0);
        let too_fast = sess(2, T0 + 4000.0, T0 + 7600.0, 30000, 25.0, 3600.0); // 25 km/h
        let too_many_steps = sess(3, T0 + 8000.0, T0 + 8600.0, 3000, 2.0, 600.0); // 300/min
        let weird_step = sess(4, T0 + 9000.0, T0 + 12600.0, 1000, 4.0, 3600.0); // krok 4 m
        let no_motion = sess(5, T0 + 13000.0, T0 + 13600.0, 0, 0.0, 0.0);
        let no_steps = sess(6, T0 + 14000.0, T0 + 17600.0, 0, 4.0, 3600.0); // bez licznika
        let walks = walks_from_sessions(&[
            ok,
            too_fast,
            too_many_steps,
            weird_step,
            no_motion,
            no_steps,
        ]);
        assert_eq!(walks.iter().map(|w| w.km).collect::<Vec<_>>(), [4.5, 4.0]);
    }

    #[test]
    fn overlapping_session_is_skipped_and_day_is_capped_at_8_hours() {
        let h = 3600.0;
        let first = sess(1, T0, T0 + 3.0 * h, 20000, 15.0, 3.0 * h);
        let overlap = sess(2, T0 + h, T0 + 2.0 * h, 6000, 4.0, h);
        let later = sess(3, T0 + 4.0 * h, T0 + 10.0 * h, 40000, 30.0, 6.0 * h); // razem 9 h
        let walks = walks_from_sessions(&[first, overlap, later]);
        assert_eq!(walks.len(), 2);
        assert_eq!(walks[0].km, 15.0);
        assert!((walks[1].km - 25.0).abs() < 1e-9); // z 6 h liczy się 5 h
    }

    #[test]
    fn python_floor_division_and_modulo() {
        assert_eq!(py_floor_div(-1.0, DAY), -1.0);
        assert_eq!(py_floor_div(DAY, DAY), 1.0);
        assert_eq!(py_floor_div(DAY - 0.5, DAY), 0.0);
        assert_eq!(py_mod(-3600.0, DAY), DAY - 3600.0);
        let w = Walk {
            start: -1.0,
            ..walk(0, 1.0)
        };
        assert_eq!((w.day(), w.hour()), (-1, 23));
    }

    #[test]
    fn other_definitions_file_loads_with_unknown_fields() {
        let text = r#"{
            "wersja": 2, "xp_za_km": 50, "nowe_pole": {"x": 1},
            "xp_za_rzadkosc": {"pospolite": 10}, "rzadkosci": ["pospolite"],
            "poziomy_xp": [0, 100], "rangi": [{"od_poziomu": 1, "nazwa": "A", "opis": "a"}],
            "osiagniecia": [
                {"id": "km1", "typ": "dystans", "km": 1, "rzadkosc": "pospolite", "znak": "s",
                 "nazwa": "N", "klimat": "K", "warunek": "W", "ikona_v2": "nowa"},
                {"id": "x", "typ": "nowy_typ", "rzadkosc": "pospolite", "znak": "s",
                 "nazwa": "N", "klimat": "K", "warunek": "W"}
            ]
        }"#;
        let d = Definitions::from_json(text).expect("wersja 2");
        assert_eq!(d.version, 2);
        assert_eq!(
            d.achievements[0].params.get("ikona_v2"),
            Some(&Value::from("nowa"))
        );
        let s = evaluate(&d, &[walk(0, 2.0)], &at(T0 + DAY));
        assert_eq!(s.xp, 2 * 50 + 10);
        assert_eq!(s.level, 2);
        assert_eq!(s.level_ceiling, s.level_floor);
        assert_eq!(s.level_fraction, 1.0);
        assert!(!p(&s, "x").earned());

        let dir = tempfile::tempdir().expect("katalog");
        let path = dir.path().join("definicje.json");
        std::fs::write(&path, text).expect("zapis");
        assert_eq!(Definitions::load_file(&path).expect("plik"), d);
        assert!(matches!(
            Definitions::load_file(&dir.path().join("brak.json")),
            Err(LevelsError::Read { .. })
        ));
    }

    fn got(id: &str, earned_at: Option<f64>) -> Progress {
        let mut p = Progress::new(defs().by_id(id).expect("osiągnięcie").clone());
        p.earned_at = earned_at;
        p
    }

    fn ids(v: &[&Progress]) -> Vec<String> {
        v.iter().map(|p| p.achievement.id.clone()).collect()
    }

    #[test]
    fn popups_up_to_the_limit_go_from_the_oldest() {
        let list = [
            got("km57", Some(T0 + 50.0)),
            got("km2", Some(T0 + 10.0)),
            got("km1", Some(T0 + 10.0)), // ten sam czas - kolejność definicji
            got("noc", None),            // bez czasu jak 0 w 0.5.x
        ];
        let q = popup_queue(defs(), &list, MAX_POPUPS_AT_ONCE);
        assert_eq!(ids(&q.single), ["noc", "km1", "km2", "km57"]);
        assert!(q.grouped.is_empty());
        assert_eq!(q.popups(), 4);
        let empty = popup_queue(defs(), Vec::<&Progress>::new(), MAX_POPUPS_AT_ONCE);
        assert_eq!((empty.popups(), empty.single.len()), (0, 0));
    }

    #[test]
    fn many_new_achievements_keep_the_rarest_and_group_the_rest() {
        let list = [
            got("km30", Some(T0 + 5.0)),
            got("km483", Some(T0 + 4.0)),
            got("km2", Some(T0 + 3.0)),
            got("km160", Some(T0 + 2.0)),
            got("km1", Some(T0 + 1.0)),
        ];
        let q = popup_queue(defs(), &list, MAX_POPUPS_AT_ONCE);
        // 3 najrzadsze (przy równej rzadkości starsze) osobno, od najstarszego
        assert_eq!(ids(&q.single), ["km1", "km160", "km483"]);
        assert_eq!(ids(&q.grouped), ["km2", "km30"]);
        assert_eq!(q.popups(), MAX_POPUPS_AT_ONCE);
        // limit 2 - jak 0.5.x: najrzadsze i zbiorczy
        let q2 = popup_queue(defs(), &list, 2);
        assert_eq!(ids(&q2.single), ["km483"]);
        assert_eq!(q2.grouped.len(), 4);
        // limit 1 i 0 - tylko zbiorczy
        for max in [0, 1] {
            let q = popup_queue(defs(), &list, max);
            assert!(q.single.is_empty());
            assert_eq!(ids(&q.grouped), ["km1", "km160", "km2", "km483", "km30"]);
        }
    }

    #[test]
    fn unknown_rarity_counts_as_the_most_common() {
        let mut odd = got("km160", Some(T0 + 1.0));
        odd.achievement.rarity = "nowa".into();
        let list = [odd, got("km1", Some(T0 + 2.0)), got("km2", Some(T0 + 3.0))];
        let q = popup_queue(defs(), &list, 2);
        assert_eq!(ids(&q.single), ["km1"]);
        assert_eq!(ids(&q.grouped), ["km160", "km2"]);
    }

    #[test]
    fn one_long_first_session_queues_its_trail_stops() {
        // jedna sesja 12 km z v2: km1, km2, km57, km113 i inne progi naraz
        let walks = [Walk {
            end: T0 + 3.0 * 3600.0,
            ..walk_full(0, 12.0, 0, "g1", "gra", 0)
        }];
        let s = evaluate(defs(), &walks, &at(T0 + DAY));
        let new = s.earned();
        assert!(new.len() > MAX_POPUPS_AT_ONCE, "{:?}", ids(&new));
        let q = popup_queue(defs(), new.iter().copied(), MAX_POPUPS_AT_ONCE);
        assert_eq!(q.popups(), MAX_POPUPS_AT_ONCE);
        assert_eq!(q.single.len() + q.grouped.len(), new.len());
        let times: Vec<f64> = q.single.iter().filter_map(|p| p.earned_at).collect();
        assert!(times.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn invalid_definitions_are_rejected() {
        assert!(matches!(
            Definitions::from_json("{"),
            Err(LevelsError::Parse(_))
        ));
        let base = |levels: &str, extra: &str| {
            format!(
                r#"{{"wersja": 1, "xp_za_km": 100, "xp_za_rzadkosc": {{}}, "rzadkosci": [],
                "poziomy_xp": {levels}, "rangi": [{{"od_poziomu": 1, "nazwa": "A", "opis": "a"}}],
                "osiagniecia": [{extra}]}}"#
            )
        };
        assert!(Definitions::from_json(&base("[0, 10]", "")).is_ok());
        assert!(matches!(
            Definitions::from_json(&base("[]", "")),
            Err(LevelsError::Invalid(_))
        ));
        assert!(matches!(
            Definitions::from_json(&base("[10, 0]", "")),
            Err(LevelsError::Invalid(_))
        ));
        let no_km = r#"{"id": "k", "typ": "dystans", "rzadkosc": "r", "znak": "s",
            "nazwa": "N", "klimat": "K", "warunek": "W"}"#;
        assert!(matches!(
            Definitions::from_json(&base("[0]", no_km)),
            Err(LevelsError::Invalid(_))
        ));
    }
}
