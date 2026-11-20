"""Protokół KingSmith i sterowniki - na wirtualnej bieżni zachowującej się jak X21 w QZ."""

from __future__ import annotations

import asyncio
import itertools
import queue
from dataclasses import dataclass, field
from types import SimpleNamespace
from typing import Any

import pytest

from bieznia.ble import drivers, ftms, kingsmith
from bieznia.ble.drivers import (
    FtmsDriver,
    KingSmithDriver,
    UnsupportedDevice,
    looks_like_treadmill,
    make_driver,
)

# --- kodowanie --------------------------------------------------------------


@pytest.mark.parametrize("name", list(kingsmith.TABLES))
def test_tables_are_permutations_of_base64_alphabet(name: str) -> None:
    assert sorted(kingsmith.TABLES[name]) == sorted(kingsmith.PLAINTEXT)


@pytest.mark.parametrize("name", list(kingsmith.TABLES))
def test_encode_decode_round_trip(name: str) -> None:
    text = "props CurrentSpeed 3.5 spm 98"
    data = kingsmith.encode(text, name)
    assert data.endswith(b"\r")
    assert kingsmith.decode(data[:-1], name) == text


def test_known_vector_matches_qz_algorithm() -> None:
    # "shake" -> base64 "c2hha2U=" -> podstawienie tablicą v1 (indeks w PLAINTEXT_TABLE)
    expected = bytes(kingsmith.TABLES["v1"][kingsmith.PLAINTEXT.index(c)] for c in b"c2hha2U=")
    assert kingsmith.encode("shake", "v1") == expected + b"\r"


REPLIES = [
    "format error",
    "shake 00",
    "props CurrentSpeed 3.5 RunningSteps 120 runState 1",
    "props ControlMode 1 spm 0",
    "version 0014",
]


@pytest.mark.parametrize("name", list(kingsmith.TABLES))
def test_codec_narrows_to_the_right_table_and_never_trusts_wrong_numbers(name: str) -> None:
    codec = kingsmith.Codec()
    for reply in REPLIES:
        decoded = codec.decode(kingsmith.encode(reply, name)[:-1])
        assert decoded is not None
        if decoded.trusted:
            assert decoded.text == reply  # pewny tekst zawsze jest prawdziwy
        assert name in codec.candidates  # właściwa tablica nigdy nie odpada


def test_wrong_table_artifacts_are_rejected() -> None:
    assert not kingsmith.is_known_reply("props ;urrentSpeed 3.5")
    assert not kingsmith.is_known_reply("shake 0?")
    assert kingsmith.is_known_reply("props CurrentSpeed 3.5")


def test_frame_buffer_joins_chunks() -> None:
    fb = kingsmith.FrameBuffer()
    msg = kingsmith.encode("props CurrentSpeed 4.5 RunningSteps 1200", "v1")
    parts = list(kingsmith.chunks(msg))
    assert len(parts) > 1
    frames: list[bytes] = []
    for part in parts:
        frames += fb.feed(part)
    assert frames == [msg[:-1]]


def test_frame_buffer_drops_garbage() -> None:
    fb = kingsmith.FrameBuffer()
    assert fb.feed(b"x" * (kingsmith.MAX_BUFFER + 1)) == []
    assert fb.feed(b"ab\r") == [b"ab"]


def test_parse_props_like_qz() -> None:
    text = "props CurrentSpeed 3.5 mcu_version 1.2.3 spm 98 goal x Error -5000"
    assert kingsmith.parse_props(text) == {"CurrentSpeed": 3.5, "spm": 98.0}
    assert kingsmith.parse_props("shake 00") == {}


def test_x23_prefers_table_v7() -> None:
    assert kingsmith.table_order("KS-NACH-MXG-1234")[0] == "v7"
    assert kingsmith.table_order("KS-HDSY-X21C")[0] == "v1"


