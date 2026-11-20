"""Protokół KingSmith i sterowniki - na wirtualnej bieżni zachowującej się jak X21 w QZ."""

from __future__ import annotations

import asyncio
import itertools
from dataclasses import dataclass, field
from typing import Any

import pytest

from bieznia.ble import ftms, kingsmith
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
            self._reply(prefix, f"props CurrentSpeed {self.speed} RunningSteps 120 runState 1")
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

    from bieznia.ble import drivers

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
