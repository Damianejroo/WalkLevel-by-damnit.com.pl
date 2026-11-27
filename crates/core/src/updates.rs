//! Aktualizacja na żądanie z GitHub Releases (port `bieznia/aktualizacje.py`).
//!
//! 1. Pytamy API GitHuba o najnowsze wydanie TEGO repozytorium ([`fetch_latest`]).
//! 2. Jeśli jest nowsza wersja ([`is_newer`]), pobieramy jej instalator i podpis wyłącznie
//!    z `github.com/<REPOSITORY>/releases/download/...` i z CDN GitHuba, na który to
//!    przekierowuje ([`download`]).
//! 3. Sprawdzamy SHA-256 z danych wydania i podpis minisign kluczem autora
//!    ([`SIGNING_PUBLIC_KEY`], docs/release-signing.md) - inna suma, brak podpisu albo zły
//!    podpis = plik odrzucony i usunięty.
//! 4. Uruchamiamy instalator w trybie cichym ([`run_installer`]); aplikacja zaraz potem się
//!    zamyka, instalator podmienia pliki i uruchamia nową wersję. Dane użytkownika zostają.
//!
//! Bez kliknięcia nic nie jest pobierane ani instalowane. Sieć idzie przez [`HttpClient`],
//! więc testy podstawiają własne odpowiedzi. [`ReqwestClient`] jest blokujący - w aplikacji
//! wywołuj te funkcje z wątku roboczego (np. `spawn_blocking`), nie z zadania async.
//!
//! Świadome różnice względem 0.5.x (tam kończyły się wyjątkiem albo były luką): adres musi
//! wskazywać ten sam host także według parsera reqwest; nazwa zasobu z końcowym "\n" nie
//! pasuje; dziwny `size` (ujemny, nieliczbowy) to rozmiar nieznany; `assets` innego typu niż
//! lista to brak instalatora; błąd tworzenia folderu to zwykły [`UpdateError::DownloadFailed`].

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Duration;

use base64::Engine;
use minisign_verify::{PublicKey, Signature};
use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::compat::{strip as py_strip, truthy};

/// Repozytorium z wydaniami (konto przemianowane w 0.6.4; stara nazwa jest wolna).
pub const REPOSITORY: &str = "damnit-damian/WalkLevel-by-damnit.com.pl";
/// Adres API z najnowszym wydaniem.
pub const API_LATEST: &str =
    "https://api.github.com/repos/damnit-damian/WalkLevel-by-damnit.com.pl/releases/latest";
/// Host API. Its answer is checked by host only: after a rename GitHub redirects the API to
/// `/repositories/<id>/...`, so the path cannot be pinned.
pub const API_HOST: &str = "api.github.com";
/// Host of `browser_download_url`; the path must also be this repository's release download.
pub const DOWNLOAD_HOST: &str = "github.com";
/// GitHub CDN that release downloads redirect to. Their paths carry only a numeric id and a
/// signed token (an undocumented format that GitHub has already changed once), so only the
/// host is checked; the file itself is checked by SHA-256 and the signature.
pub const CDN_HOSTS: [&str; 2] = [
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];
/// The owner's minisign public key for release signatures: the content of the `.pub` file
/// from `npx tauri signer generate` (docs/release-signing.md). A release build does not
/// compile while this is the placeholder.
pub const SIGNING_PUBLIC_KEY: &str =
    "PLACEHOLDER: paste the owner's public key here, see docs/release-signing.md";

#[cfg(not(debug_assertions))]
const _: () = assert!(
    !starts_with(SIGNING_PUBLIC_KEY.as_bytes(), b"PLACEHOLDER"),
    "SIGNING_PUBLIC_KEY in crates/core/src/updates.rs is not set, see docs/release-signing.md"
);

#[cfg(not(debug_assertions))]
const fn starts_with(text: &[u8], prefix: &[u8]) -> bool {
    if text.len() < prefix.len() {
        return false;
    }
    let mut i = 0;
    while i < prefix.len() {
        if text[i] != prefix[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Górny limit rozmiaru instalatora.
pub const MAX_SIZE: u64 = 200 * 1024 * 1024;
/// Limit czasu połączenia i każdego odczytu.
pub const TIMEOUT: Duration = Duration::from_secs(20);
/// Parametry cichej instalacji (Inno Setup), jak w 0.5.x.
pub const INSTALLER_ARGS: [&str; 4] = [
    "/VERYSILENT",
    "/SUPPRESSMSGBOXES",
    "/NORESTART",
    "/CLOSEAPPLICATIONS",
];

const CHUNK: usize = 256 * 1024;
const MAX_API_BYTES: u64 = 1_000_000;
const MAX_SIGNATURE_BYTES: u64 = 4096;

/// Installer name since 0.6.4; `Bieznia-Setup-X.Y.Z.exe` (0.5.x-0.6.3) is the fallback.
static ASSET: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^(WalkLevel|Bieznia)-Setup-\d+\.\d+\.\d+\.exe$").ok());
static VERSION: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^v?(\d+)\.(\d+)\.(\d+)$").ok());
/// Version with a pre-release (`-rc.1`) or build (`+5`) suffix, e.g. a test build.
static SUFFIXED: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"^v?(\d+)\.(\d+)\.(\d+)(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$").ok()
});

/// Błąd aktualizacji. `Display` to gotowy komunikat dla użytkownika (po polsku).
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("{}", crate::texts::t("aktualizacja.powod.adres", &[]))]
    UnexpectedUrl,
    #[error("{}", crate::texts::t("aktualizacja.powod.numer", &[]))]
    InvalidVersion,
    #[error("{}", crate::texts::t("aktualizacja.powod.suma_brak", &[]))]
    MissingChecksum,
    #[error("{}", crate::texts::t("aktualizacja.powod.brak_instalatora", &[]))]
    MissingInstaller,
    #[error("{}", crate::texts::t("aktualizacja.powod.brak_wydan", &[]))]
    NoReleases,
    #[error("{}", crate::texts::t("aktualizacja.powod.kod", &[("kod", &.0.to_string())]))]
    HttpStatus(u16),
    #[error("{}", crate::texts::t("aktualizacja.powod.internet", &[]))]
    Network,
    #[error("{}", crate::texts::t("aktualizacja.powod.odpowiedz", &[]))]
    UnexpectedResponse,
    #[error("{}", crate::texts::t("aktualizacja.powod.za_duzy", &[]))]
    TooLarge,
    #[error("{}", crate::texts::t("aktualizacja.powod.suma", &[]))]
    ChecksumMismatch,
    #[error("{}", crate::texts::t("aktualizacja.powod.signature-missing", &[]))]
    MissingSignature,
    #[error("{}", crate::texts::t("aktualizacja.powod.signature-invalid", &[]))]
    BadSignature,
    #[error("{}", crate::texts::t("aktualizacja.powod.przerwane", &[]))]
    DownloadFailed,
    #[error("{}", crate::texts::t("aktualizacja.powod.system", &[]))]
    WindowsOnly,
    #[error("{}", crate::texts::t("aktualizacja.instalator_blad", &[]))]
    Launch(#[source] io::Error),
}

/// Błąd warstwy HTTP zwracany przez [`HttpClient`].
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    /// Serwer odpowiedział kodem spoza 2xx.
    #[error("HTTP {0}")]
    Status(u16),
    /// Brak sieci, przekroczony czas, błąd TLS itp.
    #[error("{0}")]
    Other(String),
}

