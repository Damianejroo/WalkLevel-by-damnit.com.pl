"""Odtwarzanie nagrania .jsonl w czasie rzeczywistym.

Format linii: {"t": sekundy_od_startu, "speed": km/h, "steps": liczba_lub_null}
"""

from __future__ import annotations

import json
from pathlib import Path

from bieznia.sources import Reading, SourceFinished


def load_recording(path: Path) -> list[tuple[float, float, int | None]]:
    rows: list[tuple[float, float, int | None]] = []
    with path.open(encoding="utf-8") as f:
        for line_no, line in enumerate(f, start=1):
            line = line.strip()
            if not line:
                continue
            try:
                data = json.loads(line)
                rows.append((float(data["t"]), float(data["speed"]), data.get("steps")))
            except (ValueError, KeyError, TypeError) as exc:
                raise ValueError(f"{path}:{line_no}: zła linia nagrania: {exc}") from exc
    rows.sort(key=lambda row: row[0])
    return rows


class ReplaySource:
    def __init__(self, path: Path) -> None:
        self.rows = load_recording(path)
        self._index = 0
        self._start: float | None = None

    def poll(self, now: float, actions: set[str]) -> Reading | None:
        if self._start is None:
            self._start = now
        if self._index >= len(self.rows):
            raise SourceFinished("koniec nagrania")
        t, speed, steps = self.rows[self._index]
        if now - self._start < t:
            return None
        self._index += 1
        return Reading(t=now, speed_kmh=speed, steps=steps)

    def close(self) -> None:
        pass
