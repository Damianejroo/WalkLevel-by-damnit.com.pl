//! Stan włączony/wyłączony między restartami po aktualizacji - port części bieznia/settings.py.
//! (Ustawienia 0.2 z dane/ustawienia.json czyta tylko przeniesienie do osób:
//! [`super::legacy_settings`].)

use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::py::truthy;

/// Stan włączony/wyłączony w folderze danych.
pub const STATE_FILE: &str = "stan.json";
/// Zmienna środowiska ustawiana przy restarcie po aktualizacji.
pub const RESUME_ENV: &str = "BIEZNIA_RESUME";

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
}
