"""Standard FTMS (Urevo SpaceWalk 5L): dekoder ramek i sterowanie na wirtualnej bieżni."""

from __future__ import annotations

import asyncio
from dataclasses import dataclass, field
from typing import Any

from bieznia.sterowniki import FtmsDriver, looks_like_treadmill, make_driver
from bieznia.sterowniki.ftms import protokol as ftms
from bieznia.sterowniki.ftms import sterownik as drivers

# Prawdziwa ramka z URTM054 (fw V90.08.15): taśma stoi, nachylenie 9 %, 90 m, 12 kcal,
# 224 s, pole spoza FTMS 1.0 (bit 13) = 291.
REAL_IDLE = bytes.fromhex(
    "9c 25 00 00 5a 00 00 5a 00 00 00 00 00 00 00 0c 00 00 00 00 00 e0 00 23 01 00"
)
# Ramka z referencyjnego projektu (1,0 km/h, 20 m, 74 s).
REAL_WALK = bytes.fromhex(
    "9c 25 64 00 14 00 00 00 00 00 00 00 00 00 00 00 02 00 00 00 00 00 4a 00 00 00 00"
)


def test_decodes_real_urevo_frames() -> None:
    idle = ftms.decode_treadmill_data(REAL_IDLE)
    assert idle is not None
    assert idle.speed_kmh == 0.0 and idle.distance_m == 90 and idle.incline_pct == 9.0
    assert idle.energy_kcal == 12 and idle.elapsed_s == 224 and idle.steps == 291
    walk = ftms.decode_treadmill_data(REAL_WALK)
    assert walk is not None and walk.speed_kmh == 1.0 and walk.distance_m == 20
    assert ftms.decode_treadmill_data(REAL_IDLE[:10]) is None  # ucięta ramka
    assert ftms.decode_treadmill_data(b"\x01") is None
    assert ftms.parse_treadmill_data(REAL_WALK) == 1.0


def test_encodes_control_point_commands() -> None:
    assert ftms.encode_request_control() == b"\x00"
    assert ftms.encode_start() == b"\x07"
    assert ftms.encode_pause() == b"\x08\x02"
    assert ftms.encode_stop() == b"\x08\x01"
    assert ftms.encode_set_speed(2.5) == bytes.fromhex("02 fa 00")
    assert ftms.parse_response(bytes.fromhex("80 07 01")) == (0x07, 1)
    assert ftms.parse_response(bytes.fromhex("01 02")) is None
    assert ftms.decode_speed_range(bytes.fromhex("64 00 58 02 0a 00")) == (1.0, 6.0, 0.1)


@dataclass(eq=False)
class Char:
    uuid: str
    properties: list[str]


@dataclass(eq=False)
class Service:
    uuid: str
    characteristics: list[Char]


