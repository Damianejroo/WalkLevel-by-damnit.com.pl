"""Wspólna konfiguracja logowania.

- konsola + logs/<nazwa>.log: poziom INFO,
- logs/bluetooth.log: pełny ruch Bluetooth (DEBUG) - do zdalnej diagnozy bieżni.
"""

from __future__ import annotations

import logging
import sys
import threading
from logging.handlers import RotatingFileHandler
from pathlib import Path
from types import TracebackType

FORMAT = "%(asctime)s %(levelname)-7s %(name)s: %(message)s"
FILE_DATEFMT = "%Y-%m-%d %H:%M:%S"  # w pliku z datą: zgłoszenie bierze dokładnie wybrany okres


def _file(path: Path, level: int) -> RotatingFileHandler:
    handler = RotatingFileHandler(path, maxBytes=2_000_000, backupCount=3, encoding="utf-8")
    handler.setLevel(level)
    return handler


_raw_handler: logging.Handler | None = None


def setup_logging(
    name: str, root: Path | None = None, console: bool = True, detailed: bool = True
) -> None:
    global _raw_handler
    log_dir = (root or Path.cwd()) / "logs"
    log_dir.mkdir(exist_ok=True)
    main_file = _file(log_dir / f"{name}.log", logging.INFO)
    main_file.setFormatter(logging.Formatter(FORMAT, FILE_DATEFMT))
    handlers: list[logging.Handler] = [main_file]
    if console and sys.stderr is not None:  # exe bez konsoli nie ma stderr
        stream = logging.StreamHandler()
        stream.setLevel(logging.INFO)
        handlers.append(stream)
    logging.basicConfig(
        level=logging.INFO, format=FORMAT, datefmt="%H:%M:%S", handlers=handlers, force=True
    )
    if name == "bieznia":
        ble = logging.getLogger("bieznia.ble")
        ble.setLevel(logging.DEBUG)
        if _raw_handler is not None:
            ble.removeHandler(_raw_handler)
        _raw_handler = _file(log_dir / "bluetooth.log", logging.DEBUG)
        _raw_handler.setFormatter(
            logging.Formatter("%(asctime)s.%(msecs)03d %(message)s", FILE_DATEFMT)
        )
        ble.addHandler(_raw_handler)
        set_detailed(detailed)
    # bleak sam w sobie gada dużo na DEBUG - tylko ostrzeżenia
    logging.getLogger("bleak").setLevel(logging.WARNING)
    install_crash_logging()


def set_detailed(enabled: bool) -> None:
    """Szczegółowe logi = surowa rozmowa z bieżnią w logs/bluetooth.log (DEBUG)."""
    if _raw_handler is not None:
        _raw_handler.setLevel(logging.DEBUG if enabled else logging.INFO)


def install_crash_logging() -> None:
    """Nieobsłużone błędy (także z wątków) trafiają do logu - exe nie ma konsoli."""
    crash = logging.getLogger("bieznia.blad")

    def hook(kind: type[BaseException], value: BaseException, tb: TracebackType | None) -> None:
        if issubclass(kind, KeyboardInterrupt):
            sys.__excepthook__(kind, value, tb)
            return
        crash.critical("Nieobsłużony błąd", exc_info=(kind, value, tb))

    def thread_hook(args: threading.ExceptHookArgs) -> None:
        if args.exc_value is not None:
            crash.critical(
                "Nieobsłużony błąd w wątku %s",
                args.thread.name if args.thread else "?",
                exc_info=(args.exc_type, args.exc_value, args.exc_traceback),
            )

    sys.excepthook = hook
    threading.excepthook = thread_hook
