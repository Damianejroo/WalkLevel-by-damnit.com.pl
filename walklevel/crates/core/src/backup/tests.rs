//! Kopia danych: eksport z filtrami, sprawdzanie pliku, import, przywracanie (tests/test_kopia.py)
//! oraz format wierszy historia.jsonl i odczyt kopii z 0.3.x.

use super::*;
use crate::history::new_session;
use crate::store::NewProfile;
use serde_json::json;
use tempfile::TempDir;

fn png() -> Vec<u8> {
    [PNG_MAGIC, b"ikona"].concat()
}

/// Folder danych testu: `dane/osoby.json`, `dane/historia.db`, `dane/ikony`, `logs`.
struct World {
    dir: TempDir,
    store: Store,
    history: History,
}

impl World {
    fn root(&self) -> &Path {
        self.dir.path()
    }
    fn history_path(&self) -> PathBuf {
        History::default_path(self.root())
    }
}

#[allow(clippy::too_many_arguments)]
fn session(
    id: &str,
    person: &Person,
    profile: &str,
    profile_name: &str,
    start: f64,
    end: f64,
    steps: i64,
    km: f64,
    active: f64,
) -> Session {
    let mut s = new_session(
        id,
        &person.id,
        &person.name,
        profile,
        profile_name,
        start,
        end,
    );
    s.steps = steps;
    s.distance_km = km;
    s.active_s = active;
    s
}

fn world() -> World {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("dane");
    let legacy = json!({"walk_speed": 1.5}).as_object().unwrap().clone();
    let mut store = Store::create(
        &Store::default_path(&data),
        "Szalony Biegacz",
        "Morski",
        Some(&legacy),
    )
    .unwrap();
    let default_id = store.profile().id.clone();
    fs::create_dir_all(store.icons_dir()).unwrap();
    fs::write(store.icons_dir().join("cp.png"), png()).unwrap();
    let cp = store
        .add_profile(
            "Cyberpunk 2077",
            NewProfile {
                icon_file: Some("cp.png".into()),
                color: Some("Banan".into()),
                copy_from: Some(default_id.clone()),
                ..NewProfile::default()
            },
        )
        .unwrap()
        .id
        .clone();
    store.set("walk_speed", json!(2.2)).unwrap();
    store
        .add_profile(
            "Stardew Valley",
            NewProfile {
                copy_from: Some(default_id.clone()),
                ..NewProfile::default()
            },
        )
        .unwrap();
    let a = store.person().clone();
    let b = store
        .add_person("Nocna Wędrowniczka", "Orchidea", None)
        .unwrap()
        .clone();
    let history = History::open(&History::default_path(dir.path())).unwrap();
    let b_profile = b.profiles()[0].id.clone();
    for mut s in [
        session(
            "s1",
            &a,
            &cp,
            "Cyberpunk 2077",
            100.0,
            700.0,
            1000,
            0.5,
            600.0,
        ),
        session(
            "s2",
            &a,
            &default_id,
            "Domyślny",
            50.0,
            90.0,
            30,
            0.01,
            40.0,
        ),
        session(
            "s3",
            &b,
            &b_profile,
            "Domyślny",
            10.0,
            610.0,
            900,
            0.4,
            600.0,
        ),
    ] {
        history.save(&mut s).unwrap();
    }
    store.switch_person(&a.id).unwrap();
    World {
        dir,
        store,
        history,
    }
}

fn ids(store: &Store, person: &str) -> HashMap<String, String> {
    store
        .people()
        .iter()
        .find(|p| p.name == person)
        .unwrap()
        .profiles()
        .iter()
        .map(|p| (p.name.clone(), p.id.clone()))
        .collect()
}

fn all_people(store: &Store) -> BTreeMap<String, Option<HashSet<String>>> {
    store
        .people()
        .iter()
        .map(|p| (p.id.clone(), None))
        .collect()
}

fn zip_names(path: &Path) -> HashSet<String> {
    let zip = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
    zip.file_names().map(|n| n.unwrap().into_owned()).collect()
}

fn zip_text(path: &Path, name: &str) -> String {
    let mut zip = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
    String::from_utf8(entry(&mut zip, name).unwrap()).unwrap()
}

