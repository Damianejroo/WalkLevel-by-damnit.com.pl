"""Gdzie co leży: w wersji zainstalowanej (exe) i przy uruchamianiu z kodu.

- zasoby (config.toml, ikona, wersja.txt): w paczce exe albo w repozytorium,
- dane użytkownika (dane/, logs/, nagrania/): w %LOCALAPPDATA%\\Bieznia przy exe
  (przetrwają aktualizacje i odinstalowanie), w bieżącym folderze przy kodzie.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

FROZEN = bool(getattr(sys, "frozen", False))
APP_DIR_NAME = "Bieznia"


def resource_dir() -> Path:
    if FROZEN:
        return Path(getattr(sys, "_MEIPASS", Path(sys.executable).parent))
    return Path(__file__).resolve().parents[1]


def data_dir() -> Path:
    if FROZEN:
        base = os.environ.get("LOCALAPPDATA") or str(Path.home() / "AppData" / "Local")
        return Path(base) / APP_DIR_NAME
    return Path.cwd()


def enter_data_dir() -> Path:
    """W wersji exe przechodzi do folderu danych - reszta kodu używa ścieżek względnych."""
    directory = data_dir()
    directory.mkdir(parents=True, exist_ok=True)
    os.chdir(directory)
    return directory


def app_version() -> str:
    """Wersja z tagu wydania (wersja.txt w paczce) albo "dev" przy uruchamianiu z kodu."""
    try:
        return (resource_dir() / "wersja.txt").read_text(encoding="utf-8").strip() or "dev"
    except OSError:
        return "dev"
