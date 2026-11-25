"""Kontrakt sterownika: te same scenariusze dla każdego modułu (X21, Urevo/FTMS, symulator).

Nowy moduł bieżni dopisuje tu swoją wirtualną bieżnię i musi przejść wszystko bez zmian
w testach. Do tego ślady z prawdziwych bieżni (tests/dane/*-ramki.txt) odtwarzane przez
sterownik - zachowanie na prawdziwych danych nie może się rozjechać przy refaktorze.
"""

from __future__ import annotations

import asyncio
import contextlib
from collections.abc import AsyncIterator, Callable
from dataclasses import dataclass, replace
from pathlib import Path
from typing import Any

import pytest
from test_ble import FakeTreadmill
from test_ftms import FakeUrevo

from bieznia.sterowniki import SimulatorDriver, make_driver, profile
from bieznia.sterowniki import symulator as sim_mod
from bieznia.sterowniki.ftms import protokol as ftms
from bieznia.sterowniki.ftms import sterownik as ftms_driver
from bieznia.sterowniki.kingsmith import protokol as kingsmith
from bieznia.sterowniki.kingsmith import sterownik as ks_driver
from bieznia.sterowniki.kontrakt import Capabilities

DANE = Path(__file__).with_name("dane")
MODULES = ("kingsmith", "ftms", "symulator")


class MovingX21(FakeTreadmill):
    """Wirtualny X21, który naprawdę zmienia prędkość na polecenie (jak w QZ)."""

    def _on_command(self, prefix: str, frame: bytes) -> None:
        text = kingsmith.decode(frame, self.table) or ""
        if text.startswith("props CurrentSpeed"):
            self.received.append(text)
            self.speed = float(text.rsplit(" ", 1)[-1])
            self._reply(prefix, f"props CurrentSpeed {self.speed} runState {int(self.running)}")
            return
        super()._on_command(prefix, frame)


@dataclass
class Harness:
    driver: Any
    readings: list[tuple[float, int | None]]
    pump: Callable[[], None]  # jedna „ramka” z danymi od bieżni


@contextlib.asynccontextmanager
async def harness(kind: str, monkeypatch: pytest.MonkeyPatch) -> AsyncIterator[Harness]:
    readings: list[tuple[float, int | None]] = []

    def sink(speed: float, steps: int | None) -> None:
        readings.append((speed, steps))

    if kind == "kingsmith":
        monkeypatch.setattr(ks_driver, "VERDICT_FRAME_S", 0.05)
        monkeypatch.setattr(ks_driver, "VERDICT_MAX_S", 0.4)
        monkeypatch.setattr(ks_driver, "QUIET_AFTER_PUSH_S", 0.0)
        monkeypatch.setattr(ks_driver, "FRAGMENT_GAP_S", 0.0)
        monkeypatch.setattr(ks_driver, "LINE_GAP_S", 0.0)
        x21 = MovingX21(running=False, speed=0.0)
        driver: Any = make_driver(x21, "KS-HDSY-X21C", sink)
        driver.REPLY_WAIT_S = 0.02
        await driver.start()
        driver.codec.candidates = ["v1"]

        def pump() -> None:
            asyncio.get_running_loop().create_task(driver.poll())

    elif kind == "ftms":
        monkeypatch.setattr(ftms_driver, "FTMS_START_WAIT_S", 0.3)
        monkeypatch.setattr(ftms_driver, "FTMS_STOP_SLOWING_S", 0.2)
        monkeypatch.setattr(ftms_driver, "FTMS_STOP_WAIT_S", 1.0)
        urevo = FakeUrevo()
        driver = make_driver(urevo, "URTM054", sink)  # type: ignore[arg-type]
        await driver.start()
        pump = urevo.frame
    else:
        clock = [0.0]
        caps = replace(sim_mod.CAPS, start_delay_s=0.01)
        driver = SimulatorDriver(sink, clock=lambda: clock[0], capabilities=caps)
        await driver.start()

        def pump() -> None:
            clock[0] += 1.0
            driver.advance()

    stop = asyncio.Event()

    async def frames() -> None:  # bieżnia „żyje” w tle - jak prawdziwa, co chwilę ramka
        while not stop.is_set():
            pump()
            await asyncio.sleep(0.03)

    task = asyncio.create_task(frames())
    try:
        yield Harness(driver, readings, pump)
    finally:
        stop.set()
        await task


