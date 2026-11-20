import json
from pathlib import Path

import pytest

from bieznia import settings
from bieznia.klawisze import RUN_DEFAULT, WALK_DEFAULT, Key
from bieznia.mapping import Control
from bieznia.outputs import keyboard

WALK = Control(speed=3.0, stick=0.2, walk=True, run=False)
RUN = Control(speed=7.0, stick=0.7, walk=True, run=True)
W, SHIFT = WALK_DEFAULT.scan, RUN_DEFAULT.scan


@pytest.fixture
def sent(monkeypatch: pytest.MonkeyPatch) -> list[tuple[int, bool, bool]]:
    events: list[tuple[int, bool, bool]] = []
    monkeypatch.setattr(
        keyboard, "send_scancode", lambda scan, down, ext=False: events.append((scan, down, ext))
    )
    return events


def test_keys_are_sent_only_on_change(sent: list[tuple[int, bool, bool]]) -> None:
    out = keyboard.KeyboardOutput()
    out.apply(WALK)
    out.apply(WALK)
    out.apply(RUN)
    out.apply(Control.idle())
    assert sent == [
        (W, True, False),
        (SHIFT, True, False),
        (W, False, False),
        (SHIFT, False, False),
    ]


def test_custom_keys_arrow_up_and_ctrl(sent: list[tuple[int, bool, bool]]) -> None:
    up = Key(0x48, True, "Strzałka w górę")
    ctrl = Key(0x1D, False, "Ctrl")
    out = keyboard.KeyboardOutput(up, ctrl)
    out.apply(RUN)
    assert sent == [(0x1D, True, False), (0x48, True, True)]  # strzałka jako klawisz rozszerzony
    sent.clear()
    out.release_all()
    assert (0x48, False, True) in sent and (0x1D, False, False) in sent


def test_rejected_key_is_retried_instead_of_crashing(monkeypatch: pytest.MonkeyPatch) -> None:
    calls: list[tuple[int, bool]] = []

    def flaky(scan: int, down: bool, extended: bool = False) -> None:
        calls.append((scan, down))
        if len(calls) == 1:
            raise OSError("ekran blokady")

    monkeypatch.setattr(keyboard, "send_scancode", flaky)
    out = keyboard.KeyboardOutput()
    out.apply(WALK)  # odrzucone
    out.apply(WALK)  # ponowiona próba
    assert calls == [(W, True), (W, True)]


def test_enabled_state_survives_update_restart(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    state = tmp_path / "stan.json"
    settings.save_enabled(True, state)
    assert json.loads(state.read_text(encoding="utf-8")) == {"enabled": True}

    monkeypatch.delenv(settings.RESUME_ENV, raising=False)
    assert settings.load_resumed_enabled(state) is None  # zwykły start: zawsze z konfiguracji

    monkeypatch.setenv(settings.RESUME_ENV, "1")
    assert settings.load_resumed_enabled(state) is True


def test_user_settings_round_trip_and_merge(tmp_path: Path) -> None:
    path = tmp_path / "ustawienia.json"
    saved = {"run_on": 7.0, "output": "keys", "device_name": "KS-X21", "obcy": 1}
    settings.save_settings(saved, path)
    loaded = settings.load_settings(path)
    assert loaded == {"run_on": 7.0, "output": "keys", "device_name": "KS-X21"}
    config = settings.apply_settings(
        {"mapping": {"run_on": 6.3}, "output": {"type": "dry"}}, loaded
    )
    assert config["mapping"]["run_on"] == 7.0
    assert config["output"]["type"] == "keys"
    assert config["ble"]["device_name"] == "KS-X21"


def test_broken_settings_file_falls_back_to_defaults(tmp_path: Path) -> None:
    path = tmp_path / "ustawienia.json"
    path.write_text("{ to nie jest json", encoding="utf-8")
    assert settings.load_settings(path) == {}


def test_held_walk_key_repeats_like_a_real_keyboard(sent: list[tuple[int, bool, bool]]) -> None:
    now = [0.0]
    out = keyboard.KeyboardOutput(clock=lambda: now[0])
    for step in range(20):  # 1 s chodu, klatki co 50 ms
        now[0] = step * 0.05
        out.apply(WALK)
    downs = [e for e in sent if e == (W, True, False)]
    assert len(downs) > 5  # pierwsze wciśnięcie + powtórzenia po ok. 0,35 s
    assert all(e[1] for e in sent)  # w trakcie chodu nic nie jest puszczane
    out.apply(Control.idle())
    assert sent[-1] == (W, False, False)
