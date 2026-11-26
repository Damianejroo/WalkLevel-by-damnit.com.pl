//! Testy zgłoszeń: porównanie z fixturami z Pythona (tools/fixtures/zgloszenia.py)
//! i testy przeniesione z tests/test_zgloszenia.py.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, UNIX_EPOCH};

use base64::Engine;
use serde_json::Value;

use super::diagnostics::{DiagnosticReport, ScannedDevice};
use super::*;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/zgloszenia/zgloszenia.json"
);

/// Komunikat z fixtury 0.5.2 w brzmieniu 0.6.0 (docs/teksty.md, zgloszenie.*).
fn old(message: &str) -> String {
    crate::teksty::from_052(message, &["zgloszenie."])
}

fn fixture() -> Value {
    let text = fs::read_to_string(FIXTURE).expect("fixtura zgłoszeń");
    serde_json::from_str(&text).expect("JSON fixtury")
}

fn b64(value: &Value) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(value.as_str().expect("tekst base64"))
        .expect("base64")
}

fn s(value: &Value) -> &str {
    value.as_str().expect("tekst")
}

fn ts(value: &Value) -> f64 {
    local_timestamp(s(value)).expect("data lokalna")
}

fn set_mtime(path: &Path, at: f64) {
    let file = File::options().write(true).open(path).expect("plik");
    file.set_modified(UNIX_EPOCH + Duration::from_secs_f64(at))
        .expect("czas modyfikacji");
}

/// Pliki z fixtury: nazwa (względna), treść base64, wiek w sekundach względem `now`.
fn write_files(root: &Path, files: &Value, now: f64) {
    for f in files.as_array().expect("lista plików") {
        let path = root.join(s(&f["name"]));
        fs::create_dir_all(path.parent().expect("folder")).expect("folder");
        fs::write(&path, b64(&f["data"])).expect("zapis");
        set_mtime(&path, now - f["ago_s"].as_f64().expect("wiek"));
    }
}

fn zip_entries(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut archive = zip::ZipArchive::new(File::open(path).expect("paczka")).expect("zip");
    let mut out = BTreeMap::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).expect("wpis");
        let mut data = Vec::new();
        entry.read_to_end(&mut data).expect("odczyt");
        out.insert(entry.name().expect("nazwa").to_string(), data);
    }
    out
}

fn info(description: &str, contact: &str, period_s: f64) -> ReportInfo {
    ReportInfo {
        description: description.to_string(),
        contact: contact.to_string(),
        version: "0.6.0".to_string(),
        system: "Windows-test".to_string(),
        period_s,
    }
}

// --- zgodność z Pythonem (fixtury) -----------------------------------------------------

#[test]
fn constants_match_python() {
    let fx = fixture();
    let c = &fx["constants"];
    assert_eq!(c["max_log_bytes"].as_u64(), Some(MAX_LOG_BYTES));
    assert_eq!(c["recent_s"].as_f64(), Some(RECENT_S));
    assert_eq!(c["max_report_bytes"].as_u64(), Some(MAX_REPORT_BYTES));
    let limits: Vec<u64> = c["tail_limits"]
        .as_array()
        .expect("limity")
        .iter()
        .filter_map(Value::as_u64)
        .collect();
    assert_eq!(limits, TAIL_LIMITS);
    let periods: Vec<(u64, String)> = c["periods"]
        .as_array()
        .expect("okresy")
        .iter()
        .map(|p| (p[0].as_u64().expect("s"), s(&p[1]).to_string()))
        .collect();
    let ours: Vec<(u64, String)> = PERIODS.iter().map(|(a, b)| (*a, b.to_string())).collect();
    assert_eq!(periods, ours);
}

#[test]
fn scrub_matches_python() {
    let fx = fixture();
    for case in fx["scrub"].as_array().expect("przypadki") {
        let out = scrub(&b64(&case["input"]), s(&case["keep"]), s(&case["home"]));
        assert_eq!(
            String::from_utf8_lossy(&out),
            String::from_utf8_lossy(&b64(&case["output"])),
            "wejście: {}",
            String::from_utf8_lossy(&b64(&case["input"]))
        );
    }
}

#[test]
fn tail_since_matches_python() {
    let fx = fixture();
    let now = ts(&fx["now"]);
    for (i, case) in fx["tail_since"]
        .as_array()
        .expect("przypadki")
        .iter()
        .enumerate()
    {
        let dir = tempfile::tempdir().expect("tmp");
        write_files(dir.path(), &Value::Array(vec![case["file"].clone()]), now);
        let path = dir.path().join(s(&case["file"]["name"]));
        let limit = case["limit"].as_u64().expect("limit");
        let out = tail_since(&path, ts(&case["since"]), limit).expect("tail");
        assert_eq!(out, b64(&case["output"]), "przypadek {i}");
    }
}