async def settle(h: Harness, predicate: Callable[[], bool], seconds: float = 3.0) -> bool:
    loop = asyncio.get_running_loop()
    end = loop.time() + seconds
    while loop.time() < end:
        if predicate():
            return True
        await asyncio.sleep(0.03)
    return predicate()


def speed_of(h: Harness) -> float:
    reading = h.driver.belt_speed()
    return reading[0] if reading else -1.0


@pytest.mark.parametrize("kind", MODULES)
def test_identity_and_capabilities(kind: str, monkeypatch: pytest.MonkeyPatch) -> None:
    async def scenario() -> None:
        async with harness(kind, monkeypatch) as h:
            caps: Capabilities = h.driver.capabilities
            assert 0 < caps.speed_min < caps.speed_max
            assert caps.control
            assert h.driver.info.driver == kind
            assert h.driver.info.model and h.driver.describe()
            assert not h.driver.busy()

    asyncio.run(scenario())


@pytest.mark.parametrize("kind", MODULES)
def test_start_speed_stop(kind: str, monkeypatch: pytest.MonkeyPatch) -> None:
    async def scenario() -> None:
        async with harness(kind, monkeypatch) as h:
            assert await h.driver.start_belt() == "started"
            assert await settle(h, lambda: h.driver.belt_running() is True)
            assert await h.driver.set_speed(2.5)
            assert await settle(h, lambda: abs(speed_of(h) - 2.5) < 0.15), speed_of(h)
            await h.driver.stop_belt()
            assert await settle(h, lambda: h.driver.belt_running() is False)
            assert not h.driver.busy()
            assert h.readings, "sterownik nie oddał żadnego odczytu"

    asyncio.run(scenario())


@pytest.mark.parametrize("kind", MODULES)
def test_speed_stays_within_capabilities(kind: str, monkeypatch: pytest.MonkeyPatch) -> None:
    async def scenario() -> None:
        async with harness(kind, monkeypatch) as h:
            caps: Capabilities = h.driver.capabilities
            await h.driver.start_belt()
            await h.driver.set_speed(99.0)
            assert await settle(h, lambda: speed_of(h) > 0)
            await asyncio.sleep(0.2)
            assert speed_of(h) <= caps.speed_max + 0.05

    asyncio.run(scenario())


@pytest.mark.parametrize("kind", MODULES)
def test_running_belt_start_is_a_no_op(kind: str, monkeypatch: pytest.MonkeyPatch) -> None:
    async def scenario() -> None:
        async with harness(kind, monkeypatch) as h:
            assert await h.driver.start_belt() == "started"
            assert await settle(h, lambda: h.driver.belt_running() is True)
            before = h.driver.counts.get("commands", 0)
            assert await h.driver.start_belt() == "started"
            assert h.driver.counts.get("commands", 0) - before <= 1

    asyncio.run(scenario())


# --- ślady z prawdziwych bieżni ------------------------------------------------------


def load_trace(name: str) -> list[tuple[float, str, bytes]]:
    rows = []
    for line in (DANE / name).read_text(encoding="utf-8").splitlines():
        if line.startswith("#") or not line.strip():
            continue
        t, kind, hx = line.split()
        rows.append((float(t), kind, bytes.fromhex(hx)))
    return rows


