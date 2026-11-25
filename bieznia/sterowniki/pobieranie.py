"""Pobieranie profili modeli bieżni - DANE (JSON), nigdy kod.

Lista modeli (indeks.json) i pliki profili leżą w repozytorium aplikacji na GitHubie.
Pobieramy wyłącznie przez HTTPS z raw.githubusercontent.com, każdy plik sprawdzamy sumą
SHA-256 z listy, rozmiarem i pełną walidacją profilu (profile.parse_profile). Zapis do
dane/profile/ - nowy sterownik (kod) przychodzi tylko zwykłą aktualizacją aplikacji.

Indeks po zmianie wbudowanych profili: python -m bieznia.sterowniki.pobieranie
"""

from __future__ import annotations

import hashlib
import json
import logging
import shutil
import sys
from dataclasses import dataclass, field
from pathlib import Path
from urllib.parse import urlparse

from bieznia import REPOSITORY
from bieznia.aktualizacje import Opener, _default_opener
from bieznia.sterowniki import profile as profiles

log = logging.getLogger("bieznia")

BRANCH = "main"
BASE_URL = f"https://raw.githubusercontent.com/{REPOSITORY}/{BRANCH}/bieznia/sterowniki/profile"
INDEX = "indeks.json"
INDEX_FORMAT = 1
ALLOWED_HOSTS = frozenset({"raw.githubusercontent.com"})
MAX_BYTES = 32 * 1024
TIMEOUT_S = 15


class ProfileDownloadError(Exception):
    """Komunikat dla użytkownika."""


@dataclass
class DownloadResult:
    added: list[str] = field(default_factory=list)  # nazwy modeli
    updated: list[str] = field(default_factory=list)
    same: int = 0  # już były (wbudowane albo pobrane wcześniej)
    rejected: list[str] = field(default_factory=list)  # pliki, które nie przeszły sprawdzenia


def sha256(data: bytes) -> str:
    """Suma treści niezależna od końców linii (git na Windows potrafi zamienić LF na CRLF)."""
    return hashlib.sha256(data.replace(b"\r\n", b"\n")).hexdigest()


def build_index(directory: Path = profiles.BUILTIN_DIR) -> dict[str, object]:
    files = sorted(p for p in directory.glob("*.json") if p.name != INDEX)
    return {
        "format": INDEX_FORMAT,
        "profile": [{"plik": p.name, "sha256": sha256(p.read_bytes())} for p in files],
    }


def write_index(directory: Path = profiles.BUILTIN_DIR) -> Path:
    path = directory / INDEX
    text = json.dumps(build_index(directory), ensure_ascii=False, indent=1) + "\n"
    path.write_text(text, encoding="utf-8", newline="\n")
    return path


def _get(url: str, opener: Opener) -> bytes:
    import urllib.request

    parsed = urlparse(url)
    if parsed.scheme != "https" or parsed.hostname not in ALLOWED_HOSTS:
        raise ProfileDownloadError("Nieoczekiwany adres - przerwano dla bezpieczeństwa.")
    request = urllib.request.Request(url, headers={"User-Agent": "Bieznia-profile"})
    try:
        with opener(request, TIMEOUT_S) as response:
            final = urlparse(response.geturl())
            if final.scheme != "https" or final.hostname not in ALLOWED_HOSTS:
                raise ProfileDownloadError("Serwer przekierował poza GitHub - przerwano.")
            data = response.read(MAX_BYTES + 1)
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            raise ProfileDownloadError(
                "Lista modeli nie jest jeszcze opublikowana - spróbuj po aktualizacji aplikacji."
            ) from exc
        raise ProfileDownloadError(f"Serwer odpowiedział błędem {exc.code}.") from exc
    except (urllib.error.URLError, TimeoutError, OSError) as exc:
        raise ProfileDownloadError("Brak internetu albo GitHub nie odpowiada.") from exc
    if len(data) > MAX_BYTES:
        raise ProfileDownloadError("Plik jest za duży jak na profil bieżni - odrzucono.")
    return data


