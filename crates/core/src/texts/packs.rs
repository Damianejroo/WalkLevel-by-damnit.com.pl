//! Language packs (docs/i18n.md). The built-in pack (English, with Polish texts for keys not
//! translated yet) is compiled in from src/lib/texts-built-in.json. Other languages are folders
//! `locales/<BCP 47 tag>/<namespace>.json`: installed next to the exe by the installer, or
//! downloaded to the data folder from the repository on GitHub. A pack is data only: texts of
//! known keys whose variables match the built-in text; anything else is dropped.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::locale::NumberStyle;
use super::message;
use crate::paths::AppPaths;
use crate::treadmill::profiles::{FetchError, Fetcher};

pub const LOCALES_DIR: &str = "locales";
/// Manifest entry kept with a downloaded pack (languages unknown to this version). No namespace
/// file can have this name (namespace files never contain `_`); `language.json` is the file of
/// the `language` namespace.
pub const LANGUAGE_FILE: &str = "_language.json";
pub const MANIFEST_FILE: &str = "languages.json";
/// Request from the installer: `{"language": "pl", "id": "..."}` next to the exe.
pub const INSTALLER_FILE: &str = "installer.json";
pub const ALLOWED_HOST: &str = "raw.githubusercontent.com";
/// One namespace file (the largest Polish one has about 40 KB).
pub const MAX_FILE_BYTES: usize = 512 * 1024;

static BUILT_IN_JSON: &str = include_str!("../../../../src/lib/texts-built-in.json");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Language {
    pub code: String,
    pub name: String,
    pub english_name: String,
    pub status: String,
    #[serde(default = "ltr")]
    pub direction: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<NumberStyle>,
}

fn ltr() -> String {
    "ltr".into()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltIn {
    pub language: String,
    pub source: String,
    pub languages: Vec<Language>,
    pub namespaces: Vec<String>,
    pub from_source: HashSet<String>,
    pub texts: HashMap<String, String>,
}

impl BuiltIn {
    pub fn language(&self, code: &str) -> Option<&Language> {
        self.languages
            .iter()
            .find(|l| l.code.eq_ignore_ascii_case(code))
    }
}

pub fn built_in() -> &'static BuiltIn {
    static PACK: LazyLock<BuiltIn> =
        LazyLock::new(|| serde_json::from_str(BUILT_IN_JSON).expect("src/lib/texts-built-in.json"));
    &PACK
}

/// File of a namespace: kebab-case (profil_online -> profil-online.json).
pub fn namespace_file(namespace: &str) -> String {
    format!("{}.json", namespace.replace('_', "-"))
}

/// BCP 47 lookup chain without the built-in language: pt-BR -> [pt-BR, pt].
pub fn chain(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut tag = code.trim().to_string();
    while !tag.is_empty() {
        if !tag.eq_ignore_ascii_case(&built_in().language) {
            out.push(tag.clone());
        }
        match tag.rfind('-') {
            Some(i) => tag.truncate(i),
            None => break,
        }
    }
    out
}

/// Folders with packs: downloaded (data folder) first, then installed (next to the exe).
pub fn pack_dirs(paths: &AppPaths) -> Vec<PathBuf> {
    vec![
        paths.data.join(LOCALES_DIR),
        paths.resources.join(LOCALES_DIR),
    ]
}

/// Texts of one pack folder that fit this version; dropped entries are logged.
pub fn read_pack(dir: &Path) -> Option<HashMap<String, String>> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut texts = HashMap::new();
    let mut dropped = 0;
    let mut files = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.ends_with(".json") || name == LANGUAGE_FILE {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(Value::Object(map)) =
            serde_json::from_str::<Value>(raw.trim_start_matches('\u{feff}'))
        else {
            log::warn!("Language pack {}: not a JSON object", path.display());
            continue;
        };
        files += 1;
        for (key, value) in map {
            if key.starts_with('@') {
                continue;
            }
            match value {
                Value::String(text) if fits(&key, &text) => {
                    texts.insert(key, text);
                }
                _ => dropped += 1,
            }
        }
    }
    if files == 0 {
        return None;
    }
    if dropped > 0 {
        log::warn!(
            "Language pack {}: {dropped} texts do not fit this version",
            dir.display()
        );
    }
    Some(texts)
}