def test_urevo_trace_drives_state_like_the_real_session() -> None:
    """Log 5L: start, 1,5 -> 2,5 -> 4,0, pauza, hamowanie 7 s, „zatrzymana”."""
    urevo = FakeUrevo()
    readings: list[tuple[float, int | None]] = []
    driver: Any = make_driver(urevo, "URTM054", lambda s, k: readings.append((s, k)))  # type: ignore[arg-type]
    running: list[bool | None] = []
    for _t, kind, data in load_trace("urevo-5l-ramki.txt"):
        if kind == "data":
            urevo.callbacks.setdefault(ftms.TREADMILL_DATA, driver._on_data)
            driver._on_data(None, bytearray(data))
        elif kind == "status":
            driver._on_status(None, bytearray(data))
        running.append(driver.belt_running())
    speeds = [s for s, _ in readings]
    assert {1.0, 1.5, 2.5, 4.0} <= set(speeds)
    braking = speeds[speeds.index(4.0) :]
    assert braking[-1] == 0.0 and 2.8 in braking  # hamowanie po pauzie widać w ramkach
    assert running[-1] is False  # po „zatrzymana” taśma stoi
    assert True in running


def test_x21_trace_decodes_with_profile_table() -> None:
    """Ślad testera (KS-NACH-X21C) - tablica wykryta z danych, model z profilu."""
    driver: Any = make_driver(FakeTreadmill(), "KS-NACH-X21C", lambda s, k: None)
    assert driver.info.driver == "kingsmith"
    channel = driver.channels[0]
    for line in (DANE / "x21-ramki.txt").read_text(encoding="utf-8").splitlines():
        if line.startswith("#"):
            continue
        _t, hx = line.split()
        driver._on_notify(channel, None, bytearray(bytes.fromhex(hx)))
    assert driver.codec.table == "v5"


# --- profile modeli (dane, nie kod) -------------------------------------------------------


def test_builtin_profiles_are_valid_and_match_models() -> None:
    profiles = {p.id: p for p in profile.all_profiles(reload=True)}
    assert {"kingsmith-x21", "urevo-spacewalk-5l", "kingsmith"} <= set(profiles)
    assert profile.profile_for("URTM054") == profiles["urevo-spacewalk-5l"]
    assert profile.profile_for("URTM061").id == "urevo"  # type: ignore[union-attr]
    assert profile.profile_for("KS-HDSY-X21C").id == "kingsmith-x21"  # type: ignore[union-attr]
    assert profile.profile_for("KS-NACH-MXG").option("table") == "v7"  # type: ignore[union-attr]
    assert profile.profile_for("Mi Band") is None


@pytest.mark.parametrize(
    "bad",
    [
        {"format": 2, "id": "a", "name": "A", "driver": "ftms"},
        {"format": 1, "id": "a b", "name": "A", "driver": "ftms"},
        {"format": 1, "id": "a", "name": "A", "driver": "python"},
        {"format": 1, "id": "a", "name": "A", "driver": "ftms", "capabilities": {"exec": 1}},
        {"format": 1, "id": "a", "name": "A", "driver": "ftms", "capabilities": {"speed_max": 99}},
        {"format": 1, "id": "a", "name": "A", "driver": "ftms", "options": {"x": [1]}},
        {
            "format": 1,
            "id": "a",
            "name": "A",
            "driver": "ftms",
            "capabilities": {"speed_min": 5, "speed_max": 2},
        },
    ],
)
def test_bad_profiles_are_rejected(bad: dict[str, Any]) -> None:
    with pytest.raises(profile.ProfileError):
        profile.parse_profile(bad)


def test_user_profile_overrides_builtin(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    folder = tmp_path / "profile"
    folder.mkdir()
    (folder / "moj.json").write_text(
        '{"format": 1, "id": "urevo-spacewalk-5l", "name": "Mój 5L", "driver": "ftms",'
        ' "match": ["URTM054"], "capabilities": {"speed_max": 4.0}}',
        encoding="utf-8",
    )
    (folder / "zepsuty.json").write_text("{nie json", encoding="utf-8")
    monkeypatch.setattr(profile, "USER_DIR", folder)
    try:
        found = profile.profile_for("URTM054")
        found = profile.all_profiles(reload=True) and profile.profile_for("URTM054")
        assert found is not None and found.name == "Mój 5L" and found.source == "dodatkowy"
    finally:
        monkeypatch.undo()
        profile.all_profiles(reload=True)