def _parse_index(data: bytes) -> list[tuple[str, str]]:
    try:
        index = json.loads(data)
        if index.get("format") != INDEX_FORMAT:
            raise ValueError("format")
        entries = [(str(e["plik"]), str(e["sha256"]).lower()) for e in index["profile"]]
    except (ValueError, KeyError, TypeError, AttributeError) as exc:
        raise ProfileDownloadError("Nieznany format listy modeli - zaktualizuj aplikację.") from exc
    for name, digest in entries:
        safe = name.endswith(".json") and Path(name).name == name and name != INDEX
        if not safe or len(digest) != 64:
            raise ProfileDownloadError("Lista modeli zawiera niepoprawny wpis - przerwano.")
    return entries


def _saved_bytes(profile_id: str) -> bytes | None:
    for directory in (profiles.USER_DIR, profiles.BUILTIN_DIR):
        for path in directory.glob("*.json") if directory.is_dir() else ():
            if path.name == INDEX:
                continue
            try:
                raw = path.read_bytes()
                if json.loads(raw).get("id") == profile_id:
                    return raw
            except (OSError, ValueError, AttributeError):
                continue
    return None


def download_profiles(opener: Opener = _default_opener) -> DownloadResult:
    """Nowe i zmienione profile z repozytorium do dane/profile (w wątku w tle)."""
    entries = _parse_index(_get(f"{BASE_URL}/{INDEX}", opener))
    result = DownloadResult()
    for name, digest in entries:
        data = _get(f"{BASE_URL}/{name}", opener)
        if sha256(data) != digest:
            log.warning("Profil %s: zła suma SHA-256 - odrzucony", name)
            result.rejected.append(name)
            continue
        try:
            found = profiles.parse_profile(json.loads(data), "pobrany")
        except ValueError as exc:
            log.warning("Profil %s odrzucony: %s", name, exc)
            result.rejected.append(name)
            continue
        current = _saved_bytes(found.id)
        if current is not None and json.loads(current) == json.loads(data):
            result.same += 1
            continue
        profiles.USER_DIR.mkdir(parents=True, exist_ok=True)
        (profiles.USER_DIR / f"{found.id}.json").write_bytes(data)
        (result.updated if current is not None else result.added).append(found.name)
    profiles.all_profiles(reload=True)
    log.info(
        "Profile bieżni: nowe %d, zmienione %d, bez zmian %d, odrzucone %d",
        len(result.added),
        len(result.updated),
        result.same,
        len(result.rejected),
    )
    return result


def install_from_file(path: Path) -> profiles.ModelProfile:
    """Profil z pliku (np. od autora albo innego użytkownika) - po pełnym sprawdzeniu."""
    try:
        data = path.read_bytes()
    except OSError as exc:
        raise ProfileDownloadError("Nie da się odczytać pliku.") from exc
    if len(data) > MAX_BYTES:
        raise ProfileDownloadError("Plik jest za duży jak na profil bieżni.")
    try:
        found = profiles.parse_profile(json.loads(data), "dodatkowy")
    except ValueError as exc:
        raise ProfileDownloadError(f"To nie jest poprawny profil bieżni: {exc}") from exc
    profiles.USER_DIR.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(path, profiles.USER_DIR / f"{found.id}.json")
    profiles.all_profiles(reload=True)
    log.info("Dodano profil bieżni %s (%s) z pliku", found.name, found.id)
    return found


def remove_user_profile(profile_id: str) -> bool:
    """Usuwa dodatkowy profil (wbudowanego nie da się usunąć - wraca jego wersja)."""
    removed = False
    if profiles.USER_DIR.is_dir():
        for path in profiles.USER_DIR.glob("*.json"):
            try:
                same = json.loads(path.read_bytes()).get("id") == profile_id
            except (OSError, ValueError, AttributeError):
                continue
            if same:
                path.unlink()
                removed = True
    profiles.all_profiles(reload=True)
    return removed


if __name__ == "__main__":
    written = write_index()
    sys.stdout.write(f"Zapisano {written}\n")