#[test]
fn summary_matches_python() {
    let fx = fixture();
    let now = ts(&fx["now"]);
    for (i, case) in fx["summary"]
        .as_array()
        .expect("przypadki")
        .iter()
        .enumerate()
    {
        let dir = tempfile::tempdir().expect("tmp");
        write_files(dir.path(), &case["files"], now);
        let paths: Vec<PathBuf> = case["files"]
            .as_array()
            .expect("pliki")
            .iter()
            .map(|f| dir.path().join(s(&f["name"])))
            .collect();
        let out = summary_text(&paths, ts(&case["since"])).expect("podsumowanie");
        assert_eq!(out, s(&case["output"]), "przypadek {i}");
    }
}

#[test]
fn recent_logs_match_python() {
    let fx = fixture();
    let now = ts(&fx["now"]);
    let set = &fx["recent_logs"][0];
    let dir = tempfile::tempdir().expect("tmp");
    write_files(dir.path(), &set["files"], now);
    for case in set["cases"].as_array().expect("przypadki") {
        let period = case["period_s"].as_f64().expect("okres");
        let found: Vec<String> = recent_logs(dir.path(), now, period)
            .expect("logi")
            .iter()
            .map(|p| file_name(p))
            .collect();
        let expected: Vec<&str> = case["output"]
            .as_array()
            .expect("nazwy")
            .iter()
            .map(s)
            .collect();
        assert_eq!(found, expected, "okres {period}");
    }
}

#[test]
fn reports_match_python() {
    let fx = fixture();
    let now = ts(&fx["now"]);
    let home = s(&fx["home"]);
    for (i, case) in fx["reports"].as_array().expect("paczki").iter().enumerate() {
        let dir = tempfile::tempdir().expect("tmp");
        write_files(dir.path(), &case["files"], now);
        let report = info(
            s(&case["description"]),
            s(&case["contact"]),
            case["period_s"].as_f64().expect("okres"),
        );
        let folders = ReportFolders::in_root(dir.path());
        let path = build_report_at(&folders, &report, now, MAX_REPORT_BYTES, home).expect("paczka");
        assert!(path.starts_with(&folders.out));
        let mut entries = zip_entries(&path);
        let json: Value = serde_json::from_slice(&entries.remove("zgloszenie.json").expect("json"))
            .expect("json");
        let mut expected = BTreeMap::new();
        for (name, data) in case["entries"].as_object().expect("wpisy") {
            if name != "zgloszenie.json" {
                expected.insert(name.clone(), b64(data));
            }
        }
        assert_eq!(
            entries.keys().collect::<Vec<_>>(),
            expected.keys().collect::<Vec<_>>(),
            "paczka {i}"
        );
        for (name, data) in &expected {
            assert_eq!(
                String::from_utf8_lossy(&entries[name]),
                String::from_utf8_lossy(data),
                "paczka {i}, plik {name}"
            );
        }
        for key in ["opis", "kontakt", "wersja", "okres_logow_h"] {
            assert_eq!(json[key], case["info"][key], "paczka {i}, pole {key}");
        }
        let keys: Vec<&String> = json.as_object().expect("obiekt").keys().collect();
        let python_keys: Vec<&str> = case["info_keys"]
            .as_array()
            .expect("klucze")
            .iter()
            .map(s)
            .filter(|k| *k != "python")
            .collect();
        let mut sorted = python_keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted, "paczka {i}");
        // zapis bajt w bajt jak json.dumps(ensure_ascii=False, indent=2) - bez pola "python",
        // z czasem i systemem z tego uruchomienia
        let raw = String::from_utf8(zip_entries(&path)["zgloszenie.json"].clone()).expect("utf8");
        let python_raw = String::from_utf8(b64(&case["entries"]["zgloszenie.json"])).expect("utf8");
        let python_json: Value = serde_json::from_str(&python_raw).expect("json");
        let expected_raw = python_raw
            .split('\n')
            .filter(|line| !line.starts_with("  \"python\": "))
            .collect::<Vec<_>>()
            .join("\n")
            .replace(s(&python_json["czas"]), s(&json["czas"]))
            .replace(
                &format!("\"windows\": \"{}\"", s(&python_json["windows"])),
                "\"windows\": \"Windows-test\"",
            );
        assert_eq!(raw, expected_raw, "paczka {i}");
    }
}

