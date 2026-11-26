//! Zgodność modułu levels z Pythonem (bieznia/poziomy): fixtury z walklevel/tools/fixtures/levels.py.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;
use walklevel_core::levels::{Definitions, EvalOptions, evaluate, walks_from_sessions};
use walklevel_core::model::Session;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/levels")
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
}

fn assert_close(a: f64, b: f64, what: &str) {
    assert!(close(a, b), "{what}: Rust {a} != Python {b}");
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    name: String,
    input: Input,
    expected: Expected,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Input {
    sessions: Vec<Session>,
    now: f64,
    friends: u32,
    referrals: u32,
    earned_before: HashMap<String, f64>,
    rest_per_week: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedWalk {
    start: f64,
    end: f64,
    km: f64,
    steps: i64,
    game: String,
    mode: String,
    tz_offset_min: i64,
    day: i64,
    hour: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedProgress {
    id: String,
    earned_at: Option<f64>,
    current: f64,
    target: f64,
    fraction: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Expected {
    walks: Vec<ExpectedWalk>,
    km: f64,
    xp: i64,
    level: u32,
    rank: String,
    next_rank: Option<String>,
    level_floor: i64,
    level_ceiling: i64,
    level_fraction: f64,
    km_to_next_level: f64,
    progress: Vec<ExpectedProgress>,
    earned: Vec<String>,
    closest: Vec<String>,
    closest10: Vec<String>,
}

fn check_case(defs: &Definitions, c: &Case) {
    let n = &c.name;
    let walks = walks_from_sessions(&c.input.sessions);
    let e = &c.expected;
    assert_eq!(walks.len(), e.walks.len(), "{n}: liczba przejść");
    for (i, (w, x)) in walks.iter().zip(&e.walks).enumerate() {
        let what = format!("{n}: przejście {i}");
        assert_eq!((w.start, w.end), (x.start, x.end), "{what}");
        assert_close(w.km, x.km, &what);
        assert_eq!(w.steps, x.steps, "{what}: kroki");
        assert_eq!(
            (&w.game, &w.mode, w.tz_offset_min),
            (&x.game, &x.mode, x.tz_offset_min)
        );
        assert_eq!(
            (w.day(), w.hour()),
            (x.day, x.hour),
            "{what}: dzień i godzina"
        );
    }

    let opts = EvalOptions {
        friends: c.input.friends,
        referrals: c.input.referrals,
        earned_before: c.input.earned_before.clone(),
        now: Some(c.input.now),
        rest_per_week: c.input.rest_per_week,
    };
    let s = evaluate(defs, &walks, &opts);
    assert_close(s.km, e.km, &format!("{n}: km"));
    assert_eq!(s.xp, e.xp, "{n}: XP");
    assert_eq!(s.level, e.level, "{n}: poziom");
    assert_eq!(s.rank.name, e.rank, "{n}: ranga");
    assert_eq!(
        s.next_rank.as_ref().map(|r| r.name.clone()),
        e.next_rank,
        "{n}: następna ranga"
    );
    assert_eq!(
        (s.level_floor, s.level_ceiling),
        (e.level_floor, e.level_ceiling),
        "{n}"
    );
    assert_close(
        s.level_fraction,
        e.level_fraction,
        &format!("{n}: ułamek poziomu"),
    );
    assert_close(
        s.km_to_next_level,
        e.km_to_next_level,
        &format!("{n}: km do poziomu"),
    );

    assert_eq!(s.progress.len(), e.progress.len(), "{n}: liczba osiągnięć");
    for (p, x) in s.progress.iter().zip(&e.progress) {
        let what = format!("{n}: {}", x.id);
        assert_eq!(p.achievement.id, x.id, "{what}: kolejność");
        match (p.earned_at, x.earned_at) {
            (Some(a), Some(b)) => assert_close(a, b, &format!("{what}: czas zdobycia")),
            (a, b) => assert_eq!(a, b, "{what}: zdobyte"),
        }
        assert_close(p.current, x.current, &format!("{what}: postęp"));
        assert_close(p.target, x.target, &format!("{what}: cel"));
        assert_close(p.fraction(), x.fraction, &format!("{what}: ułamek"));
    }
    let ids = |v: Vec<&walklevel_core::levels::Progress>| -> Vec<String> {
        v.into_iter().map(|p| p.achievement.id.clone()).collect()
    };
    assert_eq!(ids(s.earned()), e.earned, "{n}: zdobyte od najnowszego");
    assert_eq!(ids(s.closest(3)), e.closest, "{n}: najbliższe");
    assert_eq!(ids(s.closest(10)), e.closest10, "{n}: najbliższe 10");
}

/// Wszystkie przypadki z katalogu (bez plików definicji i podkatalogów); zwraca ich nazwy.
fn check_dir(defs: &Definitions, dir: &Path, skip: &[&str]) -> Vec<String> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir).expect("katalog fixtur") {
        let path = entry.expect("wpis").path();
        let skipped = path
            .file_name()
            .and_then(|f| f.to_str())
            .is_some_and(|f| skip.contains(&f));
        if path.is_dir() || skipped {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("fixtura");
        let c: Case = serde_json::from_str(&text).expect("format fixtury");
        check_case(defs, &c);
        names.push(c.name);
    }
    names
}

/// Definicje v1 (0.5.x) - kopia w fixturach, bo wbudowane są już v2.
fn v1() -> Definitions {
    Definitions::load_file(&fixtures_dir().join("definicje-v1.json")).expect("definicje v1")
}

#[test]
fn evaluate_matches_python_fixtures() {
    let skip = ["definicje.json", "definicje-v1.json"];
    let names = check_dir(&v1(), &fixtures_dir(), &skip);
    assert!(names.len() >= 18, "za mało fixtur: {names:?}");
}

#[test]
fn builtin_v2_matches_python() {
    let defs = Definitions::builtin().expect("wbudowane definicje");
    let dir = fixtures_dir().join("v2");
    let text = std::fs::read_to_string(dir.join("wynik_definicji.json")).expect("fixtura");
    check_definitions(defs, &serde_json::from_str(&text).expect("json"));
    let names = check_dir(defs, &dir, &["wynik_definicji.json"]);
    assert!(names.len() >= 9, "za mało fixtur: {names:?}");
}

#[test]
fn other_definitions_file_matches_python() {
    let dir = fixtures_dir().join("inne");
    let defs = Definitions::load_file(&dir.join("definicje.json")).expect("inne definicje");
    let text = std::fs::read_to_string(dir.join("wynik_definicji.json")).expect("fixtura");
    check_definitions(&defs, &serde_json::from_str(&text).expect("json"));
    let names = check_dir(&defs, &dir, &["definicje.json", "wynik_definicji.json"]);
    assert!(names.len() >= 4, "za mało fixtur: {names:?}");
}

#[test]
fn definitions_v1_match_python() {
    let text = std::fs::read_to_string(fixtures_dir().join("definicje.json")).expect("fixtura");
    check_definitions(&v1(), &serde_json::from_str(&text).expect("json"));
}

fn check_definitions(d: &Definitions, x: &Value) {
    assert_eq!(Value::from(d.version), x["version"]);
    assert_eq!(Value::from(d.xp_per_km), x["xpPerKm"]);
    assert_eq!(
        serde_json::to_value(&d.rarity_xp).expect("json"),
        x["rarityXp"]
    );
    assert_eq!(
        serde_json::to_value(&d.rarities).expect("json"),
        x["rarities"]
    );
    assert_eq!(serde_json::to_value(&d.levels).expect("json"), x["levels"]);
    assert_eq!(serde_json::to_value(&d.ranks).expect("json"), x["ranks"]);
    assert_eq!(Value::from(d.max_level()), x["maxLevel"]);

    let achievements = x["achievements"].as_array().expect("lista");
    assert_eq!(d.achievements.len(), achievements.len());
    for (a, y) in d.achievements.iter().zip(achievements) {
        assert_eq!(Value::from(a.id.as_str()), y["id"]);
        assert_eq!(Value::from(a.kind.as_str()), y["kind"]);
        assert_eq!(Value::from(a.rarity.as_str()), y["rarity"]);
        assert_eq!(Value::from(a.mark.as_str()), y["mark"]);
        assert_eq!(Value::from(a.name.as_str()), y["name"]);
        assert_eq!(Value::from(a.flavor.as_str()), y["flavor"]);
        assert_eq!(Value::from(a.condition.as_str()), y["condition"]);
        assert_eq!(Value::Object(a.params.clone()), y["params"], "{}", a.id);
        assert_eq!(a.km(), y["km"].as_f64(), "{}", a.id);
    }
    let trail: Vec<Value> = d
        .trail()
        .iter()
        .map(|a| Value::from(a.id.as_str()))
        .collect();
    assert_eq!(Value::Array(trail), x["trail"]);

    for pair in x["levelOf"].as_array().expect("lista") {
        let xp = pair[0].as_f64().expect("xp");
        assert_eq!(Value::from(d.level_of(xp)), pair[1], "level_of({xp})");
    }
    for pair in x["xpFor"].as_array().expect("lista") {
        let level = u32::try_from(pair[0].as_i64().expect("poziom").max(0)).expect("u32");
        assert_eq!(Value::from(d.xp_for(level)), pair[1], "xp_for({level})");
    }
    for pair in x["rankFor"].as_array().expect("lista") {
        let level = u32::try_from(pair[0].as_i64().expect("poziom")).expect("u32");
        assert_eq!(Value::from(d.rank(level).name), pair[1], "rank({level})");
    }
    for pair in x["nextRankFor"].as_array().expect("lista") {
        let level = u32::try_from(pair[0].as_i64().expect("poziom")).expect("u32");
        let name = d.next_rank(level).map(|r| r.name);
        assert_eq!(
            serde_json::to_value(name).expect("json"),
            pair[1],
            "next_rank({level})"
        );
    }
}

// --- reguły definicji v2 (docs/analiza-szlak.md) ---------------------------------------------

/// Znaki, które umie narysować odznaka (docs/wyglad.md, `odznaki.glyph()`); plus `wieniecNN`.
const MARKS: &[&str] = &[
    "stopy",
    "kopula",
    "helm",
    "miecz",
    "dycha",
    "licznik",
    "luki",
    "mur",
    "lisc",
    "gora",
    "wulkan",
    "pudlo",
    "wglab",
    "kostka",
    "biegacz",
    "glob",
    "plomien",
    "pad",
    "portale",
    "dom",
    "ksiezyc",
    "slonce",
    "druzyna",
    "fala",
    "portal",
    "brama",
    "drzwi",
    "celownik",
    "cytadela",
    "gwiazda",
    "dysk",
    "pierscien",
    "planeta",
    "wieza",
    "drzewo",
    "kolo",
    "czerw",
    "radiacja",
    "korona",
];

fn known_mark(mark: &str) -> bool {
    MARKS.contains(&mark)
        || mark
            .strip_prefix("wieniec")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

#[test]
fn v2_changes_only_the_trail() {
    // szlak przebudowany 26.11 (analiza-szlak.md, sekcja 9): id przystanków mogą zniknąć,
    // bo kont poza kontem Damiana jeszcze nie ma; zdobyte nieznane id evaluate pomija
    let (old, new) = (v1(), Definitions::builtin().expect("wbudowane definicje"));
    assert_eq!((old.version, new.version), (1, 2));
    assert_eq!(new.achievements.len(), 80);
    assert_eq!(new.trail().len(), 59);
    for a in old.achievements.iter().filter(|a| a.kind != "dystans") {
        let b = new.by_id(&a.id);
        assert_eq!(Some(a), b, "osiągnięcie spoza szlaku zmienione: {}", a.id);
    }
    // to samo id na szlaku v1 i v2 = ten sam próg (zdobyte wcześniej nie przychodzą „przed czasem”)
    for a in old.trail() {
        if let Some(b) = new.by_id(&a.id) {
            assert_eq!(a.km(), b.km(), "id {} zmieniło próg", a.id);
        }
    }
    assert_eq!(
        (old.xp_per_km, &old.rarity_xp, &old.rarities),
        (new.xp_per_km, &new.rarity_xp, &new.rarities)
    );
    assert_eq!((&old.levels, &old.ranks), (&new.levels, &new.ranks));
}

#[test]
fn trail_rarity_grows_with_km_and_fits_the_bounds() {
    // analiza-szlak.md, sekcja 1: progi spełnia i v1, i v2
    // odstęp: v1 do Mordoru (2 865 km) 151 km (test 0.5.x dopuszczał 152); v2 do 2 500 km
    // najwyżej 150, do 2 865 km najwyżej 200 (analiza-szlak.md, sekcja 9)
    check_trail_rules(&v1(), 2865.0, 152.0);
    let v2 = Definitions::builtin().expect("wbudowane definicje");
    check_trail_rules(v2, 2500.0, 150.0);
    check_trail_rules(v2, 2865.0, 200.0);
}

fn check_trail_rules(d: &Definitions, until_km: f64, gap: f64) {
    // górna granica km rzadkości przy 5 km dziennie: 14 / 91 / 365 / 912 dni, dalej bez granicy
    let bound = |rarity: &str| match rarity {
        "pospolite" => Some(70.0),
        "rzadkie" => Some(455.0),
        "epickie" => Some(1825.0),
        "heroiczne" => Some(4560.0),
        _ => None,
    };
    let tier = |r: &str| d.rarities.iter().position(|x| x == r).expect("rzadkość");
    let trail = d.trail();
    for w in trail.windows(2) {
        assert!(
            tier(&w[0].rarity) <= tier(&w[1].rarity),
            "v{}: rzadkość maleje: {} -> {}",
            d.version,
            w[0].id,
            w[1].id
        );
    }
    for a in &trail {
        let km = a.km().expect("km");
        let t = tier(&a.rarity);
        if t + 1 < d.rarities.len() {
            let max = bound(&a.rarity).expect("granica rzadkości");
            assert!(
                km <= max,
                "v{}: {} ({km} km) za daleko na {}",
                d.version,
                a.id,
                a.rarity
            );
        }
        if t > 0 {
            // poprzednia rzadkość kończy się przed tym przystankiem
            let min = bound(&d.rarities[t - 1]).expect("granica rzadkości");
            assert!(
                km > min,
                "v{}: {} ({km} km) za blisko na {}",
                d.version,
                a.id,
                a.rarity
            );
        }
    }
    let km: Vec<f64> = trail.iter().filter_map(|a| a.km()).collect();
    let max_gap = km
        .windows(2)
        .filter(|w| w[1] <= until_km)
        .map(|w| w[1] - w[0])
        .fold(0.0, f64::max);
    assert!(
        max_gap <= gap,
        "v{}: odstęp {max_gap} km do {until_km} km",
        d.version
    );
    // Skyrim nadal wypada na 6. km, Erangel na 12.
    let at = |id: &str| d.by_id(id).and_then(|a| a.km()).expect("przystanek");
    assert!(5.0 < at("km57") && at("km57") <= 6.0);
    assert!(11.0 < at("km113") && at("km113") <= 12.0);
}

#[test]
fn every_badge_mark_can_be_drawn() {
    for d in [&v1(), Definitions::builtin().expect("wbudowane definicje")] {
        for a in &d.achievements {
            let what = format!("v{}: {} ma nieznany znak {}", d.version, a.id, a.mark);
            assert!(known_mark(&a.mark), "{what}");
        }
    }
}

#[test]
fn server_file_matches_app_definitions() {
    // docs/definicje-v2-serwer.json (tools/definicje_serwer.py): te same id, typy, progi i rzadkości
    let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs");
    let read = |name: &str| -> Value {
        let text = std::fs::read_to_string(docs.join(name)).expect(name);
        serde_json::from_str(&text).expect("json")
    };
    let (srv, app) = (read("definicje-v2-serwer.json"), read("definicje-v2.json"));
    let d = Definitions::builtin().expect("wbudowane definicje");
    assert_eq!(srv["wersja"], Value::from(d.version));
    assert_eq!(srv["xp_za_km"], Value::from(d.xp_per_km));
    assert_eq!(
        srv["xp_za_rzadkosc"],
        serde_json::to_value(&d.rarity_xp).expect("json")
    );
    assert_eq!(
        srv["poziomy_xp"],
        serde_json::to_value(&d.levels).expect("json")
    );
    for (key, value) in app["zasady"].as_object().expect("zasady") {
        if value.is_number() {
            assert_eq!(&srv["zasady"][key], value, "zasada {key}");
        }
    }
    let rows = srv["osiagniecia"].as_array().expect("lista");
    assert_eq!(rows.len(), d.achievements.len());
    for (a, row) in d.achievements.iter().zip(rows) {
        let mut row = row.as_object().expect("osiągnięcie").clone();
        assert_eq!(row.remove("id"), Some(Value::from(a.id.as_str())));
        assert_eq!(
            row.remove("typ"),
            Some(Value::from(a.kind.as_str())),
            "{}",
            a.id
        );
        assert_eq!(
            row.remove("rz"),
            Some(Value::from(a.rarity.as_str())),
            "{}",
            a.id
        );
        let field = match a.kind.as_str() {
            "dystans" | "sesja_km" | "gra_km" | "gry_km" => Some("km"),
            "dzien_kroki" => Some("kroki"),
            "seria" => Some("dni"),
            "znajomi" | "polecenia" => Some("ile"),
            "gra_pierwsza" | "trening" | "start_miedzy" => None,
            other => panic!("typ {other} ({}) nieznany serwerowi", a.id),
        };
        if let (Some(field), Some(prog)) = (field, row.remove("prog")) {
            row.insert(field.to_string(), prog);
        }
        let num = |m: &serde_json::Map<String, Value>| -> Vec<(String, Option<f64>)> {
            let mut v: Vec<_> = m.iter().map(|(k, x)| (k.clone(), x.as_f64())).collect();
            v.sort_by(|x, y| x.0.cmp(&y.0));
            v
        };
        assert_eq!(num(&row), num(&a.params), "parametry {}", a.id);
    }
}
