"""Moduł FTMS (standard Bluetooth Fitness Machine Service), np. Urevo SpaceWalk 5L.

Dane co ok. 1 s (Treadmill Data), stan maszyny (Machine Status), polecenia przez Control
Point - każde z odpowiedzią. Stop z okna to pauza (bieżnia trzyma sesję), przy zamykaniu
aplikacji - pełny stop. Dziwactwa modeli (zakres, odliczanie) opisują profile JSON.
"""

from __future__ import annotations

import asyncio
import contextlib
import logging
import time
from collections.abc import Iterable
from dataclasses import replace
from typing import Any

from bieznia.sterowniki.ftms import protokol as ftms
from bieznia.sterowniki.kontrakt import Capabilities, DeviceInfo, GattClient, ReadingCallback
from bieznia.sterowniki.pilnowanie import SpeedStats
from bieznia.sterowniki.profile import ModelProfile

log = logging.getLogger("bieznia.ble")

# FTMS: każde polecenie dostaje odpowiedź - czekamy na nią tyle, potem uznajemy za zgubione
FTMS_RESPONSE_S = 1.5
FTMS_START_WAIT_S = 4.0  # Urevo: po Start taśma rusza po ok. 1-3 s (odliczanie 3-2-1)
FTMS_START_ATTEMPTS = 2  # Urevo: pierwszy Start po Stopie bywa przyjęty, a taśma nie rusza
# Stop: po przyjętej pauzie taśma hamuje kilka sekund i dopiero wtedy bieżnia zgłasza
# „zatrzymana” (log 5L: 2,3-7 s). Ponawiamy (pełnym stopem) tylko, gdy polecenie przepadło
# albo taśma w tym czasie wcale nie zwalnia.
FTMS_STOP_WAIT_S = 9.0  # najdłużej na zgłoszenie „zatrzymana” po przyjętym poleceniu
FTMS_STOP_SLOWING_S = 2.5  # tyle, żeby było widać hamowanie w ramkach (co ok. 1 s)
FTMS_STOP_ATTEMPTS = 3
SPEED_ATTEMPTS = 2  # słaby sygnał: polecenie albo odpowiedź potrafi przepaść
RAW_FRAMES_AFTER_CONNECT = 40

DEVICE_INFO = "0000180a-0000-1000-8000-00805f9b34fb"
FIRMWARE_REVISION = "00002a26-0000-1000-8000-00805f9b34fb"

DEFAULT_CAPS = Capabilities(
    speed_min=1.0,
    speed_max=6.0,
    speed_step=0.1,
    start_speed=1.0,
    acks_commands=True,
    pause=True,
    steps=False,  # standard nie ma kroków - Urevo podaje je w polu spoza FTMS 1.0
    telemetry_s=1.0,
    min_command_gap_s=0.8,
)


def find_char(services: Iterable[Any], uuid: str) -> Any | None:
    for service in services:
        for char in service.characteristics:
            if char.uuid.lower() == uuid:
                return char
    return None


def has_ftms(services: Iterable[Any]) -> bool:
    return find_char(services, ftms.TREADMILL_DATA) is not None


