"""Symulowana bieżnia: F6 / F7 zmieniają zadaną prędkość, taśma dochodzi do niej płynnie.

Odczyty co 0,75 s jak w prawdziwej bieżni, więc opóźnienie jest realistyczne.
"""

from __future__ import annotations

from bieznia.sources import Reading

STEP_KMH = 0.5
MAX_KMH = 12.0
ACCEL_KMH_PER_S = 0.5
REPORT_EVERY_S = 0.75


class SimulatedTreadmill:
    def __init__(self) -> None:
        self.target = 0.0
        self.speed = 0.0
        self.steps = 0.0
        self._last_t: float | None = None
        self._last_report = float("-inf")

    def poll(self, now: float, actions: set[str]) -> Reading | None:
        if "sim_faster" in actions:
            self.target = min(MAX_KMH, self.target + STEP_KMH)
        if "sim_slower" in actions:
            self.target = max(0.0, self.target - STEP_KMH)

        dt = 0.0 if self._last_t is None else now - self._last_t
        self._last_t = now
        delta = self.target - self.speed
        max_change = ACCEL_KMH_PER_S * dt
        self.speed += max(-max_change, min(max_change, delta))
        if self.speed > 0.4:
            cadence_per_min = 60 + 15 * self.speed
            self.steps += cadence_per_min / 60 * dt

        if now - self._last_report < REPORT_EVERY_S:
            return None
        self._last_report = now
        return Reading(t=now, speed_kmh=round(self.speed, 1), steps=int(self.steps))

    def close(self) -> None:
        pass
