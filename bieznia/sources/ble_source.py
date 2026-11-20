"""Źródło odczytów z wątku Bluetooth (kolejka -> ostatni odczyt)."""

from __future__ import annotations

import queue

from bieznia.sources import Reading


class QueueSource:
    def __init__(self, readings: queue.Queue[Reading]) -> None:
        self.readings = readings

    def poll(self, now: float, actions: set[str]) -> Reading | None:
        latest: Reading | None = None
        while True:
            try:
                latest = self.readings.get_nowait()
            except queue.Empty:
                return latest

    def close(self) -> None:
        pass