#[test]
fn py_float_matches_python_repr() {
    let fx = fixture();
    for case in fx["floats"].as_array().expect("liczby") {
        let value = case["value"].as_f64().expect("liczba");
        assert_eq!(py_float(value), s(&case["repr"]), "{value:e}");
    }
    assert_eq!(py_float(f64::NAN), "nan");
    assert_eq!(py_float(f64::NEG_INFINITY), "-inf");
}

#[test]
fn device_address_matches_python() {
    let fx = fixture();
    for case in fx["device_address"].as_array().expect("ustawienia") {
        let dir = tempfile::tempdir().expect("tmp");
        let path = dir.path().join("ustawienia.json");
        fs::write(&path, b64(&case["settings"])).expect("zapis");
        assert_eq!(
            device_address(&path),
            s(&case["output"]),
            "{}",
            String::from_utf8_lossy(&b64(&case["settings"]))
        );
    }
    let dir = tempfile::tempdir().expect("tmp");
    assert_eq!(device_address(&dir.path().join("brak.json")), "");
}

#[test]
fn https_check_matches_python_urlparse() {
    let fx = fixture();
    for case in fx["https"].as_array().expect("adresy") {
        let endpoint = s(&case["endpoint"]);
        assert_eq!(
            is_https(endpoint),
            case["https"].as_bool().expect("tak/nie"),
            "{endpoint:?}"
        );
    }
}

#[test]
fn multipart_and_fields_match_python() {
    let fx = fixture();
    let send = &fx["send"];
    let boundary = s(&send["boundary"]);
    let report = b64(&send["report"]);
    let name = s(&send["report_name"]);
    let fields = vec![
        ("wersja", "0.6.0".to_string()),
        ("opis", "błąd \"cudzysłów\"\r\nlinia".to_string()),
        ("kontakt", String::new()),
    ];
    let (body, content_type) = multipart_body(&fields, name, &report, boundary);
    assert_eq!(body, b64(&send["multipart"]["body"]));
    assert_eq!(content_type, s(&send["multipart"]["content_type"]));
    for case in send["fields"].as_array().expect("pola") {
        let fields = form_fields(
            s(&case["description"]),
            s(&case["contact"]),
            case["period_s"].as_f64(),
            "0.6.0",
        );
        let (body, _) = multipart_body(&fields, name, &report, boundary);
        assert_eq!(
            String::from_utf8_lossy(&body),
            String::from_utf8_lossy(&b64(&case["body"])),
            "okres {:?}",
            case["period_s"]
        );
        assert_eq!(s(&case["user_agent"]), "Bieznia/0.6.0");
    }
}

#[test]
fn server_replies_match_python() {
    let fx = fixture();
    let send = &fx["send"];
    for case in send["replies"].as_array().expect("odpowiedzi") {
        assert_eq!(json_field(&b64(&case["body"]), "id"), s(&case["id"]));
    }
    for case in send["errors"].as_array().expect("błędy") {
        let status = u16::try_from(case["status"].as_u64().expect("kod")).expect("kod");
        assert_eq!(
            http_error(status, &b64(&case["body"])).to_string(),
            old(s(&case["message"])),
            "kod {status}"
        );
    }
    assert_eq!(network_error().to_string(), old(s(&send["network"])));
    let dir = tempfile::tempdir().expect("tmp");
    let report = dir.path().join("a.zip");
    fs::write(&report, b"PK").expect("zapis");
    let err = send_report(&report, "http://example.com/z", "x", "", None, "0.6.0")
        .expect_err("tylko https");
    assert_eq!(err.to_string(), old(s(&send["not_https"])));
}

#[test]
fn mailto_matches_python() {
    // 0.6.0: temat i treść w nowym brzmieniu (zgloszenie.email.*), kodowanie jak w Pythonie
    let fx = fixture();
    for case in fx["mailto"].as_array().expect("linki") {
        let version = fx["version"].as_str().expect("wersja");
        let (description, report) = (s(&case["description"]), s(&case["report"]));
        let subject = crate::teksty::t("zgloszenie.email.temat", &[("wersja", version)]);
        let body = crate::teksty::t(
            "zgloszenie.email.tresc",
            &[("opis", description.trim()), ("plik", report)],
        ) + "\n";
        let link = mailto_link(description, report, version);
        assert!(s(&case["output"]).starts_with(&format!("mailto:{CONTACT}?subject=")));
        assert_eq!(
            link,
            format!(
                "mailto:{CONTACT}?subject={}&body={}",
                percent_encode(&subject),
                percent_encode(&body)
            )
        );
    }
}

