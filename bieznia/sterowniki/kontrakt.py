"""Kontrakt sterownika bieżni - to, co rdzeń aplikacji wie o każdej bieżni.

Okno i wątek Bluetooth nie pytają o model, tylko o możliwości (`Capabilities`): zakres
prędkości, odliczanie przed ruszeniem, czy bieżnia potwierdza polecenia, czy zna pauzę,
czy podaje kroki. Każdy moduł (kingsmith, ftms, symulator) spełnia ten sam kontrakt i
przechodzi ten sam zestaw testów (tests/test_kontrakt.py).
"""

from __future__ import annotations

from collections.abc import Callable, Iterable
from dataclasses import dataclass, replace
from typing import Any, Literal, Protocol

ReadingCallback = Callable[[float, int | None], None]  # (km/h, kroki albo None)

# Wynik startu taśmy: ruszyła / bieżnia odmówiła / brak odpowiedzi (prędkość pokaże)
StartResult = Literal["started", "refused", "unknown", "unsupported"]


class UnsupportedDevice(Exception):
    """Urządzenie nie ma żadnej znanej usługi bieżni."""


@dataclass(frozen=True)
class Capabilities:
    """Co bieżnia potrafi - dane, nie kod. Część z profilu modelu, część z samej bieżni."""

    speed_min: float = 0.5  # km/h - najwolniej, jak taśma pojedzie
    speed_max: float = 6.0
    speed_step: float = 0.1
    start_speed: float = 1.0  # z taką prędkością taśma rusza po starcie
    start_delay_s: float = 0.0  # wbudowane odliczanie przed ruszeniem (nie da się wyłączyć)
    acks_commands: bool = False  # każde polecenie dostaje odpowiedź z wynikiem
    pause: bool = False  # pauza różna od stopu (bieżnia trzyma sesję)
    steps: bool = True  # bieżnia podaje licznik kroków
    incline: tuple[float, float, float] | None = None  # (od, do, co) w %
    telemetry_s: float = 1.0  # typowo co tyle przychodzi ramka z prędkością
    # Najkrótszy odstęp między poleceniami prędkości: przy szybkim ruszaniu gałką
    # liczy się tylko najnowsza prędkość (bieżnie z potwierdzeniem nie lubią serii).
    min_command_gap_s: float = 0.0
    control: bool = True  # aplikacja może uruchamiać, zatrzymywać i zmieniać prędkość

    def clamp(self, kmh: float) -> float:
        """Prędkość, którą taśma naprawdę pojedzie (0 = stop zostaje 0)."""
        if kmh <= 0:
            return 0.0
        return round(min(self.speed_max, max(self.speed_min, kmh)), 1)

    def with_range(self, lo: float, hi: float, step: float) -> Capabilities:
        """Zakres zgłoszony przez samą bieżnię wygrywa z profilem (sprawdzone: Urevo)."""
        if not 0 < lo < hi:
            return self
        return replace(self, speed_min=lo, speed_max=hi, speed_step=step or self.speed_step)


@dataclass(frozen=True)
class DeviceInfo:
    """Co to za bieżnia - zapisywane przy każdej sesji w historii."""

    model: str  # czytelna nazwa, np. "Urevo SpaceWalk 5L"
    driver: str  # moduł: "kingsmith" | "ftms" | "symulator"
    profile: str = ""  # id profilu modelu (dane/profile) albo ""
    name: str = ""  # nazwa Bluetooth, np. "URTM054"
    firmware: str = ""

    @property
    def simulated(self) -> bool:
        return self.driver == "symulator"


class GattClient(Protocol):
    """Podzbiór BleakClient, którego używają sterowniki (ułatwia testy na wirtualnej bieżni)."""

    @property
    def services(self) -> Iterable[Any]: ...

    async def start_notify(self, char_specifier: Any, callback: Any) -> None: ...

    async def write_gatt_char(
        self, char_specifier: Any, data: bytes, response: bool | None = None
    ) -> None: ...


class Driver(Protocol):
    """Sterownik jednej bieżni. Wszystkie metody działają w pętli asyncio wątku Bluetooth.

    - start(): subskrypcje i rozpoznanie bieżni; potem `capabilities` i `info` są pewne,
    - poll(): co poll_interval() - odpytanie i pilnowanie zadanej prędkości,
    - start_belt() / stop_belt(): z ponawianiem do skutku albo do limitu prób,
    - set_speed(): prędkość docelowa; sterownik sam pilnuje, żeby taśma jej posłuchała,
    - belt_speed() / belt_running(): ostatni stan z danych bieżni (None = nie wiadomo).
    """

    protocol: str
    capabilities: Capabilities
    info: DeviceInfo
    counts: dict[str, int]

    async def start(self) -> None: ...

    async def poll(self) -> None: ...

    def poll_interval(self) -> float: ...

    def describe(self) -> str: ...

    async def start_belt(self) -> str: ...

    async def stop_belt(self, verify: bool = True) -> None: ...

    async def set_speed(self, kmh: float) -> bool: ...

    def belt_speed(self) -> tuple[float, float] | None: ...

    def belt_running(self) -> bool | None: ...

    def busy(self) -> bool:
        """Start albo stop w toku - wtedy prędkości nie pilnujemy."""
        ...
