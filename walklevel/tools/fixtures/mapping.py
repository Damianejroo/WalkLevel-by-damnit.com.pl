"""Fixtury dla walklevel-core::mapping z prawdziwej implementacji w Pythonie (0.5.x).

Uruchom z katalogu głównego repozytorium (PYTHONPATH=. -> import bieznia):
    .venv/Scripts/python.exe walklevel/tools/fixtures/mapping.py

Zapisuje JSON w walklevel/crates/core/tests/fixtures/mapping/. Okno aplikacji nie startuje -
metody ekranu Sterowanie są wołane na atrapie bez Qt.
"""

from __future__ import annotations

import json
import logging
import random
from dataclasses import asdict
from pathlib import Path
from types import SimpleNamespace
from typing import Any

from bieznia import pad
from bieznia.engine import Bridge
from bieznia.gui import app as gui_app
from bieznia.gui.ekrany import sterowanie
from bieznia.klawisze import RUN_DEFAULT, WALK_DEFAULT
from bieznia.mapping import MappingConfig, SpeedMapper
from bieznia.sources import Reading
from bieznia.winapi import vk_from_scan

OUT = Path(__file__).resolve().parents[2] / "crates" / "core" / "tests" / "fixtures" / "mapping"


def save(name: str, data: Any) -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    path = OUT / f"{name}.json"
    path.write_text(json.dumps(data, ensure_ascii=False, separators=(",", ":")) + "\n", encoding="utf-8")
    print(f"{path.name}: {len(data)}")


def exact(obj: Any) -> Any:
    """Wyniki z ułamkami jako napisy "f64:<repr>": test Rust czyta je dokładnie (str::parse)
    i porównuje bit w bit - serde_json bez float_roundtrip potrafi pomylić się o 1 ULP."""
    if isinstance(obj, float):
        return f"f64:{obj!r}"
    if isinstance(obj, dict):
        return {k: exact(v) for k, v in obj.items()}
    if isinstance(obj, (list, tuple)):
        return [exact(v) for v in obj]
    return obj


def control(c: Any) -> dict[str, Any]:
    return {"speed": c.speed, "stick": c.stick, "walk": c.walk, "run": c.run}


# --- SpeedMapper ---------------------------------------------------------------------------


def heartbeat(frames: list[tuple[float, float]]) -> list[list[float]]:
    """Ramki co ~2 s i powtórzenie ostatniej prędkości co 0,75 s (jak w tests/test_mapping)."""
    out: list[list[float]] = []
    for (t, v), nxt in zip(
        frames, [*frames[1:], (frames[-1][0] + 2.0, frames[-1][1])], strict=True
    ):
        hb = t
        while hb < nxt[0]:
            out.append([v, hb])
            hb = round(hb + 0.75, 6)  # krótkie liczby: serde_json czyta je dokładnie
    return out


def random_inputs(rng: random.Random, n: int, rounded: bool) -> list[list[float]]:
    out: list[list[float]] = []
    t, v = round(rng.uniform(0, 100), 3), 0.0
    for _ in range(n):
        r = rng.random()
        if r < 0.08:
            v = 0.0  # stop
        elif r < 0.12:
            v = -rng.uniform(0, 2)  # błędny odczyt
        elif r < 0.25:
            v = max(0.0, v - rng.uniform(0.6, 1.5))  # hamowanie
        else:
            v = min(14.0, max(0.0, v + rng.uniform(-0.4, 0.6)))
        speed = round(v, 1) if rounded else round(v, 6)
        t = round(t + rng.choice([0.05, 0.25, 0.75, 1.0, 2.0, 2.0, 3.5, 4.0]), 6)
        out.append([speed, t])
    return out


