"""Ustawienia: 7 kategorii rozwijanych w miejscu (akordeon, otwarta najwyżej jedna).
Karta sterowania przebudowuje się przy zmianie trybu i profilu."""

from __future__ import annotations

import contextlib
import json
import logging
import time
from pathlib import Path
from typing import TYPE_CHECKING, Any

from PySide6.QtCore import (
    QAbstractAnimation,
    QEasingCurve,
    QPoint,
    QPropertyAnimation,
    Qt,
    QTimer,
    QUrl,
)
from PySide6.QtGui import QDesktopServices
from PySide6.QtWidgets import (
    QHBoxLayout,
    QLabel,
    QPushButton,
    QScrollArea,
    QVBoxLayout,
    QWidget,
)

from bieznia import HOMEPAGE, REPOSITORY, __author__, __version__, kopia
from bieznia import pad as gamepad
from bieznia.gui import komponenty as k
from bieznia.gui import theme as th
from bieznia.gui.widgets import (
    HoldButton,
    KeyButton,
    Motion,
    ResponsiveRow,
    Segmented,
    Switch,
    button,
    card,
    fmt_num,
    label,
)
from bieznia.mapping import MappingConfig
from bieznia.paths import FROZEN
from bieznia.sterowniki import model_name

if TYPE_CHECKING:
    from bieznia.gui.app import App

log = logging.getLogger("bieznia")
HYSTERESIS = 0.6
# zmieniają się w tle, a w Ustawieniach albo ich nie widać, albo odświeżają się na żywo
NOT_IN_SETTINGS_VIEW = frozenset(
    {"last_version", "window_geometry", "floating_pos", "device_address", "device_name"}
)
MODE_NAMES = {
    "keyboard": "Klawisze",
    "gamepad": "Pad",
    "treadmill": "Chodzenie",
    "training": "Trening",
}
# kategorie Ustawień: klucz, ikona (Segoe Fluent), nazwa, opis
CATEGORIES = (
    ("people", "\ue77b", "Konto i osoby", "Osoby, konto online, kto widzi wyniki"),
    ("control", "\ue7fc", "Gry i sterowanie", "Profile gier, tryb sterowania, klawisz włączania"),
    ("treadmill", k.G_WALK, "Bieżnia", "Start i stop taśmy, symulator, obsługiwane modele"),
    ("levels", "\ue734", "Poziomy i powiadomienia", "Popupy osiągnięć nad grą, dźwięk"),
    ("look", "\ue771", "Wygląd i okno", "Motyw, kolor, tekst, okno, przycisk na ekranie"),
    ("data", "\ue74e", "Dane i kopia", "Eksport, import, folder danych"),
    ("help", "\ue9d9", "Pomoc i diagnostyka", "Zgłoś problem, logi, aktualizacje"),
)


def section(title: str) -> tuple[QWidget, QVBoxLayout]:
    c = card()
    lay = QVBoxLayout(c)
    lay.setContentsMargins(14, 12, 14, 14)
    lay.setSpacing(8)
    lay.addWidget(label(title, "section"))
    return c, lay


def clear(lay: Any) -> None:
    while (item := lay.takeAt(0)) is not None:
        if (w := item.widget()) is not None:
            w.hide()
            w.deleteLater()
        elif (sub := item.layout()) is not None:
            clear(sub)