/// Usuwa czas na początku linii ("   0.03s ") - zależy od szybkości komputera.
fn without_stamp(line: &str) -> String {
    let trimmed = line.trim_start();
    if let Some((num, rest)) = trimmed.split_once("s ")
        && num.contains('.')
        && num.chars().all(|c| c.is_ascii_digit() || c == '.')
    {
        return rest.to_string();
    }
    line.to_string()
}

#[test]
fn diagnostics_text_matches_python() {
    let fx = fixture();
    for sc in fx["diagnostics"].as_array().expect("scenariusze") {
        let mut r = DiagnosticReport::new("0.6.0", "10.0", "btleplug");
        r.scan_started();
        if sc["boom"].as_bool() == Some(true) {
            r.error("RuntimeError", "boom");
        } else {
            let devices: Vec<ScannedDevice> = sc["devices"]
                .as_array()
                .expect("urządzenia")
                .iter()
                .map(|d| {
                    let local = d["local_name"].as_str().filter(|n| !n.is_empty());
                    let name = local.or(d["name"].as_str()).map(str::to_string);
                    let uuids: Vec<String> = d["uuids"]
                        .as_array()
                        .expect("usługi")
                        .iter()
                        .map(|u| s(u).to_string())
                        .collect();
                    let treadmill = name.as_deref().is_some_and(|n| n.starts_with("KS-"))
                        || uuids.iter().any(|u| u.starts_with("00001826"));
                    ScannedDevice {
                        address: s(&d["address"]).to_string(),
                        rssi: i16::try_from(d["rssi"].as_i64().expect("rssi")).expect("rssi"),
                        name,
                        service_uuids: uuids,
                        manufacturer_data: d["mfr"]
                            .as_array()
                            .expect("producent")
                            .iter()
                            .map(|m| {
                                (
                                    s(&m[0]).parse().expect("firma"),
                                    hex::decode(s(&m[1])).expect("hex"),
                                )
                            })
                            .collect(),
                        treadmill,
                    }
                })
                .collect();
            r.devices(&devices);
            let address = sc["address"].as_str().map(str::to_string).or_else(|| {
                devices
                    .iter()
                    .find(|d| d.treadmill)
                    .map(|d| d.address.clone())
            });
            r.blank();
            match address {
                None => r.no_treadmill(),
                Some(address) => {
                    r.connecting(&address);
                    if sc["gone"].as_bool() == Some(true) {
                        r.device_gone();
                    } else {
                        let services = sc["services"].as_array().expect("usługi");
                        let chars = |svc: &Value| -> Vec<(String, Vec<String>, String)> {
                            svc["chars"]
                                .as_array()
                                .expect("charakterystyki")
                                .iter()
                                .map(|c| {
                                    (
                                        s(&c[0]).to_string(),
                                        c[1].as_array()
                                            .expect("właściwości")
                                            .iter()
                                            .map(|p| s(p).to_string())
                                            .collect(),
                                        s(&c[2]).to_string(),
                                    )
                                })
                                .collect()
                        };
                        for svc in services {
                            r.service(s(&svc["uuid"]), s(&svc["description"]));
                            for (uuid, props, desc) in chars(svc) {
                                let props: Vec<&str> = props.iter().map(String::as_str).collect();
                                r.characteristic(&uuid, &props, &desc);
                            }
                        }
                        r.reads_started();
                        for svc in services {
                            for (uuid, props, _) in chars(svc) {
                                if !props.iter().any(|p| p == "read") {
                                    continue;
                                }
                                match sc["reads"][&uuid].as_str() {
                                    Some(value) => {
                                        r.read_value(&uuid, &hex::decode(value).expect("hex"))
                                    }
                                    None => r.read_failed(&uuid, "TimeoutError"),
                                }
                            }
                        }
                        r.notifications_started(diagnostics::LISTEN_S);
                        for svc in services {
                            for (uuid, props, _) in chars(svc) {
                                if !props.iter().any(|p| p == "notify" || p == "indicate") {
                                    continue;
                                }
                                match sc["notify"][&uuid].as_str() {
                                    Some(data) => {
                                        r.notification(&uuid, &hex::decode(data).expect("hex"))
                                    }
                                    None => r.subscribe_failed(&uuid, "odmowa"),
                                }
                            }
                        }
                    }
                }
            }
        }
        let text = r.text();
        let ours: Vec<String> = text.split('\n').skip(2).map(without_stamp).collect();
        let python: Vec<String> = sc["lines"]
            .as_array()
            .expect("linie")
            .iter()
            .map(|l| without_stamp(s(l)))
            .collect();
        assert_eq!(ours, python, "scenariusz {}", s(&sc["name"]));
        if cfg!(windows) {
            let dir = tempfile::tempdir().expect("tmp");
            let path = r.save(&dir.path().join("d.txt")).expect("zapis");
            let raw = fs::read(&path).expect("odczyt");
            let crlf = raw.windows(2).filter(|w| w == b"\r\n").count();
            let lf = raw.iter().filter(|&&b| b == b'\n').count();
            assert_eq!(
                crlf == lf,
                sc["crlf"].as_bool().expect("crlf"),
                "końce linii"
            );
        }
    }
}

