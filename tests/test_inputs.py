import ctypes
import json
from pathlib import Path

import pytest

from bieznia.hotkeys import VK_CODES, Hotkeys
from bieznia.recorder import Recorder
from bieznia.sources import Reading, SourceFinished
from bieznia.sources.replay import ReplaySource
from bieznia.sources.simulator import REPORT_EVERY_S, SimulatedTreadmill
from bieznia.winapi import INPUT


class FakeKeyboard:
    def __init__(self) -> None:
        self.down: set[int] = set()

    def __call__(self, vk: int) -> bool:
        return vk in self.down


def test_hotkey_fires_once_per_press() -> None:
    kb = FakeKeyboard()
    hk = Hotkeys({"toggle": "F8"}, key_down=kb)
    assert hk.poll() == set()
    kb.down.add(VK_CODES["F8"])
    assert hk.poll() == {"toggle"}
    assert hk.poll() == set()  # nadal trzymany
    kb.down.clear()
    assert hk.poll() == set()


def test_key_held_at_startup_is_ignored() -> None:
    kb = FakeKeyboard()
    kb.down.add(VK_CODES["F8"])
    hk = Hotkeys({"toggle": "f8"}, key_down=kb)
    assert hk.poll() == set()


def test_unknown_hotkey_is_rejected() -> None:
    with pytest.raises(ValueError):
        Hotkeys({"toggle": "Q"}, key_down=FakeKeyboard())
    with pytest.raises(ValueError):
        Hotkeys({"toggle": 0}, key_down=FakeKeyboard())


def test_hotkey_by_virtual_key_code() -> None:
    kb = FakeKeyboard()
    hk = Hotkeys({"toggle": 0x24, "sim_faster": "F7"}, key_down=kb)  # Home
    kb.down.add(0x24)
    assert hk.poll() == {"toggle"}


def test_simulator_ramps_towards_target() -> None:
    sim = SimulatedTreadmill()
    sim.poll(0.0, {"sim_faster"})
    sim.poll(0.0, {"sim_faster"})
    assert sim.target == 1.0
    reading = sim.poll(REPORT_EVERY_S, set())
    assert reading is not None
    assert 0.0 < reading.speed_kmh < 1.0  # taśma przyspiesza stopniowo
    assert sim.poll(REPORT_EVERY_S + 0.1, set()) is None  # odczyty co 0,75 s


def test_recording_round_trip(tmp_path: Path) -> None:
    rec = Recorder(tmp_path)
    rec.write(Reading(t=100.0, speed_kmh=3.5, steps=10))
    rec.write(Reading(t=100.75, speed_kmh=4.0, steps=12))
    rec.close()

    rows = [json.loads(line) for line in rec.path.read_text(encoding="utf-8").splitlines()]
    assert rows == [
        {"t": 0.0, "speed": 3.5, "steps": 10},
        {"t": 0.75, "speed": 4.0, "steps": 12},
    ]

    replay = ReplaySource(rec.path)
    first = replay.poll(50.0, set())
    assert first is not None and first.speed_kmh == 3.5
    assert replay.poll(50.5, set()) is None
    second = replay.poll(50.8, set())
    assert second is not None and second.speed_kmh == 4.0
    with pytest.raises(SourceFinished):
        replay.poll(51.0, set())


def test_bad_recording_line_is_reported(tmp_path: Path) -> None:
    path = tmp_path / "zle.jsonl"
    path.write_text('{"t": 0, "speed": 1}\n{"t": 1}\n', encoding="utf-8")
    with pytest.raises(ValueError, match=":2:"):
        ReplaySource(path)


def test_input_struct_matches_windows_abi() -> None:
    # SendInput odrzuca wywołanie, jeśli rozmiar struktury się nie zgadza.
    assert ctypes.sizeof(INPUT) == (40 if ctypes.sizeof(ctypes.c_void_p) == 8 else 28)
