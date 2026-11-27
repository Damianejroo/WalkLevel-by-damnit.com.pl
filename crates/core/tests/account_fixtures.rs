//! Zgodność z Pythonem: wyniki z tools/fixtures/konto.py (prawdziwa implementacja
//! 0.5.x, bieznia/konto) porównane z modułem account dla tych samych danych.

use serde_json::Value;
use walklevel_core::account::{
    self, AccountState, check_base_url, field_errors, friendly, load_states, session_payload,
    state::states_text,
};
use walklevel_core::history::Session;

fn fixture() -> Value {
    let path = format!(
        "{}/tests/fixtures/account/account.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).expect("fixtura");
    serde_json::from_str(&text).expect("JSON fixtury")
}

fn s(v: &Value) -> &str {
    v.as_str().expect("tekst")
}

fn cases<'a>(data: &'a Value, key: &str) -> &'a Vec<Value> {
    data[key].as_array().expect("lista przypadków")
}

/// Tekst z fixtury 0.5.2 w brzmieniu 0.6.0 (docs/texts.md).
fn old(text: &str) -> String {
    walklevel_core::texts::from_052(text, &["konto.blad."])
}

#[test]
fn field_checks_match_python() {
    let data = fixture();
    type Check = fn(&str) -> String;
    let checks: [(&str, Check); 4] = [
        ("names", account::check_name),
        ("emails", account::check_email),
        ("passwords", account::check_password),
        ("codes", account::check_code),
    ];
    for (key, check) in checks {
        for case in cases(&data, key) {
            assert_eq!(
                check(s(&case["in"])),
                old(s(&case["out"])),
                "{key}: {:?}",
                case["in"]
            );
        }
    }
}

#[test]
fn canonical_uuid_matches_python() {
    let data = fixture();
    for case in cases(&data, "uuids") {
        assert_eq!(account::canonical_uuid(s(&case["in"])), s(&case["out"]));
    }
}

#[test]
fn base_url_rules_match_python() {
    let data = fixture();
    for case in cases(&data, "bases") {
        let got = check_base_url(s(&case["url"]));
        match case.get("ok") {
            Some(ok) => assert_eq!(got.ok().as_deref(), Some(s(ok)), "{:?}", case["url"]),
            None => assert_eq!(
                got.err().map(|e| e.message),
                Some(old(s(&case["error"]))),
                "{:?}",
                case["url"]
            ),
        }
    }
}

#[test]
fn friendly_messages_and_field_errors_match_python() {
    let data = fixture();
    for case in cases(&data, "friendly") {
        let status = case["status"].as_u64().expect("kod") as u16;
        let got = friendly(status, &case["body"], s(&case["path"]));
        // 426 bez komunikatu serwera: w 0.6.0 tekst zastępczy konto.blad.aktualizacja
        let want = if status == 426 && s(&case["out"]).contains("426") {
            walklevel_core::texts::t("konto.blad.aktualizacja", &[])
        } else {
            old(s(&case["out"]))
        };
        assert_eq!(got, want, "{case}");
    }
    for case in cases(&data, "fields") {
        let got = serde_json::to_value(field_errors(&case["body"])).expect("json");
        assert_eq!(got, case["out"], "{case}");
    }
}

fn session(v: &Value) -> Session {
    let t = |k: &str| s(&v[k]).to_string();
    let f = |k: &str| v[k].as_f64().expect("liczba");
    Session {
        id: t("id"),
        person: t("person"),
        person_name: t("person_name"),
        profile: t("profile"),
        profile_name: t("profile_name"),
        start: f("start"),
        end: f("end"),
        steps: v["steps"].as_i64().expect("kroki"),
        distance_km: f("distance_km"),
        active_s: f("active_s"),
        device_model: t("device_model"),
        driver: t("driver"),
        device_profile: t("device_profile"),
        firmware: t("firmware"),
        app_version: t("app_version"),
        source: t("source"),
        visibility: t("visibility"),
        tz_offset_min: v["tz_offset_min"].as_i64().expect("strefa"),
        updated: f("updated"),
        mode: t("mode"),
    }
}

#[test]
fn upload_payload_matches_python() {
    // 0.6.1: nazwa gry idzie zawsze (po regulaminie z grami), przełącznik profilu to
    // `gra_publiczna` - poza tymi dwoma polami sesja jak w 0.5.x
    let data = fixture();
    let all = cases(&data, "payloads");
    assert!(all.len() > 50);
    for case in all {
        let game = case["game"].as_bool().expect("game");
        let games = case["details"].as_bool().unwrap_or(game);
        let session = session(&case["session"]);
        let got = session_payload(&session, s(&case["visibility"]), games, game);
        let mut got = serde_json::to_value(&got).expect("json");
        let has_id = got.get("gra_id").is_some();
        let fields = got.as_object_mut().expect("obiekt");
        assert_eq!(
            fields.remove("gra_publiczna"),
            has_id.then_some(Value::Bool(game)),
            "{}",
            case["name"]
        );
        let name = fields.remove("gra");
        if has_id && !account::game_name(&session.profile_name).is_empty() {
            assert!(name.is_some(), "{}: nazwa gry zawsze", case["name"]);
        }
        let mut want = case["payload"].clone();
        if let Some(w) = want.as_object_mut() {
            w.remove("gra");
        }
        assert_eq!(got, want, "{} {:?}", case["name"], case["details"]);
        if game && games {
            assert_eq!(
                name,
                case["payload"].get("gra").cloned(),
                "{}",
                case["name"]
            );
        }
    }
}

#[test]
fn account_file_is_written_and_read_like_python() {
    let data = fixture();
    let states = &data["states"];
    let saved: Value = serde_json::from_str(s(&states["saved_two"])).expect("json");
    let two: Vec<AccountState> = saved["konta"]
        .as_array()
        .expect("konta")
        .iter()
        .map(AccountState::from_value)
        .collect();
    assert_eq!(states_text(&two), s(&states["saved_two"])); // co do bajtu
    assert_eq!(states_text(&[]), s(&states["saved_none"]));

    let dir = tempfile::tempdir().expect("katalog");
    let path = dir.path().join("konto.json");
    for case in states["reads"].as_array().expect("odczyty") {
        let text = match case.get("input") {
            Some(raw) => raw.to_string(),
            None => s(&case["text"]).to_string(),
        };
        std::fs::write(&path, text).expect("zapis");
        let got = serde_json::to_value(load_states(&path)).expect("json");
        assert_eq!(got, case["states"], "{case}");
    }
}

#[test]
fn terms_version_matches_python() {
    assert_eq!(account::TERMS_VERSION, s(&fixture()["terms_version"]));
}
