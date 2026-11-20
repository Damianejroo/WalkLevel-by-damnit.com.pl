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


def test_theme_rebuild_keeps_working(window: gui.App) -> None:
    window.settings["theme"] = "light"
    window.build()
    window.tick()
    assert window.t.name == "light"