class FtmsDriver:
    protocol = "FTMS"

    def __init__(
        self,
        client: GattClient,
        services: Iterable[Any],
        device_name: str,
        on_reading: ReadingCallback,
        profile: ModelProfile | None = None,
    ) -> None:
        services = list(services)
        self.client = client
        self.on_reading = on_reading
        self.profile = profile
        self.capabilities = profile.apply(DEFAULT_CAPS) if profile else DEFAULT_CAPS
        self.info = DeviceInfo(
            model=profile.name if profile else (device_name or "Bieżnia FTMS"),
            driver="ftms",
            profile=profile.id if profile else "",
            name=device_name,
        )
        self.data_char = find_char(services, ftms.TREADMILL_DATA)
        self.cp_char = find_char(services, ftms.CONTROL_POINT)
        self.status_char = find_char(services, ftms.MACHINE_STATUS)
        self.range_char = find_char(services, ftms.SPEED_RANGE)
        self.firmware_char = find_char(services, FIRMWARE_REVISION)
        if self.cp_char is None:
            self.capabilities = replace(self.capabilities, control=False)
        self.last_speed: tuple[float, float] | None = None
        self.running: bool | None = None
        self.has_control = False
        self.speed_stats = SpeedStats()
        self._pending: asyncio.Future[int] | None = None
        self._pending_op = -1
        self._lock = asyncio.Lock()
        self._started = asyncio.Event()
        self._stopped = asyncio.Event()
        self._belt_busy = 0
        self.counts = {"commands": 0, "acks": 0, "refused": 0, "frames": 0, "undecoded": 0}
        self.raw_budget = RAW_FRAMES_AFTER_CONNECT

    @property
    def speed_min(self) -> float:
        return self.capabilities.speed_min

    @property
    def speed_max(self) -> float:
        return self.capabilities.speed_max

    def describe(self) -> str:
        return f"{self.info.model} (FTMS)"

    async def start(self) -> None:
        read = getattr(self.client, "read_gatt_char", None)
        if read is not None and self.range_char is not None:
            with contextlib.suppress(Exception):
                found = ftms.decode_speed_range(bytes(await read(self.range_char)))
                if found:
                    self.capabilities = self.capabilities.with_range(*found)
        if read is not None and self.firmware_char is not None:
            with contextlib.suppress(Exception):
                raw = bytes(await read(self.firmware_char))
                firmware = raw.decode("ascii", "replace").strip("\x00 ").strip()
                if firmware:
                    self.info = replace(self.info, firmware=firmware)
        caps = self.capabilities
        log.info(
            "FTMS: %s%s, prędkość %.1f-%.1f km/h co %.1f",
            self.info.model,
            f" (firmware {self.info.firmware})" if self.info.firmware else "",
            caps.speed_min,
            caps.speed_max,
            caps.speed_step,
        )
        if self.cp_char is not None:
            await self.client.start_notify(self.cp_char, self._on_response)
        if self.status_char is not None:
            await self.client.start_notify(self.status_char, self._on_status)
        await self.client.start_notify(self.data_char, self._on_data)

    # --- powiadomienia -------------------------------------------------------------------

    def _on_data(self, _char: Any, data: bytearray) -> None:
        raw = bytes(data)
        if self.raw_budget > 0:
            self.raw_budget -= 1
            log.debug("<< FTMS %s", raw.hex(" "))
        decoded = ftms.decode_treadmill_data(raw)
        if decoded is None or decoded.speed_kmh is None:
            self.counts["undecoded"] += 1
            log.debug("<< FTMS (nie do odczytania) %s", raw.hex(" "))
            return
        self.counts["frames"] += 1
        if decoded.steps is not None and not self.capabilities.steps:
            self.capabilities = replace(self.capabilities, steps=True)
        speed = decoded.speed_kmh
        self.last_speed = (speed, time.monotonic())
        if speed > 0:
            self.running = True
            self._started.set()
        elif self.running is None:
            self.running = False
        self.on_reading(speed, decoded.steps)

    def _on_status(self, _char: Any, data: bytearray) -> None:
        raw = bytes(data)
        if not raw:
            return
        code = raw[0]
        log.info("Stan bieżni: %s (%s)", ftms.STATUS_NAMES.get(code, f"kod {code}"), raw.hex(" "))
        if code == ftms.STATUS_STARTED:
            self.running = True
            self._started.set()
        elif code in (ftms.STATUS_STOPPED, ftms.STATUS_SAFETY_KEY, ftms.STATUS_RESET):
            self.running = False
            self._stopped.set()
            if code == ftms.STATUS_RESET:
                self.has_control = False

    def _on_response(self, _char: Any, data: bytearray) -> None:
        parsed = ftms.parse_response(bytes(data))
        if parsed is None:
            return
        op, result = parsed
        pending = self._pending
        if pending is not None and not pending.done() and op == self._pending_op:
            pending.set_result(result)

    # --- polecenia -----------------------------------------------------------------------

    async def _send(self, payload: bytes, what: str) -> int | None:
        """Polecenie i jego wynik (1 = przyjęte) albo None, gdy bieżnia nie odpowiedziała."""
        if self.cp_char is None:
            return None
        async with self._lock:
            loop = asyncio.get_running_loop()
            pending: asyncio.Future[int] = loop.create_future()
            self._pending = pending
            self._pending_op = payload[0]
            self.counts["commands"] += 1
            log.debug(">> FTMS %s (%s)", payload.hex(" "), what)
            try:
                await self.client.write_gatt_char(self.cp_char, payload, response=True)
                result = await asyncio.wait_for(pending, FTMS_RESPONSE_S)
            except TimeoutError:
                log.warning("FTMS: brak odpowiedzi na %s", what)
                return None
            finally:
                self._pending = None
        if result == ftms.RESULT_OK:
            self.counts["acks"] += 1
        else:
            self.counts["refused"] += 1
            log.warning("FTMS: %s - %s", what, ftms.RESULTS.get(result, f"wynik {result}"))
        return result

    async def _take_control(self) -> None:
        got = await self._send(ftms.encode_request_control(), "przejęcie kontroli")
        self.has_control = got == ftms.RESULT_OK

    async def _command(self, payload: bytes, what: str) -> int | None:
        """Polecenie z przejęciem kontroli, gdy bieżnia jej jeszcze nie oddała."""
        if not self.has_control:
            await self._take_control()
        result = await self._send(payload, what)
        if result == ftms.RESULT_NOT_PERMITTED:
            await self._take_control()
            if self.has_control:
                result = await self._send(payload, what)
        return result

    async def poll(self) -> None:
        pass  # bieżnia sama wysyła dane co ok. 1 s

    def poll_interval(self) -> float:
        return 1.0

    def busy(self) -> bool:
        return self._belt_busy > 0

    def belt_running(self) -> bool | None:
        return self.running

    def belt_speed(self) -> tuple[float, float] | None:
        return self.last_speed

    def reachable_speed(self, kmh: float) -> float:
        """Prędkość, którą bieżnia naprawdę pojedzie (zakres zgłoszony przez bieżnię)."""
        caps = self.capabilities
        return round(min(caps.speed_max, max(caps.speed_min, kmh)), 1)

    async def start_belt(self) -> str:
        if self.running:
            return "started"
        self._belt_busy += 1
        try:
            return await self._start_belt()
        finally:
            self._belt_busy -= 1

    async def _start_belt(self) -> str:
        sent = time.monotonic()
        for attempt in range(1, FTMS_START_ATTEMPTS + 1):
            self._started.clear()
            result = await self._command(ftms.encode_start(), "start")
            if result is None:
                continue
            if result != ftms.RESULT_OK:
                return "refused"
            with contextlib.suppress(TimeoutError):
                await asyncio.wait_for(self._started.wait(), FTMS_START_WAIT_S)
            if self.running:
                log.info("Start: przyjęty po %.1f s (próba %d)", time.monotonic() - sent, attempt)
                return "started"
            log.info("Start przyjęty, ale taśma nie ruszyła - ponawiam")
        return "unknown"

    async def stop_belt(self, verify: bool = True) -> None:
        """Stop z okna = pauza (szybki powrót), zamykanie (verify=False) = pełny stop.

        Bezpieczeństwo przede wszystkim: dopóki bieżnia nie zgłosi, że stoi, ponawiamy -
        kolejne próby to już pełny stop (Urevo potrafi zgubić polecenie albo odpowiedź).
        """
        self._belt_busy += 1
        try:
            await self._stop_belt(verify)
        finally:
            self._belt_busy -= 1

    async def _stop_belt(self, verify: bool) -> None:
        sent = time.monotonic()
        for attempt in range(1, FTMS_STOP_ATTEMPTS + 1):
            self._stopped.clear()
            pause = verify and attempt == 1 and self.capabilities.pause
            payload = ftms.encode_pause() if pause else ftms.encode_stop()
            before = self.last_speed[0] if self.last_speed else 0.0
            result = await self._command(payload, "pauza" if pause else "stop")
            if result == ftms.RESULT_OK and await self._stopping(before):
                log.info("Stop: przyjęty po %.1f s (próba %d)", time.monotonic() - sent, attempt)
                return
            log.warning("Stop bez potwierdzenia bieżni - ponawiam pełnym stopem (%d)", attempt)
        log.error("Bieżnia nie potwierdziła stopu - zatrzymaj ją pilotem albo kluczem")

    async def _stopping(self, before: float) -> bool:
        """Po przyjętym stopie: czekamy na „zatrzymana”, dopóki taśma zwalnia."""
        start = time.monotonic()
        while time.monotonic() - start < FTMS_STOP_WAIT_S:
            with contextlib.suppress(TimeoutError):
                await asyncio.wait_for(self._stopped.wait(), 0.25)
            if self._stopped.is_set() or self.running is False:
                return True
            if time.monotonic() - start >= FTMS_STOP_SLOWING_S:
                speed = self.last_speed[0] if self.last_speed else 0.0
                if speed <= 0.0:
                    self.running = False  # taśma stoi - zgłoszenie stanu przyjdzie za chwilę
                    return True
                if speed >= before - 0.05:
                    return False  # przyjęte, a taśma nie zwalnia - ponawiamy
        return False

    async def set_speed(self, kmh: float) -> bool:
        """Prędkość w zakresie zgłoszonym przez bieżnię - poza nim Urevo w ogóle nie
        odpowiada (log: 0,5 i 0,6 km/h bez odpowiedzi), więc niżej niż minimum nie wysyłamy.
        Polecenie bez odpowiedzi (słaby sygnał) wysyłamy jeszcze raz."""
        target = self.reachable_speed(kmh)
        if target != round(kmh, 1):
            log.debug("%.1f km/h poza zakresem bieżni - wysyłam %.1f", kmh, target)
        self.speed_stats.sent += 1
        began = time.monotonic()
        for attempt in range(SPEED_ATTEMPTS):
            if attempt:
                self.speed_stats.resends += 1
            result = await self._command(ftms.encode_set_speed(target), f"prędkość {target:.1f}")
            if result is None:
                continue
            if result == ftms.RESULT_OK:
                self.speed_stats.ok += 1
                self.speed_stats.latencies.append(time.monotonic() - began)
                return True
            break
        self.speed_stats.failed += 1
        return False
