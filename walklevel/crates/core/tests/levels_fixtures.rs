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

#[test]
fn evaluate_matches_python_fixtures() {
    let defs = Definitions::builtin().expect("wbudowane definicje");
    let mut names = Vec::new();
    for entry in std::fs::read_dir(fixtures_dir()).expect("katalog fixtur") {
        let path = entry.expect("wpis").path();
        if path.file_name().is_some_and(|f| f == "definicje.json") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("fixtura");
        let c: Case = serde_json::from_str(&text).expect("format fixtury");
        check_case(defs, &c);
        names.push(c.name);
    }
    assert!(names.len() >= 15, "za mało fixtur: {names:?}");
}

#[test]
fn definitions_match_python() {
    let d = Definitions::builtin().expect("wbudowane definicje");
    let text = std::fs::read_to_string(fixtures_dir().join("definicje.json")).expect("fixtura");
    let x: Value = serde_json::from_str(&text).expect("json");

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
