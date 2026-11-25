"""Popupy osiągnięć i awansów na pulpicie - jak na Steamie: prawy dolny róg ekranu z grą.

Okienka nie zabierają fokusu i nie przyjmują kliknięć (przechodzą do gry), kilka naraz układa
się w stos, każde znika po kilku sekundach. Gra w trybie wyłącznego pełnego ekranu może je
przykryć - wtedy zdobycz i tak czeka na liście osiągnięć.
"""

from __future__ import annotations

import logging
import sys
from dataclasses import dataclass
from pathlib import Path

from PySide6.QtCore import QEasingCurve, QPropertyAnimation, QRectF, Qt, QTimer
from PySide6.QtGui import QColor, QGuiApplication, QLinearGradient, QPainter, QPaintEvent, QPen
from PySide6.QtWidgets import QHBoxLayout, QLabel, QVBoxLayout, QWidget

from bieznia import winapi
from bieznia.gui.odznaki import NUM, RARITY_COLORS, Hex, XPBar, caps, num_font
from bieznia.gui.theme import Theme

log = logging.getLogger(__name__)

SHOW_MS = 6000
FADE_MS = 220
MAX_VISIBLE = 3
WIDTH, HEIGHT = 380, 96
MARGIN, GAP = 16, 10
POPUP_BG = "#15171d"  # ciemny niezależnie od motywu - ma być czytelny nad każdą grą


@dataclass(frozen=True)
class PopupData:
    kicker: str  # np. „OSIĄGNIĘCIE · EPICKIE”
    title: str
    line: str
    color: str
    rarity: str = "epickie"
    mark: str = ""
    badge_text: str | None = None  # liczba poziomu zamiast znaku
    progress: float | None = None  # pasek do następnego poziomu


class Popup(QWidget):
    def __init__(self, t: Theme, data: PopupData) -> None:
        super().__init__(
            None,
            Qt.WindowType.FramelessWindowHint
            | Qt.WindowType.WindowStaysOnTopHint
            | Qt.WindowType.Tool
            | Qt.WindowType.WindowDoesNotAcceptFocus
            | Qt.WindowType.WindowTransparentForInput,
        )
        self.data = data
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        self.setAttribute(Qt.WidgetAttribute.WA_ShowWithoutActivating)
        self.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
        self.setAttribute(Qt.WidgetAttribute.WA_DeleteOnClose)
        self.setFixedSize(WIDTH, HEIGHT)
        self.setWindowTitle(f"{data.title} - WalkLevel")
        lay = QHBoxLayout(self)
        lay.setContentsMargins(14, 8, 16, 8)
        lay.setSpacing(12)
        lay.addWidget(
            Hex(t, data.rarity, data.mark, size=64, text=data.badge_text),
            0,
            Qt.AlignmentFlag.AlignVCenter,
        )
        col = QVBoxLayout()
        col.setSpacing(2)
        col.addStretch(1)
        col.addWidget(caps(data.kicker, data.color, 11, 1.6))
        title = QLabel(data.title)
        title.setFont(num_font(17))
        title.setStyleSheet(
            "color: #f2f3f5; background: transparent; font-size: 17px; font-weight: 700;"
        )
        col.addWidget(title)
        if data.progress is not None:
            col.addWidget(XPBar(t, data.progress, data.color, 8))
        line = QLabel(data.line)
        line.setStyleSheet("color: #a9afbb; background: transparent; font-size: 12px;")
        col.addWidget(line)
        col.addStretch(1)
        lay.addLayout(col, 1)
        self.fade = QPropertyAnimation(self, b"windowOpacity", self)
        self.fade.setDuration(FADE_MS)
        self.fade.setEasingCurve(QEasingCurve.Type.OutCubic)

    def paintEvent(self, _e: QPaintEvent) -> None:
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        r = QRectF(self.rect()).adjusted(1, 1, -1, -1)
        col = QColor(self.data.color)
        edge = QColor(col)
        edge.setAlpha(150)
        p.setPen(QPen(edge, 1.5))
        p.setBrush(QColor(POPUP_BG))
        p.drawRoundedRect(r, 8, 8)
        g = QLinearGradient(r.topLeft(), r.topRight())
        tint = QColor(col)
        tint.setAlpha(70)
        g.setColorAt(0, tint)
        g.setColorAt(0.55, QColor(POPUP_BG))
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(g)
        p.drawRoundedRect(r, 8, 8)
        p.setBrush(col)
        p.drawRoundedRect(QRectF(r.x(), r.y(), 4, r.height()), 2, 2)
        p.end()

    def appear(self) -> None:
        self.setWindowOpacity(0.0)
        self.show()
        if sys.platform == "win32" and QGuiApplication.platformName() == "windows":
            winapi.make_overlay(int(self.winId()))
        self.fade.stop()
        self.fade.setStartValue(0.0)
        self.fade.setEndValue(1.0)
        self.fade.start()

    def vanish(self) -> None:
        self.fade.stop()
        self.fade.setStartValue(self.windowOpacity())
        self.fade.setEndValue(0.0)
        self.fade.finished.connect(self.close)
        self.fade.start()


