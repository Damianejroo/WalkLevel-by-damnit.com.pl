import json
from pathlib import Path

import pytest

from bieznia import app
from bieznia.mapping import Control
from bieznia.outputs import keyboard
from bieznia.winapi import SCAN_LSHIFT, SCAN_W

WALK = Control(speed=3.0, stick=0.2, walk=True, run=False)
RUN = Control(speed=7.0, stick=0.7, walk=True, run=True)


@pytest.fixture
def sent(monkeypatch: pytest.MonkeyPatch) -> list[tuple[int, bool]]:
    events: list[tuple[int, bool]] = []
    monkeypatch.setattr(keyboard, "send_scancode", lambda scan, down: events.append((scan, down)))
    monkeypatch.setattr(keyboard, "release_movement_keys", lambda: None)
    return events


def test_keys_are_sent_only_on_change(sent: list[tuple[int, bool]]) -> None:
    out = keyboard.KeyboardOutput()
    out.apply(WALK)
    out.apply(WALK)
    out.apply(RUN)
    out.apply(Control.idle())
    assert sent == [
        (SCAN_W, True),
        (SCAN_LSHIFT, True),
        (SCAN_W, False),
        (SCAN_LSHIFT, False),
    ]


def test_rejected_key_is_retried_instead_of_crashing(monkeypatch: pytest.MonkeyPatch) -> None:
    calls: list[tuple[int, bool]] = []

    def flaky(scan: int, down: bool) -> None:
        calls.append((scan, down))
        if len(calls) == 1:
            raise OSError("ekran blokady")

    monkeypatch.setattr(keyboard, "send_scancode", flaky)
    out = keyboard.KeyboardOutput()
    out.apply(WALK)  # odrzucone
    out.apply(WALK)  # ponowiona próba
    assert calls == [(SCAN_W, True), (SCAN_W, True)]


def test_enabled_state_survives_update_restart(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(app, "STATE_FILE", tmp_path / "stan.json")
    app.save_state(True)
    assert json.loads((tmp_path / "stan.json").read_text(encoding="utf-8")) == {"enabled": True}

    monkeypatch.delenv(app.RESUME_ENV, raising=False)
    assert app.load_resumed_state() is None  # zwykły start: zawsze z konfiguracji

    monkeypatch.setenv(app.RESUME_ENV, "1")
    assert app.load_resumed_state() is True
