//! Zgodność modułu mapping z Pythonem 0.5.x: te same wejścia -> te same wyniki, bit w bit.
//! Fixtury generuje walklevel/tools/fixtures/mapping.py z prawdziwej implementacji.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use walklevel_core::mapping::{
    BeltCommand, BeltSteering, Bridge, Control, Direction, FrameInput, MappingConfig, Output,
    OutputKind, PadIdle, PadStep, Reading, SpeedMapper, SteeringSettings, StepPicker,
    clamp_training_level, pad_preset, parse_pad_steps, parse_training_levels, round_tenth,
    step_training_level,
};

fn fixture(name: &str) -> Vec<Value> {
    let path = format!(
        "{}/tests/fixtures/mapping/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).expect("plik fixtury");
    serde_json::from_str(&text).expect("JSON fixtury")
}

fn f(v: &Value) -> f64 {
    v.as_f64().expect("liczba")
}

/// Porównanie JSON: liczby zmiennoprzecinkowe do 4 ULP, reszta dokładnie. serde_json bez
/// cechy `float_roundtrip` czyta długie liczby (17 cyfr) z błędem do 1-2 ULP; wejścia fixtur
/// mają krótkie liczby, więc czyta je dokładnie i liczy dokładnie to samo co Python.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) if x.is_f64() || y.is_f64() => {
            let (x, y) = (
                x.as_f64().unwrap_or(f64::NAN),
                y.as_f64().unwrap_or(f64::NAN),
            );
            x == y || (x - y).abs() <= 4.0 * f64::EPSILON * x.abs().max(y.abs())
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| same(x, y))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w)))
        }
        _ => a == b,
    }
}

fn control_json(c: &Control) -> Value {
    json!({"speed": c.speed, "stick": c.stick, "walk": c.walk, "run": c.run})
}

#[test]
fn mapper_matches_python() {
    let cases = fixture("mapper");
    assert!(cases.len() > 20);
    for case in cases {
        let name = case["name"].as_str().expect("nazwa");
        let config = MappingConfig::from_value(&case["config"]).expect("ustawienia");
        let mut m = SpeedMapper::new(config);
        let inputs = case["inputs"].as_array().expect("wejścia");
        let outputs = case["outputs"].as_array().expect("wyniki");
        assert_eq!(inputs.len(), outputs.len());
        for (i, (input, want)) in inputs.iter().zip(outputs).enumerate() {
            let c = m.update(f(&input[0]), f(&input[1]));
            let mut got = control_json(&c);
            got["braking"] = json!(m.braking());
            if want.get("reset_after").is_some() {
                m.reset();
                got["reset_after"] = json!(true);
            }
            assert!(
                same(&got, want),
                "{name}, odczyt {i}
  jest: {got}
  ma być: {want}"
            );
        }
    }
}

#[test]
fn config_matches_python() {
    for case in fixture("config") {
        let got = MappingConfig::from_value(&case["input"]);
        if case["ok"] == json!(true) {
            let want: MappingConfig =
                serde_json::from_value(case["config"].clone()).expect("ustawienia z Pythona");
            assert_eq!(got.expect("poprawne"), want, "{}", case["input"]);
        } else {
            assert!(got.is_err(), "{}", case["input"]);
        }
    }
}

struct SpyOutput(Arc<Mutex<Vec<Value>>>);

impl Output for SpyOutput {
    fn apply(&mut self, c: &Control) {
        self.0
            .lock()
            .expect("spy")
            .push(json!(["apply", c.walk, c.run]));
    }
    fn release_all(&mut self) {
        self.0.lock().expect("spy").push(json!(["release"]));
    }
}

