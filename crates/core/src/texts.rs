//! User-facing texts in the current language (docs/i18n.md). Polish keys and texts come from
//! docs/texts.md (`npm run texts`), messages are ICU MessageFormat:
//! `t("glowny.podpowiedz.trening", &[("klawisz_szybciej", "F7"), ("klawisz_wolniej", "F6")])`.
//! Lookup order: packs of the language and its BCP 47 parents (pt-BR, pt), then the built-in
//! English pack, which carries Polish texts for keys not translated yet.
//! Numbers passed as variables are already formatted with [`number`] in the current language.

mod locale;
pub mod message;
pub mod packs;

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, RwLock};

use serde::Serialize;

pub use locale::{LocaleFormat, NumberStyle};
pub use packs::{Availability, Language, LanguageEntry, built_in};

use crate::paths::AppPaths;

struct Overlay {
    code: String,
    format: Arc<LocaleFormat>,
    texts: HashMap<String, String>,
}

struct Active {
    /// language shown (its pack, a parent of it, or the built-in one)
    code: String,
    format: Arc<LocaleFormat>,
    overlays: Vec<Overlay>,
}

fn format_for(code: &str) -> Arc<LocaleFormat> {
    static CACHE: LazyLock<Mutex<HashMap<String, Arc<LocaleFormat>>>> =
        LazyLock::new(Default::default);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    cache
        .entry(code.to_string())
        .or_insert_with(|| {
            let style = built_in()
                .language(code)
                .and_then(|l| l.number.clone())
                .unwrap_or_default();
            Arc::new(LocaleFormat::new(code, style))
        })
        .clone()
}

fn built_in_active() -> Active {
    let code = built_in().language.clone();
    Active {
        format: format_for(&code),
        code,
        overlays: Vec::new(),
    }
}

/// Running from the repository (tests, `tauri dev`) starts in the source language, so texts
/// are Polish until the app picks its language; the installed app starts in English.
fn initial() -> Active {
    if crate::paths::is_installed() {
        return built_in_active();
    }
    let source = built_in().source.clone();
    let dir = crate::paths::resource_dir()
        .join(packs::LOCALES_DIR)
        .join(&source);
    match packs::read_pack(&dir) {
        Some(texts) => Active {
            format: format_for(&source),
            overlays: vec![Overlay {
                code: source.clone(),
                format: format_for(&source),
                texts,
            }],
            code: source,
        },
        None => built_in_active(),
    }
}

static ACTIVE: LazyLock<RwLock<Arc<Active>>> = LazyLock::new(|| RwLock::new(Arc::new(initial())));

fn active() -> Arc<Active> {
    ACTIVE.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Result of switching the language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Switched {
    /// language shown now
    pub shown: String,
    /// the chosen language has no pack on this computer (it should be downloaded)
    pub missing: bool,
}

/// Shows texts in `code` (BCP 47) from the packs on this computer.
pub fn set_language(paths: &AppPaths, code: &str) -> Switched {
    let mut overlays = Vec::new();
    for tag in packs::chain(code) {
        if let Some((dir, _)) = packs::local_pack(paths, &tag)
            && let Some(texts) = packs::read_pack(&dir)
        {
            overlays.push(Overlay {
                format: format_for(&tag),
                code: tag,
                texts,
            });
        }
    }
    let own = packs::chain(code);
    let missing = own
        .first()
        .is_some_and(|tag| !overlays.iter().any(|o| o.code == *tag));
    let shown = overlays
        .first()
        .map_or_else(|| built_in().language.clone(), |o| o.code.clone());
    *ACTIVE.write().unwrap_or_else(|e| e.into_inner()) = Arc::new(Active {
        format: format_for(&shown),
        code: shown.clone(),
        overlays,
    });
    log::info!(
        "[language-set] {code}: shown {shown}{}",
        if missing { ", pack missing" } else { "" }
    );
    Switched { shown, missing }
}

/// Language shown now (BCP 47).
pub fn language() -> String {
    active().code.clone()
}

/// Texts of the active packs for the interface (`language_pack`); the interface has the
/// built-in pack itself.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackView {
    pub code: String,
    pub direction: String,
    pub overlays: Vec<OverlayView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverlayView {
    pub code: String,
    pub texts: HashMap<String, String>,
}

pub fn pack_view() -> PackView {
    let a = active();
    PackView {
        code: a.code.clone(),
        direction: built_in()
            .language(&a.code)
            .map_or_else(|| "ltr".to_string(), |l| l.direction.clone()),
        overlays: a
            .overlays
            .iter()
            .map(|o| OverlayView {
                code: o.code.clone(),
                texts: o.texts.clone(),
            })
            .collect(),
    }
}

/// Whether the key exists.
pub fn has(key: &str) -> bool {
    built_in().texts.contains_key(key)
}

struct Context<'a> {
    /// numbers in variables are written in the shown language
    values: &'a LocaleFormat,
    /// plural rules of the message's language
    rules: &'a LocaleFormat,
}