/// Odpowiedź HTTP: adres końcowy (po przekierowaniach) i treść do czytania strumieniowo.
pub struct HttpResponse {
    /// Adres po wszystkich przekierowaniach (sprawdzany tak jak adres początkowy).
    pub final_url: String,
    /// Treść odpowiedzi.
    pub body: Box<dyn Read + Send>,
}

/// Klient HTTP GET - w aplikacji [`ReqwestClient`], w testach odpowiedzi z pamięci.
pub trait HttpClient {
    /// Wysyła GET z nagłówkami; kod spoza 2xx zwraca jako [`TransportError::Status`].
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<HttpResponse, TransportError>;
}

/// Prawdziwy klient (reqwest, blokujący, rustls) z limitem czasu [`TIMEOUT`].
pub struct ReqwestClient {
    client: reqwest::blocking::Client,
}

impl ReqwestClient {
    /// Tworzy klienta. Nie wolno go tworzyć ani używać wewnątrz zadania async (tokio).
    pub fn new() -> Result<Self, UpdateError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(TIMEOUT)
            .connect_timeout(TIMEOUT)
            .build()
            .map_err(|e| {
                log::warn!("Aktualizacje: nie udało się utworzyć klienta HTTP: {e}");
                UpdateError::Network
            })?;
        Ok(Self { client })
    }
}

impl HttpClient for ReqwestClient {
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<HttpResponse, TransportError> {
        let mut request = self.client.get(url);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = request
            .send()
            .map_err(|e| TransportError::Other(e.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(TransportError::Status(status.as_u16()));
        }
        Ok(HttpResponse {
            final_url: response.url().to_string(),
            body: Box::new(response),
        })
    }
}

/// Wydanie z instalatorem gotowym do pobrania.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Release {
    /// Numer wersji bez "v", np. "0.6.1".
    pub version: String,
    /// Opis wydania (Markdown z GitHuba), przycięty.
    pub notes: String,
    /// Adres pobrania instalatora (sprawdzony host GitHuba).
    pub url: String,
    /// Oczekiwana suma SHA-256 (małe litery hex).
    pub sha256: String,
    /// Zapowiadany rozmiar w bajtach; 0 = nieznany.
    pub size: u64,
    /// Nazwa pliku, np. "WalkLevel-Setup-0.6.4.exe".
    pub name: String,
    /// Address of `<name>.sig` (checked like `url`); `None` when the release has no signature.
    #[serde(skip)]
    pub signature_url: Option<String>,
}

/// "v1.2.3" / "1.2.3" -> (1, 2, 3). Wszystko inne (np. "dev") to (0, 0, 0), czyli najstarsza.
pub fn parse_version(text: &str) -> (u64, u64, u64) {
    let Some(re) = VERSION.as_ref() else {
        return (0, 0, 0);
    };
    let Some(caps) = re.captures(py_strip(text)) else {
        return (0, 0, 0);
    };
    let part = |i: usize| caps.get(i).and_then(|m| m.as_str().parse::<u64>().ok());
    match (part(1), part(2), part(3)) {
        (Some(a), Some(b), Some(c)) => (a, b, c),
        _ => (0, 0, 0),
    }
}

/// Version and whether it is a release: 0.7.0-rc.1 comes before 0.7.0, a build suffix does
/// not count. Anything else (e.g. "dev") is the oldest, like in [`parse_version`].
fn version_order(text: &str) -> ((u64, u64, u64), bool) {
    let caps = SUFFIXED.as_ref().and_then(|re| re.captures(py_strip(text)));
    let Some(caps) = caps else {
        return (parse_version(text), true);
    };
    let part = |i: usize| caps.get(i).and_then(|m| m.as_str().parse::<u64>().ok());
    match (part(1), part(2), part(3)) {
        (Some(a), Some(b), Some(c)) => ((a, b, c), caps.get(4).is_none()),
        _ => ((0, 0, 0), true),
    }
}

/// Czy `candidate` jest nowsza niż `current` (np. `crate::version()`).
pub fn is_newer(candidate: &str, current: &str) -> bool {
    version_order(candidate) > version_order(current)
}

/// Odpowiednik `str()` z Pythona dla wartości JSON.
fn py_str(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => "None".to_owned(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(true)) => "True".to_owned(),
        Some(Value::Bool(false)) => "False".to_owned(),
        Some(other) => other.to_string(),
    }
}

/// Odpowiednik `str(obj.get(key, ""))`.
fn py_get_str(obj: &Value, key: &str) -> String {
    match obj.get(key) {
        None => String::new(),
        value => py_str(value),
    }
}

/// Odpowiednik `str(x or "")`: wartość "fałszywa" daje pusty tekst.
fn py_str_or_empty(value: Option<&Value>) -> String {
    if value.is_some_and(truthy) {
        py_str(value)
    } else {
        String::new()
    }
}

/// Odpowiednik `int(x or 0)`; ujemne i nieczytelne wartości dają 0 (rozmiar nieznany).
fn py_size(value: Option<&Value>) -> u64 {
    match value {
        Some(Value::Number(n)) => n
            .as_u64()
            .or_else(|| n.as_f64().filter(|f| *f >= 0.0).map(|f| f.trunc() as u64))
            .unwrap_or(0),
        Some(Value::String(s)) => py_strip(s).parse::<u64>().unwrap_or(0),
        Some(Value::Bool(true)) => 1,
        _ => 0,
    }
}

/// Schemat i host tak, jak widzi je `urllib.parse.urlparse` w 0.5.x (małe litery).
fn py_scheme_host(url: &str) -> Option<(String, String)> {
    let url: String = url
        .trim_start_matches(|c: char| c <= ' ')
        .chars()
        .filter(|c| !matches!(c, '\t' | '\r' | '\n'))
        .collect();
    let (scheme, rest) = url.split_once(':')?;
    let valid_scheme = scheme
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    let authority = rest.strip_prefix("//").filter(|_| valid_scheme)?;
    let netloc = authority.split(['/', '?', '#']).next().unwrap_or_default();
    let hostinfo = netloc.rsplit('@').next().unwrap_or_default();
    let host = match hostinfo.split_once('[') {
        Some((_, bracketed)) => bracketed.split(']').next().unwrap_or_default(),
        None => hostinfo.split(':').next().unwrap_or_default(),
    };
    (!host.is_empty()).then(|| (scheme.to_lowercase(), host.to_lowercase()))
}

/// What a checked address may be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    /// The GitHub API (host only).
    Api,
    /// A file of this repository's release: `github.com/<REPOSITORY>/releases/download/<tag>/<file>`.
    Asset,
    /// Where an [`Target::Asset`] download ends up: the same address or GitHub's CDN.
    AssetRedirect,
}

/// `/<REPOSITORY>/releases/download/<tag>/<file>`; the owner and repository ignore case, as
/// on GitHub. The path comes from reqwest's parser, so `..` is already resolved; `%` is
/// refused so an encoded `/` cannot reach another path on the server.
fn is_release_download(path: &str) -> bool {
    let prefix = format!("/{REPOSITORY}/releases/download/");
    let Some((head, rest)) = path.split_at_checked(prefix.len()) else {
        return false;
    };
    let segments: Vec<&str> = rest.split('/').collect();
    head.eq_ignore_ascii_case(&prefix)
        && !path.contains('%')
        && segments.len() == 2
        && segments.iter().all(|s| !s.is_empty())
}