fn full_export(w: &World, name: &str) -> PathBuf {
    let out = w.root().join(name);
    let choice = ExportChoice::new(all_people(&w.store));
    export(
        &w.store,
        Some(&w.history),
        &choice,
        &out,
        &w.root().join("logs"),
    )
    .unwrap();
    out
}

#[test]
fn export_respects_person_profile_and_content_filters() {
    let w = world();
    let a = &w.store.people()[0];
    let cp = ids(&w.store, &a.name)["Cyberpunk 2077"].clone();
    let choice = ExportChoice::new(BTreeMap::from([(a.id.clone(), Some(HashSet::from([cp])))]));
    let out = w.root().join("kopia.bieznia");
    let summary = export(
        &w.store,
        Some(&w.history),
        &choice,
        &out,
        &w.root().join("logs"),
    )
    .unwrap();
    assert_eq!(
        (summary.people, summary.profiles, summary.sessions),
        (1, 1, 1)
    );
    assert_eq!(summary.bytes, fs::metadata(&out).unwrap().len());
    let names = zip_names(&out);
    for n in [
        "manifest.json",
        "osoby.json",
        "historia.jsonl",
        "ikony/cp.png",
    ] {
        assert!(names.contains(n), "{n}");
    }
    let people: Value = serde_json::from_str(&zip_text(&out, "osoby.json")).unwrap();
    let profiles: Vec<&str> = people["osoby"][0]["profile"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["nazwa"].as_str().unwrap())
        .collect();
    assert_eq!(profiles, ["Cyberpunk 2077"]);
    let lines: Vec<String> = zip_text(&out, "historia.jsonl")
        .lines()
        .map(String::from)
        .collect();
    assert_eq!(lines.len(), 1);
    assert_eq!(
        serde_json::from_str::<Value>(&lines[0]).unwrap()["kroki"],
        1000
    );
    assert!(!names.iter().any(|n| n.starts_with("logi/")));
    // podsumowanie na żywo daje te same liczby (bez rozmiaru)
    let live = summarize(&w.store, Some(&w.history), &choice).unwrap();
    assert_eq!(
        live,
        ExportSummary {
            bytes: 0,
            ..summary
        }
    );
}

#[test]
fn export_results_only_and_logs() {
    let w = world();
    let logs = w.root().join("logs");
    fs::create_dir_all(&logs).unwrap();
    fs::write(logs.join("bieznia.log"), "linia\n").unwrap();
    fs::write(logs.join("inne.txt"), "nie").unwrap();
    let choice = ExportChoice {
        settings: false,
        logs: true,
        ..ExportChoice::new(all_people(&w.store))
    };
    let out = w.root().join("wyniki.bieznia");
    let summary = export(&w.store, Some(&w.history), &choice, &out, &logs).unwrap();
    assert_eq!(
        (summary.people, summary.profiles, summary.sessions),
        (2, 0, 3)
    );
    let names = zip_names(&out);
    assert!(names.contains("logi/bieznia.log"));
    assert!(!names.contains("logi/inne.txt"));
    assert!(!names.iter().any(|n| n.starts_with("ikony/")));
    let people: Value = serde_json::from_str(&zip_text(&out, "osoby.json")).unwrap();
    // bez ustawień sterowania: profil to id, nazwa i kolor, pusty wygląd
    let first = people["osoby"][0]["profile"][0].as_object().unwrap();
    assert_eq!(first.keys().collect::<Vec<_>>(), ["id", "kolor", "nazwa"]);
    assert_eq!(people["osoby"][0]["wyglad"], json!({}));
    let err = export(
        &w.store,
        Some(&w.history),
        &ExportChoice::new(BTreeMap::new()),
        &out,
        &logs,
    )
    .unwrap_err();
    assert_eq!(err.to_string(), "Zaznacz co najmniej jedną osobę.");
    let nothing = ExportChoice {
        settings: false,
        results: false,
        ..ExportChoice::new(all_people(&w.store))
    };
    assert!(matches!(
        export(&w.store, Some(&w.history), &nothing, &out, &logs),
        Err(BackupError::Invalid(m)) if m.starts_with("Zaznacz, co zapisać")
    ));
}

#[test]
fn log_tail_is_limited() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("a.log");
    fs::write(&f, b"0123456789").unwrap();
    assert_eq!(tail(&f, 4).unwrap(), b"6789");
    assert_eq!(tail(&f, 100).unwrap(), b"0123456789");
}

