"""Okno uruchamia się, przełącza widoki i reaguje na zdarzenia Bluetooth (bez Bluetooth)."""

from __future__ import annotations

import os
import queue
from collections.abc import Iterator
from pathlib import Path
from typing import Any

import pytest

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
QtWidgets = pytest.importorskip("PySide6.QtWidgets")

from bieznia.ble.worker import BleEvent, FoundDevice, Link  # noqa: E402
from bieznia.config import load_config  # noqa: E402
from bieznia.gui import app as gui  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]


class FakeWorker:
    def __init__(self) -> None:
        self.events: queue.Queue[BleEvent] = queue.Queue()
        self.readings: queue.Queue[Any] = queue.Queue()
        self.calls: list[str] = []

    def scan(self) -> None:
        self.calls.append("scan")

    def connect(self, address: str, name: str) -> None:
        self.calls.append(f"connect {name}")

    def disconnect(self) -> None:
        self.calls.append("disconnect")

    def diagnose(self, path: Path) -> None:
        self.calls.append("diagnose")

    def belt(self, command: str) -> None:
        self.calls.append(f"belt {command}")

    def set_speed(self, kmh: float) -> None:
        self.calls.append(f"speed {kmh:g}")

    def stop_belt_now(self, timeout: float = 1.5) -> bool:
        self.calls.append("stop_belt_now")
        return True

    def shutdown(self, timeout: float = 4.0) -> None:
        self.calls.append("shutdown")


