"""Pad: wychylenie lewej gałki zamienione na skoki prędkości taśmy.

Odczyt bez dodatkowych bibliotek i bez fokusu (gra może być na wierzchu):
- XInput - pady Xbox i wszystko, co Steam/DS4Windows udaje jako pad Xbox,
- winmm (DirectInput) - DualSense, DualShock i inne pady HID.
Program tylko czyta pozycję gałki, niczego nie wysyła do gry.
"""

from __future__ import annotations

import ctypes
import itertools
import logging
import math
import time
from collections.abc import Callable, Iterable
from ctypes import wintypes
from dataclasses import dataclass
from typing import Any, Protocol

from bieznia.winapi import IS_WINDOWS

log = logging.getLogger("bieznia.pad")

# Gałka w spoczynku potrafi stać na kilku procentach, a do końca rzadko dochodzi równe 100%.
INNER_DEADZONE = 0.08
OUTER_DEADZONE = 0.95
RESCAN_S = 2.0

STEPS_MIN, STEPS_MAX = 2, 5
# Gotowe skoki: (wychylenie w %, prędkość km/h). Pierwszy zawsze od 0% = gałka puszczona.
PRESETS: dict[int, list[tuple[int, float]]] = {
    2: [(0, 1.0), (100, 2.5)],
    3: [(0, 1.0), (50, 2.0), (100, 3.0)],
    4: [(0, 1.0), (25, 1.5), (50, 2.5), (100, 4.0)],
    5: [(0, 1.0), (25, 1.5), (50, 2.5), (75, 3.5), (100, 4.5)],
}
MIN_GAP = 5  # skoki muszą różnić się co najmniej o tyle punktów procentowych


@dataclass(frozen=True)
class PadState:
    name: str
    x: float  # -1..1, w prawo dodatnie
    y: float  # -1..1, do przodu dodatnie

    @property
    def deflection(self) -> float:
        """Wychylenie 0..1 w dowolną stronę - postać idzie także w bok i do tyłu."""
        r = math.hypot(self.x, self.y)
        if r <= INNER_DEADZONE:
            return 0.0
        return min(1.0, (r - INNER_DEADZONE) / (OUTER_DEADZONE - INNER_DEADZONE))


# --- skoki ---------------------------------------------------------------------


def parse_steps(data: Any) -> list[tuple[int, float]]:
    """Skoki z ustawień; błędne - gotowe 4 skoki."""
    try:
        steps = [(int(at), round(float(speed), 1)) for at, speed in data]
    except (TypeError, ValueError):
        return list(PRESETS[4])
    if not STEPS_MIN <= len(steps) <= STEPS_MAX or steps[0][0] != 0:
        return list(PRESETS[4])
    for (a, _), (b, _) in itertools.pairwise(steps):
        if b - a < MIN_GAP or b > 100:
            return list(PRESETS[4])
    if any(not 0.5 <= speed <= 12.0 for _, speed in steps):
        return list(PRESETS[4])
    return steps


class StepPicker:
    """Który skok teraz obowiązuje. Przy cofaniu gałki mały zapas, żeby skok nie migał."""

    HYSTERESIS = 0.04

    def __init__(self) -> None:
        self.index = 0

    def update(self, steps: list[tuple[int, float]], deflection: float) -> int:
        self.index = min(self.index, len(steps) - 1)
        up = max(i for i, (at, _) in enumerate(steps) if deflection >= at / 100)
        if up >= self.index:
            self.index = up
        else:
            keep = max(
                i for i, (at, _) in enumerate(steps) if deflection >= at / 100 - self.HYSTERESIS
            )
            self.index = min(self.index, keep)
        return self.index


# --- odczyt pada ---------------------------------------------------------------


class Source(Protocol):
    name: str

    def read(self) -> PadState | None: ...


class XINPUT_GAMEPAD(ctypes.Structure):
    _fields_ = (
        ("wButtons", wintypes.WORD),
        ("bLeftTrigger", wintypes.BYTE),
        ("bRightTrigger", wintypes.BYTE),
        ("sThumbLX", wintypes.SHORT),
        ("sThumbLY", wintypes.SHORT),
        ("sThumbRX", wintypes.SHORT),
        ("sThumbRY", wintypes.SHORT),
    )


class XINPUT_STATE(ctypes.Structure):
    _fields_ = (("dwPacketNumber", wintypes.DWORD), ("Gamepad", XINPUT_GAMEPAD))


class JOYINFOEX(ctypes.Structure):
    _fields_ = (
        ("dwSize", wintypes.DWORD),
        ("dwFlags", wintypes.DWORD),
        ("dwXpos", wintypes.DWORD),
        ("dwYpos", wintypes.DWORD),
        ("dwZpos", wintypes.DWORD),
        ("dwRpos", wintypes.DWORD),
        ("dwUpos", wintypes.DWORD),
        ("dwVpos", wintypes.DWORD),
        ("dwButtons", wintypes.DWORD),
        ("dwButtonNumber", wintypes.DWORD),
        ("dwPOV", wintypes.DWORD),
        ("dwReserved1", wintypes.DWORD),
        ("dwReserved2", wintypes.DWORD),
    )


