"""Osoby i profile gier (od 0.3.0).

Jeden plik dane/osoby.json (zapis atomowy):
- wspólne: bieżnia, klawisz start/stop, logi - jedno dla wszystkich osób,
- osoby: nazwa, kolor, wygląd (motyw, duży tekst, przycisk na ekranie...) i profile gier,
- profil gry: nazwa, ikona (z gry albo z zestawu), kolor, ustawienia sterowania.

Aplikacja widzi to jako jeden płaski słownik ustawień (flat), złożony z części wspólnej,
wyglądu aktywnej osoby i ustawień jej aktywnego profilu. Zmiana klucza trafia tam,
gdzie ten klucz należy.
"""

from __future__ import annotations

import contextlib
import copy
import json
import logging
import os
import shutil
import time
import uuid
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from bieznia import paleta

log = logging.getLogger("bieznia")

DATA_DIR = Path("dane")
STORE_FILE = DATA_DIR / "osoby.json"
ICONS_DIR = DATA_DIR / "ikony"
BACKUP_DIR = DATA_DIR / "kopie"
OLD_SETTINGS = DATA_DIR / "ustawienia.json"
FORMAT_VERSION = 1

# Gdzie należy każdy klucz płaskich ustawień.
SHARED_KEYS = frozenset(
    {
        "device_address",
        "device_name",
        "belt_control",
        "quick_stop",
        "toggle_key",
        "toggle_pad",
        "simulator",
        "record",
        "detailed_logs",
        "last_version",
        "window_geometry",
    }
)
LOOK_KEYS = frozenset(
    {
        "theme",
        "large_text",
        "always_on_top",
        "floating_button",
        "floating_pos",
        "show_person_switch",
        "show_profile_switch",
    }
)
PROFILE_KEYS = frozenset(
    {
        "direction",
        "output",
        "walk_key",
        "run_key",
        "stand_below",
        "run_on",
        "run_off",
        "idle_speed",
        "walk_speed",
        "run_speed",
        "pad_steps",
        "pad_idle",
        "training_levels",
        "training_level",
    }
)
LOOK_DEFAULTS: dict[str, Any] = {"show_person_switch": True, "show_profile_switch": True}


def new_id() -> str:
    return uuid.uuid4().hex[:10]


@dataclass
class Profile:
    id: str
    name: str
    color: str = paleta.DOMYSLNY  # nazwa z palety
    icon_color: str | None = None  # kolor dobrany z ikony gry (podpowiedź przy zmianie)
    glyph: str | None = None  # ikona z zestawu (znak czcionki Segoe Fluent)
    icon_file: str | None = None  # ikona z gry: plik PNG w dane/ikony
    exe: str | None = None
    settings: dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "nazwa": self.name,
            "kolor": self.color,
            "kolor_z_ikony": self.icon_color,
            "glif": self.glyph,
            "ikona": self.icon_file,
            "exe": self.exe,
            "ustawienia": self.settings,
        }

    @classmethod
    def from_dict(cls, d: dict[str, Any]) -> Profile:
        return cls(
            id=str(d.get("id") or new_id()),
            name=str(d.get("nazwa") or "Profil"),
            color=paleta.kolor(d.get("kolor")).name,
            icon_color=d.get("kolor_z_ikony"),
            glyph=d.get("glif"),
            icon_file=d.get("ikona"),
            exe=d.get("exe"),
            settings={
                k: v for k, v in dict(d.get("ustawienia") or {}).items() if k in PROFILE_KEYS
            },
        )


@dataclass
class Person:
    id: str
    name: str
    color: str = paleta.DOMYSLNY
    look: dict[str, Any] = field(default_factory=dict)
    profiles: list[Profile] = field(default_factory=list)
    active_profile: str = ""

    @property
    def profile(self) -> Profile:
        for p in self.profiles:
            if p.id == self.active_profile:
                return p
        return self.profiles[0]

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "nazwa": self.name,
            "kolor": self.color,
            "wyglad": self.look,
            "aktywny_profil": self.active_profile,
            "profile": [p.to_dict() for p in self.profiles],
        }

    @classmethod
    def from_dict(cls, d: dict[str, Any]) -> Person:
        profiles = [Profile.from_dict(p) for p in d.get("profile") or [] if isinstance(p, dict)]
        if not profiles:
            profiles = [Profile(new_id(), "Domyślny", glyph=DEFAULT_GLYPH)]
        person = cls(
            id=str(d.get("id") or new_id()),
            name=str(d.get("nazwa") or "Gracz"),
            color=paleta.kolor(d.get("kolor")).name,
            look={k: v for k, v in dict(d.get("wyglad") or {}).items() if k in LOOK_KEYS},
            profiles=profiles,
            active_profile=str(d.get("aktywny_profil") or profiles[0].id),
        )
        if all(p.id != person.active_profile for p in profiles):
            person.active_profile = profiles[0].id
        return person