def test_ftms_speed() -> None:
    assert ftms.parse_treadmill_data(bytes([0x00, 0x00, 0x5E, 0x01])) == 3.5
    assert ftms.parse_treadmill_data(bytes([0x01, 0x00, 0x5E, 0x01])) is None
    assert ftms.parse_treadmill_data(b"\x00") is None


def test_treadmill_names() -> None:
    assert looks_like_treadmill("KS-HDSY-X21C-ABCD")
    assert looks_like_treadmill(None, ["00001826-0000-1000-8000-00805f9b34fb"])
    assert not looks_like_treadmill("JBL Flip 5")


# --- wirtualna bieżnia ------------------------------------------------------

BASE = "-0000-1000-8000-00805f9b34fb"


@dataclass(eq=False)
class FakeChar:
    uuid: str
    properties: list[str]


@dataclass(eq=False)
class FakeService:
    uuid: str
    characteristics: list[FakeChar]


@dataclass
class FakeTreadmill:
    """Odszyfrowuje polecenia i odpowiada jak X21 (wg sterownika QZ)."""

    table: str = "v1"
    prefix: str = "0002"
    speed: float = 3.5
    answer_empty: bool = True
    extra_silent_channel: bool = False
    running: bool = True
    refuse_start: bool = False
    callbacks: dict[str, Any] = field(default_factory=dict)
    received: list[str] = field(default_factory=list)
    _buf: dict[str, bytearray] = field(default_factory=dict)

    def __post_init__(self) -> None:
        def service(prefix: str) -> FakeService:
            return FakeService(
                f"{prefix}1234{BASE}",
                [
                    FakeChar(f"{prefix}fed7{BASE}", ["write-without-response", "write"]),
                    FakeChar(f"{prefix}fed8{BASE}", ["notify"]),
                ],
            )

        self._services = [service(self.prefix)]
        if self.extra_silent_channel:
            self._services.insert(0, service("0000"))

    @property
    def services(self) -> list[FakeService]:
        return self._services

    async def start_notify(self, char: FakeChar, callback: Any) -> None:
        self.callbacks[char.uuid[:4]] = callback

    async def write_gatt_char(
        self, char: FakeChar, data: bytes, response: bool | None = None
    ) -> None:
        prefix = char.uuid[:4]
        if prefix != self.prefix:
            return  # ten kanał nie należy do bieżni
        buf = self._buf.setdefault(prefix, bytearray())
        buf += data
        while b"\r" in buf:
            frame, _, rest = bytes(buf).partition(b"\r")
            self._buf[prefix] = buf = bytearray(rest)
            self._on_command(prefix, frame)

    def _on_command(self, prefix: str, frame: bytes) -> None:
        if not frame:
            if self.answer_empty:
                self._reply(prefix, "format error")
            return
        text = kingsmith.decode(frame, self.table)
        if text is None:
            return  # zła tablica - prawdziwa bieżnia milczy
        self.received.append(text)
        word = text.split()[0]
        if word == "servers":
            speed = self.speed if self.running else 0.0
            state = int(self.running)
            self._reply(prefix, f"props CurrentSpeed {speed} RunningSteps 120 runState {state}")
        elif text == kingsmith.CMD_CONTROL:
            self._reply(prefix, "props ControlMode 1")
        elif text == kingsmith.CMD_START:
            if self.refuse_start:
                self._reply(prefix, "props Error ErrorCode -5000")
            else:
                self.running = True
                self._reply(prefix, "props runState 1")
        elif text == kingsmith.CMD_STOP:
            self.running = False
            self._reply(prefix, "props runState 0")
        else:
            self._reply(prefix, f"{word} ok")

    def _reply(self, prefix: str, text: str) -> None:
        for chunk in kingsmith.chunks(kingsmith.encode(text, self.table)):
            asyncio.get_running_loop().call_soon(self.callbacks[prefix], None, bytearray(chunk))