impl message::Context for Context<'_> {
    fn plural(&self, value: &str) -> Option<(&'static str, f64)> {
        let d = self.values.parse(value)?;
        Some((self.rules.plural(&d), d.to_string().parse().ok()?))
    }
    fn number(&self, value: f64) -> String {
        self.values
            .number(value, if value.fract() == 0.0 { 0 } else { 1 })
    }
}

/// Text with variables. A missing variable stays visible as `{name}`; a missing key gives the
/// key (an error in tests).
pub fn t(key: &str, vars: &[(&str, &str)]) -> String {
    let a = active();
    let (pattern, rules) = match a
        .overlays
        .iter()
        .find_map(|o| o.texts.get(key).map(|t| (t, o)))
    {
        Some((text, o)) => (text.as_str(), o.format.clone()),
        None => {
            let Some(text) = built_in().texts.get(key) else {
                debug_assert!(false, "No text {key} in docs/texts.md");
                return key.to_string();
            };
            let lang = if built_in().from_source.contains(key) {
                &built_in().source
            } else {
                &built_in().language
            };
            (text.as_str(), format_for(lang))
        }
    };
    match message::parse(pattern) {
        Ok(parts) => message::format(
            &parts,
            vars,
            &Context {
                values: &a.format,
                rules: &rules,
            },
        ),
        Err(e) => {
            log::warn!("Text {key}: {e}");
            pattern.to_string()
        }
    }
}

/// Text of a key only when a language other than the source one translates it (names from
/// definicje.json: the definitions file itself is the Polish text and may be newer).
pub fn translated(key: &str) -> Option<String> {
    let a = active();
    let source = &built_in().source;
    let in_pack = a
        .overlays
        .iter()
        .find(|o| o.texts.contains_key(key))
        .map(|o| o.code != *source);
    match in_pack {
        Some(true) => Some(t(key, &[])),
        Some(false) => None,
        None => (has(key) && !built_in().from_source.contains(key)).then(|| t(key, &[])),
    }
}

/// Number in the current language: `places` digits after the decimal separator.
pub fn number(value: f64, places: usize) -> String {
    active().format.number(value, places)
}

/// `{kmh}` always with one digit after the decimal separator: "2,5", "6,0".
pub fn kmh(value: f64) -> String {
    number(value, 1)
}

