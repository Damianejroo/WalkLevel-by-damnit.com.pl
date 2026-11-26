//! Osoby i profile gier: zapis, przełączanie, kopie profili, migracja z 0.2 (tests/test_dane.py)
//! oraz zgodność z plikami zapisanymi przez 0.5.x (fixtury z Pythona).

use super::*;
use serde_json::json;
use tempfile::TempDir;

const NEW_ID: &str = "<nowe-id>";

fn obj(v: Value) -> Settings {
    v.as_object().unwrap().clone()
}

fn defaults() -> Settings {
    obj(json!({
        "direction": "keyboard",
        "walk_speed": 1.5,
        "theme": "system",
        "device_address": "",
        "belt_control": true,
    }))
}

fn legacy() -> Settings {
    obj(json!({
        "direction": "gamepad",
        "pad_idle": "stop",
        "walk_speed": 2.0,
        "theme": "dark",
        "floating_button": true,
        "device_address": "A0:24:42:30:E5:B3",
        "belt_control": true,
        "toggle_key": {"scan": 66, "extended": false, "name": "F8", "vk": 119},
        "nieznany_klucz": 1,
    }))
}

fn store() -> (TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let path = Store::default_path(&dir.path().join("dane"));
    let s = Store::create(&path, "Szalony Biegacz", "Morski", Some(&legacy())).unwrap();
    (dir, s)
}

fn profile_names(p: &Person) -> Vec<&str> {
    p.profiles().iter().map(|p| p.name.as_str()).collect()
}

#[test]
fn migration_puts_old_settings_where_they_belong() {
    let (_dir, store) = store();
    assert_eq!(store.shared()["device_address"], "A0:24:42:30:E5:B3"); // wspólne
    assert_eq!(
        store.person().look,
        obj(json!({"theme": "dark", "floating_button": true}))
    );
    assert_eq!(store.profile().name, "Domyślny");
    assert_eq!(store.profile().glyph.as_deref(), Some(DEFAULT_GLYPH));
    assert_eq!(
        store.profile().settings,
        obj(json!({"direction": "gamepad", "pad_idle": "stop", "walk_speed": 2.0}))
    );
    let flat = store.flat(&defaults());
    assert_eq!(flat["walk_speed"], json!(2.0));
    assert_eq!(flat["theme"], "dark");
    assert_eq!(flat["belt_control"], true);
    assert_eq!(flat["show_person_switch"], true); // domyślne nowe opcje wyglądu
    assert_eq!(flat["profile_color"], "Mięta");
    assert_eq!(flat["person_color"], "Morski");
    assert!(!flat.contains_key("nieznany_klucz"));
}

#[test]
fn store_round_trips_through_disk() {
    let (_dir, mut store) = store();
    store
        .add_profile(
            "Cyberpunk 2077",
            NewProfile {
                icon_color: Some("Banan".into()),
                icon_file: Some("x.png".into()),
                exe: Some(r"D:\cp.exe".into()),
                glyph: Some("\u{e7fc}".into()),
                ..Default::default()
            },
        )
        .unwrap();
    let again = Store::load(store.path()).unwrap();
    assert_eq!(
        profile_names(again.person()),
        ["Domyślny", "Cyberpunk 2077"]
    );
    let cp = again.profile();
    assert_eq!(
        (
            cp.color.as_str(),
            cp.icon_color.as_deref(),
            cp.icon_file.as_deref(),
            cp.glyph.as_deref()
        ),
        ("Banan", Some("Banan"), Some("x.png"), None)
    );
    let raw: Value = serde_json::from_str(&fs::read_to_string(store.path()).unwrap()).unwrap();
    assert_eq!(raw["wersja"], FORMAT_VERSION);
    assert_eq!(raw, store.to_json().unwrap());
    assert!(!store.path().with_extension("tmp").exists()); // zapis atomowy bez resztek
}