async def run_driver(treadmill: FakeTreadmill, polls: int = 1) -> tuple[Any, list[float]]:
    readings: list[float] = []
    driver = make_driver(treadmill, "KS-HDSY-X21C", lambda s, _st: readings.append(s))
    driver.REPLY_WAIT_S = 0.05  # type: ignore[attr-defined]
    await driver.start()
    for _ in range(polls):
        await driver.poll()
        await asyncio.sleep(0.01)
    return driver, readings


def test_x21_default_table_streams_speed() -> None:
    treadmill = FakeTreadmill()
    driver, readings = asyncio.run(run_driver(treadmill))
    assert isinstance(driver, KingSmithDriver)
    assert readings and readings[-1] == 3.5
    assert "v1" in driver.codec.candidates
    assert treadmill.received[:2] == ["shake", "net"]
    assert "servers getProp 1 2 7 12 23 24 31" in treadmill.received


@pytest.mark.parametrize("table", ["v2", "v5", "v7"])
def test_other_tables_are_detected_from_empty_message(table: str) -> None:
    treadmill = FakeTreadmill(table=table)
    driver, readings = asyncio.run(run_driver(treadmill))
    assert table in driver.codec.candidates
    assert readings and readings[-1] == 3.5
    assert any(t.startswith("servers getProp") for t in treadmill.received)


def test_silent_treadmill_is_found_by_trying_tables() -> None:
    # Bieżnia ignoruje pustą wiadomość - tablicę znajdujemy kolejnymi próbami odczytu.
    treadmill = FakeTreadmill(table="v3", answer_empty=False)
    driver, readings = asyncio.run(run_driver(treadmill, 25))
    assert "v3" in driver.codec.candidates
    assert readings and readings[-1] == 3.5


def test_driver_picks_the_channel_that_answers() -> None:
    treadmill = FakeTreadmill(prefix="0002", extra_silent_channel=True)
    driver, readings = asyncio.run(run_driver(treadmill))
    assert driver.active is not None and driver.active.label.startswith("0002")
    assert readings[-1] == 3.5


def test_stopped_belt_reports_zero() -> None:
    driver = KingSmithDriver(FakeTreadmill(), [], "KS-X21", lambda s, _st: None)
    driver._handle("props CurrentSpeed 4.0")
    driver._handle("props runState 0")
    assert driver.last_speed == 0.0


def test_ftms_and_unknown_devices() -> None:
    class FtmsOnly:
        services = [FakeService(ftms.FTMS_SERVICE, [FakeChar(ftms.TREADMILL_DATA, ["notify"])])]

    class Headphones:
        services = [FakeService(f"0000180f{BASE}", [FakeChar(f"00002a19{BASE}", ["read"])])]

    assert isinstance(make_driver(FtmsOnly(), "X", lambda *_: None), FtmsDriver)  # type: ignore[arg-type]
    with pytest.raises(UnsupportedDevice):
        make_driver(Headphones(), "JBL", lambda *_: None)  # type: ignore[arg-type]


# --- prawdziwa sesja z bieżnią KS-NACH-X21C (log testera) ----------------------