def mapper_cases() -> list[dict[str, Any]]:
    stops = {
        "stop_1": [(44.6, 2.3), (46.6, 2.2), (48.6, 1.2), (50.6, 1.0), (53.0, 0.0), (55.0, 0.0)],
        "stop_2": [
            (12.8, 3.6), (14.8, 2.7), (16.8, 1.8), (18.8, 1.6), (20.7, 1.4), (22.7, 1.0), (25.5, 0.0)
        ],
        "stop_3": [(0.0, 1.7), (2.0, 1.7), (4.0, 1.7), (6.1, 1.0), (8.0, 0.0), (10.0, 0.0)],
        "stop_slow": [(0.0, 1.7), (2.0, 1.7), (4.0, 1.5), (6.0, 1.0), (8.0, 0.0), (10.0, 0.0)],
        "stop_ramp": [(0.0, 2.0), (2.0, 2.0), (4.0, 1.9), (6.0, 1.0), (8.0, 0.0), (10.0, 0.0)],
        "remote_slowdown": [(0.0, 3.0), (2.0, 2.8), (4.0, 2.5), (6.0, 2.5), (8.0, 2.5)],
    }
    cases: list[dict[str, Any]] = []
    for name, frames in stops.items():
        for quick in (False, True):
            cases.append(
                {
                    "name": f"{name}_quick_{quick}",
                    "config": {
                        "stand_below": 1.0, "run_on": 4.0, "run_off": 3.4, "quick_stop": quick
                    },
                    "inputs": heartbeat(frames),
                }
            )
    t = 0.0
    hyst = []
    for v in [0.0, 0.5, 3.0, 6.0, 6.3, 5.8, 5.6, 6.0, 6.3, 8.0, 0.0, 6.0, 12.0, 20.0, -5.0]:
        t += 2.0
        hyst.append([v, t])
    cases.append({"name": "hysteresis", "config": {}, "inputs": hyst})
    cases.append(
        {
            "name": "full_x21_range",
            "config": {"stand_below": 0.0, "run_on": 12.0, "run_off": 11.4, "max_speed": 12.0},
            "inputs": [[0.0, 0.0], [0.5, 1.0], [12.0, 2.0], [12.0, 3.0]],
        }
    )
    cases.append(
        {
            "name": "slowdown_resumes",
            "config": {"stand_below": 1.0, "run_on": 6.3, "run_off": 5.7},
            "inputs": [[5.0, 0.0], [3.5, 2.0], [3.5, 4.0], [3.5, 5.5], [4.0, 6.0]],
        }
    )
    cases.append(
        {
            "name": "remote_half_kmh",
            "config": {"stand_below": 1.0, "run_on": 4.0, "run_off": 3.4},
            "inputs": [[5.0, 0.0], [4.5, 2.0], [4.0, 4.0], [3.5, 6.0]],
        }
    )
    cases.append(
        {
            "name": "smoothing_3",
            "config": {"smoothing_samples": 3, "quick_stop": False},
            "inputs": [[3.0, 0.0], [6.0, 1.0], [9.0, 2.0], [9.0, 3.0], [0.1, 4.0], [0.7, 5.0]],
        }
    )
    rng = random.Random(20261126)
    configs = [
        {},
        {"quick_stop": False},
        {"stand_below": 0.0, "run_on": 12.0, "run_off": 11.4},
        {"stand_below": 1.0, "run_on": 4.0, "run_off": 3.4, "brake_hold_s": 2.0},
        {"smoothing_samples": 3},
        {"smoothing_samples": 5, "quick_stop": False, "max_speed": 8.0},
        {"smoothing_samples": 4, "brake_drop": 0.3, "brake_near": 1.2},
        {"stand_below": 1.0, "run_on": 3.0, "run_off": 2.4, "quick_stop": False},
        {"stand_below": 0.5, "run_on": 2.0, "run_off": 1.5, "smoothing_samples": 2},
    ]
    for i, cfg in enumerate(configs):
        for rounded in (True, False):
            cases.append(
                {
                    "name": f"random_{i}_rounded_{rounded}",
                    "config": cfg,
                    "inputs": random_inputs(rng, 300, rounded),
                }
            )
    for case in cases:
        m = SpeedMapper(MappingConfig.from_dict(case["config"]))
        outputs = []
        for speed, now in case["inputs"]:
            c = m.update(speed, now)
            outputs.append({**control(c), "braking": m.braking})
            if case["name"].startswith("random_") and len(outputs) == 150:
                m.reset()  # reset w połowie: okno i hamowanie od nowa
                outputs[-1]["reset_after"] = True
        case["outputs"] = exact(outputs)
    return cases


