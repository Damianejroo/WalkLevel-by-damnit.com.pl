"""Obsługiwane bieżnie: lista modeli (profile), pobieranie nowych i dodawanie z pliku.

Profil to dane (zakres prędkości, odliczanie, nazwa) - nie program. Nowe modele tego
samego rodzaju (KingSmith, FTMS) działają od razu po pobraniu profilu; zupełnie nowy
protokół przychodzi zwykłą aktualizacją aplikacji.
"""

from __future__ import annotations

import logging
from pathlib import Path
from typing import TYPE_CHECKING

from PySide6.QtWidgets import QFileDialog, QHBoxLayout, QLabel, QPushButton, QVBoxLayout, QWidget

from bieznia.gui.ekrany.ustawienia import section
from bieznia.gui.widgets import button, fmt_num, label
from bieznia.sterowniki import known_capabilities, pobieranie, profile_for
from bieznia.sterowniki import profile as profiles
from bieznia.sterowniki.kontrakt import Capabilities

if TYPE_CHECKING:
    from bieznia.gui.app import App

log = logging.getLogger("bieznia")

DRIVER_NAMES = {"kingsmith": "KingSmith", "ftms": "Bluetooth FTMS"}
SOURCE_NAMES = {"wbudowany": "w aplikacji", "dodatkowy": "dodany", "pobrany": "pobrany"}


class ModelsScreen:
    models_status: QLabel

    def open_models(self: App) -> None:
        self.replace_page(self.MODELS, self.build_models_page())
        self.pages.go(self.MODELS)

    def build_models_page(self: App) -> QWidget:
        page, lay = self.sticky_page("Obsługiwane bieżnie", back=self.SETTINGS)
        intro = label(
            "Bieżnię rozpoznajemy po nazwie Bluetooth. Profil modelu to dane: zakres prędkości,"
            " odliczanie przy starcie, nazwa - nie program. Nowy model tego samego rodzaju"
            " działa od razu po pobraniu profilu.",
            "small",
        )
        intro.setWordWrap(True)
        lay.addWidget(intro)

        mine = profile_for(self.settings["device_name"]) if self.settings["device_name"] else None
        by_driver: dict[str, list[profiles.ModelProfile]] = {}
        for p in profiles.all_profiles():
            by_driver.setdefault(p.driver, []).append(p)
        for driver, items in sorted(by_driver.items()):
            c, cl = section(DRIVER_NAMES.get(driver, driver))
            for p in sorted(items, key=lambda q: (is_generic(q), q.name)):
                cl.addWidget(self.model_row(p, mine is not None and mine.id == p.id))
            lay.addWidget(c)

        self.models_status = label("", "small")
        self.models_status.setWordWrap(True)
        lay.addWidget(self.models_status)
        row = QHBoxLayout()
        from_file = button("Dodaj z pliku…")
        from_file.setToolTip("Profil .json od autora albo innego użytkownika")
        from_file.clicked.connect(self.add_model_file)
        fetch = button("Sprawdź nowe modele", "primary")
        fetch.clicked.connect(self.fetch_models)
        row.addWidget(from_file)
        row.addWidget(fetch, 1)
        lay.addLayout(row)
        lay.addStretch(1)
        return page

    def model_row(self: App, p: profiles.ModelProfile, mine: bool) -> QWidget:
        caps = p.apply(known_capabilities_for(p))
        details = [f"{fmt_num(caps.speed_min)}-{fmt_num(caps.speed_max)} km/h"]
        if is_generic(p):
            details.insert(0, "pozostałe modele marki")
        if caps.start_delay_s >= 1:
            details.append(f"rozruch ok. {caps.start_delay_s:.0f} s")
        if p.match:
            details.append("nazwy " + ", ".join(m + "…" for m in p.match[:3]))
        details.append(SOURCE_NAMES.get(p.source, p.source))
        row = QWidget()
        rl = QHBoxLayout(row)
        rl.setContentsMargins(0, 2, 0, 2)
        rl.setSpacing(8)
        texts = QVBoxLayout()
        texts.setSpacing(0)
        title = label(f"{p.name}{' · Twoja bieżnia' if mine else ''}")
        title.setObjectName("accent" if mine else "strong")
        texts.addWidget(title)
        sub = label(" · ".join(details), "small")
        sub.setWordWrap(True)
        texts.addWidget(sub)
        rl.addLayout(texts, 1)
        if p.source != "wbudowany":
            remove: QPushButton = button("Usuń")
            remove.setAccessibleName(f"Usuń profil {p.name}")
            remove.clicked.connect(lambda _=False, pid=p.id: self.remove_model(pid))
            rl.addWidget(remove)
        return row

    def fetch_models(self: App) -> None:
        self.models_status.setText("Sprawdzam listę modeli na GitHubie…")
        self.run_task(self.fetch_models_task)

    def fetch_models_task(self: App) -> None:
        """W wątku: pobranie profili; wynik idzie do kolejki zadań okna."""
        try:
            result = pobieranie.download_profiles()
        except pobieranie.ProfileDownloadError as exc:
            self.tasks.put(("models", str(exc)))
            return
        except Exception:
            log.exception("Pobieranie profili bieżni")
            self.tasks.put(("models", "Nie udało się pobrać listy modeli."))
            return
        parts = []
        if result.added:
            parts.append("Nowe modele: " + ", ".join(result.added) + ".")
        if result.updated:
            parts.append("Zaktualizowane: " + ", ".join(result.updated) + ".")
        if result.rejected:
            parts.append(f"Odrzucone (nie przeszły sprawdzenia): {len(result.rejected)}.")
        if not parts:
            parts.append("Masz wszystkie znane modele.")
        self.tasks.put(("models", " ".join(parts)))

    def on_models_result(self: App, text: str) -> None:
        self.models_refresh(text)

    def add_model_file(self: App) -> None:
        path, _ = QFileDialog.getOpenFileName(
            self.window, "Dodaj profil bieżni", str(Path.home()), "Profil bieżni (*.json)"
        )
        if not path:
            return
        try:
            found = pobieranie.install_from_file(Path(path))
        except pobieranie.ProfileDownloadError as exc:
            self.models_status.setText(str(exc))
            return
        self.models_refresh(f"Dodano: {found.name}.")

    def remove_model(self: App, profile_id: str) -> None:
        pobieranie.remove_user_profile(profile_id)
        self.models_refresh("Profil usunięty.")

    def models_refresh(self: App, text: str) -> None:
        """Lista od nowa (profile się zmieniły) i możliwości zapamiętanej bieżni też."""
        self.caps = known_capabilities(self.settings["device_name"], self.settings["simulator"])
        if self.pages.currentIndex() == self.MODELS:
            self.replace_page(self.MODELS, self.build_models_page())
        self.models_status.setText(text)
        self.render_device_label()


def is_generic(p: profiles.ModelProfile) -> bool:
    """Profil całej marki (krótkie początki nazw, np. „URTM”) - po konkretnych modelach."""
    return not p.match or max(len(m) for m in p.match) < 6


def known_capabilities_for(p: profiles.ModelProfile) -> Capabilities:
    """Domyślne możliwości modułu, na które nakładamy profil."""
    from bieznia.sterowniki import FTMS_CAPS, KINGSMITH_CAPS

    return FTMS_CAPS if p.driver == "ftms" else KINGSMITH_CAPS
