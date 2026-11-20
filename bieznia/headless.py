"""Tryb bez okna (testy, e2e, nagrania): python -m bieznia --headless [...]."""

from __future__ import annotations

import logging
import time
from pathlib import Path
from typing import Any

from bieznia.engine import Bridge
from bieznia.hotkeys import Hotkeys
from bieznia.klawisze import RUN_DEFAULT, WALK_DEFAULT, Key
from bieznia.mapping import MappingConfig
from bieznia.recorder import Recorder
from bieznia.sources import SourceFinished, create_source

log = logging.getLogger("bieznia")

TICK_S = 0.05


class StopFlag:
    def __init__(self) -> None:
        self.stop = False

    def __call__(self, signum: int, frame: object) -> None:
        self.stop = True


def run(config: dict[str, Any], enabled: bool, stop: StopFlag) -> int:
    keys = (
        Key.from_dict(config["output"].get("walk_key"), WALK_DEFAULT),
        Key.from_dict(config["output"].get("run_key"), RUN_DEFAULT),
    )
    bridge = Bridge(
        MappingConfig.from_dict(config["mapping"]), config["output"]["type"], enabled, keys
    )
    bridge.set_source(create_source(config["source"]))
    if config["record"]["enabled"]:
        recorder = Recorder(Path("nagrania"))
        bridge.set_recording(recorder)
        log.info("Nagrywam do %s", recorder.path)
    hotkeys = Hotkeys(config["hotkeys"])
    log.info(
        "Tryb bez okna | źródło=%s wyjście=%s | %s włącz/wyłącz | sterowanie %s",
        config["source"]["type"],
        config["output"]["type"],
        config["hotkeys"]["toggle"],
        "WŁĄCZONE" if enabled else "WYŁĄCZONE",
    )
    last: tuple[object, ...] | None = None
    try:
        while not stop.stop:
            state = bridge.tick(time.monotonic(), hotkeys.poll())
            c = state.control
            key = (state.enabled, c.walk, c.run, round(c.speed * 2) / 2)
            if key != last:
                last = key
                mode = "BIEG" if c.run else "CHÓD" if c.walk else "stoi"
                log.info("%-4s %4.1f km/h  gałka %3.0f%%", mode, c.speed, c.stick * 100)
            time.sleep(TICK_S)
    except SourceFinished as exc:
        log.info("Źródło zakończone: %s", exc)
    finally:
        bridge.shutdown()
    return 0