def config_cases() -> list[dict[str, Any]]:
    inputs: list[dict[str, Any]] = [
        {},
        {"run_on": 7.0, "foo": 1},
        {"stand_below": 0, "run_on": 12, "run_off": 11.4},
        {"run_on": 6.3, "run_off": 6.3},
        {"run_on": 12.0, "max_speed": 12.0},
        {"quick_stop": False, "brake_drop": 0.4, "brake_near": 0.2, "brake_hold_s": 1.5},
        {"smoothing_samples": 3, "stale_after_s": 5.0},
        {"run_on": 5.0, "run_off": 5.5},
        {"stand_below": 7.0},
        {"max_speed": 6.0},
        {"smoothing_samples": 0},
        {"stand_below": -0.1},
        {"stand_below": 5.7},
        {"stand_below": "x"},
    ]
    out = []
    for data in inputs:
        try:
            cfg = MappingConfig.from_dict(data)
        except (ValueError, TypeError):
            out.append({"input": data, "ok": False})
        else:
            out.append({"input": data, "ok": True, "config": asdict(cfg)})
    return out


# --- Bridge (silnik) -----------------------------------------------------------------------


class ScriptSource:
    def __init__(self) -> None:
        self.next: Reading | None = None

    def poll(self, now: float, actions: set[str]) -> Reading | None:
        r, self.next = self.next, None
        return r

    def close(self) -> None:
        pass


class SpyOutput:
    def __init__(self) -> None:
        self.events: list[list[Any]] = []

    def apply(self, c: Any) -> None:
        self.events.append(["apply", c.walk, c.run])

    def release_all(self) -> None:
        self.events.append(["release"])


MAPPINGS: list[dict[str, Any]] = [
    {},
    {"stand_below": 1.0, "run_on": 4.0, "run_off": 3.4},
    {"smoothing_samples": 2, "quick_stop": False},
]


def bridge_ticks_random(rng: random.Random) -> list[dict[str, Any]]:
    ticks: list[dict[str, Any]] = []
    now, v, steps = round(rng.uniform(0, 50), 3), 0.0, 0
    next_reading = now
    for _ in range(800):
        now = round(now + rng.choice([0.05, 0.05, 0.05, 0.1, 0.3, 1.7]), 6)
        reading = None
        if now >= next_reading and rng.random() > 0.01:
            r = rng.random()
            if r < 0.05:
                v = 0.0
            elif r < 0.1:
                v = max(0.0, v - 1.0)
            else:
                v = min(12.0, max(0.0, v + rng.uniform(-0.3, 0.5)))
            steps += int(v * rng.uniform(0.5, 1.5))
            if rng.random() < 0.01:
                steps = 0  # licznik bieżni od nowa
            s: int | None = None if rng.random() < 0.05 else steps
            reading = {"t": round(now - rng.choice([0.0, 0.01, 0.2]), 6), "speed_kmh": round(v, 1),
                       "steps": s}
            next_reading = now + rng.choice([0.75, 0.75, 2.0, 4.5])
        tick: dict[str, Any] = {"now": now, "toggle": rng.random() < 0.01, "reading": reading}
        r = rng.random()
        if r < 0.005:
            tick["new_source"] = True  # set_source: postać staje, dane nieświeże
        elif r < 0.01:
            tick["mapping"] = rng.choice(MAPPINGS)  # set_mapping: mapowanie od nowa
        ticks.append(tick)
    return ticks


