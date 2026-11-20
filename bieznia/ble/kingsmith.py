"""Protokół KingSmith (WalkingPad R2 / X21 / G1 / X23): kodowanie wiadomości.

Na podstawie otwartego sterownika QZ (qdomyos-zwift, GPL-3.0, plik
src/devices/kingsmithr2treadmill). Wiadomość to tekst, np. "servers getProp 1 2 7",
który idzie tak: UTF-8 -> base64 -> podstawienie znaków tablicą -> znak '\\r' ->
kawałki po 16 bajtów. Odpowiedzi wracają tak samo, np. "props CurrentSpeed 3.5 spm 98".

Bieżnie różnią się tablicą podstawień (QZ zna 7 wariantów). Nie zgadujemy: tablicę
wykrywamy z odpowiedzi bieżni, bo tylko właściwa daje sensowny tekst.
"""

from __future__ import annotations

import base64
import binascii
import re
from collections.abc import Iterator
from typing import NamedTuple

PLAINTEXT = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/="

TABLES: dict[str, bytes] = {
    "v1": b"SaCw4FGHIJqLhN+P9RVTU/WcY6ObDdefgEijklmnopQrsBuvMxXz1yA2t5078KZ3=",
    "v2": b"ZaCw4FGHIJqLhN+P9RMTU/WcY6ObDdefgEijklmnopQrsBuvVxXz1yA2t5078KS3=",
    "v3": b"0aCw4FGHIJqLhN+P9RVTU/WcY6ObDdefgEijklmnopQrsBuvMxXz1yA2t5Z78KS3=",
    "v4": b"ZaCw4FGHIJqLhN9P+RVTU/WcY6ObDdefgEijklmnopQrsBuvMxXz1yA2t5078KS3=",
    "v5": b"iaCw4FGHIJqLhN+P9RVTU/WcY6ObDdefgEZjklmnopQrsBuvMxXz1yA2t5078KS3=",
    "v6": b"ZaCw4FGHIJqLhN+P8RVTU/WcY6ObDdefgEijklmnopQrsBuvMxXz1yA2t5079KS3=",
    "v7": b"baCw4FGHIJqLhN+P9RVTU/WcY6OZDdefgEijklmnopQrsBuvMxXz1yA2t5078KS3=",
}

_ENCODE = {name: bytes.maketrans(PLAINTEXT, table) for name, table in TABLES.items()}
_DECODE = {name: bytes.maketrans(table, PLAINTEXT) for name, table in TABLES.items()}
_ALPHABET = {name: frozenset(table) for name, table in TABLES.items()}

END = b"\r"
CHUNK = 16
MAX_BUFFER = 4096

# Pierwsze słowa odpowiedzi, które na pewno pochodzą od bieżni KingSmith.
KNOWN_WORDS = frozenset(
    {"props", "shake", "net", "get_dn", "get_pk", "time_posix", "version", "servers", "format"}
)
_PRINTABLE = re.compile(r"^[\x20-\x7e\t]*$")

# Sekwencja startowa jak w QZ (bez niej bieżnia nie odpowiada na odczyty).
INIT_COMMANDS = ("shake", "net", "get_dn", "get_pk", "time_posix {now}", "version")
GET_PROPS = "servers getProp 1 2 7 12 23 24 31"
# Subskrypcja telemetrii oficjalnej aplikacji KS Fit (wg opisu protokołu z projektu
# beltcontrol). Tylko odczyt. Na części modeli daje częstsze, częściowe ramki "props".
TELEMETRY_PROPS = "servers getProp 1 9 15 2 10 11 12 13 14"

# Sterowanie (wg QZ i opisu protokołu KS Fit z projektu beltcontrol). Start: przejęcie
# sterowania (ControlMode 1) i ruch (runState 1) - taśma rusza ze stałą prędkością
# startową bieżni (X21: 1,0 km/h). Stop i pauza to to samo polecenie.
CMD_CONTROL = "props ControlMode 1"
CMD_START = "props runState 1"
CMD_STOP = "props runState 0"
# Prędkość docelowa jak w QZ (forceSpeed): liczba bez zbędnych zer, np. "1.5", "3".
SPEED_MIN = 0.5
SPEED_MAX = 12.0


def speed_command(kmh: float) -> str:
    value = round(min(SPEED_MAX, max(SPEED_MIN, kmh)), 1)
    return f"props CurrentSpeed {value:g}"


def table_order(device_name: str) -> list[str]:
    """Kolejność prób: X23 (KS-NACH-MXG) używa v7, reszta zwykle v1."""
    order = list(TABLES)
    if device_name.upper().startswith("KS-NACH-MXG"):
        order.remove("v7")
        order.insert(0, "v7")
    return order