@pytest.fixture
def window(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> Iterator[gui.App]:
    config = load_config(ROOT)
    monkeypatch.chdir(tmp_path)
    monkeypatch.setattr(gui, "BleWorker", FakeWorker)
    qt = QtWidgets.QApplication.instance() or QtWidgets.QApplication([])
    app = gui.App(qt, config, enabled=False)
    yield app
    app.quit()


def fake(app: gui.App) -> FakeWorker:
    worker = app.worker
    assert isinstance(worker, FakeWorker)
    return worker


def test_first_start_scans_and_auto_connects_single_treadmill(window: gui.App) -> None:
    worker = fake(window)
    assert worker.calls == ["scan"]
    devices = (
        FoundDevice("AA", "KS-HDSY-X21C", -50, True),
        FoundDevice("BB", "JBL Flip", -60, False),
    )
    worker.events.put(BleEvent("devices", devices=devices))
    worker.events.put(BleEvent("link", link=Link.STREAMING, text="KingSmith"))
    window.tick()
    assert "connect KS-HDSY-X21C" in worker.calls
    assert window.status_title.text() == "Połączono: KS-HDSY-X21C"
    assert window.settings["device_address"] == "AA"


def test_several_treadmills_show_the_list(window: gui.App) -> None:
    devices = (FoundDevice("AA", "KS-X21", -50, True), FoundDevice("BB", "KS-X21", -70, True))
    fake(window).events.put(BleEvent("devices", devices=devices))
    window.tick()
    assert window.pages.currentIndex() == window.DEVICES


def test_bluetooth_off_shows_clear_message(window: gui.App) -> None:
    fake(window).events.put(
        BleEvent("scan_failed", link=Link.BLUETOOTH_OFF, text="Bluetooth wyłączony")
    )
    window.tick()
    assert window.status_title.text() == "Bluetooth wyłączony"
    assert window.status_action.text() == "Spróbuj ponownie"
    assert window.pages.currentIndex() == window.DEVICES


def test_pages_switch_and_controls(window: gui.App) -> None:
    for page in (window.DEVICES, window.SETTINGS, window.LOG, window.MAIN):
        window.pages.go(page)
        window.tick()
    window.enable_switch.setChecked(True)
    assert window.bridge.state.enabled
    window.on_output("keys")
    assert window.bridge.output_kind == "keys"
    window.on_output("dry")
    window.enable_switch.setChecked(False)


def test_simulator_drives_gauge(window: gui.App) -> None:
    window.on_direction("treadmill")  # chodzenie naciska klawisze
    window.on_simulator(True)
    source: Any = window.bridge.source
    source.target = source.speed = 5.0
    for _ in range(30):
        window.tick()
    assert window.gauge.mode == "walk"
    window.on_simulator(False)


def test_theme_changes_live_and_stays_in_settings(window: gui.App) -> None:
    window.pages.go(window.SETTINGS)
    before = window.window
    window.on_look("theme", "light")
    window.tick()
    assert window.t.name == "light"
    assert window.gauge.t.name == "light" and window.enable_switch.t.name == "light"
    assert window.window is before  # to samo okno, bez przebudowy
    assert window.pages.currentIndex() == window.SETTINGS
    window.on_look("theme", "dark")
    window.on_look("large_text", True)
    assert window.pages.currentIndex() == window.SETTINGS


def test_key_capture_changes_walk_key(window: gui.App) -> None:
    from PySide6.QtCore import QEvent, Qt
    from PySide6.QtGui import QKeyEvent

    window.walk_btn.start()
    event = QKeyEvent(
        QEvent.Type.KeyPress, Qt.Key.Key_Up, Qt.KeyboardModifier.NoModifier, 0x148, 0x26, 0, ""
    )
    window.walk_btn.keyPressEvent(event)
    walk, run = window.bridge.keys
    assert (walk.scan, walk.extended, walk.name) == (0x48, True, "Strzałka w górę")
    assert window.walk_btn.text() == "Chód: Strzałka w górę"
    assert window.settings["walk_key"]["scan"] == 0x48
    # ten sam klawisz dla biegu jest odrzucany z wyjaśnieniem
    window.run_btn.start()
    window.run_btn.keyPressEvent(event)
    assert window.bridge.keys[1] == run
    assert "różne" in window.output_hint.text()


def test_narrow_window_clips_no_buttons(window: gui.App) -> None:
    from PySide6.QtWidgets import QApplication, QPushButton

    assert window.window is not None
    # offscreen ma zastępczą czcionkę ok. 2x szerszą niż Segoe UI - okno proporcjonalnie szersze
    offscreen = QApplication.platformName() == "offscreen"
    window.window.resize(560 if offscreen else 420, 760)
    for page in (window.MAIN, window.SETTINGS, window.LOG, window.REPORT):
        window.pages.go(page)
        for _ in range(3):
            QApplication.processEvents()
        current = window.pages.currentWidget()
        assert current is not None
        for b in current.findChildren(QPushButton):
            if not b.isVisible() or b.objectName() in ("icon", "seg", "row"):
                continue
            if QApplication.platformName() != "offscreen":  # prawdziwe czcionki tylko na ekranie
                needed = b.fontMetrics().horizontalAdvance(b.text()) + 2 * 16
                assert b.width() >= needed, f"ucięty przycisk {b.text()!r}: {b.width()} < {needed}"
            right = b.mapTo(current, b.rect().topRight()).x()
            assert right <= current.width(), f"przycisk {b.text()!r} wystaje poza okno"


def test_report_page_prepares_email_without_endpoint(
    window: gui.App, monkeypatch: pytest.MonkeyPatch
) -> None:
    opened: list[str] = []
    monkeypatch.setattr(gui.zgloszenia, "open_email", lambda text, path: opened.append(path.name))
    window.pages.go(window.REPORT)
    window.send_report()
    assert "kilku słowach" in window.report_status.text()  # pusty opis nie przechodzi
    window.report_text.setPlainText("Bieżnia rozłącza się po 2 minutach")
    window.send_report()
    for _ in range(100):
        window.tick()
        if opened:
            break
        __import__("time").sleep(0.02)
    assert opened and opened[0].startswith("zgloszenie-")
    assert "e-mail" in window.report_status.text()


def test_update_button_disabled_when_running_from_source(window: gui.App) -> None:
    assert not window.update_btn.isEnabled()
    assert window.update_label.text().startswith("wersja")


def test_wheel_over_slider_scrolls_page_instead_of_changing_value(window: gui.App) -> None:
    from PySide6.QtCore import QPoint, QPointF, Qt
    from PySide6.QtGui import QWheelEvent

    window.pages.go(window.SETTINGS)
    slider = window.run_slider
    before = slider.value()
    event = QWheelEvent(
        QPointF(5, 5),
        QPointF(5, 5),
        QPoint(0, 0),
        QPoint(0, -120),
        Qt.MouseButton.NoButton,
        Qt.KeyboardModifier.NoModifier,
        Qt.ScrollPhase.NoScrollPhase,
        False,
    )
    slider.wheelEvent(event)
    assert slider.value() == before
    assert not event.isAccepted()  # zdarzenie idzie dalej do przewijania strony


def test_buttons_keep_size_when_text_changes(window: gui.App) -> None:
    from PySide6.QtWidgets import QApplication

    assert window.window is not None
    window.window.resize(460, 840)
    for _ in range(3):
        QApplication.processEvents()
    walk, run = window.walk_btn.width(), window.run_btn.width()
    window.walk_btn.start()  # "Chód: naciśnij klawisz…"
    for _ in range(3):
        QApplication.processEvents()
    assert (window.walk_btn.width(), window.run_btn.width()) == (walk, run)
    window.walk_btn.stop()
    status = window.status_action.width()
    for text in ("Rozłącz", "Wyłącz symulator", "Szukaj bieżni"):
        window.status_action.setText(text)
        QApplication.processEvents()
        assert window.status_action.width() == status


def test_always_on_top_really_sets_topmost(window: gui.App) -> None:
    import sys

    from PySide6.QtWidgets import QApplication

    from bieznia.winapi import is_topmost

    if sys.platform != "win32" or QApplication.platformName() != "windows":
        pytest.skip("wymaga prawdziwego okna Windows")
    assert window.window is not None
    hwnd = int(window.window.winId())
    window.on_always_on_top(True)
    assert is_topmost(hwnd)
    window.on_always_on_top(False)
    assert not is_topmost(hwnd)


def test_threshold_sliders_cover_0_to_12_and_keep_order(window: gui.App) -> None:
    walk, run = window.walk_slider, window.run_slider
    assert (walk.minimum(), walk.maximum(), run.minimum(), run.maximum()) == (0, 120, 0, 120)
    walk.setValue(100)  # chód 10 km/h - bieg musi przesunąć się w górę
    window.on_thresholds(walk)
    assert run.value() / 10 >= walk.value() / 10 + 0.8
    run.setValue(20)  # bieg 2 km/h - chód zjeżdża w dół
    window.on_thresholds(run)
    assert walk.value() / 10 <= 2.0 - 0.8
    assert window.bridge.mapping.run_on == run.value() / 10


def test_floating_button_toggles_control_and_opens_window(window: gui.App) -> None:
    from PySide6.QtCore import Qt

    bubble = window.bubble
    assert not window.settings["floating_button"]  # domyślnie wyłączony
    assert not bubble.isVisible()
    assert bubble.windowFlags() & Qt.WindowType.WindowDoesNotAcceptFocus  # nie zabiera fokusu grze
    assert bubble.windowFlags() & Qt.WindowType.WindowStaysOnTopHint
    window.on_floating_button(True)
    assert bubble.isVisible()
    before = window.bridge.state.enabled
    bubble.toggled.emit()  # lewy klik
    assert window.bridge.state.enabled is not before
    assert window.enable_switch.isChecked() is window.bridge.state.enabled
    window.tick()
    assert bubble.enabled is window.bridge.state.enabled
    bubble.open_requested.emit()  # prawy klik
    assert window.window is not None and window.window.isVisible()
    window.on_floating_button(False)
    assert not bubble.isVisible()


def test_floating_button_remembers_position(window: gui.App) -> None:
    from PySide6.QtCore import QPoint

    window.on_bubble_moved(QPoint(100, 200))
    assert window.settings["floating_pos"] == [100, 200]


def test_control_switch_starts_and_stops_the_belt(window: gui.App) -> None:
    worker = fake(window)
    assert window.settings["belt_control"]  # domyślnie włączone
    window.enable_switch.setChecked(True)
    assert window.bridge.state.enabled
    assert worker.calls[-1] == "belt start"
    window.handle_event(BleEvent("belt", text="refused"))
    assert "klucz" in window.output_hint.text()
    window.toggle_from_bubble()  # przycisk na ekranie wyłącza i zatrzymuje
    assert not window.bridge.state.enabled
    assert not window.enable_switch.isChecked()
    assert worker.calls[-1] == "belt stop"
    window.handle_event(BleEvent("belt", text="stopped"))
    assert "zatrzymana" in window.output_hint.text()


def test_belt_control_can_be_turned_off(window: gui.App) -> None:
    worker = fake(window)
    window.on_belt_control(False)
    assert not window.settings["belt_control"]
    assert "włącza i wyłącza" in window.control_subtitle.text()
    window.set_control(True)
    window.set_control(False)
    assert not [c for c in worker.calls if c.startswith("belt")]


def test_simulator_never_sends_belt_commands(window: gui.App) -> None:
    worker = fake(window)
    window.on_simulator(True)
    window.set_control(True)
    assert not [c for c in worker.calls if c.startswith("belt")]


def test_hotkey_toggles_control_and_belt_but_not_while_capturing(window: gui.App) -> None:
    from bieznia.hotkeys import Hotkeys

    worker = fake(window)
    down: set[int] = set()
    window.hotkeys = Hotkeys({"toggle": window.toggle_key.vk}, key_down=lambda vk: vk in down)
    down.add(0x77)  # F8
    window.tick()
    assert window.bridge.state.enabled
    assert "belt start" in worker.calls
    down.clear()
    window.tick()
    window.walk_btn.start()  # wybór klawisza: F8 nie może przy okazji ruszyć taśmy
    down.add(0x77)
    window.tick()
    assert window.bridge.state.enabled
    window.walk_btn.stop()


def test_toggle_key_can_be_changed(window: gui.App) -> None:
    from PySide6.QtCore import QEvent, Qt
    from PySide6.QtGui import QKeyEvent

    assert window.toggle_btn.text() == "Start/stop: F8"
    window.toggle_btn.start()
    event = QKeyEvent(
        QEvent.Type.KeyPress, Qt.Key.Key_F9, Qt.KeyboardModifier.NoModifier, 0x43, 0x78, 0, ""
    )
    window.toggle_btn.keyPressEvent(event)
    assert (window.toggle_key.scan, window.toggle_key.vk) == (0x43, 0x78)
    assert window.settings["toggle_key"]["vk"] == 0x78
    assert window.control_subtitle.text().startswith("F9")
    # klawisz chodu nie może być jednocześnie start/stop
    window.toggle_btn.start()
    w = QKeyEvent(
        QEvent.Type.KeyPress, Qt.Key.Key_W, Qt.KeyboardModifier.NoModifier, 0x11, 0x57, 0, "w"
    )
    window.toggle_btn.keyPressEvent(w)
    assert window.toggle_key.vk == 0x78
    assert "Start/stop" in window.output_hint.text()


def test_quit_stops_running_belt(window: gui.App) -> None:
    worker = fake(window)
    window.set_control(True)
    window.quit()
    assert "stop_belt_now" in worker.calls
    assert worker.calls.index("stop_belt_now") < worker.calls.index("shutdown")


def speeds(worker: FakeWorker) -> list[str]:
    return [c for c in worker.calls if c.startswith("speed")]


def test_held_keys_set_belt_speed(window: gui.App) -> None:
    worker = fake(window)
    assert window.settings["direction"] == "keyboard"  # domyślnie klawisze sterują taśmą
    assert window.bridge.output_kind == "dry"  # aplikacja sama niczego nie naciska
    assert window.output_seg.isHidden()
    down: set[int] = set()
    window.key_down = lambda vk: vk in down
    window.tick()
    assert speeds(worker) == []  # sterowanie wyłączone - taśma w spokoju
    window.set_control(True)
    window.tick()
    window.tick()
    assert speeds(worker) == ["speed 1"]  # bez klawisza: bieg jałowy, wysłany raz
    down.add(0x57)  # W
    window.tick()
    assert speeds(worker)[-1] == "speed 1.5"
    down.add(0xA0)  # lewy Shift
    window.tick()
    assert speeds(worker)[-1] == "speed 2.5"
    down.discard(0x57)  # sam Shift to nie bieg
    window.tick()
    assert speeds(worker)[-1] == "speed 1"
    window.walk_speed_slider.setValue(20)  # Chód: 2,0 km/h
    assert window.settings["walk_speed"] == 2.0
    down.add(0x57)
    down.discard(0xA0)
    window.tick()
    assert speeds(worker)[-1] == "speed 2"


def test_treadmill_direction_presses_keys_and_never_sets_speed(window: gui.App) -> None:
    worker = fake(window)
    window.direction_seg.changed.emit("treadmill")
    assert window.settings["direction"] == "treadmill"
    assert not window.output_seg.isHidden()
    window.key_down = lambda vk: True
    window.set_control(True)
    window.tick()
    assert speeds(worker) == []
    window.direction_seg.changed.emit("keyboard")
    window.tick()
    assert speeds(worker) == ["speed 2.5"]


def test_without_belt_start_stop_turning_off_slows_down(window: gui.App) -> None:
    worker = fake(window)
    window.on_belt_control(False)
    window.key_down = lambda vk: vk == 0x57
    window.set_control(True)
    window.tick()
    window.set_control(False)
    assert speeds(worker) == ["speed 1.5", "speed 1"]
    assert not [c for c in worker.calls if c.startswith("belt")]


class StickSource:
    name = "DualSense"

    def __init__(self) -> None:
        self.y = 0.0

    def read(self) -> Any:
        from bieznia.pad import PadState

        return PadState(self.name, 0.0, self.y)


def settle(window: gui.App) -> None:
    import time

    window.tick()
    time.sleep(gui.PAD_SETTLE_S + 0.05)
    window.tick()


def test_gamepad_stick_steps_set_belt_speed(window: gui.App) -> None:
    from bieznia.pad import PadReader

    worker = fake(window)
    stick = StickSource()
    window.pad = PadReader(lambda: [stick])  # type: ignore[list-item]
    window.direction_seg.changed.emit("gamepad")
    assert window.keys_row.isHidden()  # klawisze chodu i biegu niepotrzebne
    assert window.bridge.output_kind == "dry"
    window.set_control(True)
    settle(window)
    assert speeds(worker) == ["speed 1"]  # gałka puszczona = skok 1
    stick.y = 0.4  # ok. 37% wychylenia
    settle(window)
    assert speeds(worker)[-1] == "speed 1.5"
    stick.y = 1.0
    window.tick()  # w drodze - jeszcze nic
    assert speeds(worker)[-1] == "speed 1.5"
    settle(window)
    assert speeds(worker)[-1] == "speed 4"
    window.render_pad_status()
    assert "DualSense" in window.pad_status.text()
    assert "skok 4" in window.pad_status.text()


def test_gamepad_step_count_and_thresholds_are_editable(window: gui.App) -> None:
    window.pad_count_seg.changed.emit("3")
    assert window.settings["pad_steps"] == [(0, 1.0), (50, 2.0), (100, 3.0)]
    assert len(window.pad_at_sliders) == 3
    middle = window.pad_at_sliders[1]
    assert middle is not None
    middle.setValue(100)  # nie może przeskoczyć następnego skoku
    assert window.settings["pad_steps"][1][0] == 100 - 5
    middle.setValue(30)
    assert window.settings["pad_steps"][1][0] == 30
    window.pad_count_seg.changed.emit("5")
    assert len(window.settings["pad_steps"]) == 5
    window.direction_seg.changed.emit("gamepad")
    assert "od 75%" in window.output_hint.text()


def test_gamepad_without_pad_keeps_idle_speed(window: gui.App) -> None:
    from bieznia.pad import PadReader

    worker = fake(window)
    window.pad = PadReader(lambda: [])
    window.direction_seg.changed.emit("gamepad")
    window.set_control(True)
    settle(window)
    assert speeds(worker) == ["speed 1"]
    window.render_pad_status()
    assert "Nie widzę pada" in window.pad_status.text()