def game_screen_area() -> QRectF:
    """Obszar roboczy (bez paska zadań) ekranu, na którym jest okno na pierwszym planie."""
    screens = QGuiApplication.screens()
    target = QGuiApplication.primaryScreen()
    center = winapi.foreground_center()
    if center is not None:
        x, y = center
        for s in screens:
            g = s.geometry()  # w Qt 6 początek ekranu w pikselach fizycznych, rozmiar logiczny
            ratio = s.devicePixelRatio()
            if g.x() <= x < g.x() + g.width() * ratio and g.y() <= y < g.y() + g.height() * ratio:
                target = s
                break
    return QRectF(target.availableGeometry())


class PopupStack:
    """Kolejka popupów: najwyżej trzy naraz, nowsze nad starszymi, reszta czeka."""

    def __init__(self, t: Theme) -> None:
        self.t = t
        self.queue: list[PopupData] = []
        self.visible: list[Popup] = []

    def push(self, data: PopupData) -> None:
        self.queue.append(data)
        self.pump()

    def pump(self) -> None:
        while self.queue and len(self.visible) < MAX_VISIBLE:
            data = self.queue.pop(0)
            try:
                popup = Popup(self.t, data)
            except Exception:  # popup nie może zatrzymać aplikacji
                log.exception("Popup osiągnięcia")
                continue
            popup.destroyed.connect(lambda *_a, p=popup: self.closed(p))
            self.visible.append(popup)
            self.place()
            popup.appear()
            QTimer.singleShot(SHOW_MS, popup.vanish)

    def closed(self, popup: Popup) -> None:
        if popup in self.visible:
            self.visible.remove(popup)
        self.place()
        QTimer.singleShot(0, self.pump)

    def place(self) -> None:
        area = game_screen_area()
        y = area.bottom() - MARGIN
        for popup in reversed(self.visible):  # najnowszy na dole, starsze wyżej
            y -= HEIGHT
            popup.move(int(area.right() - MARGIN - WIDTH), int(y))
            y -= GAP

    def clear(self) -> None:
        self.queue.clear()
        for popup in list(self.visible):
            popup.close()


SOUNDS = Path(__file__).resolve().parent / "dzwieki"
SOUND_FILES = {"achievement": "osiagniecie.wav", "level": "awans.wav"}


def sound_path(kind: str) -> Path:
    return SOUNDS / SOUND_FILES.get(kind, SOUND_FILES["achievement"])


def play_sound(kind: str = "achievement") -> None:
    """Krótki dźwięk popupu: „achievement” albo „level” (awans). Gra w tle, nie blokuje okna;
    bez pliku - cisza (nigdy systemowy „ding”)."""
    if sys.platform != "win32":
        return
    path = sound_path(kind)
    if not path.is_file():
        log.debug("Brak dźwięku %s", path)
        return
    try:
        import winsound

        flags = winsound.SND_FILENAME | winsound.SND_ASYNC | winsound.SND_NODEFAULT
        winsound.PlaySound(str(path), flags)
    except Exception:  # brak dźwięku nie jest błędem
        log.debug("Dźwięk popupu niedostępny", exc_info=True)


__all__ = ["NUM", "RARITY_COLORS", "PopupData", "PopupStack", "play_sound"]