/// Wording of 0.6.0 for a 0.5.2 message (Python fixtures in the compatibility tests): the
/// docs/texts.md row whose old text („Obecny tekst”) matches `old` (a key with one of the
/// prefixes) gives the new text with the same variables. No match - `old` unchanged. Tests only.
#[doc(hidden)]
pub fn from_052(old: &str, prefixes: &[&str]) -> String {
    let md = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/texts.md"),
    )
    .expect("docs/texts.md");
    // the most exact row: the most fixed text (not „{rola}: {klawisz}”)
    let mut best: Option<(usize, String)> = None;
    for line in md.lines().filter(|l| l.starts_with("| `")) {
        let cells: Vec<&str> = line.split(" | ").collect();
        let (Some(key), Some(before)) = (cells.first(), cells.get(1)) else {
            continue;
        };
        let key = key.trim_start_matches("| ").trim_matches('`');
        if !prefixes.iter().any(|p| key.starts_with(p)) || !has(key) {
            continue;
        }
        // the old text as a pattern: {variable} and {f1/f2/f5} match any text
        let mut names = Vec::new();
        let mut pattern = String::from("^");
        let mut rest = *before;
        while let Some(open) = rest.find('{') {
            pattern.push_str(&regex::escape(&rest[..open]));
            let Some(close) = rest[open..].find('}') else {
                break;
            };
            names.push(rest[open + 1..open + close].to_string());
            pattern.push_str("(.+?)");
            rest = &rest[open + close + 1..];
        }
        pattern.push_str(&regex::escape(rest));
        pattern.push('$');
        let Some(caps) = regex::Regex::new(&pattern)
            .ok()
            .and_then(|re| re.captures(old))
        else {
            continue;
        };
        let values: Vec<(String, String)> = names
            .iter()
            .enumerate()
            .map(|(i, n)| {
                // the reason (e.g. „F6 to skrót symulatora”) is a 0.5.2 text too
                let v = &caps[i + 1];
                let v = if n == "powod" {
                    from_052(v, prefixes)
                } else {
                    v.to_string()
                };
                (n.clone(), v)
            })
            .collect();
        // in 0.5.2 the faster/slower keys were fixed (F7/F6)
        let vars: Vec<(&str, &str)> = values
            .iter()
            .map(|(n, v)| (n.as_str(), v.as_str()))
            .chain([("klawisz_szybciej", "F7"), ("klawisz_wolniej", "F6")])
            .collect();
        let fixed = before.len() - names.iter().map(|n| n.len() + 2).sum::<usize>();
        if best.as_ref().is_none_or(|(f, _)| fixed > *f) {
            best = Some((fixed, t(key, &vars)));
        }
    }
    best.map_or_else(|| old.to_string(), |(_, text)| text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_variables_and_plurals() {
        assert_eq!(
            t("glowny.osoby.dzis", &[("kroki", "1\u{202f}234")]),
            "dziś 1\u{202f}234 kroki"
        );
        assert_eq!(t("glowny.osoby.dzis", &[("kroki", "1")]), "dziś 1 krok");
        assert_eq!(t("glowny.osoby.dzis", &[("kroki", "12")]), "dziś 12 kroków");
        assert_eq!(
            t("glowny.osoby.dzis", &[("kroki", "2,5")]),
            "dziś 2,5 kroki"
        );
        // a missing variable stays visible
        assert_eq!(
            t("glowny.osoby.dzis", &[]),
            "dziś {kroki} {kroki, plural, one {krok} few {kroki} many {kroków} other {kroki}}"
        );
    }

    #[test]
    fn polish_numbers() {
        assert_eq!(number(12345.0, 0), "12\u{202f}345");
        assert_eq!(number(1234.0, 0), "1\u{202f}234");
        assert_eq!(number(999.0, 0), "999");
        assert_eq!(kmh(6.0), "6,0");
        assert_eq!(kmh(2.54), "2,5");
        assert_eq!(number(-0.04, 1), "0,0");
    }

    /// Every built-in text parses and English texts keep the Polish variables.
    #[test]
    fn built_in_pack_is_valid() {
        let b = built_in();
        assert!(b.texts.len() > 1000);
        for (key, text) in &b.texts {
            assert!(message::parse(text).is_ok(), "{key}: {text}");
            assert!(packs::fits(key, text));
        }
        assert!(b.language(&b.source).is_some() && b.language(&b.language).is_some());
    }

    /// Every key used in Rust (`t("...")`) exists.
    #[test]
    fn every_used_key_exists() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut missing = Vec::new();
        let mut used = 0;
        for dir in ["crates/core/src", "src-tauri/src"] {
            let mut stack = vec![root.join(dir)];
            while let Some(d) = stack.pop() {
                for e in std::fs::read_dir(&d).unwrap().flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        stack.push(p);
                        continue;
                    }
                    if p.extension().is_none_or(|x| x != "rs") {
                        continue;
                    }
                    let src = std::fs::read_to_string(&p).unwrap();
                    for (i, _) in src.match_indices("t(\"") {
                        let before = src[..i].chars().next_back().unwrap_or(' ');
                        if before.is_alphanumeric() || before == '_' {
                            continue;
                        }
                        let rest = &src[i + 3..];
                        let key = &rest[..rest.find('"').unwrap()];
                        let key_chars = key.chars().all(|c| {
                            c.is_ascii_lowercase()
                                || c.is_ascii_digit()
                                || c == '_'
                                || c == '-'
                                || c == '.'
                        });
                        if !key_chars
                            || !key.contains('.')
                            || !key.starts_with(|c: char| c.is_ascii_lowercase())
                        {
                            continue;
                        }
                        used += 1;
                        if !has(key) {
                            missing.push(format!("{}: {key}", p.display()));
                        }
                    }
                }
            }
        }
        assert!(used > 20, "too few keys: {used}");
        assert!(missing.is_empty(), "not in docs/texts.md: {missing:#?}");
    }
}
