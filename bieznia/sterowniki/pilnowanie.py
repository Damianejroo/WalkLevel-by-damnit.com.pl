"""Pilnowanie prędkości dla bieżni, które nie potwierdzają poleceń (KingSmith X21).

X21 po cichu gubi część poleceń prędkości. Czy posłuchał, widać dopiero w ramce z danymi:
bez reakcji wysyłamy to samo polecenie jeszcze raz (najwyżej RESENDS razy), a taśmie,
która już zmienia prędkość, dajemy czas na rozpędzenie.
"""

from __future__ import annotations

import logging
import time
from collections.abc import Awaitable, Callable
from dataclasses import dataclass, field

log = logging.getLogger("bieznia.ble")

TOLERANCE = 0.15
RESEND_S = 2.5
RAMP_S = 8.0  # tyle najwyżej czekamy, gdy taśma już zmienia prędkość
RESENDS = 5


@dataclass
class SpeedStats:
    """Liczniki poleceń prędkości jednego połączenia (do podsumowania w logu)."""

    sent: int = 0
    ok: int = 0
    failed: int = 0
    resends: int = 0
    latencies: list[float] = field(default_factory=list)


class SpeedGuard:
    def __init__(
        self,
        send: Callable[[float], Awaitable[bool]],
        reading: Callable[[], tuple[float, float] | None],
        reachable: Callable[[float], float] = lambda kmh: kmh,
        clock: Callable[[], float] = time.monotonic,
    ) -> None:
        self._send = send
        self._reading = reading
        self._reachable = reachable
        self._clock = clock
        self.stats = SpeedStats()
        self.target: float | None = None
        self.confirmed = True
        self._sent_at = 0.0
        self._first_sent_at = 0.0
        self._speed_at_send = 0.0
        self._resends = 0

    def reset(self) -> None:
        """Stop taśmy: zadanej prędkości już nie pilnujemy."""
        self.target = None
        self.confirmed = True

    async def set(self, kmh: float) -> bool:
        """Nowa prędkość docelowa - wysyłana od razu, potem pilnowana w check()."""
        if kmh != self.target or self.confirmed:
            self.stats.sent += 1
        self._resends = 0
        self._first_sent_at = self._clock()
        return await self._transmit(kmh)

    async def _transmit(self, kmh: float) -> bool:
        reading = self._reading()
        if not await self._send(kmh):
            return False
        self.target = kmh
        self._sent_at = self._clock()
        self._speed_at_send = reading[0] if reading else 0.0
        self.confirmed = False
        return True

    async def check(self) -> None:
        """Czy taśma posłuchała? Bez reakcji - to samo polecenie jeszcze raz."""
        target = self.target
        if target is None or self.confirmed:
            return
        reading = self._reading()
        if reading is None:
            return
        speed, at = reading
        want = self._reachable(target)  # np. 0 km/h przy bieżni, która jedzie najwolniej 1,0
        if abs(speed - want) <= TOLERANCE:
            self.confirmed = True  # od teraz pilot może zmieniać prędkość bez walki
            took = at - self._first_sent_at
            self.stats.ok += 1
            self.stats.latencies.append(took)
            log.info(
                "Prędkość %.1f: przyjęta po %.1f s (ponowienia %d)", target, took, self._resends
            )
            return
        now = self._clock()
        if speed <= 0 or at <= self._sent_at or now - self._sent_at < RESEND_S:
            return  # taśma stoi albo nie ma jeszcze świeżej ramki
        start = self._speed_at_send
        moving = (speed - start) * (want - start) > 0
        if moving and now - self._sent_at < RAMP_S:
            return
        if self._resends >= RESENDS:
            log.warning("Taśma nie przyjęła %.1f km/h (jedzie %.1f) - poddaję się", target, speed)
            self.stats.failed += 1
            self.confirmed = True
            return
        self._resends += 1
        self.stats.resends += 1
        log.info(
            "Taśma nie przyjęła %.1f km/h (jedzie %.1f) - ponawiam (%d)",
            target,
            speed,
            self._resends,
        )
        await self._transmit(target)