class JOYCAPSW(ctypes.Structure):
    _fields_ = (
        ("wMid", wintypes.WORD),
        ("wPid", wintypes.WORD),
        ("szPname", wintypes.WCHAR * 32),
        ("wXmin", wintypes.UINT),
        ("wXmax", wintypes.UINT),
        ("wYmin", wintypes.UINT),
        ("wYmax", wintypes.UINT),
        ("wZmin", wintypes.UINT),
        ("wZmax", wintypes.UINT),
        ("wNumButtons", wintypes.UINT),
        ("wPeriodMin", wintypes.UINT),
        ("wPeriodMax", wintypes.UINT),
        ("wRmin", wintypes.UINT),
        ("wRmax", wintypes.UINT),
        ("wUmin", wintypes.UINT),
        ("wUmax", wintypes.UINT),
        ("wVmin", wintypes.UINT),
        ("wVmax", wintypes.UINT),
        ("wCaps", wintypes.UINT),
        ("wMaxAxes", wintypes.UINT),
        ("wNumAxes", wintypes.UINT),
        ("wMaxButtons", wintypes.UINT),
        ("szRegKey", wintypes.WCHAR * 32),
        ("szOEMVxD", wintypes.WCHAR * 260),
    )


JOY_RETURNX = 0x01
JOY_RETURNY = 0x02
SONY = 0x054C
DUALSENSE = {0x0CE6: "DualSense", 0x0DF2: "DualSense Edge"}
DUALSHOCK = {0x05C4: "DualShock 4", 0x09CC: "DualShock 4"}
MICROSOFT = 0x045E


def _load(names: Iterable[str]) -> Any:
    for name in names:
        try:
            return ctypes.WinDLL(name)
        except OSError:
            continue
    return None


_xinput = _load(("xinput1_4", "xinput1_3", "xinput9_1_0")) if IS_WINDOWS else None
_winmm = _load(("winmm",)) if IS_WINDOWS else None
if _xinput is not None:
    _xinput.XInputGetState.argtypes = (wintypes.DWORD, ctypes.POINTER(XINPUT_STATE))
    _xinput.XInputGetState.restype = wintypes.DWORD
if _winmm is not None:
    _winmm.joyGetNumDevs.restype = wintypes.UINT
    _winmm.joyGetPosEx.argtypes = (wintypes.UINT, ctypes.POINTER(JOYINFOEX))
    _winmm.joyGetPosEx.restype = wintypes.UINT
    _winmm.joyGetDevCapsW.argtypes = (ctypes.c_size_t, ctypes.POINTER(JOYCAPSW), wintypes.UINT)
    _winmm.joyGetDevCapsW.restype = wintypes.UINT


def _axis(value: int, lo: int, hi: int) -> float:
    if hi <= lo:
        return 0.0
    return max(-1.0, min(1.0, (value - lo) / (hi - lo) * 2 - 1))


class XInputSource:
    def __init__(self, slot: int) -> None:
        self.slot = slot
        self.name = f"Pad Xbox (XInput {slot + 1})"

    def read(self) -> PadState | None:
        if _xinput is None:
            return None
        state = XINPUT_STATE()
        if _xinput.XInputGetState(self.slot, ctypes.byref(state)) != 0:
            return None
        g = state.Gamepad
        return PadState(self.name, g.sThumbLX / 32767, g.sThumbLY / 32767)


class WinmmSource:
    def __init__(self, joy_id: int, caps: JOYCAPSW) -> None:
        self.joy_id = joy_id
        self.caps = caps
        self.vendor = int(caps.wMid)
        if caps.wMid == SONY:
            self.name = DUALSENSE.get(caps.wPid) or DUALSHOCK.get(caps.wPid) or "Pad PlayStation"
        else:
            self.name = caps.szPname.strip() or f"Pad {joy_id + 1}"

    def read(self) -> PadState | None:
        if _winmm is None:
            return None
        info = JOYINFOEX()
        info.dwSize = ctypes.sizeof(JOYINFOEX)
        info.dwFlags = JOY_RETURNX | JOY_RETURNY
        if _winmm.joyGetPosEx(self.joy_id, ctypes.byref(info)) != 0:
            return None
        c = self.caps
        x = _axis(info.dwXpos, c.wXmin, c.wXmax)
        y = -_axis(info.dwYpos, c.wYmin, c.wYmax)  # w DirectInput oś Y rośnie w dół
        return PadState(self.name, x, y)


def discover() -> list[Source]:
    """Podłączone pady: najpierw XInput, potem pozostałe (DualSense i inne pady HID)."""
    found: list[Source] = []
    if _xinput is not None:
        for slot in range(4):
            source = XInputSource(slot)
            if source.read() is not None:
                found.append(source)
    if _winmm is not None:
        for joy_id in range(min(16, int(_winmm.joyGetNumDevs()))):
            caps = JOYCAPSW()
            if _winmm.joyGetDevCapsW(joy_id, ctypes.byref(caps), ctypes.sizeof(caps)) != 0:
                continue
            winmm_source = WinmmSource(joy_id, caps)
            if found and winmm_source.vendor == MICROSOFT:
                continue  # ten sam pad Xbox już jest przez XInput
            if winmm_source.read() is not None:
                found.append(winmm_source)
    return found


class PadReader:
    """Pierwszy podłączony pad. Bez pada sprawdza co RESCAN_S, nie co klatkę."""

    def __init__(
        self,
        find: Callable[[], list[Source]] = discover,
        clock: Callable[[], float] = time.monotonic,
    ) -> None:
        self._find = find
        self._clock = clock
        self._source: Source | None = None
        self._next_scan = 0.0

    @property
    def name(self) -> str | None:
        return self._source.name if self._source else None

    def read(self) -> PadState | None:
        if self._source is not None:
            state = self._source.read()
            if state is not None:
                return state
            log.info("Pad odłączony: %s", self._source.name)
            self._source = None
        now = self._clock()
        if now < self._next_scan:
            return None
        self._next_scan = now + RESCAN_S
        for source in self._find():
            state = source.read()
            if state is not None:
                self._source = source
                log.info("Pad: %s", source.name)
                return state
        return None
