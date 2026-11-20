"""Wspólna konfiguracja logowania.

- konsola + logs/<nazwa>.log: poziom INFO,
- logs/bluetooth.log: pełny ruch Bluetooth (DEBUG) - do zdalnej diagnozy bieżni.
"""

from __future__ import annotations

import logging
from logging.handlers import RotatingFileHandler
from pathlib import Path

FORMAT = "%(asctime)s %(levelname)-7s %(name)s: %(message)s"


def _file(path: Path, level: int) -> RotatingFileHandler:
    handler = RotatingFileHandler(path, maxBytes=2_000_000, backupCount=3, encoding="utf-8")
    handler.setLevel(level)
    return handler


def setup_logging(name: str, root: Path | None = None, console: bool = True) -> None:
    log_dir = (root or Path.cwd()) / "logs"
    log_dir.mkdir(exist_ok=True)
    handlers: list[logging.Handler] = [_file(log_dir / f"{name}.log", logging.INFO)]
    if console:
        stream = logging.StreamHandler()
        stream.setLevel(logging.INFO)
        handlers.append(stream)
    logging.basicConfig(
        level=logging.INFO, format=FORMAT, datefmt="%H:%M:%S", handlers=handlers, force=True
    )
    if name == "bieznia":
        ble = logging.getLogger("bieznia.ble")
        ble.setLevel(logging.DEBUG)
        raw = _file(log_dir / "bluetooth.log", logging.DEBUG)
        raw.setFormatter(logging.Formatter("%(asctime)s.%(msecs)03d %(message)s", "%H:%M:%S"))
        ble.addHandler(raw)
    # bleak sam w sobie gada dużo na DEBUG - tylko ostrzeżenia
    logging.getLogger("bleak").setLevel(logging.WARNING)
