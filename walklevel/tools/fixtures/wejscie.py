"""Fixtury modułu wejscie (input.rs) z prawdziwej implementacji w Pythonie.

Uruchamianie z katalogu głównego repozytorium:
    PYTHONPATH=. .venv/Scripts/python.exe walklevel/tools/fixtures/wejscie.py
Wyniki: walklevel/crates/core/tests/fixtures/wejscie/*.json
"""

from __future__ import annotations

import json
import random
from pathlib import Path
from types import SimpleNamespace

from bieznia import klawisze, pad, winapi
from bieznia.hotkeys import Hotkeys
from bieznia.klawisze import RUN_DEFAULT, TOGGLE_DEFAULT, WALK_DEFAULT, Key
from bieznia.mapping import Control
from bieznia.outputs import keyboard

OUT = Path(__file__).resolve().parents[2] / "crates" / "core" / "tests" / "fixtures" / "wejscie"
rng = random.Random(20261126)


def key_json(k: Key) -> dict:
    return {"scan": k.scan, "extended": k.extended, "name": k.name, "vk": k.vk, "mods": list(k.mods)}


def save(name: str, data: object) -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    text = json.dumps(data, ensure_ascii=False, separators=(",", ":"))
    (OUT / f"{name}.json").write_text(text + "\n", encoding="utf-8")


# --- klawisze --------------------------------------------------------------------

MOD_SETS = [
    (),
    ("ctrl",),
    ("alt",),
    ("shift",),
    ("win",),
    ("ctrl", "alt"),
    ("ctrl", "shift"),
    ("alt", "shift"),
    ("shift", "ctrl"),  # kolejność z konstruktora (nieposortowana)
    ("ctrl", "alt", "shift"),
    ("ctrl", "win"),
]


