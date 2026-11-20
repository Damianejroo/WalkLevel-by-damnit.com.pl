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
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path
from typing import Any

from PySide6.QtCore import QPoint, QPropertyAnimation, Qt, QTimer
from PySide6.QtGui import QCloseEvent, QColor, QFont, QIcon, QPainter, QPaintEvent, QPixmap
from PySide6.QtWidgets import (
    QApplication,
    QHBoxLayout,
    QPlainTextEdit,
    QScrollArea,
    QSlider,
    QStackedWidget,
    QVBoxLayout,
    QWidget,
)

from bieznia import __version__
from bieznia.ble.worker import BleEvent, BleWorker, FoundDevice, Link
from bieznia.engine import Bridge
from bieznia.gui import theme as th
from bieznia.gui.theme import Theme
from bieznia.gui.widgets import (
    Belt,
    DeviceRow,
    FocusRing,
    Gauge,
    HoldButton,
    Motion,
    Segmented,
    Sparkline,
    StatTile,
    StatusDot,
    Switch,
    button,
    card,
    ease,
    fmt_num,
    icon_button,
    label,
)
from bieznia.hotkeys import Hotkeys
from bieznia.mapping import MappingConfig
from bieznia.recorder import Recorder
from bieznia.settings import save_enabled, save_settings
from bieznia.sources.ble_source import QueueSource
from bieznia.sources.simulator import SimulatedTreadmill