#[test]
fn bridge_matches_python() {
    for case in fixture("bridge") {
        let name = case["name"].as_str().expect("nazwa");
        let config = MappingConfig::from_value(&case["config"]).expect("ustawienia");
        let events = Arc::new(Mutex::new(Vec::new()));
        let enabled = case["enabled"].as_bool().expect("enabled");
        let output = Box::new(SpyOutput(events.clone()));
        let mut bridge = Bridge::new(config, OutputKind::Dry, output, enabled);
        bridge.reset_source();
        let ticks = case["ticks"].as_array().expect("klatki");
        let outputs = case["outputs"].as_array().expect("wyniki");
        for (i, (tick, want)) in ticks.iter().zip(outputs).enumerate() {
            let r = &tick["reading"];
            let reading = (!r.is_null()).then(|| Reading {
                t: f(&r["t"]),
                speed_kmh: f(&r["speed_kmh"]),
                steps: r["steps"].as_i64(),
            });
            events.lock().expect("spy").clear();
            let toggle = tick["toggle"].as_bool().expect("toggle");
            let st = bridge.tick(f(&tick["now"]), toggle, reading);
            let pair = |p: Option<&(f64, f64)>| p.map_or(Value::Null, |&(a, b)| json!([a, b]));
            let got = json!({
                "enabled": st.enabled,
                "control": control_json(&st.control),
                "steps": st.steps,
                "cadence": st.cadence,
                "live": st.live,
                "distance_km": st.distance_km,
                "active_s": st.active_s,
                "history_len": st.history.len(),
                "history_first": pair(st.history.front()),
                "history_last": pair(st.history.back()),
                "events": *events.lock().expect("spy"),
            });
            assert!(
                same(&got, want),
                "{name}, klatka {i}
  jest: {got}
  ma być: {want}"
            );
        }
    }
}

#[test]
fn pad_steps_match_python() {
    for case in fixture("pad_steps") {
        let want: Vec<PadStep> = serde_json::from_value(case["output"].clone()).expect("skoki");
        assert_eq!(parse_pad_steps(&case["input"]), want, "{}", case["input"]);
    }
}

#[test]
fn step_picker_matches_python() {
    for case in fixture("step_picker") {
        let mut picker = StepPicker::default();
        let inputs = case["inputs"].as_array().expect("wejścia");
        let outputs = case["outputs"].as_array().expect("wyniki");
        for (i, (input, want)) in inputs.iter().zip(outputs).enumerate() {
            let count = input[0].as_u64().expect("liczba skoków") as usize;
            let steps = pad_preset(count).expect("gotowe skoki");
            let got = picker.update(&steps, f(&input[1]));
            assert_eq!(
                Some(got as u64),
                want.as_u64(),
                "{}, krok {i}",
                case["name"]
            );
        }
    }
}

#[test]
fn training_levels_match_python() {
    for case in fixture("training_levels") {
        let want: [f64; 4] = serde_json::from_value(case["output"].clone()).expect("poziomy");
        assert_eq!(
            parse_training_levels(&case["input"]),
            want,
            "{}",
            case["input"]
        );
    }
}

fn settings_from(v: &Value) -> SteeringSettings {
    SteeringSettings {
        direction: Direction::from_setting(v["direction"].as_str().expect("direction")),
        belt_control: v["belt_control"].as_bool().expect("belt_control"),
        idle_speed: f(&v["idle_speed"]),
        walk_speed: f(&v["walk_speed"]),
        run_speed: f(&v["run_speed"]),
        pad_steps: parse_pad_steps(&v["pad_steps"]),
        pad_idle: PadIdle::from_setting(v["pad_idle"].as_str().expect("pad_idle")),
        training_levels: parse_training_levels(&v["training_levels"]),
        training_level: v["training_level"].as_u64().expect("training_level") as usize,
    }
}

fn command_json(c: &BeltCommand) -> Value {
    match c {
        BeltCommand::Start => json!({"belt": "start"}),
        BeltCommand::Stop => json!({"belt": "stop"}),
        BeltCommand::SetSpeed {
            kmh,
            start_if_stopped,
        } => {
            json!({"speed": kmh, "start_if_stopped": start_if_stopped})
        }
    }
}

