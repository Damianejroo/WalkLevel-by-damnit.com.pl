"""Okno uruchamia się, przełącza widoki i reaguje na zdarzenia Bluetooth (bez Bluetooth).

Scenariusze 0.3.0: konfigurator (nowa instalacja i aktualizacja z 0.2), osoby, profile gier,
historia kroków, eksport i import, trzy tryby sterowania, kolory profili.
"""

from __future__ import annotations

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
from bieznia.gui.ekrany import osoba  # noqa: E402
from bieznia.gui.ekrany.profil import GamePick  # noqa: E402

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
    assert window.status_title.text() == "Połączono: KS-HDSY-X21C"
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
    window.on_simulator(True)
    window.set_control(True)
    source: Any = window.bridge.source
    source.target = source.speed = 5.0
    for _ in range(30):
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
    area = app.qt.primaryScreen().availableGeometry()
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
    area = app.qt.primaryScreen().availableGeometry()
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
    time.sleep(max(gui.PAD_SETTLE_S, gui.PAD_STOP_SETTLE_S) + 0.05)
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
    source: Any = window.bridge.source
    source.target = source.speed = 4.0
    for _ in range(40):
        window.tick()
    assert tracker.current is not None and tracker.current.profile_name == "Domyślny"


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
