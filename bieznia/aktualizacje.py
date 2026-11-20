"""Aktualizacja na żądanie z GitHub Releases (przycisk Aktualizuj w oknie).

1. Pytamy API GitHuba o najnowsze wydanie TEGO repozytorium.
2. Jeśli jest nowsza wersja, pobieramy jej instalator wyłącznie z hostów GitHuba.
3. Sprawdzamy SHA-256 z danych wydania - inna suma = plik odrzucony i usunięty.
4. Uruchamiamy instalator w trybie cichym; aplikacja się zamyka, instalator
   podmienia pliki i uruchamia nową wersję. Dane użytkownika zostają nietknięte.

Bez kliknięcia nic nie jest pobierane ani instalowane.
"""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import sys
import tempfile
import urllib.error
import urllib.request
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import Any, BinaryIO, Protocol
from urllib.parse import urlparse

from bieznia import REPOSITORY, __version__

API_LATEST = f"https://api.github.com/repos/{REPOSITORY}/releases/latest"
ASSET = re.compile(r"^Bieznia-Setup-(\d+\.\d+\.\d+)\.exe$")
ALLOWED_HOSTS = frozenset(
    {
        "api.github.com",
        "github.com",
        "objects.githubusercontent.com",
        "release-assets.githubusercontent.com",
    }
)
MAX_SIZE = 200 * 1024 * 1024
TIMEOUT_S = 20
CHUNK = 256 * 1024


class UpdateError(Exception):
    """Komunikat dla użytkownika (po polsku, bez szczegółów technicznych)."""


class Response(Protocol):
    def read(self, n: int = -1) -> bytes: ...

    def geturl(self) -> str: ...

    def __enter__(self) -> Response: ...

    def __exit__(self, *args: object) -> None: ...


Opener = Callable[[urllib.request.Request, float], Response]


def _default_opener(request: urllib.request.Request, timeout: float) -> Response:
    return urllib.request.urlopen(request, timeout=timeout)  # type: ignore[no-any-return]


@dataclass(frozen=True)
class Release:
    version: str
    notes: str
    url: str
    sha256: str
    size: int
    name: str


def parse_version(text: str) -> tuple[int, ...]:
    """'v1.2.3' / '1.2.3' -> (1, 2, 3). Wersja z kodu ("dev") jest najstarsza."""
    match = re.fullmatch(r"v?(\d+)\.(\d+)\.(\d+)", text.strip())
    return tuple(int(x) for x in match.groups()) if match else (0, 0, 0)


def is_newer(candidate: str, current: str = __version__) -> bool:
    return parse_version(candidate) > parse_version(current)


def _check_url(url: str) -> None:
    parsed = urlparse(url)
    if parsed.scheme != "https" or parsed.hostname not in ALLOWED_HOSTS:
        raise UpdateError("Nieoczekiwany adres pobierania - przerwano dla bezpieczeństwa.")


def _request(url: str) -> urllib.request.Request:
    _check_url(url)
    return urllib.request.Request(
        url,
        headers={
            "Accept": "application/vnd.github+json",
            "User-Agent": f"Bieznia/{__version__}",
            "X-GitHub-Api-Version": "2022-11-28",
        },
    )


def parse_release(data: dict[str, Any]) -> Release:
    version = str(data.get("tag_name", ""))
    if parse_version(version) == (0, 0, 0):
        raise UpdateError("Wydanie na GitHubie nie ma poprawnego numeru wersji.")
    for asset in data.get("assets", []):
        name = str(asset.get("name", ""))
        match = ASSET.match(name)
        if not match:
            continue
        digest = str(asset.get("digest") or "")
        if not digest.startswith("sha256:") or len(digest) != 71:
            raise UpdateError("Wydanie nie ma sumy kontrolnej - nie instaluję.")
        url = str(asset.get("browser_download_url", ""))
        _check_url(url)
        return Release(
            version=version.lstrip("v"),
            notes=str(data.get("body") or "").strip(),
            url=url,
            sha256=digest.removeprefix("sha256:").lower(),
            size=int(asset.get("size") or 0),
            name=name,
        )
    raise UpdateError("Wydanie nie zawiera instalatora.")


def fetch_latest(opener: Opener = _default_opener) -> Release:
    try:
        with opener(_request(API_LATEST), TIMEOUT_S) as response:
            data = json.loads(response.read(1_000_000))
    except UpdateError:
        raise
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            raise UpdateError("Na GitHubie nie ma jeszcze żadnego wydania.") from exc
        raise UpdateError(f"GitHub odpowiedział błędem {exc.code} - spróbuj później.") from exc
    except Exception as exc:
        raise UpdateError("Nie udało się sprawdzić aktualizacji - sprawdź internet.") from exc
    if not isinstance(data, dict):
        raise UpdateError("Nieoczekiwana odpowiedź GitHuba.")
    return parse_release(data)


def _copy_verified(
    source: BinaryIO | Response,
    target: BinaryIO,
    release: Release,
    progress: Callable[[int, int], None] | None,
) -> None:
    digest = hashlib.sha256()
    done = 0
    limit = min(MAX_SIZE, release.size + 1024) if release.size else MAX_SIZE
    while chunk := source.read(CHUNK):
        done += len(chunk)
        if done > limit:
            raise UpdateError("Plik aktualizacji jest większy niż zapowiadany - przerwano.")
        digest.update(chunk)
        target.write(chunk)
        if progress:
            progress(done, release.size)
    if digest.hexdigest() != release.sha256:
        raise UpdateError("Suma kontrolna się nie zgadza - plik odrzucony.")


def download(
    release: Release,
    directory: Path | None = None,
    opener: Opener = _default_opener,
    progress: Callable[[int, int], None] | None = None,
) -> Path:
    """Pobiera instalator i sprawdza SHA-256. Przy błędzie nic nie zostaje na dysku."""
    directory = directory or Path(tempfile.gettempdir())
    directory.mkdir(parents=True, exist_ok=True)
    target = directory / release.name
    partial = target.with_suffix(".czesciowy")
    try:
        with opener(_request(release.url), TIMEOUT_S) as response:
            _check_url(response.geturl())  # przekierowanie też musi prowadzić do GitHuba
            with partial.open("wb") as out:
                _copy_verified(response, out, release, progress)
        partial.replace(target)
        return target
    except UpdateError:
        partial.unlink(missing_ok=True)
        raise
    except Exception as exc:
        partial.unlink(missing_ok=True)
        raise UpdateError("Pobieranie przerwane - spróbuj ponownie.") from exc


def run_installer(installer: Path) -> None:
    """Instalator w trybie cichym; po instalacji sam uruchomi nową wersję aplikacji."""
    if sys.platform != "win32":
        raise UpdateError("Aktualizacja działa tylko na Windows.")
    flags = subprocess.DETACHED_PROCESS | subprocess.CREATE_NEW_PROCESS_GROUP
    subprocess.Popen(
        [str(installer), "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CLOSEAPPLICATIONS"],
        creationflags=flags,
        close_fds=True,
    )