DEFAULT_GLYPH = ""  # postać idąca - profil Domyślny


class Store:
    """Osoby, profile i ustawienia wspólne. Wszystkie zmiany od razu na dysk."""

    def __init__(self, path: Path = STORE_FILE) -> None:
        self.path = path
        self.shared: dict[str, Any] = {}
        self.people: list[Person] = []
        self.active_person = ""
        self.ask_at_start = True

    # --- odczyt / zapis --------------------------------------------------------------

    @classmethod
    def load(cls, path: Path = STORE_FILE) -> Store | None:
        """None, gdy pliku nie ma (pierwsze uruchomienie 0.3.0 - konfigurator)."""
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except FileNotFoundError:
            return None
        except (OSError, ValueError) as exc:
            log.warning(
                "Nie da się odczytać %s (%s) - zapisuję kopię i zaczynam od nowa", path, exc
            )
            with contextlib.suppress(OSError):
                BACKUP_DIR.mkdir(parents=True, exist_ok=True)
                shutil.copy(path, BACKUP_DIR / f"uszkodzony-{int(time.time())}.json")
            return None
        store = cls(path)
        store.shared = {
            k: v for k, v in dict(data.get("wspolne") or {}).items() if k in SHARED_KEYS
        }
        store.people = [Person.from_dict(p) for p in data.get("osoby") or [] if isinstance(p, dict)]
        if not store.people:
            return None
        store.active_person = str(data.get("aktywna") or store.people[0].id)
        if all(p.id != store.active_person for p in store.people):
            store.active_person = store.people[0].id
        store.ask_at_start = bool(data.get("pytaj_przy_starcie", True))
        return store

    def to_dict(self) -> dict[str, Any]:
        return {
            "wersja": FORMAT_VERSION,
            "aktywna": self.active_person,
            "pytaj_przy_starcie": self.ask_at_start,
            "wspolne": self.shared,
            "osoby": [p.to_dict() for p in self.people],
        }

    def save(self) -> None:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        tmp = self.path.with_suffix(".tmp")
        tmp.write_text(json.dumps(self.to_dict(), indent=1, ensure_ascii=False), encoding="utf-8")
        os.replace(tmp, self.path)  # atomowo

    # --- tworzenie ------------------------------------------------------------------------

    @classmethod
    def create(
        cls,
        person_name: str,
        person_color: str,
        legacy: dict[str, Any] | None = None,
        path: Path = STORE_FILE,
    ) -> Store:
        """Pierwsza osoba. legacy = płaskie ustawienia z 0.2 (trafiają do profilu Domyślnego)."""
        legacy = legacy or {}
        store = cls(path)
        store.shared = {k: v for k, v in legacy.items() if k in SHARED_KEYS}
        person = Person(
            new_id(),
            person_name,
            paleta.kolor(person_color).name,
            look={k: v for k, v in legacy.items() if k in LOOK_KEYS},
        )
        default = Profile(
            new_id(),
            "Domyślny",
            glyph=DEFAULT_GLYPH,
            settings={k: v for k, v in legacy.items() if k in PROFILE_KEYS},
        )
        person.profiles = [default]
        person.active_profile = default.id
        store.people = [person]
        store.active_person = person.id
        store.save()
        return store

    # --- aktywna osoba i profil ----------------------------------------------------------

    @property
    def person(self) -> Person:
        for p in self.people:
            if p.id == self.active_person:
                return p
        return self.people[0]

    @property
    def profile(self) -> Profile:
        return self.person.profile

    def flat(self, defaults: dict[str, Any]) -> dict[str, Any]:
        """Płaskie ustawienia dla aplikacji: domyślne < wspólne < wygląd osoby < profil."""
        out = dict(defaults)
        out.update(LOOK_DEFAULTS)
        out.update(self.shared)
        out.update(self.person.look)
        out.update(copy.deepcopy(self.profile.settings))
        out["profile_color"] = self.profile.color
        out["person_color"] = self.person.color
        return out

    def set(self, key: str, value: Any) -> None:
        if key in SHARED_KEYS:
            self.shared[key] = value
        elif key in LOOK_KEYS:
            self.person.look[key] = value
        elif key in PROFILE_KEYS:
            self.profile.settings[key] = copy.deepcopy(value)
        else:
            return  # klucz tylko na czas działania (np. ścieżki) - nie zapisujemy
        self.save()

    def update_flat(self, flat: dict[str, Any]) -> None:
        """Zapis płaskich ustawień aplikacji - każdy klucz tam, gdzie należy (jeden zapis)."""
        for key, value in flat.items():
            if key in SHARED_KEYS:
                self.shared[key] = copy.deepcopy(value)
            elif key in LOOK_KEYS:
                self.person.look[key] = copy.deepcopy(value)
            elif key in PROFILE_KEYS:
                self.profile.settings[key] = copy.deepcopy(value)
        self.save()

    def switch_person(self, person_id: str) -> Person:
        if any(p.id == person_id for p in self.people):
            self.active_person = person_id
            self.save()
        return self.person

    def switch_profile(self, profile_id: str) -> Profile:
        if any(p.id == profile_id for p in self.person.profiles):
            self.person.active_profile = profile_id
            self.save()
        return self.profile

    # --- osoby -----------------------------------------------------------------------------

    def names(self) -> list[str]:
        return [p.name for p in self.people]

    def add_person(self, name: str, color: str, copy_profiles_from: Person | None = None) -> Person:
        person = Person(new_id(), name.strip() or "Gracz", paleta.kolor(color).name)
        if copy_profiles_from is not None:
            person.look = copy.deepcopy(copy_profiles_from.look)
            person.profiles = [self._copy_profile(p) for p in copy_profiles_from.profiles]
        else:
            person.profiles = [Profile(new_id(), "Domyślny", glyph=DEFAULT_GLYPH)]
        person.active_profile = person.profiles[0].id
        self.people.append(person)
        self.active_person = person.id
        self.save()
        return person

    def rename_person(self, person_id: str, name: str, color: str | None = None) -> None:
        for p in self.people:
            if p.id == person_id:
                p.name = name.strip() or p.name
                if color:
                    p.color = paleta.kolor(color).name
        self.save()

    def remove_person(self, person_id: str) -> bool:
        """Ostatniej osoby nie da się usunąć."""
        if len(self.people) <= 1:
            return False
        self.people = [p for p in self.people if p.id != person_id]
        if self.active_person == person_id:
            self.active_person = self.people[0].id
        self.save()
        return True

    # --- profile gier --------------------------------------------------------------------

    def _copy_profile(self, src: Profile, name: str | None = None) -> Profile:
        dup = copy.deepcopy(src)
        dup.id = new_id()
        if name:
            dup.name = name
        if src.icon_file:
            dup.icon_file = self._copy_icon(src.icon_file, dup.id)
        return dup

    def _copy_icon(self, icon_file: str, new_profile_id: str) -> str | None:
        src = ICONS_DIR / icon_file
        dst_name = f"{new_profile_id}.png"
        with contextlib.suppress(OSError):
            shutil.copy(src, ICONS_DIR / dst_name)
            return dst_name
        return None

    def add_profile(
        self,
        name: str,
        *,
        color: str | None = None,
        icon_color: str | None = None,
        glyph: str | None = None,
        icon_file: str | None = None,
        exe: str | None = None,
        copy_from: Profile | None = None,
        activate: bool = True,
    ) -> Profile:
        settings = copy.deepcopy(copy_from.settings) if copy_from else {}
        profile = Profile(
            new_id(),
            name.strip() or "Profil",
            paleta.kolor(color or icon_color).name,
            icon_color,
            glyph if not icon_file else None,
            icon_file,
            exe,
            settings,
        )
        self.person.profiles.append(profile)
        if activate:
            self.person.active_profile = profile.id
        self.save()
        return profile

    def update_profile(self, profile_id: str, **changes: Any) -> None:
        for p in self.person.profiles:
            if p.id == profile_id:
                for key, value in changes.items():
                    if key == "color":
                        value = paleta.kolor(value).name
                    if key == "name":
                        value = str(value).strip() or p.name
                    setattr(p, key, value)
        self.save()

    def duplicate_profile(self, profile_id: str) -> Profile | None:
        for p in self.person.profiles:
            if p.id == profile_id:
                dup = self._copy_profile(p, f"{p.name} (kopia)")
                self.person.profiles.append(dup)
                self.save()
                return dup
        return None

    def remove_profile(self, profile_id: str) -> bool:
        """Ostatniego profilu osoby nie da się usunąć."""
        person = self.person
        if len(person.profiles) <= 1:
            return False
        person.profiles = [p for p in person.profiles if p.id != profile_id]
        if person.active_profile == profile_id:
            person.active_profile = person.profiles[0].id
        self.save()
        return True


# --- przejście z 0.2 -------------------------------------------------------------------------


def legacy_settings(path: Path = OLD_SETTINGS) -> dict[str, Any] | None:
    """Płaskie ustawienia 0.2 (None, gdy to nowa instalacja)."""
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    return data if isinstance(data, dict) else None


def backup_legacy(path: Path = OLD_SETTINGS) -> Path | None:
    """Kopia ustawień 0.2 przed migracją (zostają też w miejscu - stara wersja dalej je czyta)."""
    if not path.exists():
        return None
    BACKUP_DIR.mkdir(parents=True, exist_ok=True)
    dst = BACKUP_DIR / "ustawienia-0.2.json"
    if not dst.exists():
        shutil.copy(path, dst)
    return dst
