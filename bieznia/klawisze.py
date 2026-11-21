"""Klawisze sterowania (chód, bieg) wybierane przez użytkownika.

Klawisz to fizyczny kod skanowania (scan code) + flaga „rozszerzony” (strzałki, prawy
Ctrl/Alt). Kod nie zależy od układu klawiatury i widzą go też gry z DirectInput.

Klawisz włączania sterowania może być kombinacją: Ctrl / Alt / Shift + klawisz (np.
Ctrl+Shift+F8). Niedozwolone są skróty zarezerwowane przez Windows (wszystkie z klawiszem
Windows, F12 dla debugera), systemowe (Alt+Tab, Alt+F4, Ctrl+Shift+Esc...), nakładek gier
(Steam Shift+Tab, NVIDIA Alt+Z) oraz Ctrl+Alt+litera, czyli AltGr pisze polskie znaki.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True)
class Key:
    scan: int
    extended: bool
    name: str
    vk: int = 0  # kod wirtualny - potrzebny skrótom globalnym (GetAsyncKeyState)
    mods: tuple[str, ...] = ()  # "ctrl", "alt", "shift" (i "win" - tylko do odrzucenia)

    def same_key(self, other: Key) -> bool:
        return (self.scan, self.extended) == (other.scan, other.extended)

    @property
    def label(self) -> str:
        """Nazwa do pokazania: z modyfikatorami, np. „Ctrl+Shift+F8”."""
        return "+".join([*(MOD_NAMES[m] for m in self.mods), self.name])

    def to_dict(self) -> dict[str, Any]:
        out: dict[str, Any] = {
            "scan": self.scan,
            "extended": self.extended,
            "name": self.name,
            "vk": self.vk,
        }
        if self.mods:
            out["mods"] = list(self.mods)
        return out

    @classmethod
    def from_dict(cls, data: Any, default: Key) -> Key:
        try:
            scan = int(data["scan"])
            if not 0 < scan < 0x100:
                return default
            vk = int(data.get("vk") or 0)
            mods = order_mods(str(m) for m in data.get("mods") or ())
            name = str(data.get("name") or "")
            return cls(scan, bool(data.get("extended", False)), name, vk, mods)
        except (TypeError, KeyError, ValueError):
            return default


MOD_ORDER = ("ctrl", "alt", "shift", "win")
MOD_NAMES = {"ctrl": "Ctrl", "alt": "Alt", "shift": "Shift", "win": "Win"}
MOD_VK = {"ctrl": 0x11, "alt": 0x12, "shift": 0x10, "win": 0x5B}  # stan lewego albo prawego


def order_mods(mods: Any) -> tuple[str, ...]:
    chosen = set(mods)
    return tuple(m for m in MOD_ORDER if m in chosen)


WALK_DEFAULT = Key(0x11, False, "W", 0x57)
RUN_DEFAULT = Key(0x2A, False, "Shift", 0xA0)
TOGGLE_DEFAULT = Key(0x42, False, "F8", 0x77)

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

# F6/F7: skróty symulatora, Esc: anulowanie wyboru klawisza, F12: Windows (debuger),
# Print Screen: zrzut ekranu Windows.
RESERVED: dict[tuple[int, bool], str] = {
    (0x40, False): "F6 to skrót symulatora",
    (0x41, False): "F7 to skrót symulatora",
    (0x01, False): "Esc anuluje wybór klawisza",
    (0x58, False): "F12 jest zarezerwowany przez Windows",
    (0x37, True): "Print Screen robi zrzut ekranu w Windows",
}

# Kombinacje zajęte przez Windows, gry i nakładki: (modyfikatory, scan) -> powód.
TAKEN_COMBOS: dict[tuple[tuple[str, ...], int], str] = {
    (("alt",), 0x0F): "Alt+Tab przełącza okna",
    (("alt",), 0x3E): "Alt+F4 zamyka okno",
    (("alt",), 0x01): "Alt+Esc przełącza okna",
    (("alt",), 0x39): "Alt+Spacja otwiera menu okna",
    (("alt",), 0x1C): "Alt+Enter przełącza pełny ekran w grach",
    (("alt",), 0x2C): "Alt+Z otwiera nakładkę NVIDIA",
    (("alt",), 0x3B): "Alt+F1 to zrzut ekranu NVIDIA",
    (("alt",), 0x43): "Alt+F9 nagrywa w NVIDIA",
    (("alt",), 0x44): "Alt+F10 zapisuje powtórkę NVIDIA",
    (("shift",), 0x0F): "Shift+Tab otwiera nakładkę Steam",
    (("ctrl",), 0x01): "Ctrl+Esc otwiera menu Start",
    (("ctrl", "shift"), 0x01): "Ctrl+Shift+Esc otwiera Menedżer zadań",
    (("ctrl", "alt"), 0x53): "Ctrl+Alt+Delete to skrót systemowy",
    (("ctrl",), 0x2E): "Ctrl+C kopiuje",
    (("ctrl",), 0x2F): "Ctrl+V wkleja",
    (("ctrl",), 0x2D): "Ctrl+X wycina",
    (("ctrl",), 0x2C): "Ctrl+Z cofa",
    (("ctrl",), 0x15): "Ctrl+Y ponawia",
    (("ctrl",), 0x1E): "Ctrl+A zaznacza wszystko",
    (("ctrl",), 0x1F): "Ctrl+S zapisuje",
}
# Litery, z których AltGr (= Ctrl+Alt) robi polskie znaki: ą ć ę ł ń ó ś ź ż.
ALTGR_LETTERS = frozenset({0x1E, 0x2E, 0x12, 0x26, 0x31, 0x18, 0x1F, 0x2D, 0x2C})
# Litery i cyfry (Shift + taki klawisz pisze w czacie zamiast działać jak skrót).
TYPING_KEYS = frozenset(
    [*range(0x02, 0x0C), *range(0x10, 0x1A), *range(0x1E, 0x27), *range(0x2C, 0x33)]
)
MODIFIER_SCANS = frozenset(
    {
        (0x2A, False),
        (0x36, False),
        (0x1D, False),
        (0x1D, True),
        (0x38, False),
        (0x38, True),
        (0x5B, True),
        (0x5C, True),
    }
)


def combo_problem(key: Key) -> str | None:
    """Czy kombinacja nie koliduje z Windows, grami albo pisaniem."""
    if "win" in key.mods:
        return "Skróty z klawiszem Windows są zarezerwowane przez system."
    if not key.mods:
        return None
    if (key.scan, key.extended) in MODIFIER_SCANS:
        return "Wybierz klawisz razem z Ctrl, Alt albo Shift, np. Ctrl+Shift+F8."
    taken = TAKEN_COMBOS.get((key.mods, key.scan))
    if taken:
        return f"{taken} - wybierz inną kombinację."
    if set(key.mods) == {"ctrl", "alt"} and key.scan in ALTGR_LETTERS and not key.extended:
        return f"{key.label} to AltGr - pisze polskie znaki. Wybierz inną kombinację."
    if key.mods == ("shift",) and key.scan in TYPING_KEYS and not key.extended:
        return "Shift + litera pisze wielką literę - dodaj Ctrl albo Alt, np. Ctrl+Shift."
    return None


def key_from_native(native_scan: int, fallback_name: str, vk: int = 0) -> Key | None:
    """Klawisz z kodu podanego przez Windows (bit 0x100 = klawisz rozszerzony)."""
    scan = native_scan & 0xFF
    if scan == 0:
        return None
    extended = bool(native_scan & 0x100)
    name = NAMES.get((scan, extended)) or fallback_name.strip() or f"Kod {scan:#04x}"
    return Key(scan, extended, name, vk)


def problem(walk: Key, run: Key, toggle: Key = TOGGLE_DEFAULT) -> str | None:
    """Opis problemu z wybranymi klawiszami albo None, gdy wszystko w porządku."""
    for key in (walk, run, toggle):
        reason = RESERVED.get((key.scan, key.extended))
        if reason:
            return f"Nie można użyć tego klawisza: {reason}."
    if walk.same_key(run):
        return "Chód i bieg muszą mieć różne klawisze."
    if toggle.same_key(walk) or toggle.same_key(run):
        # aplikacja sama naciska klawisze chodu i biegu - skrót wyłączałby sam siebie
        return "Start/stop musi mieć inny klawisz niż chód i bieg."
    if toggle.vk == 0:
        return "Tego klawisza nie da się użyć jako start/stop - wybierz inny."
    if walk.mods or run.mods:
        return "Chód i bieg to pojedyncze klawisze - bez Ctrl, Alt i Shift."
    return combo_problem(toggle)
