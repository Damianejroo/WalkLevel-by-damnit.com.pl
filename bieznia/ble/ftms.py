"""Standard Bluetooth FTMS (Fitness Machine Service) - zapasowo, gdyby bieżnia go wystawiała."""

from __future__ import annotations

FTMS_SERVICE = "00001826-0000-1000-8000-00805f9b34fb"
TREADMILL_DATA = "00002acd-0000-1000-8000-00805f9b34fb"


def parse_treadmill_data(data: bytes) -> float | None:
    """Prędkość w km/h z charakterystyki Treadmill Data (0x2ACD) albo None.

    Bajty 0-1: flagi. Bit 0 („More Data”) = 0 oznacza, że zaraz po flagach jest
    chwilowa prędkość: uint16, little-endian, w jednostkach 0,01 km/h.
    """
    if len(data) < 4:
        return None
    flags = int.from_bytes(data[0:2], "little")
    if flags & 0x0001:
        return None
    return int.from_bytes(data[2:4], "little") / 100.0
