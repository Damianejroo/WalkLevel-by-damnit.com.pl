"""Cienka warstwa na Windows API (user32) przez ctypes: stan klawiszy i SendInput."""

from __future__ import annotations

import ctypes
import sys
from ctypes import wintypes

IS_WINDOWS = sys.platform == "win32"

INPUT_KEYBOARD = 1
KEYEVENTF_EXTENDEDKEY = 0x0001
KEYEVENTF_KEYUP = 0x0002
KEYEVENTF_SCANCODE = 0x0008

# Znacznik w dwExtraInfo, po którym da się rozpoznać nasze wstrzyknięte klawisze.
INJECT_TAG = 0xB1E5


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
    _user32.MapVirtualKeyW.argtypes = (wintypes.UINT, wintypes.UINT)
    _user32.MapVirtualKeyW.restype = wintypes.UINT

MAPVK_VSC_TO_VK_EX = 3


def is_key_down(vk: int) -> bool:
    """Czy klawisz (kod VK) jest teraz wciśnięty - działa też, gdy okno nie ma fokusu."""
    if not IS_WINDOWS:
        return False
    return bool(_user32.GetAsyncKeyState(vk) & 0x8000)


def vk_from_scan(scan: int, extended: bool = False) -> int:
    """Kod wirtualny dla kodu skanowania (lewy/prawy Shift, Ctrl i Alt rozróżnione)."""
    if not IS_WINDOWS:
        return 0
    code = (0xE000 | scan) if extended else scan
    return int(_user32.MapVirtualKeyW(code, MAPVK_VSC_TO_VK_EX))


def key_input(scan: int, down: bool, extended: bool = False) -> INPUT:
    """Zdarzenie klawiatury dla SendInput.

    Uwaga: dane klawisza MUSZĄ trafić do unii (u.ki). Zapis INPUT(ki=...) tworzy tylko
    atrybut Pythona - Windows dostaje wtedy puste zdarzenie, zgłasza sukces i nic się
    nie dzieje (tak było do wersji 0.2.2).
    """
    flags = KEYEVENTF_SCANCODE | (0 if down else KEYEVENTF_KEYUP)
    if extended:  # strzałki, prawy Ctrl/Alt, Insert, Home...
        flags |= KEYEVENTF_EXTENDEDKEY
    ki = KEYBDINPUT(wVk=0, wScan=scan, dwFlags=flags, time=0, dwExtraInfo=INJECT_TAG)
    return INPUT(type=INPUT_KEYBOARD, u=_INPUTUNION(ki=ki))


def send_scancode(scan: int, down: bool, extended: bool = False) -> None:
    """Naciska/puszcza klawisz po kodzie skanowania (gry z DirectInput go widzą)."""
    if not IS_WINDOWS:
        return
    event = key_input(scan, down, extended)
    if _user32.SendInput(1, ctypes.byref(event), ctypes.sizeof(INPUT)) != 1:
        raise ctypes.WinError(ctypes.get_last_error())


HWND_TOPMOST = -1
HWND_NOTOPMOST = -2
SWP_NOSIZE, SWP_NOMOVE, SWP_NOACTIVATE = 0x0001, 0x0002, 0x0010
GWL_EXSTYLE = -20
WS_EX_TOPMOST = 0x00000008

if IS_WINDOWS:
    # Pełne typy: bez nich HWND_TOPMOST (-1) trafiał do Windows jako 32-bitowa liczba,
    # czyli błędny uchwyt, i „zawsze na wierzchu” nic nie robiło.
    _user32.SetWindowPos.argtypes = (
        wintypes.HWND,
        wintypes.HWND,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_int,
        wintypes.UINT,
    )
    _user32.SetWindowPos.restype = wintypes.BOOL
    _user32.GetWindowLongPtrW.argtypes = (wintypes.HWND, ctypes.c_int)
    _user32.GetWindowLongPtrW.restype = ctypes.c_ssize_t


def set_topmost(hwnd: int, on: bool) -> bool:
    """Okno zawsze na wierzchu (albo z powrotem zwykłe) bez ukrywania go."""
    if not IS_WINDOWS:
        return False
    after = wintypes.HWND(HWND_TOPMOST if on else HWND_NOTOPMOST)
    flags = SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE
    return bool(_user32.SetWindowPos(wintypes.HWND(hwnd), after, 0, 0, 0, 0, flags))


def is_topmost(hwnd: int) -> bool:
    if not IS_WINDOWS:
        return False
    return bool(_user32.GetWindowLongPtrW(wintypes.HWND(hwnd), GWL_EXSTYLE) & WS_EX_TOPMOST)
