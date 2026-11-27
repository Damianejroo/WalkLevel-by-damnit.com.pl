//! Zgodność z Pythonem: wyniki z tools/fixtures/protokoly.py (prawdziwa
//! implementacja 0.5.x) porównane z rdzeniem w Rust dla tych samych danych.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use walklevel_core::treadmill::kingsmith_proto::{self as ks, Codec, FrameBuffer};
use walklevel_core::treadmill::profiles::{self, ModelProfile, ProfileRegistry};
use walklevel_core::treadmill::simulator::{self, SimulatorDriver};
use walklevel_core::treadmill::{
    Capabilities, Clock, Driver, DriverKind, FrameCallback, SpeedGuard, ftms_proto as ftms,
    known_capabilities, looks_like_treadmill, model_name,
};

fn fixture(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/protocols/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).expect("fixtura");
    serde_json::from_str(&text).expect("JSON fixtury")
}

fn bytes(value: &Value) -> Vec<u8> {
    hex::decode(value.as_str().expect("hex")).expect("hex")
}

fn f(value: &Value) -> f64 {
    value.as_f64().expect("liczba")
}

fn opt_f(value: &Value) -> Option<f64> {
    value.as_f64()
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("lista")
        .iter()
        .map(|v| v.as_str().expect("tekst").to_string())
        .collect()
}

fn caps_py(c: &Capabilities) -> Value {
    json!({
        "speed_min": c.speed_min,
        "speed_max": c.speed_max,
        "speed_step": c.speed_step,
        "start_speed": c.start_speed,
        "start_delay_s": c.start_delay_s,
        "acks_commands": c.acks_commands,
        "pause": c.pause,
        "steps": c.steps,
        "incline": c.incline.map(|(a, b, s)| vec![a, b, s]),
        "telemetry_s": c.telemetry_s,
        "min_command_gap_s": c.min_command_gap_s,
        "control": c.control,
    })
}

fn profile_py(p: Option<&ModelProfile>) -> Value {
    let Some(p) = p else { return Value::Null };
    let caps: Vec<Value> = p
        .capabilities
        .entries()
        .into_iter()
        .map(|(k, v)| json!([k, v]))
        .collect();
    let options: Vec<Value> = p.options.iter().map(|(k, v)| json!([k, v])).collect();
    json!({
        "id": p.id,
        "name": p.name,
        "driver": p.driver.as_str(),
        "match": p.match_prefixes,
        "capabilities": caps,
        "options": options,
        "source": p.source,
    })
}

/// Równe z dokładnością do ostatniego bitu: serde_json bez funkcji "float_roundtrip" czyta
/// długie liczby (np. 7.7524999999999995) z błędem 1 ULP. Wszystko inne musi być identyczne.
fn near(a: f64, b: f64) -> bool {
    a == b || (a - b).abs() <= 2.0 * f64::EPSILON * a.abs().max(b.abs())
}

/// Liczby porównujemy jako f64 (Python zapisuje 15.0, Rust może mieć 15).
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => match (x.as_f64(), y.as_f64()) {
            (Some(p), Some(q)) => near(p, q),
            _ => false,
        },
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w)))
        }
        _ => a == b,
    }
}

fn assert_same(actual: &Value, expected: &Value, what: &str) {
    assert!(
        same(actual, expected),
        "{what}: Rust {actual} != Python {expected}"
    );
}

// --- KingSmith -------------------------------------------------------------------------------

#[test]
fn kingsmith_encode_decode_match_python() {
    let data = fixture("kingsmith.json");
    for case in data["encode"].as_array().expect("encode") {
        let text = case["text"].as_str().expect("tekst");
        let table = case["table"].as_str().expect("tablica");
        assert_eq!(
            ks::encode(text, table),
            Some(bytes(&case["hex"])),
            "encode {text:?} {table}"
        );
    }
    let cases = data["decode"].as_array().expect("decode");
    assert!(cases.len() > 300);
    for case in cases {
        let frame = bytes(&case["hex"]);
        for (table, expected) in case["texts"].as_object().expect("teksty") {
            let expected = expected.as_str().map(str::to_string);
            assert_eq!(
                ks::decode(&frame, table),
                expected,
                "decode {} {table}",
                hex::encode(&frame)
            );
        }
    }
}