fn write_zip(path: &Path, files: &[(&str, &[u8])]) {
    let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
    for (name, data) in files {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap();
}

fn message(r: Result<Preview, BackupError>) -> String {
    match r {
        Err(BackupError::Invalid(m)) => m,
        other => panic!("oczekiwany komunikat, jest {other:?}"),
    }
}

#[test]
fn read_rejects_foreign_and_broken_files() {
    let w = world();
    let good = read(&full_export(&w, "pelna.bieznia")).unwrap();
    assert_eq!((good.people.len(), good.sessions.len()), (2, 3));
    assert_eq!(good.icons.get("cp.png"), Some(&png()));
    assert!(good.has_settings && good.has_results);
    assert_eq!(good.sessions_of(&good.people[1].id).len(), 1);

    let manifest = |v: i64| json!({"format": FORMAT, "wersja": v}).to_string();
    let evil = w.root().join("zly.bieznia");
    write_zip(
        &evil,
        &[
            ("manifest.json", manifest(1).as_bytes()),
            ("../../windows/system32/x.dll", b"MZ"),
        ],
    );
    assert!(message(read(&evil)).contains("nieznane"));
    let newer = w.root().join("nowszy.bieznia");
    write_zip(
        &newer,
        &[
            ("manifest.json", manifest(99).as_bytes()),
            ("osoby.json", b"{}"),
        ],
    );
    assert!(message(read(&newer)).contains("nowszej"));
    let broken = w.root().join("zepsuty.bieznia");
    fs::write(&broken, b"to nie zip").unwrap();
    assert!(message(read(&broken)).contains("uszkodzony"));
    let foreign = w.root().join("obcy.bieznia");
    write_zip(
        &foreign,
        &[
            ("manifest.json", br#"{"format": "inny"}"#),
            ("osoby.json", b"{}"),
        ],
    );
    assert_eq!(message(read(&foreign)), "To nie jest plik kopii Bieżni.");
    let no_people = w.root().join("bez-osob.bieznia");
    write_zip(&no_people, &[("manifest.json", manifest(2).as_bytes())]);
    assert!(message(read(&no_people)).contains("uszkodzony"));
    let bad_session = w.root().join("zla-sesja.bieznia");
    write_zip(
        &bad_session,
        &[
            ("manifest.json", manifest(2).as_bytes()),
            ("osoby.json", b"{\"osoby\": []}"),
            ("historia.jsonl", b"{\"id\": \"x\"}\n"),
        ],
    );
    assert!(message(read(&bad_session)).contains("uszkodzony"));
    assert!(message(read(&w.root().join("nie-ma.bieznia"))).contains("nie istnieje"));
}

#[test]
fn read_keeps_only_real_png_icons() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("k.bieznia");
    let manifest = json!({"format": FORMAT, "wersja": 2, "aplikacja": "0.5.1",
        "zawartosc": {"ustawienia": true, "wyniki": false, "logi": false}})
    .to_string();
    write_zip(
        &path,
        &[
            ("manifest.json", manifest.as_bytes()),
            ("osoby.json", b"{\"osoby\": [{\"nazwa\": \"Ala\"}, 3]}"),
            ("ikony/dobra.png", &png()),
            ("ikony/zla.png", b"MZ to nie png"),
        ],
    );
    let p = read(&path).unwrap();
    assert_eq!(p.icons.keys().collect::<Vec<_>>(), ["dobra.png"]);
    assert_eq!(p.people.len(), 1);
    assert_eq!(p.people[0].name, "Ala");
    assert_eq!(p.app_version, "0.5.1");
    assert!(p.has_settings && !p.has_results);
    assert_eq!(describe(&p), "z wersji 0.5.1 · 1 os. · 1 profili · 0 sesji");
}

#[test]
fn read_version_1_sessions_from_0_3() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("stara.bieznia");
    let line = json!({"id": "a1", "osoba": "o1", "osoba_nazwa": "Ala", "profil": "p1",
        "profil_nazwa": "Domyślny", "start": 100.0, "koniec": 200.0, "kroki": 50,
        "dystans_km": 0.04, "ruch_s": 90.0})
    .to_string();
    let manifest = json!({"format": FORMAT, "wersja": 1, "aplikacja": "0.3.2",
        "zawartosc": {"ustawienia": true, "wyniki": true}})
    .to_string();
    write_zip(
        &path,
        &[
            ("manifest.json", manifest.as_bytes()),
            (
                "osoby.json",
                b"{\"osoby\": [{\"id\": \"o1\", \"nazwa\": \"Ala\"}]}",
            ),
            ("historia.jsonl", format!("{line}\n\n").as_bytes()),
        ],
    );
    let p = read(&path).unwrap();
    let mut expected = new_session("a1", "o1", "Ala", "p1", "Domyślny", 100.0, 200.0);
    expected.steps = 50;
    expected.distance_km = 0.04;
    expected.active_s = 90.0;
    assert_eq!(p.sessions, [expected]);
}

