//! Testy historii: port tests/test_historia.py i zgodność z bazami z Pythona
//! (fixtury z walklevel/tools/fixtures/history.py).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::{FixedOffset, Local, TimeZone};
use rusqlite::types::ValueRef;
use serde_json::{Value, json};

use super::*;

fn mem() -> History {
    History::open_in_memory().unwrap()
}

fn local_ts(y: i32, m: u32, d: u32, h: u32, mi: u32) -> f64 {
    Local
        .with_ymd_and_hms(y, m, d, h, mi, 0)
        .earliest()
        .unwrap()
        .timestamp() as f64
}

#[allow(clippy::too_many_arguments)]
fn row(
    id: &str,
    person: &str,
    person_name: &str,
    profile: &str,
    profile_name: &str,
    start: f64,
    end: f64,
    steps: i64,
    distance_km: f64,
    active_s: f64,
) -> Session {
    let mut s = new_session(id, person, person_name, profile, profile_name, start, end);
    s.steps = steps;
    s.distance_km = distance_km;
    s.active_s = active_s;
    s
}

/// Zegar sterowany ręcznie i narastające liczniki aplikacji (jak Clock i walk() w Pythonie).
struct Walker {
    t: Arc<Mutex<f64>>,
    dist: f64,
    act: f64,
}

impl Walker {
    fn new(start: f64) -> (Self, SessionTracker) {
        let t = Arc::new(Mutex::new(start));
        let c = Arc::clone(&t);
        let tracker = SessionTracker::with_clock(Box::new(move || *c.lock().unwrap()));
        (
            Self {
                t,
                dist: 0.0,
                act: 0.0,
            },
            tracker,
        )
    }

    fn now(&self) -> f64 {
        *self.t.lock().unwrap()
    }

    fn tick(&self, s: f64) {
        *self.t.lock().unwrap() += s;
    }

    /// Chód: odczyt co sekundę, licznik kroków bieżni rośnie.
    fn walk(&mut self, t: &mut SessionTracker, h: &History, seconds: u32, start_steps: i64) -> i64 {
        let mut steps = start_steps;
        t.update(h, 0.0, Some(steps), self.dist, self.act).unwrap();
        for _ in 0..seconds {
            self.tick(1.0);
            steps += 100 / 60 + i64::from(self.now() % 3.0 == 0.0);
            self.dist += 2.5 / 3600.0;
            self.act += 1.0;
            t.update(h, 2.5, Some(steps), self.dist, self.act).unwrap();
        }
        steps
    }

    fn idle(&mut self, t: &mut SessionTracker, h: &History, seconds: u32, steps: i64) {
        for _ in 0..seconds {
            self.tick(1.0);
            t.update(h, 0.0, Some(steps), self.dist, self.act).unwrap();
        }
    }
}

fn all(h: &History) -> Vec<Session> {
    h.sessions(0.0, None, None, None).unwrap()
}

// --- port tests/test_historia.py ---------------------------------------------------------

#[test]
fn session_counts_steps_and_ends_after_idle() {
    let h = mem();
    let (mut w, mut t) = Walker::new(local_ts(2026, 11, 21, 18, 0));
    t.set_owner(&h, "p1", "Szalony Biegacz", "g1", "Cyberpunk 2077")
        .unwrap();
    let end_steps = w.walk(&mut t, &h, 300, 1000);
    assert_eq!(t.current().unwrap().steps, end_steps - 1000);
    w.idle(&mut t, &h, IDLE_END_S as u32 + 2, end_steps);
    assert!(t.current().is_none());
    let ss = all(&h);
    assert_eq!(ss.len(), 1);
    let s = &ss[0];
    assert_eq!(s.profile_name, "Cyberpunk 2077");
    assert_eq!(s.steps, end_steps - 1000);
    assert!((s.distance_km - 300.0 * 2.5 / 3600.0).abs() < 1e-9);
    assert!((s.avg_kmh() - 2.5).abs() < 1e-9);
    assert_eq!(s.id.len(), 32);
    assert!(s.updated > 0.0);
}

#[test]
fn treadmill_counter_reset_does_not_lose_steps() {
    let h = mem();
    let (mut w, mut t) = Walker::new(1_800_000_000.0);
    t.set_owner(&h, "p1", "A", "g1", "PEAK").unwrap();
    w.walk(&mut t, &h, 60, 5000);
    let before = t.current().unwrap().steps;
    w.walk(&mut t, &h, 60, 0); // bieżnia wyzerowała licznik
    let after = t.current().unwrap().steps;
    assert!(after > before);
    assert!(after < before * 2 + 10); // bez skoku o 5000
}

