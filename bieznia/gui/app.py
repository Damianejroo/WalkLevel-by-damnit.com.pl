"""Okno aplikacji (PySide6): kontroler, pętla i widoki wspólne (bieżnie, zgłoszenie, dziennik).

Ekrany 0.3.0 są w bieznia/gui/ekrany: główny, ustawienia, profil gry, osoby (konfigurator),
historia kroków i kopia danych. Każdy to mixin klasy App.

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

from PySide6.QtCore import QPoint, QPropertyAnimation, Qt, QTimer
from PySide6.QtGui import (
    QCloseEvent,
    QColor,
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
    QLabel,
    QLineEdit,
    QPlainTextEdit,
    QScrollArea,
    QStackedWidget,
    QVBoxLayout,
    QWidget,
)

from bieznia import (
    APP_NAME,
    TITLE,
    __version__,
    aktualizacje,
    dane,
    historia,
    zgloszenia,
)
from bieznia import pad as gamepad
from bieznia.ble.worker import BleEvent, BleWorker, FoundDevice, Link
from bieznia.engine import Bridge
from bieznia.gui import komponenty as k
from bieznia.gui import theme as th
from bieznia.gui.ekrany.glowny import STATUS_ACTIONS, MainScreen
from bieznia.gui.ekrany.historia import HistoryScreen
from bieznia.gui.ekrany.kopia import CopyScreen
from bieznia.gui.ekrany.osoba import PeopleScreen, Wizard
from bieznia.gui.ekrany.profil import ProfileScreen
from bieznia.gui.ekrany.ustawienia import SettingsScreen
from bieznia.gui.theme import Theme
from bieznia.gui.widgets import (
    DeviceRow,
    FloatingToggle,
    FocusRing,
    HoldButton,
    KeyButton,
    Motion,
    ResponsiveRow,
    Segmented,
    Switch,
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
from bieznia.kopia import Preview
from bieznia.logs import set_detailed
from bieznia.mapping import MappingConfig
from bieznia.paths import FROZEN
from bieznia.recorder import Recorder
from bieznia.settings import save_enabled
from bieznia.sources.ble_source import QueueSource
from bieznia.sources.simulator import SimulatedTreadmill
from bieznia.winapi import is_key_down, set_topmost, vk_from_scan

log = logging.getLogger("bieznia")

TICK_MS = 50
STARTED = time.monotonic()  # import tego modułu to praktycznie start procesu
HYSTERESIS = 0.6
PAD_SETTLE_S = 0.15  # gałka w drodze przez skoki - wysyłamy dopiero, gdy się zatrzyma
PAD_STOP_SETTLE_S = 0.4  # gałka przechodzi przez środek przy zmianie kierunku - to nie stop
STOPPED = -1.0  # belt_target: taśma zatrzymana przez puszczoną gałkę
TOAST_MS = 4000
NO_FOCUS_ENV = "BIEZNIA_BEZ_FOKUSU"
DEFAULT_W, DEFAULT_H = 740, 890  # okno przy pierwszym starcie (potem z ostatniej sesji)
LOOK_RELOAD = ("theme", "large_text", "profile_color")


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


def defaults_from(config: dict[str, Any]) -> dict[str, Any]:
    """Ustawienia domyślne z config.toml (to, czego profil albo osoba nie nadpisują)."""
    mapping, output = config["mapping"], config["output"]
    ble, ui = config.get("ble", {}), config.get("ui", {})
    return {
        "stand_below": mapping["stand_below"],
        "run_on": mapping["run_on"],
        "run_off": mapping["run_off"],
        "quick_stop": bool(mapping.get("quick_stop", True)),
        "output": output["type"],
        "device_address": ble.get("device_address", ""),
        "device_name": ble.get("device_name", ""),
        "simulator": bool(ui.get("simulator", False)),
        "record": bool(ui.get("record", False)),
        "always_on_top": bool(ui.get("always_on_top", False)),
        "floating_button": bool(ui.get("floating_button", False)),
        "floating_pos": ui.get("floating_pos"),
        "theme": ui.get("theme", "system"),
        "large_text": bool(ui.get("large_text", False)),
        "detailed_logs": bool(ui.get("detailed_logs", True)),
        "last_version": ui.get("last_version", ""),
        "window_geometry": None,
        "walk_key": output.get("walk_key"),
        "run_key": output.get("run_key"),
        "toggle_key": output.get("toggle_key"),
        "belt_control": bool(ble.get("belt_control", True)),
        "direction": output.get("direction", "keyboard"),
        "idle_speed": float(ble.get("idle_speed", 1.0)),
        "walk_speed": float(ble.get("walk_speed", 1.5)),
        "run_speed": float(ble.get("run_speed", 2.5)),
        "pad_steps": gamepad.parse_steps(ble.get("pad_steps")),
        "pad_idle": ble.get("pad_idle", "stop"),
        "show_person_switch": True,
        "show_profile_switch": True,
    }


def normalize(s: dict[str, Any]) -> dict[str, Any]:
    """Wartości spoza zakresu (stary albo ręcznie zmieniony plik) -> bezpieczne domyślne."""
    s["pad_steps"] = gamepad.parse_steps(s.get("pad_steps"))
    if s.get("pad_idle") not in ("stop", "slow", "zero"):
        s["pad_idle"] = "stop"
    if s.get("direction") not in ("keyboard", "gamepad", "treadmill"):
        s["direction"] = "keyboard"
    if s.get("output") not in ("dry", "keys"):
        s["output"] = "dry"
    if s.get("theme") not in ("system", "light", "dark"):
        s["theme"] = "system"
    for key in ("stand_below", "run_on", "run_off", "idle_speed", "walk_speed", "run_speed"):
        s[key] = float(s[key])
    return s


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
        if os.environ.get(NO_FOCUS_ENV) == "1":  # testy i zrzuty: okno nie zabiera fokusu
            self.setAttribute(Qt.WidgetAttribute.WA_ShowWithoutActivating)
            self.setWindowFlag(Qt.WindowType.WindowDoesNotAcceptFocus)

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


TOOLTIP_QSS = (  # podpowiedzi to osobne okna - styl całej aplikacji, jeden dla obu motywów
    "QToolTip { background: #2b2b2b; color: #f3f3f3; border: 1px solid #5a5a5a;"
    " border-radius: 6px; padding: 6px 8px; font-size: 13px; }"
)


class Pages(QStackedWidget):
    """Przejście między widokami: krótki przesuw (panel), bez przenikania.

    Styl (motyw) jest na każdym widoku osobno i trafia na widok dopiero, gdy ten się pokazuje -
    zmiana motywu przestylowuje tylko widoczny ekran (kilkadziesiąt ms zamiast pół sekundy).
    """

    def __init__(self) -> None:
        super().__init__()
        self.qss = ""
        self.qss_version = 0
        self.qss_scale: object = None
        self.currentChanged.connect(lambda i: self.style_page(self.widget(i)))

    def set_qss(self, qss: str, everywhere: bool = False) -> None:
        """everywhere: zmiana wielkości tekstu - układ każdego widoku liczy się od nowa."""
        self.qss = qss
        self.qss_version += 1
        if everywhere:
            for i in range(self.count()):
                self.style_page(self.widget(i))
        else:
            self.style_page(self.currentWidget())

    def style_page(self, page: QWidget | None) -> None:
        if page is not None and page.property("qss_version") != self.qss_version:
            page.setStyleSheet(self.qss)
            page.setProperty("qss_version", self.qss_version)

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


class App(MainScreen, SettingsScreen, ProfileScreen, PeopleScreen, HistoryScreen, CopyScreen):
    MAIN, DEVICES, SETTINGS, LOG, REPORT, PROFILE, WIZARD, HISTORY, EXPORT, IMPORT = range(10)

    # atrybuty ustawiane przez ekrany (mixiny)
    toggle_key: Key
    bridge: Bridge
    advanced_open: bool
    walk_btn: KeyButton | None
    run_btn: KeyButton | None
    toggle_btn: KeyButton
    step_ladder: k.StepLadder | None
    walk_from_step: k.Stepper | None
    run_from_step: k.Stepper | None
    pad_steppers: list[k.Stepper]
    pad_status: QLabel
    pad_count_seg: Segmented
    pad_idle_seg: Segmented
    output_seg: Segmented
    control_box: QVBoxLayout
    palette: k.PalettePicker
    device_label: QLabel
    forget_btn: HoldButton
    belt_switch: Switch
    about_status: QLabel
    wizard: Wizard
    wizard_name: QLineEdit
    wizard_game_name: QLineEdit
    export_pick: dict[str, set[str]]
    export_flags: dict[str, object]
    import_preview: Preview
    import_pick: set[str]
    import_new: set[str]
    import_flags: dict[str, bool]

    def __init__(
        self,
        qt: QApplication,
        config: dict[str, Any],
        enabled: bool,
        store_path: Path = dane.STORE_FILE,
        history_path: Path = historia.DB_FILE,
    ) -> None:
        self.qt = qt
        self.config = config
        self.defaults = normalize(defaults_from(config))
        self.store_path = store_path
        self.history_path = history_path
        self.store: dane.Store | None = dane.Store.load(store_path)
        self.history: historia.History | None = None
        self.tracker: historia.SessionTracker | None = None
        self.open_history_db()
        # ustawienia 0.2 - tylko do pierwszego uruchomienia 0.3.0 (konfigurator je przenosi)
        self.legacy: dict[str, Any] | None = None
        if self.store is not None:
            self.settings: dict[str, Any] = normalize(self.store.flat(self.defaults))
        else:
            self.legacy = dane.legacy_settings()
            old = {k: v for k, v in (self.legacy or {}).items() if k in self.defaults}
            self.settings = normalize({**self.defaults, **old})
        self.advanced_open = False
        self.settings_view = self.settings["direction"]
        self.history_scope = "me"
        self.history_range = "week"
        self.last_toast = ""
        self.pad = gamepad.PadReader(background=True)
        self.pad_picker = gamepad.StepPicker()
        self.pad_state: gamepad.PadState | None = None
        self.pad_step = 0
        self.pad_step_since = 0.0
        self.key_down = is_key_down  # podmieniane w testach
        self.belt_target: float | None = None  # ostatnia prędkość wysłana z klawiszy
        self.report_endpoint = str(config.get("zgloszenia", {}).get("adres", "")).strip()
        self.tasks: queue.Queue[tuple[str, Any]] = queue.Queue()
        self.update_busy = False
        self.update_available = ""
        self.report_busy = False
        self.bridge = Bridge(
            MappingConfig.from_dict(config["mapping"]),
            self.settings["output"],
            enabled,
            self.profile_keys(),
        )
        self.apply_mapping()
        self.toggle_key = Key.from_dict(self.settings["toggle_key"], TOGGLE_DEFAULT)
        if self.toggle_key.vk == 0:  # zapis sprzed wersji z kodem wirtualnym
            self.toggle_key = TOGGLE_DEFAULT
        self.hotkeys = self.make_hotkeys()
        self.worker = BleWorker()
        self.logs: queue.Queue[str] = queue.Queue()
        self.log_lines: list[str] = []
        self.log_handler = QueueLogHandler(self.logs)
        logging.getLogger().addHandler(self.log_handler)
        self.link = Link.IDLE
        self.link_text = ""
        self.devices: list[FoundDevice] = []
        self.auto_connect = not self.settings["device_address"]
        self.stop_requested = False
        self.quitting = False
        self.frame = 0
        Motion.enabled = not th.system_reduces_motion()

        self.window: Window | None = None
        self.frozen = 0
        self.toast_label: QLabel | None = None
        self.toast_timer = QTimer()
        self.toast_timer.setSingleShot(True)
        self.toast_timer.timeout.connect(self.hide_toast)
        self.build()
        self.apply_source()
        self.apply_recording()
        self.set_tracker_owner()

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
            self._hints = hints

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

        if self.store is None:
            self.open_wizard_first()  # pierwsze uruchomienie 0.3.0 (nowa instalacja albo z 0.2)
        if self.settings["device_address"]:
            self.worker.connect(self.settings["device_address"], self.settings["device_name"])
        elif not self.settings["simulator"]:
            self.worker.scan()  # pierwszy start: od razu szukamy bieżni

    # --- dane: osoby, profile, historia ---------------------------------------

    def open_history_db(self) -> None:
        self.close_history()
        try:
            self.history = historia.History(self.history_path)
        except Exception:  # baza zablokowana albo uszkodzona - aplikacja działa bez historii
            log.exception("Historia kroków niedostępna")
            self.history = None
            return
        self.tracker = historia.SessionTracker(self.history)

    def close_history(self) -> None:
        if self.tracker is not None:
            with contextlib.suppress(Exception):
                self.tracker.finish()
            self.tracker = None
        if self.history is not None:
            with contextlib.suppress(Exception):
                self.history.close()
            self.history = None

    def set_tracker_owner(self) -> None:
        store, tracker = self.store, self.tracker
        if store is None or tracker is None:
            return
        tracker.set_owner(store.person.id, store.person.name, store.profile.id, store.profile.name)

    def profile_keys(self) -> tuple[Key, Key]:
        return (
            Key.from_dict(self.settings["walk_key"], WALK_DEFAULT),
            Key.from_dict(self.settings["run_key"], RUN_DEFAULT),
        )

    def apply_profile(self) -> None:
        """Po zmianie osoby albo profilu: ustawienia, klawisze, tryb, wygląd, historia."""
        store = self.store
        if store is None:
            return
        before = {key: self.settings.get(key) for key in (*LOOK_RELOAD, "always_on_top")}
        floating = self.settings.get("floating_button")
        self.settings = normalize(store.flat(self.defaults))
        self.bridge.set_keys(*self.profile_keys())
        self.apply_mapping()
        if any(self.settings.get(key) != before[key] for key in LOOK_RELOAD):
            self.apply_look()
        if self.settings["always_on_top"] != before["always_on_top"]:
            self.on_always_on_top(bool(self.settings["always_on_top"]))
        if self.settings["floating_button"] != floating:
            self.on_floating_button(bool(self.settings["floating_button"]))
        self.apply_direction()
        self.settings_view = self.settings["direction"]
        self.render_header()
        self.set_tracker_owner()
        log.info(
            "Osoba %s, profil %s (%s)",
            store.person.name,
            store.profile.name,
            self.settings["direction"],
        )

    def reload_store(self) -> None:
        self.store = dane.Store.load(self.store_path)
        self.open_history_db()
        self.apply_profile()

    # --- budowa okna --------------------------------------------------------

    def page_qss(self) -> str:
        return th.qss(self.t, 1.15 if self.settings["large_text"] else 1.0)

    def make_theme(self) -> Theme:
        base = th.resolve_theme(self.settings["theme"])
        return th.with_color(base, self.settings.get("profile_color"))

    def build(self) -> None:
        self.t = self.make_theme()
        self.qt.setStyleSheet(TOOLTIP_QSS)

        w = Window(self.t, self.on_close)
        self.window = w
        root = QVBoxLayout(w)
        root.setContentsMargins(0, 0, 0, 0)
        self.pages = Pages()
        self.pages.set_qss(self.page_qss())
        root.addWidget(self.pages)
        self.pages.addWidget(self.build_main())
        self.pages.addWidget(self.build_devices())
        self.pages.addWidget(self.build_settings())
        self.pages.addWidget(self.build_log())
        self.pages.addWidget(self.build_report())
        for _ in range(self.IMPORT - self.REPORT):  # profil, konfigurator, historia, kopia
            placeholder = QWidget()
            placeholder.setObjectName("page")
            self.pages.addWidget(placeholder)
        self.pages.set_qss(self.page_qss(), everywhere=True)  # szerokości liczone już ze stylem
        self.pages.qss_scale = self.settings["large_text"]
        self.ring = FocusRing(w, self.t)

        w.setWindowFlag(Qt.WindowType.WindowStaysOnTopHint, self.settings["always_on_top"])
        self.place_window(w)
        w.show()
        QTimer.singleShot(300, self.keep_on_screen)
        w.setFocus(Qt.FocusReason.OtherFocusReason)  # nic nie ma ostrości ani obwódki na starcie
        self.apply_direction()
        self.render_header()
        self.render_link()
        self.render_devices()
        self.log_view.setPlainText("\n".join(self.log_lines))

    def place_window(self, w: QWidget) -> None:
        """Rozmiar i miejsce z ostatniej sesji, jeśli całe okno mieści się na którymś ekranie.

        Inaczej: wyśrodkowane na ekranie głównym. Zawsze jawne miejsce - przy kilku ekranach
        z różną skalą okno bez pozycji potrafi skakać między nimi i wylądować poza ekranem.
        """
        geo = self.settings.get("window_geometry")
        if isinstance(geo, list) and len(geo) == 4 and all(isinstance(v, int) for v in geo):
            x, y, width, height = geo
            screen = QGuiApplication.screenAt(QPoint(x + width // 2, y + 20))
            if screen is not None:
                area = screen.availableGeometry()
                width = max(w.minimumWidth(), min(width, area.width()))
                height = max(w.minimumHeight(), min(height, area.height()))
                x = max(area.left(), min(x, area.right() - width))
                y = max(area.top(), min(y, area.bottom() - height))
                w.move(x, y)  # najpierw ekran (inna skala zmieniłaby rozmiar), potem rozmiar
                w.resize(width, height)
                return
        self.center_window(w)

    def center_window(self, w: QWidget) -> None:
        area = self.qt.primaryScreen().availableGeometry()
        width = min(DEFAULT_W, area.width() - 40)
        height = min(DEFAULT_H, area.height() - 60)
        w.move(area.x() + (area.width() - width) // 2, area.y() + (area.height() - height) // 2)
        w.resize(width, height)

    def keep_on_screen(self) -> None:
        """Po pokazaniu: gdyby okno i tak było poza ekranami - na środek ekranu głównego."""
        w = self.window
        if w is None or w.isMinimized():
            return
        if QGuiApplication.screenAt(w.frameGeometry().center()) is None:
            log.warning("Okno poza ekranem (%s) - przenoszę na środek", w.frameGeometry())
            self.center_window(w)

    def remember_window(self) -> None:
        w = self.window
        if w is None or w.isMinimized() or w.isMaximized():
            return
        if QGuiApplication.screenAt(w.frameGeometry().center()) is None:
            return  # poza ekranem - nie zapamiętujemy
        self.settings["window_geometry"] = [w.x(), w.y(), w.width(), w.height()]

    def replace_page(self, index: int, widget: QWidget) -> None:
        """Podmiana widoku (przebudowa po zmianie) z zachowaniem miejsca przewinięcia."""
        old = self.pages.widget(index)
        current = self.pages.currentIndex() == index
        scroll = 0
        if old is not None and current:
            area = old.findChild(QScrollArea)
            if area is not None:
                scroll = area.verticalScrollBar().value()
        self.pages.style_page(widget)  # styl przed pokazaniem: bez mignięcia, dobre szerokości
        self.pages.insertWidget(index, widget)
        if old is not None:
            self.pages.removeWidget(old)
            old.deleteLater()
        if current:
            self.pages.setCurrentIndex(index)
            area = widget.findChild(QScrollArea)
            bar = area.verticalScrollBar() if area is not None else None
            # nowy widok układa się i przewija, zanim okno odmaluje się raz - bez skoku
            self.freeze(True)

            def restore() -> None:
                if bar is not None and scroll:
                    with contextlib.suppress(RuntimeError):
                        bar.setValue(scroll)
                self.freeze(False)

            QTimer.singleShot(0, restore)

    def quietly(self, change: Any) -> None:
        """Zmiana układu bez mignięcia: okno odmalowuje się raz, po ułożeniu wszystkiego."""
        self.freeze(True)
        try:
            change()
        finally:
            QTimer.singleShot(0, lambda: self.freeze(False))

    def freeze(self, on: bool) -> None:
        """Wstrzymanie odświeżania okna (zagnieżdżane): odmalowanie raz, gdy wszystko gotowe."""
        self.frozen = max(0, self.frozen + (1 if on else -1))
        w = self.window
        if w is not None:
            with contextlib.suppress(RuntimeError):
                w.setUpdatesEnabled(self.frozen == 0)

    def go_back(self, index: int) -> None:
        if index == self.SETTINGS:
            self.rebuild_settings()  # nazwy, kolory i profile mogły się zmienić
        self.pages.go(index, back=True)

    def sticky_page(self, title: str, back: int | None = None) -> tuple[QWidget, QVBoxLayout]:
        """Podstrona: pasek z Wróć zawsze na górze, przewija się tylko treść pod nim."""
        target = self.MAIN if back is None else back
        return self.framed_page(header(title, lambda: self.go_back(target)))

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
            "- logi aplikacji i Bluetooth z ostatnich 6 godzin i ich podsumowanie,",
            "- ostatni raport diagnostyczny, jeśli go zapisano,",
            "- wersja aplikacji i Windows, ustawienia (progi, wybrana bieżnia).",
            "Bez haseł, plików z komputera, wciśniętych klawiszy i historii kroków.",
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
        lay.addLayout(header("Dziennik", lambda: self.pages.go(self.SETTINGS, back=True)))
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

    # --- komunikat na dole okna --------------------------------------------------

    def toast(self, text: str) -> None:
        log.info("Komunikat: %s", text)
        self.last_toast = text
        w = self.window
        if w is None:
            return
        if self.toast_label is None:
            self.toast_label = QLabel(w)
            self.toast_label.setWordWrap(True)
            self.toast_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
            self.toast_label.mousePressEvent = lambda _e: self.hide_toast()  # type: ignore[method-assign]
        lbl, t = self.toast_label, self.t
        lbl.setStyleSheet(
            f"background: {t.text}; color: {t.bg}; border-radius: 10px; padding: 10px 14px;"
            " font-weight: 600;"
        )
        lbl.setText(text)
        lbl.setFixedWidth(min(w.width() - 32, 420))
        lbl.adjustSize()
        lbl.move((w.width() - lbl.width()) // 2, w.height() - lbl.height() - 20)
        lbl.raise_()
        lbl.show()
        self.toast_timer.start(TOAST_MS)

    def hide_toast(self) -> None:
        if self.toast_label is not None:
            self.toast_label.hide()

    # --- akcje --------------------------------------------------------------

    def save(self) -> None:
        with contextlib.suppress(OSError):
            if self.store is not None:
                self.store.update_flat(self.settings)
            # przed konfiguratorem zmiany są tylko w pamięci - trafią do pierwszej osoby,
            # a plik 0.2 zostaje nietknięty (stara wersja dalej go czyta)

    def change(self, key: str, value: Any) -> None:
        self.settings[key] = value
        self.save()

    def apply_look(self) -> None:
        """Motyw, kolor profilu i wielkość tekstu na żywo - bez przebudowy okna."""
        self.t = self.make_theme()
        if self.window is None:
            return
        self.freeze(True)  # jedno odmalowanie na końcu zamiast migania w trakcie
        qss = self.page_qss()
        # sam kolor: reszta widoków dostanie styl przy wejściu; inna wielkość tekstu: wszystkie
        self.pages.set_qss(qss, everywhere=self.pages.qss_scale != self.settings["large_text"])
        self.pages.qss_scale = self.settings["large_text"]
        for widget in [self.window, *self.window.findChildren(QWidget)]:
            if hasattr(widget, "retheme"):
                widget.retheme(self.t)
            elif hasattr(widget, "t"):
                widget.t = self.t
                widget.update()
        fix_width(self.status_action, STATUS_ACTIONS)  # duży tekst zmienia szerokość napisów
        if hasattr(self, "bubble"):
            self.bubble.retheme(self.t)
        self.render_link()
        self.render_update_label()
        self.render_header()
        self.render_zones()
        if self.pages.currentIndex() == self.SETTINGS:
            self.rebuild_settings()  # kolory zapisane w stylach kart
        self.freeze(False)

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
        live = not self.settings["simulator"]
        sets_speed = self.sets_speed() and live
        pad_stop = self.pad_stops_belt() and live  # taśmę uruchamia dopiero ruch gałki
        if changed and not on:
            if self.belt_commands_possible() or pad_stop:
                self.worker.belt("stop")
            elif sets_speed:
                self.worker.set_speed(self.idle_target())  # bez stopu: zwolnij
        if changed and on and self.belt_commands_possible() and not pad_stop:
            # czy taśma już jedzie, sprawdza sterownik (stan bieżni, nie prędkość)
            self.worker.belt("start")
            self.render_output_hint("Uruchamiam bieżnię…")

    def belt_commands_possible(self) -> bool:
        return bool(self.settings["belt_control"]) and not self.settings["simulator"]

    def on_belt_control(self, on: bool) -> None:
        self.change("belt_control", on)
        self.render_control_subtitle()

    def render_control_subtitle(self) -> None:
        belt = bool(self.settings["belt_control"])
        start = "uruchamia i zatrzymuje taśmę" if belt else "włącza i wyłącza sterowanie"
        direction = self.settings["direction"]
        if direction == "keyboard":
            text = f"{start} · {self.bridge.keys[0].name} przyspiesza"
        elif direction == "gamepad":
            text = f"{start} · gałka ustawia tempo"
        else:
            text = f"{start} · działa też w grze"
        if self.control_subtitle.text() != text:
            self.control_subtitle.setText(text)

    def on_direction(self, kind: str) -> None:
        if kind == self.settings["direction"]:
            return
        self.change("direction", kind)
        log.info("Kierunek sterowania: %s", kind)
        self.apply_direction()
        self.settings_view = kind
        self.rebuild_settings()  # karta sterowania w Ustawieniach pokazuje bieżący tryb

    def sets_speed(self) -> bool:
        """Klawisze albo pad ustawiają prędkość taśmy (a nie odwrotnie)."""
        return self.settings["direction"] in ("keyboard", "gamepad")

    def idle_target(self) -> float:
        if self.settings["direction"] == "gamepad":
            if self.settings["pad_idle"] == "zero":
                return 0.0
            return float(self.settings["pad_steps"][0][1])
        return float(self.settings["idle_speed"])

    def apply_direction(self) -> None:
        """Klawisze/pad -> bieżnia: aplikacja niczego nie naciska, tylko słucha gracza."""
        sets_speed = self.sets_speed()
        self.bridge.set_output("dry" if sets_speed else self.settings["output"])
        self.belt_target = None
        self.mode_tiles.set_value(self.settings["direction"])
        self.render_control_subtitle()
        self.render_zones()
        self.render_output_hint()

    # --- pad ------------------------------------------------------------------

    def pad_idle_text(self) -> str | None:
        """Co robi taśma przy puszczonej gałce, gdy to nie prędkość skoku 1."""
        return {"stop": "stop", "zero": "0 km/h"}.get(self.settings["pad_idle"])

    def pad_target(self, now: float) -> tuple[float, str, str, bool]:
        """Prędkość ze skoku gałki. Ostatnie pole: skok trzyma się już chwilę (można wysłać)."""
        steps = self.settings["pad_steps"]
        self.pad_state = self.pad.read()
        deflection = self.pad_state.deflection if self.pad_state else 0.0
        index = self.pad_picker.update(steps, deflection)
        if index != self.pad_step:
            self.pad_step, self.pad_step_since = index, now
        idle = self.pad_idle_text() if index == 0 else None
        speed = 0.0 if idle else float(steps[index][1])
        held = "idle" if index == 0 else ("run" if index == len(steps) - 1 else "walk")
        belt = "taśma stoi" if idle == "stop" else f"taśma {fmt_num(speed)} km/h"
        if self.pad_state is None:
            caption = f"Brak pada - {belt}"
        else:
            caption = f"Gałka {round(deflection * 100)}% - skok {index + 1} - {belt}"
        wait = PAD_STOP_SETTLE_S if idle == "stop" else PAD_SETTLE_S
        settled = now - self.pad_step_since >= wait
        return speed, held, caption, settled

    def send_pad(self, target: float) -> None:
        """Prędkość z pada; puszczona gałka w trybie stop zatrzymuje taśmę, ruch ją budzi."""
        if self.pad_step == 0 and self.settings["pad_idle"] == "stop":
            if self.belt_target != STOPPED:
                self.belt_target = STOPPED
                self.worker.belt("stop")
            return
        if target == self.belt_target:
            return
        self.belt_target = target
        wake = target > 0 and (self.pad_stops_belt() or bool(self.settings["belt_control"]))
        self.worker.set_speed(target, start_if_stopped=wake)

    def pad_stops_belt(self) -> bool:
        return bool(self.settings["direction"] == "gamepad" and self.settings["pad_idle"] == "stop")

    def render_pad_status(self) -> None:
        """Na karcie pada w Ustawieniach: który pad i gdzie jest gałka (na żywo)."""
        if self.step_ladder is None or self.pages.currentIndex() != self.SETTINGS:
            return
        state = self.pad_state
        if state is None:
            text = "Nie widzę pada. Podłącz DualSense albo pada Xbox (kablem albo Bluetooth)."
        else:
            step = self.pad_step + 1
            text = f"{state.name}: wychylenie {round(state.deflection * 100)}%, skok {step}."
        with contextlib.suppress(RuntimeError):  # karta właśnie przebudowana
            if self.pad_status.text() != text:
                self.pad_status.setText(text)
            self.step_ladder.set_stick(state.deflection if state else 0.0)

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
                "toggle": (self.toggle_key.vk, self.toggle_key.mods),
                "sim_slower": keys["sim_slower"],
                "sim_faster": keys["sim_faster"],
            }
        )

    def on_belt_event(self, result: str) -> None:
        messages = {
            "started": "Bieżnia rusza.",
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
        if os.environ.get(NO_FOCUS_ENV) != "1":
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
        self.change("output", kind)
        self.render_output_hint()

    def render_output_hint(self, problem_text: str = "") -> None:
        walk, run = self.bridge.keys
        direction = self.settings["direction"]
        if problem_text:
            text = problem_text
        elif direction == "gamepad":
            text = "Wychyl lewą gałkę - im dalej, tym szybciej. Skoki zmienisz w Ustawieniach."
        elif direction == "keyboard":
            text = f"Trzymaj {walk.name}, żeby przyspieszyć, {walk.name} + {run.name} - bieg."
        elif self.settings["output"] == "keys":
            text = f"Gra dostaje {walk.name} i {run.name} + {walk.name} (aktywne okno)."
        else:
            text = "Podgląd: nic nie jest naciskane - Klawiatura w Ustawieniach."
        self.output_hint.setText(text)

    def key_buttons(self) -> list[KeyButton]:
        return [b for b in (self.walk_btn, self.run_btn, self.toggle_btn) if b is not None]

    def capturing(self) -> bool:
        return any(b.capturing for b in self.key_buttons())

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
            self.toast(error)
            return
        self.bridge.set_keys(walk, run)
        with contextlib.suppress(RuntimeError):
            for b, value in ((self.walk_btn, walk), (self.run_btn, run), (self.toggle_btn, toggle)):
                if b is not None:
                    b.set_key(value)
        self.settings["walk_key"] = walk.to_dict()
        self.settings["run_key"] = run.to_dict()
        if toggle != self.toggle_key:
            self.toggle_key = toggle
            self.settings["toggle_key"] = toggle.to_dict()
            self.hotkeys = self.make_hotkeys()  # trzymany właśnie klawisz się nie liczy
            self.toggle_cap.setText(toggle.label)
        self.render_control_subtitle()
        self.save()
        self.render_zones()
        self.render_output_hint()

    def apply_source(self) -> None:
        if self.settings["simulator"]:
            self.bridge.set_source(SimulatedTreadmill())
        else:
            self.bridge.set_source(QueueSource(self.worker.readings))

    def on_simulator(self, on: bool) -> None:
        self.change("simulator", on)
        self.apply_source()
        log.info("Symulator %s", "włączony" if on else "wyłączony")
        self.render_link()

    def apply_recording(self) -> None:
        recorder = Recorder(Path("nagrania")) if self.settings["record"] else None
        self.bridge.set_recording(recorder)
        if recorder:
            log.info("Nagrywam do %s", recorder.path)

    def on_record(self, on: bool) -> None:
        self.change("record", on)
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
        if self.pages.currentIndex() == self.DEVICES:
            self.pages.go(self.MAIN, back=True)
        log.info("Wybrano bieżnię %s", device.name)
        self.render_device_label()

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

    def set_about_status(self, text: str) -> None:
        with contextlib.suppress(RuntimeError):  # Ustawienia mogły się właśnie przebudować
            self.about_status.setText(text)

    def handle_task(self, kind: str, data: Any) -> None:
        if kind == "update_status":
            self.update_label.setText(data)
            self.set_about_status(data)
        elif kind == "update_available":
            self.update_available = data
            self.update_label.setText(f"Dostępna wersja {data}")
            self.render_update_label()
            self.set_about_status(f"Dostępna nowa wersja {data} - kliknij Aktualizuj.")
        elif kind == "update_current":
            if data:  # tylko po kliknięciu - ciche sprawdzenie przy starcie nic nie mówi
                self.update_label.setText("Masz najnowszą wersję")
                self.set_about_status(f"Masz najnowszą wersję ({__version__}).")
        elif kind == "update_error":
            message, clicked = data
            if clicked:
                self.update_label.setText("Aktualizacja nieudana")
                self.set_about_status(message)
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
        self.render_device_label()

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
                if self.pages.currentIndex() != self.WIZARD:  # konfiguratora nie przerywamy
                    self.pages.go(self.DEVICES)
        elif event.kind == "scan_failed":
            if event.link is not None:
                self.link, self.link_text = event.link, event.text
                self.render_link()
            self.auto_connect = False
            self.render_devices()
            if self.pages.currentIndex() != self.WIZARD:
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

        capturing = self.capturing()
        actions = self.hotkeys.poll()
        if "toggle" in actions:
            actions.discard("toggle")
            if not capturing:
                self.set_control(not self.bridge.state.enabled)
        state = self.bridge.tick(time.monotonic(), actions)

        c = state.control
        direction = self.settings["direction"]
        settled = True
        speed_caption = ""
        if direction == "gamepad":
            target, held, speed_caption, settled = self.pad_target(time.monotonic())
        elif direction == "keyboard":
            target, held = self.keyboard_target()
        else:
            target, held = 0.0, "idle"
        sends = (
            direction != "treadmill"
            and state.enabled
            and settled
            and not capturing
            and not self.settings["simulator"]
        )
        if sends and direction == "gamepad":
            self.send_pad(target)
        elif sends and target != self.belt_target:
            self.belt_target = target
            self.worker.set_speed(target)
        off = f"Wyłączone - {self.toggle_key.label} włącza"
        zone = {"idle": 0, "walk": 1, "run": 2}
        offline = not state.live and not self.settings["simulator"]
        if offline and direction == "treadmill":
            mode, active = "offline", -1
            caption = (
                "Brak danych z bieżni" if self.link is not Link.STREAMING else "Czekam na dane"
            )
        elif direction == "gamepad":
            mode = held if state.enabled else "idle"
            active = self.pad_step if state.enabled else -1
            caption = speed_caption if state.enabled else off
        elif direction == "keyboard":
            walk_key, run_key = self.bridge.keys
            mode = held if state.enabled else "idle"
            active = zone[held] if state.enabled else -1
            caption = {
                "run": f"Bieg · {walk_key.name} + {run_key.name} · taśma {fmt_num(target)} km/h",
                "walk": f"Chód · {walk_key.name} · taśma {fmt_num(target)} km/h",
                "idle": f"Bez klawisza · taśma {fmt_num(target)} km/h",
            }[held]
            if not state.enabled:
                caption = off
        else:
            walk_key, run_key = self.bridge.keys
            if c.run:
                mode, caption = "run", f"Bieg · gra dostaje {run_key.name} + {walk_key.name}"
            elif c.walk:
                mode, caption = "walk", f"Chód · gra dostaje {walk_key.name}"
            else:
                mode, caption = "idle", "Stoisz"
            active = zone[mode]
            if not state.enabled:
                caption = off
            elif self.settings["output"] == "dry" and c.walk:
                caption += " (podgląd)"
        if offline:
            mode = "offline"  # żeton pokazuje, co wysyłamy; liczba bez danych jest szara
        self.gauge.set_speed(c.speed, mode, caption, active)
        if self.gauge_status.text() != caption:
            self.gauge_status.setText(caption)
        self.zone_chips.set_active(active)
        self.bubble.set_state(
            state.enabled, mode, c.speed, state.live or self.settings["simulator"]
        )

        self.frame += 1
        if self.frame % 5 == 0:  # liczby 4x na sekundę wystarczą
            self.stat_steps.set(str(state.steps) if state.steps is not None else "-")
            self.stat_cadence.set(f"{state.cadence:.0f}" if state.cadence else "-")
            self.stat_distance.set(f"{fmt_num(state.distance_km, 2)} km")
            minutes, seconds = divmod(int(state.active_s), 60)
            self.stat_time.set(f"{minutes}:{seconds:02d}")
            if self.tracker is not None and (state.live or self.settings["simulator"]):
                try:
                    self.tracker.update(c.speed, state.steps, state.distance_km, state.active_s)
                except Exception:  # historia nie może zatrzymać sterowania
                    log.exception("Historia kroków")
                    self.tracker = None
            self.render_link()
            self.render_pad_status()

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
        self.toast_timer.stop()
        self.bubble.close()
        was_enabled = self.bridge.state.enabled
        self.bridge.shutdown()
        if was_enabled and self.belt_commands_possible() and self.worker.stop_belt_now():
            log.info("Zamykanie: zatrzymano taśmę")
        self.worker.shutdown()
        self.remember_window()
        self.save()
        self.close_history()
        logging.getLogger().removeHandler(self.log_handler)
        hints = getattr(self, "_hints", None)
        if hints is not None:
            with contextlib.suppress(RuntimeError, TypeError):
                hints.colorSchemeChanged.disconnect(self.on_system_scheme)
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


__all__ = ["App", "run_gui"]
