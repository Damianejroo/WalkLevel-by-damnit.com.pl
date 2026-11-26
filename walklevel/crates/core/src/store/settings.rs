//! Ustawienia 0.2 (dane/ustawienia.json) i stan włączony/wyłączony między restartami -
//! port bieznia/settings.py (oraz `user_settings` z bieznia/cli.py).
//!
//! Kolejność ważności: config.toml < config.local.toml < ustawienia z okna < argumenty.

use std::fs;
use std::path::Path;

use serde_json::{Map, Value};

use super::{Settings, Store, StoreError, to_json_indent, truthy, write_atomic};

/// Płaskie ustawienia 0.2 w folderze danych.
pub const SETTINGS_FILE: &str = "ustawienia.json";
/// Stan włączony/wyłączony w folderze danych.
pub const STATE_FILE: &str = "stan.json";
/// Zmienna środowiska ustawiana przy restarcie po aktualizacji.
pub const RESUME_ENV: &str = "BIEZNIA_RESUME";

/// Klucze, które okno może zapisać, i sekcja config.toml, do której należą.
pub const SECTIONS: &[(&str, &str)] = &[
    ("stand_below", "mapping"),
    ("run_on", "mapping"),
    ("run_off", "mapping"),
    ("quick_stop", "mapping"),
    ("output", "output"),
    ("walk_key", "output"),
    ("run_key", "output"),
    ("toggle_key", "output"),
    ("toggle_pad", "output"),
    ("direction", "output"),
    ("belt_control", "ble"),
    ("idle_speed", "ble"),
    ("walk_speed", "ble"),
    ("run_speed", "ble"),
    ("pad_steps", "ble"),
    ("training_levels", "ble"),
    ("training_level", "ble"),
    ("pad_idle", "ble"),
    ("device_address", "ble"),
    ("device_name", "ble"),
    ("simulator", "ui"),
    ("record", "ui"),
    ("always_on_top", "ui"),
    ("floating_button", "ui"),
    ("floating_pos", "ui"),
    ("theme", "ui"),
    ("large_text", "ui"),
    ("detailed_logs", "ui"),
    ("last_version", "ui"),
];

/// Sekcja config.toml dla klucza (`None` - klucza nie zapisuje się do config).
pub fn section(key: &str) -> Option<&'static str> {
    SECTIONS.iter().find(|(k, _)| *k == key).map(|(_, s)| *s)
}

fn only_sections(m: &Settings) -> Settings {
    m.iter()
        .filter(|(k, _)| section(k).is_some())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Ustawienia z pliku (tylko znane klucze). Brak albo uszkodzony plik - pusty słownik.
pub fn load_settings(path: &Path) -> Settings {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Settings::new(),
        Err(e) => {
            log::warn!(
                "Nie da się odczytać {} ({e}) - używam domyślnych",
                path.display()
            );
            return Settings::new();
        }
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(m)) => only_sections(&m),
        Ok(_) => Settings::new(),
        Err(e) => {
            log::warn!(
                "Nie da się odczytać {} ({e}) - używam domyślnych",
                path.display()
            );
            Settings::new()
        }
    }
}

/// Zapis atomowy (wcięcie 2 jak w 0.5.x).
pub fn save_settings(settings: &Settings, path: &Path) -> Result<(), StoreError> {
    let text = to_json_indent(settings, b"  ")?;
    write_atomic(path, text.as_bytes())
}

/// Nakłada ustawienia na config (słownik sekcji). `output` trafia do `output.type`.
pub fn apply_settings(config: &mut Map<String, Value>, settings: &Settings) {
    for (key, value) in settings {
        let Some(sec) = section(key) else { continue };
        let entry = config
            .entry(sec)
            .or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(Map::new());
        }
        if let Value::Object(m) = entry {
            let name = if key == "output" {
                "type"
            } else {
                key.as_str()
            };
            m.insert(name.to_string(), value.clone());
        }
    }
}

/// Ustawienia z okna, które trafiają do config (port cli.user_settings).
///
/// Okno czyta osoby i profile samo - do config idą wtedy tylko ustawienia wspólne.
/// Tryb bez okna (`headless`) bierze cały aktywny profil. Bez osoby (pierwsze
/// uruchomienie): ustawienia z 0.2.
pub fn user_settings(data_dir: &Path, headless: bool) -> Settings {
    let Some(store) = Store::load(&Store::default_path(data_dir)) else {
        return load_settings(&data_dir.join(SETTINGS_FILE));
    };
    let flat = if headless {
        store.flat(&Settings::new())
    } else {
        store.shared().clone()
    };
    only_sections(&flat)
}

