//! Pobieranie profili bieżni (port tests/test_pobieranie.py): tylko dane z GitHuba tego
//! repozytorium, sprawdzone sumą i formatem. Bez sieci - serwer jest podróbką.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Value, json};
use walklevel_core::treadmill::profiles::{
    self, BUILTIN, BUILTIN_INDEX, DownloadError, FetchError, Fetched, Fetcher, INDEX, MAX_BYTES,
    ProfileRegistry,
};

fn new_profile() -> Value {
    json!({
        "format": 1,
        "id": "nowa-bieznia",
        "name": "Nowa Bieżnia 9",
        "driver": "ftms",
        "match": ["NB9"],
        "capabilities": {"speed_min": 0.8, "speed_max": 6.0},
    })
}

struct FakeServer {
    files: BTreeMap<String, Vec<u8>>,
    redirect: Option<String>,
}

impl Fetcher for FakeServer {
    fn fetch(&self, url: &str) -> Result<Fetched, FetchError> {
        let name = url.rsplit('/').next().unwrap_or("");
        let body = self.files.get(name).ok_or(FetchError::Status(404))?;
        let final_url = self.redirect.clone().unwrap_or_else(|| url.to_string());
        // jak prawdziwy klient: najwyżej MAX_BYTES + 1 bajtów
        Ok(Fetched {
            final_url,
            body: body.iter().take(MAX_BYTES + 1).copied().collect(),
        })
    }
}

fn server(files: BTreeMap<String, Vec<u8>>) -> FakeServer {
    FakeServer {
        files,
        redirect: None,
    }
}

fn catalog(extra: &[(&str, Vec<u8>)]) -> BTreeMap<String, Vec<u8>> {
    let mut files: BTreeMap<String, Vec<u8>> = BUILTIN
        .iter()
        .map(|(n, t)| (n.to_string(), t.as_bytes().to_vec()))
        .collect();
    for (name, data) in extra {
        files.insert(name.to_string(), data.clone());
    }
    let list: Vec<(String, Vec<u8>)> = files.iter().map(|(n, d)| (n.clone(), d.clone())).collect();
    let index = profiles::build_index(&list);
    files.insert(
        INDEX.to_string(),
        serde_json::to_vec(&index).expect("indeks"),
    );
    files
}

fn registry() -> (tempfile::TempDir, PathBuf, ProfileRegistry) {
    let tmp = tempfile::tempdir().expect("folder");
    let user_dir = tmp.path().join("profile");
    let registry = ProfileRegistry::load(&user_dir);
    (tmp, user_dir, registry)
}

fn base() -> String {
    profiles::default_base_url()
}

#[test]
fn index_in_repo_matches_builtin_profiles() {
    let saved: Value = serde_json::from_str(BUILTIN_INDEX).expect("indeks");
    let files: Vec<(String, Vec<u8>)> = BUILTIN
        .iter()
        .map(|(n, t)| (n.to_string(), t.as_bytes().to_vec()))
        .collect();
    assert_eq!(saved, profiles::build_index(&files));
}

#[test]
fn new_model_is_downloaded_and_known_ones_are_skipped() {
    let (_tmp, user_dir, mut registry) = registry();
    let files = catalog(&[(
        "nowa-bieznia.json",
        serde_json::to_vec(&new_profile()).expect("json"),
    )]);
    let result = registry
        .download(&server(files.clone()), &base())
        .expect("pobranie");
    assert_eq!(result.added, ["Nowa Bieżnia 9"]);
    assert!(result.rejected.is_empty());
    assert_eq!(result.same, files.len() - 2); // wbudowane już są
    assert!(user_dir.join("nowa-bieznia.json").exists());
    let found = registry.profile_for("NB9-1234", None).expect("profil");
    assert_eq!(found.source, profiles::SOURCE_USER);
    let again = registry
        .download(&server(files), &base())
        .expect("pobranie");
    assert!(again.added.is_empty() && again.updated.is_empty());
}

#[test]
fn changed_model_is_updated() {
    let (_tmp, _user_dir, mut registry) = registry();
    let mut changed: Value = serde_json::from_str(BUILTIN[4].1).expect("urevo");
    changed["capabilities"]["start_delay_s"] = json!(3.0);
    let files = catalog(&[("urevo.json", serde_json::to_vec(&changed).expect("json"))]);
    let result = registry
        .download(&server(files), &base())
        .expect("pobranie");
    assert_eq!(result.updated, ["Urevo"]);
    let urevo = registry.profile_for("URTM061", None).expect("urevo");
    assert_eq!(urevo.capabilities.start_delay_s, Some(3.0));
}