def gen_keys() -> None:
    defaults = {"walk": WALK_DEFAULT, "run": RUN_DEFAULT, "toggle": TOGGLE_DEFAULT}
    inputs = [
        None,
        [],
        "tekst",
        5,
        {},
        {"scan": 0x11},
        {"scan": 0x11, "name": "W"},
        {"scan": 0x48, "extended": True, "name": "Strzałka w górę", "vk": 0x26},
        {"scan": 0x42, "extended": False, "name": "F8", "vk": 0x77, "mods": ["shift", "ctrl"]},
        {"scan": 0x42, "name": "F8", "vk": 0x77, "mods": ["ctrl", "ctrl", "foo", "win"]},
        {"scan": 0x42, "name": "F8", "vk": 0x77, "mods": "ctrl"},
        {"scan": 0x42, "name": "F8", "vk": 0x77, "mods": None},
        {"scan": 0x42, "name": "F8", "vk": 0x77, "mods": 5},
        {"scan": 0x42, "name": "F8", "vk": 0x77, "mods": {"alt": 1}},
        {"scan": 0},
        {"scan": 0x100},
        {"scan": 0xFF, "name": "x"},
        {"scan": -3},
        {"scan": 9999},
        {"scan": "17", "name": "W"},
        {"scan": " 17 ", "name": "W"},
        {"scan": "0x11"},
        {"scan": "abc"},
        {"scan": 17.9, "name": "W"},
        {"scan": True, "name": "jeden"},
        {"scan": None},
        {"scan": 0x11, "vk": "87"},
        {"scan": 0x11, "vk": "zle"},
        {"scan": 0x11, "vk": None},
        {"scan": 0x11, "vk": 0},
        {"scan": 0x11, "vk": 87.5},
        {"scan": 0x11, "name": None},
        {"scan": 0x11, "name": ""},
        {"scan": 0x11, "name": 5},
        {"scan": 0x11, "extended": 1},
        {"scan": 0x11, "extended": 0},
        {"scan": 0x11, "extended": "tak"},
        {"scan": 0x11, "extended": ""},
        {"scan": 0x11, "extended": None},
        {"scan": 0x11, "extended": [0]},
        {"scan": [1]},
        {"scan": {"a": 1}},
    ]
    from_dict = []
    for data in inputs:
        for dname, default in defaults.items():
            from_dict.append(
                {"input": data, "default": dname, "output": key_json(Key.from_dict(data, default))}
            )
    to_dict = []
    for k in (WALK_DEFAULT, RUN_DEFAULT, TOGGLE_DEFAULT, Key(0x42, False, "F8", 0x77, ("shift", "ctrl"))):
        to_dict.append({"key": key_json(k), "dict": k.to_dict(), "label": k.label})

    native = []
    for code in [0, 0x100, 0x11, 0x148, 0x150, 0x14B, 0x14D, 0x1D, 0x11D, 0x38, 0x138, 0x2A, 0x36,
                 0x39, 0x0F, 0x3A, 0x1C, 0x11C, 0x0E, 0x152, 0x153, 0x147, 0x14F, 0x149, 0x151,
                 0x42, 0x12A, 0x211, 0xE048]:
        for fallback in ("", "  ", "Up", " W "):
            r = klawisze.key_from_native(code, fallback, 0x57)
            native.append({"native": code, "fallback": fallback, "vk": 0x57,
                           "key": key_json(r) if r else None})

    problems = []
    for scan in range(0x00, 0x60):
        for extended in (False, True):
            for mods in MOD_SETS:
                toggle = Key(scan, extended, f"K{scan:02X}", 0x70 + (scan % 0x20), mods)
                problems.append({  # bez walk/run = domyślne W i Shift
                    "toggle": key_json(toggle), "label": toggle.label,
                    "problem": klawisze.problem(WALK_DEFAULT, RUN_DEFAULT, toggle),
                })
    pool = [WALK_DEFAULT, RUN_DEFAULT, TOGGLE_DEFAULT, Key(0x48, True, "Strzałka w górę"),
            Key(0x1D, False, "Ctrl"), Key(0x40, False, "F6", 0x75), Key(0x41, False, "F7", 0x76),
            Key(0x58, False, "F12", 0x7B), Key(0x37, True, "Print Screen", 0x2C), Key(0x43, False, "F9", 0), Key(0x43, False, "F9", 0x78),
            Key(0x11, False, "W", 0x57, ("ctrl",)), Key(0x11, False, "Inna nazwa", 0)]
    for walk in pool:
        for run in pool:
            for toggle in pool[::2]:
                problems.append({
                    "walk": key_json(walk), "run": key_json(run), "toggle": key_json(toggle),
                    "label": toggle.label, "problem": klawisze.problem(walk, run, toggle),
                })
    save("klawisze", {"from_dict": from_dict, "to_dict": to_dict, "native": native,
                      "problems": problems})


# --- pad -------------------------------------------------------------------------