#[test]
fn file_format_is_like_python() {
    let (_dir, store) = store();
    let text = fs::read_to_string(store.path()).unwrap();
    // json.dumps(indent=1, ensure_ascii=False): wcięcie 1 spacja, polskie znaki bez \u
    assert!(
        text.starts_with("{\n \"wersja\": 1,\n \"aktywna\": \""),
        "{text}"
    );
    assert!(text.contains("\"nazwa\": \"Domyślny\""));
    assert!(text.contains("\"glif\": \"\u{e805}\""));
    let keys: Vec<String> = serde_json::from_str::<Map<String, Value>>(&text)
        .unwrap()
        .keys()
        .cloned()
        .collect();
    let mut expected = vec![
        "aktywna",
        "osoby",
        "pytaj_przy_starcie",
        "wersja",
        "wspolne",
    ];
    expected.sort_unstable();
    let mut keys_sorted = keys.clone();
    keys_sorted.sort();
    assert_eq!(keys_sorted, expected);
}

#[test]
fn speed_keys_are_shared_and_default_to_f7_f6() {
    use crate::input::Key;
    let (dir, mut store) = store();
    // dane bez klucza (osoby.json z 0.5.x i świeże) - F7 i F6, jak dotąd
    let flat = store.flat(&defaults());
    assert!(!flat.contains_key("faster_key") && !flat.contains_key("slower_key"));
    let read = |flat: &Settings, key: &str, default: &Key| {
        Key::speed_from_value(flat.get(key).unwrap_or(&Value::Null), None, default)
    };
    assert_eq!(
        read(&flat, "faster_key", &Key::faster_default()).label(),
        "F7"
    );
    assert_eq!(
        read(&flat, "slower_key", &Key::slower_default()).label(),
        "F6"
    );

    let faster = Key::new(0x49, true, "Page Up", 0x21).with_mods(vec![crate::input::Mod::Ctrl]);
    let slower = Key::new(0x51, true, "Page Down", 0x22);
    let mut flat = flat;
    flat.insert("faster_key".into(), faster.to_value());
    flat.insert("slower_key".into(), slower.to_value());
    store.update_flat(&flat).unwrap();
    assert_eq!(store.shared()["faster_key"], faster.to_value()); // wspólne, nie w profilu
    assert!(!store.profile().settings.contains_key("faster_key"));

    // po ponownym wczytaniu z dysku i u innej osoby - te same klawisze
    let loaded = Store::load(store.path()).unwrap();
    let flat = loaded.flat(&defaults());
    assert_eq!(read(&flat, "faster_key", &Key::faster_default()), faster);
    assert_eq!(read(&flat, "slower_key", &Key::slower_default()), slower);
    assert_eq!(
        read(&flat, "faster_key", &Key::faster_default()).label(),
        "Ctrl+Page Up"
    );
    drop(dir);
}

#[test]
fn settings_go_to_the_active_profile_only() {
    let (_dir, mut store) = store();
    let default_id = store.profile().id.clone();
    let game_id = store
        .add_profile(
            "Elden Ring",
            NewProfile {
                copy_from: Some(default_id.clone()),
                ..Default::default()
            },
        )
        .unwrap()
        .id
        .clone();
    store.set("walk_speed", json!(3.0)).unwrap(); // zmiana w aktywnym profilu (Elden Ring)
    let find = |s: &Store, id: &str| {
        s.person()
            .profiles()
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .unwrap()
    };
    assert_eq!(find(&store, &game_id).settings["walk_speed"], json!(3.0));
    assert_eq!(find(&store, &default_id).settings["walk_speed"], json!(2.0));
    store.switch_profile(&default_id).unwrap();
    assert_eq!(store.flat(&defaults())["walk_speed"], json!(2.0));
    store.set("theme", json!("light")).unwrap(); // wygląd - osoba, nie profil
    assert!(!find(&store, &default_id).settings.contains_key("theme"));
    assert_eq!(store.person().look["theme"], "light");
    store.set("achievement_sound", json!(true)).unwrap(); // powiadomienia - też osoba
    assert_eq!(store.person().look["achievement_sound"], true);
    store.set("device_name", json!("KS-NACH-X21C")).unwrap(); // wspólne
    assert_eq!(store.shared()["device_name"], "KS-NACH-X21C");
    store.set("tylko_na_teraz", json!(1)).unwrap(); // klucz spoza zestawów - pomijany
    assert!(!store.flat(&Settings::new()).contains_key("tylko_na_teraz"));
    // update_flat: każdy klucz tam, gdzie należy
    store
        .update_flat(&obj(
            json!({"walk_speed": 4.5, "large_text": true, "record": false, "x": 1}),
        ))
        .unwrap();
    let again = Store::load(store.path()).unwrap();
    assert_eq!(again.profile().settings["walk_speed"], json!(4.5));
    assert_eq!(again.person().look["large_text"], true);
    assert_eq!(again.shared()["record"], false);
}