#[test]
fn diagnostics_header_kingsmith_and_save() {
    let mut r = DiagnosticReport::new("0.6.0", "10.0.26300", "btleplug 0.13.5");
    r.kingsmith_channels(&["fe00", "ffc0"]);
    r.kingsmith_text("speed 2.5");
    r.reading(3.0, Some(120));
    r.reading(2.5, None);
    r.result("KingSmith X21");
    let text = r.text();
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[0].starts_with("Raport WalkLevel 0.6.0 - 20"));
    assert_eq!(lines[1], "Windows 10.0.26300 | btleplug 0.13.5");
    assert_eq!(lines[2], "");
    assert_eq!(lines[3], "Protokół KingSmith: kanały fe00, ffc0");
    assert_eq!(without_stamp(lines[4]), "tekst: speed 2.5");
    assert!(lines[4].starts_with("   0.0"));
    assert_eq!(
        without_stamp(lines[5]),
        "ODCZYT prędkość=3.0 km/h kroki=120"
    );
    assert_eq!(
        without_stamp(lines[6]),
        "ODCZYT prędkość=2.5 km/h kroki=None"
    );
    assert_eq!(lines[7], "Wynik: KingSmith X21");
    let dir = tempfile::tempdir().expect("tmp");
    let path = diagnostics::report_path(&dir.path().join("logs"));
    assert!(file_name(&path).starts_with("diagnostyka-"));
    r.save(&path).expect("zapis");
    let saved = fs::read_to_string(&path).expect("odczyt");
    assert_eq!(saved.replace("\r\n", "\n"), text);
    assert_eq!(saved.contains("\r\n"), cfg!(windows));
}

// --- testy przeniesione z tests/test_zgloszenia.py ---------------------------------------

fn make_data(root: &Path) {
    let logs = root.join("logs");
    fs::create_dir(&logs).expect("logs");
    fs::write(
        logs.join("bieznia.log"),
        format!("start\n{}\nkoniec\n", "x".repeat(50)),
    )
    .expect("log");
    let mut big = vec![b'A'; MAX_LOG_BYTES as usize + 500];
    big.extend_from_slice(b"OSTATNIE");
    fs::write(logs.join("bluetooth.log"), big).expect("log");
    fs::write(logs.join("diagnostyka-20260101-100000.txt"), "stary").expect("raport");
    fs::write(logs.join("diagnostyka-20260102-100000.txt"), "nowy").expect("raport");
    fs::create_dir(root.join("dane")).expect("dane");
    fs::write(
        root.join("dane").join("ustawienia.json"),
        r#"{"run_on": 6.3}"#,
    )
    .expect("ustawienia");
    fs::write(root.join("prywatny.txt"), "nie wysyłać").expect("prywatny");
}

#[test]
fn report_contains_only_logs_and_settings() {
    let dir = tempfile::tempdir().expect("tmp");
    make_data(dir.path());
    let folders = ReportFolders::in_root(dir.path());
    let path =
        build_report(&folders, &info("Bieżnia się rozłącza", "nick#1", RECENT_S)).expect("paczka");
    let entries = zip_entries(&path);
    assert_eq!(
        entries.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "dane/ustawienia.json",
            "logs/bieznia.log",
            "logs/bluetooth.log",
            "logs/diagnostyka-20260102-100000.txt", // tylko najnowszy raport
            "podsumowanie.txt",
            "zgloszenie.json",
        ]
    );
    let json: Value = serde_json::from_slice(&entries["zgloszenie.json"]).expect("json");
    assert_eq!(json["opis"], "Bieżnia się rozłącza");
    assert_eq!(json["kontakt"], "nick#1");
    let tail = &entries["logs/bluetooth.log"];
    assert_eq!(tail.len() as u64, MAX_LOG_BYTES);
    assert!(tail.ends_with(b"OSTATNIE"));
}