#[test]
fn kingsmith_base64_padding_matches_python() {
    let data = fixture("kingsmith.json");
    let cases = data["decode_padding"].as_array().expect("dopełnienie");
    assert!(cases.len() > 700);
    for case in cases {
        let frame = bytes(&case["hex"]);
        assert_eq!(
            ks::decode(&frame, "v1").as_deref(),
            case["text"].as_str(),
            "{}",
            String::from_utf8_lossy(&frame)
        );
    }
}

#[test]
fn kingsmith_replies_drive_state_like_python() {
    let data = fixture("x21-trace.json");
    let mut telemetry = ks::Telemetry::new(&ks::table_order("KS-HDSY-X21C"));
    for step in data["handle"].as_array().expect("odpowiedzi") {
        let text = step["text"].as_str().expect("tekst");
        let reading = telemetry.handle(text, f(&step["t"]));
        // Python oddawał ostatnie kroki przy każdej ramce; ramka 0.6 niesie tylko obecne
        // pole (kroki sprzed ramki są w `last_steps`)
        let actual: Vec<Value> = reading
            .iter()
            .map(|r| json!([r.speed_kmh, telemetry.last_steps]))
            .collect();
        assert_same(&json!(actual), &step["readings"], text);
        assert_eq!(telemetry.run_state, step["run_state"].as_i64(), "{text}");
        assert_eq!(
            telemetry.control_mode,
            step["control_mode"].as_i64(),
            "{text}"
        );
        assert_eq!(telemetry.last_error, step["last_error"].as_i64(), "{text}");
        assert_eq!(
            telemetry.firmware,
            step["firmware"].as_str().expect("firmware")
        );
        assert_eq!(telemetry.last_speed, f(&step["last_speed"]), "{text}");
        assert_eq!(telemetry.last_steps, step["last_steps"].as_i64(), "{text}");
        let belt = telemetry.belt_speed().map(|(s, t)| vec![s, t]);
        assert_same(&json!(belt), &step["belt_speed"], text);
        assert_eq!(telemetry.belt_running(), step["belt_running"].as_bool());
    }
}

#[test]
fn kingsmith_texts_match_python() {
    let data = fixture("kingsmith.json");
    for case in data["known"].as_array().expect("known") {
        let text = case["text"].as_str().expect("tekst");
        assert_eq!(
            ks::is_known_reply(text),
            case["known"].as_bool().expect("bool"),
            "{text:?}"
        );
    }
    for case in data["props"].as_array().expect("props") {
        let text = case["text"].as_str().expect("tekst");
        let actual: serde_json::Map<String, Value> = ks::parse_props(text)
            .into_iter()
            .map(|(k, v)| {
                let value = if v.is_finite() {
                    json!(v)
                } else if v.is_nan() {
                    json!("nan")
                } else {
                    json!(if v > 0.0 { "inf" } else { "-inf" })
                };
                (k, value)
            })
            .collect();
        assert_same(&Value::Object(actual), &case["props"], text);
    }
    for case in data["speed_command"].as_array().expect("speed") {
        assert_eq!(
            ks::speed_command(f(&case["kmh"])),
            case["command"].as_str().expect("tekst"),
            "{case}"
        );
    }
    for case in data["table_order"].as_array().expect("order") {
        let order: Vec<String> = ks::table_order(case["name"].as_str().expect("nazwa"))
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(order, strings(&case["order"]));
    }
}

#[test]
fn kingsmith_codec_matches_python() {
    let data = fixture("kingsmith.json");
    for scenario in data["codec"].as_array().expect("codec") {
        let start: Vec<String> = if scenario["start"].is_null() {
            vec![]
        } else {
            strings(&scenario["start"])
        };
        let start: Vec<&str> = start.iter().map(String::as_str).collect();
        let mut codec = Codec::new(&start);
        for step in scenario["steps"].as_array().expect("kroki") {
            if step["op"] == "next" {
                codec.try_next_table();
            } else {
                let decoded = codec.decode(&bytes(&step["hex"]));
                assert_eq!(
                    decoded.as_ref().map(|d| d.text.as_str()),
                    step["text"].as_str(),
                    "{step}"
                );
                assert_eq!(
                    decoded.map(|d| d.trusted),
                    step["trusted"].as_bool(),
                    "{step}"
                );
                assert_eq!(codec.locked(), step["locked"].as_bool().expect("bool"));
                assert_eq!(codec.describe(), step["describe"].as_str().expect("tekst"));
            }
            assert_eq!(
                codec.table(),
                step["table"].as_str().expect("tablica"),
                "{step}"
            );
            let candidates: Vec<String> =
                codec.candidates().iter().map(|s| s.to_string()).collect();
            assert_eq!(candidates, strings(&step["candidates"]), "{step}");
        }
    }
}

