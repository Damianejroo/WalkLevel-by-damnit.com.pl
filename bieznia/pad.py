"""Pad: wychylenie lewej gałki zamienione na skoki prędkości taśmy.

Odczyt bez dodatkowych bibliotek i bez fokusu (gra może być na wierzchu):
- HID - DualSense i DualShock 4 przez kabel i Bluetooth (raporty czytane wprost; przez
  Bluetooth, gdy Steam przełączy pada w pełny raport 0x31, winmm widzi zamrożoną gałkę),
- XInput - pady Xbox i wszystko, co Steam/DS4Windows udaje jako pad Xbox,
- winmm (DirectInput) - pozostałe pady HID.
Program tylko czyta pozycję gałki i przyciski, niczego nie wysyła do gry.
"""

from __future__ import annotations

import ctypes
import itertools
import logging
import math
import threading
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


# Przyciski pod wspólnymi nazwami (układ jak na padach: dół/prawo/lewo/góra) i ich podpisy.
BUTTONS_SONY = {
    "south": "Krzyżyk",
    "east": "Kółko",
    "west": "Kwadrat",
    "north": "Trójkąt",
    "l1": "L1",
    "r1": "R1",
    "l2": "L2",
    "r2": "R2",
    "select": "Create",
    "start": "Options",
    "l3": "L3 (lewa gałka)",
    "r3": "R3 (prawa gałka)",
    "home": "PS",
    "touchpad": "Panel dotykowy",
    "up": "Strzałka w górę",
    "down": "Strzałka w dół",
    "left": "Strzałka w lewo",
    "right": "Strzałka w prawo",
}
BUTTONS_XBOX = {
    **BUTTONS_SONY,
    "south": "A",
    "east": "B",
    "west": "X",
    "north": "Y",
    "l1": "LB",
    "r1": "RB",
    "l2": "LT",
    "r2": "RT",
    "select": "View",
    "start": "Menu",
    "l3": "LS (lewa gałka)",
    "r3": "RS (prawa gałka)",
    "home": "Xbox",
}
# Kolejność przy przypisywaniu (gdy naraz kilka): najpierw te rzadko używane w grach.
BUTTON_ORDER = (
    "touchpad",
    "select",
    "home",
    "l3",
    "r3",
    "start",
    *(f"b{i}" for i in range(1, 33)),
    "up",
    "down",
    "left",
    "right",
    "l1",
    "r1",
    "l2",
    "r2",
    "west",
    "north",
    "east",
    "south",
)


def button_label(button: str, pad_name: str | None = None) -> str:
    """Podpis przycisku jak na padzie (Krzyżyk na DualSense, A na padzie Xbox)."""
    if button.startswith("b") and button[1:].isdigit():
        return f"Przycisk {button[1:]}"
    sony = pad_name is None or any(w in pad_name for w in ("DualSense", "DualShock", "PlayStation"))
    names = BUTTONS_SONY if sony else BUTTONS_XBOX
    return names.get(button, button)


@dataclass(frozen=True)
class PadState:
    name: str
    x: float  # -1..1, w prawo dodatnie
    y: float  # -1..1, do przodu dodatnie
    buttons: frozenset[str] = frozenset()  # wciśnięte teraz (nazwy jak w BUTTONS_SONY)

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
JOY_RETURNBUTTONS = 0x80
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
        return PadState(self.name, g.sThumbLX / 32767, g.sThumbLY / 32767, xinput_buttons(g))


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
        info.dwFlags = JOY_RETURNX | JOY_RETURNY | JOY_RETURNBUTTONS
        if _winmm.joyGetPosEx(self.joy_id, ctypes.byref(info)) != 0:
            return None
        c = self.caps
        x = _axis(info.dwXpos, c.wXmin, c.wXmax)
        y = -_axis(info.dwYpos, c.wYmin, c.wYmax)  # w DirectInput oś Y rośnie w dół
        pressed = frozenset(f"b{i + 1}" for i in range(32) if info.dwButtons & (1 << i))
        return PadState(self.name, x, y, pressed)


# --- pady Sony wprost przez HID -------------------------------------------------

BT_HID = "00001124-0000-1000-8000-00805f9b34fb"  # usługa HID Bluetooth w ścieżce urządzenia