#[test]
fn report_works_without_any_logs() {
    let dir = tempfile::tempdir().expect("tmp");
    let path = build_report(
        &ReportFolders::in_root(dir.path()),
        &info("pusto", "", RECENT_S),
    )
    .expect("paczka");
    assert_eq!(
        zip_entries(&path).keys().collect::<Vec<_>>(),
        ["zgloszenie.json"]
    );
}

#[test]
fn only_https_endpoint() {
    let dir = tempfile::tempdir().expect("tmp");
    let path = build_report(
        &ReportFolders::in_root(dir.path()),
        &info("x", "", RECENT_S),
    )
    .expect("paczka");
    let err = send_report(
        &path,
        "http://example.com/zgloszenia",
        "x",
        "",
        None,
        "0.6.0",
    )
    .expect_err("http");
    assert!(err.to_string().contains("serwera zgłoszeń"));
    assert!(is_https("HTTPS://example.com"));
    assert!(!is_https("ftp://example.com"));
}

#[test]
fn multipart_body_layout() {
    let (body, content_type) =
        multipart_body(&[("opis", "błąd".to_string())], "a.zip", b"PK123", "abc");
    let boundary = content_type.split("boundary=").nth(1).expect("granica");
    assert!(body.starts_with(format!("--{boundary}\r\n").as_bytes()));
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("name=\"opis\"\r\n\r\nbłąd\r\n"));
    assert!(text.contains("filename=\"a.zip\"") && text.contains("PK123"));
    assert!(body.ends_with(format!("--{boundary}--\r\n").as_bytes()));
}

#[test]
fn mailto_link_is_encoded() {
    let link = mailto_link("Nie działa & stop", "zgloszenie-1.zip", "0.6.0");
    assert!(link.starts_with("mailto:kontakt@damnit.com.pl?subject="));
    assert!(!link.split("body=").nth(1).expect("treść").contains('&'));
    assert!(link.contains("zgloszenie-1.zip"));
}

#[test]
fn report_adds_recent_rotated_logs_and_summary() {
    let dir = tempfile::tempdir().expect("tmp");
    let logs = dir.path().join("logs");
    fs::create_dir(&logs).expect("logs");
    fs::write(
        logs.join("bieznia.log"),
        "10:00:01 INFO    bieznia.ble: Start: przyjęty po 1.6 s (próba 1)\n\
         10:00:02 INFO    bieznia: Okno gotowe po 0.4 s\n\
         10:05:00 WARNING bieznia.ble: Taśma nie przyjęła 2.5 km/h (jedzie 1.0) - ponawiam (1)\n\
         10:30:00 INFO    bieznia.ble: Podsumowanie sesji: 30 min\n",
    )
    .expect("log");
    fs::write(logs.join("bluetooth.log"), "nowy\n").expect("log");
    fs::write(logs.join("bluetooth.log.1"), "sprzed godziny\n").expect("log");
    let stale = logs.join("bluetooth.log.2");
    fs::write(&stale, "sprzed tygodnia\n").expect("log");
    set_mtime(&stale, now_s() - 7.0 * 86400.0);
    let path = build_report(
        &ReportFolders::in_root(dir.path()),
        &info("test", "", RECENT_S),
    )
    .expect("paczka");
    let entries = zip_entries(&path);
    let summary = String::from_utf8(entries["podsumowanie.txt"].clone()).expect("utf8");
    assert!(entries.contains_key("logs/bluetooth.log.1"));
    assert!(!entries.contains_key("logs/bluetooth.log.2")); // starszy niż 6 godzin
    assert!(summary.contains("Start: przyjęty") && summary.contains("Podsumowanie sesji"));
    assert!(!summary.contains("Okno gotowe")); // tylko ważne zdarzenia
    assert!(summary.contains("polecenia przyjęte: 1") && summary.contains("bez reakcji: 1"));
}

#[test]
fn report_hides_profile_path_and_other_bluetooth_devices() {
    let home = home_dir();
    let dir = tempfile::tempdir().expect("tmp");
    let logs = dir.path().join("logs");
    fs::create_dir(&logs).expect("logs");
    fs::write(
        logs.join("bieznia.log"),
        format!("plik {home}/AppData/x.txt\nmoja AA:BB:CC:DD:EE:FF obca 11-22-33-44-55-66\n"),
    )
    .expect("log");
    fs::create_dir(dir.path().join("dane")).expect("dane");
    fs::write(
        dir.path().join("dane").join("ustawienia.json"),
        r#"{"device_address": "aa:bb:cc:dd:ee:ff"}"#,
    )
    .expect("ustawienia");
    let path = build_report(
        &ReportFolders::in_root(dir.path()),
        &info("x", "", RECENT_S),
    )
    .expect("paczka");
    let text = String::from_utf8(zip_entries(&path)["logs/bieznia.log"].clone()).expect("utf8");
    if home.chars().count() > 3 {
        assert!(!text.contains(&home) && text.contains("%USERPROFILE%/AppData"));
    }
    assert!(text.contains("AA:BB:CC:DD:EE:FF")); // wybrana bieżnia zostaje
    assert!(!text.contains("11-22-33-44-55-66") && text.contains("XX:XX:XX:XX:XX:XX"));
}