#[test]
fn kingsmith_frame_buffer_matches_python() {
    let data = fixture("kingsmith.json");
    for feeds in data["frame_buffer"].as_array().expect("bufory") {
        let mut fb = FrameBuffer::new();
        for feed in feeds.as_array().expect("porcje") {
            let frames: Vec<String> = fb
                .feed(&bytes(&feed["hex"]))
                .iter()
                .map(hex::encode)
                .collect();
            assert_eq!(frames, strings(&feed["frames"]));
        }
    }
}

#[test]
fn x21_trace_is_received_like_python() {
    let data = fixture("x21-trace.json");
    let registry = ProfileRegistry::builtin();
    let name = data["device_name"].as_str().expect("nazwa");
    let profile = registry.profile_for(name, Some(DriverKind::KingSmith));
    let order = ks::Telemetry::order_for(name, profile.and_then(|p| p.option_str("table")));
    let mut telemetry = ks::Telemetry::new(&order);
    let channel = "00011234-0000-1000-8000-00805f9b34fb";
    for event in data["events"].as_array().expect("zdarzenia") {
        let now = f(&event["t"]);
        let readings = telemetry.feed(channel, &bytes(&event["hex"]), now);
        let actual: Vec<Value> = readings
            .iter()
            .map(|r| json!({"t": now, "speed": r.speed_kmh, "steps": telemetry.last_steps}))
            .collect();
        assert_same(&json!(actual), &event["readings"], "odczyty");
        assert_eq!(
            telemetry.codec.table(),
            event["table"].as_str().expect("tablica")
        );
        assert_eq!(telemetry.run_state, event["run_state"].as_i64());
        assert_eq!(telemetry.control_mode, event["control_mode"].as_i64());
        assert_eq!(telemetry.last_error, event["last_error"].as_i64());
        assert_eq!(
            telemetry.firmware,
            event["firmware"].as_str().expect("firmware")
        );
        assert_eq!(telemetry.belt_running(), event["belt_running"].as_bool());
    }
    let counts = &data["counts"];
    assert_eq!(
        telemetry.counts.frames,
        counts["frames"].as_u64().expect("frames")
    );
    assert_eq!(
        telemetry.counts.undecoded,
        counts["undecoded"].as_u64().expect("undecoded")
    );
    assert_eq!(
        telemetry.counts.acks,
        counts["acks"].as_u64().expect("acks")
    );
    assert_eq!(telemetry.codec.table(), "v5");
}

// --- FTMS ------------------------------------------------------------------------------------

#[test]
fn ftms_frames_and_commands_match_python() {
    let data = fixture("ftms.json");
    for case in data["decode"].as_array().expect("decode") {
        let actual = ftms::decode_treadmill_data(&bytes(&case["hex"])).map(|d| {
            json!({
                "speed_kmh": d.speed_kmh,
                "distance_m": d.distance_m,
                "incline_pct": d.incline_pct,
                "energy_kcal": d.energy_kcal,
                "heart_rate": d.heart_rate,
                "elapsed_s": d.elapsed_s,
                "steps": d.steps,
            })
        });
        assert_same(
            &json!(actual),
            &case["data"],
            case["hex"].as_str().unwrap_or(""),
        );
    }
    for case in data["firmware"].as_array().expect("firmware") {
        assert_eq!(
            ftms::decode_firmware(&bytes(&case["hex"])),
            case["text"].as_str().expect("tekst"),
            "{case}"
        );
    }
    for case in data["set_speed"].as_array().expect("speed") {
        assert_eq!(
            ftms::encode_set_speed(f(&case["kmh"])).ok(),
            Some(bytes(&case["hex"])),
            "{case}"
        );
    }
    for case in data["set_incline"].as_array().expect("incline") {
        assert_eq!(
            ftms::encode_set_incline(f(&case["pct"])).ok(),
            Some(bytes(&case["hex"])),
            "{case}"
        );
    }
    for case in data["responses"].as_array().expect("odpowiedzi") {
        let actual = ftms::parse_response(&bytes(&case["hex"])).map(|(a, b)| vec![a, b]);
        assert_same(&json!(actual), &case["result"], "odpowiedź");
    }
    for case in data["ranges"].as_array().expect("zakresy") {
        let raw = bytes(&case["hex"]);
        let speed = ftms::decode_speed_range(&raw).map(|(a, b, c)| vec![a, b, c]);
        let incline = ftms::decode_incline_range(&raw).map(|(a, b, c)| vec![a, b, c]);
        assert_same(&json!(speed), &case["speed"], "zakres prędkości");
        assert_same(&json!(incline), &case["incline"], "zakres nachylenia");
    }
    let commands = &data["commands"];
    assert_eq!(
        ftms::encode_request_control(),
        bytes(&commands["request_control"])
    );
    assert_eq!(ftms::encode_start(), bytes(&commands["start"]));
    assert_eq!(ftms::encode_stop(), bytes(&commands["stop"]));
    assert_eq!(ftms::encode_pause(), bytes(&commands["pause"]));
}

