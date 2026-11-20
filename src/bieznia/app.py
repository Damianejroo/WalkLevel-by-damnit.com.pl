"""Główna pętla mostu: źródło -> mapowanie -> wyjście."""

from __future__ import annotations

import argparse
import contextlib
import json
import logging
import os
import signal
import time
from pathlib import Path
from typing import Any

from bieznia import __version__
from bieznia.config import load_config
from bieznia.hotkeys import Hotkeys
from bieznia.logs import setup_logging
from bieznia.mapping import Control, MappingConfig, SpeedMapper
from bieznia.outputs import create_output
from bieznia.recorder import Recorder
from bieznia.sources import SourceFinished, create_source

log = logging.getLogger("bieznia")

TICK_S = 0.05
STATE_FILE = Path("logs") / "stan.json"
RESUME_ENV = "BIEZNIA_RESUME"


def save_state(enabled: bool) -> None:
    with contextlib.suppress(OSError):
        STATE_FILE.parent.mkdir(exist_ok=True)
        STATE_FILE.write_text(json.dumps({"enabled": enabled}), encoding="utf-8")


def load_resumed_state() -> bool | None:
    """Stan włączony/wyłączony sprzed restartu – tylko po restarcie przez aktualizację.

    Zwykły start (start.bat) zawsze zaczyna od ustawienia z konfiguracji (domyślnie wyłączony).
    """
    if os.environ.get(RESUME_ENV) != "1":
        return None
    try:
        return bool(json.loads(STATE_FILE.read_text(encoding="utf-8"))["enabled"])
    except (OSError, ValueError, KeyError, TypeError):
        return None


class _StopFlag:
    def __init__(self) -> None:
        self.stop = False

    def __call__(self, signum: int, frame: object) -> None:
        self.stop = True


def parse_args(argv: list[str] | None) -> argparse.Namespace:
    p = argparse.ArgumentParser(prog="bieznia", description="Bieżnia -> klawiatura / pad")
    p.add_argument("--source", choices=["sim", "replay"], help="źródło odczytów")
    p.add_argument("--replay", metavar="PLIK", help="nagranie .jsonl (ustawia --source replay)")
    p.add_argument("--output", choices=["dry", "keys"], help="dry = tylko log, keys = W/Shift")
    p.add_argument("--record", action="store_true", help="zapisuj odczyty do nagrania/")
    p.add_argument("--enabled", action="store_true", help="startuj od razu włączony")
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


def run(config: dict[str, Any], stop: _StopFlag) -> int:
    mapping_cfg = MappingConfig.from_dict(config["mapping"])
    mapper = SpeedMapper(mapping_cfg)
    output = create_output(config["output"]["type"])
    hotkeys = Hotkeys(config["hotkeys"])
    source = create_source(config["source"])
    recorder = Recorder(Path("nagrania")) if config["record"]["enabled"] else None
    resumed = load_resumed_state()
    enabled = bool(config["output"]["start_enabled"]) if resumed is None else resumed
    save_state(enabled)

    keys = config["hotkeys"]
    log.info(
        "Bieżnia %s | źródło=%s wyjście=%s | %s włącz/wyłącz",
        __version__,
        config["source"]["type"],
        config["output"]["type"],
        keys["toggle"],
    )
    if config["source"]["type"] == "sim":
        log.info("Symulator: %s wolniej, %s szybciej", keys["sim_slower"], keys["sim_faster"])
    if recorder:
        log.info("Nagrywam do %s", recorder.path)
    log.info("Most %s", "WŁĄCZONY" if enabled else "WYŁĄCZONY")

    control = Control.idle()
    last_reading_t: float | None = None
    last_logged: tuple[object, ...] | None = None
    try:
        while not stop.stop:
            now = time.monotonic()
            actions = hotkeys.poll()
            if "toggle" in actions:
                enabled = not enabled
                save_state(enabled)
                log.info("Most %s", "WŁĄCZONY" if enabled else "WYŁĄCZONY")
                if not enabled:
                    output.release_all()

            reading = source.poll(now, actions)
            if reading is not None:
                last_reading_t = reading.t
                control = mapper.update(reading.speed_kmh)
                if recorder:
                    recorder.write(reading)
            elif last_reading_t is not None and now - last_reading_t > mapping_cfg.stale_after_s:
                # Bezpiecznik: brak danych z bieżni = stoimy.
                log.warning("Brak odczytów od %.1f s – zatrzymuję postać", now - last_reading_t)
                last_reading_t = None
                mapper.reset()
                control = Control.idle()

            if enabled:
                output.apply(control)

            state = (enabled, control.walk, control.run, round(control.speed * 2) / 2)
            if state != last_logged:
                last_logged = state
                mode = "BIEG" if control.run else "CHÓD" if control.walk else "stoi"
                log.info(
                    "%-4s %4.1f km/h  gałka %3.0f%%%s",
                    mode,
                    control.speed,
                    control.stick * 100,
                    "" if enabled else "  (most wyłączony)",
                )
            time.sleep(TICK_S)
    except SourceFinished as exc:
        log.info("Źródło zakończone: %s", exc)
    finally:
        output.release_all()
        source.close()
        if recorder:
            recorder.close()
    return 0


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    setup_logging("bieznia")
    stop = _StopFlag()
    signal.signal(signal.SIGINT, stop)
    if hasattr(signal, "SIGBREAK"):  # Windows: tym sygnałem zatrzymuje nas nadzorca
        signal.signal(signal.SIGBREAK, stop)
    config = apply_overrides(load_config(), args)
    try:
        return run(config, stop)
    except ValueError as exc:
        log.error("Błąd konfiguracji: %s", exc)
        return 2