#[test]
fn session_json_like_python_and_back() {
    let mut s = new_session(
        "ab",
        "o1",
        "Nocna Wędrowniczka",
        "p1",
        "Gra \"X\"",
        100.0,
        1e16,
    );
    s.steps = 7;
    s.distance_km = 1e-5;
    s.visibility = "znajomi".into();
    s.tz_offset_min = -60;
    s.firmware = "\u{1F600}\t".into();
    let line = session_to_json(&s).unwrap();
    assert_eq!(
        line,
        r#"{"id": "ab", "osoba": "o1", "osoba_nazwa": "Nocna W\u0119drowniczka", "profil": "p1", "profil_nazwa": "Gra \"X\"", "start_utc_s": 100.0, "koniec_utc_s": 1e+16, "kroki": 7, "dystans_km": 1e-05, "ruch_s": 0.0, "bieznia_model": "", "sterownik": "", "bieznia_profil": "", "firmware": "\ud83d\ude00\t", "wersja_aplikacji": "", "zrodlo": "bieznia", "widocznosc": "znajomi", "strefa_min": -60, "zmieniono_utc_s": 0.0, "tryb": "gra"}"#
    );
    let Value::Object(d) = serde_json::from_str(&line).unwrap() else {
        panic!()
    };
    assert_eq!(session_from_json(&d), Some(s.clone()));
    let mut odd = d.clone();
    odd.insert("widocznosc".into(), json!("wszyscy"));
    assert_eq!(session_from_json(&odd).unwrap().visibility, "prywatna");
    let mut missing = d.clone();
    missing.remove("koniec_utc_s");
    assert_eq!(session_from_json(&missing), None);
    let mut wrong = d;
    wrong.insert("kroki".into(), json!([1]));
    assert_eq!(session_from_json(&wrong), None);
}

#[test]
fn import_on_another_computer_adds_people_profiles_icons_and_results() {
    let w = world();
    let file = full_export(&w, "pelna.bieznia");
    let other = tempfile::tempdir().unwrap();
    let mut mine = Store::create(
        &Store::default_path(&other.path().join("dane")),
        "Cicha Tropicielka",
        "Fiolet",
        None,
    )
    .unwrap();
    let history_path = History::default_path(other.path());
    let mine_history = History::open(&history_path).unwrap();
    let preview = read(&file).unwrap();
    let choice = ImportChoice::new(preview.people.iter().map(|p| p.id.clone()).collect());
    let report = apply_import(&mut mine, &mine_history, &preview, &choice, &history_path).unwrap();
    assert_eq!((report.people_added, report.sessions_added), (2, 3));
    assert_eq!(report.profiles_added, 4);
    assert_eq!(
        mine.names(),
        ["Cicha Tropicielka", "Szalony Biegacz", "Nocna Wędrowniczka"]
    );
    let cp = mine.people()[1]
        .profiles()
        .iter()
        .find(|p| p.name == "Cyberpunk 2077")
        .unwrap();
    let icon = cp.icon_file.as_deref().unwrap();
    assert_eq!(fs::read(mine.icons_dir().join(icon)).unwrap(), png());
    assert_eq!(cp.settings["walk_speed"], 2.2);
    assert_eq!(mine.person().name, "Cicha Tropicielka"); // aktywna osoba bez zmian
    let backup = report.backup.unwrap();
    assert!(backup.join("osoby.json").exists());
    // zapisane na dysku
    let again = Store::load(mine.path()).unwrap();
    assert_eq!(again.names(), mine.names());
}

