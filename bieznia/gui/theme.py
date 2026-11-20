"""Tokeny wyglądu (zasady Warsztatu), motywy jasny/ciemny, styl QSS i efekty Windows.

Kolory, odstępy i promienie biorą się wyłącznie z tokenów. Progi kontrastu
(tekst 7:1, pomocniczy 4,5:1, ramka kontrolki 3:1) pilnuje tests/test_theme.py.
"""

from __future__ import annotations

import ctypes
import sys
from dataclasses import dataclass

RADIUS = 10
CONTROL_H = 44
GAP = 12
PAD = 16
FOCUS_W = 2
FOCUS_OFFSET = 2

# Czasy animacji (ms) i krzywe z zasad Warsztatu
T_COLOR = 120
T_PRESS = 80
T_PANEL_IN = 220
T_PANEL_OUT = 180
EASE_IN = (0.2, 0.8, 0.2, 1.0)  # wejście
EASE_OUT = (0.4, 0.0, 1.0, 1.0)  # wyjście


@dataclass(frozen=True)
class Theme:
    name: str
    bg: str
    surface: str
    surface_2: str  # najechanie / wciśnięte tło kontrolki
    text: str
    text_2: str
    line: str  # tylko ozdoby, nigdy ramka kontrolki
    control: str  # ramka kontrolki (>= 3:1 do tła)
    accent: str
    on_accent: str
    danger: str
    run: str  # bieg
    contrast_bg: str  # przycisk „kontrast”: odwrócone kolory
    contrast_fg: str
    backdrop_alpha: float  # krycie tła okna nad materiałem Windows


DARK = Theme(
    name="dark",
    bg="#151514",
    surface="#1e1e1c",
    surface_2="#2a2a27",
    text="#ecebe6",
    text_2="#a5a29a",
    line="#33322e",
    control="#7a786f",
    accent="#8fc2b0",
    on_accent="#0f1f1a",
    danger="#f08a84",
    run="#e6b46e",
    contrast_bg="#ecebe6",
    contrast_fg="#151514",
    backdrop_alpha=0.82,
)

LIGHT = Theme(
    name="light",
    bg="#fbfaf7",
    surface="#ffffff",
    surface_2="#f0eee8",
    text="#1b1b19",
    text_2="#5c5951",
    line="#e4e1d8",
    control="#8a877e",
    accent="#2f5d50",
    on_accent="#ffffff",
    danger="#a4282a",
    run="#8a5410",
    contrast_bg="#1b1b19",
    contrast_fg="#fbfaf7",
    backdrop_alpha=0.86,
)

FONT = "Segoe UI Variable Text"
FONT_FALLBACK = "Segoe UI"
ICON_FONT = "Segoe Fluent Icons"
ICON_FALLBACK = "Segoe MDL2 Assets"

ICONS = {
    "settings": "",
    "back": "",
    "refresh": "",
    "bluetooth": "",
    "log": "",
    "folder": "",
    "report": "",
    "keyboard": "",
    "eye": "",
    "walk": "",
}


# --- kontrast (WCAG) --------------------------------------------------------


def _channel(c: float) -> float:
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def luminance(hex_color: str) -> float:
    h = hex_color.lstrip("#")
    r, g, b = (int(h[i : i + 2], 16) / 255 for i in (0, 2, 4))
    return 0.2126 * _channel(r) + 0.7152 * _channel(g) + 0.0722 * _channel(b)


