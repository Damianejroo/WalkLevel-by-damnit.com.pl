"""Kopia danych: eksport i import osób, profili gier i wyników (plik .bieznia = ZIP z JSON).

Eksport z filtrami: które osoby i ich profile, ustawienia i profile, wyniki (od daty), logi.
Import: podgląd pliku, te same filtry, konflikt osoby: połącz albo dodaj jako nową.
Przed importem kopia obecnych danych (dane/kopie/przed-importem-...), do przywrócenia.
Plik z zewnątrz jest sprawdzany: rozmiar, nazwy w archiwum, ikony tylko PNG.
"""

from __future__ import annotations

import copy
import json
import logging
import re
import shutil
import sqlite3
import uuid
import zipfile
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Any

from bieznia import __version__, dane
from bieznia.historia import COLUMN_NAMES, VISIBILITY, History, Session

log = logging.getLogger("bieznia")

FORMAT = "bieznia-kopia"
FORMAT_VERSION = 2  # 2: sesje z bieżnią, źródłem i widocznością, czasy *_utc_s
SUFFIX = ".bieznia"
MAX_FILE_BYTES = 50 * 1024 * 1024
MAX_LOG_BYTES = 2_000_000
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"
SAFE_NAME = re.compile(
    r"^(manifest\.json|osoby\.json|historia\.jsonl|ikony/[\w-]+\.png|logi/[\w.-]+)$"
)


class CopyError(Exception):
    """Komunikat dla użytkownika."""


@dataclass
class ExportChoice:
    people: dict[str, set[str] | None]  # osoba -> profile (None = wszystkie)
    settings: bool = True
    results: bool = True
    logs: bool = False
    since: float = 0.0


@dataclass
class ExportSummary:
    people: int
    profiles: int
    sessions: int
    bytes: int = 0


def _selected(store: dane.Store, choice: ExportChoice) -> list[dane.Person]:
    out = []
    for person in store.people:
        if person.id not in choice.people:
            continue
        wanted = choice.people[person.id]
        p = copy.deepcopy(person)
        p.profiles = [pr for pr in p.profiles if wanted is None or pr.id in wanted]
        out.append(p)
    return out


def summarize(store: dane.Store, history: History | None, choice: ExportChoice) -> ExportSummary:
    people = _selected(store, choice)
    sessions = 0
    if choice.results and history is not None:
        for p in people:
            ids = {pr.id for pr in p.profiles}
            sessions += len(history.sessions(choice.since, p.id, ids))
    profiles = sum(len(p.profiles) for p in people) if choice.settings else 0
    return ExportSummary(len(people), profiles, sessions)