#[test]
fn switching_profile_or_person_splits_the_session() {
    let h = mem();
    let (mut w, mut t) = Walker::new(1_800_000_000.0);
    t.set_owner(&h, "p1", "A", "g1", "PEAK").unwrap();
    let steps = w.walk(&mut t, &h, 120, 0);
    t.set_owner(&h, "p1", "A", "g2", "Elden Ring").unwrap();
    let steps = w.walk(&mut t, &h, 120, steps);
    t.set_owner(&h, "p2", "B", "g9", "Elden Ring").unwrap();
    w.walk(&mut t, &h, 120, steps);
    t.finish(&h).unwrap();
    let mut names: Vec<(String, String)> = all(&h)
        .into_iter()
        .map(|s| (s.person_name, s.profile_name))
        .collect();
    names.sort();
    let want = [("A", "Elden Ring"), ("A", "PEAK"), ("B", "Elden Ring")];
    let want: Vec<(String, String)> = want
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    assert_eq!(names, want);
}

#[test]
fn short_accidental_moves_are_not_saved() {
    let h = mem();
    let (w, mut t) = Walker::new(1_800_000_000.0);
    t.set_owner(&h, "p1", "A", "g1", "PEAK").unwrap();
    w.tick(1.0);
    t.update(&h, 0.5, None, 0.0001, 1.0).unwrap();
    t.finish(&h).unwrap();
    assert_eq!(h.count().unwrap(), 0);
}

#[test]
fn aggregations_by_day_and_group() {
    let h = mem();
    let day = local_ts(2026, 11, 21, 18, 0);
    let rows = [
        row(
            "a",
            "p1",
            "A",
            "g1",
            "Cyberpunk 2077",
            day,
            day + 600.0,
            1000,
            0.5,
            600.0,
        ),
        row(
            "b",
            "p1",
            "A",
            "g2",
            "PEAK",
            day - 86400.0,
            day - 85800.0,
            400,
            0.2,
            300.0,
        ),
        row(
            "c",
            "p2",
            "B",
            "g3",
            "PEAK",
            day,
            day + 600.0,
            700,
            0.4,
            500.0,
        ),
    ];
    for mut s in rows {
        h.save(&mut s).unwrap();
    }
    let days = h.by_day(7, None, GroupKey::Profile, day + 3600.0).unwrap();
    assert_eq!(days.len(), 7);
    let last: Vec<(&str, i64)> = days[6].1.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    assert_eq!(last, [("Cyberpunk 2077", 1000), ("PEAK", 700)]);
    let prev: Vec<(&str, i64)> = days[5].1.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    assert_eq!(prev, [("PEAK", 400)]);
    let mine = h.by_group(0.0, Some("p1"), GroupKey::Profile).unwrap();
    let names: Vec<&str> = mine.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["Cyberpunk 2077", "PEAK"]);
    let people = h.by_group(0.0, None, GroupKey::Person).unwrap();
    assert_eq!(people[0].0, "A");
    assert_eq!(people[0].1.steps, 1400);
    assert_eq!(h.totals(0.0, Some("p2")).unwrap().steps, 700);
}

#[test]
fn import_merge_skips_duplicates_and_deleted_person_keeps_results() {
    let h = mem();
    let s = row("a", "p1", "A", "g1", "PEAK", 1.0, 2.0, 10, 0.1, 60.0);
    assert_eq!(h.merge(std::slice::from_ref(&s)).unwrap(), 1);
    let b = row("b", "p1", "A", "g1", "PEAK", 3.0, 4.0, 5, 0.1, 60.0);
    assert_eq!(h.merge(&[s, b]).unwrap(), 1);
    h.forget_person("p1").unwrap();
    let names: HashSet<String> = all(&h).into_iter().map(|s| s.person_name).collect();
    assert_eq!(names, HashSet::from([DELETED_PERSON.to_string()]));
    h.clear(Some("p1")).unwrap();
    assert_eq!(h.count().unwrap(), 0);
}

#[test]
fn today_by_hour_splits_long_sessions() {
    let h = mem();
    let start = local_ts(2026, 11, 25, 18, 30);
    let mut s = row(
        "h1",
        "p",
        "P",
        "g",
        "Gra",
        start,
        start + 3600.0,
        1200,
        1.0,
        3600.0,
    );
    h.save(&mut s).unwrap();
    let hours: HashMap<String, BTreeMap<String, i64>> = h
        .by_hour(None, GroupKey::Profile, local_ts(2026, 11, 25, 21, 0))
        .unwrap()
        .into_iter()
        .collect();
    assert_eq!(hours.len(), 24);
    assert_eq!(hours["18"], BTreeMap::from([("Gra".to_string(), 600)]));
    assert_eq!(hours["19"], BTreeMap::from([("Gra".to_string(), 600)]));
    assert!(hours["20"].is_empty());
}

