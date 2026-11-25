"""Moduł KingSmith (WalkingPad R2 / X21 / G1 / X23): szyfrowany protokół tekstowy.

Bieżnia nie potwierdza poleceń - czy posłuchała, widać w ramkach z danymi. Dlatego start
i stop sprawdzamy w danych i ponawiamy, a zadanej prędkości pilnuje SpeedGuard.
"""

from __future__ import annotations

import asyncio
import contextlib
import itertools
import logging
import re
import time
from collections.abc import Iterable
from dataclasses import dataclass, field, replace
from typing import Any

from bieznia.sterowniki.kingsmith import protokol as kingsmith
from bieznia.sterowniki.kontrakt import (
    Capabilities,
    DeviceInfo,
    GattClient,
    ReadingCallback,
)
from bieznia.sterowniki.pilnowanie import SpeedGuard, SpeedStats
from bieznia.sterowniki.profile import ModelProfile

log = logging.getLogger("bieznia.ble")

# UUID KingSmith: 0000fed7-…, 0001fed7-…, 0002fed7-… (zależnie od wersji firmware X21)
_KS_UUID = re.compile(r"^000[0-2](fed7|fed8|1234)-0000-1000-8000-00805f9b34fb$")
_SERVICE_PREFERENCE = ("0002", "0001", "0000")

# Domyślne możliwości modułu - profil modelu (np. kingsmith-x21.json) je uzupełnia.
DEFAULT_CAPS = Capabilities(
    speed_min=kingsmith.SPEED_MIN,
    speed_max=kingsmith.SPEED_MAX,
    speed_step=0.1,
    start_speed=1.0,
    acks_commands=False,
    pause=False,
    steps=True,
    telemetry_s=2.0,
)


# Najdłuższa przerwa między ramkami z prędkością, przy której potwierdzenia odpytywania
# jeszcze podtrzymują ostatnią prędkość (X21 wysyła ją co ok. 2 s).
PROPS_TIMEOUT_S = 6.0


# Odstępy zapisu jak w KS Fit: po każdym fragmencie i po całej wiadomości.
FRAGMENT_GAP_S = 0.03
LINE_GAP_S = 0.06
START_ATTEMPTS = 4
STOP_ATTEMPTS = 3
# Czy bieżnia posłuchała, widać w pierwszej ramce z danymi, która przyszła co najmniej
# VERDICT_FRAME_S po poleceniu (ramki co ok. 2 s, więc najpóźniej po VERDICT_MAX_S).
# Log testera (v0.2.8): pierwszy start bywa gubiony bez śladu - stałe 2,5 s czekania
# przed ponowieniem dawało 5-6 s rozruchu.
VERDICT_FRAME_S = 1.0  # ramka tuż po poleceniu bywa jeszcze sprzed niego (log: do 1,6 s)
VERDICT_MAX_S = 2.6
CONTROL_GAP_S = 0.4
# Gdy bieżnia sama wysyła dane (subskrypcja KS Fit, co ok. 2 s), odpytujemy rzadziej -
# KS Fit nie zasypuje jej zapytaniami, a my gubiliśmy ok. 40% poleceń przy pytaniu co 0,75 s.
CALM_POLL_S = 2.0
# Surowy zapis szesnastkowy tylko na początku połączenia (wykrywanie tablicy szyfru)
# i dla ramek nie do odszyfrowania - reszta to powtórzenie odszyfrowanego tekstu.
RAW_FRAMES_AFTER_CONNECT = 40
BUSY_POLL_S = 0.75  # ControlMode i runState jedno po drugim - drugie potrafi przepaść
# X21 po cichu gubi część poleceń prędkości, najczęściej te wysłane tuż po jego własnej
# ramce z danymi (log testera: 6 z 12 zgubionych przyszło do 0,3 s po ramce). Czekamy chwilę.
QUIET_AFTER_PUSH_S = 0.4
# Kody błędów od producenta (MIoT: -9999..-5000) = odmowa polecenia.
VENDOR_ERRORS = range(-9999, -4999)

# Ile czekamy na prędkość po przejściu na subskrypcję KS Fit, zanim wrócimy do listy z QZ.
TELEMETRY_FALLBACK_S = 5.0
# Co ile sekund zapisać do logu, jak często bieżnia podaje prędkość.
STATS_EVERY_S = 30.0


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


def is_kingsmith_service(uuid: str) -> bool:
    return _short(uuid) == "1234"