def parse_sony(report: bytes) -> tuple[float, float] | None:
    """Lewa gałka z raportu DualSense/DualShock 4 (USB i oba tryby Bluetooth)."""
    if not report:
        return None
    rid = report[0]
    if rid == 0x01 and len(report) >= 3:  # USB, albo Bluetooth w trybie podstawowym
        lx, ly = report[1], report[2]
    elif rid == 0x31 and len(report) >= 4:  # DualSense, Bluetooth, pełny raport
        lx, ly = report[2], report[3]
    elif rid == 0x11 and len(report) >= 5:  # DualShock 4, Bluetooth, pełny raport
        lx, ly = report[3], report[4]
    else:
        return None
    x = max(-1.0, min(1.0, (lx - 127.5) / 127.5))
    y = max(-1.0, min(1.0, (127.5 - ly) / 127.5))  # w raporcie oś Y rośnie w dół
    return x, y


_HAT = {
    0: ("up",),
    1: ("up", "right"),
    2: ("right",),
    3: ("down", "right"),
    4: ("down",),
    5: ("down", "left"),
    6: ("left",),
    7: ("up", "left"),
}
_SONY_B0 = ((0x10, "west"), (0x20, "south"), (0x40, "east"), (0x80, "north"))
_SONY_B1 = (
    (0x01, "l1"),
    (0x02, "r1"),
    (0x04, "l2"),
    (0x08, "r2"),
    (0x10, "select"),
    (0x20, "start"),
    (0x40, "l3"),
    (0x80, "r3"),
)
_SONY_B2 = ((0x01, "home"), (0x02, "touchpad"))


def parse_sony_buttons(report: bytes, dualsense: bool) -> frozenset[str]:
    """Przyciski z raportu DualSense/DualShock 4. Układ bajtów zależy od modelu i trybu:
    DualSense USB i Bluetooth (pełny raport) mają przyciski za spustami, DualShock 4 i
    podstawowy raport Bluetooth - zaraz za gałkami."""
    if not report:
        return frozenset()
    rid = report[0]
    if rid == 0x01:
        start = 8 if dualsense and len(report) > 20 else 5
    elif rid == 0x31:
        start = 9
    elif rid == 0x11:
        start = 8
    else:
        return frozenset()
    if len(report) < start + 3:
        return frozenset()
    b0, b1, b2 = report[start], report[start + 1], report[start + 2]
    pressed = list(_HAT.get(b0 & 0x0F, ()))
    pressed += [name for bit, name in _SONY_B0 if b0 & bit]
    pressed += [name for bit, name in _SONY_B1 if b1 & bit]
    pressed += [name for bit, name in _SONY_B2 if b2 & bit]
    return frozenset(pressed)


_XINPUT_BITS = (
    (0x0001, "up"),
    (0x0002, "down"),
    (0x0004, "left"),
    (0x0008, "right"),
    (0x0010, "start"),
    (0x0020, "select"),
    (0x0040, "l3"),
    (0x0080, "r3"),
    (0x0100, "l1"),
    (0x0200, "r1"),
    (0x1000, "south"),
    (0x2000, "east"),
    (0x4000, "west"),
    (0x8000, "north"),
)


def xinput_buttons(g: Any) -> frozenset[str]:
    pressed = [name for bit, name in _XINPUT_BITS if g.wButtons & bit]
    if (g.bLeftTrigger & 0xFF) > 128:
        pressed.append("l2")
    if (g.bRightTrigger & 0xFF) > 128:
        pressed.append("r2")
    return frozenset(pressed)


class GUID(ctypes.Structure):
    _fields_ = (
        ("d1", wintypes.DWORD),
        ("d2", wintypes.WORD),
        ("d3", wintypes.WORD),
        ("d4", ctypes.c_ubyte * 8),
    )


class SP_DEVICE_INTERFACE_DATA(ctypes.Structure):
    _fields_ = (
        ("cbSize", wintypes.DWORD),
        ("guid", GUID),
        ("flags", wintypes.DWORD),
        ("reserved", ctypes.c_size_t),
    )


class HIDD_ATTRIBUTES(ctypes.Structure):
    _fields_ = (
        ("Size", wintypes.ULONG),
        ("VendorID", wintypes.USHORT),
        ("ProductID", wintypes.USHORT),
        ("Version", wintypes.USHORT),
    )


GENERIC_READ = 0x80000000
SHARE_READ_WRITE = 0x3  # Steam i gra czytają pada równolegle - nie zabieramy go nikomu
OPEN_EXISTING = 3
DIGCF_PRESENT_INTERFACE = 0x12
INVALID_HANDLE = ctypes.c_void_p(-1).value