#[test]
fn wrong_checksum_and_bad_profile_are_rejected() {
    let (_tmp, user_dir, mut registry) = registry();
    let mut bad = new_profile();
    bad["driver"] = json!("python");
    let mut files = catalog(&[
        ("zly.json", serde_json::to_vec(&bad).expect("json")),
        (
            "nowa-bieznia.json",
            serde_json::to_vec(&new_profile()).expect("json"),
        ),
    ]);
    let mut swapped = new_profile();
    swapped["name"] = json!("Podmieniony"); // suma nie gra
    files.insert(
        "nowa-bieznia.json".into(),
        serde_json::to_vec(&swapped).expect("json"),
    );
    let mut result = registry
        .download(&server(files), &base())
        .expect("pobranie");
    result.rejected.sort();
    assert_eq!(result.rejected, ["nowa-bieznia.json", "zly.json"]);
    let saved = std::fs::read_dir(&user_dir).map(|d| d.count()).unwrap_or(0);
    assert_eq!(saved, 0);
}

#[test]
fn index_cannot_point_outside_profiles() {
    let zeros = "0".repeat(64);
    for name in [
        "../../dane/osoby.json",
        "x.exe",
        "a\\b.json",
        "C:x.json",
        INDEX,
    ] {
        let (_tmp, _user_dir, mut registry) = registry();
        let index = json!({"format": 1, "profile": [{"plik": name, "sha256": zeros}]});
        let files =
            BTreeMap::from([(INDEX.to_string(), serde_json::to_vec(&index).expect("json"))]);
        let result = registry.download(&server(files), &base());
        assert!(
            matches!(result, Err(DownloadError::BadEntry)),
            "{name}: {result:?}"
        );
    }
}

#[test]
fn only_github_over_https() {
    let (_tmp, _user_dir, mut registry) = registry();
    let redirected = FakeServer {
        files: catalog(&[]),
        redirect: Some("https://zly.example/indeks.json".into()),
    };
    let result = registry.download(&redirected, &base());
    assert!(
        matches!(&result, Err(e) if e.to_string().contains("przekierował")),
        "{result:?}"
    );
    let result = registry.download(&server(catalog(&[])), "http://raw.githubusercontent.com/x");
    assert!(
        matches!(&result, Err(e) if e.to_string().contains("bezpieczeństwa")),
        "{result:?}"
    );
}

#[test]
fn missing_catalog_and_huge_file_have_clear_messages() {
    let (_tmp, _user_dir, mut registry) = registry();
    let result = registry.download(&server(BTreeMap::new()), &base());
    assert!(
        matches!(&result, Err(e) if e.to_string().contains("nie jest jeszcze opublikowana")),
        "{result:?}"
    );
    let huge = BTreeMap::from([(INDEX.to_string(), vec![b' '; MAX_BYTES + 10])]);
    let result = registry.download(&server(huge), &base());
    assert!(
        matches!(&result, Err(e) if e.to_string().contains("za duży")),
        "{result:?}"
    );
    let broken = BTreeMap::from([(INDEX.to_string(), b"{\"format\": 2}".to_vec())]);
    assert!(matches!(
        registry.download(&server(broken), &base()),
        Err(DownloadError::BadIndex)
    ));
}

#[test]
fn profile_from_file_and_removal() {
    let (tmp, _user_dir, mut registry) = registry();
    let good = tmp.path().join("moja.json");
    std::fs::write(&good, serde_json::to_vec(&new_profile()).expect("json")).expect("zapis");
    assert_eq!(
        registry.install_from_file(&good).expect("profil").name,
        "Nowa Bieżnia 9"
    );
    assert!(registry.profile_for("NB9", None).is_some());
    assert!(registry.remove_user_profile("nowa-bieznia"));
    assert!(registry.profile_for("NB9", None).is_none());
    let broken = tmp.path().join("zly.json");
    std::fs::write(
        &broken,
        r#"{"format": 1, "id": "x", "name": "X", "driver": "dll"}"#,
    )
    .expect("zapis");
    let result = registry.install_from_file(&broken);
    assert!(
        matches!(&result, Err(e) if e.to_string().contains("nie jest poprawny profil")),
        "{result:?}"
    );
}

#[test]
fn registry_without_data_folder_cannot_download() {
    let mut registry = ProfileRegistry::builtin();
    assert!(matches!(
        registry.download(&server(catalog(&[])), &base()),
        Err(DownloadError::NoUserDir)
    ));
}
