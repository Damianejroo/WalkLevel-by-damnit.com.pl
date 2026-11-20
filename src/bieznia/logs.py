"""Wspólna konfiguracja logowania: konsola + plik w logs/ (rotowany)."""

from __future__ import annotations

import logging
from logging.handlers import RotatingFileHandler
from pathlib import Path


def setup_logging(name: str, root: Path | None = None, console: bool = True) -> None:
    log_dir = (root or Path.cwd()) / "logs"
    log_dir.mkdir(exist_ok=True)
    handlers: list[logging.Handler] = [
        RotatingFileHandler(
            log_dir / f"{name}.log", maxBytes=1_000_000, backupCount=3, encoding="utf-8"
        )
    ]
    if console:
        handlers.append(logging.StreamHandler())
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s %(levelname)-7s %(name)s: %(message)s",
        datefmt="%H:%M:%S",
        handlers=handlers,
        force=True,
    )
