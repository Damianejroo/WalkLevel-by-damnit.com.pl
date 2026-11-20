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
