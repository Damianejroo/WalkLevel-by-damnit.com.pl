"""Zamiana prędkości taśmy na sterowanie: W / Shift+W i wychylenie gałki pada.

Czysta logika bez Windows API, w całości pokryta testami.
"""

from __future__ import annotations

from collections import deque
from dataclasses import dataclass
from typing import Any

STOPPED_KMH = 0.05


@dataclass(frozen=True)
class MappingConfig:
    stand_below: float = 0.8
    run_on: float = 6.3
    run_off: float = 5.7
    max_speed: float = 12.0
    smoothing_samples: int = 3
    stale_after_s: float = 2.0

    def __post_init__(self) -> None:
        if not 0 <= self.stand_below < self.run_off <= self.run_on <= self.max_speed:
            raise ValueError(
                "Wymagane: 0 <= stand_below < run_off <= run_on <= max_speed, "
                f"jest: {self.stand_below=}, {self.run_off=}, {self.run_on=}, {self.max_speed=}"
            )
        if self.smoothing_samples < 1:
            raise ValueError("smoothing_samples musi być >= 1")

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> MappingConfig:
        known = {k: v for k, v in data.items() if k in cls.__dataclass_fields__}
        return cls(**known)


@dataclass(frozen=True)
class Control:
    speed: float  # wygładzona prędkość, km/h
    stick: float  # 0.0-1.0, wychylenie lewej gałki do przodu
    walk: bool  # W
    run: bool  # Shift (razem z W)

    @classmethod
    def idle(cls) -> Control:
        return cls(speed=0.0, stick=0.0, walk=False, run=False)


class SpeedMapper:
    def __init__(self, config: MappingConfig) -> None:
        self.config = config
        self._window: deque[float] = deque(maxlen=config.smoothing_samples)
        self._running = False

    def reset(self) -> None:
        self._window.clear()
        self._running = False

    def update(self, speed_kmh: float) -> Control:
        cfg = self.config
        self._window.append(max(0.0, speed_kmh))
        speed = sum(self._window) / len(self._window)

        # stojąca taśma nigdy nie rusza postaci, nawet przy progu chodu 0 km/h
        if speed < cfg.stand_below or speed <= STOPPED_KMH:
            self._running = False
            return Control(speed=speed, stick=0.0, walk=False, run=False)

        if self._running:
            self._running = speed >= cfg.run_off
        else:
            self._running = speed >= cfg.run_on

        stick = (speed - cfg.stand_below) / (cfg.max_speed - cfg.stand_below)
        stick = min(1.0, max(0.0, stick))
        return Control(speed=speed, stick=stick, walk=True, run=self._running)
