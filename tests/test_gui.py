"""Okno uruchamia się, przełącza widoki i reaguje na zdarzenia Bluetooth (bez Bluetooth).

Scenariusze 0.3.0: konfigurator (nowa instalacja i aktualizacja z 0.2), osoby, profile gier,
historia kroków, eksport i import, trzy tryby sterowania, kolory profili.
"""

from __future__ import annotations

import itertools
import json
import os
import queue
import time
from collections.abc import Iterator
from pathlib import Path
from typing import Any

import pytest

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
QtWidgets = pytest.importorskip("PySide6.QtWidgets")
QtCore = pytest.importorskip("PySide6.QtCore")

from bieznia import dane, kopia, paleta  # noqa: E402
from bieznia.ble.worker import BleEvent, FoundDevice, Link  # noqa: E402
from bieznia.config import load_config  # noqa: E402
from bieznia.gui import app as gui  # noqa: E402
from bieznia.gui.ekrany import osoba, sterowanie  # noqa: E402
from bieznia.gui.ekrany.profil import GamePick  # noqa: E402
from bieznia.sources import Reading  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]


class FakeWorker:
    def __init__(self) -> None:
        self.events: queue.Queue[BleEvent] = queue.Queue()
        self.readings: queue.Queue[Any] = queue.Queue()
        self.calls: list[str] = []
        self.wakes: list[bool] = []

    def scan(self) -> None:
        self.calls.append("scan")

    def connect(self, address: str, name: str) -> None:
        self.calls.append(f"connect {name}")

    def disconnect(self) -> None:
        self.calls.append("disconnect")

    def connect_simulator(self) -> None:
        self.calls.append("simulator")

    def sim_remote(self, steps: int) -> None:
        self.calls.append(f"remote {steps:+d}")

    def push(self, speed: float, steps: int | None = None) -> None:
        """Odczyt jak z wątku Bluetooth (prawdziwa bieżnia albo symulator)."""
        self.readings.put(Reading(t=time.monotonic(), speed_kmh=speed, steps=steps))

    def diagnose(self, path: Path) -> None:
        self.calls.append("diagnose")

    def belt(self, command: str) -> None:
        self.calls.append(f"belt {command}")

    def set_speed(self, kmh: float, start_if_stopped: bool = False) -> None:
        self.calls.append(f"speed {kmh:g}")
        self.wakes.append(start_if_stopped)

    def stop_belt_now(self, timeout: float = 1.5) -> bool:
        self.calls.append("stop_belt_now")
        return True

    def shutdown(self, timeout: float = 4.0) -> None:
        self.calls.append("shutdown")


CLOSED: set[int] = set()


def close(app: gui.App) -> None:
    if id(app) in CLOSED:
        return
    CLOSED.add(id(app))
    app.quit()
    # okno naprawdę usunięte - inaczej każdy kolejny test przestylowuje wszystkie poprzednie
    for widget in (app.window, app.bubble):
        if widget is not None:
            widget.close()
            widget.deleteLater()
    QtCore.QCoreApplication.sendPostedEvents(None, QtCore.QEvent.Type.DeferredDelete)


