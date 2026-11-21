"""Kopia danych: eksport (osoby, profile, ustawienia, wyniki, logi) i import z podglądem."""

from __future__ import annotations

import logging
from datetime import date, datetime, timedelta
from pathlib import Path
from typing import TYPE_CHECKING

from PySide6.QtWidgets import QFileDialog, QHBoxLayout, QVBoxLayout, QWidget

from bieznia import kopia
from bieznia.gui import komponenty as k
from bieznia.gui.ekrany.ustawienia import section
from bieznia.gui.widgets import Segmented, Switch, button, label

if TYPE_CHECKING:
    from bieznia.gui.app import App

log = logging.getLogger("bieznia")


class CopyScreen:
    # --- eksport -------------------------------------------------------------------------------

    def open_export(self: App) -> None:
        store = self.store
        if store is None:
            return
        self.export_pick = {p.id: {pr.id for pr in p.profiles} for p in store.people}
        self.export_flags = {"settings": True, "results": True, "logs": False, "range": "all"}
        self.replace_page(self.EXPORT, self.build_export_page())
        self.pages.go(self.EXPORT)

    def build_export_page(self: App) -> QWidget:
        store = self.store
        assert store is not None
        t = self.t
        page, lay = self.sticky_page("Eksport danych", back=self.SETTINGS)
        c, cl = section("Osoby i profile gier")
        for person in store.people:
            chosen = self.export_pick.get(person.id, set())
            state = "on" if len(chosen) == len(person.profiles) else ("some" if chosen else "off")
            prow = k.CheckRow(
                t,
                person.name,
                f"{len(chosen)} z {len(person.profiles)} profili gier",
                state,
                k.avatar_pixmap(t, 28, person.name, person.color),
            )
            prow.toggled_state.connect(lambda on, p=person: self.export_toggle_person(p.id, on))
            cl.addWidget(prow)
            for pr in person.profiles:
                r = k.CheckRow(
                    t,
                    pr.name,
                    "",
                    "on" if pr.id in chosen else "off",
                    self.profile_pixmap(pr, 28),
                    indent=34,
                )
                r.toggled_state.connect(
                    lambda on, p=person, q=pr: self.export_toggle_profile(p.id, q.id, on)
                )
                cl.addWidget(r)
        lay.addWidget(c)
        c, wl = section("Co zapisać")
        for key, title, sub in (
            (
                "settings",
                "Ustawienia i profile gier",
                "tryby, klawisze, skoki pada, prędkości, ikony, kolory",
            ),
            ("results", "Wyniki", "historia kroków: sesje, kroki, dystans, czas"),
            ("logs", "Logi", "dziennik aplikacji i Bluetooth - do zgłoszenia problemu"),
        ):
            r = k.CheckRow(t, title, sub, "on" if self.export_flags[key] else "off")
            r.toggled_state.connect(lambda on, kk=key: self.export_flag(kk, on))
            wl.addWidget(r)
        lay.addWidget(c)
        c, rl = section("Zakres wyników")
        rng = Segmented(
            [("all", "Wszystko"), ("year", "Rok"), ("month", "30 dni")],
            str(self.export_flags["range"]),
            "Zakres wyników",
        )
        rng.changed.connect(lambda v: self.export_flag("range", v))
        rl.addWidget(rng)
        lay.addWidget(c)
        choice = self.export_choice()
        summary = kopia.summarize(store, self.history, choice)
        lay.addWidget(
            label(
                f"Plik: {summary.people} os., {summary.profiles} profili gier,"
                f" {summary.sessions} sesji",
                "small",
            )
        )
        save = button("Zapisz plik kopii…", "primary")
        save.clicked.connect(self.do_export)
        lay.addWidget(save)
        lay.addWidget(
            label(
                "Plik .bieznia to archiwum ZIP z danymi w JSON - można je otworzyć i sprawdzić.",
                "small",
            )
        )
        lay.addStretch(1)
        return page

    def export_choice(self: App) -> kopia.ExportChoice:
        rng = self.export_flags["range"]
        since = 0.0
        if rng in ("year", "month"):
            days = 365 if rng == "year" else 30
            since = datetime.combine(
                date.today() - timedelta(days=days), datetime.min.time()
            ).timestamp()
        return kopia.ExportChoice(
            people={pid: set(ids) for pid, ids in self.export_pick.items() if ids},
            settings=bool(self.export_flags["settings"]),
            results=bool(self.export_flags["results"]),
            logs=bool(self.export_flags["logs"]),
            since=since,
        )

    def export_toggle_person(self: App, person_id: str, on: bool) -> None:
        store = self.store
        assert store is not None
        person = next(p for p in store.people if p.id == person_id)
        self.export_pick[person_id] = {pr.id for pr in person.profiles} if on else set()
        self.replace_page(self.EXPORT, self.build_export_page())

    def export_toggle_profile(self: App, person_id: str, profile_id: str, on: bool) -> None:
        ids = self.export_pick.setdefault(person_id, set())
        (ids.add if on else ids.discard)(profile_id)
        self.replace_page(self.EXPORT, self.build_export_page())

    def export_flag(self: App, key: str, value: object) -> None:
        self.export_flags[key] = value
        self.replace_page(self.EXPORT, self.build_export_page())

    def do_export(self: App) -> None:
        store = self.store
        assert store is not None
        path, _ = QFileDialog.getSaveFileName(
            self.pages,
            "Zapisz kopię danych",
            str(Path.home() / kopia.default_name()),
            "Kopia Bieżni (*.bieznia)",
        )
        if not path:
            return
        try:
            s = kopia.export(store, self.history, self.export_choice(), Path(path))
        except kopia.CopyError as exc:
            self.toast(str(exc))
            return
        self.toast(f"Zapisano: {s.people} os., {s.profiles} profili, {s.sessions} sesji.")
        self.pages.go(self.SETTINGS, back=True)

    # --- import --------------------------------------------------------------------------------

    def open_import(self: App) -> None:
        path, _ = QFileDialog.getOpenFileName(
            self.pages, "Wybierz kopię danych", str(Path.home()), "Kopia Bieżni (*.bieznia)"
        )
        if not path:
            return
        try:
            self.import_preview = kopia.read(Path(path))
        except kopia.CopyError as exc:
            self.toast(str(exc))
            return
        prev = self.import_preview
        self.import_pick = {p.id for p in prev.people}
        self.import_new: set[str] = set()
        self.import_flags = {
            "settings": prev.has_settings,
            "results": prev.has_results,
            "keep": False,
        }
        self.replace_page(self.IMPORT, self.build_import_page())
        self.pages.go(self.IMPORT)

    def build_import_page(self: App) -> QWidget:
        store = self.store
        assert store is not None
        prev = self.import_preview
        t = self.t
        page, lay = self.sticky_page("Import danych", back=self.SETTINGS)
        c, fl = section("Plik")
        row = QHBoxLayout()
        row.setSpacing(10)
        row.addWidget(k.glyph(k.G_FILE, t.text, 20))
        texts = QVBoxLayout()
        texts.setSpacing(0)
        n = label(prev.path.name)
        n.setObjectName("strong")
        texts.addWidget(n)
        texts.addWidget(label(kopia.describe(prev), "small"))
        row.addLayout(texts, 1)
        fl.addLayout(row)
        lay.addWidget(c)
        c, cl = section("Co zaimportować")
        mine = {p.id for p in store.people}
        for person in prev.people:
            exists = person.id in mine
            r = k.CheckRow(
                t,
                person.name,
                ("już jest na tym komputerze" if exists else "nowa osoba")
                + f" · {len(person.profiles)} prof. · {len(prev.sessions_of(person.id))} sesji",
                "on" if person.id in self.import_pick else "off",
                k.avatar_pixmap(t, 28, person.name, person.color),
            )
            r.toggled_state.connect(lambda on, pid=person.id: self.import_toggle(pid, on))
            cl.addWidget(r)
        if prev.has_settings:
            r = k.CheckRow(
                t,
                "Ustawienia i profile gier",
                "tryby, klawisze, prędkości, ikony, kolory",
                "on" if self.import_flags["settings"] else "off",
            )
            r.toggled_state.connect(lambda on: self.import_flag("settings", on))
            cl.addWidget(r)
        if prev.has_results:
            r = k.CheckRow(
                t, "Wyniki", "historia kroków", "on" if self.import_flags["results"] else "off"
            )
            r.toggled_state.connect(lambda on: self.import_flag("results", on))
            cl.addWidget(r)
        lay.addWidget(c)
        for person in prev.people:
            if person.id in mine and person.id in self.import_pick:
                c, kl = section(f"{person.name} już jest")
                seg = Segmented(
                    [("merge", "Połącz"), ("new", "Dodaj jako nową")],
                    "new" if person.id in self.import_new else "merge",
                    "Konflikt osoby",
                )
                seg.changed.connect(lambda v, pid=person.id: self.import_conflict(pid, v))
                kl.addWidget(seg)
                kl.addWidget(
                    label(
                        "Połącz: wyniki się dodają (bez dublowania tych samych sesji),"
                        " profile o tej"
                        " samej nazwie dostają ustawienia z pliku, nowe profile dochodzą.",
                        "small",
                    )
                )
                sw = Switch(t, "Zachowaj moje ustawienia profili", bool(self.import_flags["keep"]))
                sw.toggled.connect(lambda v: self.import_flags.__setitem__("keep", v))
                self.setting_row(
                    kl,
                    "Zachowaj moje ustawienia profili",
                    "Z pliku weź tylko nowe profile i wyniki",
                    sw,
                )
                lay.addWidget(c)
        go = button("Importuj", "primary")
        go.clicked.connect(self.do_import)
        go.setEnabled(bool(self.import_pick))
        lay.addWidget(go)
        lay.addWidget(
            label(
                "Przed importem aplikacja robi kopię obecnych danych - można ją"
                " przywrócić w Ustawieniach.",
                "small",
            )
        )
        lay.addStretch(1)
        return page

    def import_toggle(self: App, person_id: str, on: bool) -> None:
        (self.import_pick.add if on else self.import_pick.discard)(person_id)
        self.replace_page(self.IMPORT, self.build_import_page())

    def import_flag(self: App, key: str, on: bool) -> None:
        self.import_flags[key] = on
        self.replace_page(self.IMPORT, self.build_import_page())

    def import_conflict(self: App, person_id: str, mode: str) -> None:
        (self.import_new.add if mode == "new" else self.import_new.discard)(person_id)

    def do_import(self: App) -> None:
        store, history = self.store, self.history
        assert store is not None and history is not None
        choice = kopia.ImportChoice(
            people=set(self.import_pick),
            settings=bool(self.import_flags["settings"]),
            results=bool(self.import_flags["results"]),
            new_person=set(self.import_new),
            keep_my_settings=bool(self.import_flags["keep"]),
        )
        try:
            r = kopia.apply_import(store, history, self.import_preview, choice, self.history_path)
        except (OSError, ValueError) as exc:
            log.exception("Import")
            self.toast(f"Import nie powiódł się: {exc}")
            return
        self.apply_profile()
        self.toast(
            f"Zaimportowano: nowe osoby {r.people_added}, połączone {r.people_merged},"
            f" profile +{r.profiles_added}, sesje +{r.sessions_added}."
        )
        self.open_settings()

    def restore_backup(self: App, folder: Path) -> None:
        if self.store is None:
            return
        self.close_history()
        kopia.restore(folder, self.store.path, self.history_path)
        self.reload_store()
        self.toast("Przywrócono dane sprzed importu.")
        self.open_settings()
