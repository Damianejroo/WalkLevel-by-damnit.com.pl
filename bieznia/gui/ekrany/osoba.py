"""Osoby: konfigurator (pierwsze uruchomienie i „Dodaj osobę”) oraz edycja osoby."""

from __future__ import annotations

import logging
import random
import shutil
from typing import TYPE_CHECKING

from PySide6.QtCore import Qt, QTimer
from PySide6.QtWidgets import (
    QFrame,
    QHBoxLayout,
    QLabel,
    QLayout,
    QLineEdit,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from bieznia import dane, nazwy, paleta
from bieznia.gui import komponenty as k
from bieznia.gui import theme as th
from bieznia.gui.ekrany.profil import GamePick, pick_game
from bieznia.gui.ekrany.ustawienia import section
from bieznia.gui.widgets import HoldButton, Switch, button, label

if TYPE_CHECKING:
    from bieznia.gui.app import App

log = logging.getLogger("bieznia")


class Wizard:
    """Stan konfiguratora (3 kroki)."""

    def __init__(self, mode: str, taken: list[str], used_colors: list[str]) -> None:
        self.mode = mode  # "first" (po instalacji/aktualizacji) | "add"
        self.step = 1
        self.name = nazwy.random_name(taken)
        free = [c for c in paleta.names() if c not in used_colors and c != paleta.DOMYSLNY]
        self.color = random.choice(free or paleta.names())
        self.game: GamePick | None = None
        self.game_name = ""
        self.game_color = paleta.DOMYSLNY
        self.use_game = False
        self.copy_profiles = False
        self.direction = "keyboard"
        self.suggestions = nazwy.suggestions(3, [*taken, self.name])


class PeopleScreen:
    store: dane.Store | None

    # --- konfigurator ------------------------------------------------------------------------

    def open_wizard_first(self: App) -> None:
        self.wizard = Wizard("first", [], [])
        legacy = self.legacy
        if legacy:
            self.wizard.direction = str(legacy.get("direction", "keyboard"))
        self.show_wizard()

    def open_wizard_add(self: App) -> None:
        self.close_popups()
        store = self.store
        taken = store.names() if store else []
        colors = [p.color for p in store.people] if store else []
        self.wizard = Wizard("add", taken, colors)
        self.wizard.direction = str(self.settings.get("direction", "keyboard"))
        self.show_wizard()

    def show_wizard(self: App) -> None:
        self.replace_page(self.WIZARD, self.build_wizard_page())
        self.pages.go(self.WIZARD)

    def build_wizard_page(self: App) -> QWidget:
        wz = self.wizard
        t = self.t
        page = QWidget()
        page.setObjectName("page")
        lay = QVBoxLayout(page)
        lay.setContentsMargins(th.PAD, 20, th.PAD, th.PAD)
        lay.setSpacing(12)
        bars = QHBoxLayout()
        bars.setSpacing(6)
        for i in range(1, 4):
            b = QFrame()
            b.setFixedHeight(6)
            b.setStyleSheet(
                f"background: {t.accent if i <= wz.step else t.surface_2}; border-radius: 3px;"
            )
            bars.addWidget(b, 1)
        lay.addLayout(bars)
        lay.addWidget(label(f"Krok {wz.step} z 3", "small"))
        body = QVBoxLayout()
        body.setSpacing(12)
        lay.addLayout(body)
        if wz.step == 1:
            self.wizard_step_name(body)
        elif wz.step == 2:
            self.wizard_step_game(body)
        else:
            self.wizard_step_mode(body)
        lay.addStretch(1)
        nav = QHBoxLayout()
        nav.setSpacing(8)
        if wz.step > 1 or (wz.mode == "add"):
            back = button("Wstecz" if wz.step > 1 else "Anuluj")
            back.clicked.connect(self.wizard_back)
            nav.addWidget(back, 1)
        nxt = button("Gotowe" if wz.step == 3 else "Dalej", "primary")
        nxt.clicked.connect(self.wizard_next)
        nav.addWidget(nxt, 2)
        lay.addLayout(nav)
        return page

    def wizard_step_name(self: App, body: QVBoxLayout) -> None:
        wz, t = self.wizard, self.t
        updated = wz.mode == "first" and self.legacy is not None
        title = label("WalkLevel ma teraz osoby" if updated else "Jak się nazywasz?", "title")
        title.setStyleSheet("font-size: 24px;")
        body.addWidget(title)
        body.addWidget(
            label(
                "Każda osoba ma swoje profile gier i kroki. Nazwij siebie - Twoje dotychczasowe"
                " ustawienia trafią do tej osoby."
                if updated
                else "Nazwa osoby na tej bieżni. Wylosowaliśmy jedną - możesz losować dalej albo"
                " wpisać swoją.",
                "muted",
            )
        )
        avatar = QLabel()
        avatar.setFixedSize(76, 76)
        avatar.setPixmap(k.avatar_pixmap(t, 76, wz.name, wz.color))
        body.addWidget(avatar, 0, Qt.AlignmentFlag.AlignHCenter)
        row = QHBoxLayout()
        row.setSpacing(8)
        self.wizard_name = QLineEdit(wz.name)
        self.wizard_name.setFixedHeight(th.CONTROL_H)
        self.wizard_name.setAccessibleName("Nazwa osoby")
        self.wizard_name.textChanged.connect(
            lambda txt: avatar.setPixmap(k.avatar_pixmap(t, 76, txt or "?", wz.color))
        )
        row.addWidget(self.wizard_name, 1)
        dice = QPushButton(k.G_SHUFFLE)
        dice.setObjectName("icon")
        dice.setFixedSize(th.CONTROL_H, th.CONTROL_H)
        dice.setAccessibleName("Losuj inną nazwę")
        dice.setToolTip("Losuj inną nazwę")
        taken = self.store.names() if self.store else []

        def reroll() -> None:
            self.wizard_name.setText(nazwy.random_name([*taken, self.wizard_name.text()]))

        dice.clicked.connect(reroll)
        row.addWidget(dice)
        body.addLayout(row)
        body.addWidget(label("Albo jedna z tych:", "small"))
        chips = k.FlowLayout(6)  # obok siebie, w razie potrzeby w kolejnym wierszu
        for n in wz.suggestions:
            c = QPushButton(n)
            c.setFixedHeight(34)
            c.setCursor(Qt.CursorShape.PointingHandCursor)
            c.setStyleSheet(
                f"QPushButton {{ border: 1px solid {t.control}; border-radius: 17px;"
                f" padding: 0 12px;"
                f" color: {t.text}; background: transparent; font-size: 13px; }}"
            )
            c.clicked.connect(lambda _=False, v=n: self.wizard_name.setText(v))
            chips.addWidget(c)
        body.addLayout(chips)
        body.addWidget(label("Kolor osoby", "section"))
        pal = k.PalettePicker(t, wz.color)

        def on_color(c: str) -> None:
            wz.color = c
            avatar.setPixmap(k.avatar_pixmap(t, 76, self.wizard_name.text() or "?", c))

        pal.picked.connect(on_color)
        body.addWidget(pal)

    def wizard_step_game(self: App, body: QVBoxLayout) -> None:
        wz, t = self.wizard, self.t
        title = label("Pierwszy profil gry", "title")
        title.setStyleSheet("font-size: 24px;")
        body.addWidget(title)
        body.addWidget(
            label(
                "Profil to ustawienia sterowania dla jednej gry. Kolejne dodasz kiedy chcesz.",
                "muted",
            )
        )
        game_btn = self.wizard_option(
            k.G_FOLDER,
            "Wskaż folder gry",
            "Nazwa, ikona i kolor wezmą się z pliku gry",
            wz.use_game,
        )
        game_lay = game_btn.layout()
        assert isinstance(game_lay, QVBoxLayout)
        if wz.game is not None:
            g = wz.game
            row = QHBoxLayout()
            row.setSpacing(10)
            icon = dane.ICONS_DIR / g.icon_file if g.icon_file else None
            row.addWidget(k.picture(k.profile_pixmap(t, 40, wz.game_color, None, icon), 40))
            texts = QVBoxLayout()
            texts.setSpacing(0)
            exe = label(g.exe.name)
            exe.setObjectName("strong")
            texts.addWidget(exe)
            texts.addWidget(label(f"…\\{g.exe.parent.name}", "small"))
            row.addLayout(texts, 1)
            color_btn = k.ColorButton(t, wz.game_color)
            row.addWidget(color_btn)
            change = button("Zmień")
            change.setFixedWidth(84)
            change.clicked.connect(self.wizard_pick_game)
            row.addWidget(change)
            game_lay.addLayout(row)
            game_lay.addWidget(label("Nazwa profilu - wpisz dowolną", "small"))
            self.wizard_game_name = QLineEdit(wz.game_name)
            self.wizard_game_name.setFixedHeight(th.CONTROL_H)
            self.wizard_game_name.setAccessibleName("Nazwa profilu gry")
            self.wizard_game_name.textChanged.connect(lambda v: setattr(wz, "game_name", v))
            game_lay.addWidget(self.wizard_game_name)
            picker = k.PalettePicker(t, wz.game_color, g.color)
            picker.setVisible(False)

            def on_color(c: str) -> None:
                wz.game_color = c
                color_btn.set_color(c)

            picker.picked.connect(on_color)
            color_btn.clicked.connect(lambda: picker.setVisible(not picker.isVisible()))
            game_lay.addWidget(picker)
            game_lay.addWidget(
                label(
                    f"Kolor {wz.game_color} dobrany z ikony. Przycisk Kolor otwiera 20 kolorów.",
                    "small",
                )
            )
        if wz.game is None:
            game_btn.clicked.connect(self.wizard_pick_game)
        else:  # gra już wskazana: klik w kartę ją wybiera, Zmień wskazuje inną
            game_btn.clicked.connect(lambda: self.wizard_set(use_game=True))
        body.addWidget(game_btn)
        legacy = wz.mode == "first" and self.legacy is not None
        default_btn = self.wizard_option(
            k.G_WALK,
            "Zostaw moje obecne ustawienia" if legacy else "Profil Domyślny",
            "Profil Domyślny z dotychczasowymi ustawieniami"
            if legacy
            else "Dla dowolnej gry - nazwę i ikonę zmienisz później",
            not wz.use_game,
        )
        default_btn.clicked.connect(lambda: self.wizard_set(use_game=False))
        body.addWidget(default_btn)
        if wz.mode == "add" and self.store is not None:
            sw = Switch(t, "Skopiuj moje profile gier", wz.copy_profiles)
            sw.toggled.connect(lambda v: setattr(wz, "copy_profiles", v))
            self.setting_row(
                body,
                f"Skopiuj profile gier: {self.store.person.name}",
                "Z ikonami i ustawieniami. Historia kroków nowej osoby zaczyna się od zera.",
                sw,
            )

    def wizard_step_mode(self: App, body: QVBoxLayout) -> None:
        wz = self.wizard
        title = label("Jak sterujesz?", "title")
        title.setStyleSheet("font-size: 24px;")
        body.addWidget(title)
        target = wz.game_name if (wz.use_game and wz.game) else "Domyślny"
        body.addWidget(
            label(f"Dla profilu {target}. Wszystko zmienisz później w Ustawieniach.", "muted")
        )
        for key, icon, name, sub in (
            (
                "keyboard",
                k.G_KEYBOARD,
                "Klawisze",
                "Trzymasz W - taśma przyspiesza, puszczasz - zwalnia",
            ),
            ("gamepad", k.G_PAD, "Pad", "Wychylenie gałki ustawia prędkość taśmy skokami"),
            (
                "treadmill",
                k.G_WALK,
                "Chodzenie",
                "Chodzisz po bieżni - gra dostaje W, szybciej W + Shift",
            ),
        ):
            b = self.wizard_option(icon, name, sub, wz.direction == key)
            b.clicked.connect(lambda _=False, d=key: self.wizard_set(direction=d))
            body.addWidget(b)

    def wizard_option(self: App, icon: str, title: str, sub: str, selected: bool) -> QPushButton:
        t = self.t
        b = QPushButton()
        b.setObjectName("row")
        b.setCursor(Qt.CursorShape.PointingHandCursor)
        b.setAccessibleName(f"{title}{' (wybrane)' if selected else ''}")
        border = f"2px solid {t.accent}" if selected else f"1px solid {t.control}"
        b.setStyleSheet(f"QPushButton#row {{ border: {border}; border-radius: 12px; }}")
        bl = QVBoxLayout(b)
        bl.setSizeConstraint(QLayout.SizeConstraint.SetMinimumSize)  # przycisk rośnie z treścią
        bl.setContentsMargins(14, 12, 14, 12)
        bl.setSpacing(8)
        top = QHBoxLayout()
        top.setSpacing(12)
        ic = QLabel(icon)
        ic.setFixedSize(40, 40)
        ic.setAlignment(Qt.AlignmentFlag.AlignCenter)
        ic.setStyleSheet(
            f"font-family: '{th.ICON_FONT}'; font-size: 20px; border-radius: 10px;"
            f" background: {t.surface_2}; color: {t.accent if selected else t.text};"
        )
        top.addWidget(ic)
        texts = QVBoxLayout()
        texts.setSpacing(0)
        n = label(title)
        n.setObjectName("bold")
        texts.addWidget(n)
        texts.addWidget(label(sub, "small"))
        top.addLayout(texts, 1)
        if selected:
            top.addWidget(k.glyph(k.G_CHECK, t.accent))
        for w in (ic, n):
            w.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
        bl.addLayout(top)
        # wysokość z układu (karta gry rośnie o nazwę i kolor) - bez stałego minimum
        return b

    def wizard_set(self: App, **changes: object) -> None:
        for key, value in changes.items():
            setattr(self.wizard, key, value)
        self.show_wizard()

    def wizard_pick_game(self: App) -> None:
        wz = self.wizard
        got = pick_game(self.pages)
        if got is None:
            if wz.game is None:
                self.toast("Nie znalazłem pliku gry w tym miejscu.")
            return
        wz.game, wz.game_name, wz.game_color, wz.use_game = got, got.name, got.color, True
        self.show_wizard()

    def wizard_back(self: App) -> None:
        wz = self.wizard
        if wz.step == 1:
            self.pages.go(self.SETTINGS if self.store else self.MAIN, back=True)
            return
        wz.step -= 1
        self.show_wizard()

    def wizard_next(self: App) -> None:
        wz = self.wizard
        if wz.step == 1:
            wz.name = self.wizard_name.text().strip() or wz.name
        if wz.step < 3:
            wz.step += 1
            self.show_wizard()
            return
        self.finish_wizard()

    def finish_wizard(self: App) -> None:
        wz = self.wizard
        if wz.mode == "first" or self.store is None:
            legacy = self.legacy or {}
            dane.backup_legacy()
            legacy_flat = dict(self.settings)  # już z ustawieniami 0.2 i zmianami z tej sesji
            store = dane.Store.create(wz.name, wz.color, legacy_flat)
            self.store = store
            log.info("Pierwsza osoba: %s%s", wz.name, " (ustawienia z 0.2)" if legacy else "")
        else:
            store = self.store
            store.add_person(wz.name, wz.color, store.person if wz.copy_profiles else None)
            log.info("Nowa osoba: %s", wz.name)
        base = store.profile
        if wz.use_game and wz.game is not None:
            g = wz.game
            profile = store.add_profile(
                wz.game_name or g.name,
                color=wz.game_color,
                icon_color=g.color,
                exe=str(g.exe),
                copy_from=base,
            )
            if g.icon_file:
                final = f"{profile.id}.png"
                shutil.move(str(dane.ICONS_DIR / g.icon_file), str(dane.ICONS_DIR / final))
                store.update_profile(profile.id, icon_file=final, glyph=None)
        store.set("direction", wz.direction)
        self.apply_profile()
        self.pages.go(self.MAIN)

    # --- edycja osoby -------------------------------------------------------------------------

    def open_person_edit(self: App, person_id: str) -> None:
        self.replace_page(self.PROFILE, self.build_person_page(person_id))
        self.pages.go(self.PROFILE)

    def build_person_page(self: App, person_id: str) -> QWidget:
        store = self.store
        assert store is not None
        t = self.t
        person = next(p for p in store.people if p.id == person_id)
        page, lay = self.sticky_page("Edytuj osobę", back=self.SETTINGS)
        c, cl = section("Nazwa")
        top = QHBoxLayout()
        top.setSpacing(12)
        avatar = QLabel()
        avatar.setFixedSize(52, 52)
        avatar.setPixmap(k.avatar_pixmap(t, 52, person.name, person.color))
        top.addWidget(avatar)
        name = QLineEdit(person.name)
        name.setFixedHeight(th.CONTROL_H)
        name.setAccessibleName("Nazwa osoby")
        top.addWidget(name, 1)
        dice = QPushButton(k.G_SHUFFLE)
        dice.setObjectName("icon")
        dice.setFixedSize(th.CONTROL_H, th.CONTROL_H)
        dice.setAccessibleName("Losuj inną nazwę")
        dice.clicked.connect(lambda: name.setText(nazwy.random_name(store.names())))
        top.addWidget(dice)
        cl.addLayout(top)
        lay.addWidget(c)

        note = k.SaveNote(t)

        def save_name() -> None:
            if not name.text().strip() or name.text().strip() == person.name:
                return
            store.rename_person(person.id, name.text())
            avatar.setPixmap(k.avatar_pixmap(t, 52, person.name, person.color))
            note.saved()
            self.render_header()

        typing = QTimer(page)  # zapis chwilę po ostatniej literze
        typing.setSingleShot(True)
        typing.setInterval(500)
        typing.timeout.connect(save_name)
        name.textChanged.connect(lambda _t: typing.start())
        name.editingFinished.connect(save_name)
        dice.clicked.connect(save_name)
        c, kl = section("Kolor osoby")
        pal = k.PalettePicker(t, person.color)

        def on_color(col: str) -> None:
            store.rename_person(person.id, person.name, col)
            avatar.setPixmap(k.avatar_pixmap(t, 52, person.name, col))
            note.saved()
            self.render_header()

        pal.picked.connect(on_color)
        kl.addWidget(pal)
        lay.addWidget(c)
        lay.addWidget(note)
        if len(store.people) > 1:
            lay.addWidget(
                label(
                    "Usunięcie osoby usuwa jej profile gier. Jej wyniki zostają w historii jako"
                    " „Usunięte osoby”.",
                    "small",
                )
            )
            remove = HoldButton("Usuń osobę", t)
            remove.confirmed.connect(lambda: self.remove_person(person.id))
            lay.addWidget(remove)
        lay.addStretch(1)
        return page

    def remove_person(self: App, person_id: str) -> None:
        store = self.store
        if store is None:
            return
        if store.remove_person(person_id):
            self.accounts.forget_person(person_id)  # konto zostaje na serwerze, tu wylogowane
            if self.history is not None:
                self.history.forget_person(person_id)
            self.apply_profile()
            self.open_settings()

    def choose_person(self: App, person_id: str) -> None:
        store = self.store
        if store is None:
            return
        self.close_popups()

        def switch() -> None:
            if person_id != store.person.id:
                store.switch_person(person_id)
                log.info("Osoba: %s", store.person.name)
                self.online.clear()  # inna osoba - inne konto (albo żadne)
                self.apply_profile()
                self.render_online_button()
            if self.pages.currentIndex() == self.SETTINGS:
                self.refresh_settings()  # raz - motyw mógł już przebudować widok

        if self.pages.currentIndex() == self.SETTINGS:
            self.quietly(switch, th.T_PANEL_IN)  # w Ustawieniach: przenikanie, bez przeskoku
        else:
            switch()
