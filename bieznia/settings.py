"""Ustawienia użytkownika zmieniane w oknie (dane/ustawienia.json, poza gitem).

Kolejność ważności: config.toml < config.local.toml < dane/ustawienia.json < argumenty.
"""

from __future__ import annotations

import contextlib
import json
import logging
import os
from pathlib import Path
from typing import Any

log = logging.getLogger("bieznia")

DATA_DIR = Path("dane")
SETTINGS_FILE = DATA_DIR / "ustawienia.json"
STATE_FILE = DATA_DIR / "stan.json"
RESUME_ENV = "BIEZNIA_RESUME"

# Klucze, które okno może zapisać, i sekcja config.toml, do której należą.
SECTIONS = {
    "stand_below": "mapping",
    "run_on": "mapping",
    "run_off": "mapping",
    "output": "output",
    "walk_key": "output",
    "run_key": "output",
    "device_address": "ble",
    "device_name": "ble",
    "simulator": "ui",
    "record": "ui",
    "always_on_top": "ui",
    "floating_button": "ui",
    "floating_pos": "ui",
    "theme": "ui",
    "large_text": "ui",
    "detailed_logs": "ui",
    "last_version": "ui",
}


def load_settings(path: Path = SETTINGS_FILE) -> dict[str, Any]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return {}
    except (OSError, ValueError) as exc:
        log.warning("Nie da się odczytać %s (%s) - używam domyślnych", path, exc)
        return {}
    return {k: v for k, v in data.items() if k in SECTIONS} if isinstance(data, dict) else {}


def save_settings(settings: dict[str, Any], path: Path = SETTINGS_FILE) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(".tmp")
    tmp.write_text(json.dumps(settings, indent=2, ensure_ascii=False), encoding="utf-8")
    os.replace(tmp, path)  # atomowo: zamknięcie okna w trakcie nie zostawi pustego pliku


def apply_settings(config: dict[str, Any], settings: dict[str, Any]) -> dict[str, Any]:
    for key, value in settings.items():
        section = SECTIONS[key]
        if key == "output":
            config.setdefault("output", {})["type"] = value
        else:
            config.setdefault(section, {})[key] = value
    return config


# --- stan włączony/wyłączony między restartami po aktualizacji --------------


def save_enabled(enabled: bool, path: Path = STATE_FILE) -> None:
    with contextlib.suppress(OSError):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps({"enabled": enabled}), encoding="utf-8")


def load_resumed_enabled(path: Path = STATE_FILE) -> bool | None:
    """Tylko po restarcie przez aktualizację. Zwykły start zawsze zaczyna wyłączony."""
    if os.environ.get(RESUME_ENV) != "1":
        return None
    try:
        return bool(json.loads(path.read_text(encoding="utf-8"))["enabled"])
    except (OSError, ValueError, KeyError, TypeError):
        return None
