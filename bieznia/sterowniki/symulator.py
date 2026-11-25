"""Symulator bieżni jako pełnoprawny sterownik: ten sam kontrakt co prawdziwe bieżnie.

Dzięki temu całe sterowanie (start, stop, prędkość z pada, pilnowanie) działa i da się
je przetestować bez Bluetooth - w aplikacji i w GitHub Actions. Taśma rozpędza się płynnie,
po starcie jest krótkie odliczanie, kroki rosną z kadencją zależną od prędkości.
F6 / F7 działają jak pilot bieżni: zmieniają prędkość bez udziału aplikacji.
"""

from __future__ import annotations

import asyncio
import logging
import time
from collections.abc import Callable

from bieznia.sterowniki.kontrakt import Capabilities, DeviceInfo, ReadingCallback
from bieznia.sterowniki.pilnowanie import SpeedStats

log = logging.getLogger("bieznia.ble")

REPORT_EVERY_S = 0.75
ACCEL_KMH_PER_S = 1.0
REMOTE_STEP_KMH = 0.5

CAPS = Capabilities(
    speed_min=0.5,
    speed_max=6.0,
    speed_step=0.1,
    start_speed=1.0,
    start_delay_s=1.0,
    acks_commands=True,
    pause=True,
    steps=True,
    telemetry_s=REPORT_EVERY_S,
)
INFO = DeviceInfo(model="Symulator bieżni", driver="symulator", profile="symulator")


class SimulatorDriver:
    protocol = "symulator"

    def __init__(
        self,
        on_reading: ReadingCallback,
        clock: Callable[[], float] = time.monotonic,
        capabilities: Capabilities = CAPS,
    ) -> None:
        self.on_reading = on_reading
        self.clock = clock
        self.capabilities = capabilities
        self.info = INFO
        self.counts = {"commands": 0, "acks": 0, "frames": 0}
        self.speed_stats = SpeedStats()
        self.running = False
        self.target = 0.0
        self.speed = 0.0
        self.steps = 0.0
        self._last_t: float | None = None
        self._report_at = float("-inf")
        self._belt_busy = 0

    def describe(self) -> str:
        return "Symulator bieżni"

    async def start(self) -> None:
        self._last_t = self.clock()
        log.info("Symulator bieżni: %.1f-%.1f km/h", CAPS.speed_min, CAPS.speed_max)

    def advance(self) -> None:
        """Fizyka taśmy do chwili obecnej i ramka z danymi (co REPORT_EVERY_S)."""
        now = self.clock()
        dt = 0.0 if self._last_t is None else max(0.0, now - self._last_t)
        self._last_t = now
        goal = self.target if self.running else 0.0
        change = ACCEL_KMH_PER_S * dt
        self.speed += max(-change, min(change, goal - self.speed))
        if self.speed > 0.4:
            self.steps += (60 + 15 * self.speed) / 60 * dt
        if now - self._report_at >= REPORT_EVERY_S - 1e-6:
            self._report_at = now
            self.counts["frames"] += 1
            self.on_reading(round(self.speed, 1), int(self.steps))

    async def poll(self) -> None:
        self.advance()

    def poll_interval(self) -> float:
        return REPORT_EVERY_S

    def busy(self) -> bool:
        return self._belt_busy > 0

    def belt_running(self) -> bool | None:
        return self.running

    def belt_speed(self) -> tuple[float, float] | None:
        if self._report_at == float("-inf"):
            return None
        return round(self.speed, 1), self._report_at

    async def start_belt(self) -> str:
        self.counts["commands"] += 1
        if self.running:
            return "started"
        self._belt_busy += 1
        try:
            await asyncio.sleep(self.capabilities.start_delay_s)  # odliczanie jak w bieżni
            self.running = True
            self.target = max(self.target, self.capabilities.start_speed)
            self.counts["acks"] += 1
            return "started"
        finally:
            self._belt_busy -= 1

    async def stop_belt(self, verify: bool = True) -> None:
        self.counts["commands"] += 1
        self.counts["acks"] += 1
        self.running = False
        self.target = 0.0

    async def set_speed(self, kmh: float) -> bool:
        self.counts["commands"] += 1
        self.counts["acks"] += 1
        self.speed_stats.sent += 1
        self.speed_stats.ok += 1
        if self.running:
            self.target = self.capabilities.clamp(kmh) or self.capabilities.speed_min
        return True

    def remote(self, steps: int) -> None:
        """Pilot bieżni (F6/F7): prędkość zmienia sama bieżnia, nie aplikacja."""
        target = (self.target if self.running else 0.0) + steps * REMOTE_STEP_KMH
        if target <= 0.0:
            self.running, self.target = False, 0.0
            return
        self.running = True
        self.target = self.capabilities.clamp(target)