def contrast(a: str, b: str) -> float:
    la, lb = sorted((luminance(a), luminance(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)


# --- system Windows ---------------------------------------------------------


def system_prefers_light() -> bool:
    if sys.platform != "win32":
        return False
    try:
        import winreg

        key = winreg.OpenKey(
            winreg.HKEY_CURRENT_USER,
            r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
        )
        value, _ = winreg.QueryValueEx(key, "AppsUseLightTheme")
        return bool(value)
    except OSError:
        return False


def system_reduces_motion() -> bool:
    """Windows: Ustawienia > Ułatwienia dostępu > Efekty animacji = wyłączone."""
    if sys.platform != "win32":
        return False
    enabled = ctypes.c_int(1)
    SPI_GETCLIENTAREAANIMATION = 0x1042
    ok = ctypes.windll.user32.SystemParametersInfoW(
        SPI_GETCLIENTAREAANIMATION, 0, ctypes.byref(enabled), 0
    )
    return bool(ok) and not enabled.value


def resolve_theme(choice: str) -> Theme:
    if choice == "light":
        return LIGHT
    if choice == "dark":
        return DARK
    return LIGHT if system_prefers_light() else DARK


class _Margins(ctypes.Structure):
    _fields_ = (("l", ctypes.c_int), ("r", ctypes.c_int), ("t", ctypes.c_int), ("b", ctypes.c_int))


def apply_backdrop(hwnd: int, dark: bool) -> bool:
    """Materiał systemowy Windows 11 (Mica) pod oknem + ciemny/jasny pasek tytułu.

    Zwraca False na starszych Windows - wtedy okno ma po prostu pełne tło.
    """
    if sys.platform != "win32":
        return False
    try:
        dwm = ctypes.windll.dwmapi
        value = ctypes.c_int(1 if dark else 0)
        dwm.DwmSetWindowAttribute(hwnd, 20, ctypes.byref(value), 4)  # DWMWA_USE_IMMERSIVE_DARK_MODE
        corner = ctypes.c_int(2)  # DWMWCP_ROUND
        dwm.DwmSetWindowAttribute(hwnd, 33, ctypes.byref(corner), 4)
        backdrop = ctypes.c_int(2)  # DWMSBT_MAINWINDOW (Mica)
        hr = dwm.DwmSetWindowAttribute(hwnd, 38, ctypes.byref(backdrop), 4)
        if hr != 0:
            return False
        margins = _Margins(-1, -1, -1, -1)
        result: int = dwm.DwmExtendFrameIntoClientArea(hwnd, ctypes.byref(margins))
        return result == 0
    except (AttributeError, OSError):
        return False


def qss(t: Theme, scale: float = 1.0) -> str:
    """Arkusz stylów. Najechanie i fokus zmieniają wyłącznie kolory."""

    def px(v: float) -> str:
        return f"{round(v * scale)}px"

    return f"""
* {{ font-family: "{FONT}", "{FONT_FALLBACK}"; font-size: {px(16)}; color: {t.text}; }}
QWidget#page, QWidget#root {{ background: transparent; }}
QFrame#card {{ background: {t.surface}; border: 1px solid {t.line}; border-radius: {RADIUS}px; }}
QLabel {{ background: transparent; }}
QLabel#muted {{ color: {t.text_2}; font-size: {px(14)}; }}
QLabel#small {{ color: {t.text_2}; font-size: {px(12)}; }}
QLabel#title {{ font-size: {px(20)}; font-weight: 600; }}
QLabel#section {{ color: {t.text_2}; font-size: {px(13)}; font-weight: 600; }}
QLabel#statValue {{ font-size: {px(20)}; font-weight: 600; }}
QLabel#danger {{ color: {t.danger}; font-size: {px(14)}; }}
QPushButton {{ border-radius: {RADIUS}px; padding: 0 {px(16)}; font-weight: 600;
    background: {t.surface}; border: 2px solid {t.control}; color: {t.text}; }}
QPushButton:hover {{ background: {t.surface_2}; }}
QPushButton:pressed {{ background: {t.line}; }}
QPushButton:disabled {{ color: {t.text_2}; border-color: {t.line}; }}
QPushButton#primary {{ background: {t.accent}; border-color: {t.accent}; color: {t.on_accent}; }}
QPushButton#primary:hover {{ background: {t.text}; border-color: {t.text}; color: {t.bg}; }}
QPushButton#contrast {{ background: {t.contrast_bg}; border-color: {t.contrast_bg};
    color: {t.contrast_fg}; }}
QPushButton#icon {{ font-family: "{ICON_FONT}", "{ICON_FALLBACK}"; font-size: {px(16)};
    font-weight: 400; padding: 0; border-color: transparent; background: transparent; }}
QPushButton#icon:hover {{ background: {t.surface_2}; border-color: {t.control}; }}
QPushButton#row {{ text-align: left; padding: 0 {px(14)}; font-weight: 400; }}
QPushButton#seg {{ border: none; border-radius: {RADIUS - 3}px; background: transparent;
    color: {t.text_2}; font-weight: 600; padding: 0 {px(10)}; }}
QPushButton#seg:hover:!checked {{ color: {t.text}; }}
QPushButton#seg:checked {{ background: {t.accent}; color: {t.on_accent}; }}
QFrame#segbox {{ background: {t.bg}; border: 2px solid {t.control}; border-radius: {RADIUS}px; }}
QPlainTextEdit {{ background: {t.bg}; border: 2px solid {t.control}; border-radius: {RADIUS}px;
    font-family: "Cascadia Mono", "Consolas"; font-size: {px(13)}; padding: 6px; }}
QSlider {{ min-height: {CONTROL_H}px; }}
QSlider::groove:horizontal {{ height: 6px; background: {t.line}; border: 1px solid {t.control};
    border-radius: 3px; }}
QSlider::sub-page:horizontal {{ background: {t.accent}; border-radius: 3px; }}
QSlider::handle:horizontal {{ background: {t.text}; width: 22px; height: 22px; margin: -9px 0;
    border-radius: 11px; border: 2px solid {t.bg}; }}
QScrollArea, QScrollArea > QWidget > QWidget {{ background: transparent; border: none; }}
QScrollBar:vertical {{ background: transparent; width: 8px; margin: 4px 0; }}
QScrollBar::handle:vertical {{ background: {t.control}; border-radius: 4px; min-height: 30px; }}
QScrollBar::add-line, QScrollBar::sub-line, QScrollBar::add-page, QScrollBar::sub-page
    {{ height: 0; background: none; }}
QToolTip {{ background: {t.surface}; color: {t.text}; border: 1px solid {t.control};
    border-radius: 6px; padding: 6px 8px; font-size: {px(13)}; }}
"""
