"""Pad: skoki wychylenia gałki, martwe strefy i wykrywanie pada (bez prawdziwego pada)."""

from __future__ import annotations

import pytest

from bieznia import pad
from bieznia.pad import PRESETS, PadReader, PadState, StepPicker, parse_steps


def test_deflection_has_dead_zones_and_works_in_any_direction() -> None:
    assert PadState("p", 0.05, 0.03).deflection == 0.0  # dryf gałki w spoczynku
    assert PadState("p", 0.0, 0.96).deflection == 1.0  # prawie do końca = 100%
    assert PadState("p", 0.0, -0.96).deflection == 1.0  # do tyłu też idziesz
    half = PadState("p", 0.0, (pad.INNER_DEADZONE + pad.OUTER_DEADZONE) / 2).deflection
    assert half == pytest.approx(0.5)


def test_saved_steps_are_validated() -> None:
    assert parse_steps([[0, 1.0], [25, 1.5], [50, 2.5], [100, 4.0]]) == PRESETS[4]
    assert parse_steps([[0, 1], [60, 3]]) == [(0, 1.0), (60, 3.0)]
    for bad in (
        None,
        [[0, 1.0]],  # za mało skoków
        [[0, 1.0]] * 6,  # za dużo
        [[10, 1.0], [50, 2.0]],  # pierwszy musi być od 0%
        [[0, 1.0], [52, 2.0], [50, 3.0]],  # nie rosną
        [[0, 1.0], [50, 20.0]],  # prędkość poza zakresem
        "zepsute",
    ):
        assert parse_steps(bad) == PRESETS[4], bad
    assert all(parse_steps(steps) == steps for steps in PRESETS.values())


def test_step_picker_follows_stick_with_hysteresis_on_the_way_back() -> None:
    steps = PRESETS[4]  # 0 / 25 / 50 / 100 %
    picker = StepPicker()
    assert picker.update(steps, 0.0) == 0
    assert picker.update(steps, 0.24) == 0
    assert picker.update(steps, 0.30) == 1
    assert picker.update(steps, 0.55) == 2
    assert picker.update(steps, 1.0) == 3
    assert picker.update(steps, 0.98) == 3  # tuż pod końcem - bez migania
    assert picker.update(steps, 0.90) == 2
    assert picker.update(steps, 0.48) == 2  # w zapasie
    assert picker.update(steps, 0.40) == 1
    assert picker.update(steps, 0.0) == 0
    picker.update(steps, 1.0)
    assert picker.update(PRESETS[2], 1.0) == 1  # mniej skoków po zmianie w ustawieniach


class FakeSource:
    def __init__(self, name: str) -> None:
        self.name = name
        self.state: PadState | None = PadState(name, 0.0, 0.0)

    def read(self) -> PadState | None:
        return self.state


def test_reader_finds_pad_rescans_rarely_and_survives_unplugging() -> None:
    clock = [0.0]
    sources: list[FakeSource] = []
    scans = []

    def find() -> list[FakeSource]:
        scans.append(clock[0])
        return list(sources)

    reader = PadReader(find, lambda: clock[0])  # type: ignore[arg-type]
    assert reader.read() is None
    clock[0] = 1.0
    assert reader.read() is None
    assert scans == [0.0]  # bez pada nie szuka co klatkę
    dualsense = FakeSource("DualSense")
    sources.append(dualsense)
    clock[0] = 2.0
    state = reader.read()
    assert state is not None and reader.name == "DualSense"
    dualsense.state = None  # odłączony
    sources.clear()
    assert reader.read() is None
    assert reader.name is None


def test_real_pad_discovery_does_not_crash() -> None:
    # Na komputerze bez pada lista jest pusta; z padem - pierwszy da się odczytać.
    for source in pad.discover():
        state = source.read()
        assert state is None or -1.0 <= state.x <= 1.0