def test_real_x21_session_streams_speed_without_gaps(monkeypatch: pytest.MonkeyPatch) -> None:
    from pathlib import Path

    lines = Path(__file__).with_name("dane").joinpath("x21-ramki.txt").read_text(encoding="utf-8")
    frames = [
        (float(t), bytes.fromhex(h))
        for t, h in (line.split() for line in lines.splitlines() if not line.startswith("#"))
    ]
    clock = [0.0]
    monkeypatch.setattr(drivers.time, "monotonic", lambda: clock[0])
    readings: list[tuple[float, float]] = []
    channel = drivers.Channel(
        f"00011234{BASE}",
        FakeChar(f"0001fed7{BASE}", ["write-without-response"]),
        FakeChar(f"0001fed8{BASE}", ["notify"]),
    )
    driver = KingSmithDriver(
        FakeTreadmill(), [channel], "KS-NACH-X21C", lambda s, _st: readings.append((clock[0], s))
    )
    undecoded = 0
    for t, data in frames:
        clock[0] = t
        for frame in channel.frames.feed(data):
            if kingsmith.decode(frame, "v5") is None:
                undecoded += 1
        driver._on_notify(channel, None, bytearray(data))
    assert undecoded == 0  # także odpowiedzi kończące się \r ("servers 0\r")
    assert driver.codec.table == "v5"
    speeds = {s for _, s in readings}
    assert {0.0, 1.6, 2.0, 2.5, 3.0, 2.3} <= speeds
    # podczas chodu (od pierwszej prędkości > 0) dane przychodzą częściej niż bezpiecznik 3 s
    moving = [t for t, s in readings if s > 0]
    walking = [t for t, _ in readings if t >= moving[0]]
    gaps = [b - a for a, b in itertools.pairwise(walking)]
    assert max(gaps) < 1.5, f"przerwa {max(gaps):.2f} s"


def test_driver_switches_to_ks_fit_telemetry_and_falls_back(
    monkeypatch: pytest.MonkeyPatch,
) -> None:

    treadmill = FakeTreadmill()
    driver, _readings = asyncio.run(run_driver(treadmill))
    assert any(t == kingsmith.TELEMETRY_PROPS for t in treadmill.received)
    assert driver.poll_command == kingsmith.TELEMETRY_PROPS
    # brak prędkości przez dłużej niż próg powrotu -> wracamy do listy z QZ
    now = [driver.telemetry_since + drivers.TELEMETRY_FALLBACK_S + 1.0]
    # tylko zegar sterownika - zegar asyncio musi iść dalej (pauzy między fragmentami)
    monkeypatch.setattr(drivers, "time", SimpleNamespace(monotonic=lambda: now[0]))
    driver.last_props_at = now[0] - drivers.TELEMETRY_FALLBACK_S - 1.0
    asyncio.run(driver.poll())
    assert driver.poll_command == kingsmith.GET_PROPS


# --- start i stop taśmy -----------------------------------------------------------


def test_start_belt_takes_control_and_starts() -> None:
    async def scenario() -> tuple[str, list[str], bool]:
        treadmill = FakeTreadmill(running=False)
        driver, _ = await run_driver(treadmill)
        result = await driver.start_belt()
        return result, treadmill.received, treadmill.running

    result, received, running = asyncio.run(scenario())
    assert result == "started"
    assert running
    i = received.index(kingsmith.CMD_CONTROL)
    assert received[i + 1] == kingsmith.CMD_START  # najpierw przejęcie sterowania


def test_refused_start_is_retried_three_times() -> None:
    async def scenario() -> tuple[str, list[str], int | None]:
        treadmill = FakeTreadmill(running=False, refuse_start=True)
        driver, _ = await run_driver(treadmill)
        result = await driver.start_belt()
        return result, treadmill.received, driver.last_error

    result, received, error = asyncio.run(scenario())
    assert result == "refused"
    assert error == -5000
    assert received.count(kingsmith.CMD_START) == drivers.START_ATTEMPTS
    assert received.count(kingsmith.CMD_CONTROL) == drivers.START_ATTEMPTS


def test_stop_belt_stops() -> None:
    async def scenario() -> tuple[bool, int | None]:
        treadmill = FakeTreadmill()
        driver, _ = await run_driver(treadmill)
        # w długiej sesji tablica jest już znana - krótkiej odpowiedzi można wtedy ufać
        driver.codec.candidates = ["v1"]
        await driver.stop_belt()
        await asyncio.sleep(0.01)
        return treadmill.running, driver.run_state

    running, state = asyncio.run(scenario())
    assert not running
    assert state == 0


