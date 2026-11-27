//! Zgodność modułu backup z Pythonem (bieznia/kopia.py): fixtury z tools/fixtures/kopia.py.
//!
//! Python -> Rust: kopie zrobione przez 0.5.x czytane w Rust (wynik jak kopia.read), eksport Rust
//! z tych samych danych daje te same pliki w archiwum, importy dają ten sam stan.
//! Rust -> Python: from-rust.bieznia (test `generate_rust_copy`, uruchamiany ręcznie z --ignored)
//! przeczytana i zaimportowana przez Pythona (from-rust-read.json).

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use walklevel_core::backup::{self, ExportChoice, ImportChoice, ImportReport, Preview};
use walklevel_core::history::{History, new_session};
use walklevel_core::store::{NewProfile, Store};

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/backup")
}

/// Kopie z 0.5.x. Wyniki z Pythona (read.json, import.json) podają je pod dawnymi nazwami plików.
fn python_copy(name: &str) -> PathBuf {
    let file = match name {
        "pelna.bieznia" => "full.bieznia",
        "wyniki.bieznia" => "results.bieznia",
        other => other,
    };
    fixtures_dir().join(file)
}

fn load(name: &str) -> Value {
    let text = fs::read_to_string(fixtures_dir().join(name)).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// Podgląd w tej samej postaci co preview_dict w skrypcie fixtur.
fn preview_json(p: &Preview) -> Value {
    let sessions: Vec<Value> = p
        .sessions
        .iter()
        .map(|s| serde_json::from_str(&backup::session_to_json(s).unwrap()).unwrap())
        .collect();
    let icons: Map<String, Value> = p
        .icons
        .iter()
        .map(|(k, v)| (k.clone(), Value::String(hex::encode(v))))
        .collect();
    json!({
        "appVersion": p.app_version,
        "hasSettings": p.has_settings,
        "hasResults": p.has_results,
        "describe": backup::describe(p),
        "people": serde_json::to_value(&p.people).unwrap(),
        "sessions": sessions,
        "icons": icons,
    })
}

/// Stan po imporcie bez losowych id (jak store_dict w skrypcie fixtur).
fn store_json(store: &Store) -> Value {
    let people: Vec<Value> = store
        .people()
        .iter()
        .map(|p| {
            let profiles: Vec<Value> = p
                .profiles()
                .iter()
                .map(|pr| {
                    let icon = pr
                        .icon_file
                        .as_ref()
                        .map(|f| hex::encode(fs::read(store.icons_dir().join(f)).unwrap()));
                    json!({"name": pr.name, "color": pr.color, "glyph": pr.glyph,
                           "settings": pr.settings, "icon": icon})
                })
                .collect();
            json!({"name": p.name, "color": p.color, "profiles": profiles})
        })
        .collect();
    json!({"active": store.person().name, "people": people})
}

fn report_json(r: &ImportReport) -> Value {
    json!({
        "peopleAdded": r.people_added,
        "peopleMerged": r.people_merged,
        "profilesAdded": r.profiles_added,
        "profilesUpdated": r.profiles_updated,
        "sessionsAdded": r.sessions_added,
    })
}

fn all_ids(p: &Preview) -> HashSet<String> {
    p.people.iter().map(|x| x.id.clone()).collect()
}

/// Import całego pliku na „innym komputerze” (jak import_into_fresh w skrypcie).
fn import_into_fresh(file: &Path) -> Value {
    let dir = tempfile::tempdir().unwrap();
    let mut mine = Store::create(
        &Store::default_path(&dir.path().join("dane")),
        "Cicha Tropicielka",
        "Fiolet",
        None,
    )
    .unwrap();
    let hp = History::default_path(dir.path());
    let history = History::open(&hp).unwrap();
    let preview = backup::read(file).unwrap();
    let choice = ImportChoice::new(all_ids(&preview));
    let report = backup::apply_import(&mut mine, &history, &preview, &choice, &hp).unwrap();
    json!({"report": report_json(&report), "store": store_json(&mine),
           "sessions": history.count().unwrap()})
}

#[test]
fn python_copies_read_like_python() {
    let expected = load("read.json");
    for (name, want) in expected.as_object().unwrap() {
        let preview = backup::read(&python_copy(name)).unwrap();
        assert_eq!(&preview_json(&preview), want, "{name}");
    }
}

#[test]
fn python_copies_import_like_python() {
    let expected = load("import.json");
    for (name, want) in expected["fresh"].as_object().unwrap() {
        assert_eq!(&import_into_fresh(&python_copy(name)), want, "{name}");
    }
}

/// Dane źródłowe skryptu (te same id): osoby.json z Pythona, ikony i sesje z pełnej kopii.
struct Source {
    store: Store,
    history: History,
    // dropped last: Windows does not delete a folder with the database still open
    dir: tempfile::TempDir,
}

fn python_source() -> Source {
    let dir = tempfile::tempdir().unwrap();
    let path = Store::default_path(&dir.path().join("dane"));
    let raw = load("source-people.json");
    let store = Store::from_json(&path, raw.as_object().unwrap()).unwrap();
    store.save().unwrap();
    let full = backup::read(&fixtures_dir().join("full.bieznia")).unwrap();
    fs::create_dir_all(store.icons_dir()).unwrap();
    for (name, data) in &full.icons {
        fs::write(store.icons_dir().join(name), data).unwrap();
    }
    let history = History::open(&History::default_path(dir.path())).unwrap();
    assert_eq!(history.merge(&full.sessions).unwrap(), 3);
    let logs = dir.path().join("logs");
    fs::create_dir_all(&logs).unwrap();
    fs::write(
        logs.join("bieznia.log"),
        "2026-11-26 10:00:00 INFO    bieznia: start\n",
    )
    .unwrap();
    Source {
        dir,
        store,
        history,
    }
}

fn entries(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut zip = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
    let names: Vec<String> = zip.file_names().map(|n| n.unwrap().into_owned()).collect();
    names
        .into_iter()
        .map(|n| {
            let mut buf = Vec::new();
            zip.by_name(&n).unwrap().read_to_end(&mut buf).unwrap();
            (n, buf)
        })
        .collect()
}

fn json_of(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap()
}

#[test]
fn rust_export_matches_python_export() {
    let src = python_source();
    let everyone: BTreeMap<String, Option<HashSet<String>>> = src
        .store
        .people()
        .iter()
        .map(|p| (p.id.clone(), None))
        .collect();
    let logs = src.dir.path().join("logs");
    for (name, choice) in [
        ("full.bieznia", ExportChoice::new(everyone.clone())),
        (
            "results.bieznia",
            ExportChoice {
                settings: false,
                logs: true,
                ..ExportChoice::new(everyone.clone())
            },
        ),
    ] {
        let out = src.dir.path().join(name);
        backup::export(&src.store, Some(&src.history), &choice, &out, &logs).unwrap();
        let mine = entries(&out);
        let python = entries(&fixtures_dir().join(name));
        let names = |m: &BTreeMap<String, Vec<u8>>| m.keys().cloned().collect::<BTreeSet<_>>();
        assert_eq!(names(&mine), names(&python), "{name}");
        // wiersze historii bajt w bajt (json.dumps), ikony i logi bez zmian
        for (entry, data) in &python {
            if entry.ends_with(".json") {
                continue;
            }
            assert_eq!(
                String::from_utf8_lossy(&mine[entry]),
                String::from_utf8_lossy(data),
                "{name}: {entry}"
            );
        }
        assert_eq!(
            json_of(&mine["osoby.json"]),
            json_of(&python["osoby.json"]),
            "{name}"
        );
        let mut m1 = json_of(&mine["manifest.json"]);
        let mut m2 = json_of(&python["manifest.json"]);
        for m in [&mut m1, &mut m2] {
            let o = m.as_object_mut().unwrap();
            o.remove("utworzono");
            o.remove("aplikacja");
        }
        assert_eq!(m1, m2, "{name}");
    }
}

#[test]
fn own_imports_match_python() {
    let mut src = python_source();
    let a_id = src.store.people()[0].id.clone();
    let cp = src.store.people()[0]
        .profiles()
        .iter()
        .find(|p| p.name == "Cyberpunk 2077")
        .unwrap()
        .id
        .clone();
    src.store.switch_profile(&cp).unwrap();
    src.store.set("walk_speed", json!(9.9)).unwrap();
    let preview = backup::read(&fixtures_dir().join("full.bieznia")).unwrap();
    let hp = History::default_path(src.dir.path());
    let only_a = HashSet::from([a_id.clone()]);
    let expected = load("import.json");
    for want in expected["own"].as_array().unwrap() {
        let choice = match want["label"].as_str().unwrap() {
            "keep" => ImportChoice {
                keep_my_settings: true,
                ..ImportChoice::new(only_a.clone())
            },
            "replace" => ImportChoice::new(only_a.clone()),
            _ => ImportChoice {
                new_person: only_a.clone(),
                ..ImportChoice::new(only_a.clone())
            },
        };
        let r = backup::apply_import(&mut src.store, &src.history, &preview, &choice, &hp).unwrap();
        let got = json!({"label": want["label"], "report": report_json(&r),
                         "store": store_json(&src.store), "sessions": src.history.count().unwrap()});
        assert_eq!(&got, want);
    }
}

#[test]
fn rust_copy_read_by_python() {
    let file = fixtures_dir().join("from-rust.bieznia");
    let expected = load("from-rust-read.json");
    let preview = backup::read(&file).unwrap();
    assert_eq!(preview_json(&preview), expected["read"]);
    assert_eq!(import_into_fresh(&file), expected["import"]);
}

/// Tworzy tests/fixtures/backup/from-rust.bieznia (potem skrypt Pythona: from-rust-read.json).
#[test]
#[ignore = "generuje fixturę dla skryptu Pythona"]
fn generate_rust_copy() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("dane");
    let legacy = json!({"walk_speed": 1.7, "theme": "light", "large_text": true});
    let mut store = Store::create(
        &Store::default_path(&data),
        "Żwawa Łucja",
        "Malina",
        Some(legacy.as_object().unwrap()),
    )
    .unwrap();
    let default_id = store.profile().id.clone();
    fs::create_dir_all(store.icons_dir()).unwrap();
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend_from_slice(b"ikona-wiedzmin");
    fs::write(store.icons_dir().join("w3.png"), &png).unwrap();
    let w3 = store
        .add_profile(
            "Wiedźmin 3",
            NewProfile {
                icon_file: Some("w3.png".into()),
                color: Some("Banan".into()),
                copy_from: Some(default_id.clone()),
                ..NewProfile::default()
            },
        )
        .unwrap()
        .id
        .clone();
    store.set("walk_speed", json!(2.75)).unwrap();
    let a = store.person().clone();
    let b = store
        .add_person("Ósma Gęś", "Orchidea", None)
        .unwrap()
        .clone();
    let history = History::open(&History::default_path(dir.path())).unwrap();
    let mut s1 = new_session(
        &walklevel_core::history::uuid7(),
        &a.id,
        &a.name,
        &w3,
        "Wiedźmin 3",
        1764300000.5,
        1764301200.25,
    );
    s1.steps = 1500;
    s1.distance_km = 1.125;
    s1.active_s = 1180.0;
    s1.device_model = "KS-NACH-X21C".into();
    s1.driver = "kingsmith".into();
    s1.visibility = "znajomi".into();
    s1.tz_offset_min = 60;
    s1.app_version = "0.6.0".into();
    let mut s2 = new_session(
        &walklevel_core::history::uuid7(),
        &b.id,
        &b.name,
        &b.profiles()[0].id,
        "Domyślny",
        1764400000.0,
        1764400300.0,
    );
    s2.steps = 400;
    s2.distance_km = 0.2;
    s2.active_s = 290.0;
    s2.source = "symulator".into();
    s2.mode = "trening".into();
    for s in [&mut s1, &mut s2] {
        history.save(s).unwrap();
    }
    let everyone = store
        .people()
        .iter()
        .map(|p| (p.id.clone(), None))
        .collect();
    let out = fixtures_dir().join("from-rust.bieznia");
    backup::export(
        &store,
        Some(&history),
        &ExportChoice::new(everyone),
        &out,
        &dir.path().join("logs"),
    )
    .unwrap();
}
