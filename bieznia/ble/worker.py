"""Bluetooth w osobnym wątku: skanowanie, połączenie z auto-ponawianiem, diagnostyka.

Interfejs (wątek główny) wywołuje metody BleWorker i odbiera zdarzenia z kolejki
`events`, a odczyty prędkości z kolejki `readings`. Cała komunikacja z bleak
odbywa się w jednej pętli asyncio w wątku "bluetooth".

Worker nie zna modeli bieżni: rozmawia ze sterownikiem przez kontrakt
(bieznia.sterowniki.kontrakt). Symulator to taki sam sterownik - bez Bluetooth.
"""

from __future__ import annotations

import asyncio
import concurrent.futures
import contextlib
import logging
import queue
import threading
import time
from collections.abc import Coroutine
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Any

from bieznia.sources import Reading
from bieznia.sterowniki import (
    Capabilities,
    DeviceInfo,
    Driver,
    SimulatorDriver,
    UnsupportedDevice,
    looks_like_treadmill,
    make_driver,
)
from bieznia.sterowniki.pilnowanie import SpeedStats

log = logging.getLogger("bieznia.ble")


@dataclass
class SessionStats:
    """Liczniki jednego połączenia - na koniec jedna linia podsumowania w logu."""

    began: float = field(default_factory=time.monotonic)
    starts: int = 0
    starts_ok: int = 0
    stops: int = 0

    def log_summary(self, counts: dict[str, int], speed: SpeedStats | None = None) -> None:
        minutes = (time.monotonic() - self.began) / 60
        speed = speed or SpeedStats()
        lat = sorted(speed.latencies)
        typical = f"{lat[len(lat) // 2]:.1f} s" if lat else "-"
        worst = f"{lat[-1]:.1f} s" if lat else "-"
        log.info(
            "Podsumowanie sesji: %.0f min · prędkość: wysłana %d, przyjęta %d, bez skutku %d,"
            " ponowienia %d, czas do zmiany typowo %s, najdłużej %s · start %d/%d · stop %d ·"
            " ramki %d, zapytania %d, nieodszyfrowane %d",
            minutes,
            speed.sent,
            speed.ok,
            speed.failed,
            speed.resends,
            typical,
            worst,
            self.starts_ok,
            self.starts,
            self.stops,
            counts.get("frames", 0),
            counts.get("polls", 0),
            counts.get("undecoded", 0),
        )


POLL_S = 0.75
SPEED_ATTEMPTS = 3  # tyle razy prędkość bez potwierdzenia idzie od nowa (co odpytanie)
SCAN_S = 6.0
FIND_S = 10.0
CONNECT_TIMEOUT_S = 20.0
RETRY_MAX_S = 20.0


class Link(Enum):
    IDLE = "idle"  # nic nie robimy
    SEARCHING = "searching"  # skan / szukanie znanej bieżni
    CONNECTING = "connecting"
    CONNECTED = "connected"  # połączono, czekamy na dane
    STREAMING = "streaming"  # dane płyną
    RETRYING = "retrying"
    BLUETOOTH_OFF = "bluetooth_off"
    UNSUPPORTED = "unsupported"


@dataclass(frozen=True)
class FoundDevice:
    address: str
    name: str
    rssi: int
    treadmill: bool
    services: tuple[str, ...] = ()


@dataclass(frozen=True)
class BleEvent:
    kind: str  # "link" | "devices" | "scan_failed" | "report" | "belt" | "device"
    link: Link | None = None
    text: str = ""
    devices: tuple[FoundDevice, ...] = field(default_factory=tuple)
    device: DeviceInfo | None = None  # "device": co to za bieżnia i co potrafi
    caps: Capabilities | None = None