def bridge_cases() -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []
    cadence = [
        {"now": i * 0.25, "toggle": False,
         "reading": {"t": i * 0.25, "speed_kmh": 6.0, "steps": i // 2}}
        for i in range(41)
    ]
    cases.append({"name": "distance_cadence", "enabled": True, "config": {}, "ticks": cadence})
    stale = [
        {"now": 0.0, "toggle": False, "reading": {"t": 0.0, "speed_kmh": 4.0, "steps": None}},
        {"now": 1.0, "toggle": False, "reading": None},
        {"now": 4.0, "toggle": False, "reading": None},
        {"now": 4.5, "toggle": False, "reading": None},
        {"now": 5.0, "toggle": False, "reading": {"t": 5.0, "speed_kmh": 4.0, "steps": 10}},
        {"now": 5.1, "toggle": True, "reading": None},
        {"now": 5.2, "toggle": False, "reading": {"t": 5.2, "speed_kmh": 7.0, "steps": 11}},
        {"now": 5.3, "toggle": True, "reading": {"t": 5.3, "speed_kmh": 7.0, "steps": 12}},
    ]
    cases.append({"name": "stale_and_toggle", "enabled": True, "config": {}, "ticks": stale})
    cases.append(
        {"name": "disabled", "enabled": False, "config": {}, "ticks": stale[:5]}
    )
    history = [{"now": i * 0.5, "toggle": False, "reading": None} for i in range(200)]
    cases.append({"name": "history_minute", "enabled": True, "config": {}, "ticks": history})
    rng = random.Random(777)
    for i, cfg in enumerate([{}, {"stand_below": 1.0, "run_on": 4.0, "run_off": 3.4},
                             {"smoothing_samples": 3, "stale_after_s": 2.0}]):
        cases.append({"name": f"random_{i}", "enabled": i != 1, "config": cfg,
                      "ticks": bridge_ticks_random(rng)})
    for case in cases:
        bridge = Bridge(MappingConfig.from_dict(case["config"]), "dry", case["enabled"])
        spy = SpyOutput()
        bridge.output = spy
        source = ScriptSource()
        bridge.set_source(source)
        outputs = []
        for tick in case["ticks"]:
            if tick.get("new_source"):
                bridge.set_source(source)
            if "mapping" in tick:
                bridge.set_mapping(MappingConfig.from_dict(tick["mapping"]))
            r = tick["reading"]
            source.next = None if r is None else Reading(r["t"], r["speed_kmh"], r["steps"])
            spy.events = []
            st = bridge.tick(tick["now"], {"toggle"} if tick["toggle"] else frozenset())
            hist = list(st.history)
            outputs.append(
                {
                    "enabled": st.enabled,
                    "control": control(st.control),
                    "steps": st.steps,
                    "cadence": st.cadence,
                    "live": st.live,
                    "distance_km": st.distance_km,
                    "active_s": st.active_s,
                    "history_len": len(hist),
                    "history_first": list(hist[0]) if hist else None,
                    "history_last": list(hist[-1]) if hist else None,
                    "events": spy.events,
                }
            )
        case["outputs"] = exact(outputs)
    return cases


# --- pad: skoki i wybór skoku --------------------------------------------------------------


def pad_steps_cases() -> list[dict[str, Any]]:
    inputs: list[Any] = [
        [[0, 1.0], [25, 1.5], [50, 2.5], [100, 4.0]],
        [[0, 1], [60, 3]],
        [[0, 1.04], [30, 2.25], [100, 12.0]],
        [[0, 0.5], [5, 0.55], [10, 0.6], [15, 0.65], [100, 11.96]],
        [[0.9, 1.0], [50.7, 2.0]],
        [["0", "1.5"], ["40", "2"]],
        [[0, True], [50, 2.0]],
        None,
        [[0, 1.0]],
        [[0, 1.0]] * 6,
        [[10, 1.0], [50, 2.0]],
        [[0, 1.0], [52, 2.0], [50, 3.0]],
        [[0, 1.0], [4, 2.0]],
        [[0, 1.0], [101, 2.0]],
        [[0, 1.0], [50, 20.0]],
        [[0, 0.4], [50, 2.0]],
        [[0, 1.0], [50]],
        [[0, 1.0], [50, "x"]],
        "zepsute",
        42,
        *[list(map(list, v)) for v in pad.PRESETS.values()],
    ]
    return [{"input": data, "output": [list(s) for s in pad.parse_steps(data)]} for data in inputs]


def step_picker_cases() -> list[dict[str, Any]]:
    rng = random.Random(4242)
    cases = []
    seq: list[list[Any]] = []
    for d in [0.0, 0.24, 0.30, 0.55, 1.0, 0.98, 0.90, 0.48, 0.40, 0.0, 1.0]:
        seq.append([4, d])
    seq.append([2, 1.0])
    cases.append({"name": "test_pad", "inputs": seq})
    for n in range(3):
        seq = []
        d = 0.0
        for _ in range(400):
            d = round(min(1.0, max(0.0, d + rng.uniform(-0.15, 0.15))), 4)
            count = rng.choice([4, 4, 4, 2, 3, 5]) if rng.random() < 0.03 else None
            seq.append([count or (seq[-1][0] if seq else 4), d])
        cases.append({"name": f"random_{n}", "inputs": seq})
    for case in cases:
        picker = pad.StepPicker()
        case["outputs"] = [picker.update(pad.PRESETS[c], d) for c, d in case["inputs"]]
    return cases


def training_levels_cases() -> list[dict[str, Any]]:
    inputs: list[Any] = [
        [1.5, 2.5, 3.5, 4.5],
        [1, 2, 3, 4],
        [0.5, 20.0, 0.55, 19.96],
        [1.25, 2.35, 3.45, 0.15],
        ["1.5", "2", "3", "4.04"],
        [True, 2, 3, 4],
        [0.4, 2, 3, 4],
        [1, 2, 3, 20.1],
        [1, 2, 3],
        [1, 2, 3, 4, 5],
        [1, 2, "x", 4],
        [1, 2, None, 4],
        None,
        7,
    ]
    return [{"input": d, "output": gui_app.training_levels(d)} for d in inputs]


# --- sterowanie taśmą z klawiszy, pada i treningu ------------------------------------------


class Worker:
    def __init__(self) -> None:
        self.calls: list[dict[str, Any]] = []

    def belt(self, command: str) -> None:
        self.calls.append({"belt": command})

    def set_speed(self, kmh: float, start_if_stopped: bool = False) -> None:
        self.calls.append({"speed": kmh, "start_if_stopped": start_if_stopped})


class FakeApp(sterowanie.ControlScreen):
    """Atrapa okna: tylko to, czego używają metody ekranu Sterowanie."""

    def __init__(self, settings: dict[str, Any], caps_control: bool) -> None:
        self.settings = settings
        self.caps = SimpleNamespace(control=caps_control, start_delay_s=1.0)
        self.worker = Worker()
        state = SimpleNamespace(enabled=False)
        self.bridge = SimpleNamespace(
            state=state,
            keys=(WALK_DEFAULT, RUN_DEFAULT),
            set_enabled=lambda on: setattr(state, "enabled", on),
        )
        self.enable_switch = SimpleNamespace(set_silently=lambda on: None)
        self.belt_target = None
        self.pad_picker = pad.StepPicker()
        self.pad_step = 0
        self.pad_step_since = 0.0
        self.pad_state = None
        self.down: set[int] = set()
        self.key_down = lambda vk: vk in self.down

    def render_training(self) -> None:
        pass

    def render_output_hint(self, problem_text: str = "") -> None:
        pass

    def render_zones(self) -> None:
        pass

    def change(self, key: str, value: Any) -> None:
        self.settings[key] = value


WALK_VK = WALK_DEFAULT.vk or vk_from_scan(WALK_DEFAULT.scan, WALK_DEFAULT.extended)
RUN_VK = RUN_DEFAULT.vk or vk_from_scan(RUN_DEFAULT.scan, RUN_DEFAULT.extended)


def frame(app: FakeApp, op: dict[str, Any]) -> dict[str, Any]:
    """Fragment App.tick (gui/app.py) od wyboru celu do wysłania prędkości - 1:1."""
    now, capturing = op["now"], op["capturing"]
    app.down = ({WALK_VK} if op["walk"] else set()) | ({RUN_VK} if op["run"] else set())
    d = op["deflection"]
    app.pad_state = None if d is None else SimpleNamespace(deflection=d)
    enabled = app.bridge.state.enabled
    direction = app.settings["direction"]
    settled = True
    if direction == "gamepad":
        target, held, _caption, settled = app.pad_target(now)
    elif direction == "keyboard":
        target, held = app.keyboard_target()
    elif direction == "training":
        target, held = app.training_target()
    else:
        target, held = 0.0, "idle"
    sends = direction != "treadmill" and enabled and settled and not capturing
    if sends and direction == "gamepad":
        app.send_pad(target)
    elif sends and target != app.belt_target:
        app.belt_target = target
        app.worker.set_speed(target)
    return {"target": float(target), "held": held, "settled": settled, "pad_step": app.pad_step}


def run_ops(app: FakeApp, ops: list[dict[str, Any]]) -> list[dict[str, Any]]:
    out = []
    for op in ops:
        before = len(app.worker.calls)
        result: dict[str, Any] = {}
        kind = op["op"]
        if kind == "frame":
            result = frame(app, op)
        elif kind == "set_control":
            app.set_control(op["on"])
        elif kind == "direction":  # on_direction -> apply_direction
            app.settings["direction"] = op["value"]
            app.belt_target = None
        elif kind == "setting":
            app.settings[op["key"]] = op["value"]
        elif kind == "training_level":
            app.set_training_level(op["value"])
        elif kind == "training_step":
            app.change_training_level(op["value"])
        elif kind == "training_speed":
            app.on_training_speed(op["index"], op["value"])
        else:
            raise ValueError(kind)
        result["commands"] = app.worker.calls[before:]
        result["enabled"] = app.bridge.state.enabled
        result["training_level"] = app.settings["training_level"]
        out.append(result)
    return out


def base_settings(**kw: Any) -> dict[str, Any]:
    s = {
        "direction": "keyboard",
        "belt_control": True,
        "idle_speed": 1.0,
        "walk_speed": 1.5,
        "run_speed": 2.5,
        "pad_steps": list(pad.PRESETS[4]),
        "pad_idle": "stop",
        "training_levels": list(gui_app.TRAINING_LEVELS),
        "training_level": 0,
    }
    s.update(kw)
    return s


def f(now: float, walk: bool = False, run: bool = False, d: float | None = None,
      capturing: bool = False) -> dict[str, Any]:
    return {"op": "frame", "now": now, "walk": walk, "run": run, "deflection": d,
            "capturing": capturing}


def random_ops(rng: random.Random, n: int) -> list[dict[str, Any]]:
    ops: list[dict[str, Any]] = []
    now, d = 10.0, 0.0
    for _ in range(n):
        r = rng.random()
        if r < 0.03:
            ops.append({"op": "set_control", "on": rng.random() < 0.6})
        elif r < 0.05:
            ops.append({"op": "direction",
                        "value": rng.choice(["keyboard", "gamepad", "treadmill", "training"])})
        elif r < 0.06:
            ops.append({"op": "setting", "key": "pad_idle",
                        "value": rng.choice(["stop", "slow", "zero"])})
        elif r < 0.065:
            ops.append({"op": "setting", "key": "belt_control", "value": rng.random() < 0.7})
        elif r < 0.07:
            ops.append({"op": "setting", "key": "pad_steps",
                        "value": list(pad.PRESETS[rng.choice([2, 3, 4, 5])])})
        elif r < 0.08:
            ops.append({"op": "training_step", "value": rng.choice([-1, 1])})
        elif r < 0.085:
            ops.append({"op": "training_level", "value": rng.randint(-1, 4)})
        elif r < 0.09:
            ops.append({"op": "training_speed", "index": rng.randint(0, 3),
                        "value": round(rng.uniform(0.5, 8.0), 4)})
        else:
            now = round(now + rng.choice([0.05, 0.05, 0.05, 0.1, 0.2, 0.5]), 6)
            d = round(min(1.0, max(0.0, d + rng.uniform(-0.3, 0.3))), 4)
            if rng.random() < 0.1:
                d = 0.0
            ops.append(f(now, rng.random() < 0.5, rng.random() < 0.3,
                         None if rng.random() < 0.05 else d, rng.random() < 0.03))
    return ops


def steering_cases() -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = [
        {
            "name": "keyboard_held_keys",
            "settings": base_settings(),
            "caps_control": True,
            "ops": [
                f(0.0), {"op": "set_control", "on": True}, f(0.05), f(0.1),
                f(0.15, walk=True), f(0.2, walk=True, run=True), f(0.25, run=True),
                {"op": "setting", "key": "walk_speed", "value": 1.6}, f(0.3, walk=True),
                f(0.35, walk=True, capturing=True), {"op": "set_control", "on": False},
            ],
        },
        {
            "name": "keyboard_without_belt_control",
            "settings": base_settings(belt_control=False),
            "caps_control": True,
            "ops": [{"op": "set_control", "on": True}, f(0.0, walk=True),
                    {"op": "set_control", "on": False}, {"op": "set_control", "on": False}],
        },
        {
            "name": "treadmill_never_sets_speed",
            "settings": base_settings(direction="treadmill"),
            "caps_control": True,
            "ops": [{"op": "set_control", "on": True}, f(0.0, walk=True, run=True),
                    {"op": "direction", "value": "keyboard"}, f(0.05, walk=True, run=True),
                    {"op": "set_control", "on": False}],
        },
        {
            "name": "pad_slow",
            "settings": base_settings(direction="gamepad", pad_idle="slow"),
            "caps_control": True,
            "ops": [{"op": "set_control", "on": True}, f(100.0, d=0.0), f(100.5, d=0.0),
                    f(101.0, d=0.37), f(101.5, d=0.37), f(102.0, d=1.0), f(102.05, d=1.0),
                    f(102.6, d=1.0), {"op": "set_control", "on": False}],
        },
        {
            "name": "pad_stop_wakes",
            "settings": base_settings(direction="gamepad"),
            "caps_control": True,
            "ops": [{"op": "set_control", "on": True}, f(100.0, d=0.0), f(100.5, d=0.0),
                    f(101.0, d=0.0), f(101.5, d=0.0), f(102.0, d=0.37), f(102.5, d=0.37),
                    f(102.6, d=0.0), f(102.7, d=0.0), f(103.2, d=0.0),
                    {"op": "set_control", "on": False}],
        },
        {
            "name": "pad_zero",
            "settings": base_settings(direction="gamepad", pad_idle="zero", belt_control=False),
            "caps_control": True,
            "ops": [{"op": "set_control", "on": True}, f(100.0), f(100.5),
                    f(101.0, d=0.6), f(101.5, d=0.6), {"op": "set_control", "on": False}],
        },
        {
            "name": "training_levels",
            "settings": base_settings(direction="training"),
            "caps_control": False,
            "ops": [{"op": "training_level", "value": 1}, f(0.0), f(0.05),
                    {"op": "training_step", "value": 1}, f(0.1),
                    {"op": "training_step", "value": 1}, {"op": "training_step", "value": 1},
                    f(0.15), {"op": "training_speed", "index": 3, "value": 5.04}, f(0.2),
                    {"op": "set_control", "on": False}, {"op": "training_level", "value": 9},
                    f(0.25)],
        },
    ]
    rng = random.Random(99)
    for i, direction in enumerate(["keyboard", "gamepad", "training", "gamepad", "treadmill"]):
        cases.append(
            {
                "name": f"random_{i}",
                "settings": base_settings(direction=direction, pad_idle=["stop", "slow"][i % 2]),
                "caps_control": i != 3,
                "ops": random_ops(rng, 600),
            }
        )
    sterowanie.save_enabled = lambda on: None  # atrapa nie zapisuje stanu na dysku
    for case in cases:
        app = FakeApp(dict(case["settings"]), case["caps_control"])
        case["outputs"] = exact(run_ops(app, case["ops"]))
    return cases


def main() -> None:
    logging.disable(logging.CRITICAL)  # bez ostrzeżeń silnika na konsoli
    save("mapper", mapper_cases())
    save("config", config_cases())
    save("bridge", bridge_cases())
    save("pad_steps", pad_steps_cases())
    save("step_picker", step_picker_cases())
    save("training_levels", training_levels_cases())
    save("steering", steering_cases())


if __name__ == "__main__":
    main()