#[test]
fn import_merge_keeps_or_replaces_profile_settings() {
    let mut w = world();
    let file = full_export(&w, "pelna.bieznia");
    let a_id = w.store.people()[0].id.clone();
    let cp_id = ids(&w.store, "Szalony Biegacz")["Cyberpunk 2077"].clone();
    w.store.switch_profile(&cp_id).unwrap();
    w.store.set("walk_speed", json!(9.9)).unwrap(); // zmiana po eksporcie
    let speed = |store: &Store| store.people()[0].profiles()[1].settings["walk_speed"].clone();
    let preview = read(&file).unwrap();
    let hp = w.history_path();
    let keep = ImportChoice {
        keep_my_settings: true,
        ..ImportChoice::new(HashSet::from([a_id.clone()]))
    };
    let r1 = apply_import(&mut w.store, &w.history, &preview, &keep, &hp).unwrap();
    assert_eq!((r1.people_merged, r1.sessions_added), (1, 0)); // te same sesje - bez dublowania
    assert_eq!(speed(&w.store), 9.9);
    let replace = ImportChoice::new(HashSet::from([a_id.clone()]));
    let r2 = apply_import(&mut w.store, &w.history, &preview, &replace, &hp).unwrap();
    assert_eq!((r2.profiles_updated, r2.profiles_added), (3, 0));
    assert_eq!(speed(&w.store), 2.2);
    let as_new = ImportChoice {
        new_person: HashSet::from([a_id.clone()]),
        ..ImportChoice::new(HashSet::from([a_id.clone()]))
    };
    let r3 = apply_import(&mut w.store, &w.history, &preview, &as_new, &hp).unwrap();
    assert_eq!(r3.people_added, 1);
    let last = w.store.people().last().unwrap();
    assert_eq!(last.name, "Szalony Biegacz 2");
    assert_ne!(last.id, a_id);
    assert_eq!(r3.sessions_added, 2); // wyniki skopiowane pod nową osobę
    let copied = w.history.sessions(0.0, Some(&last.id), None, None).unwrap();
    let profile_ids: HashSet<&str> = last.profiles().iter().map(|p| p.id.as_str()).collect();
    assert!(copied.iter().all(|s| {
        s.person_name == "Szalony Biegacz 2" && profile_ids.contains(s.profile.as_str())
    }));
    assert_eq!(w.store.active_person_id(), a_id);
}

#[test]
fn merge_adds_new_profile_with_free_id_and_icon() {
    let mut w = world();
    let file = full_export(&w, "pelna.bieznia");
    let a_id = w.store.people()[0].id.clone();
    let cp_id = ids(&w.store, "Szalony Biegacz")["Cyberpunk 2077"].clone();
    // profil przemianowany po eksporcie: z kopii wraca „Cyberpunk 2077” pod nowym id
    w.store
        .update_profile(
            &cp_id,
            crate::store::ProfileChanges {
                name: Some("CP".into()),
                ..Default::default()
            },
        )
        .unwrap();
    let preview = read(&file).unwrap();
    let hp = w.history_path();
    let r = apply_import(
        &mut w.store,
        &w.history,
        &preview,
        &ImportChoice::new(HashSet::from([a_id])),
        &hp,
    )
    .unwrap();
    assert_eq!((r.profiles_added, r.profiles_updated), (1, 2));
    let back = w.store.people()[0]
        .profiles()
        .iter()
        .find(|p| p.name == "Cyberpunk 2077")
        .unwrap();
    assert_ne!(back.id, cp_id);
    assert_eq!(
        back.icon_file.as_deref(),
        Some(format!("{}.png", back.id).as_str())
    );
    assert!(
        w.store
            .icons_dir()
            .join(format!("{}.png", back.id))
            .exists()
    );
}

