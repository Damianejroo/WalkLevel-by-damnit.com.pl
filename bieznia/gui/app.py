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
from pathlib import Path
from typing import Any

from PySide6.QtCore import QCoreApplication, QPoint, Qt, QTimer
from PySide6.QtGui import (
    QGuiApplication,
)
from PySide6.QtWidgets import (
    QApplication,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPlainTextEdit,
    QScrollArea,
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
from bieznia.ble.worker import BleWorker, FoundDevice, Link
from bieznia.engine import Bridge
from bieznia.gui import komponenty as k
from bieznia.gui import theme as th
from bieznia.gui.ekrany.bieznia import TreadmillScreen
from bieznia.gui.ekrany.glowny import STATUS_ACTIONS, MainScreen
from bieznia.gui.ekrany.historia import HistoryScreen
from bieznia.gui.ekrany.konto import AccountScreen
from bieznia.gui.ekrany.kopia import CopyScreen
from bieznia.gui.ekrany.modele import ModelsScreen
from bieznia.gui.ekrany.osoba import PeopleScreen, Wizard
from bieznia.gui.ekrany.profil import ProfileScreen
from bieznia.gui.ekrany.sterowanie import (
    ControlScreen,
)
from bieznia.gui.ekrany.ustawienia import SettingsScreen
from bieznia.gui.okno import (
    NO_FOCUS_ENV,
    TEST_SCREEN_ENV,
    CalmScrollBar,
    Cover,
    Pages,
    Window,
    header,
    page_layout,
    settle_layout,
)
from bieznia.gui.theme import Theme
from bieznia.gui.widgets import (
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
    fix_width,
    fmt_num,
    label,
)
from bieznia.hotkeys import Hotkeys
from bieznia.klawisze import RUN_DEFAULT, TOGGLE_DEFAULT, WALK_DEFAULT, Key, problem
from bieznia.kopia import Preview
from bieznia.logs import set_detailed
from bieznia.mapping import MappingConfig
from bieznia.paths import FROZEN
from bieznia.recorder import Recorder
from bieznia.sterowniki import Capabilities, DeviceInfo, known_capabilities
from bieznia.winapi import is_key_down, set_topmost

log = logging.getLogger("bieznia")

TICK_MS = 50
STARTED = time.monotonic()  # import tego modułu to praktycznie start procesu
TOAST_MS = 4000
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


TOOLTIP_QSS = (  # podpowiedzi to osobne okna - styl całej aplikacji, jeden dla obu motywów
    "QToolTip { background: #2b2b2b; color: #f3f3f3; border: 1px solid #5a5a5a;"
    " border-radius: 6px; padding: 6px 8px; font-size: 13px; }"
)


# --- kontroler -------------------------------------------------------------


class App(
    MainScreen,
    SettingsScreen,
    ProfileScreen,
    PeopleScreen,
    HistoryScreen,
    CopyScreen,
    TreadmillScreen,
    ControlScreen,
    ModelsScreen,
    AccountScreen,
):
    (MAIN, DEVICES, SETTINGS, LOG, REPORT, PROFILE, WIZARD, HISTORY, EXPORT, IMPORT) = range(10)
    MODELS, ACCOUNT, ONLINE = range(10, 13)

    # atrybuty ustawiane przez ekrany (mixiny)
    toggle_key: Key
    bridge: Bridge
    worker: BleWorker
    pad_step: int
    belt_target: float | None
    device: DeviceInfo | None
    devices: list[FoundDevice]
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
    control_body: k.SmoothBox
    mode_view: Segmented
    profile_rows: dict[str, k.ChoiceRow]
    history_built: str
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
        # co to za bieżnia i co potrafi: od razu z profilu zapamiętanej, potem od niej samej
        self.device: DeviceInfo | None = None
        self.caps: Capabilities = known_capabilities(
            self.settings["device_name"], self.settings["simulator"]
        )
        self.set_tracker_source()
        self.devices: list[FoundDevice] = []
        self.auto_connect = not self.settings["device_address"]
        self.stop_requested = False
        self.quitting = False
        self.frame = 0
        Motion.enabled = not th.system_reduces_motion()

        self.window: Window | None = None
        self.toast_label: QLabel | None = None
        self.toast_timer = QTimer()
        self.toast_timer.setSingleShot(True)
        self.toast_timer.timeout.connect(self.hide_toast)
        self.init_account()  # przed budową okna - karta konta w Ustawieniach
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
        if self.settings["simulator"]:
            self.worker.connect_simulator()
        elif self.settings["device_address"]:
            self.worker.connect(self.settings["device_address"], self.settings["device_name"])
        else:
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
        self.set_tracker_source()

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
        t = th.with_color(base, self.settings.get("profile_color"))
        Segmented.accent = t.accent  # podświetlenie przełączników rysowane przez nie same
        return t

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
        for _ in range(self.ONLINE - self.REPORT):  # profil, konfigurator, historia, kopia…
            placeholder = QWidget()
            placeholder.setObjectName("page")
            self.pages.addWidget(placeholder)
        self.pages.set_qss(self.page_qss(), everywhere=True)  # szerokości liczone już ze stylem
        self.pages.qss_scale = self.settings["large_text"]
        self.settings_built = self.settings_stamp()  # Ustawienia zbudowane - bez ponownej budowy
        self.pages.cover = Cover(w, self.pages)
        self.pages.background = self.t.bg
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
            if os.environ.get(TEST_SCREEN_ENV) == "1" and screen is not self.home_screen():
                screen = None  # testy i pomiary: tylko ekran testowy
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

    def home_screen(self) -> Any:
        """Ekran okna: główny; dla testów i pomiarów (BIEZNIA_EKRAN_TESTOW=1) - inny niż główny,
        żeby okna testowe nie przeszkadzały w pracy."""
        primary = self.qt.primaryScreen()
        if os.environ.get(TEST_SCREEN_ENV) == "1":
            others = [s for s in self.qt.screens() if s is not primary]
            if others:
                return others[0]
        return primary

    def center_window(self, w: QWidget) -> None:
        area = self.home_screen().availableGeometry()
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
            settle_layout(widget)  # od razu właściwa wysokość - przewinięcie bez skoku przez 0
            area = widget.findChild(QScrollArea)
            if area is not None and scroll:
                area.verticalScrollBar().setValue(scroll)

    def morph_card(self, body: QWidget, rebuild: Any) -> None:
        """Przebudowa fragmentu ekranu (np. karty sterowania) z płynną zmianą wysokości:
        układ od razu docelowy, a na ekranie animacja zrzutów (Cover.morph)."""
        cover = self.pages.cover
        visible = (
            cover is not None
            and Motion.enabled
            and self.window is not None
            and self.window.isVisible()
            and body.isVisible()
        )
        if not visible:
            rebuild()
            return
        assert cover is not None
        top = body.mapTo(self.pages, QPoint(0, 0)).y()
        old_h = body.height()
        cover.capture(self.pages.background)
        rebuild()
        # nowe elementy Qt pokazuje dopiero w następnej klatce - tu od razu, żeby zrzut
        # i wysokość były już docelowe
        lay = body.layout()
        if lay is not None:
            k.show_pending(lay)
        page = self.pages.currentWidget()
        if page is not None:
            # do skutku: elementy dopasowujące się do szerokości mogą zmienić wysokość jeszcze
            # raz - na końcu animacji nic nie może dociągać
            last = -1
            for _ in range(6):
                QCoreApplication.sendPostedEvents()
                settle_layout(page)
                if body.height() == last:
                    break
                last = body.height()
        cover.morph(top, old_h, body.height(), th.T_MORPH)

    def quietly(self, change: Any, ms: int = th.T_COLOR) -> None:
        """Zmiana układu bez mrugania: zrzut przed, zmiana i ułożenie pod spodem, przenikanie."""
        cover = self.pages.cover if (self.window is not None and self.window.isVisible()) else None
        if cover is not None:
            cover.capture(self.pages.background)
        try:
            change()
            current = self.pages.currentWidget()
            if current is not None:
                settle_layout(current)
        finally:
            if cover is not None:
                cover.reveal(ms)

    def go_back(self, index: int) -> None:
        if index == self.SETTINGS:
            self.refresh_settings()  # przebudowa tylko, gdy nazwy, kolory, profile się zmieniły
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
        # miejsce na pasek zawsze zarezerwowane - treść nie przesuwa się w bok, gdy strona
        # zrobi się dłuższa niż okno; bez czego przewijać - pasek niewidoczny
        area.setVerticalScrollBar(CalmScrollBar())
        area.setVerticalScrollBarPolicy(Qt.ScrollBarPolicy.ScrollBarAlwaysOn)
        area.setWidget(inner)
        return area

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
        self.quietly(self._restyle, th.T_PANEL_IN)  # motyw przenika, zamiast przeskakiwać

    def _restyle(self) -> None:
        assert self.window is not None
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
        self.pages.background = self.t.bg

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
        area = self.home_screen().availableGeometry()  # domyślnie: prawa krawędź, środek
        self.bubble.move(area.right() - FloatingToggle.SIZE - 24, area.center().y())

    def on_bubble_moved(self, point: QPoint) -> None:
        self.change("floating_pos", [point.x(), point.y()])

    # --- pad ------------------------------------------------------------------

    def make_hotkeys(self) -> Hotkeys:
        keys = self.config["hotkeys"]
        return Hotkeys(
            {
                "toggle": (self.toggle_key.vk, self.toggle_key.mods),
                "sim_slower": keys["sim_slower"],
                "sim_faster": keys["sim_faster"],
            }
        )

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

    def apply_recording(self) -> None:
        recorder = Recorder(Path("nagrania")) if self.settings["record"] else None
        self.bridge.set_recording(recorder)
        if recorder:
            log.info("Nagrywam do %s", recorder.path)

    def on_record(self, on: bool) -> None:
        self.change("record", on)
        self.apply_recording()

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
        if kind == "konto":
            self.on_account_result(data)
        elif kind == "models":
            self.on_models_result(data)
        elif kind == "update_status":
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
        for action, step in (("sim_faster", 1), ("sim_slower", -1)):
            if action in actions:
                actions.discard(action)
                if self.settings["simulator"] and not capturing:
                    self.worker.sim_remote(step)  # pilot symulowanej bieżni
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
        sends = direction != "treadmill" and state.enabled and settled and not capturing
        if sends and direction == "gamepad":
            self.send_pad(target)
        elif sends and target != self.belt_target:
            self.belt_target = target
            self.worker.set_speed(target)
        off = f"Wyłączone - {self.toggle_key.label} włącza"
        zone = {"idle": 0, "walk": 1, "run": 2}
        offline = not state.live
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
        self.bubble.set_state(state.enabled, mode, c.speed, state.live)

        self.frame += 1
        if self.frame % 5 == 0:  # liczby 4x na sekundę wystarczą
            self.stat_steps.set(str(state.steps) if state.steps is not None else "-")
            self.stat_cadence.set(f"{state.cadence:.0f}" if state.cadence else "-")
            self.stat_distance.set(f"{fmt_num(state.distance_km, 2)} km")
            minutes, seconds = divmod(int(state.active_s), 60)
            self.stat_time.set(f"{minutes}:{seconds:02d}")
            if self.tracker is not None and state.live:
                try:
                    self.tracker.update(c.speed, state.steps, state.distance_km, state.active_s)
                except Exception:  # historia nie może zatrzymać sterowania
                    log.exception("Historia kroków")
                    self.tracker = None
            self.render_link()
            self.render_pad_status()
            self.account_tick(time.monotonic(), c.speed, state.steps, state.active_s)

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
        if self.window is not None:
            self.window.hide()  # okno znika od razu; stop taśmy może chwilę potrwać
        self.bridge.shutdown()
        # zamknięcie zawsze zatrzymuje jadącą taśmę (także gdy sterowanie już wyłączone,
        # a bieżnia zgubiła pauzę) - worker sprawdza, czy taśma w ogóle jedzie
        if self.worker.stop_belt_now(timeout=8.0):
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
