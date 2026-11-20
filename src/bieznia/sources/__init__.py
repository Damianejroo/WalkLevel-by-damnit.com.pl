"""Źródła odczytów z bieżni (prawdziwa bieżnia, symulator, nagranie)."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any, Protocol


@dataclass(frozen=True)
class Reading:
    t: float  # czas odczytu (time.monotonic())
    speed_kmh: float
    steps: int | None = None


class SourceFinished(Exception):
    """Źródło nie da już żadnych odczytów (np. koniec nagrania)."""


class Source(Protocol):
    def poll(self, now: float, actions: set[str]) -> Reading | None:
        """Nieblokujące: zwraca nowy odczyt albo None, gdy nic nowego nie ma."""

    def close(self) -> None: ...


def create_source(config: dict[str, Any]) -> Source:
    kind = config.get("type", "sim")
    if kind == "sim":
        from bieznia.sources.simulator import SimulatedTreadmill

        return SimulatedTreadmill()
    if kind == "replay":
        from bieznia.sources.replay import ReplaySource

        path = config.get("replay_file") or ""
        if not path:
            raise ValueError("Dla source.type = 'replay' ustaw replay_file albo użyj --replay PLIK")
        return ReplaySource(Path(path))
    raise ValueError(f"Nieznane źródło {kind!r} (dozwolone: sim, replay)")