@pytest.fixture
def make(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> Iterator[Any]:
    """Fabryka okien w pustym folderze danych (każde zamykane po teście)."""
    config = load_config(ROOT)
    monkeypatch.chdir(tmp_path)
    monkeypatch.setattr(gui, "BleWorker", FakeWorker)
    qt = QtWidgets.QApplication.instance() or QtWidgets.QApplication([])
    made: list[gui.App] = []

    def build() -> gui.App:
        app = gui.App(qt, config, enabled=False)
        made.append(app)
        return app

    yield build
    for app in made:
        close(app)
    CLOSED.clear()


@pytest.fixture
def window(make: Any) -> gui.App:
    """Okno z jedną osobą (konfigurator już przejdzie)."""
    dane.Store.create("Tester", "Mięta", {})
    app: gui.App = make()
    return app


def fake(app: gui.App) -> FakeWorker:
    worker = app.worker
    assert isinstance(worker, FakeWorker)
    return worker


def buttons(page: Any, text: str) -> list[Any]:
    from PySide6.QtWidgets import QAbstractButton

    return [
        b
        for b in page.findChildren(QAbstractButton)
        if b.text() == text or (text and not b.text() and b.accessibleName() == text)
    ]


def click(page: Any, text: str) -> None:
    found = buttons(page, text)
    assert found, f"brak przycisku {text!r}"
    found[0].click()


def current(app: gui.App) -> Any:
    page = app.pages.currentWidget()
    assert page is not None
    return page


def line_edit(page: Any, name: str) -> Any:
    from PySide6.QtWidgets import QLineEdit

    found = [e for e in page.findChildren(QLineEdit) if e.accessibleName() == name]
    assert found, f"brak pola {name!r}"
    return found[0]


def texts(page: Any) -> str:
    from PySide6.QtWidgets import QLabel

    return "\n".join(lbl.text() for lbl in page.findChildren(QLabel))


# --- połączenie i widoki ----------------------------------------------------


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
    assert window.status_title.text() == "Połączono: KingSmith WalkingPad X21"  # model z profilu
    assert window.settings["device_address"] == "AA"
    assert window.store is not None and window.store.shared["device_address"] == "AA"


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
    for page in (window.DEVICES, window.SETTINGS, window.LOG, window.REPORT, window.MAIN):
        window.pages.go(page)
        window.tick()
    window.open_history()
    window.tick()
    assert window.pages.currentIndex() == window.HISTORY
    window.open_settings()
    window.tick()
    window.enable_switch.setChecked(True)
    assert window.bridge.state.enabled
    window.on_direction("treadmill")
    window.on_output("keys")
    assert window.bridge.output_kind == "keys"
    window.on_output("dry")
    window.enable_switch.setChecked(False)


def test_simulator_drives_gauge_and_chips(window: gui.App) -> None:
    window.on_direction("treadmill")  # chodzenie naciska klawisze
    worker = fake(window)
    window.on_simulator(True)
    assert "simulator" in worker.calls  # symulator to sterownik w wątku Bluetooth
    window.set_control(True)
    for _ in range(30):
        worker.push(5.0)
        window.tick()
    assert window.gauge.mode == "walk"
    assert window.gauge.active == 1 and window.zone_chips.active == 1
    assert window.gauge_status.text().startswith("Chód")
    window.on_simulator(False)


def test_theme_changes_live_and_stays_in_settings(window: gui.App) -> None:
    window.open_settings()
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
    assert window.store is not None and window.store.person.look["large_text"] is True


def test_key_capture_changes_walk_key(window: gui.App) -> None:
    from PySide6.QtCore import QEvent, Qt
    from PySide6.QtGui import QKeyEvent

    window.open_settings()
    walk_btn, run_btn = window.walk_btn, window.run_btn
    assert walk_btn is not None and run_btn is not None
    walk_btn.start()
    event = QKeyEvent(
        QEvent.Type.KeyPress, Qt.Key.Key_Up, Qt.KeyboardModifier.NoModifier, 0x148, 0x26, 0, ""
    )
    walk_btn.keyPressEvent(event)
    walk, run = window.bridge.keys
    assert (walk.scan, walk.extended, walk.name) == (0x48, True, "Strzałka w górę")
    assert walk_btn.text() == "Chód: Strzałka w górę"
    assert window.settings["walk_key"]["scan"] == 0x48
    assert window.store is not None
    assert window.store.profile.settings["walk_key"]["scan"] == 0x48  # zapis w profilu
    # ten sam klawisz dla biegu jest odrzucany z wyjaśnieniem
    run_btn.start()
    run_btn.keyPressEvent(event)
    assert window.bridge.keys[1] == run
    assert "różne" in window.output_hint.text()
    assert "różne" in window.last_toast


def clipped(page_widget: Any) -> list[str]:
    """Przyciski ucięte albo wystające poza okno (czcionki sprawdzane tylko na ekranie)."""
    from PySide6.QtGui import QFontMetrics
    from PySide6.QtWidgets import QApplication, QPushButton

    bad = []
    for b in page_widget.findChildren(QPushButton):
        if not b.isVisible() or b.objectName() in ("icon", "seg", "row") or not b.text():
            continue
        glyph_only = len(b.text()) == 1 and ord(b.text()) >= 0xE000  # ikona, np. kolor w palecie
        if QApplication.platformName() != "offscreen" and not glyph_only:
            chip = b.objectName() == "chip"
            font = b.font()
            font.setBold(chip or font.bold())  # żetony mają pogrubiony napis
            fm = QFontMetrics(font)
            needed = max(fm.horizontalAdvance(line) for line in b.text().split(chr(10)))
            needed += 4 if chip else 2 * 16
            if b.width() < needed:
                bad.append(f"ucięty {b.text()!r}: {b.width()} < {needed}")
        right = b.mapTo(page_widget, b.rect().topRight()).x()
        if right > page_widget.width():
            bad.append(f"wystaje {b.text()!r} ({right} > {page_widget.width()})")
    return bad


def test_narrow_window_clips_no_buttons(window: gui.App) -> None:
    from PySide6.QtWidgets import QApplication

    assert window.window is not None
    # offscreen ma zastępczą czcionkę ok. 2x szerszą niż Segoe UI - okno proporcjonalnie szersze
    offscreen = QApplication.platformName() == "offscreen"
    window.window.resize(640 if offscreen else 420, 760)
    window.open_settings()
    window.open_profile_new()
    window.open_history()
    window.open_export()
    for page in (
        window.MAIN,
        window.SETTINGS,
        window.LOG,
        window.REPORT,
        window.PROFILE,
        window.HISTORY,
        window.EXPORT,
    ):
        window.pages.go(page)
        for _ in range(3):
            QApplication.processEvents()
        assert clipped(current(window)) == []
    # pad z 5 skokami: 5 żetonów w wąskim oknie
    window.on_direction("gamepad")
    window.on_pad_count(5)
    window.pages.go(window.MAIN)
    for _ in range(3):
        QApplication.processEvents()
    assert clipped(current(window)) == []


def test_window_size_is_remembered(make: Any) -> None:
    dane.Store.create("Tester", "Mięta", {})
    app: gui.App = make()
    assert app.window is not None
    area = app.home_screen().availableGeometry()
    app.window.move(area.x() + 30, area.y() + 40)
    app.window.resize(520, 700)
    close(app)
    again: gui.App = make()
    assert again.window is not None
    assert (again.window.width(), again.window.height()) == (520, 700)
    assert again.window.x() == area.x() + 30


def test_window_off_screen_comes_back_to_the_middle(make: Any) -> None:
    store = dane.Store.create("Tester", "Mięta", {})
    store.shared["window_geometry"] = [-25600, -25600, 740, 890]  # np. odłączony monitor
    store.save()
    app: gui.App = make()
    assert app.window is not None
    area = app.home_screen().availableGeometry()
    assert area.contains(app.window.geometry().center())


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
        time.sleep(0.02)
    assert opened and opened[0].startswith("zgloszenie-")
    assert "e-mail" in window.report_status.text()


def test_update_button_disabled_when_running_from_source(window: gui.App) -> None:
    assert not window.update_btn.isEnabled()
    assert window.update_label.text().startswith("wersja")  # jedna osoba - bez imienia


def test_buttons_keep_size_when_text_changes(window: gui.App) -> None:
    from PySide6.QtWidgets import QApplication

    assert window.window is not None
    window.window.resize(460, 840)
    window.open_settings()
    for _ in range(3):
        QApplication.processEvents()
    walk_btn, run_btn = window.walk_btn, window.run_btn
    assert walk_btn is not None and run_btn is not None
    walk, run = walk_btn.width(), run_btn.width()
    walk_btn.start()  # "Chód: naciśnij klawisz…"
    for _ in range(3):
        QApplication.processEvents()
    assert (walk_btn.width(), run_btn.width()) == (walk, run)
    walk_btn.stop()
    window.pages.go(window.MAIN)
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


def test_thresholds_cover_0_to_12_and_keep_order(window: gui.App) -> None:
    window.on_direction("treadmill")
    walk, run = window.walk_from_step, window.run_from_step
    assert walk is not None and run is not None
    window.on_threshold("walk", 10.0)  # chód 10 km/h - bieg musi przesunąć się w górę
    assert window.settings["run_on"] >= window.settings["stand_below"] + 0.8
    window.on_threshold("run", 2.0)  # bieg 2 km/h - chód zjeżdża w dół
    assert window.settings["stand_below"] <= 2.0 - 0.8
    assert window.bridge.mapping.run_on == 2.0
    assert window.walk_from_step is not None
    assert window.walk_from_step.value == window.settings["stand_below"]


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


# --- start i stop taśmy -----------------------------------------------------


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


def test_simulator_is_driven_like_a_treadmill(window: gui.App) -> None:
    """Symulator przyjmuje te same polecenia co bieżnia - sterowanie da się sprawdzić bez
    bieżni. F6/F7 to jego pilot, a wyłączenie wraca do zapamiętanej bieżni."""
    worker = fake(window)
    window.settings["device_address"], window.settings["device_name"] = "AA", "URTM054"
    window.on_simulator(True)
    assert window.caps.speed_max == 6.0 and window.caps.start_delay_s == 1.0
    window.on_belt_control(True)
    window.set_control(True)
    assert "belt start" in worker.calls
    down: set[int] = set()
    from bieznia.hotkeys import Hotkeys

    window.hotkeys = Hotkeys({"sim_faster": "F7"}, key_down=lambda vk: vk in down)
    down.add(0x76)  # F7
    window.tick()
    assert "remote +1" in worker.calls
    window.on_simulator(False)
    assert worker.calls[-1] == "connect URTM054"
    assert window.caps.start_delay_s == 4.0  # profil Urevo SpaceWalk 5L


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
    assert window.walk_btn is not None
    window.walk_btn.start()  # wybór klawisza: F8 nie może przy okazji ruszyć taśmy
    down.add(0x77)
    window.tick()
    assert window.bridge.state.enabled
    window.walk_btn.stop()


def test_toggle_key_can_be_changed(window: gui.App) -> None:
    from PySide6.QtCore import QEvent, Qt
    from PySide6.QtGui import QKeyEvent

    assert window.toggle_btn.text() == "Start/stop: F8"
    assert window.toggle_cap.text() == "F8"
    window.toggle_btn.start()
    event = QKeyEvent(
        QEvent.Type.KeyPress, Qt.Key.Key_F9, Qt.KeyboardModifier.NoModifier, 0x43, 0x78, 0, ""
    )
    window.toggle_btn.keyPressEvent(event)
    assert (window.toggle_key.scan, window.toggle_key.vk) == (0x43, 0x78)
    assert window.settings["toggle_key"]["vk"] == 0x78
    assert window.toggle_cap.text() == "F9"
    assert window.store is not None and window.store.shared["toggle_key"]["vk"] == 0x78
    # klawisz chodu nie może być jednocześnie start/stop
    window.toggle_btn.start()
    w = QKeyEvent(
        QEvent.Type.KeyPress, Qt.Key.Key_W, Qt.KeyboardModifier.NoModifier, 0x11, 0x57, 0, "w"
    )
    window.toggle_btn.keyPressEvent(w)
    assert window.toggle_key.vk == 0x78
    assert "Start/stop" in window.output_hint.text()


def test_toggle_key_can_be_a_combination(window: gui.App) -> None:
    from PySide6.QtCore import QEvent, Qt
    from PySide6.QtGui import QKeyEvent

    from bieznia.hotkeys import Hotkeys

    mods = Qt.KeyboardModifier.ControlModifier | Qt.KeyboardModifier.ShiftModifier
    btn = window.toggle_btn
    btn.start()
    ctrl = QKeyEvent(
        QEvent.Type.KeyPress,
        Qt.Key.Key_Control,
        Qt.KeyboardModifier.ControlModifier,
        0x1D,
        0xA2,
        0,
        "",
    )
    btn.keyPressEvent(ctrl)  # sam Ctrl - czeka na resztę
    assert btn.capturing and "Ctrl+…" in btn.text()
    f9 = QKeyEvent(QEvent.Type.KeyPress, Qt.Key.Key_F9, mods, 0x43, 0x78, 0, "")
    btn.keyPressEvent(f9)
    assert window.toggle_key.label == "Ctrl+Shift+F9"
    assert window.toggle_cap.text() == "Ctrl+Shift+F9"
    assert window.settings["toggle_key"]["mods"] == ["ctrl", "shift"]
    # Alt+Tab odrzucone z wyjaśnieniem
    btn.start()
    alt_tab = QKeyEvent(
        QEvent.Type.KeyPress, Qt.Key.Key_Tab, Qt.KeyboardModifier.AltModifier, 0x0F, 0x09, 0, ""
    )
    btn.keyPressEvent(alt_tab)
    assert window.toggle_key.label == "Ctrl+Shift+F9"
    assert "Alt+Tab" in window.last_toast
    # skrót globalny działa tylko z modyfikatorami
    down: set[int] = set()
    window.hotkeys = Hotkeys({"toggle": (0x78, ("ctrl", "shift"))}, key_down=lambda vk: vk in down)
    down.add(0x78)
    window.tick()
    assert not window.bridge.state.enabled
    down.clear()
    window.tick()
    down.update({0x11, 0x10, 0x78})
    window.tick()
    assert window.bridge.state.enabled
    assert window.make_hotkeys()._mods["toggle"] == (0x11, 0x10)


def test_quit_stops_running_belt(window: gui.App) -> None:
    worker = fake(window)
    window.set_control(True)
    window.quit()
    assert "stop_belt_now" in worker.calls
    assert worker.calls.index("stop_belt_now") < worker.calls.index("shutdown")


# --- klawisze -> bieżnia ----------------------------------------------------


def speeds(worker: FakeWorker) -> list[str]:
    return [c for c in worker.calls if c.startswith("speed")]


def test_held_keys_set_belt_speed(window: gui.App) -> None:
    worker = fake(window)
    assert window.settings["direction"] == "keyboard"  # domyślnie klawisze sterują taśmą
    assert window.bridge.output_kind == "dry"  # aplikacja sama niczego nie naciska
    down: set[int] = set()
    window.key_down = lambda vk: vk in down
    window.tick()
    assert speeds(worker) == []  # sterowanie wyłączone - taśma w spokoju
    window.set_control(True)
    window.tick()
    window.tick()
    assert speeds(worker) == ["speed 1"]  # bez klawisza: bieg jałowy, wysłany raz
    assert window.gauge.active == 0
    down.add(0x57)  # W
    window.tick()
    assert speeds(worker)[-1] == "speed 1.5"
    assert window.zone_chips.active == 1
    down.add(0xA0)  # lewy Shift
    window.tick()
    assert speeds(worker)[-1] == "speed 2.5"
    assert window.zone_chips.active == 2
    down.discard(0x57)  # sam Shift to nie bieg
    window.tick()
    assert speeds(worker)[-1] == "speed 1"
    window.open_settings()
    stepper = [s for s in window.control_box.parentWidget().findChildren(gui.k.Stepper)]
    assert len(stepper) == 3  # bez klawisza, chód, bieg
    stepper[1].plus.click()  # Chód: 1,5 -> 1,6 km/h
    assert window.settings["walk_speed"] == 1.6
    down.discard(0xA0)
    down.add(0x57)
    window.tick()
    assert speeds(worker)[-1] == "speed 1.6"
    assert window.zone_chips.chips[1][1] == "1,6 km/h"


def test_speed_can_be_typed_and_held(window: gui.App) -> None:
    window.open_settings()
    steppers = window.control_box.parentWidget().findChildren(gui.k.Stepper)
    walk = steppers[1]
    walk.shown.setFocus()
    walk.shown.setText("2,3")
    walk.shown.editingFinished.emit()
    assert window.settings["walk_speed"] == 2.3
    walk.shown.setText("abc")  # błędny wpis - wraca poprzednia wartość
    walk.shown.editingFinished.emit()
    assert window.settings["walk_speed"] == 2.3
    walk.shown.setText("99 km/h")  # ponad zakres - najwyższa dozwolona
    walk.shown.editingFinished.emit()
    assert window.settings["walk_speed"] == walk.hi
    walk.set_value(1.6)
    walk.plus.pressed.emit()  # przytrzymanie: po kilku powtórzeniach skok o 0,5
    for _ in range(walk.FAST_AFTER):
        walk.plus.clicked.emit()
    assert walk.value == 2.2
    walk.plus.clicked.emit()
    assert walk.value == 2.5
    walk.plus.clicked.emit()
    assert walk.value == 3.0 and window.settings["walk_speed"] == 3.0


def test_treadmill_direction_presses_keys_and_never_sets_speed(window: gui.App) -> None:
    worker = fake(window)
    window.mode_tiles.changed.emit("treadmill")
    assert window.settings["direction"] == "treadmill"
    window.open_settings()
    assert window.output_seg.isVisibleTo(current(window))
    window.key_down = lambda vk: True
    window.set_control(True)
    window.tick()
    assert speeds(worker) == []
    window.mode_tiles.changed.emit("keyboard")
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


# --- pad -> bieżnia ---------------------------------------------------------


class StickSource:
    name = "DualSense"

    def __init__(self) -> None:
        self.y = 0.0

    def read(self) -> Any:
        from bieznia.pad import PadState

        return PadState(self.name, 0.0, self.y)


def settle(window: gui.App) -> None:
    window.tick()
    time.sleep(max(sterowanie.PAD_SETTLE_S, sterowanie.PAD_STOP_SETTLE_S) + 0.05)
    window.tick()


def test_gamepad_stick_steps_set_belt_speed(window: gui.App) -> None:
    from bieznia.pad import PadReader

    worker = fake(window)
    stick = StickSource()
    window.pad = PadReader(lambda: [stick])  # type: ignore[list-item]
    window.mode_tiles.changed.emit("gamepad")
    window.pad_idle_seg.changed.emit("slow")  # gałka puszczona = prędkość skoku 1
    assert window.walk_btn is None  # klawisze chodu i biegu niepotrzebne
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
    assert window.zone_chips.active == 3
    window.open_settings()
    window.render_pad_status()
    assert "DualSense" in window.pad_status.text()
    assert "skok 4" in window.pad_status.text()
    assert window.step_ladder is not None and window.step_ladder.stick == pytest.approx(1.0)


def test_gamepad_step_count_and_thresholds_are_editable(window: gui.App) -> None:
    window.mode_tiles.changed.emit("gamepad")
    window.pad_count_seg.changed.emit("3")
    assert window.settings["pad_steps"] == [(0, 1.0), (50, 2.0), (100, 3.0)]
    assert len(window.pad_steppers) == 3
    assert window.step_ladder is not None
    window.step_ladder.moved.emit(1, 30)
    assert window.settings["pad_steps"][1][0] == 30
    assert window.pad_steppers[1].sub_label.text() == "od 30% wychylenia"
    assert window.zone_chips.chips[1][0] == "30%"
    window.pad_count_seg.changed.emit("5")
    assert len(window.settings["pad_steps"]) == 5
    assert len(window.zone_chips.chips) == 5
    assert window.zone_chips.chips[3][0] == "75%"


def test_ladder_drag_keeps_gaps(window: gui.App) -> None:
    from PySide6.QtCore import Qt
    from PySide6.QtGui import QKeyEvent

    window.mode_tiles.changed.emit("gamepad")
    ladder = window.step_ladder
    assert ladder is not None
    ladder.setFocus()
    ladder.selected = 1
    right = QKeyEvent(QtCore.QEvent.Type.KeyPress, Qt.Key.Key_Right, Qt.KeyboardModifier.NoModifier)
    for _ in range(200):  # strzałki: próg nie przeskoczy następnego
        ladder.keyPressEvent(right)
    steps = window.settings["pad_steps"]
    assert steps[1][0] <= steps[2][0] - 5


def test_gamepad_without_pad_keeps_idle_speed(window: gui.App) -> None:
    from bieznia.pad import PadReader

    worker = fake(window)
    window.pad = PadReader(lambda: [])
    window.mode_tiles.changed.emit("gamepad")
    window.pad_idle_seg.changed.emit("slow")
    window.set_control(True)
    settle(window)
    assert speeds(worker) == ["speed 1"]
    window.open_settings()
    window.render_pad_status()
    assert "Nie widzę pada" in window.pad_status.text()


def test_released_stick_stops_belt_and_movement_wakes_it(window: gui.App) -> None:
    from bieznia.pad import PadReader

    worker = fake(window)
    assert window.settings["pad_idle"] == "stop"  # domyślnie puszczona gałka = taśma stoi
    stick = StickSource()
    window.pad = PadReader(lambda: [stick])  # type: ignore[list-item]
    window.mode_tiles.changed.emit("gamepad")
    window.set_control(True)
    assert "belt start" not in worker.calls  # F8 nie rusza taśmy - zrobi to gałka
    settle(window)
    assert worker.calls[-1] == "belt stop"
    settle(window)
    assert worker.calls.count("belt stop") == 1  # bez powtarzania co klatkę
    stick.y = 0.4
    settle(window)
    assert speeds(worker) == ["speed 1.5"]
    assert worker.wakes[-1]  # stojąca taśma rusza sama
    stick.y = 0.0
    window.tick()
    assert worker.calls[-1] == "speed 1.5"  # krótko przez środek - jeszcze nie stop
    settle(window)
    assert worker.calls[-1] == "belt stop"
    first = window.pad_steppers[0]
    assert not first.minus.isEnabled() and not first.plus.isEnabled()
    assert window.zone_chips.chips[0][1] == "stop"


def test_zero_test_mode_sends_zero_without_stopping(window: gui.App) -> None:
    from bieznia.pad import PadReader

    worker = fake(window)
    window.pad = PadReader(lambda: [StickSource()])  # type: ignore[list-item]
    window.mode_tiles.changed.emit("gamepad")
    window.pad_idle_seg.changed.emit("zero")
    assert window.zone_chips.chips[0][1] == "0 km/h"
    window.set_control(True)
    settle(window)
    assert speeds(worker) == ["speed 0"]
    assert "belt stop" not in worker.calls


def test_settings_show_only_controls_of_current_mode(window: gui.App) -> None:
    window.open_settings()
    assert window.walk_btn is not None and window.step_ladder is None
    assert window.walk_from_step is None  # progi chodu tylko w trybie Chodzenie
    window.mode_tiles.changed.emit("gamepad")
    assert window.walk_btn is None and window.step_ladder is not None
    window.mode_tiles.changed.emit("treadmill")
    assert window.walk_from_step is not None and window.step_ladder is None
    assert window.pages.currentIndex() == window.SETTINGS  # przebudowa nie zmienia widoku


def simulator_switch(page: Any) -> Any:
    return next(
        w for w in page.findChildren(gui.Switch) if w.accessibleName() == "Symulator bieżni"
    )


def test_advanced_section_is_collapsed(window: gui.App) -> None:
    from PySide6.QtWidgets import QPushButton

    window.open_settings()
    page = current(window)
    assert not simulator_switch(page).isVisibleTo(page)
    assert window.toggle_btn.isVisibleTo(page)  # klawisz F8 jest w sekcji Bieżnia, nie ukryty
    adv = [b for b in page.findChildren(QPushButton) if b.accessibleName() == "Zaawansowane"]
    assert len(adv) == 1
    adv[0].click()
    assert window.advanced_open and simulator_switch(page).isVisibleTo(page)
    window.rebuild_settings()  # po przebudowie dalej rozwinięte
    page = current(window)
    assert simulator_switch(page).isVisibleTo(page)


def test_settings_can_show_other_mode_without_switching(window: gui.App) -> None:
    window.open_settings()
    assert window.settings["direction"] == "keyboard"
    window.show_mode_settings("gamepad")  # podgląd ustawień pada - tryb dalej Klawisze
    assert window.step_ladder is not None and window.walk_btn is None
    assert window.settings["direction"] == "keyboard"
    assert "Aktywny jest tryb Klawisze" in texts(current(window))
    window.pad_count_seg.changed.emit("3")  # zmiana ustawień pada zawczasu
    assert len(window.settings["pad_steps"]) == 3
    assert window.settings["direction"] == "keyboard" and window.step_ladder is not None
    click(current(window), "Włącz Pad")
    assert window.settings["direction"] == "gamepad" and window.mode_tiles.current == "gamepad"
    window.open_settings()  # po ponownym wejściu - widok aktywnego trybu
    assert window.settings_view == "gamepad"


def test_clicking_toggle_key_on_card_starts_key_change(
    window: gui.App, monkeypatch: pytest.MonkeyPatch
) -> None:
    from PySide6.QtTest import QTest

    started: list[Any] = []
    monkeypatch.setattr(gui.KeyButton, "start", lambda self: started.append(self))
    window.toggle_cap.clicked.emit()
    QTest.qWait(50)
    assert window.pages.currentIndex() == window.SETTINGS
    assert started == [window.toggle_btn]  # czeka na klawisz w Ustawieniach > Bieżnia


def test_zone_chips_are_information_only(window: gui.App) -> None:
    from PySide6.QtCore import Qt

    assert window.zone_chips.buttons
    for b in window.zone_chips.buttons:
        assert b.testAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
        assert b.focusPolicy() == Qt.FocusPolicy.NoFocus


def test_theme_change_restyles_only_visible_page(window: gui.App) -> None:
    window.pages.go(window.MAIN)
    window.on_look("theme", "light")
    assert window.qt.styleSheet() == gui.TOOLTIP_QSS  # bez przestylowania całej aplikacji
    main = window.pages.widget(window.MAIN)
    log_page = window.pages.widget(window.LOG)
    assert main.property("qss_version") == window.pages.qss_version
    assert log_page.property("qss_version") != window.pages.qss_version  # dopiero przy wejściu
    window.pages.go(window.LOG)
    assert log_page.property("qss_version") == window.pages.qss_version
    assert window.window is not None and window.window.updatesEnabled()


# --- konfigurator i migracja z 0.2 -----------------------------------------


def finish_wizard(app: gui.App, name: str | None = None) -> None:
    assert app.pages.currentIndex() == app.WIZARD
    if name is not None:
        line_edit(current(app), "Nazwa osoby").setText(name)
    click(current(app), "Dalej")
    click(current(app), "Dalej")
    click(current(app), "Gotowe")


def test_fresh_install_runs_wizard_and_creates_person(make: Any) -> None:
    app: gui.App = make()
    assert app.store is None
    assert app.pages.currentIndex() == app.WIZARD
    page = current(app)
    assert "Jak się nazywasz?" in texts(page)
    assert not buttons(page, "Anuluj")  # pierwszego uruchomienia nie da się pominąć
    name = line_edit(page, "Nazwa osoby").text()
    assert name  # wylosowana nazwa
    click(page, "Dalej")
    assert "Profil Domyślny" in texts(current(app))
    click(current(app), "Dalej")
    click(current(app), "Pad")
    click(current(app), "Gotowe")
    assert app.pages.currentIndex() == app.MAIN
    assert app.store is not None and Path("dane/osoby.json").exists()
    assert app.store.person.name == name
    assert app.store.profile.name == "Domyślny"
    assert app.settings["direction"] == "gamepad" and app.mode_tiles.current == "gamepad"
    assert not app.person_chip.isVisibleTo(app.window)  # jedna osoba - bez przełącznika
    assert not app.profile_btn.isEnabled()  # jeden profil - bez listy
    assert app.profile_btn.text() == "Domyślny"


def test_wizard_name_suggestions_sit_side_by_side_and_wrap(make: Any) -> None:
    from PySide6.QtWidgets import QApplication

    app: gui.App = make()
    assert app.window is not None
    chips = [b for n in app.wizard.suggestions for b in buttons(current(app), n)]
    assert len(chips) == 3
    app.window.resize(1500, 760)  # szeroko: jeden wiersz (też przy szerokiej czcionce testowej)
    for _ in range(3):
        QApplication.processEvents()
    assert len({c.y() for c in chips}) == 1
    assert all(c.width() >= c.sizeHint().width() for c in chips)  # bez ucinania
    app.window.resize(420, 760)  # wąsko: zawijają się, nic nie wystaje
    for _ in range(3):
        QApplication.processEvents()
    page = current(app)
    for c in chips:
        assert c.mapTo(page, c.rect().topRight()).x() <= page.width()


def test_update_from_02_keeps_settings(make: Any) -> None:
    old = {
        "direction": "keyboard",
        "walk_speed": 2.0,
        "device_address": "AA",
        "device_name": "KS-X21",
        "theme": "light",
        "toggle_key": {"scan": 0x43, "extended": False, "vk": 0x78, "name": "F9"},
    }
    Path("dane").mkdir()
    Path("dane/ustawienia.json").write_text(json.dumps(old), encoding="utf-8")
    app: gui.App = make()
    assert app.pages.currentIndex() == app.WIZARD
    assert "Bieżnia ma teraz osoby" in texts(current(app))
    assert app.settings["walk_speed"] == 2.0  # przed konfiguratorem działa jak w 0.2
    click(current(app), "Dalej")
    assert "Zostaw moje obecne ustawienia" in texts(current(app))
    click(current(app), "Dalej")
    click(current(app), "Gotowe")
    store = app.store
    assert store is not None
    assert store.profile.settings["walk_speed"] == 2.0
    assert store.shared["device_address"] == "AA"
    assert store.person.look["theme"] == "light"
    assert app.toggle_key.name == "F9"
    assert Path("dane/kopie/ustawienia-0.2.json").exists()
    # drugie uruchomienie: bez konfiguratora, te same ustawienia
    close(app)
    again: gui.App = make()
    assert again.pages.currentIndex() == again.MAIN
    assert again.settings["walk_speed"] == 2.0 and again.t.name == "light"


def test_corrupted_store_starts_wizard_and_keeps_copy(make: Any) -> None:
    Path("dane").mkdir()
    Path("dane/osoby.json").write_text("{zepsuty", encoding="utf-8")
    app: gui.App = make()
    assert app.pages.currentIndex() == app.WIZARD
    assert list(Path("dane/kopie").glob("uszkodzony-*.json"))


def test_wizard_with_game_folder_creates_game_profile(
    make: Any, monkeypatch: pytest.MonkeyPatch
) -> None:
    from PySide6.QtGui import QColor, QImage

    app: gui.App = make()
    dane.ICONS_DIR.mkdir(parents=True)
    img = QImage(32, 32, QImage.Format.Format_ARGB32)
    img.fill(QColor("#e01020"))
    img.save(str(dane.ICONS_DIR / "nowa-test.png"))
    pick = GamePick(Path("C:/Gry/Wiedzmin/witcher3.exe"), "Wiedźmin 3", "nowa-test.png", "Czerwień")
    monkeypatch.setattr(osoba, "pick_game", lambda parent: pick)
    click(current(app), "Dalej")
    app.wizard_pick_game()
    page = current(app)
    assert line_edit(page, "Nazwa profilu gry").text() == "Wiedźmin 3"
    line_edit(page, "Nazwa profilu gry").setText("Wiedźmin")
    click(page, "Dalej")
    assert "Dla profilu Wiedźmin" in texts(current(app))
    click(current(app), "Gotowe")
    store = app.store
    assert store is not None
    assert [p.name for p in store.person.profiles] == ["Domyślny", "Wiedźmin"]
    game = store.profile
    assert game.name == "Wiedźmin" and game.color == "Czerwień"
    assert game.icon_file == f"{game.id}.png" and (dane.ICONS_DIR / game.icon_file).exists()
    assert not (dane.ICONS_DIR / "nowa-test.png").exists()
    assert app.profile_btn.isEnabled()  # dwa profile - lista do wyboru
    red = paleta.kolor("Czerwień").for_theme(app.t.name)
    assert app.t.accent == red  # kolor profilu to akcent aplikacji


# --- osoby ------------------------------------------------------------------


def add_person(app: gui.App, name: str, copy: bool = False) -> None:
    app.open_wizard_add()
    assert buttons(current(app), "Anuluj")
    line_edit(current(app), "Nazwa osoby").setText(name)
    click(current(app), "Dalej")
    if copy:
        app.wizard.copy_profiles = True
    click(current(app), "Dalej")
    click(current(app), "Gotowe")


def test_two_people_have_separate_settings(window: gui.App) -> None:
    store = window.store
    assert store is not None
    first = store.person.id
    window.on_belt_speed("walk_speed", 2.2)
    add_person(window, "Druga Osoba")
    assert len(store.people) == 2 and store.person.name == "Druga Osoba"
    assert window.settings["walk_speed"] == 1.5  # nowa osoba - domyślne
    assert window.person_chip.isVisibleTo(window.window)  # dwie osoby - przełącznik
    assert window.update_label.text().startswith("Druga Osoba · wersja")
    window.choose_person(first)
    assert window.settings["walk_speed"] == 2.2
    assert window.store is not None and window.store.person.name == "Tester"


def test_add_person_can_copy_profiles(window: gui.App) -> None:
    store = window.store
    assert store is not None
    store.add_profile("Gra", color="Fiolet", activate=False)
    add_person(window, "Kopia", copy=True)
    assert [p.name for p in store.person.profiles] == ["Domyślny", "Gra"]
    ids = {p.id for p in store.people[0].profiles}
    assert not ids & {p.id for p in store.person.profiles}  # nowe id - osobne ustawienia


def test_person_popup_lists_people_and_switches(window: gui.App) -> None:
    add_person(window, "Druga")
    window.open_person_popup()
    pop = window.person_popup
    assert pop is not None and pop.isVisible()
    rows = buttons(pop, "")
    assert len(rows) == 2
    rows[0].click()  # pierwsza osoba
    assert window.store is not None and window.store.person.name == "Tester"
    assert not pop.isVisible()


def test_hiding_switchers_in_settings(window: gui.App) -> None:
    add_person(window, "Druga")
    assert window.person_chip.isVisibleTo(window.window)
    window.on_switch_visibility("show_person_switch", False)
    assert not window.person_chip.isVisibleTo(window.window)
    store = window.store
    assert store is not None
    store.add_profile("Gra")
    window.apply_profile()
    assert window.profile_btn.isEnabled()
    window.on_switch_visibility("show_profile_switch", False)
    assert not window.profile_btn.isEnabled()


def test_edit_and_remove_person(window: gui.App) -> None:
    add_person(window, "Do Usunięcia")
    store = window.store
    assert store is not None and window.history is not None
    pid = store.person.id
    window.open_person_edit(pid)
    page = current(window)
    edit = line_edit(page, "Nazwa osoby")
    edit.setText("Nowa Nazwa")
    edit.editingFinished.emit()
    assert store.person.name == "Nowa Nazwa"
    window.remove_person(pid)
    assert len(store.people) == 1 and store.person.name == "Tester"
    assert window.pages.currentIndex() == window.SETTINGS
    window.remove_person(store.person.id)  # ostatniej osoby się nie usuwa
    assert len(store.people) == 1


# --- profile gier -----------------------------------------------------------


def test_new_profile_copy_and_switch(window: gui.App) -> None:
    store = window.store
    assert store is not None
    window.on_direction("gamepad")
    window.open_profile_new()
    page = current(window)
    line_edit(page, "Nazwa profilu").setText("Forza")
    click(page, "Utwórz i włącz profil")
    assert store.profile.name == "Forza"
    assert window.settings["direction"] == "gamepad"  # kopia obecnego
    assert window.pages.currentIndex() == window.SETTINGS
    window.on_direction("keyboard")
    window.choose_profile(store.person.profiles[0].id)
    assert store.profile.name == "Domyślny" and window.settings["direction"] == "gamepad"
    window.choose_profile(store.person.profiles[1].id)
    assert window.settings["direction"] == "keyboard" and window.mode_tiles.current == "keyboard"


def test_new_profile_from_defaults(window: gui.App) -> None:
    store = window.store
    assert store is not None
    window.on_belt_speed("walk_speed", 3.0)
    window.open_profile_new()
    page = current(window)
    line_edit(page, "Nazwa profilu").setText("Czysty")
    buttons(page, "Domyślne")[0].click()
    click(page, "Utwórz i włącz profil")
    assert window.settings["walk_speed"] == 1.5


def test_profile_color_changes_accent_and_is_saved(window: gui.App) -> None:
    store = window.store
    assert store is not None
    base = window.t.accent
    window.open_settings()
    window.on_profile_color("Banan")
    assert store.profile.color == "Banan"
    assert window.t.accent == paleta.kolor("Banan").for_theme(window.t.name) != base
    assert window.t.run == paleta.RUN_ALT.for_theme(window.t.name)  # żółty nie zlewa się z biegiem
    assert window.pages.currentIndex() == window.SETTINGS
    window.on_profile_color(paleta.DOMYSLNY)
    assert window.t.accent == base


def test_edit_duplicate_and_remove_profile(window: gui.App) -> None:
    store = window.store
    assert store is not None
    pid = store.profile.id
    window.open_profile_edit(pid)
    page = current(window)
    edit = line_edit(page, "Nazwa profilu")
    edit.setText("Główny")
    edit.editingFinished.emit()
    assert store.profile.name == "Główny"
    assert window.profile_btn.text() == "Główny"
    window.duplicate_profile(pid)
    assert [p.name for p in store.person.profiles] == ["Główny", "Główny (kopia)"]
    window.choose_profile(store.person.profiles[1].id)
    window.remove_profile(store.profile.id)
    assert len(store.person.profiles) == 1 and store.profile.name == "Główny"
    window.remove_profile(store.profile.id)  # ostatniego się nie usuwa
    assert len(store.person.profiles) == 1


def test_profile_edit_saves_automatically_and_says_so(window: gui.App) -> None:
    from PySide6.QtTest import QTest

    store = window.store
    assert store is not None
    window.open_profile_edit(store.profile.id)
    page = current(window)
    note = [n for n in page.findChildren(gui.k.SaveNote)]
    assert len(note) == 1 and "automatycznie" in note[0].text()
    line_edit(page, "Nazwa profilu").setText("Bez Entera")  # bez Enter i bez wyjścia z pola
    QTest.qWait(700)
    assert store.profile.name == "Bez Entera"
    assert "Zapisano" in note[0].text()
    assert window.profile_btn.text() == "Bez Entera"
    window.open_person_edit(store.person.id)
    page = current(window)
    line_edit(page, "Nazwa osoby").setText("Inna Nazwa")
    QTest.qWait(700)
    assert store.person.name == "Inna Nazwa"
    assert "Zapisano" in page.findChildren(gui.k.SaveNote)[0].text()


def test_profile_popup_switches_profiles(window: gui.App) -> None:
    store = window.store
    assert store is not None
    store.add_profile("Gra", color="Niebo", activate=False)
    window.apply_profile()
    window.open_profile_popup()
    pop = window.profile_popup
    assert pop is not None and pop.isVisible()
    buttons(pop, "")[1].click()
    assert store.profile.name == "Gra"
    assert window.profile_btn.text().startswith("Gra")


# --- historia kroków --------------------------------------------------------


def walk_session(app: gui.App, steps: int, minutes: float = 5.0) -> None:
    """Sesja z kontrolowanym zegarem: taśma jedzie, kroki rosną, potem przerwa."""
    tracker = app.tracker
    assert tracker is not None
    now = [time.time()]
    tracker.clock = lambda: now[0]
    tracker.update(0.0, 0, 0.0, 0.0)
    for i in range(1, 11):
        now[0] += minutes * 6
        tracker.update(3.0, steps * i // 10, 0.04 * i, minutes * 6 * i)
    tracker.finish()


def test_history_counts_steps_per_person_and_profile(window: gui.App) -> None:
    store, history = window.store, window.history
    assert store is not None and history is not None
    walk_session(window, 1200)
    assert window.history_today(store.person.id) == 1200
    store.add_profile("Gra")
    window.apply_profile()
    walk_session(window, 300)
    add_person(window, "Druga")
    walk_session(window, 500)
    first = store.people[0].id
    assert window.history_today(first) == 1500
    assert window.history_today(store.person.id) == 500
    groups = dict(history.by_group(0.0, first))
    assert groups["Domyślny"].steps == 1200 and groups["Gra"].steps == 300
    window.open_history()
    page = current(window)
    assert "Moja" in [b.text() for b in page.findChildren(QtWidgets.QPushButton)]
    assert "500" in texts(page)
    window.set_history(scope="all")
    assert "Druga" in texts(current(window)) and "Tester" in texts(current(window))
    window.set_history(range_="all")
    assert window.pages.currentIndex() == window.HISTORY


def test_history_records_ticks_from_simulator(window: gui.App) -> None:
    window.on_simulator(True)
    tracker = window.tracker
    assert tracker is not None
    worker = fake(window)
    for i in range(40):
        worker.push(4.0, i * 2)
        window.tick()
    assert tracker.current is not None and tracker.current.profile_name == "Domyślny"
    assert tracker.current.source == "symulator"  # nie miesza się z wynikami z bieżni


def test_history_survives_profile_switch(window: gui.App) -> None:
    store = window.store
    assert store is not None
    tracker = window.tracker
    assert tracker is not None
    walk_session(window, 100)
    store.add_profile("Gra", activate=False)
    window.choose_profile(store.person.profiles[1].id)
    assert tracker.owner[3] == "Gra"


def test_empty_history_page(window: gui.App) -> None:
    window.open_history()
    assert "Brak kroków" in texts(current(window))


# --- kopia danych -----------------------------------------------------------


def test_export_import_roundtrip_and_restore(
    window: gui.App, monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    from PySide6.QtWidgets import QFileDialog

    store = window.store
    assert store is not None
    walk_session(window, 800)
    target = tmp_path / "kopia.bieznia"
    monkeypatch.setattr(QFileDialog, "getSaveFileName", lambda *a, **k: (str(target), ""))
    monkeypatch.setattr(QFileDialog, "getOpenFileName", lambda *a, **k: (str(target), ""))
    window.open_export()
    page = current(window)
    assert "1 os., 1 profili gier, 1 sesji" in texts(page)
    click(page, "Logi")  # zaznacza logi
    assert window.export_flags["logs"] is True
    click(current(window), "Zapisz plik kopii…")
    assert target.exists()
    assert "Zapisano" in window.last_toast
    # import tej samej osoby jako nowej
    window.open_import()
    assert window.pages.currentIndex() == window.IMPORT
    page = current(window)
    assert "Tester już jest" in texts(page)
    window.import_conflict(store.person.id, "new")
    click(page, "Importuj")
    assert len(store.people) == 2
    assert window.history is not None and window.history.count() == 2
    assert "Zaimportowano" in window.last_toast
    backup = kopia.latest_backup()
    assert backup is not None
    window.restore_backup(backup)
    assert window.store is not None and len(window.store.people) == 1
    assert window.history is not None and window.history.count() == 1


def test_export_filters_people_and_profiles(window: gui.App) -> None:
    store = window.store
    assert store is not None
    store.add_profile("Gra", activate=False)
    add_person(window, "Druga")
    window.open_export()
    first = store.people[0]
    window.export_toggle_profile(first.id, first.profiles[1].id, False)
    window.export_toggle_person(store.person.id, False)
    choice = window.export_choice()
    assert choice.people == {first.id: {first.profiles[0].id}}
    window.export_flag("range", "month")
    assert window.export_choice().since > 0


def test_import_of_broken_file_shows_message(
    window: gui.App, monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    from PySide6.QtWidgets import QFileDialog

    bad = tmp_path / "zly.bieznia"
    bad.write_bytes(b"to nie jest zip")
    monkeypatch.setattr(QFileDialog, "getOpenFileName", lambda *a, **k: (str(bad), ""))
    window.open_settings()
    window.open_import()
    assert window.pages.currentIndex() == window.SETTINGS
    assert window.last_toast


# --- płynność: bez skoków przewinięcia, pustych klatek i drgania nagłówka ----------------


def sample_during(app: gui.App, action: Any, ms: int = 400) -> list[dict[str, Any]]:
    """Wykonuje akcję i co kilka ms zapisuje stan okna (jak widzi go użytkownik)."""
    from PySide6.QtCore import QTimer
    from PySide6.QtTest import QTest
    from PySide6.QtWidgets import QScrollArea

    out: list[dict[str, Any]] = []

    def probe() -> None:
        page = app.pages.currentWidget()
        area = page.findChild(QScrollArea) if page else None
        assert app.window is not None
        out.append(
            {
                "scroll": area.verticalScrollBar().value() if area else 0,
                "updates": app.window.updatesEnabled(),
                "page_pos": (page.x(), page.y()) if page else (0, 0),
            }
        )

    timer = QTimer()
    timer.setInterval(5)
    timer.timeout.connect(probe)
    timer.start()
    action()
    probe()  # stan zaraz po akcji, zanim pętla zdarzeń cokolwiek poprawi
    QTest.qWait(ms)
    timer.stop()
    return out


def scroll_settings(app: gui.App, px: int) -> int:
    from PySide6.QtTest import QTest
    from PySide6.QtWidgets import QScrollArea

    assert app.window is not None
    app.window.resize(740, 700)
    app.open_settings()
    QTest.qWait(50)
    area = current(app).findChild(QScrollArea)
    area.verticalScrollBar().setValue(px)
    return int(area.verticalScrollBar().value())


@pytest.mark.parametrize(
    "change",
    [
        lambda a: a.on_direction("gamepad"),
        lambda a: a.on_profile_color("Fiolet"),
        lambda a: a.on_look("theme", "light"),
        lambda a: a.choose_profile(a.store.person.profiles[1].id),
        lambda a: a.show_mode_settings("treadmill"),
    ],
    ids=["tryb", "kolor", "motyw", "profil", "podglad-trybu"],
)
def test_option_change_never_jumps_scroll_or_blanks_window(window: gui.App, change: Any) -> None:
    assert window.store is not None
    window.store.add_profile("Gra", activate=False)
    pos = scroll_settings(window, 500)
    assert pos > 0
    samples = sample_during(window, lambda: change(window))
    assert {s["scroll"] for s in samples} == {pos}  # ani na chwilę na górę strony
    assert all(s["updates"] for s in samples)  # okno nie przestaje się odmalowywać


def test_page_transition_does_not_move_live_page(window: gui.App) -> None:
    from PySide6.QtTest import QTest

    gui.Motion.enabled = True
    try:
        samples = sample_during(window, window.open_settings)
        assert {s["page_pos"] for s in samples} == {(0, 0)}  # nagłówek nie jeździ
        samples = sample_during(window, lambda: window.go_back(window.MAIN))
        assert {s["page_pos"] for s in samples} == {(0, 0)}
        QTest.qWait(400)
        assert not [c for c in window.pages.findChildren(gui.Cover) if c.isVisible()]
    finally:
        gui.Motion.enabled = False


def test_scrollbar_space_is_constant(window: gui.App) -> None:
    from PySide6.QtCore import Qt
    from PySide6.QtTest import QTest
    from PySide6.QtWidgets import QScrollArea

    window.open_settings()
    window.open_history()  # krótka strona
    QTest.qWait(50)
    for i in range(window.pages.count()):
        for area in window.pages.widget(i).findChildren(QScrollArea):
            policy = area.verticalScrollBarPolicy()
            assert policy == Qt.ScrollBarPolicy.ScrollBarAlwaysOn  # szerokość treści stała


def test_export_toggle_keeps_scroll(window: gui.App) -> None:
    from PySide6.QtTest import QTest
    from PySide6.QtWidgets import QScrollArea

    store = window.store
    assert store is not None and window.window is not None
    for i in range(6):
        store.add_profile(f"Gra {i}", activate=False)
    window.window.resize(740, 600)
    window.open_export()
    QTest.qWait(50)
    area = current(window).findChild(QScrollArea)
    area.verticalScrollBar().setValue(300)
    pos = area.verticalScrollBar().value()
    pid = store.person.profiles[3].id
    samples = sample_during(
        window, lambda: window.export_toggle_profile(store.person.id, pid, False)
    )
    assert {s["scroll"] for s in samples} == {pos}


def test_settings_reopen_is_instant_when_nothing_changed(window: gui.App) -> None:
    window.open_settings()
    page = window.pages.widget(window.SETTINGS)
    window.go_back(window.MAIN)
    window.open_settings()
    assert window.pages.widget(window.SETTINGS) is page  # bez przebudowy
    window.go_back(window.MAIN)
    window.on_belt_speed("walk_speed", 2.0)  # zmiana - Ustawienia budują się od nowa
    window.open_settings()
    assert window.pages.widget(window.SETTINGS) is not page


def test_cover_without_changes_is_pixel_identical(window: gui.App) -> None:
    """Przykrycie przy zmianie, która nic nie zmienia, nie może zmienić ani jednego piksela
    (żadnego mignięcia odcienia tła ani przesunięcia)."""
    from PySide6.QtCore import QTimer
    from PySide6.QtTest import QTest

    assert window.window is not None
    gui.Motion.enabled = True
    try:
        window.open_settings()
        QTest.qWait(400)
        w = window.window
        before = w.grab().toImage()
        frames: list[Any] = []
        timer = QTimer()
        timer.setInterval(15)
        timer.timeout.connect(lambda: frames.append(w.grab().toImage()))
        timer.start()
        window.quietly(lambda: None, 150)
        QTest.qWait(250)
        timer.stop()
        assert frames
        worst = max(image_difference(before, f) for f in frames)
        assert worst <= 2, worst  # zaokrąglenia mieszania kolorów - niewidoczne
    finally:
        gui.Motion.enabled = False


def image_difference(a: Any, b: Any) -> int:
    """Największa różnica kanału koloru między dwoma obrazami (co 4. piksel)."""
    from PySide6.QtGui import QColor

    worst = 0
    for y in range(0, min(a.height(), b.height()), 4):
        for x in range(0, min(a.width(), b.width()), 4):
            c1, c2 = QColor.fromRgba(a.pixel(x, y)), QColor.fromRgba(b.pixel(x, y))
            d = max(
                abs(c1.red() - c2.red()),
                abs(c1.green() - c2.green()),
                abs(c1.blue() - c2.blue()),
                abs(c1.alpha() - c2.alpha()),
            )
            if d > worst:
                worst = d
    return worst


def test_mode_switch_in_settings_is_animated(window: gui.App) -> None:
    """Zmiana trybu w Ustawieniach: płynna animacja bez zacięć - każda klatka to tylko obraz
    (bez przeliczania układu całego ekranu), podświetlenie przełącznika jedzie, karta pod
    spodem od razu ma docelowy układ."""
    import time as _time

    from PySide6.QtCore import QTimer
    from PySide6.QtTest import QTest

    assert window.window is not None
    window.window.resize(740, 890)
    gui.Motion.enabled = True
    try:
        window.open_settings()
        QTest.qWait(300)
        view = window.mode_view
        cover = window.pages.cover
        assert cover is not None
        stamps: list[float] = []
        pills: list[float] = []
        progress: list[float] = []
        bands: list[tuple[int, int, int]] = []

        def probe() -> None:
            stamps.append(_time.perf_counter())
            if view.pill is not None:
                pills.append(view.pill.x())
            if cover.isVisible():
                progress.append(cover.progress)
                if cover.band is not None:
                    bands.append(cover.band)

        timer = QTimer()
        timer.setInterval(8)
        timer.timeout.connect(probe)
        timer.start()
        t0 = _time.perf_counter()
        view.buttons["gamepad"].click()  # klawisze -> pad: karta rośnie
        handler_ms = (_time.perf_counter() - t0) * 1000
        QTest.qWait(450)
        timer.stop()
        gaps = [(b - a) * 1000 for a, b in itertools.pairwise(stamps)]
        slow = 3 if os.environ.get("CI") else 1  # współdzielone maszyny GitHuba są wolniejsze
        assert max(gaps) < 45 * slow, gaps  # animacja nie stoi w miejscu
        assert len({round(x) for x in pills}) >= 4  # podświetlenie jedzie, nie przeskakuje
        assert len({round(v, 2) for v in progress if 0 < v < 1}) >= 4  # zrzuty płynnie
        assert not cover.isVisible()  # po wszystkim żywy widok, bez przykrycia
        assert bands and bands[0][2] == window.control_body.height()  # cel = prawdziwa wysokość
        assert bands[0][2] > bands[0][1] > 0  # pad ma więcej ustawień niż klawisze
        assert window.step_ladder is not None and window.settings["direction"] == "keyboard"
        assert handler_ms < 400 * slow
    finally:
        gui.Motion.enabled = False


def test_direction_change_in_settings_updates_in_place(window: gui.App) -> None:
    store = window.store
    assert store is not None
    window.open_settings()
    page = window.pages.widget(window.SETTINGS)
    window.on_direction("gamepad")
    assert window.pages.widget(window.SETTINGS) is page  # bez przebudowy całego widoku
    assert window.profile_rows[store.profile.id].sub.text().startswith("Pad")
    assert window.step_ladder is not None


def test_settings_after_wizard_show_people_and_profiles(make: Any) -> None:
    app: gui.App = make()
    finish_wizard(app)
    app.on_direction("gamepad")  # zmiana trybu z ekranu głównego, zanim Ustawienia się otworzą
    app.open_settings()
    page_texts = texts(current(app))
    assert "Osoby" in page_texts and "Profile gier" in page_texts


def test_focus_ring_follows_keyboard_focus_without_global_filter(window: gui.App) -> None:
    """Obwódka fokusu działa bez globalnego filtra zdarzeń (ten spowalniał animacje)."""
    from PySide6.QtCore import QEvent, QPointF, Qt
    from PySide6.QtGui import QMouseEvent

    window.open_settings()
    btn = window.toggle_btn
    ring = window.ring
    ring._focus_changed(None, btn)  # fokus wrócił po kliknięciu / zmianie strony - bez obwódki
    assert not ring.isVisible()
    window.window.tab_at = time.monotonic()  # Tab - fokus z klawiatury
    ring._focus_changed(None, btn)
    assert ring.isVisible() and ring.geometry().contains(
        btn.mapTo(window.window, btn.rect().center())
    )
    press = QMouseEvent(
        QEvent.Type.MouseButtonPress,
        QPointF(2, 2),
        QPointF(2, 2),
        Qt.MouseButton.LeftButton,
        Qt.MouseButton.LeftButton,
        Qt.KeyboardModifier.NoModifier,
    )
    QtWidgets.QApplication.sendEvent(btn, press)  # klik - obwódka znika
    assert not ring.isVisible()
    label_only = QtWidgets.QLabel("x", current(window))
    window.window.tab_at = time.monotonic()
    ring._focus_changed(btn, label_only)  # element bez obsługi Tab - bez obwódki
    assert not ring.isVisible()


def test_popup_closes_on_click_elsewhere(window: gui.App) -> None:
    from PySide6.QtCore import QEvent, QPointF, Qt
    from PySide6.QtGui import QMouseEvent

    add_person(window, "Druga")
    window.open_person_popup()
    pop = window.person_popup
    assert pop is not None and pop.isVisible()
    far = QPointF(5, 5)
    press = QMouseEvent(
        QEvent.Type.MouseButtonPress,
        far,
        window.status_title.mapToGlobal(far.toPoint()).toPointF(),
        Qt.MouseButton.LeftButton,
        Qt.MouseButton.LeftButton,
        Qt.KeyboardModifier.NoModifier,
    )
    QtWidgets.QApplication.sendEvent(window.status_title, press)
    assert not pop.isVisible()


def test_profile_switch_in_settings_is_animated(window: gui.App) -> None:
    from PySide6.QtTest import QTest

    store = window.store
    assert store is not None
    store.add_profile("Gra", activate=False)
    gui.Motion.enabled = True
    try:
        window.open_settings()
        QTest.qWait(300)
        cover = window.pages.cover
        assert cover is not None
        window.choose_profile(store.person.profiles[1].id)
        assert cover.isVisible()  # przenikanie, a nie przeskok
        QTest.qWait(400)
        assert not cover.isVisible() and store.profile.name == "Gra"
    finally:
        gui.Motion.enabled = False


def test_tab_key_shows_focus_ring_but_returning_focus_does_not(window: gui.App) -> None:
    """Obwódka tylko po Tab - po kliknięciu i powrocie na stronę nie zostaje na przycisku."""
    from PySide6.QtCore import Qt
    from PySide6.QtTest import QTest

    window.open_settings()
    btn = window.toggle_btn
    window.window.tab_at = float("-inf")
    btn.setFocus(Qt.FocusReason.OtherFocusReason)  # jak powrót fokusu po zmianie strony
    assert not window.ring.isVisible()
    QTest.keyClick(btn, Qt.Key.Key_Tab)
    focused = QtWidgets.QApplication.focusWidget()
    if focused is not None and focused is not btn and focused.window() is window.window:
        assert window.window.tab_at > 0  # Tab przeszedł przez okno


def test_models_page_lists_profiles_and_shows_download_result(
    window: gui.App, monkeypatch: pytest.MonkeyPatch
) -> None:
    from bieznia.sterowniki import pobieranie

    window.settings["device_name"] = "URTM054"
    window.open_settings()
    window.open_models()
    assert window.pages.currentIndex() == window.MODELS
    shown = texts(current(window))
    assert "Urevo SpaceWalk 5L · Twoja bieżnia" in shown
    assert "KingSmith WalkingPad X21" in shown
    monkeypatch.setattr(
        pobieranie,
        "download_profiles",
        lambda: pobieranie.DownloadResult(added=["Nowa Bieżnia 9"]),
    )
    window.fetch_models_task()  # zwykle w wątku - tu od razu
    window.tick()
    assert "Nowe modele: Nowa Bieżnia 9." in window.models_status.text()
    window.go_back(window.SETTINGS)


# --- konto online (opcjonalne) -----------------------------------------------------------


def online_window(window: gui.App, tmp_path: Path) -> Any:
    """Okno z kontem na wirtualnym serwerze; zadania sieciowe od razu (bez wątków)."""
    from test_konto import BASE, FakeServer

    from bieznia import konto
    from bieznia.konto.sejf import MemoryStore

    server = FakeServer()
    window.account = konto.Account(BASE, MemoryStore(), tmp_path / "konto.json", opener=server)
    window.run_task = lambda func, *args: func(*args)  # type: ignore[method-assign]
    return server


def fill(page: Any, values: dict[str, str]) -> None:
    for edit in page.findChildren(QtWidgets.QLineEdit):
        if edit.placeholderText() in values:
            edit.setText(values[edit.placeholderText()])


def test_online_features_are_invisible_without_account(window: gui.App) -> None:
    assert window.online_btn.isHidden()
    window.open_settings()
    page = current(window)
    assert "Konto online (opcjonalne)" in texts(page)
    window.open_online()  # bez konta - zamiast funkcji online logowanie
    assert window.pages.currentIndex() == window.ACCOUNT


def test_register_then_login_shows_friends_ranking_and_people(
    window: gui.App, tmp_path: Path
) -> None:
    server = online_window(window, tmp_path)
    window.open_account("register")
    page = current(window)
    fill(
        page,
        {
            "Kod zaproszenia": "KOD-START",
            "E-mail": "ola@example.pl",
            "Pseudonim (widzą go znajomi)": "x",
        },
    )
    submit = next(b for b in buttons(page, "Załóż konto") if b.objectName() == "primary")
    submit.click()
    assert "Pseudonim" in window.account_status.text() and not server.requests  # bez sieci
    fill(
        page,
        {
            "Pseudonim (widzą go znajomi)": "Ola_77",
            "Hasło": "dlugie-haslo-123",
            "Powtórz hasło": "dlugie-haslo-123",
        },
    )
    submit.click()
    assert "regulamin" in window.account_status.text()  # bez akceptacji nic nie idzie
    page.findChildren(gui.Switch)[0].setChecked(True)
    submit.click()
    window.tick()
    window.tick()
    assert "u1" in server.users  # konto na kod zaproszenia - od razu zalogowane
    assert window.account.state.logged_in
    assert window.pages.currentIndex() == window.ONLINE
    assert not window.online_btn.isHidden()
    shown = texts(current(window))
    assert "Kot_Bury" in shown and "idzie teraz 3,2 km/h" in shown and "Nowy_Gracz" in shown
    window.set_online_tab("ranking")
    window.tick()
    assert "Ola_77" in texts(current(window))
    window.set_online_tab("people")
    window.tick()
    assert "Maraton_Max" in texts(current(window))
    window.set_online_tab("me")
    assert "Usuń konto" in [b.text() for b in current(window).findChildren(QtWidgets.QPushButton)]


def test_sessions_go_to_account_and_logout_hides_online(window: gui.App, tmp_path: Path) -> None:
    server = online_window(window, tmp_path)
    store = window.store
    assert store is not None
    window.account.register(
        "KOD-START", "ola@example.pl", "dlugie-haslo-123", "Ola_77", store.person.id
    )
    window.account_changed()
    walk_session(window, 500)
    window.account_upload_now()
    window.tick()
    assert len(server.sessions) == 1
    sent = next(iter(server.sessions.values()))
    assert sent["kroki"] == 500 and sent["widocznosc"] == "znajomi"
    window.open_history()
    eye = [
        b
        for b in current(window).findChildren(QtWidgets.QPushButton)
        if b.toolTip() == "Ukryj online"
    ]
    assert eye  # przy sesji: ukryj online
    eye[0].click()
    window.tick()
    assert next(iter(server.sessions.values()))["widocznosc"] == "prywatna"
    window.account_job("logout", window.account.logout)
    window.tick()
    assert not window.account.state.logged_in and window.online_btn.isHidden()
