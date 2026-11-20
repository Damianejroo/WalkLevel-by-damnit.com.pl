"""Okno aplikacji (PySide6): widok główny, wybór bieżni, ustawienia, dziennik.

Zasady (Warsztat): jedna główna akcja na widok, stan zawsze w tym samym miejscu,
kontrolki 44 px z ramką, cyfry o stałej szerokości, animacje tylko z powodem,
dwa motywy. Tło okna to materiał systemowy Windows (Mica), treść na pełnych kartach.
"""

from __future__ import annotations

import contextlib
import logging
import os
import queue
import signal
import sys
import threading
import time
from datetime import datetime
from pathlib import Path
from typing import Any

from PySide6.QtCore import QPoint, QPropertyAnimation, Qt, QTimer, QUrl
from PySide6.QtGui import (
    QCloseEvent,
    QColor,
    QDesktopServices,
    QFont,
    QGuiApplication,
    QIcon,
    QPainter,
    QPaintEvent,
    QPixmap,
)
from PySide6.QtWidgets import (
    QApplication,
    QHBoxLayout,
    QLineEdit,
    QPlainTextEdit,
    QScrollArea,
    QStackedWidget,
    QVBoxLayout,
    QWidget,
)

from bieznia import (
    APP_NAME,
    HOMEPAGE,
    REPOSITORY,
    TITLE,
    __author__,
    __version__,
    aktualizacje,
    zgloszenia,
)
from bieznia.ble import kingsmith
from bieznia.ble.worker import BleEvent, BleWorker, FoundDevice, Link
from bieznia.engine import Bridge
from bieznia.gui import theme as th
from bieznia.gui.theme import Theme
from bieznia.gui.widgets import (
    Belt,
    DeviceRow,
    ElidedLabel,
    FloatingToggle,
    FocusRing,
    Gauge,
    HoldButton,
    KeyButton,
    Motion,
    ResponsiveRow,
    Segmented,
    Slider,
    Sparkline,
    StatTile,
    StatusDot,
    Switch,
    TileGrid,
    button,
    card,
    ease,
    fix_width,
    fmt_num,
    icon_button,
    label,
)
from bieznia.hotkeys import Hotkeys
from bieznia.klawisze import RUN_DEFAULT, TOGGLE_DEFAULT, WALK_DEFAULT, Key, problem
from bieznia.logs import set_detailed
from bieznia.mapping import MappingConfig
from bieznia.paths import FROZEN
from bieznia.recorder import Recorder
from bieznia.settings import save_enabled, save_settings
from bieznia.sources.ble_source import QueueSource
from bieznia.sources.simulator import SimulatedTreadmill
from bieznia.winapi import is_key_down, set_topmost, vk_from_scan

log = logging.getLogger("bieznia")

STATUS_ACTIONS = (
    "Szukaj bieżni",
    "Anuluj",
    "Rozłącz",
    "Spróbuj ponownie",
    "Wyłącz symulator",
)
TICK_MS = 50
STARTED = time.monotonic()  # import tego modułu to praktycznie start procesu
HYSTERESIS = 0.6


class QueueLogHandler(logging.Handler):
    def __init__(self, target: queue.Queue[str]) -> None:
        super().__init__(logging.INFO)
        self.target = target
        self.setFormatter(logging.Formatter("%(asctime)s  %(message)s", "%H:%M:%S"))

    def emit(self, record: logging.LogRecord) -> None:
        with contextlib.suppress(Exception):
            self.target.put_nowait(self.format(record))


def app_icon(t: Theme) -> QIcon:
    png = Path(__file__).with_name("ikona.png")
    if png.exists():
        return QIcon(str(png))
    pix = QPixmap(64, 64)
    pix.fill(Qt.GlobalColor.transparent)
    p = QPainter(pix)
    p.setRenderHint(QPainter.RenderHint.Antialiasing)
    p.setBrush(QColor(t.accent))
    p.setPen(Qt.PenStyle.NoPen)
    p.drawRoundedRect(0, 0, 64, 64, 14, 14)
    font = QFont(th.ICON_FONT)
    font.setPixelSize(38)
    p.setFont(font)
    p.setPen(QColor(t.on_accent))
    p.drawText(pix.rect(), Qt.AlignmentFlag.AlignCenter, th.ICONS["walk"])
    p.end()
    return QIcon(pix)


# --- okno ------------------------------------------------------------------


class Window(QWidget):
    def __init__(self, t: Theme, on_close: Any) -> None:
        super().__init__()
        self.t = t
        self.on_close = on_close
        self.backdrop = False
        self.setWindowTitle(TITLE)
        self.setWindowIcon(app_icon(t))
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        self.setMinimumSize(420, 640)

    def retheme(self, t: Theme) -> None:
        self.t = t
        if sys.platform == "win32" and self.isVisible():
            ok = th.apply_backdrop(int(self.winId()), t.name == "dark")  # pasek tytułu też
            self.backdrop = self.backdrop or ok
        self.update()

    def showEvent(self, e: Any) -> None:
        if not self.backdrop:
            self.backdrop = th.apply_backdrop(int(self.winId()), self.t.name == "dark")
            self.update()
        super().showEvent(e)

    def paintEvent(self, _e: QPaintEvent) -> None:
        p = QPainter(self)
        bg = QColor(self.t.bg)
        if self.backdrop:
            bg.setAlphaF(self.t.backdrop_alpha)  # przez tło prześwituje materiał Windows
        p.fillRect(self.rect(), bg)

    def closeEvent(self, e: QCloseEvent) -> None:
        if self.on_close():
            e.accept()
        else:
            e.ignore()


class Pages(QStackedWidget):
    """Przejście między widokami: krótki przesuw (panel), bez przenikania."""

    def go(self, index: int, back: bool = False) -> None:
        if index == self.currentIndex():
            return
        self.setCurrentIndex(index)
        page = self.currentWidget()
        if not Motion.enabled or page is None:
            return
        anim = QPropertyAnimation(page, b"pos", page)
        anim.setDuration(th.T_PANEL_IN)
        anim.setEasingCurve(ease(th.EASE_IN))
        anim.setStartValue(QPoint(-28 if back else 28, 0))
        anim.setEndValue(QPoint(0, 0))
        anim.start(QPropertyAnimation.DeletionPolicy.DeleteWhenStopped)


def page_layout(widget: QWidget) -> QVBoxLayout:
    widget.setObjectName("page")
    lay = QVBoxLayout(widget)
    lay.setContentsMargins(th.PAD, th.PAD, th.PAD, th.PAD)
    lay.setSpacing(th.GAP)
    return lay


def header(title: str, on_back: Any) -> QHBoxLayout:
    row = QHBoxLayout()
    back = icon_button("back", "Wróć")
    back.clicked.connect(on_back)
    row.addWidget(back)
    row.addWidget(label(title, "title"))
    row.addStretch(1)
    return row


# --- kontroler -------------------------------------------------------------