/// A pack text fits when the key is known and it has the same variables as the built-in text.
pub fn fits(key: &str, text: &str) -> bool {
    let Some(built_in) = built_in().texts.get(key) else {
        return false;
    };
    match (message::parse(text), message::parse(built_in)) {
        (Ok(a), Ok(b)) => message::argument_names(&a) == message::argument_names(&b),
        _ => false,
    }
}

/// Where a language is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Availability {
    BuiltIn,
    Installed,
    Downloaded,
    /// listed in a manifest, not on this computer
    Download,
}

/// A language for the list in Settings.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageEntry {
    #[serde(flatten)]
    pub language: Language,
    pub availability: Availability,
}

/// Folder of a pack on this computer: downloaded first, then installed.
pub fn local_pack(paths: &AppPaths, code: &str) -> Option<(PathBuf, Availability)> {
    let dirs = pack_dirs(paths);
    [(0, Availability::Downloaded), (1, Availability::Installed)]
        .into_iter()
        .map(|(i, a)| (dirs[i].join(code), a))
        .find(|(dir, _)| {
            std::fs::read_dir(dir).is_ok_and(|mut d| {
                d.any(|e| {
                    e.ok().is_some_and(|e| {
                        let n = e.file_name();
                        let n = n.to_string_lossy();
                        n.ends_with(".json") && n != LANGUAGE_FILE
                    })
                })
            })
        })
}

/// Built-in manifest, manifests of downloaded packs and the remote one (if fetched), with what
/// is on this computer.
pub fn languages(paths: &AppPaths, remote: &[Language]) -> Vec<LanguageEntry> {
    let mut list: Vec<Language> = built_in().languages.clone();
    let downloaded = pack_dirs(paths)[0].clone();
    if let Ok(dirs) = std::fs::read_dir(&downloaded) {
        for dir in dirs.flatten() {
            let file = dir.path().join(LANGUAGE_FILE);
            if let Some(l) = std::fs::read_to_string(file)
                .ok()
                .and_then(|t| serde_json::from_str::<Language>(&t).ok())
            {
                add_language(&mut list, l);
            }
        }
    }
    for l in remote {
        add_language(&mut list, l.clone());
    }
    list.into_iter()
        .map(|language| {
            let availability = if language.code.eq_ignore_ascii_case(&built_in().language) {
                Availability::BuiltIn
            } else {
                local_pack(paths, &language.code).map_or(Availability::Download, |(_, a)| a)
            };
            LanguageEntry {
                language,
                availability,
            }
        })
        .collect()
}

fn add_language(list: &mut Vec<Language>, l: Language) {
    if valid_code(&l.code) && !list.iter().any(|x| x.code.eq_ignore_ascii_case(&l.code)) {
        list.push(l);
    }
}

/// A BCP 47 tag as used for folder names: letters, digits and hyphens only.
pub fn valid_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= 35
        && code
            .split('-')
            .all(|s| !s.is_empty() && s.len() <= 8 && s.chars().all(|c| c.is_ascii_alphanumeric()))
}

/// The best language of the list for a BCP 47 tag (pl-PL -> pl), case-insensitive.
pub fn best_match<'a>(
    tag: &str,
    codes: impl IntoIterator<Item = &'a str> + Clone,
) -> Option<String> {
    let mut tag = tag.trim().replace('_', "-");
    while !tag.is_empty() {
        if let Some(c) = codes
            .clone()
            .into_iter()
            .find(|c| c.eq_ignore_ascii_case(&tag))
        {
            return Some(c.to_string());
        }
        match tag.rfind('-') {
            Some(i) => tag.truncate(i),
            None => break,
        }
    }
    None
}

