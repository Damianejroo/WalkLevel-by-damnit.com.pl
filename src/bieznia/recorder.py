"""Zapis odczytów do nagrania .jsonl (format jak w sources/replay.py)."""

from __future__ import annotations

import json
from datetime import datetime
from pathlib import Path

from bieznia.sources import Reading


class Recorder:
    def __init__(self, directory: Path) -> None:
        directory.mkdir(parents=True, exist_ok=True)
        self.path = directory / f"{datetime.now():%Y%m%d-%H%M%S}.jsonl"
        self._file = self.path.open("w", encoding="utf-8")
        self._start: float | None = None

    def write(self, reading: Reading) -> None:
        if self._start is None:
            self._start = reading.t
        row = {
            "t": round(reading.t - self._start, 3),
            "speed": reading.speed_kmh,
            "steps": reading.steps,
        }
        self._file.write(json.dumps(row) + "\n")
        self._file.flush()

    def close(self) -> None:
        self._file.close()