// --- stan włączony/wyłączony między restartami po aktualizacji --------------

/// Zapamiętanie stanu włączony/wyłączony (błędy zapisu pomijane).
pub fn save_enabled(enabled: bool, path: &Path) {
    let result = path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| fs::write(path, format!("{{\"enabled\": {enabled}}}")));
    if let Err(e) = result {
        log::debug!("Nie da się zapisać stanu {}: {e}", path.display());
    }
}

/// Stan sprzed restartu - tylko po restarcie przez aktualizację (zmienna [`RESUME_ENV`] = "1").
/// Zwykły start zawsze zaczyna wyłączony (`None`).
pub fn load_resumed_enabled(path: &Path) -> Option<bool> {
    if std::env::var(RESUME_ENV).ok().as_deref() != Some("1") {
        return None;
    }
    read_enabled(path)
}

fn read_enabled(path: &Path) -> Option<bool> {
    let text = fs::read_to_string(path).ok()?;
    let data: Value = serde_json::from_str(&text).ok()?;
    data.as_object()?.get("enabled").map(truthy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(v: Value) -> Settings {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn load_and_save_keep_only_known_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dane").join(SETTINGS_FILE);
        assert!(load_settings(&path).is_empty());
        let s = obj(json!({"walk_speed": 2.0, "theme": "dark", "obcy": 1}));
        save_settings(&s, &path).unwrap();
        let back = load_settings(&path);
        assert_eq!(back, obj(json!({"walk_speed": 2.0, "theme": "dark"})));
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("\n  \""), "wcięcie 2: {text}");
        assert!(!path.with_extension("tmp").exists());
        fs::write(&path, "{zepsuty").unwrap();
        assert!(load_settings(&path).is_empty());
        fs::write(&path, "[1, 2]").unwrap();
        assert!(load_settings(&path).is_empty());
    }

    #[test]
    fn apply_puts_keys_into_sections() {
        let mut config = obj(json!({"output": {"type": "keyboard", "start_enabled": false}}));
        apply_settings(
            &mut config,
            &obj(json!({"output": "gamepad", "walk_speed": 2.5, "theme": "dark", "obcy": 1})),
        );
        assert_eq!(
            Value::Object(config),
            json!({
                "output": {"type": "gamepad", "start_enabled": false},
                "ble": {"walk_speed": 2.5},
                "ui": {"theme": "dark"},
            })
        );
    }

    #[test]
    fn resumed_state_only_after_update_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dane").join(STATE_FILE);
        save_enabled(true, &path);
        assert_eq!(read_enabled(&path), Some(true));
        save_enabled(false, &path);
        assert_eq!(read_enabled(&path), Some(false));
        fs::write(&path, "{}").unwrap();
        assert_eq!(read_enabled(&path), None);
        // zmiennej środowiska nie ustawiamy w testach (równoległe wątki) - bez niej zawsze None
        if std::env::var(RESUME_ENV).is_err() {
            assert_eq!(load_resumed_enabled(&path), None);
        }
    }

    #[test]
    fn cli_reads_store_instead_of_old_settings() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("dane");
        fs::create_dir_all(&data).unwrap();
        fs::write(
            data.join(SETTINGS_FILE),
            r#"{"walk_speed": 2.0, "theme": "light"}"#,
        )
        .unwrap();
        assert_eq!(user_settings(&data, false)["walk_speed"], json!(2.0)); // przed konfiguratorem: 0.2
        let legacy = obj(json!({"walk_speed": 3.0, "device_name": "KS"}));
        let store =
            Store::create(&Store::default_path(&data), "Ola", "Fiolet", Some(&legacy)).unwrap();
        // okno samo czyta profil - do config tylko wspólne
        assert_eq!(
            user_settings(&data, false),
            obj(json!({"device_name": "KS"}))
        );
        let headless = user_settings(&data, true);
        assert_eq!(headless["walk_speed"], json!(3.0)); // bez okna: cały aktywny profil
        assert!(!headless.contains_key("profile_color"));
        assert!(store.path().exists());
    }
}
