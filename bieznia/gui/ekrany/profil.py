"""Profil gry: nowy (z folderu gry albo pusty) i edycja - nazwa, ikona, kolor."""

from __future__ import annotations

import logging
import shutil
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING

from PySide6.QtCore import QTimer
from PySide6.QtWidgets import QFileDialog, QHBoxLayout, QLineEdit, QVBoxLayout, QWidget

from bieznia import dane, ikony, paleta
from bieznia.gui import komponenty as k
from bieznia.gui import theme as th
from bieznia.gui.ekrany.ustawienia import section
from bieznia.gui.widgets import HoldButton, Segmented, Switch, button, label

if TYPE_CHECKING:
    from bieznia.gui.app import App

log = logging.getLogger("bieznia")


@dataclass
class GamePick:
    exe: Path
    name: str
    icon_file: str | None  # tymczasowy PNG w dane/ikony
    color: str


def pick_game(parent: QWidget, folder: bool = True) -> GamePick | None:
    """Okno wyboru folderu (albo pliku .exe / skrótu) -> gra z ikoną i kolorem."""
    if folder:
        path = QFileDialog.getExistingDirectory(parent, "Wskaż folder gry")
    else:
        path, _ = QFileDialog.getOpenFileName(
            parent, "Wskaż plik gry", "", "Gry (*.exe *.lnk);;Wszystkie pliki (*)"
        )
    if not path:
        return None
    return game_from(Path(path))


def game_from(path: Path) -> GamePick | None:
    found = ikony.resolve(path)
    if found is None:
        return None
    dane.ICONS_DIR.mkdir(parents=True, exist_ok=True)
    tmp = f"nowa-{uuid.uuid4().hex[:8]}.png"
    icon_file: str | None = tmp
    color = paleta.DOMYSLNY
    try:
        if ikony.extract_icon(found.exe, dane.ICONS_DIR / tmp):
            color = ikony.icon_color(dane.ICONS_DIR / tmp).name
        else:
            icon_file = None
    except Exception:
        log.exception("Ikona z %s", found.exe)
        icon_file = None
    log.info("Gra: %s (%s), kolor z ikony %s", found.name, found.exe.name, color)
    return GamePick(found.exe, found.name, icon_file, color)


