"""Klawisze sterowania (chód, bieg) wybierane przez użytkownika.

Klawisz to fizyczny kod skanowania (scan code) + flaga „rozszerzony” (strzałki, prawy
Ctrl/Alt). Kod nie zależy od układu klawiatury i widzą go też gry z DirectInput.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True)
class Key:
    scan: int
    extended: bool
    name: str

    def to_dict(self) -> dict[str, Any]:
        return {"scan": self.scan, "extended": self.extended, "name": self.name}

    @classmethod
    def from_dict(cls, data: Any, default: Key) -> Key:
        try:
            scan = int(data["scan"])
            if not 0 < scan < 0x100:
                return default
            return cls(scan, bool(data.get("extended", False)), str(data.get("name") or ""))
        except (TypeError, KeyError, ValueError):
            return default


WALK_DEFAULT = Key(0x11, False, "W")
RUN_DEFAULT = Key(0x2A, False, "Shift")

# Polskie nazwy klawiszy, których tekst nie mówi sam za siebie.
NAMES: dict[tuple[int, bool], str] = {
    (0x48, True): "Strzałka w górę",
    (0x50, True): "Strzałka w dół",
    (0x4B, True): "Strzałka w lewo",
    (0x4D, True): "Strzałka w prawo",
    (0x2A, False): "Shift",
    (0x36, False): "Prawy Shift",
    (0x1D, False): "Ctrl",
    (0x1D, True): "Prawy Ctrl",
    (0x38, False): "Alt",
    (0x38, True): "Prawy Alt",
    (0x39, False): "Spacja",
    (0x0F, False): "Tab",
    (0x3A, False): "Caps Lock",
    (0x1C, False): "Enter",
    (0x0E, False): "Backspace",
    (0x52, True): "Insert",
    (0x53, True): "Delete",
    (0x47, True): "Home",
    (0x4F, True): "End",
    (0x49, True): "Page Up",
    (0x51, True): "Page Down",
}

# F6-F8: skróty samej aplikacji, Esc: anulowanie wyboru klawisza.
RESERVED: dict[tuple[int, bool], str] = {
    (0x40, False): "F6 to skrót symulatora",
    (0x41, False): "F7 to skrót symulatora",
    (0x42, False): "F8 włącza i wyłącza sterowanie",
    (0x01, False): "Esc anuluje wybór klawisza",
}


def key_from_native(native_scan: int, fallback_name: str) -> Key | None:
    """Klawisz z kodu podanego przez Windows (bit 0x100 = klawisz rozszerzony)."""
    scan = native_scan & 0xFF
    if scan == 0:
        return None
    extended = bool(native_scan & 0x100)
    name = NAMES.get((scan, extended)) or fallback_name.strip() or f"Kod {scan:#04x}"
    return Key(scan, extended, name)


def problem(walk: Key, run: Key) -> str | None:
    """Opis problemu z wybranymi klawiszami albo None, gdy wszystko w porządku."""
    for key in (walk, run):
        reason = RESERVED.get((key.scan, key.extended))
        if reason:
            return f"Nie można użyć tego klawisza: {reason}."
    if (walk.scan, walk.extended) == (run.scan, run.extended):
        return "Chód i bieg muszą mieć różne klawisze."
    return None