def encode(text: str, table: str) -> bytes:
    """Gotowa wiadomość do wysłania (razem z kończącym '\\r')."""
    return base64.b64encode(text.encode("utf-8")).translate(_ENCODE[table]) + END


def chunks(data: bytes, size: int = CHUNK) -> Iterator[bytes]:
    for i in range(0, len(data), size):
        yield data[i : i + size]


def decode(frame: bytes, table: str) -> str | None:
    """Odszyfrowuje jedną ramkę (bez '\\r'). None, jeśli tablica nie pasuje."""
    if not frame or not set(frame) <= _ALPHABET[table]:
        return None
    try:
        raw = base64.b64decode(frame.translate(_DECODE[table]), validate=True)
        # X21 kończy odpowiedzi znakiem CR/LF wewnątrz zaszyfrowanej treści
        text = raw.decode("ascii").rstrip("\r\n")
    except (binascii.Error, ValueError):
        return None
    return text if _PRINTABLE.match(text) else None


_STRICT = re.compile(r"^[A-Za-z0-9_.:+\-/,\"=() ]+$")


def is_known_reply(text: str) -> bool:
    """Odpowiedź, która na pewno jest poprawnie odszyfrowana.

    Zła tablica zmienia pojedyncze znaki (np. 'props ;urrentSpeed', 'shake 0?'),
    więc oprócz znanego pierwszego słowa wymagamy samych „zwykłych” znaków.
    """
    words = text.split(maxsplit=1)
    return bool(words) and words[0].lower() in KNOWN_WORDS and bool(_STRICT.match(text))


class FrameBuffer:
    """Skleja powiadomienia BLE w pełne ramki zakończone '\\r'."""

    def __init__(self) -> None:
        self._buffer = bytearray()

    def feed(self, data: bytes) -> list[bytes]:
        self._buffer += data
        if len(self._buffer) > MAX_BUFFER:  # śmieci bez '\r' - nie rośnij w nieskończoność
            self._buffer.clear()
            return []
        *frames, rest = bytes(self._buffer).split(END)
        self._buffer = bytearray(rest)
        return [f for f in frames if f]


class Decoded(NamedTuple):
    text: str
    trusted: bool  # wszystkie możliwe tablice dają ten sam tekst - można ufać liczbom


class Codec:
    """Szyfrowanie z wykrywaniem tablicy na podstawie odpowiedzi bieżni.

    Tablice różnią się tylko kilkoma znakami, więc jedna ramka często pasuje do kilku
    z nich. Trzymamy zbiór kandydatów i zawężamy go każdą poprawną ramką. Tekst
    oznaczamy jako pewny tylko wtedy, gdy wszyscy kandydaci odszyfrowują go
    identycznie - dzięki temu nigdy nie podamy błędnej prędkości.
    """

    def __init__(self, order: list[str] | None = None) -> None:
        self.candidates = list(order or TABLES)
        self.table = self.candidates[0]

    @property
    def locked(self) -> bool:
        return len(self.candidates) == 1

    def describe(self) -> str:
        return self.table if self.locked else "/".join(self.candidates)

    def encode(self, text: str) -> bytes:
        return encode(text, self.table)

    def decode(self, frame: bytes) -> Decoded | None:
        texts = {name: decode(frame, name) for name in self.candidates}
        good = [name for name, text in texts.items() if text and is_known_reply(text)]
        if not good:
            # Nic pewnego (albo bieżnia zmieniła zachowanie) - sprawdź wszystkie tablice.
            texts = {name: decode(frame, name) for name in TABLES}
            good = [name for name, text in texts.items() if text and is_known_reply(text)]
            if not good:
                return None
        self.candidates = good
        if self.table not in good:
            self.table = good[0]
        variants = {texts[name] for name in good}
        text = texts[self.table]
        assert text is not None
        return Decoded(text, trusted=len(variants) == 1)

    def try_next_table(self) -> None:
        """Bieżnia milczy - wysyłaj kolejną z pozostałych tablic."""
        if not self.locked:
            i = self.candidates.index(self.table)
            self.table = self.candidates[(i + 1) % len(self.candidates)]


def parse_props(text: str) -> dict[str, float]:
    """'props CurrentSpeed 3.5 spm 98 ...' -> {'CurrentSpeed': 3.5, 'spm': 98.0}.

    Jak w QZ: pary klucz-wartość, 'Error' kończy listę, wartości tekstowe pomijamy.
    """
    parts = text.split()
    if not parts or parts[0] != "props":
        return {}
    props: dict[str, float] = {}
    for i in range(1, len(parts) - 1, 2):
        key, value = parts[i], parts[i + 1]
        if key == "Error":
            break
        try:
            props[key] = float(value)
        except ValueError:
            continue
    return props
