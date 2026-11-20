"""Bluetooth w osobnym wątku: skanowanie, połączenie z auto-ponawianiem, diagnostyka.

Interfejs (wątek główny) wywołuje metody BleWorker i odbiera zdarzenia z kolejki
`events`, a odczyty prędkości z kolejki `readings`. Cała komunikacja z bleak
odbywa się w jednej pętli asyncio w wątku "bluetooth".
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
from enum import Enum
from pathlib import Path
from typing import Any

from bieznia.ble.drivers import Driver, UnsupportedDevice, looks_like_treadmill, make_driver
from bieznia.sources import Reading

log = logging.getLogger("bieznia.ble")

POLL_S = 0.75
# Polecenie prędkości bez reakcji taśmy wysyłamy ponownie (X21 gubi część poleceń).
SPEED_TOLERANCE = 0.15
SPEED_RESEND_S = 2.5
SPEED_RAMP_S = 8.0  # tyle najwyżej czekamy, gdy taśma już zmienia prędkość
SPEED_RESENDS = 5
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
    kind: str  # "link" | "devices" | "scan_failed" | "report"
    link: Link | None = None
    text: str = ""
    devices: tuple[FoundDevice, ...] = field(default_factory=tuple)


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
        self._target_speed: float | None = None  # ostatnia prośba z okna
        self._sent_speed: float | None = None  # ostatnia wysłana do bieżni
        self._sent_at = 0.0
        self._speed_at_send = 0.0
        self._resends = 0
        self._speed_confirmed = True
        self._thread.start()

    # --- API dla wątku głównego --------------------------------------------

    def scan(self) -> None:
        self._submit(self._scan())

    def connect(self, address: str, name: str) -> None:
        self._submit(self._start_session(address, name))

    def disconnect(self) -> None:
        self._submit(self._stop_session(forget=True))

    def diagnose(self, path: Path) -> None:
        self._submit(self._diagnose(path))

    def belt(self, command: str) -> None:
        """Start ("start") albo stop ("stop") taśmy. Wynik przychodzi jako zdarzenie "belt"."""
        if command not in ("start", "stop"):
            raise ValueError(command)
        self._loop.call_soon_threadsafe(self._queue_command, command)

    def set_speed(self, kmh: float) -> None:
        """Prędkość docelowa taśmy. Liczy się tylko najnowsza - starsze prośby przepadają."""
        self._loop.call_soon_threadsafe(self._queue_speed, round(kmh, 1))

    def _queue_speed(self, kmh: float) -> None:
        self._target_speed = kmh
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
        await driver.stop_belt(verify=False)  # zamykanie: bez czekania na potwierdzenie
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
                    self._link(Link.CONNECTED, f"Połączono: {driver.describe()}")
                    await driver.start()
                    self._link(Link.STREAMING, driver.describe())
                    attempt = 0
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
                log.warning("Bieżnia się rozłączyła")
                self._link(Link.RETRYING, "Bieżnia się rozłączyła - łączę ponownie…")
            except asyncio.CancelledError:
                raise
            except Exception as exc:
                link, text = friendly_error(exc)
                if isinstance(exc, TimeoutError) and str(exc) == "not found":
                    link, text = (
                        Link.RETRYING,
                        "Nie widzę bieżni - włącz ją i rozłącz telefon (KS Fit)",
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
                await self._check_speed(driver)
            try:
                command = await asyncio.wait_for(
                    self._commands.get(), max(0.0, next_poll - time.monotonic())
                )
            except TimeoutError:
                continue
            if command == "speed":
                starting = self._belt_task is not None and not self._belt_task.done()
                if not starting:  # w trakcie startu prędkość ustawi _start_belt
                    await self._apply_speed(driver)
            elif command == "stop":
                if self._belt_task is not None and not self._belt_task.done():
                    self._belt_task.cancel()  # stop przerywa start w toku
                self._sent_speed = None
                self._belt_task = asyncio.create_task(self._stop_belt(driver))
                self._belt_kind = "stop"
            elif command == "start":
                busy = self._belt_task is not None and not self._belt_task.done()
                if busy and self._belt_kind == "start":
                    continue  # start już w toku
                if busy and self._belt_task is not None:
                    self._belt_task.cancel()  # sprawdzanie stopu przerwane nowym startem
                self._belt_task = asyncio.create_task(self._start_belt(driver))
                self._belt_kind = "start"

    async def _stop_belt(self, driver: Driver) -> None:
        await driver.stop_belt()
        self.events.put(BleEvent("belt", text="stopped"))

    async def _start_belt(self, driver: Driver) -> None:
        result = await driver.start_belt()
        self._sent_speed = None  # po starcie taśma jedzie z prędkością startową bieżni
        if result in ("started", "unknown"):
            await self._apply_speed(driver)
        self.events.put(BleEvent("belt", text=result))

    async def _apply_speed(self, driver: Driver) -> None:
        target = self._target_speed
        if target is None or target == self._sent_speed:
            return
        await self._send_speed(driver, target)
        self._resends = 0

    async def _send_speed(self, driver: Driver, target: float) -> None:
        reading = driver.belt_speed()
        if await driver.set_speed(target):
            self._sent_speed = target
            self._sent_at = time.monotonic()
            self._speed_at_send = reading[0] if reading else 0.0
            self._speed_confirmed = False

    async def _check_speed(self, driver: Driver) -> None:
        """Czy taśma posłuchała? Bez reakcji - to samo polecenie jeszcze raz."""
        target = self._sent_speed
        if target is None or self._speed_confirmed or target != self._target_speed:
            return
        if self._belt_task is not None and not self._belt_task.done():
            return
        reading = driver.belt_speed()
        if reading is None:
            return
        speed, at = reading
        if abs(speed - target) <= SPEED_TOLERANCE:
            self._speed_confirmed = True  # od teraz pilot może zmieniać prędkość bez walki
            if self._resends:
                log.info("Taśma przyjęła %.1f km/h po %d ponowieniu/ach", target, self._resends)
            return
        now = time.monotonic()
        if speed <= 0 or at <= self._sent_at or now - self._sent_at < SPEED_RESEND_S:
            return  # taśma stoi albo nie ma jeszcze świeżej ramki
        start = self._speed_at_send
        moving = (speed - start) * (target - start) > 0
        if moving and now - self._sent_at < SPEED_RAMP_S:
            return
        if self._resends >= SPEED_RESENDS:
            log.warning("Taśma nie przyjęła %.1f km/h (jedzie %.1f) - poddaję się", target, speed)
            self._speed_confirmed = True
            return
        self._resends += 1
        log.info(
            "Taśma nie przyjęła %.1f km/h (jedzie %.1f) - ponawiam (%d)",
            target,
            speed,
            self._resends,
        )
        await self._send_speed(driver, target)

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