/// Odpowiednik ControlScreen.set_training_level: poziom, a gdy wyłączone - włącza sterowanie.
fn set_level(
    b: &mut BeltSteering,
    s: &mut SteeringSettings,
    enabled: &mut bool,
    level: i64,
    belt_allowed: bool,
) -> Vec<BeltCommand> {
    s.training_level = clamp_training_level(level);
    if *enabled {
        return Vec::new();
    }
    let commands = b.set_control(s, false, true, belt_allowed);
    *enabled = true;
    commands
}

#[test]
fn steering_matches_python() {
    let cases = fixture("steering");
    let mut seen_commands = 0;
    for case in cases {
        let name = case["name"].as_str().expect("nazwa");
        let mut s = settings_from(&case["settings"]);
        let belt_allowed = case["caps_control"].as_bool().expect("caps_control");
        let mut b = BeltSteering::new();
        let mut enabled = false;
        let ops = case["ops"].as_array().expect("operacje");
        let outputs = case["outputs"].as_array().expect("wyniki");
        for (i, (op, want)) in ops.iter().zip(outputs).enumerate() {
            let mut got = json!({});
            let commands: Vec<BeltCommand> = match op["op"].as_str().expect("op") {
                "frame" => {
                    let input = FrameInput {
                        now: f(&op["now"]),
                        enabled,
                        capturing: op["capturing"].as_bool().expect("capturing"),
                        walk_down: op["walk"].as_bool().expect("walk"),
                        run_down: op["run"].as_bool().expect("run"),
                        deflection: op["deflection"].as_f64().unwrap_or(0.0),
                    };
                    let frame = b.frame(&s, &input);
                    got = json!({
                        "target": frame.target,
                        "held": frame.held.as_str(),
                        "settled": frame.settled,
                        "pad_step": frame.pad_step,
                    });
                    frame.command.into_iter().collect()
                }
                "set_control" => {
                    let on = op["on"].as_bool().expect("on");
                    let commands = b.set_control(&s, enabled, on, belt_allowed);
                    enabled = on;
                    commands
                }
                "direction" => {
                    s.direction = Direction::from_setting(op["value"].as_str().expect("kierunek"));
                    b.forget_target();
                    Vec::new()
                }
                "setting" => {
                    let value = &op["value"];
                    match op["key"].as_str().expect("klucz") {
                        "belt_control" => s.belt_control = value.as_bool().expect("bool"),
                        "pad_idle" => {
                            s.pad_idle = PadIdle::from_setting(value.as_str().expect("napis"))
                        }
                        "pad_steps" => s.pad_steps = parse_pad_steps(value),
                        "walk_speed" => s.walk_speed = f(value),
                        other => panic!("nieznane ustawienie {other}"),
                    }
                    Vec::new()
                }
                "training_level" => {
                    let level = op["value"].as_i64().expect("poziom");
                    set_level(&mut b, &mut s, &mut enabled, level, belt_allowed)
                }
                "training_step" => {
                    let step = op["value"].as_i64().expect("krok");
                    match step_training_level(s.training_level, step) {
                        Some(level) => {
                            set_level(&mut b, &mut s, &mut enabled, level as i64, belt_allowed)
                        }
                        None => Vec::new(),
                    }
                }
                "training_speed" => {
                    let index = op["index"].as_u64().expect("indeks") as usize;
                    s.training_levels[index] = round_tenth(f(&op["value"]));
                    b.forget_target();
                    Vec::new()
                }
                other => panic!("nieznana operacja {other}"),
            };
            seen_commands += commands.len();
            got["commands"] = commands.iter().map(command_json).collect();
            got["enabled"] = json!(enabled);
            got["training_level"] = json!(s.training_level);
            assert!(
                same(&got, want),
                "{name}, operacja {i}: {op}
  jest: {got}
  ma być: {want}"
            );
        }
    }
    assert!(
        seen_commands > 100,
        "za mało poleceń w fixturach: {seen_commands}"
    );
}
