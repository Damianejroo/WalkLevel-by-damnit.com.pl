//! Gdzie co leży - port bieznia/paths.py.
//!
//! - zasoby (config.toml, ikony, poziomy): obok exe w wersji zainstalowanej, w repozytorium
//!   przy uruchamianiu z kodu (build debug),
//! - dane użytkownika (dane/, logs/, nagrania/): %LOCALAPPDATA%\Bieznia w wersji zainstalowanej
//!   (przetrwają aktualizacje i odinstalowanie, ten sam folder co 0.5.x), bieżący folder przy kodzie.
//!
//! Zmienna środowiskowa `WALKLEVEL_DANE` wskazuje inny folder danych w obu trybach: testy
//! i wersja testowa nie ruszają wtedy prawdziwych danych.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Nazwa folderu danych w %LOCALAPPDATA% (bez zmian od 0.1 - stare dane wczytują się same).
pub const APP_DIR_NAME: &str = "Bieznia";
/// Zmienna środowiskowa z folderem danych (testy, wersja testowa).
pub const DATA_ENV: &str = "WALKLEVEL_DANE";
/// Podfolder z danymi użytkownika (osoby.json, konto.json, historia.db).
pub const DATA_SUBDIR: &str = "dane";
/// Podfolder z logami.
pub const LOGS_SUBDIR: &str = "logs";
/// Podfolder z nagraniami odczytów bieżni.
pub const RECORDINGS_SUBDIR: &str = "nagrania";

/// Wersja zainstalowana (build release) czy uruchamianie z kodu (build debug).
pub fn is_installed() -> bool {
    !cfg!(debug_assertions)
}

/// Folder zasobów: obok exe w wersji zainstalowanej, katalog główny repozytorium z kodu.
pub fn resource_dir() -> PathBuf {
    if is_installed()
        && let Some(dir) = exe_dir()
    {
        return dir;
    }
    repo_root()
}

/// Folder danych użytkownika według zasad z opisu modułu (bez tworzenia go).
pub fn data_dir() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    resolve_data_dir(
        is_installed(),
        std::env::var_os(DATA_ENV),
        std::env::var_os("LOCALAPPDATA"),
        std::env::var_os("USERPROFILE"),
        &cwd,
    )
}

/// Wersja aplikacji zapisywana w sesjach: numer z Cargo w wersji zainstalowanej,
/// "dev" przy uruchamianiu z kodu (jak w Pythonie bez wersja.txt).
pub fn app_version() -> &'static str {
    if is_installed() {
        env!("CARGO_PKG_VERSION")
    } else {
        "dev"
    }
}

/// Logika wyboru folderu danych - osobno, żeby dało się ją sprawdzić bez zmiennych procesu.
///
/// Kolejność: `WALKLEVEL_DANE` (względna liczy się od `cwd`), potem w wersji zainstalowanej
/// %LOCALAPPDATA%\Bieznia (bez LOCALAPPDATA: %USERPROFILE%\AppData\Local\Bieznia), z kodu `cwd`.
pub fn resolve_data_dir(
    installed: bool,
    data_env: Option<OsString>,
    local_app_data: Option<OsString>,
    user_profile: Option<OsString>,
    cwd: &Path,
) -> PathBuf {
    if let Some(dir) = data_env.filter(|v| !v.is_empty()) {
        return cwd.join(dir);
    }
    if !installed {
        return cwd.to_path_buf();
    }
    let base = match local_app_data.filter(|v| !v.is_empty()) {
        Some(base) => PathBuf::from(base),
        None => user_profile
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| cwd.to_path_buf())
            .join("AppData")
            .join("Local"),
    };
    base.join(APP_DIR_NAME)
}

/// Ścieżki aplikacji w jednym miejscu: zasoby i dane użytkownika.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    /// zasoby tylko do odczytu (config.toml i inne pliki dołączone do wydania)
    pub resources: PathBuf,
    /// folder danych użytkownika (w nim dane/, logs/, nagrania/, config.local.toml)
    pub data: PathBuf,
}

impl AppPaths {
    /// Ścieżki dla bieżącego procesu (zasady z opisu modułu).
    pub fn detect() -> Self {
        Self {
            resources: resource_dir(),
            data: data_dir(),
        }
    }

