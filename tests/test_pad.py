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
        if isinstance(source, pad.HidSource):
            source.stop()


def test_sony_reports_usb_and_bluetooth() -> None:
    from bieznia.pad import parse_sony

    # prawdziwe ramki DualSense z Bluetooth (pełny raport 0x31, gałka w spoczynku)
    rest = parse_sony(bytes.fromhex("31d1817d7e7f0000") + bytes(70))
    assert rest is not None and abs(rest[0]) < 0.02 and abs(rest[1]) < 0.03
    forward = parse_sony(bytes([0x31, 0x00, 128, 0]) + bytes(74))
    assert forward is not None and forward[1] == 1.0  # Y=0 w raporcie = do przodu
    usb = parse_sony(bytes([0x01, 255, 128]) + bytes(61))
    assert usb is not None and usb[0] == 1.0
    ds4_bt = parse_sony(bytes([0x11, 0xC0, 0x00, 0, 255]) + bytes(73))
    assert ds4_bt is not None and ds4_bt == (-1.0, -1.0)
    assert parse_sony(bytes([0x05, 1, 2, 3])) is None  # inne raporty pomijamy
    assert parse_sony(b"") is None


def test_background_scan_does_not_block_and_delivers_pad() -> None:
    import threading
    import time

    release = threading.Event()

    def slow_find() -> list[FakeSource]:
        release.wait(2)  # szukanie trwa - okno w tym czasie działa dalej
        return [FakeSource("DualSense (Bluetooth)")]

    reader = PadReader(slow_find, background=True)  # type: ignore[arg-type]
    t0 = time.perf_counter()
    assert reader.read() is None
    assert time.perf_counter() - t0 < 0.05  # bez czekania na szukanie
    release.set()
    for _ in range(100):
        state = reader.read()
        if state is not None:
            break
        time.sleep(0.01)
    assert state is not None and reader.name == "DualSense (Bluetooth)"


def test_sony_buttons_in_every_report_layout() -> None:
    from bieznia.pad import parse_sony_buttons

    # DualSense USB (64 bajty): przyciski w bajtach 8-10, hat 8 = strzałki puszczone
    usb = bytearray(64)
    usb[0], usb[8], usb[10] = 0x01, 0x08 | 0x20, 0x02  # krzyżyk + panel dotykowy
    assert parse_sony_buttons(bytes(usb), dualsense=True) == {"south", "touchpad"}
    # DualSense Bluetooth, pełny raport 0x31 (o bajt dalej)
    bt = bytearray(78)
    bt[0], bt[9], bt[10] = 0x31, 0x08, 0x10  # Create
    assert parse_sony_buttons(bytes(bt), dualsense=True) == {"select"}
    # DualShock 4 USB: przyciski zaraz za gałkami (bajty 5-7), strzałka w górę = hat 0
    ds4 = bytearray(64)
    ds4[0], ds4[5], ds4[6] = 0x01, 0x00 | 0x80, 0x01  # góra + trójkąt, L1
    assert parse_sony_buttons(bytes(ds4), dualsense=False) == {"up", "north", "l1"}
    # DualSense przez Bluetooth w trybie podstawowym (krótki raport) - układ jak DS4
    short = bytes([0x01, 128, 128, 128, 128, 0x08 | 0x40, 0x00, 0x01, 0, 0])
    assert parse_sony_buttons(short, dualsense=True) == {"east", "home"}
    assert parse_sony_buttons(b"", dualsense=True) == frozenset()


def test_xinput_buttons_and_labels() -> None:
    from types import SimpleNamespace

    from bieznia.pad import button_label, xinput_buttons

    g = SimpleNamespace(wButtons=0x1000 | 0x0020, bLeftTrigger=200, bRightTrigger=10)
    assert xinput_buttons(g) == {"south", "select", "l2"}
    assert button_label("south", "DualSense (USB)") == "Krzyżyk"
    assert button_label("south", "Pad Xbox (XInput 1)") == "A"
    assert button_label("touchpad", None) == "Panel dotykowy"
    assert button_label("b5", "Gamepad") == "Przycisk 5"
