//! Ustawienia z config.toml + opcjonalnego config.local.toml - port bieznia/config.py
//! i `apply_settings` z bieznia/settings.py.
//!
//! Kolejność ważności: config.toml (zasoby) < config.local.toml (folder danych) < ustawienia
//! z okna. Konfiguracja trzymana jest jako mapa JSON (jak słownik w Pythonie), żeby ustawienia
//! z plików JSON i okno Tauri dało się do niej dołożyć bez przekształceń.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

/// Domyślne ustawienia w folderze zasobów.
pub const DEFAULT_FILE: &str = "config.toml";
/// Lokalne nadpisania w folderze danych (nie trafiają do gita ani do wydania).
pub const LOCAL_FILE: &str = "config.local.toml";

/// Klucze ustawień zapisywanych w oknie i sekcja config.toml, do której należą
/// (1:1 z `SECTIONS` w bieznia/settings.py).
pub const SETTINGS_SECTIONS: &[(&str, &str)] = &[
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

/// Błędy wczytywania konfiguracji.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Nie da się odczytać {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Błędny plik {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("Plik {path} nie zamienia się na ustawienia: {source}")]
    Convert {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("Błędna sekcja [{section}] w ustawieniach: {source}")]
    Section {
        section: String,
        source: serde_json::Error,
    },
}

/// Wczytana konfiguracja: sekcje config.toml jako mapa JSON.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Config {
    /// cała konfiguracja (sekcja -> klucze), tak jak słownik w Pythonie
    pub values: Map<String, Value>,
}

impl Config {
    /// Sekcja jako mapa (None, gdy jej nie ma albo nie jest tabelą).
    pub fn section(&self, name: &str) -> Option<&Map<String, Value>> {
        self.values.get(name)?.as_object()
    }

    /// Sekcja odczytana jako typ modułu (np. ustawienia mapowania prędkości).
    pub fn section_as<T: DeserializeOwned>(&self, name: &str) -> Result<T, ConfigError> {
        let value = self
            .values
            .get(name)
            .cloned()
            .unwrap_or(Value::Object(Map::new()));
        serde_json::from_value(value).map_err(|source| ConfigError::Section {
            section: name.to_string(),
            source,
        })
    }

    /// Tekst `[sekcja] klucz` (None, gdy brak albo to nie tekst).
    pub fn str_value(&self, section: &str, key: &str) -> Option<&str> {
        self.section(section)?.get(key)?.as_str()
    }

    /// Adres serwera kont (`[konto] adres`); None, gdy pusty - wtedy sekcji konta nie widać.
    pub fn account_url(&self) -> Option<&str> {
        self.str_value("konto", "adres")
            .filter(|s| !s.trim().is_empty())
    }

    /// Link do regulaminu konta online (`[konto] regulamin`).
    pub fn terms_url(&self) -> Option<&str> {
        self.str_value("konto", "regulamin")
            .filter(|s| !s.trim().is_empty())
    }

    /// Adres wysyłki zgłoszeń (`[zgloszenia] adres`); None = zgłoszenie mailem.
    pub fn reports_url(&self) -> Option<&str> {
        self.str_value("zgloszenia", "adres")
            .filter(|s| !s.trim().is_empty())
    }

    /// Dokłada ustawienia z okna (patrz [`apply_settings`]).
    pub fn apply_settings(&mut self, settings: &Map<String, Value>) {
        apply_settings(&mut self.values, settings);
    }
}

/// Domyślne ustawienia z `resources`/config.toml + `data`/config.local.toml, jeśli jest.
pub fn load_config(resources: &Path, data: &Path) -> Result<Config, ConfigError> {
    let mut values = read_toml(&resources.join(DEFAULT_FILE))?;
    let local = data.join(LOCAL_FILE);
    if local.exists() {
        values = merge(&values, &read_toml(&local)?);
    }
    Ok(Config { values })
}

/// Łączy dwie mapy: tabele w obu łączą się rekurencyjnie, resztę zastępuje `over`.
pub fn merge(base: &Map<String, Value>, over: &Map<String, Value>) -> Map<String, Value> {
    let mut result = base.clone();
    for (key, value) in over {
        let merged = match (value, result.get(key)) {
            (Value::Object(o), Some(Value::Object(b))) => Value::Object(merge(b, o)),
            _ => value.clone(),
        };
        result.insert(key.clone(), merged);
    }
    result
}

/// Sekcja config.toml dla klucza ustawień z okna (None dla nieznanych kluczy).
pub fn settings_section(key: &str) -> Option<&'static str> {
    SETTINGS_SECTIONS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, s)| *s)
}

/// Wpisuje ustawienia z okna do konfiguracji: klucz trafia do swojej sekcji, a `output`
/// do `[output] type`. Nieznane klucze są pomijane (Python odfiltrowuje je przy wczytaniu).
pub fn apply_settings(config: &mut Map<String, Value>, settings: &Map<String, Value>) {
    for (key, value) in settings {
        let Some(section) = settings_section(key) else {
            continue;
        };
        let entry = config
            .entry(section)
            .or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(Map::new());
        }
        if let Value::Object(map) = entry {
            let name = if key == "output" {
                "type"
            } else {
                key.as_str()
            };
            map.insert(name.to_string(), value.clone());
        }
    }
}