class SettingsScreen:
    belt_target: float | None
    settings_view: str  # tryb, którego ustawienia widać na karcie (nie musi być aktywny)
    settings_built: str  # stan, z którego zbudowano widok Ustawień
    settings_category: str | None  # rozwinięta kategoria (najwyżej jedna)
    settings_fold_token: int  # numer kliknięcia - opóźnione rozwinięcie po nowszym przepada
    settings_building = ""  # kategoria, której nagłówek właśnie powstaje

    def open_settings(self: App) -> None:
        self.close_popups()
        self.settings_view = self.settings["direction"]
        self.refresh_settings()
        self.pages.go(self.SETTINGS)

    def settings_stamp(self: App) -> str:
        """Wszystko, co widać w Ustawieniach - gdy się nie zmieniło, widok nie jest budowany od
        nowa (wejście bez opóźnienia). Kroki „dziś” odświeżają się co minutę."""
        store = self.store
        return json.dumps(
            [
                {k: v for k, v in self.settings.items() if k not in NOT_IN_SETTINGS_VIEW},
                [
                    [p.id, p.name, p.color, p.active_profile]
                    + [
                        [
                            q.id,
                            q.name,
                            q.color,
                            q.glyph,
                            q.icon_file,
                            self.profile_summary(q.settings),
                        ]
                        for q in p.profiles
                    ]
                    for p in store.people
                ]
                if store
                else None,
                store.active_person if store else None,
                self.t.name,
                self.t.accent,
                self.settings_view,
                [self.caps.speed_min, self.caps.speed_max, self.caps.start_delay_s],
                [vars(a.state) for a in self.accounts.logged_in()]
                if hasattr(self, "accounts")
                else None,
                str(kopia.latest_backup()),
                int(time.time() // 60),
            ],
            sort_keys=True,
            default=str,
        )

    def refresh_settings(self: App) -> None:
        if getattr(self, "settings_built", "") != self.settings_stamp():
            self.rebuild_settings()

    def rebuild_settings(self: App) -> None:
        self.replace_page(self.SETTINGS, self.build_settings())
        self.settings_built = self.settings_stamp()

    def build_settings(self: App) -> QWidget:
        t = self.t
        page, lay = self.sticky_page("Ustawienia")
        self.profile_rows: dict[str, k.ChoiceRow] = {}
        store = self.store
        folds: dict[str, QVBoxLayout] = {key: QVBoxLayout() for key, *_ in CATEGORIES}

        # --- Konto i osoby --------------------------------------------------------------------
        if store is not None:
            c, cl = section("Osoby")
            cl.addWidget(
                label("Każda osoba ma swoje profile gier, ustawienia i historię kroków.", "small")
            )
            for p in store.people:
                today = self.history_today(p.id) if self.history else 0
                linked = self.accounts.items.get(p.id) if hasattr(self, "accounts") else None
                online = (
                    f" · konto {linked.state.name}" if linked and linked.state.logged_in else ""
                )
                row = k.ChoiceRow(
                    t,
                    k.avatar_pixmap(t, 38, p.name, p.color),
                    38,
                    p.name,
                    f"{len(p.profiles)} prof. gier · dziś {fmt_num(today, 0)} kroków{online}",
                    p.id == store.person.id,
                    t.accent,
                )
                row.clicked.connect(lambda _=False, pid=p.id: self.choose_person(pid))
                row.edit.connect(lambda pid=p.id: self.open_person_edit(pid))
                cl.addWidget(row)
            add = k.AddRow(t, "Dodaj osobę")
            add.clicked.connect(self.open_wizard_add)
            cl.addWidget(add)
            if getattr(self, "accounts", None) is not None and self.accounts.enabled:
                add_online = k.AddRow(t, "Dodaj osobę z kontem online")
                add_online.clicked.connect(lambda: self.open_account("login", new_person=True))
                cl.addWidget(add_online)
            folds["people"].addWidget(c)
        account_card = self.build_account_card()
        if account_card is not None:
            folds["people"].addWidget(account_card)

        # --- Gry i sterowanie -----------------------------------------------------------------
        if store is not None:
            c, cl = section(f"Profile gier · {store.person.name}")
            cl.addWidget(
                label("Ustawienia sterowania i kolor zapisują się w zaznaczonym profilu.", "small")
            )
            for pr in store.person.profiles:
                row = k.ChoiceRow(
                    t,
                    self.profile_pixmap(pr, 38),
                    38,
                    pr.name,
                    self.profile_summary(pr.settings),
                    pr.id == store.profile.id,
                    t.accent,
                )
                row.clicked.connect(lambda _=False, pid=pr.id: self.choose_profile(pid))
                row.edit.connect(lambda pid=pr.id: self.open_profile_edit(pid))
                self.profile_rows[pr.id] = row
                cl.addWidget(row)
            add = k.AddRow(t, "Nowy profil")
            add.clicked.connect(self.open_profile_new)
            cl.addWidget(add)
            folds["control"].addWidget(c)

        title = "Sterowanie"
        if store is not None:
            title += f" · {store.profile.name}"
        c, cl = section(title)
        # przełącznik trybów stoi nad przebudowywaną częścią - podświetlenie płynnie jedzie
        self.mode_view = Segmented(list(MODE_NAMES.items()), self.settings_view, "Ustawienia trybu")
        self.mode_view.changed.connect(self.show_mode_settings)
        cl.addWidget(self.mode_view)
        self.control_body = k.SmoothBox()  # zmiana trybu: karta płynnie rośnie albo maleje
        self.control_box = self.control_body.lay
        cl.addWidget(self.control_body)
        folds["control"].addWidget(c)
        self.build_control_settings()

        c, cl = section("Klawisz włączania sterowania")
        self.toggle_btn = KeyButton("Start/stop", self.toggle_key, combos=True)
        self.toggle_btn.captured.connect(lambda key: self.on_key("toggle", key))
        cl.addWidget(
            label(
                "Kliknij i naciśnij klawisz albo kombinację, np. Ctrl+Shift+F8. Działa też w grze."
                " Bez skrótów Windows (Win, Alt+Tab, Alt+F4) i nakładek (Shift+Tab, Alt+Z).",
                "small",
            )
        )
        cl.addWidget(self.toggle_btn)
        pad_row = QHBoxLayout()
        self.pad_toggle_btn = button("")
        self.pad_toggle_btn.setToolTip(
            "Przycisk pada działa jak klawisz włączania - też w grze. Najlepiej taki, którego"
            " gra nie używa: panel dotykowy, Create albo kliknięcie gałki."
        )
        self.pad_toggle_btn.clicked.connect(self.capture_pad_button)
        self.pad_clear_btn = button("Usuń")
        self.pad_clear_btn.clicked.connect(lambda: self.set_toggle_pad(""))
        pad_row.addWidget(self.pad_toggle_btn, 1)
        pad_row.addWidget(self.pad_clear_btn)
        cl.addWidget(label("Albo przycisk na padzie (DualSense, DualShock, Xbox)", "small"))
        cl.addLayout(pad_row)
        self.render_pad_button()
        sw = Switch(
            t, "Zmiana profilu na karcie sterowania", bool(self.settings["show_profile_switch"])
        )
        sw.toggled.connect(lambda v: self.on_switch_visibility("show_profile_switch", v))
        self.setting_row(
            cl,
            "Zmiana profilu na karcie sterowania",
            "Lista profili po kliknięciu nazwy (przy jednym profilu i tak jej nie ma)",
            sw,
        )
        folds["control"].addWidget(c)

        # --- Bieżnia ------------------------------------------------------------------------
        c, bl = section("Taśma i połączenie")
        self.device_label = label("", "muted")
        bl.addWidget(self.device_label)
        self.belt_switch = Switch(t, "Start i stop bieżni", self.settings["belt_control"])
        self.belt_switch.toggled.connect(self.on_belt_control)
        self.setting_row(
            bl,
            "Klawisz uruchamia i zatrzymuje taśmę",
            "Wyłącz, jeśli wolisz startować tylko pilotem",
            self.belt_switch,
        )
        sim = Switch(t, "Symulator bieżni", self.settings["simulator"])
        sim.toggled.connect(self.on_simulator)
        keys = self.config["hotkeys"]
        self.setting_row(
            bl,
            "Symulator bieżni",
            f"Bez bieżni: {keys['sim_slower']} wolniej, {keys['sim_faster']} szybciej",
            sim,
        )
        models = button("Obsługiwane bieżnie…")
        models.setToolTip("Lista modeli, pobieranie nowych i dodawanie profilu z pliku")
        models.clicked.connect(self.open_models)
        self.forget_btn = HoldButton("Zapomnij bieżnię", t)
        self.forget_btn.confirmed.connect(self.forget_device)
        bl.addWidget(ResponsiveRow(models, self.forget_btn))
        folds["treadmill"].addWidget(c)

        # --- Poziomy i powiadomienia --------------------------------------------------------
        c, ll = section("Popupy osiągnięć i awansów")
        for key, title_, hint, default in (
            (
                "achievement_popups",
                "Popupy na ekranie",
                "Osiągnięcia i awanse w prawym dolnym rogu, także nad grą."
                " Nie zabierają fokusu, kliknięcia przechodzą do gry",
                True,
            ),
            ("achievement_sound", "Dźwięk przy popupie", "Krótki dźwięk przy zdobyciu", False),
        ):
            sw = Switch(t, title_, bool(self.settings.get(key, default)))
            sw.toggled.connect(lambda v, k=key: self.change(k, v))
            self.setting_row(ll, title_, hint, sw)
        preview = button("Pokaż przykładowy popup")
        preview.clicked.connect(self.show_sample_popup)
        ll.addWidget(preview)
        folds["levels"].addWidget(c)

        # --- Wygląd i okno ------------------------------------------------------------------
        c, wl = section("Motyw i okno")
        if store is not None:
            wl.addWidget(label(f"Kolor · zapisuje się w profilu {store.profile.name}"))
            self.palette = k.PalettePicker(t, store.profile.color, store.profile.icon_color)
            self.palette.picked.connect(self.on_profile_color)
            wl.addWidget(self.palette)
        theme_seg = Segmented(
            [("system", "Jak Windows"), ("light", "Jasny"), ("dark", "Ciemny")],
            self.settings["theme"],
            "Motyw",
        )
        theme_seg.changed.connect(lambda v: self.on_look("theme", v))
        wl.addWidget(theme_seg)
        for key, title_, hint, handler in (
            (
                "large_text",
                "Duży tekst",
                "Wszystkie napisy większe o 15%",
                lambda v: self.on_look("large_text", v),
            ),
            (
                "always_on_top",
                "Zawsze na wierzchu",
                "Okno nie chowa się pod grą",
                self.on_always_on_top,
            ),
            (
                "floating_button",
                "Przycisk na ekranie",
                "Klik włącza i wyłącza sterowanie, prawy przycisk otwiera okno",
                self.on_floating_button,
            ),
            (
                "show_person_switch",
                "Zmiana osoby na pasku",
                "Awatar z listą osób (przy jednej osobie i tak go nie ma)",
                lambda v: self.on_switch_visibility("show_person_switch", v),
            ),
        ):
            sw = Switch(t, title_, bool(self.settings.get(key, False)))
            sw.toggled.connect(handler)
            self.setting_row(wl, title_, hint, sw)
        folds["look"].addWidget(c)

        # --- Dane i kopia ------------------------------------------------------------------
        if store is not None:
            c, kl = section("Kopia danych")
            kl.addWidget(
                label(
                    "Przenieś osoby, profile gier i wyniki na inny komputer albo zrób kopię.",
                    "small",
                )
            )
            export_btn = button("Eksportuj…")
            export_btn.clicked.connect(self.open_export)
            import_btn = button("Importuj…")
            import_btn.clicked.connect(self.open_import)
            kl.addWidget(ResponsiveRow(export_btn, import_btn))
            backup = kopia.latest_backup()
            if backup is not None:
                restore = HoldButton("Przywróć dane sprzed ostatniego importu", t)
                restore.confirmed.connect(lambda b=backup: self.restore_backup(b))
                kl.addWidget(restore)
            data_btn = button("Folder danych")
            data_btn.setToolTip("Osoby, profile, historia i ustawienia na tym komputerze")
            data_btn.clicked.connect(lambda: self.open_folder(Path("dane")))
            kl.addWidget(data_btn)
            folds["data"].addWidget(c)

        # --- Pomoc i diagnostyka ----------------------------------------------------------
        c, hl = section("Diagnostyka")
        report = button("Zgłoś problem…", "primary")
        report.clicked.connect(lambda: self.pages.go(self.REPORT))
        hl.addWidget(report)
        detailed = Switch(t, "Szczegółowe logi Bluetooth", self.settings["detailed_logs"])
        detailed.toggled.connect(self.on_detailed_logs)
        self.setting_row(
            hl,
            "Szczegółowe logi Bluetooth",
            "Rozmowa z bieżnią w logach - pomaga naprawić połączenie."
            " Bez klawiszy i danych z gier.",
            detailed,
        )
        rec = Switch(t, "Nagrywaj przejścia", self.settings["record"])
        rec.toggled.connect(self.on_record)
        self.setting_row(hl, "Nagrywaj przejścia", "Odczyty trafiają do folderu nagrania", rec)
        journal = button("Dziennik")
        journal.clicked.connect(lambda: self.pages.go(self.LOG))
        folder = button("Folder logów")
        folder.clicked.connect(lambda: self.open_folder(Path("logs")))
        hl.addWidget(ResponsiveRow(journal, folder))
        folds["help"].addWidget(c)
        c, ul = section("Aktualizacje i o aplikacji")
        self.about_status = label("", "muted")
        self.about_status.setWordWrap(True)
        ul.addWidget(self.about_status)
        check = button("Sprawdź aktualizacje")
        check.clicked.connect(lambda: self.run_task(self.check_update_task, True))
        check.setEnabled(FROZEN)
        ul.addWidget(check)
        links = []
        for text, url in (
            ("Strona autora", HOMEPAGE),
            ("Prywatność", f"https://github.com/{REPOSITORY}/blob/main/POLITYKA-PRYWATNOSCI.md"),
            ("Kod", f"https://github.com/{REPOSITORY}"),
        ):
            links.append(f"<a style='color:{t.accent}' href='{url}'>{text}</a>")
        about = label(
            f"WalkLevel {__version__} · © 2026 {__author__}<br>" + " · ".join(links),
            "small",
        )
        about.setOpenExternalLinks(False)
        about.linkActivated.connect(lambda url: QDesktopServices.openUrl(QUrl(url)))
        ul.addWidget(about)
        folds["help"].addWidget(c)

        # --- akordeon: kategorie rozwijane w miejscu, otwarta najwyżej jedna ---------------
        self.settings_folds: dict[str, tuple[QWidget, QLabel, QPushButton]] = {}
        self.settings_value_labels: dict[str, QLabel] = {}
        container = QWidget()
        cont = QVBoxLayout(container)
        cont.setContentsMargins(0, 0, 0, 0)
        cont.setSpacing(th.GAP)
        values = self.settings_values()
        for key, icon, name, hint in CATEGORIES:
            if folds[key].count() == 0:
                continue
            open_now = key == self.settings_category
            self.settings_building = key
            header, chevron = self.category_header(icon, name, hint, values.get(key, ""), open_now)
            header.clicked.connect(lambda _=False, cat=key: self.toggle_settings_category(cat))
            cont.addWidget(header)
            body = QWidget()
            folds[key].setContentsMargins(0, 0, 0, 6)
            folds[key].setSpacing(th.GAP)
            body.setLayout(folds[key])
            cont.addWidget(body)
            if not open_now:
                body.hide()  # widoczność dopiero po włożeniu do układu
            self.settings_folds[key] = (body, chevron, header)
        self.settings_container = container
        lay.addWidget(container)
        lay.addStretch(1)
        self.render_device_label()
        return page

    def settings_values(self: App) -> dict[str, str]:
        """Bieżąca wartość przy nagłówku kategorii - często nie trzeba nawet rozwijać."""
        store = self.store
        out: dict[str, str] = {}
        if store is not None:
            account = ""
            if hasattr(self, "account") and self.account.enabled and self.account.state.logged_in:
                account = f" · konto {self.account.state.name}"
            out["people"] = f"{store.person.name}{account}"
            out["control"] = (
                f"{store.profile.name} · {MODE_NAMES.get(self.settings['direction'], '')}"
                f" · {self.toggle_key.label}"
            )
        out["treadmill"] = (
            "symulator"
            if self.settings.get("simulator")
            else str(self.settings.get("device_name") or "nie wybrano")
        )
        out["levels"] = (
            "popupy włączone" if self.settings.get("achievement_popups", True) else "bez popupów"
        )
        out["look"] = {"system": "jak Windows", "light": "jasny", "dark": "ciemny"}.get(
            str(self.settings.get("theme")), ""
        )
        out["help"] = f"wersja {__version__}"
        return out

    def render_settings_values(self: App) -> None:
        labels = getattr(self, "settings_value_labels", {})
        if not labels:
            return
        values = self.settings_values()
        for key, lbl in labels.items():
            with contextlib.suppress(RuntimeError):
                value = values.get(key, "")
                lbl.setText(lbl.fontMetrics().elidedText(value, Qt.TextElideMode.ElideRight, 200))
                lbl.setToolTip(value)
                lbl.setFixedWidth(min(lbl.fontMetrics().horizontalAdvance(value) + 6, 206))

    def category_header(
        self: App, icon: str, name: str, hint: str, value: str, open_now: bool
    ) -> tuple[QPushButton, QLabel]:
        t = self.t
        btn = QPushButton()
        btn.setObjectName("row")
        btn.setMinimumHeight(64)
        btn.setCursor(Qt.CursorShape.PointingHandCursor)
        btn.setAccessibleName(name)
        lay = QHBoxLayout(btn)
        lay.setContentsMargins(14, 6, 14, 6)
        lay.setSpacing(14)
        box = QLabel(icon)
        box.setFixedSize(40, 40)
        box.setAlignment(Qt.AlignmentFlag.AlignCenter)
        box.setStyleSheet(
            f"background: {t.surface_2}; border-radius: 10px; color: {t.accent};"
            f" font-family: '{th.ICON_FONT}'; font-size: 20px;"
        )
        lay.addWidget(box)
        texts = QVBoxLayout()
        texts.setSpacing(0)
        title = label(name)
        title.setObjectName("strong")
        texts.addWidget(title)
        sub = label(hint, "small")
        sub.setWordWrap(True)
        texts.addWidget(sub)
        lay.addLayout(texts, 1)
        if value:
            val = label("", "small")
            val.setWordWrap(False)
            val.ensurePolished()
            metrics = val.fontMetrics()
            val.setText(metrics.elidedText(value, Qt.TextElideMode.ElideRight, 200))
            val.setToolTip(value)
            val.setFixedWidth(min(metrics.horizontalAdvance(value) + 6, 206))
            lay.addWidget(val)
            self.settings_value_labels[self.settings_building] = val
        chevron = k.glyph(k.G_CHEVRON_DOWN if open_now else k.G_CHEVRON_RIGHT, t.text_2)
        lay.addWidget(chevron)
        for w in btn.findChildren(QWidget):
            w.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
        btn.setProperty("active", open_now)  # otwarta kategoria z akcentem
        return btn, chevron

    def toggle_settings_category(self: App, key: str) -> None:
        self.show_settings_category(None if key == self.settings_category else key)

    def show_settings_category(self: App, key: str | None, animated: bool = True) -> None:
        """Rozwija kategorię (zwija pozostałe) płynną zmianą wysokości.

        Nagłówki (podświetlenie, strzałka) zmieniają się od razu, poza animacją - animowana
        jest tylko treść: przy otwarciu wysuwa się, a karty pod nią zjeżdżają; przy zmianie
        kategorii najpierw krótko zwija się stara, potem rozwija nowa (bez przenikania
        przesuniętych wierszy jeden w drugi)."""
        previous = getattr(self, "settings_category", None)
        self.settings_category = key
        self.settings_fold_token = getattr(self, "settings_fold_token", 0) + 1
        token = self.settings_fold_token
        folds = getattr(self, "settings_folds", {})
        for cat, (_body, chevron, header) in folds.items():
            with contextlib.suppress(RuntimeError):
                chevron.setText(k.G_CHEVRON_DOWN if cat == key else k.G_CHEVRON_RIGHT)
                header.setProperty("active", cat == key)
                header.style().unpolish(header)
                header.style().polish(header)

        def show_only(open_key: str | None) -> None:
            for cat, (body, _chevron, _header) in folds.items():
                with contextlib.suppress(RuntimeError):
                    body.setVisible(cat == open_key)

        container = getattr(self, "settings_container", None)
        on_screen = (
            container is not None
            and self.window is not None
            and self.window.isVisible()
            and self.pages.currentIndex() == self.SETTINGS
        )
        if not (animated and Motion.enabled and on_screen):
            show_only(key)
            return
        assert container is not None
        if previous is not None and key is not None and previous != key:
            fold_ms = th.T_MORPH * 2 // 3
            self.morph_card(container, lambda: show_only(None), fold_ms)

            def unfold() -> None:
                if getattr(self, "settings_fold_token", 0) != token:
                    return  # w międzyczasie kolejny klik - ten krok już nieaktualny
                self.morph_card(container, lambda: show_only(key))
                QTimer.singleShot(th.T_MORPH + 20, lambda: self.scroll_to_category(key))

            QTimer.singleShot(fold_ms + 10, unfold)
            return
        self.morph_card(container, lambda: show_only(key))
        if key is not None:  # po animacji: treść otwartej kategorii w widoku
            QTimer.singleShot(th.T_MORPH + 20, lambda: self.scroll_to_category(key))

    def scroll_to_category(self: App, key: str) -> None:
        """Gdy treść rozwiniętej kategorii wychodzi poza okno - płynny przewin w dół, tak by
        nagłówek został na górze (albo cała treść się zmieściła). Nigdy w górę."""
        fold = getattr(self, "settings_folds", {}).get(key)
        page = self.pages.widget(self.SETTINGS)
        area = page.findChild(QScrollArea) if page is not None else None
        if fold is None or area is None or area.widget() is None:
            return
        body, _chevron, header = fold
        with contextlib.suppress(RuntimeError):
            inner = area.widget()
            top = header.mapTo(inner, QPoint(0, 0)).y() - 8
            bottom = body.mapTo(inner, QPoint(0, 0)).y() + body.height() + 8
            bar = area.verticalScrollBar()
            view = area.viewport().height()
            if bottom <= bar.value() + view:
                return  # wszystko widać
            target = min(top, bottom - view)
            if target <= bar.value():
                return
            anim = QPropertyAnimation(bar, b"value", area)
            anim.setDuration(th.T_PANEL_IN)
            anim.setEasingCurve(QEasingCurve.Type.OutCubic)
            anim.setStartValue(bar.value())
            anim.setEndValue(min(target, bar.maximum()))
            anim.start(QAbstractAnimation.DeletionPolicy.DeleteWhenStopped)

    def profile_summary(self: App, s: dict[str, Any]) -> str:
        merged = {**self.defaults, **s}
        direction = merged.get("direction", "keyboard")
        if direction == "gamepad":
            steps = gamepad.parse_steps(merged.get("pad_steps"))
            return f"Pad · {len(steps)} skoki · do {fmt_num(max(sp for _a, sp in steps))} km/h"
        if direction == "keyboard":
            return (
                f"Klawisze · {fmt_num(float(merged['walk_speed']))} /"
                f" {fmt_num(float(merged['run_speed']))} km/h"
            )
        if direction == "training":
            levels = merged.get("training_levels") or [1.5, 2.5, 3.5, 4.5]
            return f"Trening · {fmt_num(float(levels[0]))}-{fmt_num(float(levels[-1]))} km/h"
        return f"Chodzenie · chód od {fmt_num(float(merged['stand_below']))} km/h"

    # --- karta sterowania (tylko bieżący tryb) ---------------------------------------------

    def build_control_settings(self: App) -> None:
        box = self.control_box
        while box.count():
            item = box.takeAt(0)
            if item is None:
                break
            if (w := item.widget()) is not None:
                w.hide()
                w.deleteLater()
            elif (inner := item.layout()) is not None:
                clear(inner)
        t, s = self.t, self.settings
        active = s["direction"]
        if getattr(self, "settings_view", None) not in MODE_NAMES:
            self.settings_view = active
        direction = self.settings_view
        self.render_mode_view()
        if direction == active:
            box.addWidget(
                label("✓ to aktywny tryb. Wybierz inny, żeby ustawić go zawczasu.", "small")
            )
        else:
            row = QHBoxLayout()
            row.addWidget(
                label(
                    f"Aktywny jest tryb {MODE_NAMES[active]} - te ustawienia zadziałają po"
                    " zmianie trybu.",
                    "small",
                ),
                1,
            )
            use = button(f"Włącz {MODE_NAMES[direction]}")
            use.clicked.connect(lambda: self.on_direction(direction))
            row.addWidget(use)
            box.addLayout(row)
        self.walk_btn = self.run_btn = None
        self.walk_from_step = self.run_from_step = None
        self.step_ladder = None
        self.pad_steppers = []
        if direction in ("treadmill", "keyboard"):
            walk, run = self.bridge.keys
            self.walk_btn = KeyButton("Chód", walk)
            self.run_btn = KeyButton("Bieg", run)
            self.walk_btn.captured.connect(lambda key: self.on_key("walk", key))
            self.run_btn.captured.connect(lambda key: self.on_key("run", key))
            box.addWidget(ResponsiveRow(self.walk_btn, self.run_btn))
        if direction == "treadmill":
            self.output_seg = Segmented(
                [("dry", "Podgląd"), ("keys", "Klawiatura")], s["output"], "Wyjście"
            )
            self.output_seg.changed.connect(self.on_output)
            box.addWidget(self.output_seg)
            top = float(self.config["mapping"]["max_speed"])
            self.walk_from_step = k.Stepper(t, "Chód od", "", float(s["stand_below"]), 0.0, top)
            self.run_from_step = k.Stepper(t, "Bieg od", "", float(s["run_on"]), 0.0, top)
            self.walk_from_step.changed.connect(lambda v: self.on_threshold("walk", v))
            self.run_from_step.changed.connect(lambda v: self.on_threshold("run", v))
            box.addWidget(self.walk_from_step)
            box.addWidget(self.run_from_step)
            box.addWidget(
                label(
                    f"Bieg wyłącza się {fmt_num(HYSTERESIS)} km/h niżej, żeby Shift nie migał.",
                    "small",
                )
            )
            quick = Switch(t, "Szybkie zatrzymanie", s["quick_stop"])
            quick.toggled.connect(self.on_quick_stop)
            self.setting_row(
                box, "Szybkie zatrzymanie postaci", "Gdy taśma gwałtownie hamuje", quick
            )
        elif direction == "keyboard":
            walk, run = self.bridge.keys
            for key, title, sub in (
                ("idle_speed", "Bez klawisza", "taśma zwalnia do"),
                ("walk_speed", walk.name, "chód"),
                ("walk_speed_run", f"{run.name} + {walk.name}", "bieg"),
            ):
                real = "run_speed" if key == "walk_speed_run" else key
                st = k.Stepper(
                    t, title, sub, float(s[real]), self.caps.speed_min, self.caps.speed_max
                )
                st.changed.connect(lambda v, r=real: self.on_belt_speed(r, v))
                box.addWidget(st)
        elif direction == "training":
            keys = self.config["hotkeys"]
            box.addWidget(
                label(
                    f"Zwykłe sterowanie bieżnią z komputera. {self.toggle_key.label} rusza i"
                    f" zatrzymuje, {keys['sim_faster']} i {keys['sim_slower']} zmieniają poziom,"
                    " a na ekranie głównym poziom wybierzesz kliknięciem."
                    " Gra nie dostaje klawiszy.",
                    "small",
                )
            )
            for i, kmh in enumerate(s["training_levels"]):
                st = k.Stepper(
                    t, f"Poziom {i + 1}", "", float(kmh), self.caps.speed_min, self.caps.speed_max
                )
                st.changed.connect(lambda v, n=i: self.on_training_speed(n, v))
                box.addWidget(st)
        else:
            self.pad_status = label(self.pad_status_text(), "small")  # od razu - bez skoku
            self.pad_status.setWordWrap(True)
            box.addWidget(self.pad_status)
            box.addWidget(
                label(
                    "Przeciągnij kółka, żeby zmienić, od jakiego wychylenia działa skok."
                    " Trójkąt to gałka teraz.",
                    "small",
                )
            )
            self.step_ladder = k.StepLadder(t, gamepad.MIN_GAP)
            self.step_ladder.moved.connect(self.on_pad_at)
            box.addWidget(self.step_ladder)
            row = QHBoxLayout()
            row.addWidget(label("Liczba skoków"), 1)
            self.pad_count_seg = Segmented(
                [(str(n), str(n)) for n in range(2, 6)], str(len(s["pad_steps"])), "Liczba skoków"
            )
            self.pad_count_seg.setFixedWidth(220)
            self.pad_count_seg.changed.connect(lambda v: self.on_pad_count(int(v)))
            row.addWidget(self.pad_count_seg)
            box.addLayout(row)
            box.addWidget(label("Gałka puszczona", "small"))
            self.pad_idle_seg = Segmented(
                [("stop", "Taśma staje"), ("slow", "Skok 1"), ("zero", "0 km/h (test)")],
                s["pad_idle"],
                "Gałka puszczona",
            )
            self.pad_idle_seg.changed.connect(self.on_pad_idle)
            box.addWidget(self.pad_idle_seg)
            delay = self.caps.start_delay_s
            if delay >= 2 and s["pad_idle"] == "stop":
                tip = label(
                    f"Ta bieżnia po zatrzymaniu rusza dopiero po ok. {delay:.0f} s odliczania."
                    " „Skok 1” trzyma taśmę w ruchu - gałka działa od razu.",
                    "small",
                )
                tip.setWordWrap(True)
                box.addWidget(tip)
            idle = self.pad_idle_text()
            for i, (at, speed) in enumerate(s["pad_steps"]):
                title = "Gałka puszczona" if i == 0 else f"Skok {i + 1}"
                sub = "" if i == 0 else f"od {at}% wychylenia"
                fmt = (lambda _v, txt=idle: txt) if (i == 0 and idle) else None
                st = k.Stepper(
                    t, title, sub, float(speed), self.caps.speed_min, self.caps.speed_max
                )
                st.set_format(fmt)
                st.changed.connect(lambda v, n=i: self.on_pad_speed(n, v))
                box.addWidget(st)
                self.pad_steppers.append(st)
            self.render_ladder()
        self.render_output_hint()

    def settings_direction_changed(self: App) -> None:
        """Tryb zmieniony, gdy Ustawienia są na ekranie: karta sterowania płynnie, opis profilu
        w miejscu - bez przebudowy całego widoku."""
        store = self.store
        if store is not None and store.profile.id in self.profile_rows:
            with contextlib.suppress(RuntimeError):
                self.profile_rows[store.profile.id].sub.setText(
                    self.profile_summary(store.profile.settings)
                )
        self.settings_view = self.settings["direction"]
        self.morph_card(self.control_body, self.build_control_settings)
        if self.pages.currentIndex() == self.SETTINGS:
            # widoczny widok jest aktualny; ukryty mógł być nieaktualny już wcześniej -
            # wtedy przebuduje się przy wejściu
            self.settings_built = self.settings_stamp()

    def render_mode_view(self: App) -> None:
        """Znaczek ✓ przy aktywnym trybie i zaznaczony tryb, którego ustawienia widać."""
        active = self.settings["direction"]
        with contextlib.suppress(RuntimeError):
            for key, b in self.mode_view.buttons.items():
                text = MODE_NAMES[key] + (" ✓" if key == active else "")
                if b.text() != text:
                    b.setText(text)
            self.mode_view.set_value(self.settings_view)

    def show_mode_settings(self: App, mode: str) -> None:
        """Podgląd i zmiana ustawień innego trybu - aktywny tryb zostaje."""
        self.settings_view = mode
        self.morph_card(self.control_body, self.build_control_settings)

    def render_ladder(self: App) -> None:
        if self.step_ladder is None:
            return
        s, t = self.settings, self.t
        idle = self.pad_idle_text()
        steps = [
            (int(at), idle if (i == 0 and idle) else fmt_num(float(sp)))
            for i, (at, sp) in enumerate(s["pad_steps"])
        ]
        n = len(steps)
        colors = [t.text_2] + [t.accent] * (n - 2) + [t.run]
        self.step_ladder.set_steps(steps, colors)

    # --- obsługa ---------------------------------------------------------------------------------

    def apply_mapping(self: App) -> None:
        s = self.settings
        try:
            mapping = MappingConfig.from_dict(
                {
                    **self.config["mapping"],
                    "stand_below": float(s["stand_below"]),
                    "run_on": float(s["run_on"]),
                    "run_off": float(s["run_on"]) - HYSTERESIS,
                    "quick_stop": bool(s["quick_stop"]),
                }
            )
        except ValueError as exc:
            log.warning("Progi: %s", exc)
            return
        self.bridge.set_mapping(mapping)

    def on_threshold(self: App, which: str, value: float) -> None:
        """Progi chodu i biegu: zmieniony wygrywa, drugi przesuwa się o odstęp."""
        top = float(self.config["mapping"]["max_speed"])
        gap = HYSTERESIS + 0.2
        walk, run = float(self.settings["stand_below"]), float(self.settings["run_on"])
        if which == "walk":
            walk = value
            if run < walk + gap:
                run = min(top, walk + gap)
                walk = min(walk, run - gap)
        else:
            run = value
            if run < walk + gap:
                walk = max(0.0, run - gap)
                run = max(run, walk + gap)
        walk, run = round(walk, 1), round(run, 1)
        for st, v in ((self.walk_from_step, walk), (self.run_from_step, run)):
            if st is not None:
                st.set_value(v)
        self.settings.update(stand_below=walk, run_on=run, run_off=round(run - HYSTERESIS, 1))
        self.apply_mapping()
        self.save()
        self.render_zones()

    def on_quick_stop(self: App, on: bool) -> None:
        self.settings["quick_stop"] = on
        self.apply_mapping()
        self.save()
        log.info("Szybkie zatrzymanie %s", "włączone" if on else "wyłączone")

    def on_pad_at(self: App, index: int, at: int) -> None:
        steps = list(self.settings["pad_steps"])
        steps[index] = (at, steps[index][1])
        self.settings["pad_steps"] = steps
        self.save()
        if index < len(self.pad_steppers):
            self.pad_steppers[index].sub_label.setText(f"od {at}% wychylenia")
        self.render_zones()
        self.render_output_hint()

    def on_pad_speed(self: App, index: int, kmh: float) -> None:
        steps = list(self.settings["pad_steps"])
        steps[index] = (steps[index][0], round(kmh, 1))
        self.settings["pad_steps"] = steps
        self.save()
        self.belt_target = None
        self.render_ladder()
        self.render_zones()
        self.render_output_hint()

    def on_pad_count(self: App, count: int) -> None:
        if count == len(self.settings["pad_steps"]):
            return
        self.change("pad_steps", list(gamepad.PRESETS[count]))
        self.belt_target = None
        self.morph_card(self.control_body, self.build_control_settings)
        self.render_zones()

    def on_pad_idle(self: App, mode: str) -> None:
        self.change("pad_idle", mode)
        log.info("Pad - gałka puszczona: %s", mode)
        self.belt_target = None
        if self.pad_steppers:  # tylko pierwszy wiersz i oś - bez przebudowy karty
            idle = self.pad_idle_text()
            self.pad_steppers[0].set_format((lambda _v, txt=idle: txt) if idle else None)
            self.render_ladder()
        self.render_zones()
        self.render_output_hint()

    def on_belt_speed(self: App, key: str, kmh: float) -> None:
        self.change(key, round(kmh, 1))
        self.belt_target = None
        self.render_zones()
        self.render_output_hint()

    def on_switch_visibility(self: App, key: str, on: bool) -> None:
        self.change(key, on)
        self.render_header()

    def on_profile_color(self: App, name: str) -> None:
        store = self.store
        if store is None:
            return
        store.update_profile(store.profile.id, color=name)
        self.settings["profile_color"] = name
        log.info("Kolor profilu %s: %s", store.profile.name, name)
        self.apply_look()
        self.render_header()
        self.render_zones()

    def render_device_label(self: App) -> None:
        if not hasattr(self, "device_label"):
            return
        name = self.settings.get("device_name") or ""
        address = self.settings.get("device_address") or ""
        caps, device = self.caps, self.device
        speeds = f"{fmt_num(caps.speed_min)}-{fmt_num(caps.speed_max)} km/h"
        if self.settings["simulator"]:
            text = f"Symulator bieżni · {speeds}"
        elif address:
            model = device.model if device is not None else model_name(name)
            first = model if model == name else f"{model} ({name})"
            details = [speeds]
            if caps.start_delay_s >= 1:
                details.append(f"rozruch ok. {caps.start_delay_s:.0f} s")
            if device is not None and device.firmware:
                details.append(f"firmware {device.firmware}")
            text = "\n".join((first, " · ".join(details)))
        else:
            text = "Brak zapamiętanej bieżni"
        try:
            if self.device_label.text() != text:  # wołane 4x na sekundę - bez zbędnego układu
                self.device_label.setText(text)
            self.forget_btn.setEnabled(bool(address))
        except RuntimeError:
            pass  # karta właśnie przebudowana
