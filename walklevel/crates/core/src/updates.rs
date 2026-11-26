//! Aktualizacja na żądanie z GitHub Releases (port `bieznia/aktualizacje.py`).
//!
//! 1. Pytamy API GitHuba o najnowsze wydanie TEGO repozytorium ([`fetch_latest`]).
//! 2. Jeśli jest nowsza wersja ([`is_newer`]), pobieramy jej instalator wyłącznie z hostów
//!    GitHuba ([`download`]).
//! 3. Sprawdzamy SHA-256 z danych wydania - inna suma = plik odrzucony i usunięty.
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

use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::py::{strip as py_strip, truthy};

/// Repozytorium z wydaniami (to samo co w wersji 0.5.x).
pub const REPOSITORY: &str = "Damianejroo/WalkLevel-by-damnit.com.pl";
/// Adres API z najnowszym wydaniem.
pub const API_LATEST: &str = "https://api.github.com/repos/Damianejroo/WalkLevel-by-damnit.com.pl/releases/latest";
/// Jedyne hosty, z których wolno pobierać (także po przekierowaniu).
pub const ALLOWED_HOSTS: [&str; 4] = [
    "api.github.com",
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];
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

static ASSET: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^Bieznia-Setup-(\d+\.\d+\.\d+)\.exe$").ok());
static VERSION: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^v?(\d+)\.(\d+)\.(\d+)$").ok());

/// Błąd aktualizacji. `Display` to gotowy komunikat dla użytkownika (po polsku).
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("{}", crate::teksty::t("aktualizacja.powod.adres", &[]))]
    UnexpectedUrl,
    #[error("{}", crate::teksty::t("aktualizacja.powod.numer", &[]))]
    InvalidVersion,
    #[error("{}", crate::teksty::t("aktualizacja.powod.suma_brak", &[]))]
    MissingChecksum,
    #[error("{}", crate::teksty::t("aktualizacja.powod.brak_instalatora", &[]))]
    MissingInstaller,
    #[error("{}", crate::teksty::t("aktualizacja.powod.brak_wydan", &[]))]
    NoReleases,
    #[error("{}", crate::teksty::t("aktualizacja.powod.kod", &[("kod", &.0.to_string())]))]
    HttpStatus(u16),
    #[error("{}", crate::teksty::t("aktualizacja.powod.internet", &[]))]
    Network,
    #[error("{}", crate::teksty::t("aktualizacja.powod.odpowiedz", &[]))]
    UnexpectedResponse,
    #[error("{}", crate::teksty::t("aktualizacja.powod.za_duzy", &[]))]
    TooLarge,
    #[error("{}", crate::teksty::t("aktualizacja.powod.suma", &[]))]
    ChecksumMismatch,
    #[error("{}", crate::teksty::t("aktualizacja.powod.przerwane", &[]))]
    DownloadFailed,
    #[error("{}", crate::teksty::t("aktualizacja.powod.system", &[]))]
    WindowsOnly,
    #[error("{}", crate::teksty::t("aktualizacja.instalator_blad", &[]))]
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
    /// Nazwa pliku, np. "Bieznia-Setup-0.6.1.exe".
    pub name: String,
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