class StubDriver:
    protocol = "stub"

    def __init__(self) -> None:
        self.calls: list[str] = []
        self.started = asyncio.Event()

    async def start(self) -> None:
        pass

    async def poll(self) -> None:
        pass

    def describe(self) -> str:
        return "stub"

    async def start_belt(self) -> str:
        self.calls.append("start")
        self.started.set()
        await asyncio.sleep(10)  # bieżnia długo się namyśla
        return "started"

    async def stop_belt(self) -> None:
        self.calls.append("stop")

    async def set_speed(self, kmh: float) -> bool:
        self.calls.append(f"speed {kmh:g}")
        return True

    def belt_speed(self) -> tuple[float, float] | None:
        return None


def bare_worker() -> Any:
    """BleWorker bez wątku - sama pętla poleceń, uruchamiana w teście."""
    from bieznia.ble.worker import BleWorker

    worker = BleWorker.__new__(BleWorker)
    worker.events = queue.Queue()
    worker._belt_task = None
    worker._target_speed = None
    worker._sent_speed = None
    worker._sent_at = 0.0
    worker._speed_at_send = 0.0
    worker._resends = 0
    worker._speed_confirmed = True
    return worker


def test_worker_stop_cancels_pending_start() -> None:
    worker = bare_worker()

    async def scenario() -> StubDriver:
        driver = StubDriver()
        lost = asyncio.Event()
        worker._commands = asyncio.Queue()
        stream = asyncio.create_task(worker._stream(driver, lost))  # type: ignore[arg-type]
        worker._commands.put_nowait("start")
        await asyncio.wait_for(driver.started.wait(), 2)
        worker._commands.put_nowait("start")  # drugi start w trakcie - ignorowany
        worker._commands.put_nowait("stop")
        for _ in range(50):
            await asyncio.sleep(0.01)
            if "stop" in driver.calls:
                break
        lost.set()
        await asyncio.wait_for(stream, 2)
        return driver

    driver = asyncio.run(scenario())
    assert driver.calls == ["start", "stop"]
    assert worker._belt_task is not None and worker._belt_task.cancelled()
    events = [worker.events.get_nowait() for _ in range(worker.events.qsize())]
    assert [(e.kind, e.text) for e in events] == [("belt", "stopped")]


def test_speed_command_format_like_qz() -> None:
    assert kingsmith.speed_command(1.5) == "props CurrentSpeed 1.5"
    assert kingsmith.speed_command(3.0) == "props CurrentSpeed 3"
    assert kingsmith.speed_command(0.0) == "props CurrentSpeed 0.5"  # 0 nie zatrzymuje taśmy
    assert kingsmith.speed_command(30) == "props CurrentSpeed 12"


def test_driver_sends_speed() -> None:
    async def scenario() -> list[str]:
        treadmill = FakeTreadmill()
        driver, _ = await run_driver(treadmill)
        await driver.set_speed(2.5)
        return treadmill.received

    assert asyncio.run(scenario())[-1] == "props CurrentSpeed 2.5"


def test_worker_sends_only_newest_speed_and_applies_it_after_start() -> None:
    worker = bare_worker()

    async def scenario() -> StubDriver:
        driver = StubDriver()
        driver.start_belt = quick_start  # type: ignore[method-assign]
        lost = asyncio.Event()
        worker._commands = asyncio.Queue()
        stream = asyncio.create_task(worker._stream(driver, lost))  # type: ignore[arg-type]
        for kmh in (1.0, 1.5, 2.5):  # klawisze w szybkim tempie - liczy się ostatni
            worker._queue_speed(kmh)
        await asyncio.sleep(0.05)
        worker._queue_speed(2.5)  # bez zmiany - nic nie idzie do bieżni
        worker._commands.put_nowait("start")
        await asyncio.sleep(0.05)
        lost.set()
        await asyncio.wait_for(stream, 2)
        return driver

    async def quick_start() -> str:
        return "started"

    driver = asyncio.run(scenario())
    # po starcie bieżnia jedzie z prędkością startową - docelowa idzie jeszcze raz
    assert driver.calls == ["speed 2.5", "speed 2.5"]