_setupapi = _load(("setupapi",)) if IS_WINDOWS else None
_hid = _load(("hid",)) if IS_WINDOWS else None
_k32 = ctypes.WinDLL("kernel32", use_last_error=True) if IS_WINDOWS else None
if _setupapi is not None and _hid is not None and _k32 is not None:
    _setupapi.SetupDiGetClassDevsW.restype = ctypes.c_void_p
    _setupapi.SetupDiGetClassDevsW.argtypes = (
        ctypes.POINTER(GUID),
        wintypes.LPCWSTR,
        wintypes.HWND,
        wintypes.DWORD,
    )
    _setupapi.SetupDiEnumDeviceInterfaces.argtypes = (
        ctypes.c_void_p,
        ctypes.c_void_p,
        ctypes.POINTER(GUID),
        wintypes.DWORD,
        ctypes.POINTER(SP_DEVICE_INTERFACE_DATA),
    )
    _setupapi.SetupDiGetDeviceInterfaceDetailW.argtypes = (
        ctypes.c_void_p,
        ctypes.POINTER(SP_DEVICE_INTERFACE_DATA),
        ctypes.c_void_p,
        wintypes.DWORD,
        ctypes.POINTER(wintypes.DWORD),
        ctypes.c_void_p,
    )
    _setupapi.SetupDiDestroyDeviceInfoList.argtypes = (ctypes.c_void_p,)
    _hid.HidD_GetHidGuid.argtypes = (ctypes.POINTER(GUID),)
    _hid.HidD_GetAttributes.argtypes = (wintypes.HANDLE, ctypes.POINTER(HIDD_ATTRIBUTES))
    _k32.CreateFileW.restype = wintypes.HANDLE
    _k32.CreateFileW.argtypes = (
        wintypes.LPCWSTR,
        wintypes.DWORD,
        wintypes.DWORD,
        ctypes.c_void_p,
        wintypes.DWORD,
        wintypes.DWORD,
        wintypes.HANDLE,
    )
    _k32.ReadFile.argtypes = (
        wintypes.HANDLE,
        ctypes.c_void_p,
        wintypes.DWORD,
        ctypes.POINTER(wintypes.DWORD),
        ctypes.c_void_p,
    )
    _k32.CloseHandle.argtypes = (wintypes.HANDLE,)
    _k32.CancelIoEx.argtypes = (wintypes.HANDLE, ctypes.c_void_p)


def _open(path: str, access: int) -> int | None:
    assert _k32 is not None
    handle = _k32.CreateFileW(path, access, SHARE_READ_WRITE, None, OPEN_EXISTING, 0, None)
    return None if handle in (None, 0, INVALID_HANDLE) else int(handle)


def sony_hid_paths() -> list[tuple[str, int]]:
    """Ścieżki urządzeń HID firmy Sony (pady) i ich PID."""
    if _setupapi is None or _hid is None or _k32 is None:
        return []
    guid = GUID()
    _hid.HidD_GetHidGuid(ctypes.byref(guid))
    info = _setupapi.SetupDiGetClassDevsW(ctypes.byref(guid), None, None, DIGCF_PRESENT_INTERFACE)
    if not info or info == INVALID_HANDLE:
        return []
    found: list[tuple[str, int]] = []
    try:
        for index in itertools.count():
            data = SP_DEVICE_INTERFACE_DATA()
            data.cbSize = ctypes.sizeof(data)
            if not _setupapi.SetupDiEnumDeviceInterfaces(
                info, None, ctypes.byref(guid), index, ctypes.byref(data)
            ):
                break
            need = wintypes.DWORD()
            _setupapi.SetupDiGetDeviceInterfaceDetailW(
                info, ctypes.byref(data), None, 0, ctypes.byref(need), None
            )
            if need.value <= 4:
                continue
            buf = ctypes.create_string_buffer(need.value)
            # cbSize struktury SP_DEVICE_INTERFACE_DETAIL_DATA_W (DWORD + WCHAR[1], z wyrównaniem)
            ctypes.cast(buf, ctypes.POINTER(wintypes.DWORD))[0] = (
                8 if ctypes.sizeof(ctypes.c_void_p) == 8 else 6
            )
            if not _setupapi.SetupDiGetDeviceInterfaceDetailW(
                info, ctypes.byref(data), buf, need, None, None
            ):
                continue
            path = ctypes.wstring_at(ctypes.addressof(buf) + 4)
            if "vid_054c" not in path.lower() and "vid&0002054c" not in path.lower():
                continue  # nie Sony - nie otwieramy cudzych urządzeń
            handle = _open(path, 0)
            if handle is None:
                continue
            attrs = HIDD_ATTRIBUTES()
            attrs.Size = ctypes.sizeof(attrs)
            ok = _hid.HidD_GetAttributes(handle, ctypes.byref(attrs))
            _k32.CloseHandle(handle)
            if (
                ok
                and attrs.VendorID == SONY
                and (attrs.ProductID in DUALSENSE or attrs.ProductID in DUALSHOCK)
            ):
                found.append((path, int(attrs.ProductID)))
    finally:
        _setupapi.SetupDiDestroyDeviceInfoList(info)
    return found