/// Czy `candidate` jest nowsza niż `current` (np. `crate::version()`).
pub fn is_newer(candidate: &str, current: &str) -> bool {
    parse_version(candidate) > parse_version(current)
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

/// Tylko https i host GitHuba - zarówno według reguł 0.5.x (`urlparse`), jak i według parsera,
/// którym reqwest naprawdę łączy (np. `\` czy `%68` w adresie nie przemyci innego hosta).
fn check_url(url: &str) -> Result<(), UpdateError> {
    let Some((scheme, host)) = py_scheme_host(url) else {
        return Err(UpdateError::UnexpectedUrl);
    };
    let parsed = reqwest::Url::parse(url).map_err(|_| UpdateError::UnexpectedUrl)?;
    let allowed = scheme == "https"
        && ALLOWED_HOSTS.contains(&host.as_str())
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
) -> Result<Result<HttpResponse, TransportError>, UpdateError> {
    check_url(url)?;
    let agent = format!("Bieznia/{}", crate::version());
    let headers = [
        ("Accept", "application/vnd.github+json"),
        ("User-Agent", agent.as_str()),
        ("X-GitHub-Api-Version", "2022-11-28"),
    ];
    Ok(client.get(url, &headers))
}

/// Wybiera z odpowiedzi API wydanie z instalatorem `Bieznia-Setup-X.Y.Z.exe` i jego sumą.
pub fn parse_release(data: &Value) -> Result<Release, UpdateError> {
    let version = py_get_str(data, "tag_name");
    if parse_version(&version) == (0, 0, 0) {
        return Err(UpdateError::InvalidVersion);
    }
    let assets = data
        .get("assets")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let asset_re = ASSET.as_ref().ok_or(UpdateError::MissingInstaller)?;
    for asset in assets.iter().filter(|a| a.is_object()) {
        let name = py_get_str(asset, "name");
        if !asset_re.is_match(&name) {
            continue;
        }
        let digest = py_str_or_empty(asset.get("digest"));
        let sha256 = digest
            .strip_prefix("sha256:")
            .filter(|_| digest.chars().count() == 71)
            .ok_or(UpdateError::MissingChecksum)?
            .to_lowercase();
        let url = py_get_str(asset, "browser_download_url");
        check_url(&url)?;
        return Ok(Release {
            version: version.trim_start_matches('v').to_owned(),
            notes: py_strip(&py_str_or_empty(data.get("body"))).to_owned(),
            url,
            sha256,
            size: py_size(asset.get("size")),
            name,
        });
    }
    Err(UpdateError::MissingInstaller)
}

/// Pyta API GitHuba o najnowsze wydanie i zwraca je (bez porównania wersji).
pub fn fetch_latest(client: &dyn HttpClient) -> Result<Release, UpdateError> {
    let response = request(client, API_LATEST)?.map_err(|e| match e {
        TransportError::Status(404) => UpdateError::NoReleases,
        TransportError::Status(code) => UpdateError::HttpStatus(code),
        TransportError::Other(detail) => {
            log::warn!("Aktualizacje: brak odpowiedzi GitHuba: {detail}");
            UpdateError::Network
        }
    })?;
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

/// Pobiera instalator do `directory` (domyślnie folder tymczasowy) i sprawdza SHA-256.
///
/// `progress(pobrane, rozmiar)` dostaje bajty po każdym kawałku (`rozmiar` 0 = nieznany).
/// Przy błędzie nic nie zostaje na dysku. Zwraca ścieżkę gotowego instalatora.
pub fn download(
    release: &Release,
    directory: Option<&Path>,
    client: &dyn HttpClient,
    mut progress: impl FnMut(u64, u64),
) -> Result<PathBuf, UpdateError> {
    let directory = directory.map_or_else(std::env::temp_dir, Path::to_path_buf);
    fs::create_dir_all(&directory).map_err(|e| {
        log::warn!("Aktualizacje: nie da się utworzyć folderu {directory:?}: {e}");
        UpdateError::DownloadFailed
    })?;
    let target = directory.join(&release.name);
    let partial = target.with_extension("czesciowy");
    let result = (|| {
        let mut response = request(client, &release.url)?.map_err(|e| {
            log::warn!("Aktualizacje: pobieranie nieudane: {e}");
            UpdateError::DownloadFailed
        })?;
        check_url(&response.final_url)?; // przekierowanie też musi prowadzić do GitHuba
        let mut out = File::create(&partial).map_err(|e| {
            log::warn!("Aktualizacje: nie da się zapisać {partial:?}: {e}");
            UpdateError::DownloadFailed
        })?;
        copy_verified(&mut response.body, &mut out, release, &mut progress)?;
        drop(out);
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

    const URL: &str = "https://github.com/Damianejroo/WalkLevel-by-damnit.com.pl/releases/download/v0.3.0/Bieznia-Setup-0.3.0.exe";

    fn payload() -> Vec<u8> {
        let mut data = b"MZ".to_vec();
        for _ in 0..1000 {
            data.extend_from_slice(b"instalator");
        }
        data
    }

    fn sha() -> String {
        hex::encode(Sha256::digest(payload()))
    }

    fn release_json(over: Value) -> Value {
        let mut asset = serde_json::json!({
            "name": "Bieznia-Setup-0.3.0.exe",
            "browser_download_url": URL,
            "digest": format!("sha256:{}", sha()),
            "size": payload().len(),
        });
        if let (Some(a), Some(o)) = (asset.as_object_mut(), over.as_object()) {
            for (k, v) in o {
                a.insert(k.clone(), v.clone());
            }
        }
        serde_json::json!({
            "tag_name": "v0.3.0",
            "body": "Poprawki połączenia",
            "assets": [{"name": "Bieznia-Setup-0.3.0.exe.sha256"}, asset],
        })
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
            ("0.3.0", sha(), "Bieznia-Setup-0.3.0.exe")
        );
    }

    #[test]
    fn suspicious_releases_are_rejected() {
        let cases = [
            (serde_json::json!({"digest": null}), "nie da się sprawdzić"),
            (
                serde_json::json!({"browser_download_url": "https://zly-serwer.example/Bieznia-Setup-0.3.0.exe"}),
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
    fn release_without_installer() {
        let data = serde_json::json!({"tag_name": "v1.0.0", "assets": [{"name": "inne.zip"}]});
        let err = parse_release(&data).unwrap_err();
        assert!(err.to_string().contains("instalatora"));
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
    fn download_verifies_checksum() {
        let dir = tempfile::tempdir().unwrap();
        let release = parse_release(&release_json(Value::Null)).unwrap();
        let fake = Fake::bytes(payload(), URL);
        let mut calls = Vec::new();
        let path = download(&release, Some(dir.path()), &fake, |d, t| calls.push((d, t))).unwrap();
        assert_eq!(fs::read(&path).unwrap(), payload());
        assert_eq!(path, dir.path().join("Bieznia-Setup-0.3.0.exe"));
        assert_eq!(entries(dir.path()), 1);
        let total = payload().len() as u64;
        assert_eq!(calls.last(), Some(&(total, total)));
        assert_eq!(fake.seen.borrow()[0].0, URL);
    }

    #[test]
    fn download_replaces_old_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Bieznia-Setup-0.3.0.exe"), b"stary").unwrap();
        let release = parse_release(&release_json(Value::Null)).unwrap();
        let path = download(
            &release,
            Some(dir.path()),
            &Fake::bytes(payload(), URL),
            |_, _| {},
        )
        .unwrap();
        assert_eq!(fs::read(path).unwrap(), payload());
    }

    #[test]
    fn bad_checksum_leaves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let release = parse_release(&release_json(Value::Null)).unwrap();
        let mut data = payload();
        data.push(b'x');
        let err = download(
            &release,
            Some(dir.path()),
            &Fake::bytes(data, URL),
            |_, _| {},
        )
        .unwrap_err();
        assert!(err.to_string().contains("Pobrany plik się nie zgadza"));
        assert_eq!(entries(dir.path()), 0);
    }

    #[test]
    fn redirect_outside_github_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let release = parse_release(&release_json(Value::Null)).unwrap();
        let fake = Fake::bytes(payload(), "https://evil.example/x.exe");
        let err = download(&release, Some(dir.path()), &fake, |_, _| {}).unwrap_err();
        assert!(err.to_string().contains("adres"));
        assert_eq!(entries(dir.path()), 0);
    }

    #[test]
    fn oversized_download_is_stopped() {
        let dir = tempfile::tempdir().unwrap();
        let release = parse_release(&release_json(serde_json::json!({"size": 10}))).unwrap();
        let err = download(
            &release,
            Some(dir.path()),
            &Fake::bytes(payload(), URL),
            |_, _| {},
        )
        .unwrap_err();
        assert!(err.to_string().contains("Pobrany plik się nie zgadza"));
        assert_eq!(entries(dir.path()), 0);
    }

    #[test]
    fn transport_error_during_download_leaves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let release = parse_release(&release_json(Value::Null)).unwrap();
        let fake = Fake::new(|_| Err(TransportError::Status(500)));
        let err = download(&release, Some(dir.path()), &fake, |_, _| {}).unwrap_err();
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
        let fake = Fake::new(|_| {
            Ok(HttpResponse {
                final_url: URL.to_owned(),
                body: Box::new(Broken(5)),
            })
        });
        let err = download(&release, Some(dir.path()), &fake, |_, _| {}).unwrap_err();
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

    // --- zgodność z Pythonem (fixtury z walklevel/tools/fixtures/updates.py) ---

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

    /// Błąd z fixtury 0.5.2 w brzmieniu 0.6.0 (docs/teksty.md, aktualizacja.*).
    fn old_error(case: &Value) -> String {
        crate::teksty::from_052(
            case["error"].as_str().unwrap_or_default(),
            &["aktualizacja."],
        )
    }

    /// Odpowiedź klienta testowego wg opisu przypadku z fixtury (status / brak sieci / treść).
    fn fake_for(case: &Value, body: Option<Vec<u8>>) -> Fake {
        let status = case["status"].as_u64().map(|s| s as u16);
        let final_url = case["final_url"].as_str().map(str::to_owned);
        Fake::new(move |url| {
            if let Some(code) = status {
                return Err(TransportError::Status(code));
            }
            let Some(data) = body.clone() else {
                return Err(TransportError::Other("brak sieci".into()));
            };
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
            match fetch_latest(&fake_for(case, body)) {
                Ok(r) => assert_eq!(case["version"], r.version, "{name}"),
                Err(e) => assert_eq!(old_error(case), e.to_string(), "{name}"),
            }
        }
    }

    #[test]
    fn download_matches_python() {
        let data: Value =
            serde_json::from_str(include_str!("../tests/fixtures/updates/download.json")).unwrap();
        let mut payload = b"MZ".to_vec();
        for _ in 0..data["payload_repeat"].as_u64().unwrap() {
            payload.extend_from_slice(b"instalator");
        }
        for case in data["cases"].as_array().unwrap() {
            let name = case["case"].as_str().unwrap();
            let release = Release {
                version: "0.3.0".into(),
                notes: String::new(),
                url: case["url"].as_str().unwrap_or(URL).to_owned(),
                sha256: hex::encode(Sha256::digest(&payload)),
                size: case["size"].as_u64().unwrap_or(payload.len() as u64),
                name: "Bieznia-Setup-0.3.0.exe".into(),
            };
            let mut body = payload.clone();
            body.extend_from_slice(case["extra"].as_str().unwrap_or("").as_bytes());
            let body = (!case["network"].as_bool().unwrap_or(false)).then_some(body);
            let dir = tempfile::tempdir().unwrap();
            let mut calls: Vec<Vec<u64>> = Vec::new();
            let result = download(&release, Some(dir.path()), &fake_for(case, body), |d, t| {
                calls.push(vec![d, t])
            });
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
        assert!(check_url("https://evil.example\\@github.com/x").is_err());
        assert!(check_url("https://github.com\\@evil.example/x").is_err());
        assert!(check_url("https://git%68ub.com/x").is_err());
        assert!(check_url("https://release-assets.githubusercontent.com/a?b=c").is_ok());
    }

    #[test]
    fn asset_name_with_trailing_newline_is_skipped() {
        // W Pythonie `$` dopuszcza końcowe "\n" i nazwa pliku z "\n" psuła pobieranie.
        let data = release_json(serde_json::json!({"name": "Bieznia-Setup-0.3.0.exe\n"}));
        assert!(matches!(
            parse_release(&data),
            Err(UpdateError::MissingInstaller)
        ));
    }
}