def gen_pad() -> None:
    deflection = []
    for x in [-1.0, -0.96, -0.5, -0.1, 0.0, 0.05, 0.0566, 0.08, 0.1, 0.3, 0.515, 0.7, 0.95, 1.0]:
        for y in [-1.0, -0.6, 0.0, 0.03, 0.08, 0.2, 0.515, 0.94, 0.96, 1.0]:
            deflection.append({"x": x, "y": y, "d": pad.PadState("p", x, y).deflection})
    for _ in range(300):
        x, y = rng.uniform(-1, 1), rng.uniform(-1, 1)
        deflection.append({"x": x, "y": y, "d": pad.PadState("p", x, y).deflection})

    steps_inputs = [
        None, "zepsute", 5, [], [[0, 1.0]], [[0, 1.0]] * 6, [[10, 1.0], [50, 2.0]],
        [[0, 1.0], [52, 2.0], [50, 3.0]], [[0, 1.0], [50, 20.0]], [[0, 1], [60, 3]],
        [[0, 1.0], [25, 1.5], [50, 2.5], [100, 4.0]], [[0, 1.0], [4, 2.0]], [[0, 1.0], [5, 2.0]],
        [[0, 1.0], [101, 2.0]], [[0, 0.4], [50, 2.0]], [[0, 0.5], [50, 12.0]],
        [[0, 0.44], [50, 2.0]], [[0, 0.45], [50, 2.0]], [[0, 1.26], [50, 2.04]],
        [[0, 1.25], [50, 2.35]], [[0, 12.04], [50, 2.0]], [[0, 12.05], [50, 2.0]],
        [["0", "1.5"], ["50", "2"]], [[0.9, 1.0], [50.7, 2.0]], [[0, "x"], [50, 2.0]],
        [[0, 1.0, 3], [50, 2.0]], [[0], [50, 2.0]], [[0, None], [50, 2.0]], [[0, True], [50, 2.0]],
        [[0, 1.0], [50, 2.0], [100, 3.0]], [[0, 1.0], [50, 2.0], [100, 3.0], [100, 4.0]],
        {"a": 1},
    ] + [[list(s) for s in steps] for steps in pad.PRESETS.values()]
    parse_steps = [{"input": data, "output": [list(s) for s in pad.parse_steps(data)]}
                   for data in steps_inputs]

    picker = []
    for _ in range(40):
        p = pad.StepPicker()
        seq = []
        for _ in range(40):
            steps = pad.PRESETS[rng.choice([2, 3, 4, 5, 4, 4])]
            d = rng.choice([0.0, 1.0, rng.random(), round(rng.random(), 2), 0.24, 0.25, 0.21, 0.46])
            seq.append({"steps": [list(s) for s in steps], "d": d, "index": p.update(steps, d)})
        picker.append(seq)

    sony = []
    for rid in (0x01, 0x31, 0x11, 0x05, 0x00):
        for length in (0, 1, 2, 3, 4, 5, 8, 10, 11, 12, 20, 21, 64, 78):
            for _ in range(4):
                report = bytes([rid] + [rng.randrange(256) for _ in range(max(0, length - 1))])[:length]
                xy = pad.parse_sony(report)
                sony.append({
                    "report": report.hex(),
                    "xy": list(xy) if xy is not None else None,
                    "buttons_ds": sorted(pad.parse_sony_buttons(report, True)),
                    "buttons_ds4": sorted(pad.parse_sony_buttons(report, False)),
                })

    xinput = []
    for _ in range(200):
        g = SimpleNamespace(wButtons=rng.randrange(0x10000), bLeftTrigger=rng.randrange(256),
                            bRightTrigger=rng.randrange(256))
        xinput.append({"buttons": g.wButtons, "lt": g.bLeftTrigger, "rt": g.bRightTrigger,
                       "pressed": sorted(pad.xinput_buttons(g))})
    for lt in (127, 128, 129, 255, 0):
        g = SimpleNamespace(wButtons=0, bLeftTrigger=lt, bRightTrigger=lt)
        xinput.append({"buttons": 0, "lt": lt, "rt": lt, "pressed": sorted(pad.xinput_buttons(g))})

    labels = []
    names = [*pad.BUTTONS_SONY, "b1", "b32", "b", "bx", "b1x", "nieznany"]
    for pad_name in (None, "DualSense (USB)", "DualShock 4 (Bluetooth)", "Pad PlayStation",
                     "Pad Xbox (XInput 1)", "Gamepad", ""):
        for b in names:
            labels.append({"button": b, "pad": pad_name, "label": pad.button_label(b, pad_name)})

    axes = []
    for value, lo, hi in [(0, 0, 65535), (65535, 0, 65535), (32767, 0, 65535), (5, 10, 10),
                          (5, 10, 2), (100, 0, 50), (12345, 0, 65535)]:
        axes.append({"value": value, "lo": lo, "hi": hi, "axis": pad._axis(value, lo, hi)})

    save("pad", {"deflection": deflection, "parse_steps": parse_steps, "picker": picker,
                 "sony": sony, "xinput": xinput, "labels": labels, "axes": axes,
                 "button_order": list(pad.BUTTON_ORDER)})


