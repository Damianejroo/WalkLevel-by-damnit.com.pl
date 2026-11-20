"""Sterowniki bieżni: wybór protokołu po usługach GATT i odczyt prędkości.

Tylko ODCZYT: żaden sterownik nie wysyła poleceń zmiany prędkości ani startu taśmy.
"""

from __future__ import annotations

import asyncio
import logging
import re
import time
from collections.abc import Callable, Iterable
from dataclasses import dataclass, field
from typing import Any, Protocol

from bieznia.ble import ftms, kingsmith

log = logging.getLogger("bieznia.ble")

ReadingCallback = Callable[[float, int | None], None]  # (km/h, kroki albo None)

# UUID KingSmith: 0000fed7-…, 0001fed7-…, 0002fed7-… (zależnie od wersji firmware X21)
_KS_UUID = re.compile(r"^000[0-2](fed7|fed8|1234)-0000-1000-8000-00805f9b34fb$")
_SERVICE_PREFERENCE = ("0002", "0001", "0000")


# Najdłuższa przerwa między ramkami z prędkością, przy której potwierdzenia odpytywania
# jeszcze podtrzymują ostatnią prędkość (X21 wysyła ją co ok. 2 s).
PROPS_TIMEOUT_S = 6.0


class UnsupportedDevice(Exception):
    """Urządzenie nie ma żadnej znanej usługi bieżni."""


class GattClient(Protocol):
    """Podzbiór BleakClient, którego używamy (ułatwia testy na wirtualnej bieżni)."""

    @property
    def services(self) -> Iterable[Any]: ...

    async def start_notify(self, char_specifier: Any, callback: Any) -> None: ...

    async def write_gatt_char(
        self, char_specifier: Any, data: bytes, response: bool | None = None
    ) -> None: ...


def _short(uuid: str) -> str | None:
    match = _KS_UUID.match(uuid.lower())
    return match.group(1) if match else None


@dataclass
class Channel:
    service_uuid: str
    write: Any  # BleakGATTCharacteristic
    notify: Any
    frames: kingsmith.FrameBuffer = field(default_factory=kingsmith.FrameBuffer)

    @property
    def label(self) -> str:
        return self.service_uuid[:8]


def find_kingsmith_channels(services: Iterable[Any]) -> list[Channel]:
    channels: list[Channel] = []
    for service in services:
        chars = {_short(c.uuid): c for c in service.characteristics if _short(c.uuid)}
        if "fed7" in chars and "fed8" in chars:
            channels.append(Channel(service.uuid.lower(), chars["fed7"], chars["fed8"]))

    def rank(ch: Channel) -> int:
        prefix = ch.service_uuid[:4]
        return _SERVICE_PREFERENCE.index(prefix) if prefix in _SERVICE_PREFERENCE else 9

    return sorted(channels, key=rank)


def find_ftms_char(services: Iterable[Any]) -> Any | None:
    for service in services:
        for char in service.characteristics:
            if char.uuid.lower() == ftms.TREADMILL_DATA:
                return char
    return None


class Driver(Protocol):
    protocol: str

    async def start(self) -> None: ...

    async def poll(self) -> None: ...

    def describe(self) -> str: ...