class HidSource:
    """Pad Sony czytany w osobnym wątku (pad sam wysyła raporty setki razy na sekundę)."""

    STALE_S = 1.0
    FIRST_REPORT_S = 0.3

    def __init__(self, path: str, pid: int) -> None:
        self.path = path
        model = DUALSENSE.get(pid) or DUALSHOCK.get(pid) or "Pad PlayStation"
        self.name = f"{model} ({'Bluetooth' if BT_HID in path.lower() else 'USB'})"
        self.dualsense = pid in DUALSENSE
        self._lock = threading.Lock()
        self._xy: tuple[float, float] | None = None
        self._buttons: frozenset[str] = frozenset()
        self._at = float("-inf")
        self._first = threading.Event()
        self._stop = False
        self._handle: int | None = None

    def start(self) -> bool:
        if _k32 is None:
            return False
        self._handle = _open(self.path, GENERIC_READ)
        if self._handle is None:
            log.info("Nie da się czytać pada %s (zajęty na wyłączność?)", self.name)
            return False
        threading.Thread(target=self._run, name="pad-hid", daemon=True).start()
        if self._first.wait(self.FIRST_REPORT_S):
            return True
        self.stop()
        return False

    def stop(self) -> None:
        self._stop = True
        if self._handle is not None and _k32 is not None:
            _k32.CancelIoEx(self._handle, None)

    def _run(self) -> None:
        assert _k32 is not None and self._handle is not None
        buf = ctypes.create_string_buffer(1024)
        got = wintypes.DWORD()
        try:
            while not self._stop:
                if not _k32.ReadFile(self._handle, buf, len(buf), ctypes.byref(got), None):
                    break  # pad odłączony albo odczyt przerwany
                report = buf.raw[: got.value]
                xy = parse_sony(report)
                if xy is None:
                    continue
                buttons = parse_sony_buttons(report, self.dualsense)
                with self._lock:
                    self._xy, self._buttons, self._at = xy, buttons, time.monotonic()
                self._first.set()
        finally:
            _k32.CloseHandle(self._handle)
            with self._lock:
                self._at = float("-inf")

    def read(self) -> PadState | None:
        with self._lock:
            xy, buttons, at = self._xy, self._buttons, self._at
        if xy is None or time.monotonic() - at > self.STALE_S:
            return None
        return PadState(self.name, xy[0], xy[1], buttons)


def discover() -> list[Source]:
    """Podłączone pady: Sony przez HID, potem XInput, potem pozostałe przez winmm."""
    found: list[Source] = []
    for path, pid in sony_hid_paths():
        hid_source = HidSource(path, pid)
        if hid_source.start():
            found.append(hid_source)
    sony_hid = bool(found)
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
            if sony_hid and winmm_source.vendor == SONY:
                continue  # ten sam pad Sony już jest przez HID (pewniej, także przez BT)
            if winmm_source.read() is not None:
                found.append(winmm_source)
    return found


class PadReader:
    """Pierwszy podłączony pad. Bez pada sprawdza co RESCAN_S, nie co klatkę.

    background=True: szukanie (10-40 ms: SetupAPI, winmm) idzie w osobnym wątku, żeby nie
    przycinać okna; wynik odbiera następne read().
    """

    def __init__(
        self,
        find: Callable[[], list[Source]] = discover,
        clock: Callable[[], float] = time.monotonic,
        background: bool = False,
    ) -> None:
        self._find = find
        self._clock = clock
        self._background = background
        self._source: Source | None = None
        self._next_scan = 0.0
        self._found: list[Source] | None = None
        self._scanning = False

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
        found, self._found = self._found, None
        if found is not None:
            return self._choose(found)
        now = self._clock()
        if now < self._next_scan or self._scanning:
            return None
        self._next_scan = now + RESCAN_S
        if self._background:
            self._scanning = True
            threading.Thread(target=self._scan, name="pad-szukanie", daemon=True).start()
            return None
        return self._choose(self._find())

    def _scan(self) -> None:
        try:
            self._found = self._find()
        except Exception:
            log.exception("Szukanie pada nie powiodło się")
            self._found = []
        finally:
            self._scanning = False

    def _choose(self, found: list[Source]) -> PadState | None:
        chosen: PadState | None = None
        for source in found:
            state = source.read() if chosen is None else None
            if state is not None:
                self._source, chosen = source, state
                log.info("Pad: %s", source.name)
            elif isinstance(source, HidSource):
                source.stop()  # nieużywany - nie trzymamy wątku
        return chosen