/// What decides the language at start (all read before the window opens).
#[derive(Debug, Clone, Default)]
pub struct StartInputs {
    /// `language` from the shared settings ("" - never chosen)
    pub saved: String,
    /// installer request id applied before (`installer_language_id`)
    pub seen_installer_id: String,
    /// the installer file next to the exe: (language, id)
    pub installer: Option<(String, String)>,
    /// osoby.json existed before this start (an update from 0.6.3 or older without `language`)
    pub existing_data: bool,
    /// Windows display languages in order (BCP 47)
    pub windows: Vec<String>,
    /// languages that can be shown without a download
    pub local: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartChoice {
    pub language: String,
    /// installer request to remember as applied
    pub installer_id: Option<String>,
    pub why: &'static str,
}

/// Order: a new installer request, then the saved choice, then Polish for data from before
/// languages (every earlier version was Polish only), then the Windows language when its pack
/// is on this computer, then the built-in language.
pub fn start_language(input: &StartInputs) -> StartChoice {
    if let Some((language, id)) = &input.installer
        && valid_code(language)
        && *id != input.seen_installer_id
    {
        return StartChoice {
            language: language.clone(),
            installer_id: Some(id.clone()),
            why: "installer",
        };
    }
    if valid_code(&input.saved) {
        return StartChoice {
            language: input.saved.clone(),
            installer_id: None,
            why: "settings",
        };
    }
    if input.existing_data {
        return StartChoice {
            language: built_in().source.clone(),
            installer_id: None,
            why: "data-before-languages",
        };
    }
    for tag in &input.windows {
        if let Some(code) = best_match(tag, input.local.iter().map(String::as_str)) {
            return StartChoice {
                language: code,
                installer_id: None,
                why: "windows",
            };
        }
    }
    StartChoice {
        language: built_in().language.clone(),
        installer_id: None,
        why: "default",
    }
}

/// Request written by the installer next to the exe (see docs/i18n.md).
pub fn installer_request(paths: &AppPaths) -> Option<(String, String)> {
    let raw =
        std::fs::read_to_string(paths.resources.join(LOCALES_DIR).join(INSTALLER_FILE)).ok()?;
    let v: Value = serde_json::from_str(raw.trim_start_matches('\u{feff}')).ok()?;
    Some((
        v.get("language")?.as_str()?.to_string(),
        v.get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    ))
}

/// Windows display languages (BCP 47), most preferred first.
pub fn windows_languages() -> Vec<String> {
    #[cfg(windows)]
    {
        use windows::Win32::Globalization::{GetUserPreferredUILanguages, MUI_LANGUAGE_NAME};
        let mut count = 0u32;
        let mut len = 0u32;
        // SAFETY: the first call only asks for the buffer length.
        if unsafe { GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, None, &mut len) }
            .is_err()
        {
            return Vec::new();
        }
        let mut buf = vec![0u16; len as usize];
        // SAFETY: `buf` has the length Windows asked for.
        if unsafe {
            GetUserPreferredUILanguages(
                MUI_LANGUAGE_NAME,
                &mut count,
                Some(windows::core::PWSTR(buf.as_mut_ptr())),
                &mut len,
            )
        }
        .is_err()
        {
            return Vec::new();
        }
        String::from_utf16_lossy(&buf)
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    }
    #[cfg(not(windows))]
    Vec::new()
}

// --- download ------------------------------------------------------------------------------