class KingSmithDriver:
    """WalkingPad R2 / X21 / G1 / X23 - szyfrowany protokół tekstowy (patrz kingsmith.py)."""

    protocol = "KingSmith"
    REPLY_WAIT_S = 0.6

    def __init__(
        self,
        client: GattClient,
        channels: list[Channel],
        device_name: str,
        on_reading: ReadingCallback,
    ) -> None:
        self.client = client
        self.channels = channels
        self.on_reading = on_reading
        self.codec = kingsmith.Codec(kingsmith.table_order(device_name))
        self.active: Channel | None = None
        self.last_speed = 0.0
        self.last_steps: int | None = None
        self.last_reply = 0.0
        self.last_props_at = float("-inf")
        self.unanswered_polls = 0
        self._reply = asyncio.Event()

    def describe(self) -> str:
        table = self.codec.describe()
        channel = self.active.label if self.active else "?"
        return f"KingSmith (usługa {channel}, tablica {table})"

    # --- wysyłanie ----------------------------------------------------------

    async def _write(self, channel: Channel, data: bytes) -> None:
        props = getattr(channel.write, "properties", [])
        response = "write-without-response" not in props
        for chunk in kingsmith.chunks(data):
            await self.client.write_gatt_char(channel.write, chunk, response=response)

    async def send(self, text: str) -> None:
        data = self.codec.encode(text)
        log.debug(">> %s  [%s] %s", text or "(pusta)", self.codec.table, data.hex(" "))
        for channel in [self.active] if self.active else self.channels:
            await self._write(channel, data)

    async def _wait_reply(self, timeout: float) -> bool:
        self._reply.clear()
        try:
            await asyncio.wait_for(self._reply.wait(), timeout)
            return True
        except TimeoutError:
            return False

    # --- odbiór -------------------------------------------------------------

    def _on_notify(self, channel: Channel, _char: Any, data: bytearray) -> None:
        log.debug("<< [%s] %s", channel.label, bytes(data).hex(" "))
        for frame in channel.frames.feed(bytes(data)):
            decoded = self.codec.decode(frame)
            if decoded is None:
                log.debug("   (nie da się odszyfrować ramki)")
                continue
            log.debug("   %s%s", decoded.text, "" if decoded.trusted else "  (niepewne)")
            if self.active is None:
                self.active = channel
                log.info("Bieżnia odpowiada: %s", self.describe())
            self.last_reply = time.monotonic()
            self.unanswered_polls = 0
            self._reply.set()
            if decoded.trusted:  # niepewny tekst mógłby mieć przekłamaną cyfrę prędkości
                self._handle(decoded.text)

    def _handle(self, text: str) -> None:
        props = kingsmith.parse_props(text)
        now = time.monotonic()
        if not props:
            # X21 wysyła prędkość sam co ok. 2 s, a na nasze odpytywanie (co 0,75 s)
            # odpowiada krótkim "servers 0". To potwierdzenie = bieżnia żyje, prędkość
            # bez zmian. Bez świeżej ramki z prędkością (PROPS_TIMEOUT_S) nic nie wysyłamy -
            # wtedy bezpiecznik w silniku zatrzyma postać.
            if text.startswith("servers") and now - self.last_props_at < PROPS_TIMEOUT_S:
                self.on_reading(self.last_speed, self.last_steps)
            return
        self.last_props_at = now
        if "CurrentSpeed" in props:
            self.last_speed = max(0.0, props["CurrentSpeed"])
        if props.get("runState") == 0:  # taśma zatrzymana
            self.last_speed = 0.0
        if "RunningSteps" in props:
            self.last_steps = int(props["RunningSteps"])
        # Każda ramka z danymi = żywa bieżnia, nawet jeśli prędkość się nie zmieniła.
        self.on_reading(self.last_speed, self.last_steps)

    # --- cykl życia ---------------------------------------------------------

    async def start(self) -> None:
        for channel in self.channels:
            await self.client.start_notify(
                channel.notify,
                lambda char, data, ch=channel: self._on_notify(ch, char, data),
            )
        # Pusta wiadomość nie zależy od tablicy - odpowiedź ("format error")
        # od razu mówi, którą tablicą szyfruje ta bieżnia.
        await self.send("")
        await self._wait_reply(1.5)
        now = int(time.time())
        for command in kingsmith.INIT_COMMANDS:
            await self.send(command.format(now=now))
            await self._wait_reply(self.REPLY_WAIT_S)
        # Odczyt zawiera znaki, którymi tablice się różnią - jeśli bieżnia milczy,
        # od razu próbuj kolejnej z pozostałych tablic.
        for _ in range(len(self.codec.candidates)):
            await self.send(kingsmith.GET_PROPS)
            if await self._wait_reply(self.REPLY_WAIT_S * 2):
                break
            self.codec.try_next_table()

    async def poll(self) -> None:
        """Bieżnia nie wysyła prędkości sama przy stałym tempie - pytamy ją cyklicznie."""
        if self.unanswered_polls >= 3 and not self.codec.locked:
            self.codec.try_next_table()
            log.info("Brak odpowiedzi - próbuję tablicy szyfrowania %s", self.codec.table)
            self.unanswered_polls = 0
        self.unanswered_polls += 1
        await self.send(kingsmith.GET_PROPS)


class FtmsDriver:
    """Standard FTMS: bieżnia sama wysyła Treadmill Data co ok. sekundę."""

    protocol = "FTMS"

    def __init__(self, client: GattClient, char: Any, on_reading: ReadingCallback) -> None:
        self.client = client
        self.char = char
        self.on_reading = on_reading

    def describe(self) -> str:
        return "FTMS (standard Bluetooth)"

    def _on_notify(self, _char: Any, data: bytearray) -> None:
        log.debug("<< FTMS %s", bytes(data).hex(" "))
        speed = ftms.parse_treadmill_data(bytes(data))
        if speed is not None:
            self.on_reading(speed, None)

    async def start(self) -> None:
        await self.client.start_notify(self.char, self._on_notify)

    async def poll(self) -> None:
        pass


def make_driver(client: GattClient, device_name: str, on_reading: ReadingCallback) -> Driver:
    services = list(client.services)
    channels = find_kingsmith_channels(services)
    if channels:
        return KingSmithDriver(client, channels, device_name, on_reading)
    char = find_ftms_char(services)
    if char is not None:
        return FtmsDriver(client, char, on_reading)
    raise UnsupportedDevice(
        "To urządzenie nie wygląda na bieżnię (brak usługi KingSmith ani FTMS). "
        "Zapisz raport diagnostyczny i wyślij go."
    )


def looks_like_treadmill(name: str | None, service_uuids: Iterable[str] = ()) -> bool:
    """Do sortowania wyników skanowania: KingSmith/WalkingPad albo usługa FTMS."""
    upper = (name or "").upper()
    if upper.startswith(("KS-", "WALKINGPAD")):
        return True
    return any(u.lower() == ftms.FTMS_SERVICE or _short(u) == "1234" for u in service_uuids)