@dataclass
class FakeUrevo:
    """Odpowiada jak SpaceWalk 5L: potwierdzenie każdego polecenia, stan maszyny, dane."""

    min_speed: float = 1.0
    start_ignored: int = 0  # tyle startów zostaje potwierdzonych bez ruchu taśmy (jak w 5L)
    silent: bool = False  # nie odpowiada na polecenia
    lose: int = 0  # tyle kolejnych poleceń przepada bez śladu (jak przy słabym sygnale)
    slow_stop: bool = False  # jak 5L: po pauzie hamuje kilka ramek, „zatrzymana” na końcu
    speed: float = 0.0
    received: list[str] = field(default_factory=list)
    callbacks: dict[str, Any] = field(default_factory=dict)

    def __post_init__(self) -> None:
        self.chars = {
            name: Char(uuid, props)
            for name, uuid, props in (
                ("data", ftms.TREADMILL_DATA, ["notify"]),
                ("cp", ftms.CONTROL_POINT, ["write", "indicate"]),
                ("status", ftms.MACHINE_STATUS, ["notify"]),
                ("range", ftms.SPEED_RANGE, ["read"]),
            )
        }
        self.services = [Service(ftms.FTMS_SERVICE, list(self.chars.values()))]

    async def read_gatt_char(self, char: Char) -> bytes:
        assert char is self.chars["range"]
        return bytes([round(self.min_speed * 100), 0, 0x58, 0x02, 0x0A, 0x00])

    async def start_notify(self, char: Char, callback: Any) -> None:
        self.callbacks[char.uuid] = callback

    def emit(self, char: str, data: bytes) -> None:
        self.callbacks[self.chars[char].uuid](None, bytearray(data))

    def frame(self) -> None:
        raw = bytearray(REAL_IDLE)
        raw[2:4] = round(self.speed * 100).to_bytes(2, "little")
        self.emit("data", bytes(raw))

    async def write_gatt_char(self, char: Char, data: bytes, response: bool | None = None) -> None:
        assert char is self.chars["cp"] and response is True
        self.received.append(data.hex(" "))
        if self.silent:
            return
        if self.lose > 0 and data[0] != ftms.OP_REQUEST_CONTROL:
            self.lose -= 1
            return
        op = data[0]
        loop = asyncio.get_running_loop()
        if op == ftms.OP_SET_SPEED:
            kmh = int.from_bytes(data[1:3], "little") / 100
            if kmh < self.min_speed:
                loop.call_soon(self.emit, "cp", bytes([0x80, op, 3]))  # zła wartość
                return
            self.speed = kmh
        loop.call_soon(self.emit, "cp", bytes([0x80, op, 1]))
        if op == ftms.OP_START:
            if self.start_ignored > 0:
                self.start_ignored -= 1
                loop.call_soon(self.emit, "status", b"\x02")
                return
            self.speed = max(self.speed, self.min_speed)
            loop.call_later(0.05, self.emit, "status", b"\x04")
            loop.call_later(0.08, self.frame)
        elif op == ftms.OP_STOP_PAUSE:
            if self.slow_stop:
                start = self.speed
                for i in range(1, 6):  # 5 ramek hamowania co 0,1 s, potem stan „zatrzymana”
                    loop.call_later(0.1 * i, self._brake, start * (1 - i / 5))
                loop.call_later(0.6, self.emit, "status", bytes([2, 2]))
                return
            self.speed = 0.0
            loop.call_later(0.05, self.emit, "status", b"\x02")
            loop.call_later(0.08, self.frame)


def _brake_method(self: FakeUrevo, kmh: float) -> None:
    self.speed = round(kmh, 2)
    self.frame()


FakeUrevo._brake = _brake_method  # type: ignore[attr-defined]


def run(coro: Any) -> Any:
    return asyncio.run(coro)


def make(fake: FakeUrevo, readings: list[tuple[float, int | None]] | None = None) -> FtmsDriver:
    sink = readings if readings is not None else []
    driver = make_driver(fake, "URTM054", lambda s, k: sink.append((s, k)))  # type: ignore[arg-type]
    assert isinstance(driver, FtmsDriver)
    return driver


def test_urevo_is_recognized_and_named() -> None:
    fake = FakeUrevo()
    driver = make(fake)
    assert driver.describe() == "Urevo SpaceWalk 5L (FTMS)"
    assert looks_like_treadmill("URTM054")
    assert looks_like_treadmill(None, [ftms.FTMS_SERVICE])


def test_reads_speed_range_and_streams_steps() -> None:
    async def scenario() -> None:
        fake = FakeUrevo(min_speed=1.0)
        readings: list[tuple[float, int | None]] = []
        driver = make(fake, readings)
        await driver.start()
        assert (driver.speed_min, driver.speed_max) == (1.0, 6.0)
        fake.frame()
        assert readings == [(0.0, 291)]
        assert driver.belt_running() is False

    run(scenario())


def test_start_takes_control_and_waits_for_belt() -> None:
    async def scenario() -> None:
        fake = FakeUrevo()
        driver = make(fake)
        await driver.start()
        assert await driver.start_belt() == "started"
        assert fake.received[:2] == ["00", "07"]  # najpierw przejęcie kontroli
        assert driver.belt_running() is True

    run(scenario())


def test_start_retries_when_belt_ignores_first_start(monkeypatch: Any) -> None:
    monkeypatch.setattr(drivers, "FTMS_START_WAIT_S", 0.2)

    async def scenario() -> None:
        fake = FakeUrevo(start_ignored=1)  # jak 5L: pierwszy Start po Stopie bez ruchu
        driver = make(fake)
        await driver.start()
        assert await driver.start_belt() == "started"
        assert fake.received.count("07") == 2

    run(scenario())