class ForgetfulBelt(StubDriver):
    """Jak X21 z logu testera: część poleceń prędkości przepada bez śladu."""

    def __init__(self, ignore: int) -> None:
        super().__init__()
        self.ignore = ignore
        self.speed = 1.0
        self.at = 0.0

    async def set_speed(self, kmh: float) -> bool:
        self.calls.append(f"speed {kmh:g}")
        if self.ignore > 0:
            self.ignore -= 1
        else:
            self.speed = kmh
        return True

    def belt_speed(self) -> tuple[float, float] | None:
        return self.speed, self.at


def test_ignored_speed_command_is_resent(monkeypatch: pytest.MonkeyPatch) -> None:
    from bieznia.ble import worker as worker_mod

    clock = [100.0]
    monkeypatch.setattr(worker_mod, "time", SimpleNamespace(monotonic=lambda: clock[0]))
    worker = bare_worker()
    belt = ForgetfulBelt(ignore=1)

    async def step(seconds: float) -> None:
        clock[0] += seconds
        belt.at = clock[0]  # świeża ramka z danymi
        await worker._check_speed(belt)

    async def scenario() -> None:
        worker._target_speed = 2.5
        await worker._apply_speed(belt)
        assert belt.calls == ["speed 2.5"]
        await step(1.0)
        assert belt.calls == ["speed 2.5"]  # za wcześnie na ponowienie
        await step(2.0)
        assert belt.calls == ["speed 2.5", "speed 2.5"]  # taśma nie drgnęła - ponowione
        await step(3.0)
        assert worker._speed_confirmed  # przyjęte - koniec
        belt.speed = 3.5  # pilot zmienia prędkość - aplikacja z nim nie walczy
        await step(5.0)
        assert len(belt.calls) == 2

    asyncio.run(scenario())


def test_resending_gives_up(monkeypatch: pytest.MonkeyPatch) -> None:
    from bieznia.ble import worker as worker_mod

    clock = [100.0]
    monkeypatch.setattr(worker_mod, "time", SimpleNamespace(monotonic=lambda: clock[0]))
    worker = bare_worker()
    belt = ForgetfulBelt(ignore=99)

    async def scenario() -> None:
        worker._target_speed = 2.5
        await worker._apply_speed(belt)
        for _ in range(20):
            clock[0] += 3.0
            belt.at = clock[0]
            await worker._check_speed(belt)

    asyncio.run(scenario())
    assert len(belt.calls) == 1 + worker_mod.SPEED_RESENDS


def test_ramping_belt_is_not_hurried(monkeypatch: pytest.MonkeyPatch) -> None:
    from bieznia.ble import worker as worker_mod

    clock = [100.0]
    monkeypatch.setattr(worker_mod, "time", SimpleNamespace(monotonic=lambda: clock[0]))
    worker = bare_worker()
    belt = ForgetfulBelt(ignore=1)

    async def scenario() -> None:
        worker._target_speed = 2.5
        await worker._apply_speed(belt)
        belt.speed = 1.8  # taśma już przyspiesza
        clock[0] += 3.0
        belt.at = clock[0]
        await worker._check_speed(belt)
        assert belt.calls == ["speed 2.5"]

    asyncio.run(scenario())


def test_speed_waits_after_treadmill_push(monkeypatch: pytest.MonkeyPatch) -> None:
    slept: list[float] = []

    async def fake_sleep(seconds: float) -> None:
        slept.append(seconds)

    driver = KingSmithDriver(FakeTreadmill(), [], "KS-X21", lambda s, _st: None)
    monkeypatch.setattr(drivers.asyncio, "sleep", fake_sleep)
    driver.last_props_at = drivers.time.monotonic() - 0.1  # ramka przed chwilą
    asyncio.run(driver.set_speed(2.5))
    assert slept and 0.25 < slept[0] <= drivers.QUIET_AFTER_PUSH_S
