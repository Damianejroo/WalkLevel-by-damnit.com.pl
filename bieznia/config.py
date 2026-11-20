"""Wczytywanie config.toml + opcjonalnego config.local.toml (nadpisania lokalne)."""

from __future__ import annotations

import tomllib
from pathlib import Path
from typing import Any

from bieznia.paths import resource_dir

DEFAULT_FILE = "config.toml"
LOCAL_FILE = "config.local.toml"


def _merge(base: dict[str, Any], override: dict[str, Any]) -> dict[str, Any]:
    result = dict(base)
    for key, value in override.items():
        if isinstance(value, dict) and isinstance(result.get(key), dict):
            result[key] = _merge(result[key], value)
        else:
            result[key] = value
    return result


def load_config(root: Path | None = None) -> dict[str, Any]:
    """Domyślne ustawienia z paczki (albo z `root`) + config.local.toml z folderu danych."""
    defaults = (root or resource_dir()) / DEFAULT_FILE
    with defaults.open("rb") as f:
        config = tomllib.load(f)
    local = (root or Path.cwd()) / LOCAL_FILE
    if local.exists():
        with local.open("rb") as f:
            config = _merge(config, tomllib.load(f))
    return config