def test_stop_is_pause_and_closing_is_full_stop() -> None:
    async def scenario() -> None:
        fake = FakeUrevo()
        driver = make(fake)
        await driver.start()
        await driver.start_belt()
        await driver.stop_belt()
        assert fake.received[-1] == "08 02" and driver.belt_running() is False
        await driver.start_belt()
        await driver.stop_belt(verify=False)
        assert fake.received[-1] == "08 01"

    run(scenario())


def test_speed_below_minimum_is_raised_to_lowest() -> None:
    async def scenario() -> None:
        fake = FakeUrevo(min_speed=1.0)
        driver = make(fake)
        await driver.start()
        assert await driver.set_speed(0.6)  # poniżej minimum Urevo nie odpowiada - nie wysyłamy
        assert fake.received[-1] == "02 64 00" and fake.speed == 1.0
        assert "02 3c 00" not in fake.received
        assert await driver.set_speed(9.0) and fake.speed == 6.0  # i nie ponad maksimum

    run(scenario())


def test_lost_stop_is_retried_with_full_stop(monkeypatch: Any) -> None:
    """Log z 5L: pauza po F8 przepadła i taśma jechała dalej. Stop ponawiamy do skutku."""
    monkeypatch.setattr(drivers, "FTMS_RESPONSE_S", 0.05)
    monkeypatch.setattr(drivers, "FTMS_STOP_WAIT_S", 0.2)

    async def scenario() -> None:
        fake = FakeUrevo()
        driver = make(fake)
        await driver.start()
        await driver.start_belt()
        fake.lose = 1  # pauza przepada
        await driver.stop_belt()
        assert fake.received[-2:] == ["08 02", "08 01"]
        assert fake.speed == 0.0 and driver.belt_running() is False

    run(scenario())


def test_closing_stop_retries_until_confirmed(monkeypatch: Any) -> None:
    monkeypatch.setattr(drivers, "FTMS_RESPONSE_S", 0.05)
    monkeypatch.setattr(drivers, "FTMS_STOP_WAIT_S", 0.2)

    async def scenario() -> None:
        fake = FakeUrevo()
        driver = make(fake)
        await driver.start()
        await driver.start_belt()
        fake.lose = 2
        await driver.stop_belt(verify=False)
        assert fake.received.count("08 01") == 3 and fake.speed == 0.0

    run(scenario())


def test_accepts_low_speed_when_treadmill_allows_it() -> None:
    async def scenario() -> None:
        fake = FakeUrevo(min_speed=0.5)
        driver = make(fake)
        await driver.start()
        assert await driver.set_speed(0.6) and fake.speed == 0.6

    run(scenario())


def test_silent_treadmill_does_not_hang(monkeypatch: Any) -> None:
    monkeypatch.setattr(drivers, "FTMS_RESPONSE_S", 0.05)

    async def scenario() -> None:
        fake = FakeUrevo(silent=True)
        driver = make(fake)
        await driver.start()
        assert not await driver.set_speed(2.0)
        assert await driver.start_belt() == "unknown"

    run(scenario())


def test_slow_braking_after_pause_is_not_retried(monkeypatch: Any) -> None:
    """Log z 5L: „zatrzymana” przychodzi dopiero po wyhamowaniu - to nie zgubiony stop."""
    monkeypatch.setattr(drivers, "FTMS_STOP_SLOWING_S", 0.25)
    monkeypatch.setattr(drivers, "FTMS_STOP_WAIT_S", 2.0)

    async def scenario() -> None:
        fake = FakeUrevo(slow_stop=True)
        driver = make(fake)
        await driver.start()
        await driver.start_belt()
        await driver.set_speed(3.0)
        fake.frame()
        await driver.stop_belt()
        assert fake.received.count("08 02") == 1 and "08 01" not in fake.received
        assert driver.belt_running() is False

    run(scenario())


def test_reachable_speed_respects_treadmill_range() -> None:
    driver = make(FakeUrevo())
    assert driver.reachable_speed(0.0) == 1.0 and driver.reachable_speed(9.0) == 6.0