def export(
    store: dane.Store,
    history: History | None,
    choice: ExportChoice,
    out: Path,
    logs_dir: Path = Path("logs"),
    icons_dir: Path | None = None,
) -> ExportSummary:
    icons_dir = icons_dir or dane.ICONS_DIR
    people = _selected(store, choice)
    if not people:
        raise CopyError("Zaznacz co najmniej jedną osobę.")
    if not (choice.settings or choice.results or choice.logs):
        raise CopyError("Zaznacz, co zapisać: ustawienia, wyniki albo logi.")
    summary = summarize(store, history, choice)
    out.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(out, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        manifest = {
            "format": FORMAT,
            "wersja": FORMAT_VERSION,
            "aplikacja": __version__,
            "utworzono": datetime.now().isoformat(timespec="seconds"),
            "zawartosc": {
                "ustawienia": choice.settings,
                "wyniki": choice.results,
                "logi": choice.logs,
            },
        }
        zf.writestr("manifest.json", json.dumps(manifest, ensure_ascii=False, indent=1))
        people_out = []
        for p in people:
            d = p.to_dict()
            if not choice.settings:
                d["profile"] = [
                    {"id": pr.id, "nazwa": pr.name, "kolor": pr.color} for pr in p.profiles
                ]
                d["wyglad"] = {}
            people_out.append(d)
            if choice.settings:
                for pr in p.profiles:
                    if pr.icon_file and (icons_dir / pr.icon_file).exists():
                        zf.write(icons_dir / pr.icon_file, f"ikony/{Path(pr.icon_file).stem}.png")
        zf.writestr("osoby.json", json.dumps({"osoby": people_out}, ensure_ascii=False, indent=1))
        if choice.results and history is not None:
            lines = []
            for p in people:
                ids = {pr.id for pr in p.profiles}
                for s in history.sessions(choice.since, p.id, ids):
                    lines.append(json.dumps(dict(zip(COLUMN_NAMES, s.row(), strict=True))))
            zf.writestr("historia.jsonl", "\n".join(lines) + ("\n" if lines else ""))
        if choice.logs and logs_dir.exists():
            for f in sorted(logs_dir.glob("*.log")):
                data = f.read_bytes()[-MAX_LOG_BYTES:]
                zf.writestr(f"logi/{f.name}", data)
    summary.bytes = out.stat().st_size
    log.info(
        "Eksport: %d osób, %d profili, %d sesji -> %s",
        summary.people,
        summary.profiles,
        summary.sessions,
        out.name,
    )
    return summary


# Wersja 1 (0.3.x): dziesięć pól, czasy pod nazwami start / koniec.
_V1_NAMES = {"start": "start_utc_s", "koniec": "koniec_utc_s"}


def session_from_json(d: dict[str, Any]) -> Session:
    """Sesja z historia.jsonl dowolnej wersji kopii (brakujące pola - domyślne)."""
    for old, new in _V1_NAMES.items():
        if old in d and new not in d:
            d[new] = d.pop(old)
    for required in COLUMN_NAMES[:7]:
        if required not in d:
            raise KeyError(required)
    defaults = Session("", "", "", "", "", 0.0, 0.0)
    pairs = zip(COLUMN_NAMES, _FIELDS, strict=True)
    session = Session(**{f: d.get(name, getattr(defaults, f)) for name, f in pairs})
    if session.visibility not in VISIBILITY:
        session.visibility = "prywatna"
    return session


_FIELDS = tuple(Session.__dataclass_fields__)


# --- import ---------------------------------------------------------------------------------


@dataclass
class Preview:
    path: Path
    app_version: str
    has_settings: bool
    has_results: bool
    people: list[dane.Person]
    sessions: list[Session]
    icons: dict[str, bytes] = field(default_factory=dict)

    def sessions_of(self, person_id: str) -> list[Session]:
        return [s for s in self.sessions if s.person == person_id]


def read(path: Path) -> Preview:
    """Sprawdza i wczytuje plik kopii (bez zmian w danych)."""
    if not path.exists() or path.stat().st_size > MAX_FILE_BYTES:
        raise CopyError("Plik nie istnieje albo jest za duży.")
    try:
        with zipfile.ZipFile(path) as zf:
            names = zf.namelist()
            bad = [n for n in names if not SAFE_NAME.match(n)]
            if bad:
                raise CopyError("To nie jest plik kopii Bieżni (nieznane pliki w środku).")
            manifest = json.loads(zf.read("manifest.json"))
            if manifest.get("format") != FORMAT:
                raise CopyError("To nie jest plik kopii Bieżni.")
            if int(manifest.get("wersja", 0)) > FORMAT_VERSION:
                raise CopyError("Plik pochodzi z nowszej wersji aplikacji - zaktualizuj Bieżnię.")
            raw_people = json.loads(zf.read("osoby.json")).get("osoby", [])
            people = [dane.Person.from_dict(p) for p in raw_people if isinstance(p, dict)]
            sessions: list[Session] = []
            if "historia.jsonl" in names:
                for line in zf.read("historia.jsonl").decode("utf-8").splitlines():
                    if line.strip():
                        d = json.loads(line)
                        sessions.append(session_from_json(d))
            icons = {}
            for n in names:
                if n.startswith("ikony/"):
                    data = zf.read(n)
                    if data.startswith(PNG_MAGIC):
                        icons[Path(n).name] = data
    except CopyError:
        raise
    except (zipfile.BadZipFile, KeyError, ValueError, TypeError) as exc:
        raise CopyError("Plik kopii jest uszkodzony.") from exc
    content = manifest.get("zawartosc", {})
    return Preview(
        path,
        str(manifest.get("aplikacja", "?")),
        bool(content.get("ustawienia")),
        bool(content.get("wyniki")),
        people,
        sessions,
        icons,
    )


@dataclass
class ImportChoice:
    people: set[str]  # osoby z pliku do zaimportowania
    settings: bool = True
    results: bool = True
    new_person: set[str] = field(default_factory=set)  # osoba istnieje, ale dodać jako nową
    keep_my_settings: bool = False


@dataclass
class ImportReport:
    people_added: int = 0
    people_merged: int = 0
    profiles_added: int = 0
    profiles_updated: int = 0
    sessions_added: int = 0
    backup: Path | None = None


def backup_current(store: dane.Store, history_path: Path) -> Path:
    """Kopia danych sprzed importu: osoby.json, historia.db i ikony."""
    stamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    folder = dane.BACKUP_DIR / f"przed-importem-{stamp}"
    folder.mkdir(parents=True, exist_ok=True)
    store.save()
    shutil.copy(store.path, folder / "osoby.json")
    if history_path.exists():
        src = sqlite3.connect(str(history_path))
        dst = sqlite3.connect(str(folder / "historia.db"))
        with dst:
            src.backup(dst)
        src.close()
        dst.close()
    if dane.ICONS_DIR.exists():
        shutil.copytree(dane.ICONS_DIR, folder / "ikony", dirs_exist_ok=True)
    return folder


def latest_backup() -> Path | None:
    if not dane.BACKUP_DIR.exists():
        return None
    folders = sorted(dane.BACKUP_DIR.glob("przed-importem-*"))
    return folders[-1] if folders else None


def restore(folder: Path, store_path: Path, history_path: Path) -> None:
    """Przywraca kopię sprzed importu (aplikacja wczytuje dane od nowa)."""
    shutil.copy(folder / "osoby.json", store_path)
    if (folder / "historia.db").exists():
        src = sqlite3.connect(str(folder / "historia.db"))
        dst = sqlite3.connect(str(history_path))
        with dst:
            src.backup(dst)
        src.close()
        dst.close()
    if (folder / "ikony").exists():
        shutil.copytree(folder / "ikony", dane.ICONS_DIR, dirs_exist_ok=True)
    log.info("Przywrócono kopię %s", folder.name)


def _save_icon(preview: Preview, icon_file: str | None, profile_id: str) -> str | None:
    if not icon_file:
        return None
    data = preview.icons.get(f"{Path(icon_file).stem}.png")
    if data is None:
        return None
    dane.ICONS_DIR.mkdir(parents=True, exist_ok=True)
    name = f"{profile_id}.png"
    (dane.ICONS_DIR / name).write_bytes(data)
    return name


def _unique(name: str, taken: set[str]) -> str:
    if name not in taken:
        return name
    i = 2
    while f"{name} {i}" in taken:
        i += 1
    return f"{name} {i}"


def apply_import(
    store: dane.Store, history: History, preview: Preview, choice: ImportChoice, history_path: Path
) -> ImportReport:
    report = ImportReport(backup=backup_current(store, history_path))
    existing = {p.id: p for p in store.people}
    active = store.active_person
    for src in preview.people:
        if src.id not in choice.people:
            continue
        sessions = preview.sessions_of(src.id)
        target = existing.get(src.id)
        if target is not None and src.id not in choice.new_person:
            report.people_merged += 1
            if choice.settings and preview.has_settings:
                by_name = {pr.name: pr for pr in target.profiles}
                for pr in src.profiles:
                    mine = by_name.get(pr.name)
                    if mine is None:
                        new = copy.deepcopy(pr)
                        new.id = (
                            dane.new_id() if any(x.id == pr.id for x in target.profiles) else pr.id
                        )
                        new.icon_file = _save_icon(preview, pr.icon_file, new.id)
                        target.profiles.append(new)
                        report.profiles_added += 1
                    elif not choice.keep_my_settings:
                        mine.settings = copy.deepcopy(pr.settings)
                        mine.color, mine.glyph = pr.color, pr.glyph
                        saved = _save_icon(preview, pr.icon_file, mine.id)
                        mine.icon_file = saved or mine.icon_file
                        report.profiles_updated += 1
        else:
            person = copy.deepcopy(src)
            sessions = [copy.deepcopy(x) for x in sessions]
            if person.id in existing:  # ta sama osoba jako nowa - wszystko z nowymi id
                person.id = dane.new_id()
                remap = {}
                for pr in person.profiles:
                    remap[pr.id] = pr.id = dane.new_id()
                for x in sessions:
                    x.id = uuid.uuid4().hex
                    x.person = person.id
                    x.profile = remap.get(x.profile, x.profile)
            person.name = _unique(person.name, set(store.names()))
            for x in sessions:
                x.person_name = person.name
            if not (choice.settings and preview.has_settings):
                person.profiles = [
                    dane.Profile(dane.new_id(), "Domyślny", glyph=dane.DEFAULT_GLYPH)
                ]
                person.look = {}
            for pr in person.profiles:
                pr.icon_file = _save_icon(preview, pr.icon_file, pr.id)
            person.active_profile = person.profiles[0].id
            store.people.append(person)
            existing[person.id] = person
            report.people_added += 1
            report.profiles_added += len(person.profiles)
        if choice.results and preview.has_results:
            report.sessions_added += history.merge(sessions)
    store.active_person = active
    store.save()
    log.info(
        "Import: dodane osoby %d, połączone %d, profile +%d/~%d, sesje +%d",
        report.people_added,
        report.people_merged,
        report.profiles_added,
        report.profiles_updated,
        report.sessions_added,
    )
    return report


def default_name() -> str:
    return f"bieznia-kopia-{datetime.now():%Y-%m-%d}{SUFFIX}"


def describe(preview: Preview) -> str:
    profiles = sum(len(p.profiles) for p in preview.people)
    return (
        f"z wersji {preview.app_version} · {len(preview.people)} os. · {profiles} profili"
        f" · {len(preview.sessions)} sesji"
    )
