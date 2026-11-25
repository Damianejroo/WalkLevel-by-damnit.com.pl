"""Ekran główny: pasek z osobą, wskaźnik ze strefami i żetonami, karta sterowania z profilem."""

from __future__ import annotations

import contextlib
from typing import TYPE_CHECKING

from PySide6.QtCore import Qt, QTimer
from PySide6.QtGui import QPixmap
from PySide6.QtWidgets import (
    QHBoxLayout,
    QLabel,
    QPushButton,
    QScrollArea,
    QVBoxLayout,
    QWidget,
)

from bieznia import __version__, dane
from bieznia.gui import komponenty as k
from bieznia.gui import theme as th
from bieznia.gui.widgets import (
    ElidedLabel,
    StatTile,
    StatusDot,
    Switch,
    TileGrid,
    button,
    card,
    fix_width,
    fmt_num,
    icon_button,
    label,
)
from bieznia.paths import FROZEN

if TYPE_CHECKING:
    from bieznia.gui.app import App

STATUS_ACTIONS = ("Szukaj bieżni", "Anuluj", "Rozłącz", "Spróbuj ponownie", "Wyłącz symulator")


class MainScreen:
    def build_main(self: App) -> QWidget:
        t = self.t
        bar = QHBoxLayout()
        bar.setSpacing(4)
        names = QVBoxLayout()
        names.setSpacing(0)
        names.addWidget(ElidedLabel("Bieżnia Gamingowa", "title"))
        self.update_label = ElidedLabel(f"wersja {__version__}", "small")
        names.addWidget(self.update_label)
        bar.addLayout(names, 1)
        self.person_chip = QPushButton()
        self.person_chip.setObjectName("row")
        self.person_chip.setFixedSize(70, th.CONTROL_H)
        self.person_chip.setToolTip("Zmień osobę")
        self.person_chip.setAccessibleName("Zmień osobę")
        self.person_chip.setStyleSheet("QPushButton#row { padding: 0px; }")
        chip_lay = QHBoxLayout(self.person_chip)
        chip_lay.setContentsMargins(5, 0, 8, 0)
        chip_lay.setSpacing(4)
        self.person_avatar = QLabel()
        self.person_avatar.setFixedSize(32, 32)
        self.person_avatar.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
        chip_lay.addWidget(self.person_avatar)
        chevron = k.glyph(k.G_CHEVRON_DOWN, t.text_2, 12)
        chevron.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
        chip_lay.addWidget(chevron)
        self.person_chip.clicked.connect(self.open_person_popup)
        bar.addWidget(self.person_chip)
        self.update_btn = icon_button("download", "Aktualizuj")
        self.update_btn.clicked.connect(lambda: self.run_task(self.check_update_task, True))
        if not FROZEN:
            self.update_btn.setEnabled(False)
            self.update_btn.setToolTip("Aktualizacje działają w wersji zainstalowanej")
        report_btn = icon_button("report", "Zgłoś problem")
        report_btn.clicked.connect(lambda: self.pages.go(self.REPORT))
        self.online_btn = icon_button("people", "Znajomi i ranking")
        self.online_btn.clicked.connect(self.open_online)
        self.online_btn.setVisible(self.online_visible())  # bez konta funkcji online nie widać
        history_btn = icon_button("chart", "Historia kroków")
        history_btn.clicked.connect(self.open_history)
        settings_btn = icon_button("settings", "Ustawienia")
        settings_btn.clicked.connect(self.open_settings)
        for b in (self.update_btn, report_btn, self.online_btn, history_btn, settings_btn):
            bar.addWidget(b)
        page, lay = self.framed_page(bar)

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
        self.status_title.setObjectName("strong")
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

        hero = card()
        hlay = QVBoxLayout(hero)
        hlay.setContentsMargins(14, 10, 14, 14)
        hlay.setSpacing(8)
        self.gauge = k.ZoneGauge(t)
        hlay.addWidget(self.gauge)
        self.gauge_status = QLabel("")
        self.gauge_status.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.gauge_status.setStyleSheet("font-weight: 700; font-size: 15px;")
        hlay.addWidget(self.gauge_status)
        self.zone_chips = k.ZoneChips(t)
        hlay.addWidget(self.zone_chips)
        lay.addWidget(hero)

        self.stat_steps = StatTile("Kroki")
        self.stat_cadence = StatTile("Kroki/min")
        self.stat_distance = StatTile("Dystans")
        self.stat_time = StatTile("Czas ruchu")
        lay.addWidget(
            TileGrid(self.stat_steps, self.stat_cadence, self.stat_distance, self.stat_time)
        )

        control = card()
        clay = QVBoxLayout(control)
        clay.setContentsMargins(14, 12, 14, 14)
        clay.setSpacing(10)
        head = QHBoxLayout()
        head.setSpacing(10)
        self.profile_icon = QLabel()
        self.profile_icon.setFixedSize(40, 40)
        head.addWidget(self.profile_icon)
        htexts = QVBoxLayout()
        htexts.setSpacing(2)
        self.profile_btn = QPushButton()
        self.profile_btn.setFlat(True)
        self.profile_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        self.profile_btn.setStyleSheet(
            "QPushButton { border: none; background: transparent; text-align: left;"
            " font-size: 20px; font-weight: 700; padding: 0px; }"
        )
        self.profile_btn.clicked.connect(self.open_profile_popup)
        htexts.addWidget(self.profile_btn)
        sub = QHBoxLayout()
        sub.setSpacing(6)
        self.toggle_cap = k.KeyCap(t, self.toggle_key.label)
        self.toggle_cap.clicked.connect(self.change_toggle_key)
        sub.addWidget(self.toggle_cap)
        self.control_subtitle = label("", "small")
        sub.addWidget(self.control_subtitle, 1)
        htexts.addLayout(sub)
        head.addLayout(htexts, 1)
        self.enable_switch = Switch(t, "Sterowanie w grze", self.bridge.state.enabled)
        self.enable_switch.toggled.connect(self.on_enable)
        head.addWidget(self.enable_switch)
        clay.addLayout(head)
        self.mode_tiles = k.ModeTiles(t, self.settings["direction"])
        self.mode_tiles.changed.connect(self.on_direction)
        clay.addWidget(self.mode_tiles)
        self.output_hint = label("", "small")
        self.output_hint.setWordWrap(True)
        clay.addWidget(self.output_hint)
        lay.addWidget(control)
        lay.addStretch(1)

        self.profile_popup: k.Popup | None = None
        self.person_popup: k.Popup | None = None
        return page

    # --- nagłówek: osoba i profil ------------------------------------------------------

    def render_header(self: App) -> None:
        """Osoba na pasku i profil na karcie sterowania - przełączanie tylko, gdy jest z czego."""
        t = self.t
        store = self.store
        people = len(store.people) if store else 1
        show_person = people > 1 and bool(self.settings.get("show_person_switch", True))
        self.person_chip.setVisible(show_person)
        version = f"wersja {__version__}"
        if store is not None:
            person = store.person
            self.person_avatar.setPixmap(k.avatar_pixmap(t, 32, person.name, person.color))
            self.update_label.setText(f"{person.name} · {version}" if people > 1 else version)
            profile = store.profile
            many = len(store.person.profiles) > 1 and bool(
                self.settings.get("show_profile_switch", True)
            )
            self.profile_icon.setPixmap(self.profile_pixmap(profile, 40))
            self.profile_btn.setText(
                f"{profile.name}  {k.G_CHEVRON_DOWN}" if many else profile.name
            )
            self.profile_btn.setEnabled(many)
            self.profile_btn.setAccessibleName(
                f"Profil: {profile.name}" + (". Kliknij, żeby zmienić" if many else "")
            )
        else:
            self.profile_icon.setPixmap(k.profile_pixmap(t, 40, "Mięta", k.G_WALK, None))
            self.profile_btn.setText("Sterowanie")
            self.profile_btn.setEnabled(False)
        self.toggle_cap.setText(self.toggle_key.label)
        self.render_control_subtitle()

    def profile_pixmap(self: App, profile: dane.Profile, size: int) -> QPixmap:
        icon = dane.ICONS_DIR / profile.icon_file if profile.icon_file else None
        return k.profile_pixmap(self.t, size, profile.color, profile.glyph, icon)

    def open_profile_popup(self: App) -> None:
        store, w = self.store, self.window
        if store is None or w is None:
            return
        if self.profile_popup is not None:
            with contextlib.suppress(RuntimeError):
                self.profile_popup.deleteLater()
        pop = k.Popup(self.t, self.pages.currentWidget() or w)
        for p in store.person.profiles:
            row = pop.add_row(
                self.profile_pixmap(p, 28), 28, p.name, "", p.id == store.profile.id, self.t.accent
            )
            row.clicked.connect(lambda _=False, pid=p.id: self.choose_profile(pid))
        pop.add_link("Zarządzaj profilami", self.t.accent).clicked.connect(self.open_settings)
        self.profile_popup = pop
        pop.open_under(self.profile_btn, 300)

    def open_person_popup(self: App) -> None:
        store, w = self.store, self.window
        if store is None or w is None:
            return
        if self.person_popup is not None:
            with contextlib.suppress(RuntimeError):
                self.person_popup.deleteLater()
        pop = k.Popup(self.t, self.pages.currentWidget() or w)
        for p in store.people:
            today = self.history_today(p.id) if self.history else 0
            row = pop.add_row(
                k.avatar_pixmap(self.t, 32, p.name, p.color),
                32,
                p.name,
                f"dziś {fmt_num(today, 0)} kroków",
                p.id == store.person.id,
                self.t.accent,
            )
            row.clicked.connect(lambda _=False, pid=p.id: self.choose_person(pid))
        pop.add_link("Dodaj osobę", self.t.accent).clicked.connect(self.open_wizard_add)
        pop.add_link("Zarządzaj osobami", self.t.accent).clicked.connect(self.open_settings)
        self.person_popup = pop
        pop.open_under(self.person_chip, 280)

    def change_toggle_key(self: App) -> None:
        """Klik w klawisz na karcie: Ustawienia > Bieżnia, od razu czeka na nowy klawisz."""
        self.open_settings()
        button = self.toggle_btn

        def start() -> None:
            page = self.pages.currentWidget()
            area = page.findChild(QScrollArea) if page else None
            with contextlib.suppress(RuntimeError):
                if area is not None:
                    area.ensureWidgetVisible(button, 0, 120)
                button.setFocus()
                button.start()

        QTimer.singleShot(0, start)

    def close_popups(self: App) -> None:
        for pop in (self.profile_popup, self.person_popup):
            if pop is not None:
                with contextlib.suppress(RuntimeError):  # usunięta razem z widokiem
                    pop.hide()

    # --- strefy wskaźnika ----------------------------------------------------------------------

    def render_zones(self: App) -> None:
        """Strefy, progi i żetony dla bieżącego trybu (chodzenie / klawisze / pad)."""
        t, s = self.t, self.settings
        walk, run = self.bridge.keys
        direction = s["direction"]
        if direction == "treadmill":
            lo, hi = float(s["stand_below"]), float(s["run_on"])
            top = float(self.config["mapping"]["max_speed"])
            zones = [(0.0, lo, t.text_2), (lo, hi, t.accent), (hi, top, t.run)]
            ticks = [(lo, fmt_num(lo), 1), (hi, fmt_num(hi), 2)]
            chips = [
                ("Stoisz", f"poniżej {fmt_num(lo)}", t.text_2),
                (f"Chód · {walk.name}", f"od {fmt_num(lo)} km/h", t.accent),
                (f"Bieg · {run.name}+{walk.name}", f"od {fmt_num(hi)} km/h", t.run),
            ]
            vmax = top
        elif direction == "keyboard":
            speeds = [float(s["idle_speed"]), float(s["walk_speed"]), float(s["run_speed"])]
            vmax = max(3.5, max(speeds) * 1.4)
            cols = [t.text_2, t.accent, t.run]
            cuts = [0.0, (speeds[0] + speeds[1]) / 2, (speeds[1] + speeds[2]) / 2, vmax]
            zones = [(cuts[i], cuts[i + 1], cols[i]) for i in range(3)]
            ticks = [(v, fmt_num(v), i) for i, v in enumerate(speeds)]
            chips = [
                ("Bez klawisza", f"{fmt_num(speeds[0])} km/h", cols[0]),
                (walk.name, f"{fmt_num(speeds[1])} km/h", cols[1]),
                (f"{run.name} + {walk.name}", f"{fmt_num(speeds[2])} km/h", cols[2]),
            ]
        else:
            steps = s["pad_steps"]
            idle = self.pad_idle_text()
            speeds = [0.0 if (i == 0 and idle) else float(sp) for i, (_a, sp) in enumerate(steps)]
            vmax = max(3.0, max(speeds) * 1.25)
            n = len(steps)
            cols = [t.text_2] + [t.accent] * (n - 2) + [t.run]
            cuts = [0.0] + [(speeds[i] + speeds[i + 1]) / 2 for i in range(n - 1)] + [vmax]
            zones = [(cuts[i], cuts[i + 1], cols[i]) for i in range(n)]
            ticks = [(v, fmt_num(v), i) for i, v in enumerate(speeds) if v > 0]
            chips = []
            many = n > 4  # 5 żetonów: krócej, jednostka jest pod liczbą na wskaźniku
            for i, (at, sp) in enumerate(steps):
                name = ("0%" if many else "Puszczona") if i == 0 else f"{at}%"
                unit = "" if many else " km/h"
                value = idle if (i == 0 and idle) else f"{fmt_num(float(sp))}{unit}"
                chips.append((name, value or "", cols[i]))
        self.gauge.configure(zones, ticks, vmax)
        self.zone_chips.set_chips(chips)