#[test]
fn import_without_settings_gives_default_profile() {
    let w = world();
    let out = w.root().join("wyniki.bieznia");
    let choice = ExportChoice {
        settings: false,
        ..ExportChoice::new(all_people(&w.store))
    };
    export(
        &w.store,
        Some(&w.history),
        &choice,
        &out,
        &w.root().join("logs"),
    )
    .unwrap();
    let other = tempfile::tempdir().unwrap();
    let mut mine = Store::create(
        &Store::default_path(&other.path().join("dane")),
        "Cicha Tropicielka",
        "Fiolet",
        None,
    )
    .unwrap();
    let hp = History::default_path(other.path());
    let h = History::open(&hp).unwrap();
    let preview = read(&out).unwrap();
    assert!(!preview.has_settings);
    let all: HashSet<String> = preview.people.iter().map(|p| p.id.clone()).collect();
    let r = apply_import(&mut mine, &h, &preview, &ImportChoice::new(all), &hp).unwrap();
    assert_eq!(
        (r.people_added, r.profiles_added, r.sessions_added),
        (2, 2, 3)
    );
    let added = &mine.people()[1];
    assert_eq!(added.profiles().len(), 1);
    assert_eq!(added.profiles()[0].name, "Domyślny");
    assert_eq!(added.profiles()[0].glyph.as_deref(), Some(DEFAULT_GLYPH));
}

#[test]
fn icon_with_path_in_profile_id_is_not_written() {
    let dir = tempfile::tempdir().unwrap();
    let preview = Preview {
        path: PathBuf::new(),
        app_version: String::new(),
        has_settings: true,
        has_results: false,
        people: Vec::new(),
        sessions: Vec::new(),
        icons: BTreeMap::from([("x.png".to_string(), png())]),
    };
    let icons = dir.path().join("ikony");
    assert_eq!(
        save_icon(&preview, &icons, Some("x.png"), "../zly").unwrap(),
        None
    );
    assert_eq!(
        save_icon(&preview, &icons, Some("x.png"), "abc123").unwrap(),
        Some("abc123.png".into())
    );
    assert_eq!(
        save_icon(&preview, &icons, Some("brak.png"), "abc").unwrap(),
        None
    );
    assert_eq!(save_icon(&preview, &icons, None, "abc").unwrap(), None);
}

#[test]
fn restore_brings_back_data_from_before_import() {
    let mut w = world();
    let file = full_export(&w, "pelna.bieznia");
    let preview = read(&file).unwrap();
    let a_id = w.store.people()[0].id.clone();
    let hp = w.history_path();
    let choice = ImportChoice {
        new_person: HashSet::from([a_id.clone()]),
        ..ImportChoice::new(HashSet::from([a_id]))
    };
    let report = apply_import(&mut w.store, &w.history, &preview, &choice, &hp).unwrap();
    assert_eq!(w.store.people().len(), 3);
    assert_eq!(w.history.count().unwrap(), 5);
    let data = w.store.path().parent().unwrap().to_path_buf();
    let backup = latest_backup(&data).unwrap();
    assert_eq!(Some(&backup), report.backup.as_ref());
    assert!(backup.join("historia.db").exists());
    assert!(backup.join("ikony").join("cp.png").exists());
    restore(&backup, w.store.path(), &hp).unwrap();
    let again = Store::load(w.store.path()).unwrap();
    assert_eq!(again.people().len(), 2);
    // otwarte połączenie widzi przywróconą bazę
    assert_eq!(w.history.count().unwrap(), 3);
}

#[test]
fn latest_backup_picks_newest_folder() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(latest_backup(dir.path()), None);
    let kopie = dir.path().join(BACKUP_DIR);
    for n in [
        "przed-importem-20260101-120000",
        "przed-importem-20261126-080000",
        "ustawienia-0.2.json",
    ] {
        fs::create_dir_all(kopie.join(n)).unwrap();
    }
    assert_eq!(
        latest_backup(dir.path()),
        Some(kopie.join("przed-importem-20261126-080000"))
    );
}

#[test]
fn names_and_helpers() {
    let taken = vec!["Ala".to_string(), "Ala 2".to_string()];
    assert_eq!(unique("Ola", &taken), "Ola");
    assert_eq!(unique("Ala", &taken), "Ala 3");
    let name = default_name();
    assert!(name.starts_with("bieznia-kopia-") && name.ends_with(".bieznia"));
    assert_eq!(name.len(), "bieznia-kopia-2026-11-26.bieznia".len());
    assert_eq!(icon_key("cp.png"), "cp.png");
    assert_eq!(icon_key("a.b.png"), "a.b.png");
    assert_eq!(icon_key("ikona"), "ikona.png");
}