#[test]
fn urevo_trace_drives_state_like_python() {
    let data = fixture("urevo-trace.json");
    let registry = ProfileRegistry::builtin();
    let profile = registry
        .profile_for("URTM054", Some(DriverKind::Ftms))
        .expect("profil 5L");
    let mut caps = profile.apply(&ftms::DEFAULT_CAPS);
    caps.steps = false; // jak w fixturze: sprawdzamy przełączenie po pierwszej ramce z krokami
    let mut telemetry = ftms::Telemetry::new(caps);
    let mut speeds = Vec::new();
    for event in data["events"].as_array().expect("zdarzenia") {
        let now = f(&event["t"]);
        let raw = bytes(&event["hex"]);
        let readings: Vec<Value> = if event["kind"] == "data" {
            telemetry
                .on_data(&raw, now)
                .map(|r| {
                    speeds.push(r.speed_kmh.unwrap_or_default());
                    json!({"t": now, "speed": r.speed_kmh, "steps": r.steps})
                })
                .into_iter()
                .collect()
        } else {
            telemetry.on_status(&raw);
            vec![]
        };
        assert_same(&json!(readings), &event["readings"], "odczyty");
        assert_eq!(telemetry.running, event["running"].as_bool(), "{event}");
        let belt = telemetry.last_speed.map(|(s, t)| vec![s, t]);
        assert_same(&json!(belt), &event["belt_speed"], "prędkość taśmy");
        assert_eq!(
            telemetry.capabilities.steps,
            event["steps_cap"].as_bool().expect("bool")
        );
    }
    // jak test_urevo_trace_drives_state_like_the_real_session w Pythonie
    for speed in [1.0, 1.5, 2.5, 4.0] {
        assert!(speeds.contains(&speed));
    }
    let after = speeds.iter().position(|s| *s == 4.0).expect("4,0");
    assert!(speeds[after..].contains(&2.8) && speeds.last() == Some(&0.0));
    assert_eq!(telemetry.running, Some(false));
    assert_eq!(
        telemetry.counts.frames,
        data["counts"]["frames"].as_u64().expect("frames")
    );
}

// --- profile i możliwości -----------------------------------------------------------------------

#[test]
fn profiles_match_python() {
    let data = fixture("profiles.json");
    for case in data["parse"].as_array().expect("parse") {
        let parsed = profiles::parse_profile(&case["input"], profiles::SOURCE_BUILTIN);
        assert_same(
            &profile_py(parsed.as_ref().ok()),
            &case["profile"],
            &case["input"].to_string(),
        );
        // komunikat trafia do użytkownika przy dodawaniu profilu z pliku
        if let Some(message) = case["error"].as_str() {
            let actual = parsed.err().map(|e| e.to_string());
            assert_eq!(actual.as_deref(), Some(message), "{}", case["input"]);
        }
    }
    let registry = ProfileRegistry::builtin();
    let all: Vec<Value> = registry.all().iter().map(|p| profile_py(Some(p))).collect();
    assert_same(&json!(all), &data["all"], "wszystkie profile");
    for case in data["lookup"].as_array().expect("lookup") {
        let name = case["name"].as_str().expect("nazwa");
        assert_same(
            &profile_py(registry.profile_for(name, None)),
            &case["any"],
            name,
        );
        let id = |kind| registry.profile_for(name, Some(kind)).map(|p| p.id.clone());
        assert_eq!(
            id(DriverKind::KingSmith).as_deref(),
            case["kingsmith"].as_str(),
            "{name}"
        );
        assert_eq!(
            id(DriverKind::Ftms).as_deref(),
            case["ftms"].as_str(),
            "{name}"
        );
        let caps = known_capabilities(&registry, name, false);
        assert_same(&caps_py(&caps), &case["known_capabilities"], name);
        assert_eq!(
            model_name(&registry, name),
            case["model_name"].as_str().expect("model")
        );
        let none: [&str; 0] = [];
        assert_eq!(
            looks_like_treadmill(&registry, Some(name), &none),
            case["looks_like"].as_bool().expect("bool")
        );
    }
    for case in data["services"].as_array().expect("usługi") {
        let uuids = strings(&case["uuids"]);
        assert_eq!(
            looks_like_treadmill(&registry, None, &uuids),
            case["looks_like"].as_bool().expect("bool")
        );
    }
    assert_same(
        &caps_py(&known_capabilities(&registry, "x", true)),
        &data["simulator_caps"],
        "symulator",
    );
    assert_same(
        &caps_py(&ks::DEFAULT_CAPS),
        &data["kingsmith_caps"],
        "kingsmith",
    );
    assert_same(&caps_py(&ftms::DEFAULT_CAPS), &data["ftms_caps"], "ftms");
    assert_same(
        &caps_py(&Capabilities::DEFAULT),
        &data["default_caps"],
        "domyślne",
    );
}