// --- dodatkowe: tracker ------------------------------------------------------------------

#[test]
fn other_treadmill_mode_or_flush() {
    let h = mem();
    let (mut w, mut t) = Walker::new(1_800_000_000.0);
    t.set_owner(&h, "p1", "A", "g1", "PEAK").unwrap();
    let ks = SessionSource {
        model: "KS-F0".into(),
        driver: "kingsmith".into(),
        app_version: "0.6.0".into(),
        ..SessionSource::default()
    };
    t.set_source(&h, ks.clone()).unwrap();
    let steps = w.walk(&mut t, &h, 30, 0);
    assert_eq!(h.count().unwrap(), 1); // pierwszy zapis od razu (saved_at = 0)
    // firmware odczytany później uzupełnia sesję, bez nowej sesji
    let id = t.current().unwrap().id.clone();
    t.set_source(
        &h,
        SessionSource {
            firmware: "1.2".into(),
            ..ks.clone()
        },
    )
    .unwrap();
    assert_eq!(t.current().unwrap().id, id);
    assert_eq!(t.current().unwrap().firmware, "1.2");
    t.flush(&h).unwrap();
    let saved = h.session(&id).unwrap().unwrap();
    assert_eq!(saved.firmware, "1.2");
    assert_eq!(saved.device_model, "KS-F0");
    assert_eq!(saved.driver, "kingsmith");
    assert_eq!(saved.mode, "gra");
    // symulator to nowa sesja
    t.set_source(
        &h,
        SessionSource {
            source: "symulator".into(),
            ..SessionSource::default()
        },
    )
    .unwrap();
    assert!(t.current().is_none());
    let steps = w.walk(&mut t, &h, 30, steps);
    assert_eq!(t.current().unwrap().source, "symulator");
    // tryb Trening to nowa sesja
    t.set_mode(&h, "trening").unwrap();
    assert!(t.current().is_none());
    w.walk(&mut t, &h, 30, steps);
    assert_eq!(t.current().unwrap().mode, "trening");
    t.finish(&h).unwrap();
    assert_eq!(h.count().unwrap(), 3);
    let modes: HashSet<String> = all(&h).into_iter().map(|s| s.mode).collect();
    assert_eq!(modes, HashSet::from(["gra".into(), "trening".into()]));
}

#[test]
fn saves_every_minute_and_not_without_person() {
    let h = mem();
    let (mut w, mut t) = Walker::new(1_800_000_000.0);
    // bez osoby nic nie trafia do historii
    w.walk(&mut t, &h, 90, 0);
    t.finish(&h).unwrap();
    assert_eq!(h.count().unwrap(), 0);
    t.set_owner(&h, "p1", "A", "g1", "PEAK").unwrap();
    // zapis minutę po poprzednim (finish), nie wcześniej
    let steps = w.walk(&mut t, &h, 50, 0);
    assert!(h.session(&t.current().unwrap().id).unwrap().is_none());
    let steps = w.walk(&mut t, &h, 20, steps);
    let first = h.session(&t.current().unwrap().id).unwrap().unwrap();
    let steps = w.walk(&mut t, &h, 20, steps);
    // przed upływem minuty od zapisu baza ma starą wersję
    let mid = h.session(&first.id).unwrap().unwrap();
    assert_eq!(mid.end, first.end);
    w.walk(&mut t, &h, 50, steps);
    let later = h.session(&first.id).unwrap().unwrap();
    assert!(later.end > first.end);
}

// --- dodatkowe: konto online, poziomy ----------------------------------------------------

