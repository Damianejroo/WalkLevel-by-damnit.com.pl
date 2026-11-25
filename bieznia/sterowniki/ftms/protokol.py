"""Standard Bluetooth FTMS (Fitness Machine Service): Urevo SpaceWalk 5L i inne bieżnie FTMS.

Dane (Treadmill Data 0x2ACD, co ok. 1 s), stan maszyny (0x2ADA) i polecenia przez
Control Point (0x2AD9) - każde polecenie dostaje odpowiedź (indykację) z wynikiem.

Urevo SpaceWalk 5L (URTM054, firmware V90.08.15, sprawdzone na żywym urządzeniu):
prędkość 1,00-6,00 km/h co 0,10, nachylenie 0-9 % co 1 %, flagi danych 0x259C.
Bit 13 flag (spoza FTMS 1.0) niesie 2 bajty - u Urevo najpewniej liczba kroków.
"""

from __future__ import annotations

import struct
from dataclasses import dataclass

U = "0000{}-0000-1000-8000-00805f9b34fb"
FTMS_SERVICE = U.format("1826")
FEATURE = U.format("2acc")
TREADMILL_DATA = U.format("2acd")
TRAINING_STATUS = U.format("2ad3")
SPEED_RANGE = U.format("2ad4")
INCLINE_RANGE = U.format("2ad5")
CONTROL_POINT = U.format("2ad9")
MACHINE_STATUS = U.format("2ada")

OP_REQUEST_CONTROL = 0x00
OP_SET_SPEED = 0x02
OP_SET_INCLINE = 0x03
OP_START = 0x07
OP_STOP_PAUSE = 0x08
STOP, PAUSE = 0x01, 0x02
RESPONSE = 0x80

RESULT_OK = 1
RESULT_NOT_PERMITTED = 5
RESULTS = {
    1: "przyjęte",
    2: "nieobsługiwane",
    3: "zła wartość",
    4: "nie powiodło się",
    5: "brak kontroli",
}
# Fitness Machine Status (0x2ADA)
STATUS_RESET = 0x01
STATUS_STOPPED = 0x02  # zatrzymana albo pauza (przez użytkownika albo polecenie)
STATUS_SAFETY_KEY = 0x03
STATUS_STARTED = 0x04
STATUS_SPEED = 0x05
STATUS_INCLINE = 0x06
STATUS_NAMES = {
    STATUS_RESET: "reset",
    STATUS_STOPPED: "zatrzymana",
    STATUS_SAFETY_KEY: "zatrzymana kluczem bezpieczeństwa",
    STATUS_STARTED: "uruchomiona",
    STATUS_SPEED: "zmiana prędkości",
    STATUS_INCLINE: "zmiana nachylenia",
}
STEPS_FLAG = 0x2000  # spoza FTMS 1.0 - Urevo: liczba kroków (uint16)


@dataclass(frozen=True)
class TreadmillData:
    speed_kmh: float | None = None
    distance_m: int | None = None
    incline_pct: float | None = None
    energy_kcal: int | None = None
    heart_rate: int | None = None
    elapsed_s: int | None = None
    steps: int | None = None


def decode_treadmill_data(data: bytes) -> TreadmillData | None:
    """Pola obecne w jednej ramce (wg flag). None, gdy ramka jest za krótka albo ucięta."""
    if len(data) < 2:
        return None
    flags = int.from_bytes(data[0:2], "little")
    pos = 2
    fields: dict[str, float | int] = {}

    def take(fmt: str) -> tuple[int, ...]:
        nonlocal pos
        size = struct.calcsize("<" + fmt)
        if pos + size > len(data):
            raise ValueError("ucięta ramka")
        vals = struct.unpack_from("<" + fmt, data, pos)
        pos += size
        return vals

    try:
        if not flags & 0x0001:  # bit 0 odwrotny: „More Data” = 0 -> jest prędkość
            fields["speed_kmh"] = take("H")[0] / 100
        if flags & 0x0002:
            take("H")  # średnia prędkość
        if flags & 0x0004:
            lo, hi = take("HB")
            fields["distance_m"] = lo | (hi << 16)
        if flags & 0x0008:
            incline, _ramp = take("hh")
            fields["incline_pct"] = incline / 10
        if flags & 0x0010:
            take("HH")  # przewyższenie
        if flags & 0x0020:
            take("B")  # tempo
        if flags & 0x0040:
            take("B")  # średnie tempo
        if flags & 0x0080:
            total, _per_hour, _per_min = take("HHB")
            if total != 0xFFFF:
                fields["energy_kcal"] = total
        if flags & 0x0100:
            fields["heart_rate"] = take("B")[0]
        if flags & 0x0200:
            take("B")  # MET
        if flags & 0x0400:
            fields["elapsed_s"] = take("H")[0]
        if flags & 0x0800:
            take("H")  # pozostały czas
        if flags & 0x1000:
            take("hh")  # siła i moc
        if flags & STEPS_FLAG:
            fields["steps"] = take("H")[0]
    except ValueError:
        return None
    return TreadmillData(**fields)  # type: ignore[arg-type]


def parse_treadmill_data(data: bytes) -> float | None:
    """Sama prędkość w km/h albo None."""
    decoded = decode_treadmill_data(data)
    return decoded.speed_kmh if decoded else None


def encode_request_control() -> bytes:
    return bytes([OP_REQUEST_CONTROL])


def encode_start() -> bytes:
    return bytes([OP_START])


def encode_stop() -> bytes:
    return bytes([OP_STOP_PAUSE, STOP])


def encode_pause() -> bytes:
    return bytes([OP_STOP_PAUSE, PAUSE])


def encode_set_speed(kmh: float) -> bytes:
    return struct.pack("<BH", OP_SET_SPEED, round(kmh * 100))


def encode_set_incline(pct: float) -> bytes:
    return struct.pack("<Bh", OP_SET_INCLINE, round(pct * 10))


def parse_response(data: bytes) -> tuple[int, int] | None:
    """Odpowiedź Control Point: (kod polecenia, wynik) albo None, gdy to nie odpowiedź."""
    if len(data) < 3 or data[0] != RESPONSE:
        return None
    return data[1], data[2]


def decode_speed_range(data: bytes) -> tuple[float, float, float] | None:
    if len(data) < 6:
        return None
    lo, hi, step = struct.unpack_from("<HHH", data)
    return lo / 100, hi / 100, step / 100


def decode_incline_range(data: bytes) -> tuple[float, float, float] | None:
    if len(data) < 6:
        return None
    lo, hi, step = struct.unpack_from("<hhH", data)
    return lo / 10, hi / 10, step / 10
