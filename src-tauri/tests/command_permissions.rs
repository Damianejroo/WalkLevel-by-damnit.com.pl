//! The app commands are listed in three places that must agree: `generate_handler!` in
//! `src/lib.rs`, `COMMANDS` in `build.rs` (the app manifest) and the `allow-*` permissions in
//! `capabilities/*.json`. A command missing from the manifest or from every capability is
//! rejected by the built app, so any difference fails here.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

fn read(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
    let from = text
        .find(start)
        .unwrap_or_else(|| panic!("missing {start}"))
        + start.len();
    let len = text[from..]
        .find(end)
        .unwrap_or_else(|| panic!("missing {end} after {start}"));
    &text[from..from + len]
}

fn unique(source: &str, names: impl IntoIterator<Item = String>) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for name in names {
        assert!(set.insert(name.clone()), "{source}: {name} listed twice");
    }
    set
}

fn handler_commands() -> BTreeSet<String> {
    let lib = read("src/lib.rs");
    let block = between(&lib, "generate_handler![", "]");
    unique(
        "generate_handler!",
        block
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.rsplit("::").next().unwrap_or(s).to_string()),
    )
}

fn manifest_commands() -> BTreeSet<String> {
    let build = read("build.rs");
    let block = between(&build, "const COMMANDS: &[&str] = &[", "];");
    unique(
        "COMMANDS",
        block
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.trim_matches('"').to_string()),
    )
}

fn capability_commands() -> Vec<(String, BTreeSet<String>)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities");
    let mut files: Vec<_> = fs::read_dir(&dir)
        .expect("capabilities folder")
        .map(|e| e.expect("capabilities entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let json: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(path).expect("capability file"))
                    .unwrap_or_else(|e| panic!("{name}: {e}"));
            let permissions = json["permissions"].as_array().expect("permissions array");
            let commands = permissions
                .iter()
                .filter_map(|p| p.as_str())
                .filter_map(|p| p.strip_prefix("allow-"))
                .map(|p| p.replace('-', "_"));
            (name.clone(), unique(&name, commands))
        })
        .collect()
}

fn difference(a: &BTreeSet<String>, b: &BTreeSet<String>) -> Vec<String> {
    a.difference(b).cloned().collect()
}

#[test]
fn handler_manifest_and_capabilities_list_the_same_commands() {
    let handler = handler_commands();
    let manifest = manifest_commands();
    assert!(
        !handler.is_empty(),
        "no commands found in generate_handler!"
    );
    assert_eq!(
        (
            difference(&handler, &manifest),
            difference(&manifest, &handler)
        ),
        (vec![], vec![]),
        "generate_handler! vs COMMANDS: (missing in COMMANDS, not in generate_handler!)"
    );

    let mut allowed = BTreeSet::new();
    for (file, commands) in capability_commands() {
        let unknown = difference(&commands, &manifest);
        assert!(unknown.is_empty(), "{file}: unknown commands {unknown:?}");
        allowed.extend(commands);
    }
    let unreachable = difference(&manifest, &allowed);
    assert!(
        unreachable.is_empty(),
        "commands no window may call: {unreachable:?}"
    );
}

#[test]
fn every_command_called_by_the_interface_is_registered() {
    let api = read("../src/lib/api.ts");
    let handler = handler_commands();
    let called: BTreeSet<String> = api
        .split("wywolaj")
        .skip(1)
        .filter_map(|rest| {
            let start = rest.find('(')? + 1;
            let quoted = rest[start..].trim_start().strip_prefix('\'')?;
            Some(quoted[..quoted.find('\'')?].to_string())
        })
        .collect();
    assert!(!called.is_empty(), "no commands found in api.ts");
    let missing = difference(&called, &handler);
    assert!(
        missing.is_empty(),
        "api.ts calls unregistered commands: {missing:?}"
    );
}