/// Tylko https i host GitHuba - zarówno według reguł 0.5.x (`urlparse`), jak i według parsera,
/// którym reqwest naprawdę łączy (np. `\` czy `%68` w adresie nie przemyci innego hosta).
fn check_url(url: &str, target: Target) -> Result<(), UpdateError> {
    let Some((scheme, host)) = py_scheme_host(url) else {
        return Err(UpdateError::UnexpectedUrl);
    };
    let parsed = reqwest::Url::parse(url).map_err(|_| UpdateError::UnexpectedUrl)?;
    let asset = || host == DOWNLOAD_HOST && is_release_download(parsed.path());
    let place = match target {
        Target::Api => host == API_HOST,
        Target::Asset => asset(),
        Target::AssetRedirect => asset() || CDN_HOSTS.contains(&host.as_str()),
    };
    let allowed = scheme == "https"
        && place
        && parsed.scheme() == "https"
        && parsed.host_str() == Some(host.as_str());
    if allowed {
        Ok(())
    } else {
        Err(UpdateError::UnexpectedUrl)
    }
}

/// Sprawdza adres i wysyła GET z nagłówkami jak w 0.5.x.
fn request(
    client: &dyn HttpClient,
    url: &str,
    target: Target,
) -> Result<Result<HttpResponse, TransportError>, UpdateError> {
    check_url(url, target)?;
    let agent = format!("Bieznia/{}", crate::version());
    let headers = [
        ("Accept", "application/vnd.github+json"),
        ("User-Agent", agent.as_str()),
        ("X-GitHub-Api-Version", "2022-11-28"),
    ];
    Ok(client.get(url, &headers))
}

/// Wybiera z odpowiedzi API wydanie z instalatorem `WalkLevel-Setup-X.Y.Z.exe` (a gdy go nie
/// ma, `Bieznia-Setup-X.Y.Z.exe`), jego sumą i adresem podpisu `<nazwa>.sig`.
pub fn parse_release(data: &Value) -> Result<Release, UpdateError> {
    let version = py_get_str(data, "tag_name");
    if parse_version(&version) == (0, 0, 0) {
        return Err(UpdateError::InvalidVersion);
    }
    let assets: Vec<&Value> = data
        .get("assets")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter(|a| a.is_object())
        .collect();
    let asset_re = ASSET.as_ref().ok_or(UpdateError::MissingInstaller)?;
    let installer = |prefix: &str| {
        assets.iter().copied().find(|asset| {
            let name = py_get_str(asset, "name");
            asset_re
                .captures(&name)
                .and_then(|c| c.get(1))
                .is_some_and(|m| m.as_str() == prefix)
        })
    };
    let asset = installer("WalkLevel")
        .or_else(|| installer("Bieznia"))
        .ok_or(UpdateError::MissingInstaller)?;
    let name = py_get_str(asset, "name");
    let digest = py_str_or_empty(asset.get("digest"));
    let sha256 = digest
        .strip_prefix("sha256:")
        .filter(|_| digest.chars().count() == 71)
        .ok_or(UpdateError::MissingChecksum)?
        .to_lowercase();
    let url = py_get_str(asset, "browser_download_url");
    check_url(&url, Target::Asset)?;
    let signature_name = format!("{name}.sig");
    let signature_url = assets
        .iter()
        .find(|a| py_get_str(a, "name") == signature_name)
        .map(|a| py_get_str(a, "browser_download_url"));
    if let Some(address) = &signature_url {
        check_url(address, Target::Asset)?;
    }
    Ok(Release {
        version: version.trim_start_matches('v').to_owned(),
        notes: py_strip(&py_str_or_empty(data.get("body"))).to_owned(),
        url,
        sha256,
        size: py_size(asset.get("size")),
        name,
        signature_url,
    })
}

/// Pyta API GitHuba o najnowsze wydanie i zwraca je (bez porównania wersji).
pub fn fetch_latest(client: &dyn HttpClient) -> Result<Release, UpdateError> {
    let response = request(client, API_LATEST, Target::Api)?.map_err(|e| match e {
        TransportError::Status(404) => UpdateError::NoReleases,
        TransportError::Status(code) => UpdateError::HttpStatus(code),
        TransportError::Other(detail) => {
            log::warn!("Aktualizacje: brak odpowiedzi GitHuba: {detail}");
            UpdateError::Network
        }
    })?;
    check_url(&response.final_url, Target::Api)?; // like download(): a redirect must stay on GitHub
    let mut raw = Vec::new();
    response
        .body
        .take(MAX_API_BYTES)
        .read_to_end(&mut raw)
        .map_err(|e| {
            log::warn!("Aktualizacje: przerwany odczyt odpowiedzi: {e}");
            UpdateError::Network
        })?;
    // json.loads(bytes) w Pythonie pomija BOM UTF-8.
    let json = raw.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&raw);
    let data: Value = serde_json::from_slice(json).map_err(|e| {
        log::warn!("Aktualizacje: odpowiedź GitHuba to nie JSON: {e}");
        UpdateError::Network
    })?;
    if !data.is_object() {
        return Err(UpdateError::UnexpectedResponse);
    }
    parse_release(&data)
}

/// Kopiuje strumień do pliku, pilnując rozmiaru i sumy SHA-256.
fn copy_verified(
    source: &mut dyn Read,
    target: &mut dyn Write,
    release: &Release,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<(), UpdateError> {
    let io_err = |e: io::Error| {
        log::warn!("Aktualizacje: błąd pobierania: {e}");
        UpdateError::DownloadFailed
    };
    let limit = if release.size > 0 {
        MAX_SIZE.min(release.size.saturating_add(1024))
    } else {
        MAX_SIZE
    };
    let mut digest = Sha256::new();
    let mut done: u64 = 0;
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = match source.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(io_err(e)),
        };
        done += n as u64;
        if done > limit {
            return Err(UpdateError::TooLarge);
        }
        let chunk = buf.get(..n).ok_or(UpdateError::DownloadFailed)?;
        digest.update(chunk);
        target.write_all(chunk).map_err(io_err)?;
        progress(done, release.size);
    }
    target.flush().map_err(io_err)?;
    if hex::encode(digest.finalize()) != release.sha256 {
        return Err(UpdateError::ChecksumMismatch);
    }
    Ok(())
}