#[test]
fn unsent_sent_rejected_and_hidden() {
    let h = mem();
    let mut a = row("a", "p1", "A", "g1", "PEAK", 10.0, 20.0, 10, 0.1, 10.0);
    let mut b = row("b", "p1", "A", "g1", "PEAK", 30.0, 40.0, 10, 0.1, 10.0);
    let mut sim = row("c", "p1", "A", "g1", "PEAK", 50.0, 60.0, 10, 0.1, 10.0);
    sim.source = "symulator".into();
    for s in [&mut a, &mut b, &mut sim] {
        h.save(s).unwrap();
    }
    let ids = |v: Vec<Session>| v.into_iter().map(|s| s.id).collect::<Vec<_>>();
    assert_eq!(ids(h.unsent("p1", 100.0).unwrap()), ["a", "b"]);
    h.mark_sent(&[h.session("a").unwrap().unwrap()]).unwrap();
    assert!(h.was_sent("a").unwrap());
    let b_saved = h.session("b").unwrap().unwrap();
    h.mark_rejected(&[(&b_saved, &"x".repeat(300))], 1000.0)
        .unwrap();
    assert_eq!(h.rejected().unwrap()["b"].chars().count(), 200);
    assert!(h.unsent("p1", 100.0).unwrap().is_empty());
    assert_eq!(ids(h.unsent("p1", 1000.0).unwrap()), ["b"]); // termin ponowienia minął
    h.set_hidden_online("a", true).unwrap();
    assert_eq!(h.hidden_online().unwrap(), HashSet::from(["a".to_string()]));
    assert!(!h.was_sent("a").unwrap()); // ukrycie wysyła sesję jeszcze raz
    h.mark_sent(&[b_saved]).unwrap();
    assert!(h.rejected().unwrap().is_empty());
    h.forget_sent().unwrap();
    assert!(!h.was_sent("b").unwrap());
    h.delete("a").unwrap();
    assert!(h.session("a").unwrap().is_none());
    assert!(h.hidden_online().unwrap().is_empty());
}

#[test]
fn earned_and_shown_level() {
    let h = mem();
    assert_eq!(h.shown_level("p1").unwrap(), None);
    h.set_shown_level("p1", 3).unwrap();
    h.set_shown_level("p1", 4).unwrap();
    assert_eq!(h.shown_level("p1").unwrap(), Some(4));
    h.add_earned("p1", &HashMap::from([("x".into(), 5.0)]), 1)
        .unwrap();
    h.add_earned(
        "p1",
        &HashMap::from([("x".into(), 9.0), ("y".into(), 6.0)]),
        2,
    )
    .unwrap();
    assert_eq!(
        h.earned("p1").unwrap(),
        HashMap::from([("x".into(), 5.0), ("y".into(), 6.0)])
    );
    assert!(h.earned("p2").unwrap().is_empty());
    h.clear(None).unwrap();
    assert!(h.earned("p1").unwrap().is_empty());
    assert_eq!(h.shown_level("p1").unwrap(), None);
}

#[test]
fn ranges_since_and_chart() {
    let tz = FixedOffset::east_opt(3600).unwrap();
    let now = tz
        .with_ymd_and_hms(2026, 11, 25, 21, 0, 0)
        .unwrap()
        .timestamp() as f64;
    let midnight = |d: u32| {
        tz.with_ymd_and_hms(2026, 11, d, 0, 0, 0)
            .unwrap()
            .timestamp() as f64
    };
    assert_eq!(HistoryRange::Day.since_in(&tz, now).unwrap(), midnight(25));
    assert_eq!(HistoryRange::Week.since_in(&tz, now).unwrap(), midnight(19));
    let month = tz
        .with_ymd_and_hms(2026, 10, 27, 0, 0, 0)
        .unwrap()
        .timestamp() as f64;
    assert_eq!(HistoryRange::Month.since_in(&tz, now).unwrap(), month);
    assert_eq!(HistoryRange::All.since_in(&tz, now).unwrap(), 0.0);
    assert_eq!(HistoryRange::Day.chart_days(), 7);
    assert_eq!(HistoryRange::Week.chart_days(), 7);
    assert_eq!(HistoryRange::Month.chart_days(), 30);
    assert_eq!(HistoryRange::All.chart_days(), 30);
    assert!(HistoryRange::Day.hourly() && !HistoryRange::Week.hourly());
    let r: HistoryRange = serde_json::from_str("\"month\"").unwrap();
    assert_eq!(r, HistoryRange::Month);
}

#[test]
fn open_creates_folder_and_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = History::default_path(dir.path());
    {
        let h = History::open(&path).unwrap();
        let mut s = row("a", "p1", "A", "g1", "PEAK", 1.0, 2.0, 10, 0.1, 60.0);
        h.save(&mut s).unwrap();
    }
    let h = History::open(&path).unwrap();
    assert_eq!(h.schema_version().unwrap(), SCHEMA_VERSION);
    assert_eq!(h.count().unwrap(), 1);
    assert!(path.ends_with(Path::new(DB_FILE)));
}

// --- zgodność z bazami z Pythona ---------------------------------------------------------

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/history")
}

fn load_json(name: &str) -> Value {
    let text = std::fs::read_to_string(fixtures().join(name)).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// Otwiera kopię bazy z fixtury (migracja i WAL nie zmieniają plików w repozytorium).
fn open_copy(name: &str) -> (tempfile::TempDir, History) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("historia.db");
    std::fs::copy(fixtures().join(name), &path).unwrap();
    let h = History::open(&path).unwrap();
    (dir, h)
}