#[test]
fn people_have_separate_profiles_and_settings() {
    let (_dir, mut store) = store();
    let first_id = store.person().id.clone();
    let from = store.profile().id.clone();
    store
        .add_profile(
            "Cyberpunk 2077",
            NewProfile {
                copy_from: Some(from),
                ..Default::default()
            },
        )
        .unwrap();
    let second = store
        .add_person("Nocna Wędrowniczka", "Orchidea", Some(&first_id))
        .unwrap()
        .clone();
    assert_eq!(store.person().id, second.id);
    assert_eq!(profile_names(&second), ["Domyślny", "Cyberpunk 2077"]);
    let first = store
        .people()
        .iter()
        .find(|p| p.id == first_id)
        .unwrap()
        .clone();
    assert!(
        second
            .profiles()
            .iter()
            .all(|p| first.profiles().iter().all(|q| q.id != p.id))
    );
    assert_eq!(second.look, first.look);
    store.switch_profile(&second.profiles()[1].id).unwrap();
    store.set("walk_speed", json!(4.0)).unwrap();
    store.switch_person(&first_id).unwrap();
    // osobne nawet dla tej samej gry
    assert_ne!(
        store.person().profile().settings.get("walk_speed"),
        Some(&json!(4.0))
    );
    let empty = store
        .add_person("Cicha Tropicielka", "Fiolet", None)
        .unwrap();
    assert_eq!(profile_names(empty), ["Domyślny"]);
    assert_eq!(empty.color, "Fiolet");
    assert_eq!(
        store.names(),
        ["Szalony Biegacz", "Nocna Wędrowniczka", "Cicha Tropicielka"]
    );
    // pusta nazwa - Gracz, nieznany kolor - Mięta
    let anon = store.add_person("  ", "Zielony", None).unwrap();
    assert_eq!(
        (anon.name.as_str(), anon.color.as_str()),
        ("Gracz", "Mięta")
    );
}

#[test]
fn rename_and_switch_unknown_ids() {
    let (_dir, mut store) = store();
    let id = store.person().id.clone();
    store.rename_person(&id, "  Ola  ", Some("Róż")).unwrap();
    assert_eq!(
        (store.person().name.as_str(), store.person().color.as_str()),
        ("Ola", "Róż")
    );
    store.rename_person(&id, " ", None).unwrap();
    assert_eq!(store.person().name, "Ola");
    assert_eq!(store.switch_person("nie-ma").unwrap().id, id);
    let active = store.profile().id.clone();
    assert_eq!(store.switch_profile("nie-ma").unwrap().id, active);
    store.set_ask_at_start(false).unwrap();
    assert!(!Store::load(store.path()).unwrap().ask_at_start());
}