def friendly_error(exc: BaseException) -> tuple[Link, str]:
    from bleak.exc import BleakBluetoothNotAvailableError, BleakDeviceNotFoundError

    if isinstance(exc, BleakBluetoothNotAvailableError):
        return Link.BLUETOOTH_OFF, "Bluetooth w Windows jest wyłączony albo niedostępny - włącz go"
    if isinstance(exc, BleakDeviceNotFoundError):
        return Link.RETRYING, "Nie widzę bieżni - czy jest włączona i odłączona od telefonu?"
    if isinstance(exc, UnsupportedDevice):
        return Link.UNSUPPORTED, str(exc)
    if isinstance(exc, TimeoutError):
        return Link.RETRYING, "Bieżnia nie odpowiedziała na czas"
    return Link.RETRYING, f"Błąd Bluetooth: {exc}"


class BleWorker:
    def __init__(self) -> None:
        self.events: queue.Queue[BleEvent] = queue.Queue()
        self.readings: queue.Queue[Reading] = queue.Queue()
        self._loop = asyncio.new_event_loop()
        self._thread = threading.Thread(target=self._run, name="bluetooth", daemon=True)
        self._session: asyncio.Task[None] | None = None
        self._target: tuple[str, str] | None = None
        self._commands: asyncio.Queue[str] | None = None
        self._belt_task: asyncio.Task[None] | None = None
        self._belt_kind = ""
        self._driver: Driver | None = None
        self._auto_start = False
        self._requested_at = 0.0  # kiedy okno poprosiło o obecną prędkość
        self._stats = SessionStats()
        self._rssi: dict[str, int] = {}
        self._target_speed: float | None = None  # ostatnia prośba z okna
        self._sent_speed: float | None = None  # ostatnia wysłana do bieżni
        self._sent_at = 0.0
        self._speed_later = False
        self._speed_failures = 0
        self._caps: Capabilities | None = None
        self._thread.start()

    # --- API dla wątku głównego --------------------------------------------

    def scan(self) -> None:
        self._submit(self._scan())

    def connect(self, address: str, name: str) -> None:
        self._submit(self._start_session(address, name))

    def connect_simulator(self) -> None:
        """Symulator bieżni zamiast Bluetooth - ten sam kontrakt i to samo sterowanie."""
        self._submit(self._start_simulator())

    def sim_remote(self, steps: int) -> None:
        """Pilot symulatora (F6/F7): prędkość zmienia sama bieżnia."""

        def press() -> None:
            if isinstance(self._driver, SimulatorDriver):
                self._driver.remote(steps)

        self._loop.call_soon_threadsafe(press)

    def disconnect(self) -> None:
        self._submit(self._stop_session(forget=True))

    def diagnose(self, path: Path) -> None:
        self._submit(self._diagnose(path))

    def belt(self, command: str) -> None:
        """Start ("start") albo stop ("stop") taśmy. Wynik przychodzi jako zdarzenie "belt"."""
        if command not in ("start", "stop"):
            raise ValueError(command)
        self._loop.call_soon_threadsafe(self._queue_command, command)

    def set_speed(self, kmh: float, start_if_stopped: bool = False) -> None:
        """Prędkość docelowa taśmy. Liczy się tylko najnowsza - starsze prośby przepadają.

        start_if_stopped: stojącą taśmę najpierw uruchom (pad: ruch gałki budzi bieżnię).
        """
        self._loop.call_soon_threadsafe(self._queue_speed, round(kmh, 1), start_if_stopped)

    def _queue_speed(self, kmh: float, start_if_stopped: bool = False) -> None:
        if kmh != self._target_speed:
            self._requested_at = time.monotonic()
        self._target_speed = kmh
        self._auto_start = start_if_stopped
        if self._commands is not None:
            self._commands.put_nowait("speed")

    def _queue_command(self, command: str) -> None:
        if self._commands is None:
            self.events.put(BleEvent("belt", text="not_connected"))
            return
        self._commands.put_nowait(command)

    def stop_belt_now(self, timeout: float = 1.5) -> bool:
        """Stop taśmy z czekaniem (zamykanie aplikacji). False, gdy nie było połączenia."""
        try:
            return bool(self._submit(self._stop_now()).result(timeout))
        except (concurrent.futures.TimeoutError, RuntimeError):
            log.warning("Stop taśmy nie zdążył się wysłać")
            return False

    async def _stop_now(self) -> bool:
        driver = self._driver
        if driver is None:
            return False
        if self._belt_task is not None and not self._belt_task.done():
            self._belt_task.cancel()
        if driver.belt_running() is False:
            return False  # taśma stoi - nic nie wysyłamy
        await driver.stop_belt(verify=False)  # zamykanie: pełny stop
        return True

    def shutdown(self, timeout: float = 4.0) -> None:
        try:
            self._submit(self._stop_session(forget=True)).result(timeout)
        except (concurrent.futures.TimeoutError, RuntimeError):
            log.warning("Bluetooth nie zamknął się na czas")
        self._loop.call_soon_threadsafe(self._loop.stop)
        self._thread.join(timeout)

    # --- wątek bluetooth ----------------------------------------------------

    def _run(self) -> None:
        asyncio.set_event_loop(self._loop)
        self._loop.run_forever()

    def _submit(self, coro: Coroutine[Any, Any, Any]) -> concurrent.futures.Future[Any]:
        return asyncio.run_coroutine_threadsafe(coro, self._loop)

    def _link(self, link: Link, text: str) -> None:
        self.events.put(BleEvent("link", link=link, text=text))

    def _on_reading(self, speed: float, steps: int | None) -> None:
        self.readings.put(Reading(t=time.monotonic(), speed_kmh=speed, steps=steps))

    async def _scan(self) -> None:
        from bleak import BleakScanner

        resume = await self._pause_session()
        self._link(Link.SEARCHING, "Szukam urządzeń Bluetooth…")
        try:
            found = await BleakScanner.discover(timeout=SCAN_S, return_adv=True)
        except Exception as exc:
            link, text = friendly_error(exc)
            self.events.put(BleEvent("scan_failed", link=link, text=text))
            self._link(link, text)
            return
        devices = sorted(
            (
                FoundDevice(
                    address=dev.address,
                    name=adv.local_name or dev.name or "(bez nazwy)",
                    rssi=adv.rssi,
                    treadmill=looks_like_treadmill(adv.local_name or dev.name, adv.service_uuids),
                    services=tuple(adv.service_uuids),
                )
                for dev, adv in found.values()
            ),
            key=lambda d: (not d.treadmill, d.name == "(bez nazwy)", -d.rssi),
        )
        self._rssi = {d.address: d.rssi for d in devices}
        log.info(
            "Skan: %d urządzeń, w tym bieżni: %d", len(devices), sum(d.treadmill for d in devices)
        )
        self.events.put(BleEvent("devices", devices=tuple(devices)))
        if resume:
            await self._start_session(*resume)
        else:
            self._link(Link.IDLE, "Wybierz bieżnię z listy")

    async def _pause_session(self) -> tuple[str, str] | None:
        target = self._target
        await self._stop_session(forget=False)
        return target

    async def _stop_session(self, forget: bool) -> None:
        task, self._session = self._session, None
        if forget:
            self._target = None
        if task is not None:
            task.cancel()
            with contextlib.suppress(asyncio.CancelledError):
                await task
            self._link(Link.IDLE, "Rozłączono")

    async def _start_session(self, address: str, name: str) -> None:
        await self._stop_session(forget=True)
        self._target = (address, name)
        self._session = asyncio.create_task(self._session_loop(address, name))

    async def _start_simulator(self) -> None:
        await self._stop_session(forget=True)
        self._session = asyncio.create_task(self._simulator_loop())

    async def _simulator_loop(self) -> None:
        driver = SimulatorDriver(self._on_reading)
        await self._drive(driver, asyncio.Event())

    async def _drive(self, driver: Driver, lost: asyncio.Event) -> None:
        """Wspólne dla bieżni i symulatora: start sterownika, polecenia, podsumowanie."""
        self._stats = SessionStats()
        self._link(Link.CONNECTED, f"Połączono: {driver.describe()}")
        await driver.start()
        self._caps = driver.capabilities
        self.events.put(BleEvent("device", device=driver.info, caps=driver.capabilities))
        self._link(Link.STREAMING, driver.describe())
        self._commands = asyncio.Queue()
        self._driver = driver
        self._sent_speed = None
        try:
            await self._stream(driver, lost)
        finally:
            self._commands = None
            self._driver = None
            if self._belt_task is not None:
                self._belt_task.cancel()
            self._stats.log_summary(driver.counts, getattr(driver, "speed_stats", None))

    async def _session_loop(self, address: str, name: str) -> None:
        from bleak import BleakClient, BleakScanner

        attempt = 0
        while True:
            try:
                self._link(Link.SEARCHING, f"Szukam {name}…")
                device = await BleakScanner.find_device_by_address(address, timeout=FIND_S)
                if device is None:
                    raise TimeoutError("not found")
                self._link(Link.CONNECTING, f"Łączę z {name}…")
                lost = asyncio.Event()
                loop = asyncio.get_running_loop()

                def on_lost(
                    _client: object, lp: asyncio.AbstractEventLoop = loop, ev: asyncio.Event = lost
                ) -> None:
                    lp.call_soon_threadsafe(ev.set)

                async with BleakClient(
                    device,
                    disconnected_callback=on_lost,
                    timeout=CONNECT_TIMEOUT_S,
                    winrt={"use_cached_services": False},
                ) as client:
                    driver = make_driver(client, name, self._on_reading)
                    log.info(
                        "Połączenie %s: %s, sygnał %s dBm",
                        datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
                        name,
                        self._rssi.get(address, "?"),
                    )
                    attempt = 0
                    await self._drive(driver, lost)
                log.warning("Bieżnia się rozłączyła")
                self._link(Link.RETRYING, "Bieżnia się rozłączyła - łączę ponownie…")
            except asyncio.CancelledError:
                raise
            except Exception as exc:
                link, text = friendly_error(exc)
                if isinstance(exc, TimeoutError) and str(exc) == "not found":
                    link, text = (
                        Link.RETRYING,
                        "Nie widzę bieżni - włącz ją i zamknij aplikację bieżni w telefonie",
                    )
                log.warning("Połączenie: %s (%s)", text, type(exc).__name__)
                self._link(link, text)
                if link is Link.UNSUPPORTED:
                    return
            attempt += 1
            await asyncio.sleep(min(RETRY_MAX_S, 2.0 * attempt))

    async def _stream(self, driver: Driver, lost: asyncio.Event) -> None:
        """Odpytywanie co POLL_S i polecenia start/stop z okna (stop przerywa start)."""
        assert self._commands is not None
        next_poll = 0.0
        while not lost.is_set():
            now = time.monotonic()
            if now >= next_poll:
                next_poll = now + driver.poll_interval()
                await driver.poll()
                self._watch_caps(driver)
                await self._check_speed(driver)
            try:
                command = await asyncio.wait_for(
                    self._commands.get(), max(0.0, next_poll - time.monotonic())
                )
            except TimeoutError:
                continue
            if command == "speed":
                if self._needs_start(driver):
                    self._begin_start(driver)
                elif not self._busy("start"):  # w trakcie startu prędkość ustawi _start_belt
                    await self._apply_speed(driver)
            elif command == "stop":
                if self._belt_task is not None and not self._belt_task.done():
                    self._belt_task.cancel()  # stop przerywa start w toku
                self._sent_speed = None
                self._target_speed = None  # stop kasuje prośby o prędkość i samo-uruchamianie
                self._auto_start = False
                self._belt_task = asyncio.create_task(self._stop_belt(driver))
                self._belt_kind = "stop"
            elif command == "start":
                self._begin_start(driver)

    def _watch_caps(self, driver: Driver) -> None:
        """Bieżnia potrafi zdradzić więcej w trakcie (np. Urevo: kroki w danych)."""
        if driver.capabilities != self._caps:
            self._caps = driver.capabilities
            self.events.put(BleEvent("device", device=driver.info, caps=driver.capabilities))

    def _busy(self, kind: str) -> bool:
        task = self._belt_task
        return task is not None and not task.done() and self._belt_kind == kind

    def _begin_start(self, driver: Driver) -> None:
        if self._busy("start"):
            return  # start już w toku
        if self._belt_task is not None and not self._belt_task.done():
            self._belt_task.cancel()  # sprawdzanie stopu przerwane nowym startem
        self._belt_task = asyncio.create_task(self._start_belt(driver))
        self._belt_kind = "start"

    def _needs_start(self, driver: Driver) -> bool:
        """Pad wychylony, a taśma stoi (albo właśnie ją zatrzymujemy) - trzeba ją uruchomić."""
        if not self._auto_start or not (self._target_speed or 0) > 0 or self._busy("start"):
            return False
        return self._busy("stop") or driver.belt_running() is False

    async def _stop_belt(self, driver: Driver) -> None:
        self._stats.stops += 1
        await driver.stop_belt()
        self.events.put(BleEvent("belt", text="stopped"))

    async def _start_belt(self, driver: Driver) -> None:
        self._stats.starts += 1
        result = await driver.start_belt()
        if result == "started":
            self._stats.starts_ok += 1
        self._sent_speed = None  # po starcie taśma jedzie z prędkością startową bieżni
        self._sent_at = 0.0  # prędkość po starcie od razu, bez odstępu
        if result in ("started", "unknown"):
            await self._apply_speed(driver)
        self.events.put(BleEvent("belt", text=result))

    async def _apply_speed(self, driver: Driver) -> None:
        target = self._target_speed
        if target is None or target == self._sent_speed:
            return
        gap = driver.capabilities.min_command_gap_s
        wait = gap - (time.monotonic() - self._sent_at)
        if wait > 0:  # za szybko po poprzednim - wyślemy najnowszą prędkość za chwilę
            if not self._speed_later:
                self._speed_later = True
                asyncio.get_running_loop().call_later(wait, self._speed_due)
            return
        self._sent_at = time.monotonic()
        if await driver.set_speed(target):
            self._sent_speed, self._speed_failures = target, 0
            return
        self._speed_failures += 1
        if self._speed_failures >= SPEED_ATTEMPTS:  # dalej nie walczymy - nowa prośba spróbuje
            log.warning("Bieżnia nie przyjęła %.1f km/h po %d próbach", target, SPEED_ATTEMPTS)
            self._sent_speed, self._speed_failures = target, 0

    def _speed_due(self) -> None:
        self._speed_later = False
        if self._commands is not None:
            self._commands.put_nowait("speed")

    async def _check_speed(self, driver: Driver) -> None:
        """Co odpytanie: stojąca taśma przy wychylonej gałce rusza, a prędkość, której
        bieżnia nie przyjęła (brak odpowiedzi), idzie jeszcze raz. Czy taśma posłuchała,
        pilnuje już sam sterownik."""
        if self._needs_start(driver):  # np. taśmę zatrzymał pilot albo zgubiony start
            log.info("Taśma stoi, a pad chce %.1f km/h - uruchamiam", self._target_speed or 0)
            self._begin_start(driver)
            return
        if self._belt_task is not None and not self._belt_task.done():
            return
        target = self._target_speed
        if target is not None and target != self._sent_speed and self._sent_at > 0:
            await self._apply_speed(driver)  # poprzednia próba nie dostała potwierdzenia

    async def _diagnose(self, path: Path) -> None:
        from bieznia.ble.diagnostics import run_diagnostics

        resume = await self._pause_session()
        self._link(Link.SEARCHING, "Zbieram raport diagnostyczny (ok. 30 s)…")
        try:
            await run_diagnostics(path, resume[0] if resume else None)
            self.events.put(BleEvent("report", text=str(path)))
        except Exception as exc:
            log.exception("Raport diagnostyczny nie powiódł się")
            self.events.put(BleEvent("report", text=f"Błąd raportu: {exc}"))
        if resume:
            await self._start_session(*resume)
        else:
            self._link(Link.IDLE, "Raport gotowy")