log = logging.getLogger("bieznia")

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
        self.setWindowTitle("Bieżnia")
        self.setWindowIcon(app_icon(t))
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        self.setMinimumSize(420, 640)

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
    MAIN, DEVICES, SETTINGS, LOG = range(4)

    def __init__(self, qt: QApplication, config: dict[str, Any], enabled: bool) -> None:
        self.qt = qt
        self.config = config
        self.settings: dict[str, Any] = {
            "stand_below": config["mapping"]["stand_below"],
            "run_on": config["mapping"]["run_on"],
            "run_off": config["mapping"]["run_off"],
            "output": config["output"]["type"],
            "device_address": config.get("ble", {}).get("device_address", ""),
            "device_name": config.get("ble", {}).get("device_name", ""),
            "simulator": bool(config.get("ui", {}).get("simulator", False)),
            "record": bool(config.get("ui", {}).get("record", False)),
            "always_on_top": bool(config.get("ui", {}).get("always_on_top", False)),
            "theme": config.get("ui", {}).get("theme", "system"),
            "large_text": bool(config.get("ui", {}).get("large_text", False)),
        }
        self.bridge = Bridge(
            MappingConfig.from_dict(config["mapping"]), self.settings["output"], enabled
        )
        self.hotkeys = Hotkeys(config["hotkeys"])
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

        self.timer = QTimer()
        self.timer.setInterval(TICK_MS)
        self.timer.timeout.connect(self.tick)
        self.timer.start()

        if self.settings["device_address"]:
            self.worker.connect(self.settings["device_address"], self.settings["device_name"])
        elif not self.settings["simulator"]:
            self.worker.scan()  # pierwszy start: od razu szukamy bieżni

    # --- budowa okna --------------------------------------------------------

    def build(self) -> None:
        old = self.window
        geometry = old.saveGeometry() if old else None
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
        self.ring = FocusRing(w, self.t)

        w.setWindowFlag(Qt.WindowType.WindowStaysOnTopHint, self.settings["always_on_top"])
        if geometry is not None:
            w.restoreGeometry(geometry)
        else:
            screen = self.qt.primaryScreen().availableGeometry()
            w.resize(460, min(800, screen.height() - 60))
        if old is not None:
            old.on_close = lambda: True
            old.close()
            old.deleteLater()
        w.show()
        self.render_link()
        self.render_devices()
        self.log_view.setPlainText("\n".join(self.log_lines))

    def scroll(self, inner: QWidget) -> QScrollArea:
        area = QScrollArea()
        area.setWidgetResizable(True)
        area.setFrameShape(QScrollArea.Shape.NoFrame)
        area.setWidget(inner)
        return area

    def build_main(self) -> QWidget:
        t = self.t
        inner = QWidget()
        lay = page_layout(inner)

        # stan połączenia - zawsze w tym miejscu, stała wysokość
        status = card()
        status.setFixedHeight(76)
        srow = QHBoxLayout(status)
        srow.setContentsMargins(14, 8, 10, 8)
        srow.setSpacing(10)
        self.dot = StatusDot(t)
        srow.addWidget(self.dot)
        texts = QVBoxLayout()
        texts.setSpacing(0)
        self.status_title = label("", "")
        self.status_title.setStyleSheet("font-weight: 600;")
        self.status_sub = label("", "muted")
        texts.addStretch(1)
        texts.addWidget(self.status_title)
        texts.addWidget(self.status_sub)
        texts.addStretch(1)
        srow.addLayout(texts, 1)
        self.status_action = button("Szukaj bieżni", "primary")
        self.status_action.clicked.connect(self.on_status_action)
        srow.addWidget(self.status_action)
        lay.addWidget(status)

        # prędkość
        hero = card()
        hlay = QVBoxLayout(hero)
        hlay.setContentsMargins(14, 12, 14, 14)
        hlay.setSpacing(10)
        self.gauge = Gauge(t)
        hlay.addWidget(self.gauge, 1)
        self.belt = Belt(t)
        hlay.addWidget(self.belt)
        spark_head = QHBoxLayout()
        spark_head.addWidget(label("Ostatnia minuta", "small"))
        spark_head.addStretch(1)
        spark_head.addWidget(label("linia przerywana - próg biegu", "small"))
        hlay.addLayout(spark_head)
        self.spark = Sparkline(t)
        hlay.addWidget(self.spark)
        lay.addWidget(hero)

        # liczby
        stats = QHBoxLayout()
        stats.setSpacing(8)
        self.stat_steps = StatTile("Kroki")
        self.stat_cadence = StatTile("Kroki/min")
        self.stat_distance = StatTile("Dystans")
        self.stat_time = StatTile("Czas ruchu")
        for tile in (self.stat_steps, self.stat_cadence, self.stat_distance, self.stat_time):
            stats.addWidget(tile, 1)
        lay.addLayout(stats)

        # sterowanie
        control = card()
        clay = QVBoxLayout(control)
        clay.setContentsMargins(14, 10, 14, 14)
        clay.setSpacing(8)
        crow = QHBoxLayout()
        ctexts = QVBoxLayout()
        ctexts.setSpacing(0)
        ctexts.addWidget(label("Sterowanie w grze", "title"))
        key = self.config["hotkeys"]["toggle"]
        ctexts.addWidget(label(f"{key} włącza i wyłącza - działa też w grze", "muted"))
        crow.addLayout(ctexts, 1)
        self.enable_switch = Switch(t, "Sterowanie w grze", self.bridge.state.enabled)
        self.enable_switch.toggled.connect(self.on_enable)
        crow.addWidget(self.enable_switch)
        clay.addLayout(crow)
        self.output_seg = Segmented(
            [("dry", "Podgląd"), ("keys", "Klawiatura W / Shift")],
            self.settings["output"],
            "Wyjście",
        )
        self.output_seg.changed.connect(self.on_output)
        clay.addWidget(self.output_seg)
        self.output_hint = label("", "muted")
        self.output_hint.setWordWrap(True)
        self.output_hint.setMinimumHeight(40)
        clay.addWidget(self.output_hint)
        lay.addWidget(control)
        lay.addStretch(1)

        # stopka
        foot = QHBoxLayout()
        settings_btn = icon_button("settings", "Ustawienia")
        settings_btn.clicked.connect(lambda: self.pages.go(self.SETTINGS))
        log_btn = icon_button("log", "Dziennik i raport diagnostyczny")
        log_btn.clicked.connect(lambda: self.pages.go(self.LOG))
        foot.addWidget(settings_btn)
        foot.addWidget(log_btn)
        foot.addStretch(1)
        foot.addWidget(label(f"wersja {self.version()}", "small"))
        lay.addLayout(foot)
        self.render_output_hint()
        return self.scroll(inner)

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

    def slider(self, lo: float, hi: float, value: float, name: str) -> tuple[QSlider, Any]:
        s = QSlider(Qt.Orientation.Horizontal)
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
        inner = QWidget()
        lay = page_layout(inner)
        lay.addLayout(header("Ustawienia", lambda: self.pages.go(self.MAIN, back=True)))

        speed = card()
        sl = QVBoxLayout(speed)
        sl.setContentsMargins(14, 12, 14, 14)
        sl.addWidget(label("Progi prędkości", "section"))
        self.walk_slider, walk_value = self.slider(
            0.3, 2.0, self.settings["stand_below"], "Chód od"
        )
        self.run_slider, run_value = self.slider(3.0, 9.0, self.settings["run_on"], "Bieg od")
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
            s.sliderReleased.connect(self.on_thresholds)
            s.valueChanged.connect(
                lambda _v, sl_=s: None if sl_.isSliderDown() else self.on_thresholds()
            )
        note = label(
            f"Bieg wyłącza się {fmt_num(HYSTERESIS)} km/h niżej, żeby Shift nie migał.", "small"
        )
        note.setWordWrap(True)
        sl.addWidget(note)
        lay.addWidget(speed)

        look = card()
        ll = QVBoxLayout(look)
        ll.setContentsMargins(14, 12, 14, 14)
        ll.addWidget(label("Wygląd i okno", "section"))
        theme_seg = Segmented(
            [("system", "Jak Windows"), ("light", "Jasny"), ("dark", "Ciemny")],
            self.settings["theme"],
            "Motyw",
        )
        theme_seg.changed.connect(lambda v: self.change("theme", v, rebuild=True))
        ll.addWidget(theme_seg)
        big = Switch(t, "Duży tekst", self.settings["large_text"])
        big.toggled.connect(lambda v: self.change("large_text", v, rebuild=True))
        self.setting_row(ll, "Duży tekst", "Wszystkie napisy większe o 15%", big)
        top = Switch(t, "Zawsze na wierzchu", self.settings["always_on_top"])
        top.toggled.connect(lambda v: self.change("always_on_top", v, rebuild=True))
        self.setting_row(ll, "Zawsze na wierzchu", "Okno nie chowa się pod grą", top)
        shortcut = button("Utwórz skrót na pulpicie")
        shortcut.clicked.connect(self.create_shortcut)
        ll.addWidget(shortcut)
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
        lay.addStretch(1)
        return self.scroll(inner)

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
        self.report_status = label("", "muted")
        self.report_status.setFixedHeight(24)
        lay.addWidget(self.report_status)
        row = QHBoxLayout()
        folder = button("Otwórz folder logów")
        folder.clicked.connect(lambda: self.open_folder(Path("logs")))
        report = button("Zapisz raport diagnostyczny", "primary")
        report.clicked.connect(self.diagnose)
        row.addWidget(folder, 1)
        row.addWidget(report, 1)
        lay.addLayout(row)
        return page

    # --- akcje --------------------------------------------------------------

    def version(self) -> str:
        head = Path(".git") / "HEAD"
        with contextlib.suppress(OSError, subprocess.SubprocessError):
            if head.exists():
                out = subprocess.run(
                    ["git", "rev-parse", "--short", "HEAD"],
                    capture_output=True,
                    text=True,
                    timeout=3,
                    check=False,
                ).stdout.strip()
                if out:
                    return f"{__version__} ({out})"
        return __version__

    def save(self) -> None:
        with contextlib.suppress(OSError):
            save_settings(self.settings)

    def change(self, key: str, value: Any, rebuild: bool = False) -> None:
        self.settings[key] = value
        self.save()
        if rebuild:
            QTimer.singleShot(0, self.build)

    def on_enable(self, on: bool) -> None:
        self.bridge.set_enabled(on)
        save_enabled(on)

    def on_output(self, kind: str) -> None:
        self.bridge.set_output(kind)
        self.settings["output"] = kind
        self.save()
        self.render_output_hint()

    def render_output_hint(self) -> None:
        if self.settings["output"] == "keys":
            text = "Chód naciska W, bieg Shift + W w aktywnym oknie. Kliknij w grę, zanim ruszysz."
        else:
            text = "Nic nie jest naciskane - sprawdź progi, zanim włączysz klawiaturę."
        self.output_hint.setText(text)

    def on_thresholds(self) -> None:
        walk = self.walk_slider.value() / 10
        run = max(self.run_slider.value() / 10, walk + HYSTERESIS + 0.2)
        try:
            mapping = MappingConfig.from_dict(
                {
                    **self.config["mapping"],
                    "stand_below": walk,
                    "run_on": run,
                    "run_off": run - HYSTERESIS,
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
        self.report_status.setText("Zbieram raport (ok. 30 s)…")
        self.worker.diagnose(path)

    def open_folder(self, path: Path) -> None:
        path.mkdir(exist_ok=True)
        if sys.platform == "win32":
            os.startfile(path.resolve())

    def create_shortcut(self) -> None:
        target = Path("Uruchom.bat").resolve()
        if sys.platform != "win32" or not target.exists():
            log.warning("Skrót działa tylko w wersji uruchamianej przez Uruchom.bat")
            return
        script = (
            "$d=[Environment]::GetFolderPath('Desktop');"
            "$s=(New-Object -ComObject WScript.Shell).CreateShortcut((Join-Path $d 'Bieżnia.lnk'));"
            "$s.TargetPath=$env:BZ_TARGET;$s.WorkingDirectory=$env:BZ_DIR;"
            "$s.WindowStyle=7;$s.Description='Bieżnia - sterowanie w grach';$s.Save()"
        )
        env = {**os.environ, "BZ_TARGET": str(target), "BZ_DIR": str(target.parent)}
        result = subprocess.run(
            ["powershell", "-NoProfile", "-NonInteractive", "-Command", script],
            env=env,
            capture_output=True,
            timeout=20,
            check=False,
        )
        if result.returncode == 0:
            log.info("Skrót „Bieżnia” jest na pulpicie")
        else:
            log.warning("Nie udało się utworzyć skrótu")

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
        elif event.kind == "report":
            self.report_status.setText(f"Raport: {event.text}")
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

        actions = self.hotkeys.poll()
        state = self.bridge.tick(time.monotonic(), actions)
        if "toggle" in actions:
            self.enable_switch.set_silently(state.enabled)
            save_enabled(state.enabled)

        c = state.control
        if not state.live and not self.settings["simulator"]:
            mode, caption = (
                "offline",
                "Brak danych" if self.link is not Link.STREAMING else "Czekam na dane",
            )
        elif c.run:
            mode, caption = "run", "Bieg - Shift + W"
        elif c.walk:
            mode, caption = "walk", "Chód - W"
        else:
            mode, caption = "idle", "Stoisz"
        if state.enabled and self.settings["output"] == "dry" and c.walk:
            caption += " (podgląd)"
        self.gauge.set_speed(c.speed, mode, caption)
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
        self.bridge.shutdown()
        self.worker.shutdown()
        self.save()
        self.qt.quit()


def run_gui(config: dict[str, Any], enabled: bool) -> int:
    qt = QApplication.instance() or QApplication(sys.argv)
    assert isinstance(qt, QApplication)
    qt.setApplicationName("Bieżnia")
    qt.setApplicationDisplayName("Bieżnia")
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