/// Folder dodatkowych profili: te same pliki, ta sama lista co `all_profiles()` w Pythonie
/// (fixtura z Windows: `*.json` bez wielkości liter, kolejność plików jak `sorted(Path)`).
#[cfg(windows)]
#[test]
fn user_profile_folder_loads_like_python() {
    let data = fixture("profiles.json");
    let dir = tempfile::tempdir().expect("folder");
    for (name, hex) in data["user_dir"]["files"].as_object().expect("pliki") {
        std::fs::write(dir.path().join(name), bytes(hex)).expect("zapis");
    }
    let registry = ProfileRegistry::load(dir.path());
    let all: Vec<Value> = registry.all().iter().map(|p| profile_py(Some(p))).collect();
    assert_same(&json!(all), &data["user_dir"]["all"], "folder profili");
}

#[test]
fn capabilities_match_python() {
    let data = fixture("capabilities.json");
    let caps = [Capabilities::DEFAULT, ftms::DEFAULT_CAPS, ks::DEFAULT_CAPS];
    for (c, expected) in caps.iter().zip(data["caps"].as_array().expect("caps")) {
        assert_same(&caps_py(c), expected, "możliwości");
    }
    for case in data["clamp"].as_array().expect("clamp") {
        let c = &caps[case["caps"].as_u64().expect("indeks") as usize];
        assert_eq!(c.clamp(f(&case["kmh"])), f(&case["out"]), "{case}");
    }
    for case in data["with_range"].as_array().expect("zakresy") {
        let c = &caps[case["caps"].as_u64().expect("indeks") as usize];
        let r = case["range"].as_array().expect("zakres");
        let out = c.with_range(f(&r[0]), f(&r[1]), f(&r[2]));
        assert_same(&caps_py(&out), &case["out"], "with_range");
    }
}

// --- pilnowanie i symulator ---------------------------------------------------------------------

#[test]
fn watchdog_scenarios_match_python() {
    let caps = ks::DEFAULT_CAPS;
    for scenario in fixture("watchdog.json").as_array().expect("scenariusze") {
        let outcomes = strings(&scenario["outcomes"]);
        let mut speed = f(&scenario["start_speed"]);
        let mut at = 0.0;
        let mut clock = 100.0;
        let mut calls: Vec<f64> = Vec::new();
        let mut guard = SpeedGuard::new();
        let send = |kmh: f64, speed: &mut f64, calls: &mut Vec<f64>| {
            calls.push(kmh);
            let outcome = outcomes[calls.len() - 1].as_str();
            if outcome == "ok" {
                *speed = caps.clamp(kmh);
            }
            outcome != "fail"
        };
        let reading = |speed: f64, at: f64| (at != 0.0).then_some((speed, at));
        let ops = scenario["ops"].as_array().expect("operacje");
        let steps = scenario["steps"].as_array().expect("kroki");
        for (op, step) in ops.iter().zip(steps) {
            match op["op"].as_str().expect("op") {
                "set" => {
                    let kmh = guard.set(f(&op["kmh"]), clock, reading(speed, at));
                    let ok = send(kmh, &mut speed, &mut calls);
                    assert_eq!(guard.sent(ok, clock), op["ok"].as_bool().expect("ok"));
                }
                "reset" => guard.reset(),
                "belt" => speed = f(&op["speed"]),
                _ => {
                    clock += f(&op["dt"]);
                    if op["fresh"].as_bool().expect("fresh") {
                        at = clock;
                    }
                    if let Some(kmh) = guard.check(clock, reading(speed, at), |v| caps.clamp(v)) {
                        let ok = send(kmh, &mut speed, &mut calls);
                        guard.sent(ok, clock);
                    }
                }
            }
            assert_eq!(
                calls.len() as u64,
                step["calls"].as_u64().expect("calls"),
                "{op}"
            );
            assert_eq!(guard.target(), opt_f(&step["target"]), "{op}");
            assert_eq!(
                guard.confirmed(),
                step["confirmed"].as_bool().expect("confirmed"),
                "{op}"
            );
            let s = &guard.stats;
            let stats = json!([s.sent, s.ok, s.failed, s.resends, s.latencies]);
            assert_same(&stats, &step["stats"], "liczniki");
        }
        assert_same(&json!(calls), &scenario["calls"], "polecenia");
    }
}