class App:
    MAIN, DEVICES, SETTINGS, LOG, REPORT = range(5)

    def __init__(self, qt: QApplication, config: dict[str, Any], enabled: bool) -> None:
        self.qt = qt
        self.config = config
        self.settings: dict[str, Any] = {
            "stand_below": config["mapping"]["stand_below"],
            "run_on": config["mapping"]["run_on"],
            "run_off": config["mapping"]["run_off"],
            "quick_stop": bool(config["mapping"].get("quick_stop", True)),
            "output": config["output"]["type"],
            "device_address": config.get("ble", {}).get("device_address", ""),
            "device_name": config.get("ble", {}).get("device_name", ""),
            "simulator": bool(config.get("ui", {}).get("simulator", False)),
            "record": bool(config.get("ui", {}).get("record", False)),
            "always_on_top": bool(config.get("ui", {}).get("always_on_top", False)),
            "floating_button": bool(config.get("ui", {}).get("floating_button", False)),
            "floating_pos": config.get("ui", {}).get("floating_pos"),
            "theme": config.get("ui", {}).get("theme", "system"),
            "large_text": bool(config.get("ui", {}).get("large_text", False)),
            "detailed_logs": bool(config.get("ui", {}).get("detailed_logs", True)),
            "last_version": config.get("ui", {}).get("last_version", ""),
            "walk_key": config["output"].get("walk_key"),
            "run_key": config["output"].get("run_key"),
            "toggle_key": config["output"].get("toggle_key"),
            "belt_control": bool(config.get("ble", {}).get("belt_control", True)),
            "direction": config["output"].get("direction", "keyboard"),
            "idle_speed": float(config.get("ble", {}).get("idle_speed", 1.0)),
            "walk_speed": float(config.get("ble", {}).get("walk_speed", 1.5)),
            "run_speed": float(config.get("ble", {}).get("run_speed", 2.5)),
        }
        if self.settings["direction"] not in ("keyboard", "treadmill"):
            self.settings["direction"] = "keyboard"
        self.key_down = is_key_down  # podmieniane w testach
        self.belt_target: float | None = None  # ostatnia prędkość wysłana z klawiszy
        self.report_endpoint = str(config.get("zgloszenia", {}).get("adres", "")).strip()
        self.tasks: queue.Queue[tuple[str, Any]] = queue.Queue()
        self.update_busy = False
        self.update_available = ""
        self.report_busy = False
        keys = (
            Key.from_dict(self.settings["walk_key"], WALK_DEFAULT),
            Key.from_dict(self.settings["run_key"], RUN_DEFAULT),
        )
        self.bridge = Bridge(
            MappingConfig.from_dict(config["mapping"]), self.settings["output"], enabled, keys
        )
        self.toggle_key = Key.from_dict(self.settings["toggle_key"], TOGGLE_DEFAULT)
        if self.toggle_key.vk == 0:  # zapis sprzed wersji z kodem wirtualnym
            self.toggle_key = TOGGLE_DEFAULT
        self.hotkeys = self.make_hotkeys()
        self.worker = BleWorker()
        self.logs: queue.Queue[str] = queue.Queue()
        self.log_lines: list[str] = []
        logging.getLogger().addHandler(QueueLogHandler(self.logs))
        self.link = Link.IDLE
        self.link_text = ""
        self.devices: list[FoundDevice] = []
        self.auto_connect = not self.settings["device_address"]
        self.stop_requested = False
        self.quitting = False
        self.frame = 0
        Motion.enabled = not th.system_reduces_motion()

        self.window: Window | None = None
        self.build()
        self.apply_source()
        self.apply_recording()

        self.bubble = FloatingToggle(self.t)
        self.bubble.toggled.connect(self.toggle_from_bubble)
        self.bubble.open_requested.connect(self.show_main)
        self.bubble.moved.connect(self.on_bubble_moved)
        self.place_bubble()
        if self.settings["floating_button"]:
            self.bubble.show()

        hints = self.qt.styleHints()
        if hasattr(hints, "colorSchemeChanged"):  # motyw Windows zmieniony w trakcie
            hints.colorSchemeChanged.connect(self.on_system_scheme)

        self.timer = QTimer()
        self.timer.setInterval(TICK_MS)
        self.timer.timeout.connect(self.tick)
        self.timer.start()

        previous = self.settings["last_version"]
        if previous != __version__:
            if previous:
                log.info("Zaktualizowano z %s do %s", previous, __version__)
            self.settings["last_version"] = __version__
            self.save()
        if FROZEN:
            self.run_task(self.check_update_task, False)  # tylko informacja, bez instalowania

        if self.settings["device_address"]:
            self.worker.connect(self.settings["device_address"], self.settings["device_name"])
        elif not self.settings["simulator"]:
            self.worker.scan()  # pierwszy start: od razu szukamy bieżni

    # --- budowa okna --------------------------------------------------------

    def build(self) -> None:
        self.t = th.resolve_theme(self.settings["theme"])
        scale = 1.15 if self.settings["large_text"] else 1.0
        self.qt.setStyleSheet(th.qss(self.t, scale))

        w = Window(self.t, self.on_close)
        self.window = w
        root = QVBoxLayout(w)
        root.setContentsMargins(0, 0, 0, 0)
        self.pages = Pages()
        root.addWidget(self.pages)
        self.pages.addWidget(self.build_main())
        self.pages.addWidget(self.build_devices())
        self.pages.addWidget(self.build_settings())
        self.pages.addWidget(self.build_log())
        self.pages.addWidget(self.build_report())
        self.ring = FocusRing(w, self.t)

        w.setWindowFlag(Qt.WindowType.WindowStaysOnTopHint, self.settings["always_on_top"])
        screen = self.qt.primaryScreen().availableGeometry()
        w.resize(460, min(840, screen.height() - 60))
        w.show()
        w.setFocus(Qt.FocusReason.OtherFocusReason)  # nic nie ma ostrości ani obwódki na starcie
        self.render_link()
        self.render_devices()
        self.log_view.setPlainText("\n".join(self.log_lines))

    def sticky_page(self, title: str) -> tuple[QWidget, QVBoxLayout]:
        """Podstrona: pasek z Wróć zawsze na górze, przewija się tylko treść pod nim."""
        return self.framed_page(header(title, lambda: self.pages.go(self.MAIN, back=True)))

    def framed_page(self, bar_lay: QHBoxLayout) -> tuple[QWidget, QVBoxLayout]:
        """Widok z paskiem przyklejonym u góry (nawigacja w tym samym miejscu na każdym ekranie)."""
        page = QWidget()
        page.setObjectName("page")
        outer = QVBoxLayout(page)
        outer.setContentsMargins(0, 0, 0, 0)
        outer.setSpacing(0)
        bar = QWidget()
        bar.setObjectName("page")
        bar_lay.setContentsMargins(th.PAD, 12, th.PAD, 8)
        bar.setLayout(bar_lay)
        outer.addWidget(bar)
        inner = QWidget()
        lay = page_layout(inner)
        lay.setContentsMargins(th.PAD, 0, th.PAD, th.PAD)
        outer.addWidget(self.scroll(inner), 1)
        return page, lay

    def scroll(self, inner: QWidget) -> QScrollArea:
        area = QScrollArea()
        area.setWidgetResizable(True)
        area.setFrameShape(QScrollArea.Shape.NoFrame)
        area.setHorizontalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOff)
        area.setWidget(inner)
        return area

    def build_main(self) -> QWidget:
        t = self.t
        bar = QHBoxLayout()
        bar.setSpacing(4)
        names = QVBoxLayout()
        names.setSpacing(0)
        names.addWidget(ElidedLabel(APP_NAME, "title"))
        self.update_label = ElidedLabel(f"wersja {__version__}", "small")
        names.addWidget(self.update_label)
        bar.addLayout(names, 1)
        self.update_btn = icon_button("download", "Aktualizuj")
        self.update_btn.clicked.connect(lambda: self.run_task(self.check_update_task, True))
        if not FROZEN:
            self.update_btn.setEnabled(False)
            self.update_btn.setToolTip("Aktualizacje działają w wersji zainstalowanej")
        report_btn = icon_button("report", "Zgłoś problem")
        report_btn.clicked.connect(lambda: self.pages.go(self.REPORT))
        log_btn = icon_button("log", "Dziennik i raport diagnostyczny")
        log_btn.clicked.connect(lambda: self.pages.go(self.LOG))
        settings_btn = icon_button("settings", "Ustawienia")
        settings_btn.clicked.connect(lambda: self.pages.go(self.SETTINGS))
        for b in (self.update_btn, report_btn, log_btn, settings_btn):
            bar.addWidget(b)
        page, lay = self.framed_page(bar)

        # stan połączenia - zawsze w tym miejscu, stała wysokość
        status = card()
        status.setFixedHeight(62)
        srow = QHBoxLayout(status)
        srow.setContentsMargins(14, 8, 10, 8)
        srow.setSpacing(10)
        self.dot = StatusDot(t)
        srow.addWidget(self.dot)
        texts = QVBoxLayout()
        texts.setSpacing(0)
        self.status_title = ElidedLabel("", "")
        self.status_title.setStyleSheet("font-weight: 600;")
        self.status_sub = ElidedLabel("", "muted")
        texts.addStretch(1)
        texts.addWidget(self.status_title)
        texts.addWidget(self.status_sub)
        texts.addStretch(1)
        srow.addLayout(texts, 1)
        self.status_action = button("Szukaj bieżni", "primary")
        self.status_action.clicked.connect(self.on_status_action)
        fix_width(self.status_action, STATUS_ACTIONS)
        srow.addWidget(self.status_action)
        lay.addWidget(status)

        # prędkość
        hero = card()
        hlay = QVBoxLayout(hero)
        hlay.setContentsMargins(14, 10, 14, 12)
        hlay.setSpacing(8)
        self.gauge = Gauge(t)
        self.gauge.max_speed = float(self.config["mapping"]["max_speed"])
        self.gauge.walk_from = float(self.settings["stand_below"])
        self.gauge.run_from = float(self.settings["run_on"])
        hlay.addWidget(self.gauge, 1)
        self.belt = Belt(t)
        hlay.addWidget(self.belt)
        self.spark = Sparkline(t)
        self.spark.max_speed = self.gauge.max_speed
        self.spark.run_from = self.gauge.run_from
        hlay.addWidget(self.spark)
        lay.addWidget(hero)

        # liczby
        self.stat_steps = StatTile("Kroki")
        self.stat_cadence = StatTile("Kroki/min")
        self.stat_distance = StatTile("Dystans")
        self.stat_time = StatTile("Czas ruchu")
        lay.addWidget(
            TileGrid(self.stat_steps, self.stat_cadence, self.stat_distance, self.stat_time)
        )

        # sterowanie
        control = card()
        clay = QVBoxLayout(control)
        clay.setContentsMargins(14, 10, 14, 14)
        clay.setSpacing(8)
        crow = QHBoxLayout()
        ctexts = QVBoxLayout()
        ctexts.setSpacing(0)
        ctexts.addWidget(label("Sterowanie w grze", "title"))
        self.control_subtitle = label("", "muted")
        ctexts.addWidget(self.control_subtitle)
        crow.addLayout(ctexts, 1)
        self.enable_switch = Switch(t, "Sterowanie w grze", self.bridge.state.enabled)
        self.enable_switch.toggled.connect(self.on_enable)
        crow.addWidget(self.enable_switch)
        clay.addLayout(crow)
        self.direction_seg = Segmented(
            [("keyboard", "Klawisze → bieżnia"), ("treadmill", "Bieżnia → gra")],
            self.settings["direction"],
            "Kierunek sterowania",
        )
        self.direction_seg.changed.connect(self.on_direction)
        clay.addWidget(self.direction_seg)
        self.output_seg = Segmented(
            [("dry", "Podgląd"), ("keys", "Klawiatura")],
            self.settings["output"],
            "Wyjście",
        )
        self.output_seg.changed.connect(self.on_output)
        clay.addWidget(self.output_seg)
        walk_key, run_key = self.bridge.keys
        self.walk_btn = KeyButton("Chód", walk_key)
        self.run_btn = KeyButton("Bieg", run_key)
        self.toggle_btn = KeyButton("Start/stop", self.toggle_key)
        self.walk_btn.captured.connect(lambda k: self.on_key("walk", k))
        self.run_btn.captured.connect(lambda k: self.on_key("run", k))
        self.toggle_btn.captured.connect(lambda k: self.on_key("toggle", k))
        clay.addWidget(ResponsiveRow(self.walk_btn, self.run_btn))
        clay.addWidget(self.toggle_btn)
        self.output_hint = label("", "muted")
        self.output_hint.setWordWrap(True)
        self.output_hint.setMinimumHeight(22)
        clay.addWidget(self.output_hint)
        lay.addWidget(control)
        lay.addStretch(1)

        self.apply_direction()
        return page

    def build_devices(self) -> QWidget:
        page = QWidget()
        lay = page_layout(page)
        lay.addLayout(header("Wybierz bieżnię", lambda: self.pages.go(self.MAIN, back=True)))
        self.devices_status = label("", "muted")
        self.devices_status.setFixedHeight(24)
        lay.addWidget(self.devices_status)
        self.devices_box = QVBoxLayout()
        self.devices_box.setSpacing(8)
        self.devices_box.setContentsMargins(0, 0, 0, 0)
        holder = QWidget()
        holder.setObjectName("page")
        holder.setLayout(self.devices_box)
        lay.addWidget(self.scroll(holder), 1)
        rescan = button("Szukaj ponownie", "primary")
        rescan.clicked.connect(self.scan)
        lay.addWidget(rescan)
        return page

    def slider(self, lo: float, hi: float, value: float, name: str) -> tuple[Slider, Any]:
        s = Slider(Qt.Orientation.Horizontal)
        s.setRange(round(lo * 10), round(hi * 10))
        s.setValue(round(value * 10))
        s.setAccessibleName(name)
        s.setFixedHeight(th.CONTROL_H)
        value_label = label(f"{fmt_num(value)} km/h")
        value_label.setMinimumWidth(80)
        value_label.setAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
        s.valueChanged.connect(lambda v: value_label.setText(f"{fmt_num(v / 10)} km/h"))
        return s, value_label

    def setting_row(self, lay: QVBoxLayout, title: str, hint: str, control: QWidget) -> None:
        row = QHBoxLayout()
        texts = QVBoxLayout()
        texts.setSpacing(0)
        texts.addWidget(label(title))
        if hint:
            h = label(hint, "small")
            h.setWordWrap(True)
            texts.addWidget(h)
        row.addLayout(texts, 1)
        row.addWidget(control)
        lay.addLayout(row)

    def build_settings(self) -> QWidget:
        t = self.t
        page, lay = self.sticky_page("Ustawienia")

        speed = card()
        sl = QVBoxLayout(speed)
        sl.setContentsMargins(14, 12, 14, 14)
        sl.addWidget(label("Progi prędkości", "section"))
        max_kmh = float(self.config["mapping"]["max_speed"])
        self.walk_slider, walk_value = self.slider(
            0.0, max_kmh, self.settings["stand_below"], "Chód od"
        )
        self.run_slider, run_value = self.slider(0.0, max_kmh, self.settings["run_on"], "Bieg od")
        for title, s, v in (
            ("Chód od", self.walk_slider, walk_value),
            ("Bieg od", self.run_slider, run_value),
        ):
            row = QHBoxLayout()
            name = label(title)
            name.setMinimumWidth(80)
            row.addWidget(name)
            row.addWidget(s, 1)
            row.addWidget(v)
            sl.addLayout(row)
            s.sliderReleased.connect(lambda sl_=s: self.on_thresholds(sl_))
            s.valueChanged.connect(
                lambda _v, sl_=s: None if sl_.isSliderDown() else self.on_thresholds(sl_)
            )
        note = label(
            f"Bieg wyłącza się {fmt_num(HYSTERESIS)} km/h niżej, żeby Shift nie migał.", "small"
        )
        note.setWordWrap(True)
        sl.addWidget(note)
        quick = Switch(t, "Szybkie zatrzymanie", self.settings["quick_stop"])
        quick.toggled.connect(self.on_quick_stop)
        self.setting_row(
            sl,
            "Szybkie zatrzymanie",
            "Gdy taśma gwałtownie hamuje, postać staje od razu - bez czekania kilku sekund na"
            " pełny stop (bieżnia podaje prędkość co 2 s). Wyłącz, jeśli zmieniasz tempo pilotem"
            " i postać niepotrzebnie się zatrzymuje.",
            quick,
        )
        lay.addWidget(speed)

        belt = card()
        bl = QVBoxLayout(belt)
        bl.setContentsMargins(14, 12, 14, 14)
        bl.addWidget(label("Sterowanie bieżnią", "section"))
        self.belt_switch = Switch(t, "Start i stop bieżni", self.settings["belt_control"])
        self.belt_switch.toggled.connect(self.on_belt_control)
        self.setting_row(
            bl,
            "Start i stop bieżni",
            "Włączenie sterowania uruchamia taśmę, wyłączenie ją zatrzymuje - klawiszem,"
            " przełącznikiem albo przyciskiem na ekranie. Prędkość dalej zmieniasz pilotem."
            " Wyłącz, jeśli wolisz startować tylko pilotem.",
            self.belt_switch,
        )
        for title, key in (
            ("Bez klawisza", "idle_speed"),
            ("Chód", "walk_speed"),
            ("Bieg", "run_speed"),
        ):
            s, v = self.slider(
                kingsmith.SPEED_MIN, kingsmith.SPEED_MAX, self.settings[key], f"Prędkość: {title}"
            )
            row = QHBoxLayout()
            name = label(title)
            name.setMinimumWidth(100)
            row.addWidget(name)
            row.addWidget(s, 1)
            row.addWidget(v)
            bl.addLayout(row)
            s.valueChanged.connect(lambda value, k=key: self.on_belt_speed(k, value / 10))
            setattr(self, f"{key}_slider", s)
        speeds_note = label(
            "Prędkości dla trybu Klawisze → bieżnia: bez klawisza, z klawiszem chodu i z chodem"
            " + biegiem. Taśma przyspiesza i zwalnia sama, płynnie.",
            "small",
        )
        speeds_note.setWordWrap(True)
        bl.addWidget(speeds_note)
        lay.addWidget(belt)

        look = card()
        ll = QVBoxLayout(look)
        ll.setContentsMargins(14, 12, 14, 14)
        ll.addWidget(label("Wygląd i okno", "section"))
        theme_seg = Segmented(
            [("system", "Jak Windows"), ("light", "Jasny"), ("dark", "Ciemny")],
            self.settings["theme"],
            "Motyw",
        )
        theme_seg.changed.connect(lambda v: self.on_look("theme", v))
        ll.addWidget(theme_seg)
        big = Switch(t, "Duży tekst", self.settings["large_text"])
        big.toggled.connect(lambda v: self.on_look("large_text", v))
        self.setting_row(ll, "Duży tekst", "Wszystkie napisy większe o 15%", big)
        top = Switch(t, "Zawsze na wierzchu", self.settings["always_on_top"])
        top.toggled.connect(self.on_always_on_top)
        self.setting_row(ll, "Zawsze na wierzchu", "Okno nie chowa się pod grą", top)
        floating = Switch(t, "Przycisk na ekranie", self.settings["floating_button"])
        floating.toggled.connect(self.on_floating_button)
        self.setting_row(
            ll,
            "Przycisk na ekranie",
            "Mały przycisk zawsze na wierzchu: klik włącza i wyłącza sterowanie, prawy przycisk"
            " otwiera to okno. Nie zabiera ostrości grze.",
            floating,
        )
        lay.addWidget(look)

        tests = card()
        tl = QVBoxLayout(tests)
        tl.setContentsMargins(14, 12, 14, 14)
        tl.addWidget(label("Testy", "section"))
        sim = Switch(t, "Symulator bieżni", self.settings["simulator"])
        sim.toggled.connect(self.on_simulator)
        keys = self.config["hotkeys"]
        self.setting_row(
            tl,
            "Symulator bieżni",
            f"Bez bieżni: {keys['sim_slower']} wolniej, {keys['sim_faster']} szybciej",
            sim,
        )
        rec = Switch(t, "Nagrywaj przejścia", self.settings["record"])
        rec.toggled.connect(self.on_record)
        self.setting_row(tl, "Nagrywaj przejścia", "Odczyty trafiają do folderu nagrania", rec)
        lay.addWidget(tests)

        device = card()
        dl = QVBoxLayout(device)
        dl.setContentsMargins(14, 12, 14, 14)
        dl.addWidget(label("Bieżnia", "section"))
        self.device_label = label("", "muted")
        dl.addWidget(self.device_label)
        self.forget_btn = HoldButton("Zapomnij bieżnię", t)
        self.forget_btn.confirmed.connect(self.forget_device)
        dl.addWidget(self.forget_btn)
        lay.addWidget(device)

        logs = card()
        gl = QVBoxLayout(logs)
        gl.setContentsMargins(14, 12, 14, 14)
        gl.addWidget(label("Dziennik i zgłoszenia", "section"))
        detailed = Switch(t, "Szczegółowe logi Bluetooth", self.settings["detailed_logs"])
        detailed.toggled.connect(self.on_detailed_logs)
        self.setting_row(
            gl,
            "Szczegółowe logi Bluetooth",
            "Zapisuje surową rozmowę z bieżnią (prędkość, kroki, polecenia)."
            " Pomaga naprawić połączenie.",
            detailed,
        )
        what = label(
            "Dziennik zawsze zapisuje zdarzenia aplikacji, błędy i przebieg połączenia z bieżnią. "
            "Nie zapisuje wciskanych klawiszy, haseł ani niczego z gier. Logi zostają na tym "
            "komputerze, dopóki sam nie wyślesz zgłoszenia.",
            "small",
        )
        what.setWordWrap(True)
        gl.addWidget(what)
        open_logs = button("Otwórz folder logów")
        open_logs.clicked.connect(lambda: self.open_folder(Path("logs")))
        report = button("Zgłoś problem")
        report.clicked.connect(lambda: self.pages.go(self.REPORT))
        gl.addWidget(ResponsiveRow(open_logs, report))
        lay.addWidget(logs)

        about = card()
        al = QVBoxLayout(about)
        al.setContentsMargins(14, 12, 14, 14)
        al.addWidget(label("O aplikacji", "section"))
        al.addWidget(label(TITLE))
        al.addWidget(label(f"© 2026 {__author__}", "small"))
        self.about_status = label("", "muted")
        self.about_status.setWordWrap(True)
        self.about_status.setMinimumHeight(24)
        al.addWidget(self.about_status)
        check = button("Sprawdź aktualizacje")
        check.clicked.connect(lambda: self.run_task(self.check_update_task, True))
        check.setEnabled(FROZEN)
        al.addWidget(check)
        link_buttons = []
        for text, url in (
            ("Strona autora", HOMEPAGE),
            ("Prywatność", f"https://github.com/{REPOSITORY}/blob/main/POLITYKA-PRYWATNOSCI.md"),
            ("Kod (GPL-3.0)", f"https://github.com/{REPOSITORY}"),
        ):
            b = button(text)
            b.clicked.connect(lambda _=False, u=url: QDesktopServices.openUrl(QUrl(u)))
            link_buttons.append(b)
        al.addWidget(ResponsiveRow(*link_buttons))
        lay.addWidget(about)
        lay.addStretch(1)
        return page

    def build_report(self) -> QWidget:
        page, lay = self.sticky_page("Zgłoś problem")
        intro = label(
            "Opisz, co się stało. Razem z opisem trafią logi, żeby autor mógł szybko znaleźć "
            "przyczynę.",
            "muted",
        )
        intro.setWordWrap(True)
        lay.addWidget(intro)
        lay.addWidget(label("Co się stało?"))
        self.report_text = QPlainTextEdit()
        self.report_text.setPlaceholderText(
            "Np. bieżnia rozłącza się po kilku minutach albo postać nie biegnie w grze."
        )
        self.report_text.setAccessibleName("Opis problemu")
        self.report_text.setMinimumHeight(120)
        lay.addWidget(self.report_text)
        lay.addWidget(label("Kontakt (opcjonalnie)"))
        self.report_contact = QLineEdit()
        self.report_contact.setPlaceholderText("Np. nick na Discordzie albo e-mail")
        self.report_contact.setAccessibleName("Kontakt")
        self.report_contact.setFixedHeight(th.CONTROL_H)
        lay.addWidget(self.report_contact)
        box = card()
        bl = QVBoxLayout(box)
        bl.setContentsMargins(14, 12, 14, 14)
        bl.addWidget(label("Co zostanie dołączone", "section"))
        for line in (
            "- logi aplikacji i Bluetooth (ostatnie wpisy),",
            "- ostatni raport diagnostyczny, jeśli go zapisano,",
            "- wersja aplikacji i Windows, ustawienia (progi, wybrana bieżnia).",
            "Bez haseł, plików z komputera i wciśniętych klawiszy.",
        ):
            item = label(line, "small")
            item.setWordWrap(True)
            bl.addWidget(item)
        lay.addWidget(box)
        self.report_status = label("", "muted")
        self.report_status.setWordWrap(True)
        self.report_status.setMinimumHeight(40)
        lay.addWidget(self.report_status)
        text = "Wyślij zgłoszenie" if self.report_endpoint else "Przygotuj e-mail z logami"
        self.report_send = button(text, "primary")
        self.report_send.clicked.connect(self.send_report)
        lay.addWidget(self.report_send)
        lay.addStretch(1)
        return page

    def build_log(self) -> QWidget:
        page = QWidget()
        lay = page_layout(page)
        lay.addLayout(header("Dziennik", lambda: self.pages.go(self.MAIN, back=True)))
        hint = label(
            "Gdy coś nie działa: zapisz raport diagnostyczny i wyślij plik z folderu logs.",
            "muted",
        )
        hint.setWordWrap(True)
        lay.addWidget(hint)
        self.log_view = QPlainTextEdit()
        self.log_view.setReadOnly(True)
        self.log_view.setAccessibleName("Dziennik zdarzeń")
        self.log_view.setMaximumBlockCount(500)
        lay.addWidget(self.log_view, 1)
        self.diag_status = label("", "muted")
        self.diag_status.setFixedHeight(24)
        lay.addWidget(self.diag_status)
        folder = button("Otwórz folder logów")
        folder.clicked.connect(lambda: self.open_folder(Path("logs")))
        report = button("Zapisz raport diagnostyczny", "primary")
        report.clicked.connect(self.diagnose)
        lay.addWidget(ResponsiveRow(folder, report))
        return page

    # --- akcje --------------------------------------------------------------

    def save(self) -> None:
        with contextlib.suppress(OSError):
            save_settings(self.settings)

    def change(self, key: str, value: Any) -> None:
        self.settings[key] = value
        self.save()

    def apply_look(self) -> None:
        """Motyw i wielkość tekstu na żywo - bez przebudowy okna i bez zmiany widoku."""
        self.t = th.resolve_theme(self.settings["theme"])
        scale = 1.15 if self.settings["large_text"] else 1.0
        self.qt.setStyleSheet(th.qss(self.t, scale))
        if self.window is None:
            return
        for widget in [self.window, *self.window.findChildren(QWidget)]:
            if hasattr(widget, "retheme"):
                widget.retheme(self.t)
            elif hasattr(widget, "t"):
                widget.t = self.t
                widget.update()
        fix_width(self.status_action, STATUS_ACTIONS)  # duży tekst zmienia szerokość napisów
        for key_button in (self.walk_btn, self.run_btn):
            key_button.updateGeometry()
        self.bubble.retheme(self.t)
        self.render_link()
        self.render_update_label()

    def on_look(self, key: str, value: Any) -> None:
        self.change(key, value)
        self.apply_look()

    def on_system_scheme(self, *_: Any) -> None:
        if self.settings["theme"] == "system":
            self.apply_look()

    def on_always_on_top(self, on: bool) -> None:
        self.change("always_on_top", on)
        w = self.window
        if w is None:
            return
        if sys.platform == "win32" and self.qt.platformName() == "windows":
            if not set_topmost(int(w.winId()), on):
                log.warning("Nie udało się zmienić ustawienia Zawsze na wierzchu")
        else:
            w.setWindowFlag(Qt.WindowType.WindowStaysOnTopHint, on)
            w.show()

    # --- przycisk na ekranie --------------------------------------------------

    def on_floating_button(self, on: bool) -> None:
        self.change("floating_button", on)
        if on:
            self.place_bubble()
            self.bubble.show()
        else:
            self.bubble.hide()

    def place_bubble(self) -> None:
        pos = self.settings.get("floating_pos")
        if isinstance(pos, list) and len(pos) == 2:
            point = QPoint(int(pos[0]), int(pos[1]))
            if QGuiApplication.screenAt(point + QPoint(28, 28)) is not None:
                self.bubble.move(point)
                return
        area = self.qt.primaryScreen().availableGeometry()  # domyślnie: prawa krawędź, środek
        self.bubble.move(area.right() - FloatingToggle.SIZE - 24, area.center().y())

    def on_bubble_moved(self, point: QPoint) -> None:
        self.change("floating_pos", [point.x(), point.y()])

    def toggle_from_bubble(self) -> None:
        self.set_control(not self.bridge.state.enabled)

    def set_control(self, on: bool) -> None:
        """Jedno miejsce dla skrótu, przełącznika i przycisku na ekranie.

        Wyłączenie najpierw puszcza klawisze (postać staje od razu), potem zatrzymuje taśmę.
        """
        changed = on != self.bridge.state.enabled
        self.bridge.set_enabled(on)
        self.enable_switch.set_silently(on)
        save_enabled(on)
        self.belt_target = None  # po zmianie prędkość z klawiszy idzie od nowa
        keyboard = self.settings["direction"] == "keyboard" and not self.settings["simulator"]
        if changed and not on and keyboard and not self.settings["belt_control"]:
            self.worker.set_speed(self.settings["idle_speed"])  # bez stopu: zwolnij
        if changed and self.belt_commands_possible():
            if not on:
                self.worker.belt("stop")
            elif self.bridge.state.control.speed > 0:
                log.info("Taśma już jedzie - nie wysyłam startu")
            else:
                self.worker.belt("start")
                self.render_output_hint("Uruchamiam bieżnię…")

    def belt_commands_possible(self) -> bool:
        return bool(self.settings["belt_control"]) and not self.settings["simulator"]

    def on_belt_control(self, on: bool) -> None:
        self.change("belt_control", on)
        self.render_control_subtitle()

    def render_control_subtitle(self) -> None:
        key = self.toggle_key.name
        if self.settings["direction"] == "keyboard":
            start = (
                "uruchamia i zatrzymuje" if self.settings["belt_control"] else "włącza i wyłącza"
            )
            text = f"{key} {start}, {self.bridge.keys[0].name} przyspiesza taśmę"
        elif self.settings["belt_control"]:
            text = f"{key} uruchamia i zatrzymuje bieżnię - działa też w grze"
        else:
            text = f"{key} włącza i wyłącza - działa też w grze"
        self.control_subtitle.setText(text)

    def on_direction(self, kind: str) -> None:
        self.change("direction", kind)
        log.info("Kierunek sterowania: %s", kind)
        self.apply_direction()

    def apply_direction(self) -> None:
        """Klawisze -> bieżnia: aplikacja niczego nie naciska, tylko słucha klawiszy gracza."""
        keyboard = self.settings["direction"] == "keyboard"
        self.bridge.set_output("dry" if keyboard else self.settings["output"])
        self.output_seg.setVisible(not keyboard)
        self.belt_target = None
        self.render_control_subtitle()
        self.render_output_hint()

    def on_belt_speed(self, key: str, kmh: float) -> None:
        self.change(key, round(kmh, 1))
        self.belt_target = None  # zmieniona prędkość trafia na taśmę od razu
        self.render_output_hint()

    def keyboard_target(self) -> tuple[float, str]:
        """Prędkość taśmy z trzymanych klawiszy: chód + bieg, sam chód albo nic."""
        walk, run = self.bridge.keys
        walk_down = self.key_down(walk.vk or vk_from_scan(walk.scan, walk.extended))
        run_down = self.key_down(run.vk or vk_from_scan(run.scan, run.extended))
        if walk_down and run_down:
            return self.settings["run_speed"], "run"
        if walk_down:
            return self.settings["walk_speed"], "walk"
        return self.settings["idle_speed"], "idle"

    def make_hotkeys(self) -> Hotkeys:
        keys = self.config["hotkeys"]
        return Hotkeys(
            {
                "toggle": self.toggle_key.vk,
                "sim_slower": keys["sim_slower"],
                "sim_faster": keys["sim_faster"],
            }
        )

    def on_belt_event(self, result: str) -> None:
        messages = {
            "started": "Bieżnia rusza. Prędkość zmieniasz pilotem.",
            "stopped": "Bieżnia zatrzymana.",
            "refused": "Bieżnia nie przyjęła startu. Obudź ją pilotem albo sprawdź, czy klucz"
            " bezpieczeństwa jest na miejscu, i spróbuj jeszcze raz.",
            "unknown": "Wysłano start, ale bieżnia nie odpowiedziała. Jeśli taśma nie rusza,"
            " uruchom ją pilotem.",
            "unsupported": "Ta bieżnia nie obsługuje startu z aplikacji - użyj pilota.",
            "not_connected": "Brak połączenia z bieżnią - start i stop tylko pilotem.",
        }
        log.info("Taśma: %s", result)
        self.render_output_hint(messages.get(result, ""))

    def show_main(self) -> None:
        w = self.window
        if w is None:
            return
        w.showNormal()
        w.raise_()
        w.activateWindow()

    def render_update_label(self) -> None:
        style = f"color: {self.t.accent}; font-weight: 600;" if self.update_available else ""
        self.update_label.setStyleSheet(style)
        self.update_btn.setStyleSheet(f"color: {self.t.accent};" if self.update_available else "")
        if self.update_available:
            self.update_btn.setToolTip(f"Aktualizuj do wersji {self.update_available}")

    def on_enable(self, on: bool) -> None:
        self.set_control(on)

    def on_output(self, kind: str) -> None:
        self.bridge.set_output(kind)
        self.settings["output"] = kind
        self.save()
        self.render_output_hint()

    def render_output_hint(self, problem_text: str = "") -> None:
        walk, run = self.bridge.keys
        if problem_text:
            text = problem_text
        elif self.settings["direction"] == "keyboard":
            s = self.settings
            text = (
                f"{walk.name}: {fmt_num(s['walk_speed'])} km/h, {walk.name} + {run.name}:"
                f" {fmt_num(s['run_speed'])} km/h, bez klawisza: {fmt_num(s['idle_speed'])} km/h."
            )
        elif self.settings["output"] == "keys":
            text = f"Gra dostaje {walk.name} i {run.name} + {walk.name} (aktywne okno)."
        else:
            text = "Podgląd: nic nie jest naciskane."
        self.output_hint.setText(text)

    def on_key(self, role: str, key: Key) -> None:
        walk, run = self.bridge.keys
        toggle = self.toggle_key
        if role == "walk":
            walk = key
        elif role == "run":
            run = key
        else:
            toggle = key
        error = problem(walk, run, toggle)
        if error:
            self.render_output_hint(error)
            return
        self.bridge.set_keys(walk, run)
        self.walk_btn.set_key(walk)
        self.run_btn.set_key(run)
        self.toggle_btn.set_key(toggle)
        self.settings["walk_key"] = walk.to_dict()
        self.settings["run_key"] = run.to_dict()
        if toggle != self.toggle_key:
            self.toggle_key = toggle
            self.settings["toggle_key"] = toggle.to_dict()
            self.hotkeys = self.make_hotkeys()  # trzymany właśnie klawisz się nie liczy
        self.render_control_subtitle()
        self.save()
        self.render_output_hint()

    def on_thresholds(self, moved: Any = None) -> None:
        """Progi chodu i biegu. Ruszony suwak wygrywa, drugi przesuwa się o odstęp."""
        top = float(self.config["mapping"]["max_speed"])
        gap = HYSTERESIS + 0.2  # bieg wyłącza się 0,6 niżej i nadal musi być nad chodem
        walk = self.walk_slider.value() / 10
        run = self.run_slider.value() / 10
        if run < walk + gap:
            if moved is self.walk_slider:
                run = min(top, walk + gap)
                walk = min(walk, run - gap)
            else:
                walk = max(0.0, run - gap)
                run = max(run, walk + gap)
        walk, run = round(walk, 1), round(run, 1)
        for slider, value in ((self.walk_slider, walk), (self.run_slider, run)):
            if slider.value() != round(value * 10):
                slider.blockSignals(True)
                slider.setValue(round(value * 10))
                slider.blockSignals(False)
                slider.valueChanged.emit(slider.value())  # odśwież napis obok suwaka
        try:
            mapping = MappingConfig.from_dict(
                {
                    **self.config["mapping"],
                    "stand_below": walk,
                    "run_on": run,
                    "run_off": run - HYSTERESIS,
                    "quick_stop": self.settings["quick_stop"],
                }
            )
        except ValueError as exc:
            log.warning("Progi: %s", exc)
            return
        self.bridge.set_mapping(mapping)
        self.settings.update(stand_below=walk, run_on=run, run_off=run - HYSTERESIS)
        self.gauge.walk_from, self.gauge.run_from = walk, run
        self.spark.run_from = run
        self.gauge.update()
        self.save()

    def on_quick_stop(self, on: bool) -> None:
        self.settings["quick_stop"] = on
        self.on_thresholds()
        log.info("Szybkie zatrzymanie %s", "włączone" if on else "wyłączone")

    def apply_source(self) -> None:
        if self.settings["simulator"]:
            self.bridge.set_source(SimulatedTreadmill())
        else:
            self.bridge.set_source(QueueSource(self.worker.readings))

    def on_simulator(self, on: bool) -> None:
        self.settings["simulator"] = on
        self.save()
        self.apply_source()
        log.info("Symulator %s", "włączony" if on else "wyłączony")
        self.render_link()

    def apply_recording(self) -> None:
        recorder = Recorder(Path("nagrania")) if self.settings["record"] else None
        self.bridge.set_recording(recorder)
        if recorder:
            log.info("Nagrywam do %s", recorder.path)

    def on_record(self, on: bool) -> None:
        self.settings["record"] = on
        self.save()
        self.apply_recording()

    def scan(self) -> None:
        self.devices = []
        self.render_devices(scanning=True)
        self.pages.go(self.DEVICES)
        self.worker.scan()

    def connect_device(self, device: FoundDevice) -> None:
        self.settings["device_address"] = device.address
        self.settings["device_name"] = device.name
        self.save()
        self.worker.connect(device.address, device.name)
        self.pages.go(self.MAIN, back=True)
        log.info("Wybrano bieżnię %s", device.name)

    def forget_device(self) -> None:
        self.worker.disconnect()
        self.settings["device_address"] = self.settings["device_name"] = ""
        self.save()
        self.auto_connect = True
        log.info("Bieżnia zapomniana")
        self.render_link()

    def on_status_action(self) -> None:
        if self.settings["simulator"]:
            self.on_simulator(False)
        elif self.link in (Link.STREAMING, Link.CONNECTED, Link.CONNECTING, Link.RETRYING) or (
            self.link is Link.SEARCHING and self.settings["device_address"]
        ):
            self.worker.disconnect()
        elif self.settings["device_address"] and self.link is Link.BLUETOOTH_OFF:
            self.worker.connect(self.settings["device_address"], self.settings["device_name"])
        else:
            self.scan()

    def diagnose(self) -> None:
        path = Path("logs") / f"diagnostyka-{datetime.now():%Y%m%d-%H%M%S}.txt"
        self.diag_status.setText("Zbieram raport (ok. 30 s)…")
        self.worker.diagnose(path)

    def open_folder(self, path: Path) -> None:
        path.mkdir(exist_ok=True)
        if sys.platform == "win32":
            os.startfile(path.resolve())

    # --- zadania w tle (sieć) -----------------------------------------------

    def run_task(self, func: Any, *args: Any) -> None:
        threading.Thread(target=func, args=args, daemon=True, name=func.__name__).start()

    def check_update_task(self, install: bool) -> None:
        """W wątku: sprawdza (i na żądanie pobiera) aktualizację. Wynik idzie do kolejki."""
        if install:
            if self.update_busy:
                return
            self.update_busy = True
            self.tasks.put(("update_status", "Sprawdzam aktualizacje…"))
        try:
            release = aktualizacje.fetch_latest()
            if not aktualizacje.is_newer(release.version):
                self.tasks.put(("update_current", install))
                return
            if not install:
                self.tasks.put(("update_available", release.version))
                return

            def progress(done: int, total: int) -> None:
                if total:
                    percent = done * 100 // total
                    self.tasks.put(("update_status", f"Pobieram {release.version}: {percent}%"))

            installer = aktualizacje.download(release, progress=progress)
            self.tasks.put(("update_ready", (installer, release.version)))
        except aktualizacje.UpdateError as exc:
            self.tasks.put(("update_error", (str(exc), install)))
        finally:
            if install:
                self.update_busy = False

    def send_report(self) -> None:
        if self.report_busy:
            return
        description = self.report_text.toPlainText().strip()
        if len(description) < 5:
            self.report_status.setText("Napisz w kilku słowach, co się stało.")
            return
        self.report_busy = True
        self.report_send.setEnabled(False)
        self.report_status.setText("Przygotowuję paczkę z logami…")
        self.run_task(self.report_task, description, self.report_contact.text())

    def report_task(self, description: str, contact: str) -> None:
        try:
            report = zgloszenia.build_report(description, contact)
            if self.report_endpoint:
                zgloszenia.send_report(report, self.report_endpoint, description, contact)
                self.tasks.put(("report_sent", "Wysłano - dziękuję! Autor dostał opis i logi."))
            else:
                self.tasks.put(("report_email", report))
        except zgloszenia.ReportError as exc:
            self.tasks.put(("report_error", str(exc)))
        except OSError as exc:
            self.tasks.put(("report_error", f"Nie udało się przygotować paczki: {exc}"))

    def handle_task(self, kind: str, data: Any) -> None:
        if kind == "update_status":
            self.update_label.setText(data)
            self.about_status.setText(data)
        elif kind == "update_available":
            self.update_available = data
            self.update_label.setText(f"Dostępna wersja {data}")
            self.render_update_label()
            self.about_status.setText(f"Dostępna nowa wersja {data} - kliknij Aktualizuj.")
        elif kind == "update_current":
            if data:  # tylko po kliknięciu - ciche sprawdzenie przy starcie nic nie mówi
                self.update_label.setText("Masz najnowszą wersję")
                self.about_status.setText(f"Masz najnowszą wersję ({__version__}).")
        elif kind == "update_error":
            message, clicked = data
            if clicked:
                self.update_label.setText("Aktualizacja nieudana")
                self.about_status.setText(message)
            log.warning("Aktualizacja: %s", message)
        elif kind == "update_ready":
            installer, version = data
            self.update_label.setText(f"Instaluję {version} - uruchomię się ponownie")
            log.info("Instaluję wersję %s", version)
            try:
                aktualizacje.run_installer(installer)
            except (aktualizacje.UpdateError, OSError) as exc:
                self.update_label.setText("Nie udało się uruchomić instalatora")
                log.error("Instalator: %s", exc)
                return
            QTimer.singleShot(400, self.quit)
        elif kind == "report_sent":
            self.report_busy = False
            self.report_send.setEnabled(True)
            self.report_status.setText(data)
            self.report_text.clear()
        elif kind == "report_email":
            self.report_busy = False
            self.report_send.setEnabled(True)
            self.report_status.setText(
                f"Gotowe: otworzył się e-mail do autora i folder z paczką {data.name}. "
                "Przeciągnij plik do wiadomości i wyślij."
            )
            zgloszenia.open_email(self.report_text.toPlainText(), data)
        elif kind == "report_error":
            self.report_busy = False
            self.report_send.setEnabled(True)
            self.report_status.setText(data)

    def on_detailed_logs(self, on: bool) -> None:
        set_detailed(on)
        self.change("detailed_logs", on)
        log.info("Szczegółowe logi Bluetooth %s", "włączone" if on else "wyłączone")

    # --- stan połączenia ----------------------------------------------------

    def render_link(self) -> None:
        if self.window is None:
            return
        t, name = self.t, self.settings["device_name"] or "bieżnia"
        primary = False
        if self.settings["simulator"]:
            keys = self.config["hotkeys"]
            color, busy, title = t.accent, False, "Symulator bieżni"
            sub, action = (
                f"{keys['sim_slower']} wolniej, {keys['sim_faster']} szybciej",
                "Wyłącz symulator",
            )
        elif self.link is Link.STREAMING:
            stale = not self.bridge.state.live
            color, busy = (t.run if stale else t.accent), stale
            title = f"Połączono: {name}"
            sub = "Czekam na dane z bieżni" if stale else self.link_text
            action = "Rozłącz"
        elif self.link in (Link.SEARCHING, Link.CONNECTING, Link.CONNECTED):
            color, busy, title, sub = (
                t.accent,
                True,
                self.link_text,
                "Bieżnia włączona, telefon rozłączony",
            )
            action = "Anuluj" if self.settings["device_address"] else "Szukaj bieżni"
        elif self.link is Link.RETRYING:
            color, busy, title, sub, action = (
                t.run,
                True,
                "Ponawiam połączenie",
                self.link_text,
                "Rozłącz",
            )
        elif self.link is Link.BLUETOOTH_OFF:
            color, busy, title = t.danger, False, "Bluetooth wyłączony"
            sub, action, primary = "Włącz Bluetooth w Windows", "Spróbuj ponownie", True
        elif self.link is Link.UNSUPPORTED:
            color, busy, title = t.danger, False, "To urządzenie nie jest bieżnią"
            sub, action, primary = "Wybierz inne albo zapisz raport", "Szukaj bieżni", True
        else:
            color, busy, title = t.text_2, False, "Bieżnia nie połączona"
            sub, action, primary = "Włącz bieżnię i kliknij Szukaj bieżni", "Szukaj bieżni", True
        self.dot.set_state(color, busy)
        if self.status_title.text() != title:
            self.status_title.setText(title)
        if self.status_sub.text() != sub:
            self.status_sub.setText(sub)
        if self.status_action.text() != action:
            self.status_action.setText(action)
            self.status_action.setAccessibleName(action)
        kind = "primary" if primary else ""
        if self.status_action.objectName() != kind:
            self.status_action.setObjectName(kind)
            self.status_action.style().unpolish(self.status_action)
            self.status_action.style().polish(self.status_action)
        self.forget_btn.setEnabled(bool(self.settings["device_address"]))
        self.device_label.setText(
            f"Zapamiętana: {self.settings['device_name']}"
            if self.settings["device_address"]
            else "Brak zapamiętanej bieżni"
        )

    def render_devices(self, scanning: bool = False) -> None:
        while self.devices_box.count():
            item = self.devices_box.takeAt(0)
            if item is not None and item.widget() is not None:
                item.widget().deleteLater()  # type: ignore[union-attr]
        if scanning:
            self.devices_status.setText("Szukam urządzeń Bluetooth (ok. 6 s)…")
            self.devices_box.addStretch(1)
            return
        treadmills = [d for d in self.devices if d.treadmill]
        others = [d for d in self.devices if not d.treadmill]
        self.devices_status.setText(
            f"Znalezione bieżnie: {len(treadmills)}, inne urządzenia: {len(others)}"
            if self.devices
            else ""
        )
        if not treadmills:
            empty = card()
            el = QVBoxLayout(empty)
            el.setContentsMargins(14, 12, 14, 14)
            steps: tuple[str, ...]
            if self.link is Link.BLUETOOTH_OFF:
                title, steps = (
                    "Bluetooth jest wyłączony",
                    (
                        "1. Otwórz Start > Ustawienia > Bluetooth i urządzenia.",
                        "2. Włącz Bluetooth.",
                        "3. Wróć tutaj i kliknij Szukaj ponownie.",
                    ),
                )
            else:
                title, steps = (
                    "Nie widzę bieżni",
                    (
                        "1. Włącz bieżnię (pilotem albo przyciskiem).",
                        "2. Rozłącz telefon: zamknij KS Fit albo wyłącz w nim Bluetooth."
                        " Bieżnia przyjmuje jedno połączenie naraz.",
                        "3. Sprawdź, czy Bluetooth w Windows jest włączony.",
                        "4. Kliknij Szukaj ponownie.",
                    ),
                )
            el.addWidget(label(title, "title"))
            for line in steps:
                step = label(line, "muted")
                step.setWordWrap(True)
                el.addWidget(step)
            self.devices_box.addWidget(empty)
        for device in treadmills:
            row = DeviceRow(device.name, f"Bieżnia · sygnał {device.rssi} dBm", self.t)
            row.clicked.connect(lambda _=False, d=device: self.connect_device(d))
            self.devices_box.addWidget(row)
        if others:
            self.devices_box.addWidget(label("Inne urządzenia w pobliżu", "section"))
            for device in others[:12]:
                row = DeviceRow(device.name, f"sygnał {device.rssi} dBm", self.t)
                row.clicked.connect(lambda _=False, d=device: self.connect_device(d))
                self.devices_box.addWidget(row)
        self.devices_box.addStretch(1)

    def handle_event(self, event: BleEvent) -> None:
        if event.kind == "link" and event.link is not None:
            self.link, self.link_text = event.link, event.text
            self.render_link()
        elif event.kind == "devices":
            self.devices = list(event.devices)
            treadmills = [d for d in self.devices if d.treadmill]
            if self.auto_connect and len(treadmills) == 1:
                self.auto_connect = False
                log.info("Znaleziono jedną bieżnię - łączę automatycznie")
                self.connect_device(treadmills[0])
            else:
                self.auto_connect = False
                self.render_devices()
                self.pages.go(self.DEVICES)
        elif event.kind == "scan_failed":
            if event.link is not None:
                self.link, self.link_text = event.link, event.text
                self.render_link()
            self.auto_connect = False
            self.render_devices()
            self.pages.go(self.DEVICES)
            self.devices_status.setText(event.text)
        elif event.kind == "belt":
            self.on_belt_event(event.text)
        elif event.kind == "report":
            self.diag_status.setText(f"Raport: {event.text}")
            log.info("Raport diagnostyczny: %s", event.text)

    # --- pętla --------------------------------------------------------------

    def tick(self) -> None:
        if self.stop_requested:
            self.stop_requested = False
            self.quit()
            return
        while True:
            try:
                line = self.logs.get_nowait()
            except queue.Empty:
                break
            self.log_lines = [*self.log_lines, line][-500:]
            self.log_view.appendPlainText(line)
        while True:
            try:
                self.handle_event(self.worker.events.get_nowait())
            except queue.Empty:
                break
        while True:
            try:
                self.handle_task(*self.tasks.get_nowait())
            except queue.Empty:
                break

        actions = self.hotkeys.poll()
        if "toggle" in actions:
            actions.discard("toggle")
            if not any(b.capturing for b in (self.walk_btn, self.run_btn, self.toggle_btn)):
                self.set_control(not self.bridge.state.enabled)
        state = self.bridge.tick(time.monotonic(), actions)

        c = state.control
        keyboard = self.settings["direction"] == "keyboard"
        capturing = any(b.capturing for b in (self.walk_btn, self.run_btn, self.toggle_btn))
        target, held = self.keyboard_target() if keyboard else (0.0, "idle")
        sends = keyboard and state.enabled and not capturing and not self.settings["simulator"]
        if sends and target != self.belt_target:
            self.belt_target = target
            self.worker.set_speed(target)
        if not state.live and not self.settings["simulator"]:
            mode, caption = (
                "offline",
                "Brak danych" if self.link is not Link.STREAMING else "Czekam na dane",
            )
        elif keyboard:
            walk_key, run_key = self.bridge.keys
            mode = held if state.enabled else "idle"
            caption = {
                "run": f"{walk_key.name} + {run_key.name} - taśma {fmt_num(target)} km/h",
                "walk": f"{walk_key.name} - taśma {fmt_num(target)} km/h",
                "idle": f"Bez klawisza - taśma {fmt_num(target)} km/h",
            }[held]
            if not state.enabled:
                caption = f"Wyłączone - {self.toggle_key.name} włącza"
        elif c.run:
            walk_key, run_key = self.bridge.keys
            mode, caption = "run", f"Bieg - {run_key.name} + {walk_key.name}"
        elif c.walk:
            mode, caption = "walk", f"Chód - {self.bridge.keys[0].name}"
        else:
            mode, caption = "idle", "Stoisz"
        if state.enabled and not keyboard and self.settings["output"] == "dry" and c.walk:
            caption += " (podgląd)"
        self.gauge.set_speed(c.speed, mode, caption)
        self.bubble.set_state(
            state.enabled, mode, c.speed, state.live or self.settings["simulator"]
        )
        self.belt.set_speed(c.speed)

        self.frame += 1
        if self.frame % 5 == 0:  # liczby i wykres 4x na sekundę wystarczą
            self.stat_steps.set(str(state.steps) if state.steps is not None else "-")
            self.stat_cadence.set(f"{state.cadence:.0f}" if state.cadence else "-")
            self.stat_distance.set(f"{fmt_num(state.distance_km, 2)} km")
            minutes, seconds = divmod(int(state.active_s), 60)
            self.stat_time.set(f"{minutes}:{seconds:02d}")
            self.spark.set_points(list(state.history))
            self.render_link()

    def request_stop(self, *_args: Any) -> None:
        self.stop_requested = True  # wywoływane z obsługi sygnału - właściwe zamknięcie w tick()

    def on_close(self) -> bool:
        self.quit()
        return True

    def quit(self) -> None:
        if self.quitting:
            return
        self.quitting = True
        self.timer.stop()
        self.bubble.close()
        was_enabled = self.bridge.state.enabled
        self.bridge.shutdown()
        if was_enabled and self.belt_commands_possible() and self.worker.stop_belt_now():
            log.info("Zamykanie: zatrzymano taśmę")
        self.worker.shutdown()
        self.save()
        self.qt.quit()


def run_gui(config: dict[str, Any], enabled: bool) -> int:
    qt = QApplication.instance() or QApplication(sys.argv)
    assert isinstance(qt, QApplication)
    qt.setApplicationName(APP_NAME)
    qt.setApplicationDisplayName(TITLE)  # równe tytułowi okna - Qt nie dopisze go drugi raz
    if sys.platform == "win32":
        with contextlib.suppress(AttributeError, OSError):
            import ctypes

            ctypes.windll.shell32.SetCurrentProcessExplicitAppUserModelID("Bieznia.App")
    app = App(qt, config, enabled)
    log.info("Okno gotowe po %.1f s", time.monotonic() - STARTED)
    signal.signal(signal.SIGINT, app.request_stop)
    if hasattr(signal, "SIGBREAK"):  # tak zamyka nas nadzorca przed aktualizacją
        signal.signal(signal.SIGBREAK, app.request_stop)
    qt.exec()
    return 0