def find_channels(services: Iterable[Any]) -> list[Channel]:
    channels: list[Channel] = []
    for service in services:
        chars = {_short(c.uuid): c for c in service.characteristics if _short(c.uuid)}
        if "fed7" in chars and "fed8" in chars:
            channels.append(Channel(service.uuid.lower(), chars["fed7"], chars["fed8"]))

    def rank(ch: Channel) -> int:
        prefix = ch.service_uuid[:4]
        return _SERVICE_PREFERENCE.index(prefix) if prefix in _SERVICE_PREFERENCE else 9

    return sorted(channels, key=rank)


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
        profile: ModelProfile | None = None,
    ) -> None:
        self.client = client
        self.channels = channels
        self.on_reading = on_reading
        self.capabilities = profile.apply(DEFAULT_CAPS) if profile else DEFAULT_CAPS
        self.info = DeviceInfo(
            model=profile.name if profile else (device_name or "KingSmith"),
            driver="kingsmith",
            profile=profile.id if profile else "",
            name=device_name,
        )
        order = kingsmith.table_order(device_name)
        table = profile.option("table") if profile else None
        if table in order:
            order.remove(table)
            order.insert(0, table)
        self.codec = kingsmith.Codec(order)
        self.active: Channel | None = None
        self.last_speed = 0.0
        self.last_steps: int | None = None
        self.last_reply = 0.0
        self.last_props_at = float("-inf")
        self.unanswered_polls = 0
        self.poll_command = kingsmith.GET_PROPS
        self.telemetry_since = 0.0
        self.props_times: list[float] = []
        self.stats_at = 0.0
        self.write_lock = asyncio.Lock()  # całe wiadomości nie mogą się przeplatać
        self.raw_budget = RAW_FRAMES_AFTER_CONNECT
        self.counts = {"polls": 0, "acks": 0, "frames": 0, "undecoded": 0, "commands": 0}
        self.run_state: int | None = None
        self.control_mode: int | None = None
        self.last_error: int | None = None
        self._state_event = asyncio.Event()
        self._reply = asyncio.Event()
        self._belt_busy = 0
        self.guard = SpeedGuard(self._send_speed, self.belt_speed, self.capabilities.clamp)

    @property
    def speed_stats(self) -> SpeedStats:
        return self.guard.stats

    def busy(self) -> bool:
        return self._belt_busy > 0

    def describe(self) -> str:
        table = self.codec.describe()
        channel = self.active.label if self.active else "?"
        return f"{self.info.model} (KingSmith, usługa {channel}, tablica {table})"

    # --- wysyłanie ----------------------------------------------------------

    async def _write(self, channel: Channel, data: bytes) -> None:
        props = getattr(channel.write, "properties", [])
        response = "write-without-response" not in props
        for chunk in kingsmith.chunks(data):
            await self.client.write_gatt_char(channel.write, chunk, response=response)
            await asyncio.sleep(FRAGMENT_GAP_S)
        await asyncio.sleep(LINE_GAP_S)

    async def send(self, text: str) -> None:
        data = self.codec.encode(text)
        async with self.write_lock:
            poll = text.startswith("servers getProp")
            if poll:
                self.counts["polls"] += 1
            else:
                self.counts["commands"] += 1
            if self.raw_budget > 0:
                self.raw_budget -= 1
                log.debug(">> %s  [%s] %s", text or "(pusta)", self.codec.table, data.hex(" "))
            elif not poll:
                log.debug(">> %s", text)
            for channel in [self.active] if self.active else self.channels:
                await self._write(channel, data)

    # --- sterowanie taśmą ---------------------------------------------------

    async def _wait_state(self, timeout: float) -> None:
        self._state_event.clear()
        with contextlib.suppress(TimeoutError):
            await asyncio.wait_for(self._state_event.wait(), timeout)

    async def _quiet(self) -> None:
        """Chwila ciszy po ramce z danymi - polecenia wysłane tuż po niej X21 często gubi."""
        since_push = time.monotonic() - self.last_props_at
        if 0 <= since_push < QUIET_AFTER_PUSH_S:
            await asyncio.sleep(QUIET_AFTER_PUSH_S - since_push)

    async def _verdict(self, sent_at: float, want: int) -> str:
        """'ok' (runState == want), 'error' (odmowa) albo 'no' (świeża ramka bez zmiany)."""
        deadline = sent_at + VERDICT_MAX_S
        while True:
            if self.run_state == want:
                return "ok"
            if self.last_error is not None:
                return "error"
            if self.last_props_at >= sent_at + VERDICT_FRAME_S:
                return "no"
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                return "no"
            await self._wait_state(remaining)

    async def start_belt(self) -> str:
        self._belt_busy += 1
        try:
            return await self._start_belt()
        finally:
            self._belt_busy -= 1

    async def stop_belt(self, verify: bool = True) -> None:
        self.guard.reset()
        self._belt_busy += 1
        try:
            await self._stop_belt(verify)
        finally:
            self._belt_busy -= 1

    async def _start_belt(self) -> str:
        """Uruchamia taśmę. Zwraca 'started', 'refused' albo 'unknown'.

        Bieżnia potrafi odmówić (błąd -5000, np. w trybie czuwania albo bez klucza) albo
        zgubić polecenie bez śladu - ponawiamy, gdy tylko ramka z danymi pokaże, że stoi.
        """
        if self.run_state == 1:
            log.info("Taśma już jedzie - nie wysyłam startu")
            return "started"
        began = time.monotonic()
        for attempt in range(1, START_ATTEMPTS + 1):
            self.last_error = None
            await self._quiet()
            if self.control_mode != 1:  # jak QZ: przejęcie sterowania tylko, gdy potrzebne
                await self.send(kingsmith.CMD_CONTROL)
                await asyncio.sleep(CONTROL_GAP_S)
            await self.send(kingsmith.CMD_START)
            verdict = await self._verdict(time.monotonic(), 1)
            if verdict == "ok":
                log.info("Start: przyjęty po %.1f s (próba %d)", time.monotonic() - began, attempt)
                return "started"
            if verdict == "error":
                log.warning("Bieżnia odmówiła startu (kod %s), próba %d", self.last_error, attempt)
                self.control_mode = None  # następna próba z przejęciem sterowania od nowa
            else:
                log.info("Bieżnia nie zareagowała na start (próba %d) - ponawiam", attempt)
        if self.last_error is not None:
            return "refused"
        return "unknown"  # brak odpowiedzi - prędkość pokaże, czy taśma jedzie

    async def _stop_belt(self, verify: bool) -> None:
        """Stop od razu; potem sprawdzamy w danych i w razie potrzeby ponawiamy."""
        began = time.monotonic()
        for attempt in range(1, STOP_ATTEMPTS + 1):
            self.last_error = None
            await self.send(kingsmith.CMD_STOP)
            if not verify:
                log.info("Wysłano stop taśmy")
                return
            verdict = await self._verdict(time.monotonic(), 0)
            if verdict == "ok":
                log.info("Stop: przyjęty po %.1f s (próba %d)", time.monotonic() - began, attempt)
                return
            log.warning("Bieżnia nie zareagowała na stop (próba %d) - ponawiam", attempt)
            await self._quiet()
        log.warning("Bieżnia nie potwierdziła stopu")

    def belt_running(self) -> bool | None:
        """Czy taśma jedzie (runState z ramek); None, gdy jeszcze nie wiadomo."""
        return None if self.run_state is None else self.run_state == 1

    def belt_speed(self) -> tuple[float, float] | None:
        """Prędkość z ostatniej ramki z danymi i czas tej ramki (time.monotonic)."""
        if self.last_props_at == float("-inf"):
            return None
        return self.last_speed, self.last_props_at

    async def set_speed(self, kmh: float) -> bool:
        """Prędkość docelowa - wysyłana od razu, a w poll() pilnowana (X21 gubi polecenia)."""
        return await self.guard.set(kmh)

    async def _send_speed(self, kmh: float) -> bool:
        command = kingsmith.speed_command(kmh)
        await self._quiet()
        await self.send(command)
        log.info("Prędkość taśmy: %s", command.rsplit(" ", 1)[-1])
        return True

    async def _wait_reply(self, timeout: float) -> bool:
        self._reply.clear()
        try:
            await asyncio.wait_for(self._reply.wait(), timeout)
            return True
        except TimeoutError:
            return False

    # --- odbiór -------------------------------------------------------------

    def _on_notify(self, channel: Channel, _char: Any, data: bytearray) -> None:
        raw = bytes(data)
        if self.raw_budget > 0:
            self.raw_budget -= 1
            log.debug("<< [%s] %s", channel.label, raw.hex(" "))
        for frame in channel.frames.feed(raw):
            decoded = self.codec.decode(frame)
            if decoded is None:
                self.counts["undecoded"] += 1
                log.debug("   (nie da się odszyfrować ramki) %s", frame.hex(" "))
                continue
            self.counts["frames"] += 1
            if decoded.text.startswith("servers"):
                self.counts["acks"] += 1  # odpowiedź na zapytanie - liczona, nie zapisywana
            else:
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
        self._track_state(text)
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
        self._state_event.set()  # każda ramka rozstrzyga, czy polecenie zadziałało
        self.props_times.append(now)
        if "CurrentSpeed" in props:
            self.last_speed = max(0.0, props["CurrentSpeed"])
        if props.get("runState") == 0:  # taśma zatrzymana
            self.last_speed = 0.0
        if "RunningSteps" in props:
            self.last_steps = int(props["RunningSteps"])
        # Każda ramka z danymi = żywa bieżnia, nawet jeśli prędkość się nie zmieniła.
        self.on_reading(self.last_speed, self.last_steps)

    def _track_state(self, text: str) -> None:
        """Stan taśmy i odpowiedzi na polecenia (start/stop, odmowa)."""
        parts = text.split()
        if len(parts) == 2 and parts[0] == "version" and not self.info.firmware:
            self.info = replace(self.info, firmware=parts[1])
        if len(parts) >= 3 and parts[0] == "props" and parts[1] == "Error":
            with contextlib.suppress(ValueError):
                self.last_error = int(parts[-1])
                if self.last_error not in VENDOR_ERRORS:
                    log.info("Bieżnia zgłasza błąd %s (nie odmowa)", self.last_error)
            self._state_event.set()
            return
        props = kingsmith.parse_props(text)
        if "runState" in props:
            self.run_state = int(props["runState"])
            self._state_event.set()
        if "ControlMode" in props:
            self.control_mode = int(props["ControlMode"])
            self._state_event.set()
        if props.get("CurrentSpeed", 0.0) > 0 and self.run_state is None:
            self.run_state = 1

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
        # Próba szybszej telemetrii (jak KS Fit). Gdy prędkość przestanie przychodzić,
        # poll() sam wróci do listy z QZ, która na X21 działa na pewno.
        self.poll_command = kingsmith.TELEMETRY_PROPS
        self.telemetry_since = self.stats_at = time.monotonic()
        await self.send(self.poll_command)

    async def poll(self) -> None:
        """Bieżnia nie wysyła prędkości sama przy stałym tempie - pytamy ją cyklicznie."""
        if self.unanswered_polls >= 3 and not self.codec.locked:
            self.codec.try_next_table()
            log.info("Brak odpowiedzi - próbuję tablicy szyfrowania %s", self.codec.table)
            self.unanswered_polls = 0
        self.unanswered_polls += 1
        now = time.monotonic()
        if (
            self.poll_command == kingsmith.TELEMETRY_PROPS
            and now - self.telemetry_since > TELEMETRY_FALLBACK_S
            and now - self.last_props_at > TELEMETRY_FALLBACK_S
        ):
            log.warning("Subskrypcja KS Fit nie daje prędkości - wracam do odpytywania z QZ")
            self.poll_command = kingsmith.GET_PROPS
        self._log_stats(now)
        await self.send(self.poll_command)
        if not self.busy():
            await self.guard.check()

    def poll_interval(self) -> float:
        pushing = time.monotonic() - self.last_props_at < PROPS_TIMEOUT_S / 2
        if self.poll_command == kingsmith.TELEMETRY_PROPS and pushing:
            return CALM_POLL_S
        return BUSY_POLL_S

    def _log_stats(self, now: float) -> None:
        """Raz na 30 s: jak często naprawdę przychodzi prędkość (do diagnozy opóźnień)."""
        if now - self.stats_at < STATS_EVERY_S:
            return
        self.stats_at = now
        times = [t for t in self.props_times if now - t <= STATS_EVERY_S]
        self.props_times = times
        if len(times) >= 2:
            gaps = sorted(b - a for a, b in itertools.pairwise(times))
            log.info(
                "Telemetria (%s): %d ramek z danymi w %.0f s, typowo co %.2f s, najdłużej %.2f s"
                " · zapytania %d, odpowiedzi %d, nieodszyfrowane %d",
                "KS Fit" if self.poll_command == kingsmith.TELEMETRY_PROPS else "QZ",
                len(times),
                STATS_EVERY_S,
                gaps[len(gaps) // 2],
                gaps[-1],
                self.counts["polls"],
                self.counts["acks"],
                self.counts["undecoded"],
            )