/// Porównanie JSON z tolerancją liczb (serde_json bez float_roundtrip bywa o 1 ulp obok).
fn assert_close(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(e)) => {
            let (a, e) = (a.as_f64().unwrap(), e.as_f64().unwrap());
            assert!(
                (a - e).abs() <= 1e-9 * e.abs().max(1.0),
                "{path}: {a} != {e}"
            );
        }
        (Value::Array(a), Value::Array(e)) => {
            assert_eq!(a.len(), e.len(), "{path}: długość {actual} != {expected}");
            for (i, (x, y)) in a.iter().zip(e).enumerate() {
                assert_close(x, y, &format!("{path}[{i}]"));
            }
        }
        (Value::Object(a), Value::Object(e)) => {
            let mut ka: Vec<&String> = a.keys().collect();
            let mut ke: Vec<&String> = e.keys().collect();
            ka.sort();
            ke.sort();
            assert_eq!(ka, ke, "{path}: klucze");
            for (k, y) in e {
                assert_close(&a[k], y, &format!("{path}.{k}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

fn cell(v: ValueRef) -> Value {
    match v {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(n) => json!(n),
        ValueRef::Real(f) => json!(f),
        ValueRef::Text(t) | ValueRef::Blob(t) => json!(String::from_utf8_lossy(t)),
    }
}

fn rows_json(h: &History, sql: &str) -> Value {
    let mut st = h.conn().prepare(sql).unwrap();
    let n = st.column_count();
    let rows = st
        .query_map([], |r| {
            Ok(Value::Array(
                (0..n).map(|i| cell(r.get_ref(i).unwrap())).collect(),
            ))
        })
        .unwrap();
    Value::Array(rows.map(|r| r.unwrap()).collect())
}

fn table_info(h: &History) -> Value {
    let names = rows_json(
        h,
        "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name",
    );
    let mut out = serde_json::Map::new();
    for n in names.as_array().unwrap() {
        let n = n[0].as_str().unwrap();
        let info = rows_json(
            h,
            &format!(
                "SELECT name, type, \"notnull\", dflt_value, pk FROM pragma_table_info('{n}')"
            ),
        );
        out.insert(n.to_string(), info);
    }
    Value::Object(out)
}

fn dump_tables(h: &History) -> Value {
    let hidden: Vec<Value> = rows_json(h, "SELECT id FROM ukryte_online ORDER BY id")
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r[0].clone())
        .collect();
    json!({
        "wyslane": rows_json(h, "SELECT * FROM wyslane ORDER BY id"),
        "odrzucone": rows_json(h, "SELECT * FROM odrzucone ORDER BY id"),
        "ukryte_online": hidden,
        "osiagniecia": rows_json(h, "SELECT * FROM osiagniecia ORDER BY osoba, id"),
        "poziom_osoby": rows_json(h, "SELECT * FROM poziom_osoby ORDER BY osoba"),
    })
}

fn stamp(h: &History) -> Value {
    let v = h.version().unwrap();
    json!([v.count, v.steps, v.last_end])
}

fn ids(v: Vec<Session>) -> Value {
    json!(v.into_iter().map(|s| s.id).collect::<Vec<_>>())
}

#[test]
fn old_python_databases_migrate_without_loss() {
    let expected = load_json("migrated.json");
    for key in ["v0", "v2", "v3", "v4"] {
        let want = &expected[key];
        let (_dir, h) = open_copy(&format!("{key}.db"));
        assert_eq!(h.schema_version().unwrap(), SCHEMA_VERSION, "{key}");
        let got = json!({
            "schemaVersion": h.schema_version().unwrap(),
            "sessions": serde_json::to_value(all(&h)).unwrap(),
            "tables": table_info(&h),
            "data": dump_tables(&h),
            "version": stamp(&h),
        });
        assert_close(&got, want, key);
        // po migracji baza dalej działa: zapis i ponowne otwarcie
        let mut s = row("nowa", "p1", "Ala", "g1", "PEAK", 5.0, 6.0, 1, 0.0, 1.0);
        h.save(&mut s).unwrap();
        assert_eq!(h.session("nowa").unwrap().unwrap().mode, "gra");
    }
}

#[test]
fn fresh_schema_is_identical_to_python() {
    let want = load_json("v5.json");
    let h = mem();
    let sql = rows_json(
        &h,
        "SELECT sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY name",
    );
    let sql: Vec<Value> = sql
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r[0].clone())
        .collect();
    assert_eq!(Value::Array(sql), want["schemaSql"]);
    assert_close(&table_info(&h), &want["tables"], "tables");
}

#[test]
fn python_v5_database_gives_same_results() {
    let want = load_json("v5.json");
    let offsets: Vec<i64> = want["tzOffsetMin"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    assert!(
        offsets.iter().all(|o| *o == offsets[0]),
        "fixtura z oknem przez zmianę czasu: {offsets:?}"
    );
    let tz = FixedOffset::east_opt(i32::try_from(offsets[0] * 60).unwrap()).unwrap();
    let now = want["now"].as_f64().unwrap();
    let (_dir, h) = open_copy("v5.db");

    let since_week = HistoryRange::Week.since_in(&tz, now).unwrap();
    let mut hidden: Vec<String> = h.hidden_online().unwrap().into_iter().collect();
    hidden.sort();
    let earned: serde_json::Map<String, Value> = ["p1", "p2"]
        .iter()
        .map(|p| (p.to_string(), json!(h.earned(p).unwrap())))
        .collect();
    let shown: serde_json::Map<String, Value> = ["p1", "p2", "p3"]
        .iter()
        .map(|p| (p.to_string(), json!(h.shown_level(p).unwrap())))
        .collect();
    let sent: serde_json::Map<String, Value> = ["s1", "s2", "s6"]
        .iter()
        .map(|i| (i.to_string(), json!(h.was_sent(i).unwrap())))
        .collect();
    let g2: HashSet<String> = HashSet::from(["g2".into(), "g9".into()]);
    let g1: HashSet<String> = HashSet::from(["g1".into()]);
    let day = |n, p, k| serde_json::to_value(h.by_day_in(&tz, n, p, k, now).unwrap()).unwrap();
    let hour = |p, k| serde_json::to_value(h.by_hour_in(&tz, p, k, now).unwrap()).unwrap();
    let group = |s, p, k| serde_json::to_value(h.by_group(s, p, k).unwrap()).unwrap();
    let tot = |s, p| serde_json::to_value(h.totals(s, p).unwrap()).unwrap();
    let reject_at = now - 9000.0 + 7200.0; // dzień 18:30 + 2 h (jak w skrypcie)

    let got = json!({
        "now": now,
        "tzOffsetMin": want["tzOffsetMin"],
        "schemaVersion": h.schema_version().unwrap(),
        "count": h.count().unwrap(),
        "version": stamp(&h),
        "schemaSql": want["schemaSql"],
        "tables": table_info(&h),
        "data": dump_tables(&h),
        "rejected": h.rejected().unwrap(),
        "hiddenOnline": hidden,
        "earned": earned,
        "shownLevel": shown,
        "wasSent": sent,
        "unsent": {
            "p1_now": ids(h.unsent("p1", now).unwrap()),
            "p1_later": ids(h.unsent("p1", reject_at).unwrap()),
            "p2_now": ids(h.unsent("p2", now).unwrap()),
        },
        "sessions": {
            "all": serde_json::to_value(all(&h)).unwrap(),
            "week_p1": ids(h.sessions(since_week, Some("p1"), None, None).unwrap()),
            "limit3": ids(h.sessions(0.0, None, None, Some(3)).unwrap()),
            "profiles_g2": ids(h.sessions(0.0, None, Some(&g2), None).unwrap()),
            "profiles_limit": ids(h.sessions(0.0, Some("p1"), Some(&g1), Some(4)).unwrap()),
        },
        "totals": {
            "all": tot(0.0, None),
            "p1": tot(0.0, Some("p1")),
            "week_p2": tot(since_week, Some("p2")),
        },
        "byDay": {
            "7_profile": day(7, None, GroupKey::Profile),
            "7_person": day(7, None, GroupKey::Person),
            "30_p1": day(30, Some("p1"), GroupKey::Profile),
            "1_p2": day(1, Some("p2"), GroupKey::Profile),
        },
        "byHour": {
            "profile": hour(None, GroupKey::Profile),
            "person": hour(None, GroupKey::Person),
            "p2": hour(Some("p2"), GroupKey::Profile),
        },
        "byGroup": {
            "all_profile": group(0.0, None, GroupKey::Profile),
            "all_person": group(0.0, None, GroupKey::Person),
            "week_p1": group(since_week, Some("p1"), GroupKey::Profile),
        },
    });
    assert_close(&got, &want, "v5");
}

// --- dodatkowe po przeglądzie: sumy, tracker z Pythona, zmiana czasu, zaokrąglanie ----

#[test]
fn sums_match_python_bit_for_bit() {
    let want = load_json("sums.json");
    let cases = want["cases"].as_array().unwrap();
    assert!(cases.len() > 100);
    for (i, c) in cases.iter().enumerate() {
        let xs: Vec<f64> = c["in"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| f64::from_bits(b.as_u64().unwrap()))
            .collect();
        let got = py_sum(xs.iter().copied());
        let exp = f64::from_bits(c["sum"].as_u64().unwrap());
        assert_eq!(
            got.to_bits(),
            exp.to_bits(),
            "przypadek {i}: {got} != {exp}"
        );
    }
    // ta sama suma w totals (0.1 * 10 = 1.0 jak w Pythonie, zwykłe dodawanie daje 0.999...)
    let h = mem();
    for i in 0..10 {
        let mut s = row(
            &format!("s{i}"),
            "p1",
            "A",
            "g1",
            "PEAK",
            f64::from(i),
            1e3,
            1,
            0.1,
            0.1,
        );
        h.save(&mut s).unwrap();
    }
    let t = h.totals(0.0, None).unwrap();
    assert_eq!(t.distance_km, 1.0);
    assert_eq!(t.active_s, 1.0);
    assert_eq!(h.by_group(0.0, None, GroupKey::Profile).unwrap()[0].1, t);
}

/// Zdarzenia SessionTracker z Pythona odtworzone w Rust: stan po każdym i zapisane sesje.
#[test]
fn tracker_replays_python_events() {
    let want = load_json("tracker.json");
    let events = want["events"].as_array().unwrap();
    let states = want["states"].as_array().unwrap();
    assert_eq!(events.len(), states.len());
    let h = mem();
    let (w, mut t) = Walker::new(0.0);
    let f = |v: &Value| v.as_f64().unwrap();
    let st = |v: &Value| v.as_str().unwrap().to_string();
    for (i, (e, state)) in events.iter().zip(states).enumerate() {
        *w.t.lock().unwrap() = f(&e["t"]);
        match e["op"].as_str().unwrap() {
            "update" => t
                .update(
                    &h,
                    f(&e["speed"]),
                    e["steps"].as_i64(),
                    f(&e["dist"]),
                    f(&e["act"]),
                )
                .unwrap(),
            "owner" => {
                let a = e["args"].as_array().unwrap();
                t.set_owner(&h, &st(&a[0]), &st(&a[1]), &st(&a[2]), &st(&a[3]))
                    .unwrap();
            }
            "source" => {
                let s = &e["src"];
                let src = SessionSource {
                    model: st(&s["model"]),
                    driver: st(&s["driver"]),
                    profile: st(&s["profile"]),
                    firmware: st(&s["firmware"]),
                    app_version: st(&s["app_version"]),
                    source: st(&s["source"]),
                };
                t.set_source(&h, src).unwrap();
            }
            "mode" => t.set_mode(&h, e["mode"].as_str().unwrap()).unwrap(),
            "flush" => t.flush(&h).unwrap(),
            "finish" => t.finish(&h).unwrap(),
            other => panic!("nieznane zdarzenie {other}"),
        }
        let got = match t.current() {
            None => json!([h.count().unwrap()]),
            Some(s) => json!([
                h.count().unwrap(),
                s.steps,
                s.distance_km,
                s.active_s,
                s.start,
                s.end,
                s.firmware
            ]),
        };
        assert_close(&got, state, &format!("zdarzenie {i} ({})", e["op"]));
    }
    let mut saved = all(&h);
    saved.sort_by(|a, b| a.start.total_cmp(&b.start));
    for s in &saved {
        assert_eq!(s.id.len(), 32);
        assert_eq!(s.tz_offset_min, utc_offset_min(s.start));
    }
    let got: Vec<Value> = saved
        .into_iter()
        .map(|s| {
            let mut v = serde_json::to_value(s).unwrap();
            let m = v.as_object_mut().unwrap();
            for k in ["id", "updated", "tzOffsetMin"] {
                m.remove(k);
            }
            v
        })
        .collect();
    assert_close(&Value::Array(got), &want["sessions"], "sesje");
}

/// Strefa z jedną zmianą czasu w chwili `at` (UTC): przed nią `pre`, od niej `post`.
#[derive(Debug, Clone, Copy)]
struct OneShift {
    pre: FixedOffset,
    post: FixedOffset,
    at: i64,
}

impl OneShift {
    fn new(pre_h: i32, post_h: i32, at: i64) -> Self {
        Self {
            pre: FixedOffset::east_opt(pre_h * 3600).unwrap(),
            post: FixedOffset::east_opt(post_h * 3600).unwrap(),
            at,
        }
    }

    fn at_utc(&self, utc: i64) -> FixedOffset {
        if utc < self.at { self.pre } else { self.post }
    }
}

impl TimeZone for OneShift {
    type Offset = FixedOffset;

    fn from_offset(offset: &FixedOffset) -> Self {
        Self {
            pre: *offset,
            post: *offset,
            at: 0,
        }
    }

    fn offset_from_local_date(&self, local: &NaiveDate) -> LocalResult<FixedOffset> {
        self.offset_from_local_datetime(&local.and_time(NaiveTime::MIN))
    }

    fn offset_from_local_datetime(
        &self,
        local: &chrono::NaiveDateTime,
    ) -> LocalResult<FixedOffset> {
        let l = local.and_utc().timestamp();
        let mut ok: Vec<(i64, FixedOffset)> = [self.pre, self.post]
            .into_iter()
            .map(|o| (l - i64::from(o.local_minus_utc()), o))
            .filter(|(u, o)| self.at_utc(*u) == *o)
            .collect();
        ok.sort_by_key(|(u, _)| *u);
        ok.dedup();
        match ok.as_slice() {
            [] => LocalResult::None,
            [(_, o)] => LocalResult::Single(*o),
            [(_, a), (_, b), ..] => LocalResult::Ambiguous(*a, *b),
        }
    }

    fn offset_from_utc_date(&self, utc: &NaiveDate) -> FixedOffset {
        self.at_utc(utc.and_time(NaiveTime::MIN).and_utc().timestamp())
    }

    fn offset_from_utc_datetime(&self, utc: &chrono::NaiveDateTime) -> FixedOffset {
        self.at_utc(utc.and_utc().timestamp())
    }
}

/// Zmiana czasu o północy: jak datetime.combine(dzień, 0:00).timestamp() w Pythonie (fold=0).
#[test]
fn midnight_on_dst_change_days_like_python() {
    let utc = |d: u32, h: u32| {
        chrono::Utc
            .with_ymd_and_hms(2026, 11, d, h, 0, 0)
            .unwrap()
            .timestamp()
    };
    let day = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    // brak północy (UTC-3 -> UTC-2 o 0:00): północ to chwila zmiany, czyli 0:00 według UTC-3
    let gap_west = OneShift::new(-3, -2, utc(1, 3));
    assert_eq!(local_midnight(&gap_west, day), utc(1, 3) as f64);
    // brak północy (UTC+1 -> UTC+2 o 0:00)
    let gap_east = OneShift::new(1, 2, utc(1, 0) - 3600);
    assert_eq!(local_midnight(&gap_east, day), (utc(1, 0) - 3600) as f64);
    // podwójna północ (UTC+2 -> UTC+1 o 1:00): wcześniejsza z dwóch
    let twice = OneShift::new(2, 1, utc(1, 0) - 3600);
    assert_eq!(local_midnight(&twice, day), (utc(1, 0) - 7200) as f64);
    // zakres "dziś" w dniu zmiany zaczyna się od tej samej chwili
    let now = utc(1, 18) as f64;
    assert_eq!(
        HistoryRange::Day.since_in(&gap_west, now).unwrap(),
        utc(1, 3) as f64
    );
}

/// datetime.fromtimestamp zaokrągla do mikrosekund połówkami do parzystej.
#[test]
fn timestamp_rounding_like_python() {
    use chrono::Datelike;
    let midnight = chrono::Utc
        .with_ymd_and_hms(2026, 11, 26, 0, 0, 0)
        .unwrap()
        .timestamp() as f64;
    let day = |ts: f64| local_date(&chrono::Utc, ts).unwrap().day();
    assert_eq!(day(midnight - 4e-7), 26); // 23:59:59.9999996 -> 0:00:00.000000
    assert_eq!(day(midnight - 1.5e-6), 25);
    assert_eq!(day(-0.25), 31); // 1969-12-31 23:59:59.75
    assert!(local_date(&chrono::Utc, f64::NAN).is_err());
    assert!(local_date(&chrono::Utc, 1e300).is_err());
}

#[test]
fn by_day_edge_counts_return_errors_not_panics() {
    let h = mem();
    let tz = FixedOffset::east_opt(3600).unwrap();
    let day = |n| h.by_day_in(&tz, n, None, GroupKey::Profile, 1e9);
    assert!(day(0).unwrap().is_empty());
    assert!(day(-5).unwrap().is_empty());
    assert!(matches!(day(i64::MAX), Err(HistoryError::Days(_))));
    let days = day(3).unwrap();
    let names: Vec<&str> = days.iter().map(|(d, _)| d.as_str()).collect();
    assert_eq!(names, ["2001-09-07", "2001-09-08", "2001-09-09"]);
    assert!(
        h.by_hour_in(&tz, None, GroupKey::Profile, f64::INFINITY)
            .is_err()
    );
}
