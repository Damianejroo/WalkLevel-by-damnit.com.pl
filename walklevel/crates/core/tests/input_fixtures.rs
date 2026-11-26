//! Zgodność modułu input z Pythonem (0.5.x): fixtury z walklevel/tools/fixtures/wejscie.py.

use std::sync::{Arc, Mutex};

use serde_json::Value;
use walklevel_core::input::InputError;
use walklevel_core::input::hotkeys::{Binding, Hotkeys};
use walklevel_core::input::keys::{Key, Mod, key_from_native, problem};
use walklevel_core::input::pad::{
    PadState, StepPicker, axis, button_label, button_order, parse_sony, parse_sony_buttons,
    parse_steps, xinput_buttons,
};
use walklevel_core::input::send::{INPUT_KEYBOARD, KeyEvent, KeySender, KeyboardOutput};

fn load(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/wejscie/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap()
}

fn key(v: &Value) -> Key {
    serde_json::from_value(v.clone()).unwrap()
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-12
}

/// Tekst z fixtury 0.5.2 w brzmieniu 0.6.0 (docs/teksty.md).
fn old(text: &str) -> String {
    walklevel_core::teksty::from_052(text, &["klawisz.", "pad."])
}

#[test]
fn keys_load_and_save_like_python() {
    let f = load("klawisze");
    for case in f["from_dict"].as_array().unwrap() {
        let default = match case["default"].as_str().unwrap() {
            "walk" => Key::walk_default(),
            "run" => Key::run_default(),
            _ => Key::toggle_default(),
        };
        let got = Key::from_value(&case["input"], &default);
        assert_eq!(got, key(&case["output"]), "{}", case["input"]);
    }
    for case in f["to_dict"].as_array().unwrap() {
        let k = key(&case["key"]);
        assert_eq!(k.to_value(), case["dict"]);
        assert_eq!(serde_json::to_value(&k).unwrap(), case["dict"]);
        assert_eq!(k.label(), case["label"].as_str().unwrap());
    }
}

#[test]
fn native_codes_like_python() {
    let f = load("klawisze");
    for case in f["native"].as_array().unwrap() {
        let got = key_from_native(
            case["native"].as_u64().unwrap() as u32,
            case["fallback"].as_str().unwrap(),
            case["vk"].as_i64().unwrap(),
        );
        let want = (!case["key"].is_null()).then(|| {
            let mut k = key(&case["key"]);
            k.name = old(&k.name);
            k
        });
        assert_eq!(got, want, "{case}");
    }
}

#[test]
fn key_problems_like_python() {
    let f = load("klawisze");
    let cases = f["problems"].as_array().unwrap();
    assert!(cases.len() > 1000);
    for case in cases {
        let walk = case.get("walk").map_or_else(Key::walk_default, key);
        let run = case.get("run").map_or_else(Key::run_default, key);
        let toggle = key(&case["toggle"]);
        assert_eq!(toggle.label(), case["label"].as_str().unwrap());
        let want = case["problem"].as_str().map(old);
        assert_eq!(problem(&walk, &run, &toggle), want, "{case}");
    }
}

#[test]
fn pad_logic_like_python() {
    let f = load("pad");
    for case in f["deflection"].as_array().unwrap() {
        let s = PadState::new(
            "p",
            case["x"].as_f64().unwrap(),
            case["y"].as_f64().unwrap(),
        );
        assert!(close(s.deflection(), case["d"].as_f64().unwrap()), "{case}");
    }
    for case in f["parse_steps"].as_array().unwrap() {
        let want: Vec<(i64, f64)> = serde_json::from_value(case["output"].clone()).unwrap();
        assert_eq!(parse_steps(&case["input"]), want, "{}", case["input"]);
    }
    for seq in f["picker"].as_array().unwrap() {
        let mut picker = StepPicker::new();
        for step in seq.as_array().unwrap() {
            let steps: Vec<(i64, f64)> = serde_json::from_value(step["steps"].clone()).unwrap();
            let got = picker.update(&steps, step["d"].as_f64().unwrap());
            assert_eq!(got as u64, step["index"].as_u64().unwrap(), "{step}");
        }
    }
    for case in f["axes"].as_array().unwrap() {
        let n = |k: &str| case[k].as_u64().unwrap() as u32;
        assert!(
            close(
                axis(n("value"), n("lo"), n("hi")),
                case["axis"].as_f64().unwrap()
            ),
            "{case}"
        );
    }
    assert_eq!(button_order(), strings(&f["button_order"]));
}