class ProfileScreen:
    def open_profile_new(self: App) -> None:
        self.replace_page(self.PROFILE, self.build_profile_page(None))
        self.pages.go(self.PROFILE)

    def open_profile_edit(self: App, profile_id: str) -> None:
        self.replace_page(self.PROFILE, self.build_profile_page(profile_id))
        self.pages.go(self.PROFILE)

    def build_profile_page(self: App, profile_id: str | None) -> QWidget:
        store = self.store
        assert store is not None
        t = self.t
        editing = next((p for p in store.person.profiles if p.id == profile_id), None)
        page, lay = self.sticky_page(
            "Edytuj profil" if editing else "Nowy profil", back=self.SETTINGS
        )
        state: dict[str, object] = {
            "game": None,
            "color": editing.color if editing else paleta.DOMYSLNY,
            "icon_color": editing.icon_color if editing else None,
            "glyph": editing.glyph if editing else k.G_PAD,
            "icon_file": editing.icon_file if editing else None,
            "exe": editing.exe if editing else None,
        }

        game_card, gl = section("Gra")
        game_box = QVBoxLayout()
        gl.addLayout(game_box)
        lay.addWidget(game_card)
        name_card, nl = section("Nazwa profilu")
        name = QLineEdit(editing.name if editing else "")
        name.setPlaceholderText("Nazwa gry, np. Cyberpunk 2077")
        name.setFixedHeight(th.CONTROL_H)
        name.setAccessibleName("Nazwa profilu")
        nl.addWidget(name)
        if editing is not None:
            online = Switch(t, "Pokazuj tę grę na koncie online", editing.online)

            def on_online(on: bool) -> None:
                store.update_profile(editing.id, online=on)
                if self.history is not None:
                    self.history.forget_sent()  # sesje tej gry pójdą jeszcze raz (z grą albo bez)
                note.saved()

            online.toggled.connect(on_online)
            self.setting_row(
                nl,
                "Pokazuj tę grę na koncie online",
                "Gdy wyłączone, sesje idą na konto bez nazwy gry",
                online,
            )
        lay.addWidget(name_card)
        color_card, cl = section("Kolor")
        palette = k.PalettePicker(t, str(state["color"]), state["icon_color"])  # type: ignore[arg-type]
        cl.addWidget(palette)
        lay.addWidget(color_card)
        icon_card, il = section("Ikona")
        grid = k.IconGrid(
            t, None if state["icon_file"] else str(state["glyph"]), str(state["color"])
        )
        il.addWidget(grid)
        lay.addWidget(icon_card)

        def icon_path() -> Path | None:
            f = state["icon_file"]
            return dane.ICONS_DIR / str(f) if f else None

        def render_game() -> None:
            while (item := game_box.takeAt(0)) is not None:
                if (w := item.widget()) is not None:
                    w.hide()
                    w.deleteLater()
            exe = state["exe"]
            if exe:
                pix = k.profile_pixmap(t, 44, str(state["color"]), None, icon_path())
                texts = QVBoxLayout()
                texts.setSpacing(0)
                n = label(Path(str(exe)).name)
                n.setObjectName("strong")
                texts.addWidget(n)
                texts.addWidget(label(f"…\\{Path(str(exe)).parent.name}", "small"))
                holder = QWidget()
                hl = QHBoxLayout(holder)
                hl.setContentsMargins(0, 0, 0, 0)
                hl.addWidget(k.picture(pix, 44))
                hl.addLayout(texts, 1)
                change = button("Zmień")
                change.setFixedWidth(84)
                change.clicked.connect(lambda: choose(True))
                hl.addWidget(change)
                game_box.addWidget(holder)
                game_box.addWidget(
                    label("Ikona i kolor wzięte z pliku gry - możesz je zmienić niżej.", "small")
                )
            else:
                game_box.addWidget(
                    label(
                        "Wskaż folder gry - aplikacja weźmie z pliku gry nazwę, ikonę i kolor.",
                        "small",
                    )
                )
                pick = button("Wskaż folder gry", "primary")
                pick.clicked.connect(lambda: choose(True))
                exe_btn = button("Wybierz plik .exe albo skrót")
                exe_btn.clicked.connect(lambda: choose(False))
                game_box.addWidget(pick)
                game_box.addWidget(exe_btn)

        def choose(folder: bool) -> None:
            got = pick_game(page, folder)
            if got is None:
                self.toast("Nie znalazłem pliku gry w tym miejscu.")
                return
            state.update(
                game=got,
                exe=str(got.exe),
                icon_file=got.icon_file,
                icon_color=got.color,
                color=got.color,
                glyph=None,
            )
            if not name.text().strip() or not editing:
                name.setText(got.name)
            palette.from_icon = got.color
            palette._pick(got.color)
            grid.selected = None
            grid.set_color(got.color)
            render_game()

        def on_color(c: str) -> None:
            state["color"] = c
            grid.set_color(c)
            render_game()
            if editing:
                store.update_profile(editing.id, color=c)
                note.saved()
                if editing.id == store.profile.id:
                    self.settings["profile_color"] = c
                    self.apply_look()
                    self.render_header()
                    self.render_zones()

        def on_glyph(code: str) -> None:
            state["glyph"] = code
            state["icon_file"] = None
            render_game()
            if editing:
                store.update_profile(editing.id, glyph=code, icon_file=None)
                note.saved()
                self.render_header()

        note = k.SaveNote(t)
        palette.picked.connect(on_color)
        grid.picked.connect(on_glyph)
        render_game()

        if editing:

            def save_name() -> None:
                if name.text().strip() and name.text().strip() != editing.name:
                    store.update_profile(editing.id, name=name.text())
                    note.saved()
                    self.render_header()

            typing = QTimer(page)  # zapis chwilę po ostatniej literze
            typing.setSingleShot(True)
            typing.setInterval(500)
            typing.timeout.connect(save_name)
            name.textChanged.connect(lambda _t: typing.start())
            name.editingFinished.connect(save_name)
            lay.addWidget(note)
            row = QHBoxLayout()
            row.setSpacing(8)
            dup = button("Duplikuj")
            dup.clicked.connect(lambda: self.duplicate_profile(editing.id))
            row.addWidget(dup, 1)
            if len(store.person.profiles) > 1:
                remove = HoldButton("Usuń profil", t)
                remove.confirmed.connect(lambda: self.remove_profile(editing.id))
                row.addWidget(remove, 1)
            lay.addLayout(row)
        else:
            start_card, sl = section("Zacznij od")
            start = Segmented(
                [("copy", "Kopia obecnego"), ("default", "Domyślne")], "copy", "Zacznij od"
            )
            sl.addWidget(start)
            sl.addWidget(
                label(
                    f"Kopia obecnego bierze tryb, klawisze, skoki pada i prędkości z profilu"
                    f" {store.profile.name}.",
                    "small",
                )
            )
            lay.addWidget(start_card)
            create = button("Utwórz i włącz profil", "primary")

            def do_create() -> None:
                icon_file = state["icon_file"]
                profile = store.add_profile(
                    name.text() or "Profil",
                    color=str(state["color"]),
                    icon_color=state["icon_color"],  # type: ignore[arg-type]
                    glyph=state["glyph"],  # type: ignore[arg-type]
                    exe=state["exe"],  # type: ignore[arg-type]
                    copy_from=store.profile if start.buttons["copy"].isChecked() else None,
                )
                if icon_file:
                    final = f"{profile.id}.png"
                    shutil.move(str(dane.ICONS_DIR / str(icon_file)), str(dane.ICONS_DIR / final))
                    store.update_profile(profile.id, icon_file=final, glyph=None)
                log.info("Nowy profil: %s", profile.name)
                self.apply_profile()
                self.open_settings()

            create.clicked.connect(do_create)
            lay.addWidget(create)
        lay.addStretch(1)
        return page

    def duplicate_profile(self: App, profile_id: str) -> None:
        if self.store is not None and self.store.duplicate_profile(profile_id):
            self.open_settings()

    def remove_profile(self: App, profile_id: str) -> None:
        if self.store is not None and self.store.remove_profile(profile_id):
            self.apply_profile()
            self.open_settings()

    def choose_profile(self: App, profile_id: str) -> None:
        if self.store is None:
            return
        self.close_popups()
        store = self.store

        def switch() -> None:
            if profile_id != store.profile.id:
                store.switch_profile(profile_id)
                log.info("Profil: %s", store.profile.name)
                self.apply_profile()
            if self.pages.currentIndex() == self.SETTINGS:
                self.refresh_settings()  # raz - motyw mógł już przebudować widok

        if self.pages.currentIndex() == self.SETTINGS:
            self.quietly(switch, th.T_PANEL_IN)  # w Ustawieniach: przenikanie, bez przeskoku
        else:
            switch()