#[tokio::test]
async fn simulator_scenarios_match_python() {
    for scenario in fixture("simulator.json").as_array().expect("scenariusze") {
        let clock = Arc::new(AtomicU64::new(0f64.to_bits()));
        let readings = Arc::new(Mutex::new(Vec::new()));
        let sink_readings = readings.clone();
        let sink: FrameCallback = Arc::new(move |r| sink_readings.lock().expect("odczyty").push(r));
        let tick = clock.clone();
        let now: Clock = Arc::new(move || f64::from_bits(tick.load(Ordering::SeqCst)));
        let caps = Capabilities {
            start_delay_s: 0.0,
            ..simulator::CAPS
        };
        let sim = SimulatorDriver::new(sink, now, caps);
        sim.start().await.expect("start");
        let steps = scenario["steps"].as_array().expect("kroki");
        for (op, step) in scenario["ops"]
            .as_array()
            .expect("operacje")
            .iter()
            .zip(steps)
        {
            match op["op"].as_str().expect("op") {
                "start_belt" => {
                    let result = sim.start_belt().await.expect("start taśmy");
                    assert_eq!(result.as_str(), op["result"].as_str().expect("wynik"));
                }
                "stop_belt" => {
                    sim.stop_belt(true).await.expect("stop");
                }
                "set_speed" => {
                    let ok = sim.set_speed(f(&op["kmh"])).await.expect("prędkość");
                    assert_eq!(ok, op["result"].as_bool().expect("wynik"));
                }
                "remote" => sim.remote(op["steps"].as_i64().expect("kroki") as i32),
                _ => {
                    let t = f64::from_bits(clock.load(Ordering::SeqCst)) + f(&op["dt"]);
                    clock.store(t.to_bits(), Ordering::SeqCst);
                    sim.advance();
                }
            }
            let state = sim.state();
            let count = readings.lock().expect("odczyty").len() as u64;
            assert_eq!(count, step["readings"].as_u64().expect("odczyty"), "{op}");
            // kroki: 0.6 ma kadencję 40 + 14·v (docs/analysis-distance.md, R9), Python 60 + 15·v
            let floats = [
                (state.speed, &step["speed"]),
                (state.target, &step["target"]),
            ];
            for (actual, expected) in floats {
                assert!(
                    near(actual, f(expected)),
                    "{op}: Rust {actual} != Python {expected}"
                );
            }
            assert_eq!(
                state.running,
                step["running"].as_bool().expect("running"),
                "{op}"
            );
            let belt = sim.belt_speed().map(|(s, t)| vec![s, t]);
            assert_same(&json!(belt), &step["belt_speed"], "prędkość taśmy");
            let counts = sim.counts();
            let actual =
                json!({"commands": counts.commands, "acks": counts.acks, "frames": counts.frames});
            assert_same(&actual, &step["counts"], "liczniki");
        }
        let actual: Vec<Value> = readings
            .lock()
            .expect("odczyty")
            .iter()
            .map(|r| json!(r.speed_kmh))
            .collect();
        let expected: Vec<Value> = scenario["readings"]
            .as_array()
            .expect("odczyty")
            .iter()
            .map(|r| r[0].clone())
            .collect();
        assert_same(&json!(actual), &json!(expected), "odczyty");
    }
}
