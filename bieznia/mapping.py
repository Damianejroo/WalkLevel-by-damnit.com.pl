"""Zamiana prędkości taśmy na sterowanie: W / Shift+W i wychylenie gałki pada.

Czysta logika bez Windows API, w całości pokryta testami.
"""

from __future__ import annotations

import time
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
    # Bieżnia podaje już płynną prędkość taśmy - wygładzanie tylko dodawało opóźnienie.
    smoothing_samples: int = 1
    stale_after_s: float = 3.0
    # Szybkie zatrzymanie: przy stopie X21 zwalnia ok. 0,7-1 km/h na odczyt (co 2 s),
    # a zmiana tempa pilotem to ok. 0,2-0,3 km/h. Duży spadek = hamowanie = postać staje
    # od razu, zamiast czekać kilka sekund na pełne zatrzymanie taśmy.
    quick_stop: bool = True
    brake_drop: float = 0.6  # pilot zmienia do 0,5 km/h na odczyt, stop to 0,7-1,0
    # Blisko progu chodu każdy spadek prędkości oznacza zatrzymywanie się
    # (X21 przy stopie zwalnia przez 1,5 -> 1,0 -> 0 km/h).
    brake_near: float = 0.6
    # Ile sekund bez dalszego spadku, zanim uznamy, że to było tylko zwolnienie.
    brake_hold_s: float = 3.0

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
        self._last_raw: float | None = None
        self._braking = False
        self._last_fall = 0.0

    @property
    def braking(self) -> bool:
        return self._braking

    def reset(self) -> None:
        self._window.clear()
        self._running = False
        self._last_raw = None
        self._braking = False

    def _track_braking(self, raw: float, now: float) -> None:
        cfg = self.config
        last = self._last_raw
        self._last_raw = raw
        if not cfg.quick_stop or raw <= STOPPED_KMH:
            self._braking = False
            return
        if last is not None and raw < last - 1e-9:
            if last - raw >= cfg.brake_drop or raw <= cfg.stand_below + cfg.brake_near:
                self._braking = True
            self._last_fall = now  # każdy dalszy spadek podtrzymuje hamowanie
        elif last is not None and raw > last + 1e-9:
            self._braking = False  # taśma przyspiesza - idziemy dalej
        if self._braking and now - self._last_fall > cfg.brake_hold_s:
            self._braking = False  # przestało spadać - to było tylko zwolnienie

    def update(self, speed_kmh: float, now: float | None = None) -> Control:
        cfg = self.config
        raw = max(0.0, speed_kmh)
        self._track_braking(raw, time.monotonic() if now is None else now)
        self._window.append(raw)
        speed = sum(self._window) / len(self._window)

        if self._braking:
            self._running = False
            return Control(speed=speed, stick=0.0, walk=False, run=False)

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