# --- wysyłanie i skróty ------------------------------------------------------------


def gen_send() -> None:
    events = []
    for scan in (0x11, 0x48, 0x2A, 0x1D):
        for down in (True, False):
            for ext in (False, True):
                e = winapi.key_input(scan, down, ext)
                events.append({"scan": scan, "down": down, "extended": ext,
                               "wScan": e.u.ki.wScan, "flags": e.u.ki.dwFlags,
                               "extra": e.u.ki.dwExtraInfo, "type": e.type, "vk": e.u.ki.wVk})

    runs = []
    for case in range(25):
        walk, run = rng.choice([(WALK_DEFAULT, RUN_DEFAULT),
                                (Key(0x48, True, "Strzałka w górę"), Key(0x1D, False, "Ctrl"))])
        fail_at = set(rng.sample(range(60), rng.choice([0, 0, 3, 8])))
        sent: list[list] = []
        calls = [0]
        now = [0.0]

        def fake_send(scan: int, down: bool, ext: bool = False) -> None:
            i = calls[0]
            calls[0] += 1
            if i in fail_at:
                raise OSError("odrzucone")
            sent.append([scan, down, ext])

        keyboard.send_scancode = fake_send  # type: ignore[assignment]
        out = keyboard.KeyboardOutput(walk, run, clock=lambda: now[0])
        steps = []
        t = 0.0
        for _ in range(rng.randrange(20, 60)):
            t += rng.choice([0.016, 0.05, 0.05, 0.1, 0.3])
            now[0] = t
            action = rng.choice(["idle", "walk", "walk", "walk", "run", "run", "release"])
            if action == "release":
                out.release_all()
            else:
                ctrl = {"idle": Control.idle(), "walk": Control(3.0, 0.2, True, False),
                        "run": Control(7.0, 0.7, True, True)}[action]
                out.apply(ctrl)
            steps.append({"t": repr(t), "action": action})  # napis: dokładna wartość float
        runs.append({"walk": key_json(walk), "run": key_json(run), "fail_at": sorted(fail_at),
                     "steps": steps, "sent": sent})

    hot = []
    for case in range(15):
        bindings = rng.choice([
            {"toggle": "F8"},
            {"toggle": "f8", "sim_slower": "F6", "sim_faster": "F7"},
            {"toggle": [0x77, ["ctrl", "shift"]], "sim_faster": "F7"},
            {"toggle": [0x77, []]},
            {"toggle": 0x24, "sim_slower": "F6"},
        ])
        py_bindings = {a: (b[0], tuple(b[1])) if isinstance(b, list) else b for a, b in bindings.items()}
        down: set[int] = set()
        if rng.random() < 0.3:
            down.add(0x77)
        polls = []
        start_down = sorted(down)
        hk = Hotkeys(py_bindings, key_down=lambda vk: vk in down)
        for _ in range(30):
            down = set(rng.sample([0x77, 0x75, 0x76, 0x24, 0x10, 0x11, 0x12], rng.randrange(0, 4)))
            polls.append({"down": sorted(down), "pressed": sorted(hk.poll())})
        hot.append({"bindings": bindings, "start_down": start_down, "polls": polls})

    bad = []
    for b in ("Q", "F13", "", "F0", 0, 256, -1, [0, []], [0x100, []]):
        py = (b[0], tuple(b[1])) if isinstance(b, list) else b
        try:
            Hotkeys({"toggle": py}, key_down=lambda vk: False)
            bad.append({"binding": b, "error": None})
        except ValueError as exc:
            bad.append({"binding": b, "error": str(exc)})

    save("wysylanie", {"events": events, "keyboard": runs, "hotkeys": hot, "bad_hotkeys": bad})


if __name__ == "__main__":
    gen_keys()
    gen_pad()
    gen_send()
    print("zapisano", OUT)