#[test]
fn pad_reports_like_python() {
    let f = load("pad");
    for case in f["sony"].as_array().unwrap() {
        let report = hex::decode(case["report"].as_str().unwrap()).unwrap();
        match (parse_sony(&report), case["xy"].as_array()) {
            (None, None) => {}
            (Some((x, y)), Some(want)) => {
                assert!(
                    close(x, want[0].as_f64().unwrap()) && close(y, want[1].as_f64().unwrap()),
                    "{case}"
                );
            }
            (got, _) => panic!("{case}: {got:?}"),
        }
        let ds: Vec<String> = parse_sony_buttons(&report, true).into_iter().collect();
        assert_eq!(ds, strings(&case["buttons_ds"]), "{case}");
        let ds4: Vec<String> = parse_sony_buttons(&report, false).into_iter().collect();
        assert_eq!(ds4, strings(&case["buttons_ds4"]), "{case}");
    }
    for case in f["xinput"].as_array().unwrap() {
        let n = |k: &str| case[k].as_u64().unwrap();
        let got: Vec<String> = xinput_buttons(n("buttons") as u16, n("lt") as u8, n("rt") as u8)
            .into_iter()
            .collect();
        assert_eq!(got, strings(&case["pressed"]), "{case}");
    }
    for case in f["labels"].as_array().unwrap() {
        let got = button_label(case["button"].as_str().unwrap(), case["pad"].as_str());
        assert_eq!(got, old(case["label"].as_str().unwrap()), "{case}");
    }
}

#[test]
fn key_events_like_python() {
    let f = load("wysylanie");
    for case in f["events"].as_array().unwrap() {
        let e = KeyEvent::new(
            case["scan"].as_u64().unwrap() as u16,
            case["down"].as_bool().unwrap(),
            case["extended"].as_bool().unwrap(),
        );
        assert_eq!(u64::from(e.scan), case["wScan"].as_u64().unwrap());
        assert_eq!(u64::from(e.flags), case["flags"].as_u64().unwrap());
        assert_eq!(e.extra as u64, case["extra"].as_u64().unwrap());
        assert_eq!(u64::from(INPUT_KEYBOARD), case["type"].as_u64().unwrap());
        assert_eq!(case["vk"].as_u64(), Some(0));
    }
}

type Sent = Arc<Mutex<Vec<(u16, bool, bool)>>>;

struct Flaky {
    fail_at: Vec<u64>,
    calls: u64,
    sent: Sent,
}

impl KeySender for Flaky {
    fn send(&mut self, scan: u16, down: bool, extended: bool) -> Result<(), InputError> {
        let call = self.calls;
        self.calls += 1;
        if self.fail_at.contains(&call) {
            return Err(InputError::SendRejected("odrzucone".into()));
        }
        self.sent.lock().unwrap().push((scan, down, extended));
        Ok(())
    }
}

#[test]
fn keyboard_output_sends_the_same_events_as_python() {
    let f = load("wysylanie");
    for case in f["keyboard"].as_array().unwrap() {
        let sent: Sent = Arc::default();
        let fail_at = case["fail_at"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        let now = Arc::new(Mutex::new(0.0f64));
        let clock = now.clone();
        let mut out = KeyboardOutput::with_sender(
            key(&case["walk"]),
            key(&case["run"]),
            Flaky {
                fail_at,
                calls: 0,
                sent: sent.clone(),
            },
            Box::new(move || *clock.lock().unwrap()),
        );
        for step in case["steps"].as_array().unwrap() {
            *now.lock().unwrap() = step["t"].as_str().unwrap().parse().unwrap();
            match step["action"].as_str().unwrap() {
                "idle" => out.set_keys(false, false),
                "walk" => out.set_keys(true, false),
                "run" => out.set_keys(true, true),
                _ => out.release(),
            }
        }
        let want: Vec<(u16, bool, bool)> = serde_json::from_value(case["sent"].clone()).unwrap();
        assert_eq!(*sent.lock().unwrap(), want);
    }
}

fn binding(v: &Value) -> Binding {
    match v {
        Value::String(s) => Binding::Name(s.clone()),
        Value::Number(n) => Binding::Vk(n.as_i64().unwrap()),
        Value::Array(pair) => {
            let mods = pair[1]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| Mod::from_name(m.as_str().unwrap()).unwrap())
                .collect();
            Binding::Combo(pair[0].as_i64().unwrap(), mods)
        }
        other => panic!("{other}"),
    }
}

#[test]
fn hotkeys_fire_like_python() {
    let f = load("wysylanie");
    for case in f["hotkeys"].as_array().unwrap() {
        let down: Arc<Mutex<Vec<u32>>> = Arc::default();
        let as_vks = |v: &Value| -> Vec<u32> {
            v.as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().unwrap() as u32)
                .collect()
        };
        *down.lock().unwrap() = as_vks(&case["start_down"]);
        let bindings = case["bindings"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(a, b)| (a.clone(), binding(b)))
            .collect();
        let d = down.clone();
        let mut hk =
            Hotkeys::new(bindings, move |vk: u32| d.lock().unwrap().contains(&vk)).unwrap();
        for poll in case["polls"].as_array().unwrap() {
            *down.lock().unwrap() = as_vks(&poll["down"]);
            let mut got = hk.poll();
            got.sort();
            assert_eq!(got, strings(&poll["pressed"]), "{case}");
        }
    }
    for case in f["bad_hotkeys"].as_array().unwrap() {
        let result = Hotkeys::new(
            vec![("toggle".into(), binding(&case["binding"]))],
            |_: u32| false,
        );
        let got = result.err().map(|e| e.to_string());
        assert_eq!(got.as_deref(), case["error"].as_str(), "{case}");
    }
}
