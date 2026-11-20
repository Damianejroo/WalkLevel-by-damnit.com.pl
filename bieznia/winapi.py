"""Cienka warstwa na Windows API (user32) przez ctypes: stan klawiszy i SendInput."""

from __future__ import annotations

import ctypes
import sys
from ctypes import wintypes

IS_WINDOWS = sys.platform == "win32"

INPUT_KEYBOARD = 1
KEYEVENTF_KEYUP = 0x0002
KEYEVENTF_SCANCODE = 0x0008

# Znacznik w dwExtraInfo, po którym da się rozpoznać nasze wstrzyknięte klawisze.
INJECT_TAG = 0xB1E5

# Kody skanowania klawiszy, którymi most steruje postacią.
SCAN_W = 0x11
SCAN_LSHIFT = 0x2A
MOVEMENT_SCANCODES = (SCAN_W, SCAN_LSHIFT)

ULONG_PTR = ctypes.c_size_t


class KEYBDINPUT(ctypes.Structure):
    _fields_ = (
        ("wVk", wintypes.WORD),
        ("wScan", wintypes.WORD),
        ("dwFlags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ULONG_PTR),
    )


class MOUSEINPUT(ctypes.Structure):
    _fields_ = (
        ("dx", wintypes.LONG),
        ("dy", wintypes.LONG),
        ("mouseData", wintypes.DWORD),
        ("dwFlags", wintypes.DWORD),
        ("time", wintypes.DWORD),
        ("dwExtraInfo", ULONG_PTR),
    )


class HARDWAREINPUT(ctypes.Structure):
    _fields_ = (
        ("uMsg", wintypes.DWORD),
        ("wParamL", wintypes.WORD),
        ("wParamH", wintypes.WORD),
    )


class _INPUTUNION(ctypes.Union):
    _fields_ = (("mi", MOUSEINPUT), ("ki", KEYBDINPUT), ("hi", HARDWAREINPUT))


class INPUT(ctypes.Structure):
    _fields_ = (("type", wintypes.DWORD), ("u", _INPUTUNION))


if IS_WINDOWS:
    _user32 = ctypes.WinDLL("user32", use_last_error=True)
    _user32.SendInput.argtypes = (wintypes.UINT, ctypes.POINTER(INPUT), ctypes.c_int)
    _user32.SendInput.restype = wintypes.UINT
    _user32.GetAsyncKeyState.argtypes = (ctypes.c_int,)
    _user32.GetAsyncKeyState.restype = ctypes.c_short


def is_key_down(vk: int) -> bool:
    """Czy klawisz (kod VK) jest teraz wciśnięty - działa też, gdy okno nie ma fokusu."""
    if not IS_WINDOWS:
        return False
    return bool(_user32.GetAsyncKeyState(vk) & 0x8000)


def send_scancode(scan: int, down: bool) -> None:
    """Naciska/puszcza klawisz po kodzie skanowania (gry z DirectInput go widzą)."""
    if not IS_WINDOWS:
        return
    flags = KEYEVENTF_SCANCODE | (0 if down else KEYEVENTF_KEYUP)
    event = INPUT(
        type=INPUT_KEYBOARD,
        ki=KEYBDINPUT(wVk=0, wScan=scan, dwFlags=flags, time=0, dwExtraInfo=INJECT_TAG),
    )
    if _user32.SendInput(1, ctypes.byref(event), ctypes.sizeof(INPUT)) != 1:
        raise ctypes.WinError(ctypes.get_last_error())


def release_movement_keys() -> None:
    """Bezpiecznik: puszcza W i Shift bez względu na to, kto je wcisnął."""
    for scan in MOVEMENT_SCANCODES:
        send_scancode(scan, down=False)