/// Bajty, które się nie kompresują (xorshift - bez dodatkowych bibliotek).
fn noise(len: usize) -> Vec<u8> {
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 32) as u8
        })
        .collect()
}

#[test]
fn too_big_report_uses_shorter_log_tails() {
    let dir = tempfile::tempdir().expect("tmp");
    let logs = dir.path().join("logs");
    fs::create_dir(&logs).expect("logs");
    fs::write(logs.join("bieznia.log"), noise(MAX_LOG_BYTES as usize)).expect("log");
    let folders = ReportFolders::in_root(dir.path());
    let report = info("x", "", RECENT_S);
    let path = build_report_at(&folders, &report, now_s(), 500_000, &home_dir()).expect("paczka");
    assert!(fs::metadata(&path).expect("rozmiar").len() <= 500_000);
    fs::remove_file(&path).expect("usunięcie");
    let err =
        build_report_at(&folders, &report, now_s(), 50_000, &home_dir()).expect_err("za duża");
    assert!(err.to_string().contains("za duża"));
    assert!(fs::read_dir(&folders.out).expect("folder").next().is_none()); // usunięta
}

#[test]
fn report_takes_only_the_chosen_period() {
    let dir = tempfile::tempdir().expect("tmp");
    let logs = dir.path().join("logs");
    fs::create_dir(&logs).expect("logs");
    let now = now_s();
    let stamp = |ago: f64| {
        chrono::DateTime::from_timestamp((now - ago) as i64, 0)
            .expect("czas")
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
    };
    fs::write(
        logs.join("bieznia.log"),
        format!(
            "{} INFO  stare\n{} INFO  piec godzin temu\n{} ERROR swieze\nTraceback: dalsza linia\n",
            stamp(3.0 * 86400.0),
            stamp(5.0 * 3600.0),
            stamp(1800.0)
        ),
    )
    .expect("log");
    let folders = ReportFolders::in_root(dir.path());
    let one_hour = build_report(&folders, &info("x", "", 3600.0)).expect("paczka");
    let entries = zip_entries(&one_hour);
    let text = String::from_utf8(entries["logs/bieznia.log"].clone()).expect("utf8");
    let json: Value = serde_json::from_slice(&entries["zgloszenie.json"]).expect("json");
    assert!(text.contains("swieze") && text.contains("Traceback"));
    assert!(!text.contains("piec godzin") && !text.contains("stare"));
    assert_eq!(json["okres_logow_h"].as_f64(), Some(1.0));
    fs::remove_file(&one_hour).expect("usunięcie");
    let week = build_report(&folders, &info("x", "", 7.0 * 86400.0)).expect("paczka");
    let text = String::from_utf8(zip_entries(&week)["logs/bieznia.log"].clone()).expect("utf8");
    assert!(text.contains("stare"));
}

/// Lokalny serwer HTTP na 127.0.0.1 (bez sieci): jedna odpowiedź, oddaje surowe żądanie.
fn local_server(status: &'static str, body: &'static str) -> (String, mpsc::Receiver<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("port");
    let url = format!(
        "http://{}/zgloszenie",
        listener.local_addr().expect("adres")
    );
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (stream, _) = listener.accept().expect("połączenie");
        let mut reader = BufReader::new(stream.try_clone().expect("strumień"));
        let mut head = Vec::new();
        let mut length = 0usize;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).expect("nagłówek");
            if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                length = v.trim().parse().expect("długość");
            }
            head.extend_from_slice(line.as_bytes());
            if line == "\r\n" || line.is_empty() {
                break;
            }
        }
        let mut data = vec![0; length];
        reader.read_exact(&mut data).expect("treść");
        head.extend_from_slice(&data);
        let mut stream = stream;
        let reply = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(reply.as_bytes()).expect("odpowiedź");
        let _ = tx.send(head);
    });
    (url, rx)
}