    /// Ścieżki podane wprost (testy, narzędzia).
    pub fn new(resources: impl Into<PathBuf>, data: impl Into<PathBuf>) -> Self {
        Self {
            resources: resources.into(),
            data: data.into(),
        }
    }

    /// dane/ - osoby.json, konto.json, historia.db
    pub fn data_files(&self) -> PathBuf {
        self.data.join(DATA_SUBDIR)
    }

    /// logs/ - bieznia.log, bluetooth.log
    pub fn logs(&self) -> PathBuf {
        self.data.join(LOGS_SUBDIR)
    }

    /// nagrania/ - zapisane odczyty bieżni
    pub fn recordings(&self) -> PathBuf {
        self.data.join(RECORDINGS_SUBDIR)
    }

    /// Tworzy folder danych i jego podfoldery dane/ oraz logs/.
    pub fn ensure(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.data_files())?;
        std::fs::create_dir_all(self.logs())
    }
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(Path::to_path_buf)
}

/// Katalog walklevel/ (z kodu: tam leży config.toml) - walklevel/crates/core -> dwa poziomy wyżej.
fn repo_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .nth(2)
        .unwrap_or(manifest)
        .to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(s: &str) -> Option<OsString> {
        Some(OsString::from(s))
    }

    #[test]
    fn installed_uses_local_app_data() {
        let cwd = Path::new(r"C:\gdzies");
        let dir = resolve_data_dir(true, None, os(r"C:\Users\a\AppData\Local"), None, cwd);
        assert_eq!(dir, PathBuf::from(r"C:\Users\a\AppData\Local\Bieznia"));
    }

    #[test]
    fn installed_without_local_app_data_falls_back_to_profile() {
        let cwd = Path::new(r"C:\gdzies");
        let dir = resolve_data_dir(true, os(""), os(""), os(r"C:\Users\a"), cwd);
        assert_eq!(dir, PathBuf::from(r"C:\Users\a\AppData\Local\Bieznia"));
    }

    #[test]
    fn data_dir_matches_python() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/baza/data_dir.json");
        let cases: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let text = |v: &serde_json::Value| v.as_str().map(OsString::from);
        for case in cases.as_array().unwrap() {
            let dir = resolve_data_dir(
                case["frozen"].as_bool().unwrap(),
                None,
                text(&case["local_app_data"]),
                text(&case["user_profile"]),
                Path::new(case["cwd"].as_str().unwrap()),
            );
            assert_eq!(
                dir,
                PathBuf::from(case["result"].as_str().unwrap()),
                "{case}"
            );
        }
    }

    #[test]
    fn dev_uses_current_dir() {
        let cwd = Path::new(r"D:\repo");
        assert_eq!(resolve_data_dir(false, None, os(r"C:\L"), None, cwd), cwd);
    }

    #[test]
    fn env_variable_wins_in_both_modes() {
        let cwd = Path::new(r"D:\repo");
        for installed in [true, false] {
            let abs = resolve_data_dir(installed, os(r"E:\test"), os(r"C:\L"), None, cwd);
            assert_eq!(abs, PathBuf::from(r"E:\test"));
            let rel = resolve_data_dir(installed, os("dane-testowe"), os(r"C:\L"), None, cwd);
            assert_eq!(rel, PathBuf::from(r"D:\repo\dane-testowe"));
        }
    }

    #[test]
    fn subfolders_and_dev_resources() {
        let paths = AppPaths::new("zasoby", "dane-uzytkownika");
        assert_eq!(
            paths.data_files(),
            Path::new("dane-uzytkownika").join("dane")
        );
        assert_eq!(paths.logs(), Path::new("dane-uzytkownika").join("logs"));
        assert_eq!(
            paths.recordings(),
            Path::new("dane-uzytkownika").join("nagrania")
        );
        if !is_installed() {
            // z kodu zasoby to repozytorium - leży tam config.toml
            assert!(resource_dir().join("config.toml").is_file());
            assert_eq!(app_version(), "dev");
        }
    }

    #[test]
    fn ensure_creates_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(tmp.path(), tmp.path().join("nowe"));
        paths.ensure().unwrap();
        assert!(paths.data_files().is_dir());
        assert!(paths.logs().is_dir());
    }
}