/// Packs in the repository (branch main), the same place as treadmill profiles.
pub fn default_base_url() -> String {
    format!(
        "https://raw.githubusercontent.com/{}/{}/{LOCALES_DIR}",
        crate::treadmill::profiles::REPOSITORY,
        crate::treadmill::profiles::BRANCH
    )
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DownloadError {
    #[error("{}", super::t("language.error.offline", &[]))]
    Offline,
    #[error("{}", super::t("language.error.not-published", &[]))]
    NotPublished,
    #[error("{}", super::t("language.error.status", &[("kod", &.0.to_string())]))]
    Status(u16),
    #[error("{}", super::t("language.error.address", &[]))]
    Address,
    #[error("{}", super::t("language.error.format", &[]))]
    Format,
    #[error("{}", super::t("language.error.write", &[("szczegoly", .0)]))]
    Write(String),
}

fn get(fetcher: &dyn Fetcher, url: &str) -> Result<Vec<u8>, DownloadError> {
    let allowed = |u: &str| {
        reqwest::Url::parse(u)
            .is_ok_and(|u| u.scheme() == "https" && u.host_str() == Some(ALLOWED_HOST))
    };
    if !allowed(url) {
        return Err(DownloadError::Address);
    }
    let fetched = fetcher.fetch(url).map_err(|e| match e {
        FetchError::Status(404) => DownloadError::NotPublished,
        FetchError::Status(code) => DownloadError::Status(code),
        FetchError::Network => DownloadError::Offline,
    })?;
    if !allowed(&fetched.final_url) {
        return Err(DownloadError::Address);
    }
    if fetched.body.len() > MAX_FILE_BYTES {
        return Err(DownloadError::Format);
    }
    Ok(fetched.body)
}

/// Languages listed in the repository manifest (to offer languages newer than this version).
pub fn fetch_manifest(
    fetcher: &dyn Fetcher,
    base_url: &str,
) -> Result<Vec<Language>, DownloadError> {
    let body = get(fetcher, &format!("{base_url}/{MANIFEST_FILE}"))?;
    let v: Value = serde_json::from_slice(&body).map_err(|_| DownloadError::Format)?;
    let list = v.get("languages").cloned().ok_or(DownloadError::Format)?;
    let list: Vec<Language> = serde_json::from_value(list).map_err(|_| DownloadError::Format)?;
    Ok(list.into_iter().filter(|l| valid_code(&l.code)).collect())
}

/// Downloads every namespace this version knows into `<data>/locales/<code>/` (a namespace
/// the repository does not have yet is skipped: its keys fall back to the built-in pack).
/// `progress(done, total)` after each file. Returns the number of texts that fit.
pub fn download(
    fetcher: &dyn Fetcher,
    base_url: &str,
    language: &Language,
    paths: &AppPaths,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<usize, DownloadError> {
    if !valid_code(&language.code) {
        return Err(DownloadError::Address);
    }
    let namespaces = &built_in().namespaces;
    let mut files = Vec::new();
    for (i, ns) in namespaces.iter().enumerate() {
        let name = namespace_file(ns);
        match get(fetcher, &format!("{base_url}/{}/{name}", language.code)) {
            Ok(body) => {
                let v: Value = serde_json::from_slice(&body).map_err(|_| DownloadError::Format)?;
                if !v.is_object() {
                    return Err(DownloadError::Format);
                }
                files.push((name, body));
            }
            Err(DownloadError::NotPublished) => {}
            Err(e) => return Err(e),
        }
        progress(i + 1, namespaces.len());
    }
    if files.is_empty() {
        return Err(DownloadError::NotPublished);
    }
    let root = pack_dirs(paths)[0].clone();
    let target = root.join(&language.code);
    let temp = root.join(format!(".{}.download", language.code));
    let write = |e: std::io::Error| DownloadError::Write(e.to_string());
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).map_err(write)?;
    for (name, body) in &files {
        std::fs::write(temp.join(name), body).map_err(write)?;
    }
    let entry =
        serde_json::to_vec_pretty(language).map_err(|e| DownloadError::Write(e.to_string()))?;
    std::fs::write(temp.join(LANGUAGE_FILE), entry).map_err(write)?;
    let count = read_pack(&temp).map_or(0, |t| t.len());
    let _ = std::fs::remove_dir_all(&target);
    std::fs::rename(&temp, &target).map_err(write)?;
    log::info!(
        "[language-downloaded] {}: {} files, {count} texts",
        language.code,
        files.len()
    );
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::treadmill::profiles::Fetched;

    #[test]
    fn chain_and_matching() {
        assert_eq!(chain("pt-BR"), ["pt-BR", "pt"]);
        assert_eq!(chain("en-GB"), ["en-GB"]);
        assert!(chain("en").is_empty());
        let codes = ["en", "pl", "pt"];
        assert_eq!(best_match("pl-PL", codes).as_deref(), Some("pl"));
        assert_eq!(best_match("PT_br", codes).as_deref(), Some("pt"));
        assert_eq!(best_match("de-DE", codes), None);
        assert!(
            valid_code("pt-BR") && !valid_code("../x") && !valid_code("") && !valid_code("a b")
        );
    }

    #[test]
    fn start_language_order() {
        let base = StartInputs {
            windows: vec!["de-DE".into(), "pl-PL".into()],
            local: vec!["en".into(), "pl".into()],
            ..Default::default()
        };
        let pick = |i: &StartInputs| start_language(i).language;
        assert_eq!(pick(&base), "pl", "Windows language with a pack");
        let no_pack = StartInputs {
            local: vec!["en".into()],
            ..base.clone()
        };
        assert_eq!(pick(&no_pack), "en", "nothing else: English");
        let old = StartInputs {
            existing_data: true,
            windows: vec!["en-US".into()],
            ..no_pack.clone()
        };
        assert_eq!(pick(&old), "pl", "data from 0.6.3 and older was Polish");
        let saved = StartInputs {
            saved: "en".into(),
            ..old.clone()
        };
        assert_eq!(pick(&saved), "en");
        let installer = StartInputs {
            installer: Some(("pl".into(), "20261127-1".into())),
            ..saved.clone()
        };
        let c = start_language(&installer);
        assert_eq!(
            (c.language.as_str(), c.installer_id.as_deref()),
            ("pl", Some("20261127-1"))
        );
        // the same request (silent update) does not override a later choice in Settings
        let seen = StartInputs {
            seen_installer_id: "20261127-1".into(),
            ..installer
        };
        assert_eq!(pick(&seen), "en");
    }

    struct Fake(HashMap<String, Result<String, FetchError>>);
    impl Fetcher for Fake {
        fn fetch(&self, url: &str) -> Result<Fetched, FetchError> {
            match self.0.get(url) {
                Some(Ok(body)) => Ok(Fetched {
                    final_url: url.to_string(),
                    body: body.clone().into_bytes(),
                }),
                Some(Err(e)) => Err(*e),
                None => Err(FetchError::Status(404)),
            }
        }
    }

    fn german() -> Language {
        Language {
            code: "de".into(),
            name: "Deutsch".into(),
            english_name: "German".into(),
            status: "in-progress".into(),
            direction: "ltr".into(),
            number: None,
        }
    }

    #[test]
    fn download_keeps_only_fitting_texts() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(dir.path().join("res"), dir.path().join("data"));
        let base = "https://raw.githubusercontent.com/x/y/main/locales";
        let mut files = HashMap::new();
        files.insert(
            format!("{base}/de/app.json"),
            Ok(r#"{"@@locale":"de","app.wroc":"Zurück","app.wersja":"Version {falsch}","x.y":"?"}"#.to_string()),
        );
        files.insert(
            format!("{base}/de/language.json"),
            Ok(r#"{"@@locale":"de","language.title":"Sprache"}"#.to_string()),
        );
        let mut steps = 0;
        let count = download(&Fake(files), base, &german(), &paths, &mut |_, _| {
            steps += 1
        })
        .unwrap();
        assert_eq!(count, 2);
        assert_eq!(steps, built_in().namespaces.len());
        let texts = read_pack(&paths.data.join("locales/de")).unwrap();
        assert_eq!(texts.get("app.wroc").map(String::as_str), Some("Zurück"));
        assert_eq!(
            texts.get("language.title").map(String::as_str),
            Some("Sprache")
        );
        assert!(!texts.contains_key("app.wersja"));
        let list = languages(&paths, &[]);
        let de = list.iter().find(|l| l.language.code == "de").unwrap();
        assert_eq!(de.availability, Availability::Downloaded);
        assert_eq!(
            list.iter()
                .find(|l| l.language.code == "en")
                .unwrap()
                .availability,
            Availability::BuiltIn
        );
    }

    #[test]
    fn download_errors() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(dir.path().join("res"), dir.path().join("data"));
        let base = "https://raw.githubusercontent.com/x/y/main/locales";
        let err = |e| {
            download(
                &Fake(HashMap::from([(format!("{base}/de/app.json"), Err(e))])),
                base,
                &german(),
                &paths,
                &mut |_, _| {},
            )
        };
        assert_eq!(err(FetchError::Network), Err(DownloadError::Offline));
        assert_eq!(
            err(FetchError::Status(500)),
            Err(DownloadError::Status(500))
        );
        assert_eq!(
            download(
                &Fake(HashMap::new()),
                base,
                &german(),
                &paths,
                &mut |_, _| {}
            ),
            Err(DownloadError::NotPublished)
        );
        assert_eq!(
            download(
                &Fake(HashMap::new()),
                "http://example.com",
                &german(),
                &paths,
                &mut |_, _| {}
            ),
            Err(DownloadError::Address)
        );
        assert!(!paths.data.join("locales/de").exists());
    }
}
