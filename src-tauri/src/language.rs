//! Language of the app (docs/i18n.md): the choice at start, switching without a restart and
//! downloading packs. Texts themselves live in `walklevel_core::texts`.

use serde::Serialize;
use serde_json::{Value, json};
use tauri::State;
use walklevel_core::texts::{self, Availability, Language, LanguageEntry, packs};

use crate::state::{App, Core, lock};
use crate::views::{CmdError, CmdResult, SliceName};

/// Shared settings keys (osoby.json `wspolne`).
pub const LANGUAGE_KEY: &str = "language";
pub const INSTALLER_ID_KEY: &str = "installer_language_id";

#[derive(Debug, Clone, Default)]
pub struct LanguageState {
    /// language chosen (shown when its pack is on this computer)
    pub chosen: String,
    /// languages from the repository manifest (newer than this version)
    pub remote: Vec<Language>,
    pub download: Option<Progress>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub code: String,
    pub done: usize,
    pub total: usize,
}

/// `language` slice (docs/contract.md).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageView {
    pub chosen: String,
    pub shown: String,
    pub direction: String,
    pub languages: Vec<LanguageEntry>,
    pub download: Option<Progress>,
    pub error: Option<String>,
}

impl App {
    pub fn language_view(&self) -> LanguageView {
        let shown = texts::language();
        let languages = packs::languages(&self.paths, &self.language.remote);
        let direction = languages
            .iter()
            .find(|l| l.language.code == shown)
            .map_or_else(|| "ltr".to_string(), |l| l.language.direction.clone());
        LanguageView {
            chosen: self.language.chosen.clone(),
            shown,
            direction,
            languages,
            download: self.language.download.clone(),
            error: self.language.error.clone(),
        }
    }

    /// Language at start; true - its pack has to be downloaded first.
    pub fn start_language(&mut self) -> bool {
        let local: Vec<String> = packs::languages(&self.paths, &[])
            .into_iter()
            .filter(|l| l.availability != Availability::Download)
            .map(|l| l.language.code)
            .collect();
        let choice = packs::start_language(&packs::StartInputs {
            saved: self.s(LANGUAGE_KEY),
            seen_installer_id: self.s(INSTALLER_ID_KEY),
            installer: packs::installer_request(&self.paths),
            existing_data: self.store.is_some() || self.legacy.is_some(),
            windows: packs::windows_languages(),
            local,
        });
        log::info!("[language-start] {} ({})", choice.language, choice.why);
        let saved = self.s(LANGUAGE_KEY) == choice.language;
        if let Some(id) = &choice.installer_id {
            self.settings.insert(INSTALLER_ID_KEY.into(), json!(id));
        }
        if !saved || choice.installer_id.is_some() {
            self.settings
                .insert(LANGUAGE_KEY.into(), json!(choice.language));
            self.save();
        }
        self.language.chosen = choice.language.clone();
        texts::set_language(&self.paths, &choice.language).missing
    }

    /// Everything the core writes as text is rebuilt in the new language.
    fn language_changed(&mut self) {
        self.mark(&SliceName::ALL);
    }
}

fn entry(core: &Core, code: &str) -> Option<Language> {
    let app = lock(&core.0.app);
    packs::languages(&app.paths, &app.language.remote)
        .into_iter()
        .find(|l| l.language.code.eq_ignore_ascii_case(code))
        .map(|l| l.language)
}

fn fetcher() -> Option<walklevel_core::treadmill::profiles::HttpFetcher> {
    walklevel_core::treadmill::profiles::HttpFetcher::with_limit(
        &format!("WalkLevel/{}", walklevel_core::version()),
        packs::MAX_FILE_BYTES,
    )
}

/// Downloads the chosen language in this thread (blocking) and shows it when it is the choice.
pub fn download(core: &Core, code: &str, announce: bool) {
    let Some(language) = entry(core, code) else {
        return;
    };
    let paths = core.with(|a| {
        a.language.download = Some(Progress {
            code: language.code.clone(),
            done: 0,
            total: packs::built_in().namespaces.len(),
        });
        a.language.error = None;
        a.mark(&[SliceName::Language]);
        a.paths.clone()
    });
    let result = match fetcher() {
        Some(f) => packs::download(
            &f,
            &packs::default_base_url(),
            &language,
            &paths,
            &mut |done, total| {
                core.with(|a| {
                    a.language.download = Some(Progress {
                        code: language.code.clone(),
                        done,
                        total,
                    });
                    a.mark(&[SliceName::Language]);
                });
            },
        ),
        None => Err(packs::DownloadError::Offline),
    };
    core.with(|a| {
        a.language.download = None;
        match result {
            Ok(_) => {
                if a.language.chosen.eq_ignore_ascii_case(&language.code) {
                    texts::set_language(&a.paths, &a.language.chosen);
                    a.language_changed();
                }
                if announce {
                    a.toast_text("language.downloaded", &[("jezyk", &language.name)]);
                }
            }
            Err(e) => {
                log::warn!("[language-download-failed] {}: {e:?}", language.code);
                a.language.error = Some(e.to_string());
            }
        }
        a.mark(&[SliceName::Language]);
    });
}

// --- commands ------------------------------------------------------------------------------

/// Texts of the shown language for a window (it has the built-in pack itself).
#[tauri::command]
pub async fn language_pack() -> CmdResult<texts::PackView> {
    Ok(texts::pack_view())
}

/// Chooses a language: shown at once when it is on this computer, otherwise downloaded first.
#[tauri::command]
pub async fn language_set(core: State<'_, Core>, code: String) -> CmdResult<()> {
    let core = core.inner().clone();
    if entry(&core, &code).is_none() {
        return Err(CmdError::invalid(texts::t("wspolne.blad_nieznany", &[])));
    }
    // one download at a time; the row of the language being downloaded shows the progress
    let busy = lock(&core.0.app).language.download.is_some();
    if busy {
        return Ok(());
    }
    let missing = core.with(|a| {
        a.language.chosen = code.clone();
        a.language.error = None;
        a.settings
            .insert(LANGUAGE_KEY.into(), Value::String(code.clone()));
        a.save();
        let switched = texts::set_language(&a.paths, &code);
        a.language_changed();
        switched.missing
    });
    if missing {
        tauri::async_runtime::spawn_blocking(move || download(&core, &code, true));
    }
    Ok(())
}

/// Languages from the repository manifest; without internet the list stays local (no error).
#[tauri::command]
pub async fn languages_refresh(core: State<'_, Core>) -> CmdResult<()> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(f) = fetcher() else {
            return;
        };
        match packs::fetch_manifest(&f, &packs::default_base_url()) {
            Ok(list) => core.with(|a| {
                a.language.remote = list;
                a.mark(&[SliceName::Language]);
            }),
            Err(e) => log::info!("[language-manifest] {e:?}"),
        }
    });
    Ok(())
}