/// Minisign text as `minisign` writes it, or base64 of that text as `tauri signer` writes it.
fn minisign_text(raw: &str) -> Option<String> {
    let raw = raw.trim_start_matches('\u{feff}').trim();
    if raw.starts_with("untrusted comment:") {
        return Some(raw.to_owned());
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(raw).ok()?;
    String::from_utf8(bytes).ok()
}

/// The `.pub` file of `tauri signer` or `minisign`, or only its key line (`RW...`).
fn public_key(text: &str) -> Option<PublicKey> {
    PublicKey::from_base64(text.trim())
        .ok()
        .or_else(|| PublicKey::decode(&minisign_text(text)?).ok())
}

/// Checks that `file` is signed by `key` and that the signature names `version`
/// (`tauri signer sign --app-version`), so an older signed installer cannot pass for a new
/// version. Only prehashed signatures (the default of minisign and tauri) are accepted.
fn verify_signature(
    file: &Path,
    signature: &str,
    version: &str,
    key: &str,
) -> Result<(), UpdateError> {
    let Some(key) = public_key(key) else {
        log::warn!("Aktualizacje: brak klucza publicznego do sprawdzenia podpisu");
        return Err(UpdateError::BadSignature);
    };
    let signature = minisign_text(signature)
        .and_then(|text| Signature::decode(&text).ok())
        .ok_or(UpdateError::BadSignature)?;
    let expected = format!("version:{version}");
    if !signature
        .trusted_comment()
        .split('\t')
        .any(|part| part == expected)
    {
        log::warn!(
            "Aktualizacje: podpis nie dotyczy wersji {version}: {}",
            signature.trusted_comment()
        );
        return Err(UpdateError::BadSignature);
    }
    let rejected = |e: minisign_verify::Error| {
        log::warn!("Aktualizacje: podpis odrzucony: {e}");
        UpdateError::BadSignature
    };
    let mut verifier = key.verify_stream(&signature).map_err(rejected)?;
    let mut input = File::open(file).map_err(|e| {
        log::warn!("Aktualizacje: nie da się odczytać {file:?}: {e}");
        UpdateError::DownloadFailed
    })?;
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = match input.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                log::warn!("Aktualizacje: nie da się odczytać {file:?}: {e}");
                return Err(UpdateError::DownloadFailed);
            }
        };
        verifier.update(buf.get(..n).ok_or(UpdateError::DownloadFailed)?);
    }
    verifier.finalize().map_err(rejected)
}

/// Downloads the release's `.sig` (a few hundred bytes) before the installer.
fn fetch_signature(client: &dyn HttpClient, release: &Release) -> Result<String, UpdateError> {
    let url = release
        .signature_url
        .as_deref()
        .ok_or(UpdateError::MissingSignature)?;
    let response = request(client, url, Target::Asset)?.map_err(|e| match e {
        TransportError::Status(404) => UpdateError::MissingSignature,
        other => {
            log::warn!("Aktualizacje: pobieranie podpisu nieudane: {other}");
            UpdateError::DownloadFailed
        }
    })?;
    check_url(&response.final_url, Target::AssetRedirect)?;
    let mut raw = Vec::new();
    response
        .body
        .take(MAX_SIGNATURE_BYTES)
        .read_to_end(&mut raw)
        .map_err(|e| {
            log::warn!("Aktualizacje: przerwany odczyt podpisu: {e}");
            UpdateError::DownloadFailed
        })?;
    String::from_utf8(raw).map_err(|_| UpdateError::BadSignature)
}

/// Pobiera instalator do `directory` (domyślnie folder tymczasowy) i sprawdza SHA-256 oraz
/// podpis kluczem [`SIGNING_PUBLIC_KEY`].
///
/// `progress(pobrane, rozmiar)` dostaje bajty po każdym kawałku (`rozmiar` 0 = nieznany).
/// Przy błędzie nic nie zostaje na dysku. Zwraca ścieżkę gotowego instalatora.
pub fn download(
    release: &Release,
    directory: Option<&Path>,
    client: &dyn HttpClient,
    progress: impl FnMut(u64, u64),
) -> Result<PathBuf, UpdateError> {
    download_signed_by(release, directory, client, progress, SIGNING_PUBLIC_KEY)
}

