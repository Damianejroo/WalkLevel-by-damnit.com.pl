"""Punkt wejścia: python -m bieznia [opcje]. Domyślnie okno, --headless bez okna."""

from __future__ import annotations

import argparse
import asyncio
import logging
import signal
from datetime import datetime
from pathlib import Path
from typing import Any

from bieznia import __version__
from bieznia.config import load_config
from bieznia.logs import setup_logging
from bieznia.paths import FROZEN, enter_data_dir
from bieznia.settings import apply_settings, load_resumed_enabled, load_settings, save_enabled

log = logging.getLogger("bieznia")


def parse_args(argv: list[str] | None) -> argparse.Namespace:
    p = argparse.ArgumentParser(prog="bieznia", description="Bieżnia -> sterowanie w grach")
    p.add_argument("--headless", action="store_true", help="bez okna (logi w konsoli)")
    p.add_argument("--source", choices=["sim", "replay"], help="źródło w trybie bez okna")
    p.add_argument("--replay", metavar="PLIK", help="nagranie .jsonl (ustawia --source replay)")
    p.add_argument("--output", choices=["dry", "keys"], help="dry = podgląd, keys = W/Shift")
    p.add_argument("--record", action="store_true", help="zapisuj odczyty do nagrania/")
    p.add_argument("--enabled", action="store_true", help="startuj z włączonym sterowaniem")
    p.add_argument(
        "--diagnostyka", action="store_true", help="tylko raport Bluetooth do logs/ i wyjście"
    )
    return p.parse_args(argv)


def apply_overrides(config: dict[str, Any], args: argparse.Namespace) -> dict[str, Any]:
    if args.replay:
        config["source"]["type"] = "replay"
        config["source"]["replay_file"] = args.replay
    if args.source:
        config["source"]["type"] = args.source
    if args.output:
        config["output"]["type"] = args.output
    if args.record:
        config["record"]["enabled"] = True
    if args.enabled:
        config["output"]["start_enabled"] = True
    return config


def report_path() -> Path:
    return Path("logs") / f"diagnostyka-{datetime.now():%Y%m%d-%H%M%S}.txt"


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    enter_data_dir()  # exe: %LOCALAPPDATA%\Bieznia, z kodu: bieżący folder
    user = load_settings()
    setup_logging("bieznia", detailed=bool(user.get("detailed_logs", True)))
    log.info("Bieżnia %s (%s)", __version__, "exe" if FROZEN else "z kodu")
    config = apply_overrides(apply_settings(load_config(), user), args)
    resumed = load_resumed_enabled()
    enabled = bool(config["output"]["start_enabled"]) if resumed is None else resumed
    save_enabled(enabled)

    if args.diagnostyka:
        from bieznia.ble.diagnostics import run_diagnostics

        address = config.get("ble", {}).get("device_address") or None
        path = asyncio.run(run_diagnostics(report_path(), address))
        log.info("Raport zapisany: %s", path.resolve())
        return 0

    try:
        if args.headless:
            from bieznia.headless import StopFlag, run

            stop = StopFlag()
            signal.signal(signal.SIGINT, stop)
            if hasattr(signal, "SIGBREAK"):  # tak zatrzymuje nas nadzorca
                signal.signal(signal.SIGBREAK, stop)
            return run(config, enabled, stop)

        from bieznia.gui.app import run_gui

        return run_gui(config, enabled)
    except ValueError as exc:
        log.error("Błąd konfiguracji: %s", exc)
        return 2