fn read_toml(path: &Path) -> Result<Map<String, Value>, ConfigError> {
    let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let table: toml::Table = toml::from_str(&text).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })?;
    match serde_json::to_value(table) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(_) => Ok(Map::new()),
        Err(source) => Err(ConfigError::Convert {
            path: path.to_path_buf(),
            source,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    fn fixture(name: &str) -> Value {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/baza")
            .join(name);
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .unwrap()
            .to_path_buf()
    }

    #[test]
    fn repo_config_is_valid() {
        // bez config.local.toml: folder danych bez tego pliku
        let empty = tempfile::tempdir().unwrap();
        let config = load_config(&repo_root(), empty.path()).unwrap();
        // domyślnie bezpiecznie: nic nie naciska
        assert_eq!(config.str_value("output", "type"), Some("dry"));
        assert_eq!(
            config.section("output").unwrap()["start_enabled"],
            json!(false)
        );
        let mapping = config.section("mapping").unwrap();
        for key in [
            "stand_below",
            "run_on",
            "run_off",
            "max_speed",
            "stale_after_s",
        ] {
            assert!(mapping[key].is_number(), "{key}");
        }
        assert!(mapping["run_on"].as_f64() > mapping["run_off"].as_f64());
        assert_eq!(config.account_url(), Some("https://bieznia.damnit-live.pl"));
        assert!(config.reports_url().unwrap().starts_with("https://"));
        assert!(config.terms_url().is_some());
    }

    #[test]
    fn repo_config_matches_python() {
        let empty = tempfile::tempdir().unwrap();
        let config = load_config(&repo_root(), empty.path()).unwrap();
        assert_eq!(Value::Object(config.values), fixture("config_repo.json"));
    }

    #[test]
    fn local_config_overrides_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("config.toml"),
            "[output]\ntype = \"dry\"\nstart_enabled = false\n",
        )
        .unwrap();
        fs::write(
            tmp.path().join("config.local.toml"),
            "[output]\ntype = \"keys\"\n",
        )
        .unwrap();
        let config = load_config(tmp.path(), tmp.path()).unwrap();
        assert_eq!(
            config.values["output"],
            json!({"type": "keys", "start_enabled": false})
        );
    }

    #[test]
    fn merge_cases_match_python() {
        for case in fixture("config_merge.json").as_array().unwrap() {
            let tmp = tempfile::tempdir().unwrap();
            fs::write(
                tmp.path().join("config.toml"),
                case["base"].as_str().unwrap(),
            )
            .unwrap();
            if let Some(local) = case["local"].as_str() {
                fs::write(tmp.path().join("config.local.toml"), local).unwrap();
            }
            let config = load_config(tmp.path(), tmp.path()).unwrap();
            assert_eq!(Value::Object(config.values), case["result"], "{case}");
        }
    }

    #[test]
    fn apply_settings_matches_python() {
        let data = fixture("apply_settings.json");
        let sections = data["sections"].as_object().unwrap();
        assert_eq!(sections.len(), SETTINGS_SECTIONS.len());
        for (key, section) in sections {
            assert_eq!(settings_section(key), section.as_str(), "{key}");
        }
        for case in data["cases"].as_array().unwrap() {
            let mut config = Config {
                values: case["config"].as_object().unwrap().clone(),
            };
            config.apply_settings(case["settings"].as_object().unwrap());
            assert_eq!(Value::Object(config.values), case["result"]);
        }
    }

    #[test]
    fn unknown_settings_are_skipped() {
        let mut config = Map::new();
        let settings = json!({"obcy": 1, "theme": "dark"});
        apply_settings(&mut config, settings.as_object().unwrap());
        assert_eq!(Value::Object(config), json!({"ui": {"theme": "dark"}}));
    }

    #[test]
    fn errors_name_the_file() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = load_config(tmp.path(), tmp.path()).unwrap_err();
        assert!(matches!(missing, ConfigError::Read { .. }));
        fs::write(tmp.path().join("config.toml"), "[a]\n").unwrap();
        fs::write(tmp.path().join("config.local.toml"), "to nie jest = = toml").unwrap();
        let broken = load_config(tmp.path(), tmp.path()).unwrap_err();
        assert!(broken.to_string().contains("config.local.toml"), "{broken}");
    }

    #[test]
    fn section_as_reads_typed_values() {
        #[derive(serde::Deserialize)]
        struct Ble {
            pad_steps: Vec<(f64, f64)>,
            #[serde(default)]
            brak: Option<i64>,
        }
        let empty = tempfile::tempdir().unwrap();
        let config = load_config(&repo_root(), empty.path()).unwrap();
        let ble: Ble = config.section_as("ble").unwrap();
        assert_eq!(ble.pad_steps.first(), Some(&(0.0, 1.0)));
        assert!(ble.brak.is_none());
        assert!(config.section_as::<Ble>("nie_ma").is_err());
    }
}