fn send_local(url: &str, period_s: Option<f64>, report: &Path) -> Result<String, ReportError> {
    let fields = form_fields("opis", "", period_s, "0.6.0");
    let file = fs::read(report).expect("paczka");
    let (body, content_type) = multipart_body(&fields, &file_name(report), &file, "granica");
    post(url, body, &content_type, "0.6.0")
}

#[test]
fn send_report_returns_number_and_maps_errors() {
    let dir = tempfile::tempdir().expect("tmp");
    let path = build_report(
        &ReportFolders::in_root(dir.path()),
        &info("x", "", RECENT_S),
    )
    .expect("paczka");

    let (url, rx) = local_server("200 OK", r#"{"id": "42"}"#);
    assert_eq!(send_local(&url, None, &path).expect("wysyłka"), "42");
    let request = String::from_utf8_lossy(&rx.recv().expect("żądanie")).into_owned();
    assert!(request.starts_with("POST /zgloszenie"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("user-agent: bieznia/0.6.0")
    );
    assert!(request.contains("multipart/form-data; boundary=granica"));
    assert!(request.contains("name=\"paczka\""));
    assert!(!request.contains("name=\"okres\""));

    let (url, rx) = local_server("200 OK", "{}");
    assert_eq!(
        send_local(&url, Some(6.0 * 3600.0), &path).expect("wysyłka"),
        ""
    );
    let request = String::from_utf8_lossy(&rx.recv().expect("żądanie")).into_owned();
    assert!(request.contains("name=\"okres\"\r\n\r\n6\r\n")); // okres w godzinach

    let (url, _rx) = local_server("400 Bad Request", r#"{"message": "Brak opisu."}"#);
    let err = send_local(&url, None, &path).expect_err("400");
    assert_eq!(err.to_string(), "Brak opisu.");
    let (url, _rx) = local_server("413 Payload Too Large", "<html>too large</html>");
    assert!(
        send_local(&url, None, &path)
            .expect_err("413")
            .to_string()
            .contains("za duża")
    );
    let (url, _rx) = local_server("503 Service Unavailable", "{}");
    assert!(
        send_local(&url, None, &path)
            .expect_err("503")
            .to_string()
            .contains("Zapisz paczkę")
    );

    // 307 jak w urllib: bez ponownego wysłania paczki, błąd z kodem (nagłówek Location
    // doklejony do linii statusu)
    let (url, _rx) = local_server(
        "307 Temporary Redirect\r\nLocation: /gdzie-indziej",
        r#"{"message": ""}"#,
    );
    assert_eq!(
        send_local(&url, None, &path).expect_err("307").to_string(),
        "Serwer nie przyjął zgłoszenia. Zapisz paczkę na dysk i wyślij ją e-mailem. (kod 307)"
    );

    // nikt nie słucha na porcie - komunikat o internecie
    let closed = TcpListener::bind("127.0.0.1:0").expect("port");
    let url = format!("http://{}/z", closed.local_addr().expect("adres"));
    drop(closed);
    assert_eq!(
        send_local(&url, None, &path)
            .expect_err("brak serwera")
            .to_string(),
        network_error().to_string()
    );
}

#[test]
fn short_description_messages() {
    assert_eq!(
        description_problem("   ").as_deref(),
        Some("Opisz, co się stało. Wystarczy jedno zdanie.")
    );
    assert_eq!(
        description_problem("abcd").as_deref(),
        Some("Opis jest za krótki. Dopisz jeszcze 1 znak, np. co poszło nie tak.")
    );
    assert_eq!(
        description_problem(" ab ").as_deref(),
        Some("Opis jest za krótki. Dopisz jeszcze 3 znaki, np. co poszło nie tak.")
    );
    assert_eq!(description_problem("żółć!"), None);
    assert_eq!(
        sent_message("7"),
        "Wysłano zgłoszenie nr 7. Dziękujemy, autor dostał opis i dziennik."
    );
    assert_eq!(
        sent_message(""),
        "Wysłano zgłoszenie. Dziękujemy, autor dostał opis i dziennik."
    );
}

#[test]
fn python_rounding_and_float_repr() {
    assert_eq!(round1(5580.0 / 3600.0), 1.6);
    assert_eq!(round1(0.25), 0.2);
    assert_eq!(py_float(3.0), "3.0");
    assert_eq!(py_float(2.5), "2.5");
    let hours = |p: f64| form_fields("x", "", Some(p), "v").pop().map(|f| f.1);
    assert_eq!(hours(5400.0).as_deref(), Some("2"));
    assert_eq!(hours(9000.0).as_deref(), Some("2")); // 2.5 -> 2 jak round() w Pythonie
    assert_eq!(hours(600.0).as_deref(), Some("1"));
}