fn download_signed_by(
    release: &Release,
    directory: Option<&Path>,
    client: &dyn HttpClient,
    mut progress: impl FnMut(u64, u64),
    key: &str,
) -> Result<PathBuf, UpdateError> {
    let directory = directory.map_or_else(std::env::temp_dir, Path::to_path_buf);
    fs::create_dir_all(&directory).map_err(|e| {
        log::warn!("Aktualizacje: nie da się utworzyć folderu {directory:?}: {e}");
        UpdateError::DownloadFailed
    })?;
    let target = directory.join(&release.name);
    let partial = target.with_extension("czesciowy");
    let result = (|| {
        let signature = fetch_signature(client, release)?;
        let mut response = request(client, &release.url, Target::Asset)?.map_err(|e| {
            log::warn!("Aktualizacje: pobieranie nieudane: {e}");
            UpdateError::DownloadFailed
        })?;
        // przekierowanie też musi prowadzić do GitHuba
        check_url(&response.final_url, Target::AssetRedirect)?;
        let mut out = File::create(&partial).map_err(|e| {
            log::warn!("Aktualizacje: nie da się zapisać {partial:?}: {e}");
            UpdateError::DownloadFailed
        })?;
        copy_verified(&mut response.body, &mut out, release, &mut progress)?;
        drop(out);
        verify_signature(&partial, &signature, &release.version, key)?;
        fs::rename(&partial, &target).map_err(|e| {
            log::warn!("Aktualizacje: nie da się zapisać {target:?}: {e}");
            UpdateError::DownloadFailed
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(&partial);
    }
    result.map(|()| target)
}

/// Uruchamia instalator w trybie cichym, odłączony od aplikacji; po instalacji sam uruchomi
/// nową wersję. Aplikacja powinna zamknąć się zaraz po udanym wywołaniu (0.5.x: po 400 ms).
pub fn run_installer(installer: &Path) -> Result<(), UpdateError> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use std::process::{Command, Stdio};
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        Command::new(installer)
            .args(INSTALLER_ARGS)
            .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(drop)
            .map_err(UpdateError::Launch)
    }
    #[cfg(not(windows))]
    {
        let _ = installer;
        Err(UpdateError::WindowsOnly)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::io::Cursor;

    const URL: &str = "https://github.com/damnit-damian/WalkLevel-by-damnit.com.pl/releases/download/v0.3.0/WalkLevel-Setup-0.3.0.exe";
    const SIG_URL: &str = "https://github.com/damnit-damian/WalkLevel-by-damnit.com.pl/releases/download/v0.3.0/WalkLevel-Setup-0.3.0.exe.sig";
    const OLD_URL: &str = "https://github.com/damnit-damian/WalkLevel-by-damnit.com.pl/releases/download/v0.3.0/Bieznia-Setup-0.3.0.exe";

    /// "MZ" + "instalator" x `repeat`: the installer of these tests.
    fn payload_of(repeat: usize) -> Vec<u8> {
        let mut data = b"MZ".to_vec();
        for _ in 0..repeat {
            data.extend_from_slice(b"instalator");
        }
        data
    }

    fn payload() -> Vec<u8> {
        payload_of(1000)
    }

    fn sha() -> String {
        hex::encode(Sha256::digest(payload()))
    }

    /// TEST ONLY keys and signatures (tests/fixtures/updates/signatures.json).
    fn signatures() -> Value {
        serde_json::from_str(include_str!("../tests/fixtures/updates/signatures.json")).unwrap()
    }

    fn test_signature(name: &str) -> String {
        signatures()[name].as_str().unwrap().to_owned()
    }

    fn test_key() -> String {
        test_signature("public_key")
    }

    fn installer_asset(name: &str, url: &str) -> Value {
        serde_json::json!({
            "name": name,
            "browser_download_url": url,
            "digest": format!("sha256:{}", sha()),
            "size": payload().len(),
        })
    }

    fn release_json(over: Value) -> Value {
        let mut asset = installer_asset("WalkLevel-Setup-0.3.0.exe", URL);
        if let (Some(a), Some(o)) = (asset.as_object_mut(), over.as_object()) {
            for (k, v) in o {
                a.insert(k.clone(), v.clone());
            }
        }
        serde_json::json!({
            "tag_name": "v0.3.0",
            "body": "Poprawki połączenia",
            "assets": [
                {"name": "WalkLevel-Setup-0.3.0.exe.sha256"},
                asset,
                {"name": "WalkLevel-Setup-0.3.0.exe.sig", "browser_download_url": SIG_URL},
            ],
        })
    }

    /// Download with the TEST ONLY key instead of the owner's.
    fn fetch(release: &Release, dir: &Path, client: &Fake) -> Result<PathBuf, UpdateError> {
        download_signed_by(release, Some(dir), client, |_, _| {}, &test_key())
    }

    type Reply = Box<dyn Fn(&str) -> Result<HttpResponse, TransportError>>;
    type Headers = Vec<(String, String)>;

    /// Klient testowy: zapamiętuje zapytania i zwraca przygotowaną odpowiedź.
    struct Fake {
        reply: Reply,
        seen: RefCell<Vec<(String, Headers)>>,
    }

    impl Fake {
        fn new(reply: impl Fn(&str) -> Result<HttpResponse, TransportError> + 'static) -> Self {
            Self {
                reply: Box::new(reply),
                seen: RefCell::new(Vec::new()),
            }
        }

        fn bytes(data: Vec<u8>, final_url: &'static str) -> Self {
            Self::new(move |_| {
                Ok(HttpResponse {
                    final_url: final_url.to_owned(),
                    body: Box::new(Cursor::new(data.clone())),
                })
            })
        }

        /// A release: `signature` for `*.sig` (404 when `None`), `data` from `final_url`
        /// for anything else.
        fn release(data: Vec<u8>, signature: Option<String>, final_url: &'static str) -> Self {
            Self::new(move |url| {
                if url.ends_with(".sig") {
                    let Some(text) = signature.clone() else {
                        return Err(TransportError::Status(404));
                    };
                    return Ok(HttpResponse {
                        final_url: url.to_owned(),
                        body: Box::new(Cursor::new(text.into_bytes())),
                    });
                }
                Ok(HttpResponse {
                    final_url: final_url.to_owned(),
                    body: Box::new(Cursor::new(data.clone())),
                })
            })
        }

        /// A correctly signed release of [`payload`].
        fn signed(data: Vec<u8>, final_url: &'static str) -> Self {
            Self::release(data, Some(test_signature("payload_1000")), final_url)
        }
    }

    impl HttpClient for Fake {
        fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<HttpResponse, TransportError> {
            let headers = headers
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect();
            self.seen.borrow_mut().push((url.to_owned(), headers));
            (self.reply)(url)
        }
    }

    fn entries(dir: &Path) -> usize {
        fs::read_dir(dir).map(|d| d.count()).unwrap_or(usize::MAX)
    }

    #[test]
    fn versions() {
        assert_eq!(parse_version("v1.2.10"), (1, 2, 10));
        assert!(is_newer("0.3.0", "0.2.9"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(is_newer("0.0.1", "dev")); // kod uruchomiony z repo jest zawsze najstarszy
        assert!(!is_newer("0.6.0", crate::version()));
    }

    #[test]
    fn release_is_parsed() {
        let r = parse_release(&release_json(Value::Null)).unwrap();
        assert_eq!(
            (r.version.as_str(), r.sha256.clone(), r.name.as_str()),
            ("0.3.0", sha(), "WalkLevel-Setup-0.3.0.exe")
        );
        assert_eq!(r.signature_url.as_deref(), Some(SIG_URL));
    }

    #[test]
    fn new_installer_name_wins_over_old_copy() {
        let data = serde_json::json!({
            "tag_name": "v0.3.0",
            "assets": [
                installer_asset("Bieznia-Setup-0.3.0.exe", OLD_URL),
                installer_asset("WalkLevel-Setup-0.3.0.exe", URL),
            ],
        });
        let r = parse_release(&data).unwrap();
        assert_eq!(
            (r.name.as_str(), r.url.as_str()),
            ("WalkLevel-Setup-0.3.0.exe", URL)
        );
        assert_eq!(r.signature_url, None);
    }

    #[test]
    fn old_installer_name_is_the_fallback() {
        let data = serde_json::json!({
            "tag_name": "v0.3.0",
            "assets": [
                installer_asset("Bieznia-Setup-0.3.0.exe", OLD_URL),
                {"name": "Bieznia-Setup-0.3.0.exe.sig", "browser_download_url": format!("{OLD_URL}.sig")},
            ],
        });
        let r = parse_release(&data).unwrap();
        assert_eq!(r.name, "Bieznia-Setup-0.3.0.exe");
        assert_eq!(r.signature_url, Some(format!("{OLD_URL}.sig")));
    }

    #[test]
    fn suspicious_releases_are_rejected() {
        let cases = [
            (serde_json::json!({"digest": null}), "nie da się sprawdzić"),
            (
                serde_json::json!({"browser_download_url": "https://zly-serwer.example/WalkLevel-Setup-0.3.0.exe"}),
                "adres",
            ),
            (
                serde_json::json!({"browser_download_url": URL.replace("https", "http")}),
                "adres",
            ),
        ];
        for (over, message) in cases {
            let err = parse_release(&release_json(over)).unwrap_err();
            assert!(err.to_string().contains(message), "{err}");
        }
    }

    #[test]
    fn signature_elsewhere_is_rejected() {
        let mut data = release_json(Value::Null);
        data["assets"][2]["browser_download_url"] =
            "https://github.com/someone-else/WalkLevel-by-damnit.com.pl/releases/download/v0.3.0/WalkLevel-Setup-0.3.0.exe.sig".into();
        assert!(matches!(
            parse_release(&data),
            Err(UpdateError::UnexpectedUrl)
        ));
    }

    #[test]
    fn downloads_only_from_this_repository() {
        let ok = [
            URL,
            SIG_URL,
            OLD_URL,
            "https://github.com/Damnit-Damian/walklevel-by-damnit.com.pl/releases/download/v0.3.0/WalkLevel-Setup-0.3.0.exe",
        ];
        for url in ok {
            assert!(check_url(url, Target::Asset).is_ok(), "{url}");
        }
        let repo = "https://github.com/damnit-damian/WalkLevel-by-damnit.com.pl";
        let refused = [
            // the old account name, which anyone may now register
            "https://github.com/Damian\u{65}jroo/WalkLevel-by-damnit.com.pl/releases/download/v0.3.0/WalkLevel-Setup-0.3.0.exe".to_owned(),
            "https://github.com/someone/WalkLevel-by-damnit.com.pl/releases/download/v0.3.0/x.exe".to_owned(),
            "https://github.com/damnit-damian/other-repo/releases/download/v0.3.0/x.exe".to_owned(),
            format!("{repo}-fork/releases/download/v0.3.0/x.exe"),
            format!("{repo}/raw/main/x.exe"),
            format!("{repo}/releases/download/x.exe"),
            format!("{repo}/releases/download/v0.3.0/"),
            format!("{repo}/releases/download/v0.3.0/a/b.exe"),
            format!("{repo}/releases/download/../../../evil/releases/download/v1/x.exe"),
            format!("{repo}/releases/download/v0.3.0/..%2F..%2Fx.exe"),
            format!("{repo}/releases/download/v0.3.0/%2e%2e"),
            "https://release-assets.githubusercontent.com/github-production-release-asset/1/x"
                .to_owned(),
            "https://api.github.com/repos/damnit-damian/WalkLevel-by-damnit.com.pl/releases/latest"
                .to_owned(),
        ];
        for url in &refused {
            assert!(check_url(url, Target::Asset).is_err(), "{url}");
        }
    }

    #[test]
    fn redirect_may_go_to_github_cdn_only() {
        for url in [
            URL,
            "https://release-assets.githubusercontent.com/github-production-release-asset/1/x?sp=r",
            "https://objects.githubusercontent.com/github-production-release-asset-2e65be/1/x",
        ] {
            assert!(check_url(url, Target::AssetRedirect).is_ok(), "{url}");
        }
        for url in [
            "https://raw.githubusercontent.com/damnit-damian/WalkLevel-by-damnit.com.pl/main/x.exe",
            "https://github.com/someone/WalkLevel-by-damnit.com.pl/releases/download/v1/x.exe",
            "https://evil.example/x.exe",
            "http://release-assets.githubusercontent.com/x",
        ] {
            assert!(check_url(url, Target::AssetRedirect).is_err(), "{url}");
        }
        assert!(check_url(URL, Target::Api).is_err());
        assert!(check_url(API_LATEST, Target::Api).is_ok());
    }

    #[test]
    fn release_without_installer() {
        let data = serde_json::json!({"tag_name": "v1.0.0", "assets": [{"name": "inne.zip"}]});
        let err = parse_release(&data).unwrap_err();
        assert!(err.to_string().contains("instalatora"));
    }

    #[test]
    fn suffixed_current_version_is_not_offered_an_older_release() {
        assert!(!is_newer("0.6.3", "0.7.0-rc.1"));
        assert!(!is_newer("0.6.3", "v0.7.0-rc.1"));
        assert!(is_newer("0.7.0", "0.7.0-rc.1"));
        assert!(!is_newer("0.7.0", "0.7.0+build.5"));
        assert!(is_newer("0.7.1", "0.7.0+build.5"));
    }

    #[test]
    fn fetch_latest_rejects_redirect_off_github() {
        let body = serde_json::to_vec(&release_json(Value::Null)).unwrap();
        let fake = Fake::bytes(body, "https://example.com/releases/latest");
        assert!(matches!(
            fetch_latest(&fake),
            Err(UpdateError::UnexpectedUrl)
        ));
    }

    #[test]
    fn fetch_latest_uses_api() {
        let body = serde_json::to_vec(&release_json(Value::Null)).unwrap();
        let fake = Fake::bytes(body, API_LATEST);
        assert_eq!(fetch_latest(&fake).unwrap().version, "0.3.0");
        let seen = fake.seen.borrow();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, API_LATEST);
        let agent = format!("Bieznia/{}", crate::version());
        assert!(seen[0].1.contains(&("User-Agent".to_owned(), agent)));
        assert!(seen[0].1.contains(&(
            "Accept".to_owned(),
            "application/vnd.github+json".to_owned()
        )));
        assert!(
            seen[0]
                .1
                .contains(&("X-GitHub-Api-Version".to_owned(), "2022-11-28".to_owned()))
        );
    }

    #[test]
    fn network_error_is_friendly() {
        let fake = Fake::new(|_| Err(TransportError::Other("brak sieci".into())));
        let err = fetch_latest(&fake).unwrap_err();
        assert!(err.to_string().contains("internet"));
    }

    #[test]
    fn no_releases_yet_is_explained() {
        let fake = Fake::new(|_| Err(TransportError::Status(404)));
        let err = fetch_latest(&fake).unwrap_err();
        assert_eq!(err.to_string(), "Brak wersji do pobrania.");
    }

    #[test]
    fn other_http_error_shows_code() {
        let fake = Fake::new(|_| Err(TransportError::Status(503)));
        let err = fetch_latest(&fake).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Serwer aktualizacji nie odpowiada poprawnie. Spróbuj później. (kod 503)"
        );
    }

    #[test]
    fn bad_json_and_non_object() {
        let err = fetch_latest(&Fake::bytes(b"<html>".to_vec(), API_LATEST)).unwrap_err();
        assert!(matches!(err, UpdateError::Network));
        let err = fetch_latest(&Fake::bytes(b"[1, 2]".to_vec(), API_LATEST)).unwrap_err();
        assert!(matches!(err, UpdateError::UnexpectedResponse));
    }

    #[test]
    fn download_verifies_checksum_and_signature() {
        let dir = tempfile::tempdir().unwrap();
        let release = parse_release(&release_json(Value::Null)).unwrap();
        let fake = Fake::signed(payload(), URL);
        let mut calls = Vec::new();
        let path = download_signed_by(
            &release,
            Some(dir.path()),
            &fake,
            |d, t| calls.push((d, t)),
            &test_key(),
        )
        .unwrap();
        assert_eq!(fs::read(&path).unwrap(), payload());
        assert_eq!(path, dir.path().join("WalkLevel-Setup-0.3.0.exe"));
        assert_eq!(entries(dir.path()), 1);
        let total = payload().len() as u64;
        assert_eq!(calls.last(), Some(&(total, total)));
        let seen: Vec<String> = fake.seen.borrow().iter().map(|s| s.0.clone()).collect();
        assert_eq!(seen, [SIG_URL, URL]);
    }

    #[test]
    fn download_replaces_old_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("WalkLevel-Setup-0.3.0.exe"), b"stary").unwrap();
        let release = parse_release(&release_json(Value::Null)).unwrap();
        let path = fetch(&release, dir.path(), &Fake::signed(payload(), URL)).unwrap();
        assert_eq!(fs::read(path).unwrap(), payload());
    }

    #[test]
    fn bad_checksum_leaves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let release = parse_release(&release_json(Value::Null)).unwrap();
        let mut data = payload();
        data.push(b'x');
        let err = fetch(&release, dir.path(), &Fake::signed(data, URL)).unwrap_err();
        assert!(err.to_string().contains("Pobrany plik się nie zgadza"));
        assert_eq!(entries(dir.path()), 0);
    }

    /// Signature cases: each one leaves nothing on disk and names the reason.
    fn rejected_signature(signature: Option<String>, key: &str) -> UpdateError {
        let dir = tempfile::tempdir().unwrap();
        let release = parse_release(&release_json(Value::Null)).unwrap();
        let fake = Fake::release(payload(), signature, URL);
        let err =
            download_signed_by(&release, Some(dir.path()), &fake, |_, _| {}, key).unwrap_err();
        assert_eq!(entries(dir.path()), 0);
        err
    }

    #[test]
    fn missing_signature_is_not_installed() {
        let err = rejected_signature(None, &test_key());
        assert!(matches!(err, UpdateError::MissingSignature));
        assert_eq!(
            err.to_string(),
            "Nowa wersja nie ma podpisu autora, więc nie zostanie zainstalowana. Spróbuj później."
        );
        let mut data = release_json(Value::Null);
        data["assets"].as_array_mut().unwrap().pop();
        let release = parse_release(&data).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let fake = Fake::signed(payload(), URL);
        let err = fetch(&release, dir.path(), &fake).unwrap_err();
        assert!(matches!(err, UpdateError::MissingSignature));
        assert!(fake.seen.borrow().is_empty(), "nothing is downloaded");
    }

    #[test]
    fn bad_signature_is_not_installed() {
        let good = test_signature("payload_1000");
        let text = minisign_text(&good).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        let mut flipped: Vec<char> = lines[1].chars().collect();
        flipped[20] = if flipped[20] == 'A' { 'B' } else { 'A' };
        let damaged = format!(
            "{}\n{}\n{}\n{}",
            lines[0],
            flipped.iter().collect::<String>(),
            lines[2],
            lines[3]
        );
        for signature in [damaged, "to nie jest podpis".to_owned(), String::new()] {
            let err = rejected_signature(Some(signature), &test_key());
            assert!(matches!(err, UpdateError::BadSignature), "{err:?}");
        }
        assert_eq!(
            rejected_signature(Some("x".into()), &test_key()).to_string(),
            "Podpis nowej wersji się nie zgadza, więc nie zostanie zainstalowana. Zgłoś to w Ustawienia > Pomoc i diagnostyka."
        );
    }

    #[test]
    fn signature_of_another_file_is_not_installed() {
        let err = rejected_signature(Some(test_signature("other_file")), &test_key());
        assert!(matches!(err, UpdateError::BadSignature));
    }

    #[test]
    fn signature_of_another_version_is_not_installed() {
        // a genuine signature of the same bytes, made for 0.2.0: no replay as 0.3.0
        let err = rejected_signature(Some(test_signature("other_version")), &test_key());
        assert!(matches!(err, UpdateError::BadSignature));
    }

    #[test]
    fn signature_by_another_key_is_not_installed() {
        let err = rejected_signature(Some(test_signature("other_key")), &test_key());
        assert!(matches!(err, UpdateError::BadSignature));
        let other = test_signature("other_public_key");
        let err = rejected_signature(Some(test_signature("payload_1000")), &other);
        assert!(matches!(err, UpdateError::BadSignature));
    }

    #[test]
    fn built_in_key_is_placeholder_or_valid() {
        assert!(
            SIGNING_PUBLIC_KEY.starts_with("PLACEHOLDER")
                || public_key(SIGNING_PUBLIC_KEY).is_some(),
            "SIGNING_PUBLIC_KEY is not a minisign public key"
        );
    }

    #[test]
    fn placeholder_key_accepts_nothing() {
        let err = rejected_signature(Some(test_signature("payload_1000")), "PLACEHOLDER: x");
        assert!(matches!(err, UpdateError::BadSignature));
    }

    #[test]
    fn signature_formats_of_minisign_and_tauri() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("instalator.exe");
        fs::write(&file, payload()).unwrap();
        let tauri_sig = test_signature("payload_1000");
        let minisign_sig = minisign_text(&tauri_sig).unwrap();
        let tauri_key = test_key();
        let minisign_key = minisign_text(&tauri_key).unwrap();
        let bare_key = minisign_key.lines().nth(1).unwrap().to_owned();
        for key in [&tauri_key, &minisign_key, &bare_key] {
            for sig in [&tauri_sig, &minisign_sig] {
                verify_signature(&file, sig, "0.3.0", key).unwrap();
            }
        }
        assert!(verify_signature(&file, &tauri_sig, "0.3.1", &tauri_key).is_err());
    }

    /// Run by the release workflow on the freshly signed installer, with the key built into
    /// the app: `WALKLEVEL_SIGNED_INSTALLER=<exe> WALKLEVEL_SIGNED_VERSION=<X.Y.Z>`.
    #[test]
    #[ignore = "release workflow only"]
    fn release_installer_is_signed_with_built_in_key() {
        let installer = std::env::var("WALKLEVEL_SIGNED_INSTALLER").unwrap();
        let version = std::env::var("WALKLEVEL_SIGNED_VERSION").unwrap();
        let signature = fs::read_to_string(format!("{installer}.sig")).unwrap();
        verify_signature(
            Path::new(&installer),
            &signature,
            &version,
            SIGNING_PUBLIC_KEY,
        )
        .expect("the signature does not match SIGNING_PUBLIC_KEY");
    }

    #[test]
    fn redirect_outside_github_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let release = parse_release(&release_json(Value::Null)).unwrap();
        let fake = Fake::signed(payload(), "https://evil.example/x.exe");
        let err = fetch(&release, dir.path(), &fake).unwrap_err();
        assert!(err.to_string().contains("adres"));
        assert_eq!(entries(dir.path()), 0);
    }

    #[test]
    fn redirect_to_github_cdn_is_followed() {
        let dir = tempfile::tempdir().unwrap();
        let release = parse_release(&release_json(Value::Null)).unwrap();
        let cdn =
            "https://release-assets.githubusercontent.com/github-production-release-asset/1/x?sp=r";
        let path = fetch(&release, dir.path(), &Fake::signed(payload(), cdn)).unwrap();
        assert_eq!(fs::read(path).unwrap(), payload());
    }

    #[test]
    fn oversized_download_is_stopped() {
        let dir = tempfile::tempdir().unwrap();
        let release = parse_release(&release_json(serde_json::json!({"size": 10}))).unwrap();
        let err = fetch(&release, dir.path(), &Fake::signed(payload(), URL)).unwrap_err();
        assert!(err.to_string().contains("Pobrany plik się nie zgadza"));
        assert_eq!(entries(dir.path()), 0);
    }

    #[test]
    fn transport_error_during_download_leaves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let release = parse_release(&release_json(Value::Null)).unwrap();
        let fake = Fake::new(|_| Err(TransportError::Status(500)));
        let err = fetch(&release, dir.path(), &fake).unwrap_err();
        assert!(matches!(err, UpdateError::DownloadFailed));
        assert_eq!(entries(dir.path()), 0);
    }

    #[test]
    fn broken_stream_leaves_nothing() {
        struct Broken(usize);
        impl Read for Broken {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                if self.0 == 0 {
                    return Err(io::Error::other("zerwane połączenie"));
                }
                self.0 -= 1;
                buf[0] = b'M';
                Ok(1)
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let release = parse_release(&release_json(Value::Null)).unwrap();
        let signature = test_signature("payload_1000");
        let fake = Fake::new(move |url| {
            let body: Box<dyn Read + Send> = if url.ends_with(".sig") {
                Box::new(Cursor::new(signature.clone().into_bytes()))
            } else {
                Box::new(Broken(5))
            };
            Ok(HttpResponse {
                final_url: url.to_owned(),
                body,
            })
        });
        let err = fetch(&release, dir.path(), &fake).unwrap_err();
        assert!(matches!(err, UpdateError::DownloadFailed));
        assert_eq!(entries(dir.path()), 0);
    }

    #[test]
    fn installer_args_match_python() {
        assert_eq!(
            INSTALLER_ARGS,
            [
                "/VERYSILENT",
                "/SUPPRESSMSGBOXES",
                "/NORESTART",
                "/CLOSEAPPLICATIONS"
            ]
        );
        assert_eq!(
            API_LATEST,
            format!("https://api.github.com/repos/{REPOSITORY}/releases/latest")
        );
    }

    #[cfg(windows)]
    #[test]
    fn missing_installer_reports_launch_error() {
        let dir = tempfile::tempdir().unwrap();
        let err = run_installer(&dir.path().join("brak.exe")).unwrap_err();
        assert!(matches!(err, UpdateError::Launch(_)));
        assert_eq!(
            err.to_string(),
            "Nie udało się uruchomić instalatora. Pobierz go ze strony autora."
        );
    }

    #[test]
    fn reqwest_client_builds() {
        assert!(ReqwestClient::new().is_ok());
    }

    // --- zgodność z Pythonem (fixtury z tools/fixtures/updates.py) ---

    #[test]
    fn versions_match_python() {
        let data: Value =
            serde_json::from_str(include_str!("../tests/fixtures/updates/versions.json")).unwrap();
        for case in data["parse"].as_array().unwrap() {
            let text = case["text"].as_str().unwrap();
            let v: Vec<u64> = serde_json::from_value(case["version"].clone()).unwrap();
            assert_eq!(parse_version(text), (v[0], v[1], v[2]), "{text:?}");
        }
        for case in data["newer"].as_array().unwrap() {
            let (a, b) = (
                case["candidate"].as_str().unwrap(),
                case["current"].as_str().unwrap(),
            );
            assert_eq!(
                is_newer(a, b),
                case["newer"].as_bool().unwrap(),
                "{a} vs {b}"
            );
        }
        assert_eq!(data["api_latest"], API_LATEST);
    }

    #[test]
    fn releases_match_python() {
        let data: Value =
            serde_json::from_str(include_str!("../tests/fixtures/updates/releases.json")).unwrap();
        for case in data.as_array().unwrap() {
            let name = case["case"].as_str().unwrap();
            match parse_release(&case["input"]) {
                Ok(r) => {
                    let expected = &case["release"];
                    assert!(expected.is_object(), "{name}: Python zwrócił błąd");
                    assert_eq!(serde_json::to_value(&r).unwrap(), *expected, "{name}");
                }
                Err(e) => assert_eq!(old_error(case), e.to_string(), "{name}"),
            }
        }
    }

    /// Błąd z fixtury 0.5.2 w brzmieniu 0.6.0 (docs/texts.md, aktualizacja.*).
    fn old_error(case: &Value) -> String {
        crate::texts::from_052(
            case["error"].as_str().unwrap_or_default(),
            &["aktualizacja."],
        )
    }

    /// Odpowiedź klienta testowego wg opisu przypadku z fixtury (status / brak sieci / treść);
    /// `*.sig` dostaje `signature` (0.5.x nie miało podpisów).
    fn fake_for(case: &Value, body: Option<Vec<u8>>, signature: Option<String>) -> Fake {
        let status = case["status"].as_u64().map(|s| s as u16);
        let final_url = case["final_url"].as_str().map(str::to_owned);
        Fake::new(move |url| {
            if let Some(code) = status {
                return Err(TransportError::Status(code));
            }
            let Some(mut data) = body.clone() else {
                return Err(TransportError::Other("brak sieci".into()));
            };
            if let (true, Some(text)) = (url.ends_with(".sig"), &signature) {
                data = text.clone().into_bytes();
            }
            Ok(HttpResponse {
                final_url: final_url.clone().unwrap_or_else(|| url.to_owned()),
                body: Box::new(Cursor::new(data)),
            })
        })
    }

    #[test]
    fn fetch_matches_python() {
        let data: Value =
            serde_json::from_str(include_str!("../tests/fixtures/updates/fetch.json")).unwrap();
        for case in data.as_array().unwrap() {
            let name = case["case"].as_str().unwrap();
            let body = case["body"].as_str().map(|text| {
                let pad = case["pad"].as_u64().unwrap_or(0) as usize;
                let split = text.len() - text.chars().last().map_or(0, char::len_utf8);
                format!("{}{}{}", &text[..split], " ".repeat(pad), &text[split..]).into_bytes()
            });
            match fetch_latest(&fake_for(case, body, None)) {
                Ok(r) => assert_eq!(case["version"], r.version, "{name}"),
                Err(e) => assert_eq!(old_error(case), e.to_string(), "{name}"),
            }
        }
    }

    #[test]
    fn download_matches_python() {
        let data: Value =
            serde_json::from_str(include_str!("../tests/fixtures/updates/download.json")).unwrap();
        let repeat = data["payload_repeat"].as_u64().unwrap();
        assert_eq!(repeat, 60000, "signatures.json signs payload_60000");
        let payload = payload_of(repeat as usize);
        let signature = test_signature("payload_60000");
        for case in data["cases"].as_array().unwrap() {
            let name = case["case"].as_str().unwrap();
            let release = Release {
                version: "0.3.0".into(),
                notes: String::new(),
                url: case["url"].as_str().unwrap_or(OLD_URL).to_owned(),
                sha256: hex::encode(Sha256::digest(&payload)),
                size: case["size"].as_u64().unwrap_or(payload.len() as u64),
                name: "Bieznia-Setup-0.3.0.exe".into(),
                signature_url: Some(format!("{OLD_URL}.sig")),
            };
            let mut body = payload.clone();
            body.extend_from_slice(case["extra"].as_str().unwrap_or("").as_bytes());
            let body = (!case["network"].as_bool().unwrap_or(false)).then_some(body);
            let dir = tempfile::tempdir().unwrap();
            let mut calls: Vec<Vec<u64>> = Vec::new();
            let fake = fake_for(case, body, Some(signature.clone()));
            let result = download_signed_by(
                &release,
                Some(dir.path()),
                &fake,
                |d, t| calls.push(vec![d, t]),
                &test_key(),
            );
            match result {
                Ok(path) => {
                    assert_eq!(case["file"], path.file_name().unwrap().to_str().unwrap());
                    assert_eq!(fs::read(&path).unwrap(), payload, "{name}");
                }
                Err(e) => assert_eq!(old_error(case), e.to_string(), "{name}"),
            }
            let mut files: Vec<String> = fs::read_dir(dir.path())
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            files.sort();
            assert_eq!(
                serde_json::to_value(files).unwrap(),
                case["files"],
                "{name}"
            );
            assert_eq!(
                serde_json::to_value(calls).unwrap(),
                case["progress"],
                "{name}"
            );
        }
    }

    // --- świadome różnice względem 0.5.x (Python zachowuje się tu gorzej) ---

    #[test]
    fn url_must_point_where_reqwest_connects() {
        // urlparse widzi tu host github.com, a reqwest połączyłby się z evil.example.
        let path = "/damnit-damian/WalkLevel-by-damnit.com.pl/releases/download/v1/x.exe";
        for target in [Target::Asset, Target::AssetRedirect] {
            assert!(
                check_url(&format!("https://evil.example\\@github.com{path}"), target).is_err()
            );
            assert!(
                check_url(&format!("https://github.com\\@evil.example{path}"), target).is_err()
            );
            assert!(check_url(&format!("https://git%68ub.com{path}"), target).is_err());
        }
        assert!(
            check_url(
                "https://release-assets.githubusercontent.com/a?b=c",
                Target::AssetRedirect
            )
            .is_ok()
        );
    }

    #[test]
    fn asset_name_with_trailing_newline_is_skipped() {
        // W Pythonie `$` dopuszcza końcowe "\n" i nazwa pliku z "\n" psuła pobieranie.
        let data = release_json(serde_json::json!({"name": "WalkLevel-Setup-0.3.0.exe\n"}));
        assert!(matches!(
            parse_release(&data),
            Err(UpdateError::MissingInstaller)
        ));
    }

    #[test]
    fn installer_url_must_be_this_repository_since_0_6_4() {
        // 0.5.x-0.6.3 przyjmowały każdy adres na hostach GitHuba (fixtura drugi_host).
        let cdn = "https://objects.githubusercontent.com/x/WalkLevel-Setup-0.3.0.exe";
        let data = release_json(serde_json::json!({"browser_download_url": cdn}));
        assert!(matches!(
            parse_release(&data),
            Err(UpdateError::UnexpectedUrl)
        ));
    }
}