#[test]
fn update_profile_changes_fields() {
    let (_dir, mut store) = store();
    let id = store.profile().id.clone();
    store
        .update_profile(
            &id,
            ProfileChanges {
                name: Some("  Gra  ".into()),
                color: Some("nie ma".into()),
                icon_file: Some(Some("a.png".into())),
                glyph: Some(None),
                online: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
    let p = Store::load(store.path()).unwrap().profile().clone();
    assert_eq!(p.name, "Gra");
    assert_eq!(p.color, "Mięta");
    assert_eq!(
        (p.icon_file.as_deref(), p.glyph.as_deref(), p.online),
        (Some("a.png"), None, false)
    );
    store
        .update_profile(
            &id,
            ProfileChanges {
                name: Some("   ".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(store.profile().name, "Gra");
}

#[test]
fn last_person_and_last_profile_cannot_be_removed() {
    let (_dir, mut store) = store();
    let person = store.person().id.clone();
    let profile = store.profile().id.clone();
    assert!(!store.remove_person(&person).unwrap());
    assert!(!store.remove_profile(&profile).unwrap());
    let extra = store
        .add_profile("PEAK", NewProfile::default())
        .unwrap()
        .id
        .clone();
    assert!(store.remove_profile(&extra).unwrap());
    assert_eq!(store.profile().name, "Domyślny");
    let other = store.add_person("Ola", "Fiolet", None).unwrap().id.clone();
    assert!(store.remove_person(&other).unwrap());
    assert_eq!(store.person().id, person);
}

#[test]
fn no_limits_on_people_and_profiles() {
    let (_dir, mut store) = store();
    for i in 0..30 {
        store
            .add_profile(&format!("Gra {i}"), NewProfile::default())
            .unwrap();
    }
    for i in 0..12 {
        store
            .add_person(&format!("Osoba {i}"), "Błękit", None)
            .unwrap();
    }
    let again = Store::load(store.path()).unwrap();
    assert_eq!(again.people().len(), 13);
    assert_eq!(again.people()[0].profiles().len(), 31);
}

#[test]
fn duplicate_profile_copies_settings_and_icon() {
    let (_dir, mut store) = store();
    let icons = store.icons_dir();
    fs::create_dir_all(&icons).unwrap();
    fs::write(icons.join("ikona.png"), b"png").unwrap();
    let from = store.profile().id.clone();
    let game = store
        .add_profile(
            "PEAK",
            NewProfile {
                icon_file: Some("ikona.png".into()),
                copy_from: Some(from),
                ..Default::default()
            },
        )
        .unwrap()
        .clone();
    let dup = store.duplicate_profile(&game.id).unwrap().unwrap().clone();
    assert_eq!(dup.name, "PEAK (kopia)");
    let icon = dup.icon_file.clone().unwrap();
    assert_ne!(icon, "ikona.png");
    assert_eq!(fs::read(icons.join(&icon)).unwrap(), b"png");
    assert_eq!(dup.settings, game.settings);
    assert!(store.duplicate_profile("nie-ma").unwrap().is_none());
    // ikona, której nie ma na dysku - kopia bez ikony
    store
        .update_profile(
            &game.id,
            ProfileChanges {
                icon_file: Some(Some("brak.png".into())),
                ..Default::default()
            },
        )
        .unwrap();
    let dup2 = store.duplicate_profile(&game.id).unwrap().unwrap();
    assert_eq!(dup2.icon_file, None);
}

#[test]
fn corrupted_file_is_backed_up() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("dane");
    fs::create_dir_all(&data).unwrap();
    let path = Store::default_path(&data);
    assert!(Store::load(&path).is_none()); // brak pliku - bez kopii
    assert!(!data.join(BACKUP_DIR).exists());
    fs::write(&path, "{zepsuty").unwrap();
    assert!(Store::load(&path).is_none());
    let backups: Vec<_> = fs::read_dir(data.join(BACKUP_DIR))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(backups.len(), 1);
    assert!(backups[0].starts_with("uszkodzony-") && backups[0].ends_with(".json"));
}

#[test]
fn legacy_backup() {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("dane").join(OLD_SETTINGS);
    assert!(legacy_settings(&old).is_none()); // nowa instalacja
    assert!(backup_legacy(&old).unwrap().is_none());
    fs::create_dir_all(old.parent().unwrap()).unwrap();
    fs::write(&old, serde_json::to_string(&legacy()).unwrap()).unwrap();
    assert_eq!(legacy_settings(&old), Some(legacy()));
    let backup = backup_legacy(&old).unwrap().unwrap();
    assert_eq!(
        backup,
        dir.path()
            .join("dane")
            .join(BACKUP_DIR)
            .join("ustawienia-0.2.json")
    );
    let copied: Settings = serde_json::from_str(&fs::read_to_string(&backup).unwrap()).unwrap();
    assert_eq!(copied, legacy());
    assert!(old.exists()); // oryginał zostaje
}

// --- zgodność z 0.5.x (fixtury z walklevel/tools/fixtures/dane.py) --------------------

fn is_new_id(v: &Value) -> bool {
    v.as_str()
        .is_some_and(|s| s.len() == 10 && s.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Porównanie JSON, w którym "<nowe-id>" z Pythona pasuje do dowolnego nowego id.
fn assert_matches(expected: &Value, actual: &Value, at: &str) {
    match (expected, actual) {
        (Value::String(s), a) if s == NEW_ID => assert!(is_new_id(a), "{at}: {a} to nie nowe id"),
        (Value::Object(e), Value::Object(a)) => {
            let mut ek: Vec<_> = e.keys().collect();
            let mut ak: Vec<_> = a.keys().collect();
            ek.sort();
            ak.sort();
            assert_eq!(ek, ak, "{at}: klucze");
            for (k, v) in e {
                assert_matches(v, &a[k], &format!("{at}.{k}"));
            }
        }
        (Value::Array(e), Value::Array(a)) => {
            assert_eq!(e.len(), a.len(), "{at}: długość");
            for (i, (ev, av)) in e.iter().zip(a).enumerate() {
                assert_matches(ev, av, &format!("{at}[{i}]"));
            }
        }
        (e, a) => assert_eq!(e, a, "{at}"),
    }
}

fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/dane")
        .join(name);
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn loading_matches_python() {
    let dir = tempfile::tempdir().unwrap();
    for case in fixture("wczytanie.json").as_array().unwrap() {
        let name = case["nazwa"].as_str().unwrap();
        let path = dir.path().join(format!("{name}.json"));
        fs::write(&path, case["wejscie"].as_str().unwrap()).unwrap();
        let store = Store::load(&path);
        match &case["wynik"] {
            Value::Null => assert!(store.is_none(), "{name}: powinno być None"),
            expected => {
                let store = store.unwrap_or_else(|| panic!("{name}: nie wczytane"));
                assert_matches(expected, &store.to_json().unwrap(), name);
                assert_matches(&case["flat"], &Value::Object(store.flat(&defaults())), name);
                // niezmienniki: aktywne id wskazują istniejące
                assert!(
                    store
                        .people()
                        .iter()
                        .any(|p| p.id == store.active_person_id())
                );
                for p in store.people() {
                    assert!(p.profiles().iter().any(|q| q.id == p.active_profile_id()));
                }
            }
        }
    }
}

#[test]
fn file_from_0_5_survives_load_and_save_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let cases = fixture("wczytanie.json");
    let real = cases
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["nazwa"] == "realny_0_5")
        .unwrap();
    let text = real["wejscie"].as_str().unwrap();
    let path = Store::default_path(dir.path());
    fs::write(&path, text).unwrap();
    let mut store = Store::load(&path).unwrap();
    store.set("walk_speed", json!(2.0)).unwrap(); // zapis bez zmiany wartości
    let original: Value = serde_json::from_str(text).unwrap();
    let saved: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved, original);
}

#[test]
fn create_matches_python() {
    let f = fixture("nowy.json");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nowy.json");
    let legacy = f["legacy"].as_object().unwrap();
    let store = Store::create(&path, "Szalony Biegacz", "Morski", Some(legacy)).unwrap();
    assert_matches(&f["wynik"], &store.to_json().unwrap(), "nowy");
    let defaults = f["defaults"].as_object().unwrap();
    assert_matches(
        &f["flat"],
        &Value::Object(store.flat(defaults)),
        "nowy.flat",
    );
}

#[test]
fn python_truthiness_helpers() {
    for v in [
        json!(null),
        json!(false),
        json!(0),
        json!(0.0),
        json!(""),
        json!([]),
        json!({}),
    ] {
        assert!(!truthy(&v), "{v}");
    }
    for v in [
        json!(true),
        json!(1),
        json!("a"),
        json!([0]),
        json!({"a": 0}),
    ] {
        assert!(truthy(&v), "{v}");
    }
    assert_eq!(py_str(&json!(12345)), "12345");
    assert_eq!(py_str(&json!(2.5)), "2.5");
    assert!(is_new_id(&json!(new_id())));
}

/// Id (10 znaków hex, też w nazwie ikony „<id>.png”) zamienione na kolejne „ID<n>” według
/// pierwszego wystąpienia - porównanie struktury z odwołaniami bez losowych wartości.
fn canonical_ids(v: &Value, seen: &mut Vec<String>) -> Value {
    match v {
        Value::String(s) => {
            let (id, suffix) = s
                .strip_suffix(".png")
                .map_or((s.as_str(), ""), |i| (i, ".png"));
            if is_new_id(&json!(id)) {
                let n = seen.iter().position(|x| x == id).unwrap_or_else(|| {
                    seen.push(id.to_string());
                    seen.len() - 1
                });
                json!(format!("ID{n}{suffix}"))
            } else {
                v.clone()
            }
        }
        Value::Array(a) => Value::Array(a.iter().map(|x| canonical_ids(x, seen)).collect()),
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, x)| (k.clone(), canonical_ids(x, seen)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Te same operacje co realistic_store() w walklevel/tools/fixtures/dane.py dają ten sam plik.
#[test]
fn operations_match_python_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = Store::default_path(&dir.path().join("dane"));
    let mut store = Store::create(&path, "Szalony Biegacz", "Morski", Some(&legacy())).unwrap();
    store
        .set(
            "training_levels",
            json!([{"name": "Lekko", "speed": 2.5}, {"name": "Szybko", "speed": 5.0}]),
        )
        .unwrap();
    store
        .set("window_geometry", json!("AdnQywADAAAAAAJTAAABLQ=="))
        .unwrap();
    let from = store.profile().id.clone();
    let cp = store
        .add_profile(
            "Cyberpunk 2077",
            NewProfile {
                icon_color: Some("Banan".into()),
                icon_file: Some("cp.png".into()),
                exe: Some(r"D:\Gry\Cyberpunk 2077\bin\x64\Cyberpunk2077.exe".into()),
                copy_from: Some(from),
                ..Default::default()
            },
        )
        .unwrap()
        .id
        .clone();
    store.set("run_on", json!(5.5)).unwrap();
    store
        .update_profile(
            &cp,
            ProfileChanges {
                online: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
    store
        .add_profile(
            "Minecraft",
            NewProfile {
                color: Some("Zieleń".into()),
                glyph: Some("\u{e7fc}".into()),
                activate: false,
                ..Default::default()
            },
        )
        .unwrap();
    store.set("large_text", json!(true)).unwrap();
    let first = store.people()[0].id.clone();
    let second = store
        .add_person("Nocna Wędrowniczka", "Orchidea", Some(&first))
        .unwrap()
        .clone();
    store.set("theme", json!("light")).unwrap();
    store.switch_profile(&second.profiles()[1].id).unwrap();
    store.set("walk_speed", json!(3.25)).unwrap();
    store.switch_person(&first).unwrap();

    let cases = fixture("wczytanie.json");
    let real = cases
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["nazwa"] == "realny_0_5")
        .unwrap();
    let python: Value = serde_json::from_str(real["wejscie"].as_str().unwrap()).unwrap();
    let rust: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        canonical_ids(&rust, &mut Vec::new()),
        canonical_ids(&python, &mut Vec::new())
    );
}

#[test]
fn empty_icon_name_is_kept_on_copy() {
    // 0.5.x: `if src.icon_file:` - pusta nazwa ikony nie jest kopiowana, zostaje pusta
    let (_dir, mut store) = store();
    let id = store.profile().id.clone();
    store
        .update_profile(
            &id,
            ProfileChanges {
                icon_file: Some(Some(String::new())),
                ..Default::default()
            },
        )
        .unwrap();
    let dup = store.duplicate_profile(&id).unwrap().unwrap();
    assert_eq!(dup.icon_file.as_deref(), Some(""));
}
