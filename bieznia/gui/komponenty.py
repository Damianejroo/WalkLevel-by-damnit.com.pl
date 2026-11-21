"""Komponenty 0.3.0: wskaźnik ze strefami, żetony, kafelki trybu, oś skoków, -/+, paleta, listy.

Wszystkie kolory z motywu (Theme) - działają w jasnym i ciemnym i z kolorem profilu.
"""

from __future__ import annotations

import math
from collections.abc import Callable, Sequence
from pathlib import Path

from PySide6.QtCore import (
    QEvent,
    QObject,
    QPoint,
    QPointF,
    QRect,
    QRectF,
    QSize,
    Qt,
    QTimer,
    QVariantAnimation,
    Signal,
)
from PySide6.QtGui import (
    QColor,
    QFont,
    QFontMetricsF,
    QHideEvent,
    QKeyEvent,
    QMouseEvent,
    QPainter,
    QPainterPath,
    QPen,
    QPixmap,
)
from PySide6.QtWidgets import (
    QApplication,
    QFrame,
    QGridLayout,
    QHBoxLayout,
    QLabel,
    QLayout,
    QLayoutItem,
    QLineEdit,
    QPushButton,
    QSizePolicy,
    QVBoxLayout,
    QWidget,
)

from bieznia import paleta
from bieznia.gui import theme as th
from bieznia.gui.theme import Theme
from bieznia.gui.widgets import ElidedLabel, Motion, ease, fmt_num, label, tabular

# Znaki czcionki Segoe Fluent Icons
G_PAD = ""
G_KEYBOARD = ""
G_WALK = ""
G_CHEVRON_DOWN = ""
G_CHEVRON_RIGHT = ""
G_CHECK = ""
G_EDIT = ""
G_ADD = ""
G_MINUS = ""
G_FOLDER = ""
G_FILE = ""
G_SHUFFLE = ""
G_CHART = ""
G_BOX_ON, G_BOX_OFF, G_BOX_SOME = "", "", ""
ICON_GLYPHS: tuple[tuple[str, str], ...] = (
    ("Pad", G_PAD),
    ("Tarcza", ""),
    ("Ogień", ""),
    ("Gwiazda", ""),
    ("Auto", ""),
    ("Samolot", ""),
    ("Świat", ""),
    ("Flaga", ""),
    ("Robot", ""),
    ("Klocek", ""),
    ("Iskry", ""),
    ("Serce", ""),
)


def glyph(text: str, color: str, px: int = 16) -> QLabel:
    g = QLabel(text)
    g.setStyleSheet(
        f"font-family: '{th.ICON_FONT}', '{th.ICON_FALLBACK}'; color: {color};"
        f" font-size: {px}px; background: transparent;"
    )
    return g


# --- obrazki: ikona profilu, awatar osoby ------------------------------------------------


def _canvas(size: int) -> tuple[QPixmap, QPainter]:
    dpr = 2.0
    pix = QPixmap(int(size * dpr), int(size * dpr))
    pix.setDevicePixelRatio(dpr)
    pix.fill(Qt.GlobalColor.transparent)
    p = QPainter(pix)
    p.setRenderHint(QPainter.RenderHint.Antialiasing)
    p.setRenderHint(QPainter.RenderHint.SmoothPixmapTransform)
    return pix, p


def profile_pixmap(
    t: Theme, size: int, color: str, glyph_text: str | None, icon_path: Path | None
) -> QPixmap:
    """Ikona z gry (PNG) albo znak z zestawu na kafelku w kolorze profilu."""
    pix, p = _canvas(size)
    if icon_path is not None and icon_path.exists():
        src = QPixmap(str(icon_path))
        if not src.isNull():
            p.setRenderHint(QPainter.RenderHint.SmoothPixmapTransform, src.width() > 64)
            p.drawPixmap(QRectF(0, 0, size, size), src, QRectF(src.rect()))
            p.end()
            return pix
    col = QColor(paleta.kolor(color).for_theme(t.name))
    bg = QColor(col)
    bg.setAlphaF(0.2)
    p.setPen(Qt.PenStyle.NoPen)
    p.setBrush(bg)
    p.drawRoundedRect(QRectF(0, 0, size, size), size * 0.24, size * 0.24)
    f = QFont(th.ICON_FONT)
    f.setPixelSize(int(size * 0.5))
    p.setFont(f)
    p.setPen(col)
    p.drawText(QRectF(0, 0, size, size), Qt.AlignmentFlag.AlignCenter, glyph_text or G_PAD)
    p.end()
    return pix


def avatar_pixmap(t: Theme, size: int, name: str, color: str) -> QPixmap:
    pix, p = _canvas(size)
    p.setPen(Qt.PenStyle.NoPen)
    p.setBrush(QColor(paleta.kolor(color).swatch))
    p.drawEllipse(QRectF(0, 0, size, size))
    f = QFont(p.font())
    f.setPixelSize(int(size * 0.44))
    f.setWeight(QFont.Weight.Bold)
    p.setFont(f)
    p.setPen(QColor(paleta.ON_DARK_THEME))  # litera na żywym kolorze - zawsze ciemna
    p.drawText(QRectF(0, 0, size, size), Qt.AlignmentFlag.AlignCenter, (name[:1] or "?").upper())
    p.end()
    return pix


def picture(pix: QPixmap, size: int) -> QLabel:
    lbl = QLabel()
    lbl.setPixmap(pix)
    lbl.setFixedSize(size, size)
    lbl.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
    return lbl


# --- wskaźnik ze strefami -----------------------------------------------------------------


class ZoneGauge(QWidget):
    """Łuk 240°: strefy (stop / chód / bieg albo skoki pada) jako pasma na zewnątrz łuku.

    Etykiety progów poza łukiem, aktywna strefa świeci - nic na siebie nie nachodzi.
    """

    START, SPAN = 210.0, 240.0

    def __init__(self, t: Theme) -> None:
        super().__init__()
        self.t = t
        self.setFixedHeight(186)
        self.setMinimumWidth(280)
        self.zones: list[tuple[float, float, str]] = [(0.0, 1.0, t.text_2)]
        self.ticks: list[tuple[float, str, int]] = []
        self.vmax = 12.0
        self.active = -1
        self.mode = "idle"
        self.caption = ""
        self._value = 0.0
        self._anim = QVariantAnimation(self)
        self._anim.setDuration(420)
        self._anim.setEasingCurve(ease(th.EASE_IN))
        self._anim.valueChanged.connect(self._set_value)
        self.setAccessibleName("Prędkość taśmy")

    def configure(
        self,
        zones: list[tuple[float, float, str]],
        ticks: list[tuple[float, str, int]],
        vmax: float,
    ) -> None:
        self.zones, self.ticks, self.vmax = zones, ticks, vmax
        self.update()

    def _set_value(self, v: float) -> None:
        self._value = float(v)
        self.update()

    def set_speed(self, speed: float, mode: str, caption: str, active: int = -1) -> None:
        changed = (mode, caption, active) != (self.mode, self.caption, self.active)
        self.mode, self.caption, self.active = mode, caption, active
        self.setAccessibleDescription(f"{fmt_num(speed)} km/h, {caption}")
        if abs(speed - float(self._anim.endValue() or 0.0)) < 0.05:
            if changed:
                self.update()
            return
        self._anim.stop()
        if not Motion.enabled:
            self._set_value(speed)
            return
        self._anim.setStartValue(self._value)
        self._anim.setEndValue(speed)
        self._anim.start()

    def angle(self, v: float) -> float:
        return self.START - self.SPAN * max(0.0, min(1.0, v / self.vmax))

    @staticmethod
    def _point(c: QPointF, r: float, deg: float) -> QPointF:
        rad = math.radians(deg)
        return QPointF(c.x() + r * math.cos(rad), c.y() - r * math.sin(rad))

    def paintEvent(self, _e: QEvent) -> None:
        t = self.t
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        r = 78.0
        c = QPointF(self.width() / 2, 124)
        box = QRectF(c.x() - r, c.y() - r, 2 * r, 2 * r)
        p.setPen(QPen(QColor(t.surface_2), 14, Qt.PenStyle.SolidLine, Qt.PenCapStyle.FlatCap))
        p.drawArc(box, int(self.angle(self.vmax) * 16), int(self.SPAN * 16))
        outer = box.adjusted(-14, -14, 14, 14)
        for i, (a, b, col) in enumerate(self.zones):
            color = QColor(col)
            if i != self.active:
                color.setAlphaF(0.32)
            p.setPen(QPen(color, 5, Qt.PenStyle.SolidLine, Qt.PenCapStyle.FlatCap))
            a1, a2 = self.angle(a), self.angle(b)
            gap = 1.5 if i else 0.0
            p.drawArc(outer, int(a2 * 16), int((a1 - a2 - gap) * 16))
        if self._value > 0.05 and self.mode != "offline":
            zone_col = (
                self.zones[self.active][2] if 0 <= self.active < len(self.zones) else t.text_2
            )
            p.setPen(QPen(QColor(zone_col), 14, Qt.PenStyle.SolidLine, Qt.PenCapStyle.RoundCap))
            end = self.angle(self._value)
            p.drawArc(box, int(end * 16), int((self.START - end) * 16))
        small = QFont(p.font())
        small.setPixelSize(12)
        for v, text, zone in self.ticks:
            deg = self.angle(v)
            on = zone == self.active
            p.setPen(QPen(QColor(t.text if on else t.text_2), 1.5))
            p.drawLine(self._point(c, r + 20, deg), self._point(c, r + 27, deg))
            pos = self._point(c, r + 40, deg)
            f = QFont(small)
            f.setWeight(QFont.Weight.Bold if on else QFont.Weight.Normal)
            p.setFont(f)
            zcol = self.zones[zone][2] if 0 <= zone < len(self.zones) else t.text_2
            p.setPen(QColor(zcol if on else t.text_2))
            p.drawText(
                QRectF(pos.x() - 30, pos.y() - 9, 60, 18), Qt.AlignmentFlag.AlignCenter, text
            )
        big = tabular(QFont(p.font()))
        big.setPixelSize(44)
        big.setWeight(QFont.Weight.Bold)
        p.setFont(big)
        p.setPen(QColor(t.text if self.mode != "offline" else t.text_2))
        h = QFontMetricsF(big).height()
        p.drawText(
            QRectF(c.x() - 90, c.y() - h * 0.7, 180, h),
            Qt.AlignmentFlag.AlignCenter,
            fmt_num(self._value),
        )
        unit = QFont(p.font())
        unit.setPixelSize(13)
        unit.setWeight(QFont.Weight.Normal)
        p.setFont(unit)
        p.setPen(QColor(t.text_2))
        p.drawText(
            QRectF(c.x() - 70, c.y() + h * 0.32, 140, 18),
            Qt.AlignmentFlag.AlignCenter,
            "km/h taśmy",
        )


class ZoneChips(QWidget):
    """Strefy jako żetony pod wskaźnikiem - aktywna wypełniona kolorem strefy (tylko informacja)."""

    def __init__(self, t: Theme) -> None:
        super().__init__()
        self.t = t
        self.lay = QHBoxLayout(self)
        self.lay.setContentsMargins(0, 0, 0, 0)
        self.lay.setSpacing(6)
        self.chips: list[tuple[str, str, str]] = []
        self.active = -1
        self.buttons: list[QPushButton] = []

    def set_chips(self, chips: Sequence[tuple[str, str, str]]) -> None:
        """chips: (nazwa, wartość, kolor strefy)."""
        if list(chips) == self.chips:
            return
        self.chips = list(chips)
        for b in self.buttons:
            self.lay.removeWidget(b)
            b.hide()
            b.deleteLater()
        self.buttons = []
        for name, value, _col in self.chips:
            b = QPushButton(f"{name}\n{value}")
            b.setObjectName("chip")
            b.setFixedHeight(52)
            # żetony dzielą szerokość po równo i nigdy nie rozpychają okna
            b.setSizePolicy(QSizePolicy.Policy.Ignored, QSizePolicy.Policy.Fixed)
            # tylko informacja: bez klikania, fokusu i kursora rączki
            b.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
            b.setFocusPolicy(Qt.FocusPolicy.NoFocus)
            b.setAccessibleName(f"{name}: {value}")
            self.lay.addWidget(b, 1)
            self.buttons.append(b)
        self.active = -2
        self.set_active(-1)

    def set_active(self, index: int) -> None:
        if index == self.active:
            return
        self.active = index
        t = self.t
        size = "font-size: 12px;" if len(self.chips) > 4 else ""
        for i, (b, (_n, _v, col)) in enumerate(zip(self.buttons, self.chips, strict=True)):
            if i == index:
                fg = paleta.ON_DARK_THEME if t.name == "dark" else paleta.ON_LIGHT_THEME
                b.setStyleSheet(
                    f"QPushButton {{ background: {col}; color: {fg}; border: 1px solid {col};"
                    f" border-radius: {th.RADIUS}px; font-weight: 700; padding: 2px; {size} }}"
                )
            else:
                b.setStyleSheet(
                    f"QPushButton {{ background: {t.surface}; color: {t.text};"
                    f" border: 1px solid {t.line}; border-radius: {th.RADIUS}px;"
                    f" font-weight: 600; padding: 2px; {size} }}"
                )

    def retheme(self, t: Theme) -> None:
        self.t = t
        current, self.active = self.active, -2
        self.set_active(current)


class KeyCap(QLabel):
    """Klawisz jak na klawiaturze (np. F8). Klik = zmiana klawisza."""

    clicked = Signal()

    def __init__(self, t: Theme, text: str) -> None:
        super().__init__(text)
        self.setCursor(Qt.CursorShape.PointingHandCursor)
        self.setToolTip("Kliknij, żeby zmienić klawisz")
        self.setAccessibleName("Klawisz włączania sterowania - kliknij, żeby zmienić")
        self.retheme(t)

    def mousePressEvent(self, e: QMouseEvent) -> None:
        if e.button() == Qt.MouseButton.LeftButton:
            self.clicked.emit()

    def retheme(self, t: Theme) -> None:
        self.setStyleSheet(
            f"border: 1px solid {t.control}; border-bottom-width: 3px; border-radius: 6px;"
            f" padding: 1px 8px; font-weight: 700; color: {t.text}; background: transparent;"
        )


class ModeTiles(QWidget):
    """Trzy tryby sterowania jako kafelki z ikoną i jednym zdaniem opisu."""

    changed = Signal(str)
    MODES = (
        ("keyboard", G_KEYBOARD, "Klawisze", "W rusza taśmę"),
        ("gamepad", G_PAD, "Pad", "gałka = tempo"),
        ("treadmill", G_WALK, "Chodzenie", "ruch = W w grze"),
    )

    def __init__(self, t: Theme, current: str) -> None:
        super().__init__()
        self.t = t
        self.current = current
        lay = QHBoxLayout(self)
        lay.setContentsMargins(0, 0, 0, 0)
        lay.setSpacing(6)
        self.tiles: dict[str, tuple[QPushButton, list[QLabel]]] = {}
        for key, icon, name, sub in self.MODES:
            b = QPushButton()
            b.setFixedHeight(78)
            b.setCursor(Qt.CursorShape.PointingHandCursor)
            b.setAccessibleName(f"Tryb: {name} - {sub}")
            b.setCheckable(True)
            bl = QVBoxLayout(b)
            bl.setContentsMargins(4, 8, 4, 8)
            bl.setSpacing(1)
            labels = [QLabel(icon), QLabel(name), QLabel(sub)]
            for lbl in labels:
                lbl.setAlignment(Qt.AlignmentFlag.AlignCenter)
                lbl.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
                bl.addWidget(lbl)
            b.clicked.connect(lambda _=False, k=key: self._pick(k))
            lay.addWidget(b, 1)
            self.tiles[key] = (b, labels)
        self._paint()

    def _pick(self, key: str) -> None:
        self.set_value(key)
        self.changed.emit(key)

    def set_value(self, key: str) -> None:
        self.current = key
        self._paint()

    def retheme(self, t: Theme) -> None:
        self.t = t
        self._paint()

    def _paint(self) -> None:
        t = self.t
        on_fg = paleta.ON_DARK_THEME if t.name == "dark" else paleta.ON_LIGHT_THEME
        for key, (b, (icon, name, sub)) in self.tiles.items():
            on = key == self.current
            b.setChecked(on)
            bg, border = (t.accent, t.accent) if on else (t.surface, t.control)
            fg, fg2 = (on_fg, on_fg) if on else (t.text, t.text_2)
            b.setStyleSheet(
                f"QPushButton {{ background: {bg}; border: 1px solid {border};"
                f" border-radius: {th.RADIUS}px; }}"
            )
            icon.setStyleSheet(
                f"font-family: '{th.ICON_FONT}'; font-size: 20px; color: {fg};"
                " background: transparent;"
            )
            name.setStyleSheet(f"font-weight: 700; color: {fg}; background: transparent;")
            sub.setStyleSheet(f"font-size: 11px; color: {fg2}; background: transparent;")


# --- oś skoków pada (przeciąganie progów) ------------------------------------------------


class StepLadder(QWidget):
    """Skoki pada na jednej osi 0-100%: kolorowe odcinki, kółka progów do przeciągania,
    trójkąt = gałka teraz. Strzałki przesuwają zaznaczony próg o 1%."""

    moved = Signal(int, int)  # indeks skoku, nowy próg w %

    def __init__(self, t: Theme, min_gap: int = 5) -> None:
        super().__init__()
        self.t = t
        self.min_gap = min_gap
        self.steps: list[tuple[int, str]] = []
        self.colors: list[str] = []
        self.stick = 0.0
        self.selected = 1
        self._drag = -1
        self.setFixedHeight(96)
        self.setFocusPolicy(Qt.FocusPolicy.StrongFocus)
        self.setAccessibleName("Progi skoków pada")
        self.setCursor(Qt.CursorShape.PointingHandCursor)

    def set_steps(self, steps: list[tuple[int, str]], colors: list[str]) -> None:
        self.steps, self.colors = steps, colors
        self.selected = min(max(1, self.selected), max(1, len(steps) - 1))
        self.update()

    def set_stick(self, deflection: float) -> None:
        if abs(deflection - self.stick) > 0.005:
            self.stick = deflection
            self.update()

    def _x0w(self) -> tuple[float, float]:
        return 24.0, float(self.width() - 48)

    def _x(self, at: float) -> float:
        x0, w = self._x0w()
        return x0 + w * at / 100

    def _limits(self, i: int) -> tuple[int, int]:
        lo = self.steps[i - 1][0] + self.min_gap
        hi = self.steps[i + 1][0] - self.min_gap if i + 1 < len(self.steps) else 100
        return lo, hi

    def _move(self, i: int, at: int) -> None:
        lo, hi = self._limits(i)
        at = max(lo, min(hi, at))
        if at != self.steps[i][0]:
            self.steps[i] = (at, self.steps[i][1])
            self.update()
            self.moved.emit(i, at)

    def mousePressEvent(self, e: QMouseEvent) -> None:
        if len(self.steps) < 2:
            return
        x = e.position().x()
        self._drag = min(
            range(1, len(self.steps)), key=lambda i: abs(self._x(self.steps[i][0]) - x)
        )
        self.selected = self._drag
        self.update()

    def mouseMoveEvent(self, e: QMouseEvent) -> None:
        if self._drag > 0:
            x0, w = self._x0w()
            self._move(self._drag, round((e.position().x() - x0) / w * 100))

    def mouseReleaseEvent(self, _e: QMouseEvent) -> None:
        self._drag = -1

    def keyPressEvent(self, e: QKeyEvent) -> None:
        if len(self.steps) < 2:
            return super().keyPressEvent(e)
        if e.key() in (Qt.Key.Key_Left, Qt.Key.Key_Right):
            step = -1 if e.key() == Qt.Key.Key_Left else 1
            self._move(self.selected, self.steps[self.selected][0] + step)
        elif e.key() == Qt.Key.Key_Tab and self.selected + 1 < len(self.steps):
            self.selected += 1
            self.update()
        else:
            super().keyPressEvent(e)

    def paintEvent(self, _e: QEvent) -> None:
        t = self.t
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        y = 42.0
        small = QFont(p.font())
        small.setPixelSize(11)
        p.setFont(small)
        n = len(self.steps)
        for i, (at, speed) in enumerate(self.steps):
            end = self.steps[i + 1][0] if i + 1 < n else 100
            a, b = self._x(at), self._x(end)
            col = QColor(self.colors[i] if i < len(self.colors) else t.text_2)
            col.setAlphaF(0.4 + 0.6 * i / max(1, n - 1))
            p.setPen(Qt.PenStyle.NoPen)
            p.setBrush(col)
            p.drawRoundedRect(QRectF(a, y - 7, max(6.0, b - a - 2), 14), 4, 4)
            p.setPen(QColor(t.text))
            p.drawText(QRectF(a - 26, y + 12, 52, 16), Qt.AlignmentFlag.AlignCenter, speed)
            if i:
                p.setPen(QColor(t.text_2))
                p.drawText(QRectF(a - 22, y - 32, 44, 14), Qt.AlignmentFlag.AlignCenter, f"{at}%")
                focused = self.hasFocus() and i == self.selected
                p.setPen(QPen(QColor(t.accent if focused else t.text), 3 if focused else 2))
                p.setBrush(QColor(t.surface))
                p.drawEllipse(QPointF(a, y), 9, 9)
        sx = self._x(self.stick * 100)
        path = QPainterPath()
        path.moveTo(sx, y + 32)
        path.lineTo(sx - 6, y + 42)
        path.lineTo(sx + 6, y + 42)
        path.closeSubpath()
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(QColor(t.text))
        p.drawPath(path)


# --- przyciski - / + ----------------------------------------------------------------------


class Stepper(QWidget):
    """Wartość z - / +: klik zmienia o 0,1, przytrzymanie przyspiesza (potem co 0,5),
    w wartość można kliknąć i wpisać liczbę (strzałki ±0,1, PageUp/PageDown ±0,5)."""

    changed = Signal(float)
    FAST_AFTER = 6  # tyle powtórzeń przy przytrzymaniu, potem większy krok

    def __init__(
        self,
        t: Theme,
        title: str,
        sub: str,
        value: float,
        lo: float,
        hi: float,
        step: float = 0.1,
        fmt: Callable[[float], str] | None = None,
    ) -> None:
        super().__init__()
        self.value, self.lo, self.hi, self.step = value, lo, hi, step
        self.fmt = fmt or (lambda v: f"{fmt_num(v)} km/h")
        self.fixed = fmt is not None
        self._repeats = 0
        lay = QHBoxLayout(self)
        lay.setContentsMargins(0, 0, 0, 0)
        lay.setSpacing(8)
        texts = QVBoxLayout()
        texts.setSpacing(0)
        name = label(title)
        name.setStyleSheet("font-weight: 600;")
        texts.addWidget(name)
        self.sub_label = label(sub, "small")
        if not sub:
            self.sub_label.hide()
        texts.addWidget(self.sub_label)
        lay.addLayout(texts, 1)
        self.minus = QPushButton(G_MINUS)
        self.plus = QPushButton(G_ADD)
        for b, tip, sign in ((self.minus, "mniej", -1), (self.plus, "więcej", 1)):
            b.setObjectName("icon")
            b.setFixedSize(th.CONTROL_H, th.CONTROL_H)
            b.setAccessibleName(f"{title}: {tip}")
            b.setAutoRepeat(True)
            b.setAutoRepeatDelay(350)
            b.setAutoRepeatInterval(70)
            b.pressed.connect(self._reset_repeats)
            b.clicked.connect(lambda _=False, d=sign: self._bump(d))
        self.shown = QLineEdit()
        self.shown.setFixedWidth(96)
        self.shown.setFixedHeight(th.CONTROL_H)
        self.shown.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.shown.setAccessibleName(f"{title}: wpisz wartość")
        self.shown.setToolTip("Kliknij i wpisz wartość, np. 2,3")
        self.shown.setReadOnly(fmt is not None)  # tekst zamiast liczby (np. „stop”)
        self.shown.setStyleSheet(
            f"QLineEdit {{ font-weight: 700; font-size: 15px; border: 1px solid transparent;"
            f" border-radius: 8px; background: transparent; color: {t.text}; }}"
            f" QLineEdit:hover {{ border-color: {t.line}; }}"
            f" QLineEdit:focus {{ border-color: {t.accent}; background: {t.surface_2}; }}"
        )
        self.shown.setFont(tabular(self.shown.font()))
        self.shown.editingFinished.connect(self._typed)
        self.shown.installEventFilter(self)
        lay.addWidget(self.minus)
        lay.addWidget(self.shown)
        lay.addWidget(self.plus)
        self.set_value(value)

    def _reset_repeats(self) -> None:
        self._repeats = 0

    def _bump(self, sign: int) -> None:
        self._repeats += 1
        big = self._repeats > self.FAST_AFTER and self.step < 0.5
        step = 0.5 if big else self.step
        target = self.value + sign * step
        if big:  # od razu na okrągłe pół km/h, potem co 0,5
            target = (math.floor if sign > 0 else math.ceil)(self.value * 2 + sign) / 2
        self.set_value(target, emit=True)

    def _typed(self) -> None:
        if self.shown.isReadOnly():
            return
        text = self.shown.text().lower().replace("km/h", "").replace(",", ".").strip()
        try:
            v = float(text)
        except ValueError:
            self.set_value(self.value)  # błędny wpis - wraca poprzednia wartość
            return
        self.set_value(v, emit=True)
        self.shown.setText(self.fmt(self.value))

    def eventFilter(self, obj: QObject, e: QEvent) -> bool:
        if obj is self.shown and not self.shown.isReadOnly():
            if e.type() == QEvent.Type.FocusIn:
                self.shown.setText(fmt_num(self.value))  # sama liczba do edycji
                QTimer.singleShot(0, self.shown.selectAll)
            elif e.type() == QEvent.Type.KeyPress:
                key = e.key()  # type: ignore[attr-defined]
                steps = {
                    Qt.Key.Key_Up: self.step,
                    Qt.Key.Key_Down: -self.step,
                    Qt.Key.Key_PageUp: 0.5,
                    Qt.Key.Key_PageDown: -0.5,
                }
                if key in steps:
                    self._typed()
                    self.set_value(self.value + steps[key], emit=True)
                    self.shown.setText(fmt_num(self.value))
                    self.shown.selectAll()
                    return True
                if key in (Qt.Key.Key_Escape,):
                    self.set_value(self.value)
                    self.shown.clearFocus()
                    return True
        return False

    def set_format(self, fmt: Callable[[float], str] | None) -> None:
        """Tekst zamiast liczby (np. „stop”) blokuje zmianę; None - zwykła prędkość."""
        self.fmt = fmt or (lambda v: f"{fmt_num(v)} km/h")
        self.fixed = fmt is not None
        self.shown.setReadOnly(self.fixed)
        self.set_value(self.value)

    def set_value(self, v: float, emit: bool = False) -> None:
        v = round(max(self.lo, min(self.hi, v)), 1)
        changed = v != self.value
        self.value = v
        if not self.shown.hasFocus() or self.shown.isReadOnly():
            self.shown.setText(self.fmt(v))
        self.minus.setEnabled(v > self.lo and not self.fixed)
        self.plus.setEnabled(v < self.hi and not self.fixed)
        if emit and changed:
            self.changed.emit(v)


# --- paleta kolorów ---------------------------------------------------------------------------


class PalettePicker(QWidget):
    """20 kolorów w siatce 10 x 2; opis mówi, czy kolor jest z ikony, czy wybrany ręcznie."""

    picked = Signal(str)

    def __init__(self, t: Theme, selected: str, from_icon: str | None = None) -> None:
        super().__init__()
        self.t = t
        self.selected = paleta.kolor(selected).name
        self.from_icon = from_icon
        lay = QVBoxLayout(self)
        lay.setContentsMargins(0, 0, 0, 0)
        lay.setSpacing(8)
        grid = QGridLayout()
        grid.setSpacing(6)
        self.buttons: dict[str, QPushButton] = {}
        for i, k in enumerate(paleta.KOLORY):
            b = QPushButton()
            b.setFixedSize(32, 32)
            b.setCursor(Qt.CursorShape.PointingHandCursor)
            b.setToolTip(k.name)
            b.setAccessibleName(f"Kolor {k.name}")
            b.clicked.connect(lambda _=False, n=k.name: self._pick(n))
            grid.addWidget(b, i // 10, i % 10)
            self.buttons[k.name] = b
        lay.addLayout(grid)
        self.note = label("", "small")
        lay.addWidget(self.note)
        self._paint()

    def _pick(self, name: str) -> None:
        self.selected = name
        self._paint()
        self.picked.emit(name)

    def retheme(self, t: Theme) -> None:
        self.t = t
        self._paint()

    def _paint(self) -> None:
        t = self.t
        on = paleta.ON_DARK_THEME  # znaczek na żywym kolorze - zawsze ciemny
        for name, b in self.buttons.items():
            sel = name == self.selected
            b.setText(G_CHECK if sel else "")
            ring = f"border: 3px solid {t.text};" if sel else "border: none;"
            b.setStyleSheet(
                f"QPushButton {{ background: {paleta.kolor(name).swatch};"
                f" border-radius: 16px; {ring} padding: 0px; font-family: '{th.ICON_FONT}';"
                f" font-size: 14px; color: {on}; }}"
            )
        note = self.selected
        if self.from_icon:
            note += (
                " · dobrany z ikony gry"
                if self.from_icon == self.selected
                else f" · wybrany ręcznie (z ikony: {self.from_icon})"
            )
        self.note.setText(note)


class ColorButton(QPushButton):
    """Prostokątny przycisk w kolorze profilu (otwiera wybór koloru)."""

    def __init__(self, t: Theme, color: str, text: str = "Kolor") -> None:
        super().__init__(text)
        self.setFixedSize(96, th.CONTROL_H)
        self.setCursor(Qt.CursorShape.PointingHandCursor)
        self.color = color
        self.retheme(t)

    def set_color(self, color: str) -> None:
        self.color = color
        self.retheme(self.t)

    def retheme(self, t: Theme) -> None:
        self.t = t
        col = paleta.kolor(self.color).swatch
        on = paleta.ON_DARK_THEME
        self.setAccessibleName(f"Kolor profilu: {self.color}")
        self.setStyleSheet(
            f"QPushButton {{ background: {col}; border: 1px solid {col};"
            f" border-radius: {th.RADIUS}px; font-weight: 700; color: {on}; padding: 0px; }}"
        )


class IconGrid(QWidget):
    """Ikony z zestawu (12) do wyboru."""

    picked = Signal(str)

    def __init__(self, t: Theme, selected: str | None, color: str) -> None:
        super().__init__()
        self.t, self.selected, self.color = t, selected, color
        grid = QGridLayout(self)
        grid.setContentsMargins(0, 0, 0, 0)
        grid.setSpacing(6)
        self.buttons: dict[str, QPushButton] = {}
        for i, (name, code) in enumerate(ICON_GLYPHS):
            b = QPushButton(code)
            b.setFixedSize(48, 48)
            b.setToolTip(name)
            b.setAccessibleName(f"Ikona {name}")
            b.setCursor(Qt.CursorShape.PointingHandCursor)
            b.clicked.connect(lambda _=False, c=code: self._pick(c))
            grid.addWidget(b, i // 6, i % 6)
            self.buttons[code] = b
        self._paint()

    def _pick(self, code: str) -> None:
        self.selected = code
        self._paint()
        self.picked.emit(code)

    def set_color(self, color: str) -> None:
        self.color = color
        self._paint()

    def _paint(self) -> None:
        t = self.t
        col = paleta.kolor(self.color).for_theme(t.name)
        for code, b in self.buttons.items():
            on = code == self.selected
            b.setStyleSheet(
                f"QPushButton {{ font-family: '{th.ICON_FONT}'; font-size: 20px;"
                f" border-radius: {th.RADIUS}px; padding: 0px; color: {col if on else t.text};"
                f" background: {t.surface_2 if on else 'transparent'};"
                f" border: {'2px solid ' + col if on else '1px solid ' + t.line}; }}"
            )


# --- wiersze list -----------------------------------------------------------------------------


class ChoiceRow(QPushButton):
    """Wiersz wyboru (osoba, profil): pierścień wyboru, obrazek, nazwa, opis, ołówek."""

    edit = Signal()

    def __init__(
        self,
        t: Theme,
        pix: QPixmap,
        pix_size: int,
        title: str,
        sub: str,
        active: bool,
        accent: str,
        editable: bool = True,
    ) -> None:
        super().__init__()
        self.setObjectName("row")
        self.setFixedHeight(62)
        self.setCursor(Qt.CursorShape.PointingHandCursor)
        self.setAccessibleName(f"{title}{' (wybrany)' if active else ''}. {sub}")
        if active:
            self.setStyleSheet(f"QPushButton#row {{ border: 2px solid {accent}; }}")
        lay = QHBoxLayout(self)
        lay.setContentsMargins(10, 6, 8, 6)
        lay.setSpacing(10)
        dot = QLabel()
        dot.setFixedSize(20, 20)
        ring = f"6px solid {accent}" if active else f"2px solid {t.control}"
        dot.setStyleSheet(f"border: {ring}; border-radius: 10px;")
        dot.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
        lay.addWidget(dot)
        lay.addWidget(picture(pix, pix_size))
        texts = QVBoxLayout()
        texts.setSpacing(0)
        name = ElidedLabel(title, "")
        name.setStyleSheet("font-weight: 600;")
        self.sub = label(sub, "small")
        for w in (name, self.sub):
            w.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
            texts.addWidget(w)
        lay.addLayout(texts, 1)
        if editable:
            pencil = QPushButton(G_EDIT)
            pencil.setObjectName("icon")
            pencil.setFixedSize(th.CONTROL_H, th.CONTROL_H)
            pencil.setAccessibleName(f"Edytuj: {title}")
            pencil.setToolTip("Edytuj")
            pencil.clicked.connect(self.edit.emit)
            lay.addWidget(pencil)


class AddRow(QPushButton):
    def __init__(self, t: Theme, text: str) -> None:
        super().__init__()
        self.setObjectName("row")
        self.setFixedHeight(50)
        self.setCursor(Qt.CursorShape.PointingHandCursor)
        self.setAccessibleName(text)
        self.setStyleSheet("QPushButton#row { border-style: dashed; }")
        lay = QHBoxLayout(self)
        lay.setContentsMargins(14, 4, 14, 4)
        lay.setSpacing(10)
        icon = glyph(G_ADD, t.accent)
        lay.addWidget(icon)
        txt = label(text)
        txt.setStyleSheet(f"font-weight: 600; color: {t.accent};")
        lay.addWidget(txt, 1)
        for w in (icon, txt):
            w.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)


class CheckRow(QPushButton):
    """Pole wyboru w wierszu (zaznaczone / nie / częściowo) z obrazkiem i opisem."""

    toggled_state = Signal(bool)

    def __init__(
        self,
        t: Theme,
        title: str,
        sub: str,
        state: str,
        pix: QPixmap | None = None,
        indent: int = 0,
    ) -> None:
        super().__init__()
        self.t = t
        self.state = state  # "on" | "off" | "some"
        self.setFlat(True)
        self.setCursor(Qt.CursorShape.PointingHandCursor)
        self.setStyleSheet(
            "QPushButton { border: none; background: transparent; text-align: left; }"
        )
        self.setFixedHeight(50 if sub else 40)
        self.setAccessibleName(title)
        lay = QHBoxLayout(self)
        lay.setContentsMargins(indent, 0, 0, 0)
        lay.setSpacing(10)
        self.box = QLabel()
        self.box.setFixedSize(26, 26)
        self.box.setAlignment(Qt.AlignmentFlag.AlignCenter)
        lay.addWidget(self.box)
        if pix is not None:
            lay.addWidget(picture(pix, 28))
        texts = QVBoxLayout()
        texts.setSpacing(0)
        name = label(title)
        name.setStyleSheet("font-weight: 600;" if not indent else "")
        texts.addWidget(name)
        if sub:
            texts.addWidget(label(sub, "small"))
        for w in (self.box, name):
            w.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
        lay.addLayout(texts, 1)
        self.clicked.connect(self._toggle)
        self._paint()

    def _toggle(self) -> None:
        self.set_state("off" if self.state == "on" else "on")
        self.toggled_state.emit(self.state == "on")

    def set_state(self, state: str) -> None:
        self.state = state
        self._paint()

    def _paint(self) -> None:
        g = {"on": G_BOX_ON, "off": G_BOX_OFF, "some": G_BOX_SOME}[self.state]
        col = self.t.accent if self.state != "off" else self.t.control
        self.box.setText(g)
        self.box.setStyleSheet(f"font-family: '{th.ICON_FONT}'; font-size: 20px; color: {col};")
        self.setAccessibleDescription(
            {"on": "zaznaczone", "off": "niezaznaczone", "some": "częściowo"}[self.state]
        )


class Popup(QFrame):
    """Wysuwana lista nad treścią (nie przesuwa ekranu). Zamyka ją wybór, Esc albo klik obok."""

    def __init__(self, t: Theme, parent: QWidget) -> None:
        super().__init__(parent)
        self.setObjectName("pop")
        self.setStyleSheet(
            f"QFrame#pop {{ background: {t.surface_2}; border: 1px solid {t.control};"
            f" border-radius: 12px; }}"
        )
        self.lay = QVBoxLayout(self)
        self.lay.setContentsMargins(6, 6, 6, 6)
        self.lay.setSpacing(2)
        self.hide()
        app = QApplication.instance()  # klik gdziekolwiek w oknie (nie tylko w samym tle)
        if app is not None:
            app.installEventFilter(self)

    def add_row(
        self, pix: QPixmap, size: int, title: str, sub: str, active: bool, accent: str
    ) -> QPushButton:
        b = QPushButton()
        b.setFlat(True)
        b.setCursor(Qt.CursorShape.PointingHandCursor)
        b.setFixedHeight(50 if sub else 42)
        b.setAccessibleName(title)
        b.setStyleSheet(
            "QPushButton { border: none; border-radius: 8px; background: transparent; }"
            "QPushButton:hover { background: rgba(127,127,127,0.18); }"
        )
        r = QHBoxLayout(b)
        r.setContentsMargins(8, 4, 8, 4)
        r.setSpacing(10)
        r.addWidget(picture(pix, size))
        texts = QVBoxLayout()
        texts.setSpacing(0)
        name = ElidedLabel(title, "")  # długie nazwy z wielokropkiem, lista się nie rozjeżdża
        if active:
            name.setStyleSheet("font-weight: 700;")
        texts.addWidget(name)
        if sub:
            texts.addWidget(label(sub, "small"))
        r.addLayout(texts, 1)
        if active:
            r.addWidget(glyph(G_CHECK, accent))
        for i in range(r.count()):
            item = r.itemAt(i)
            w = item.widget() if item is not None else None
            if w is not None:
                w.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
        self.lay.addWidget(b)
        return b

    def add_link(self, text: str, accent: str) -> QPushButton:
        b = QPushButton(text)
        b.setFlat(True)
        b.setCursor(Qt.CursorShape.PointingHandCursor)
        b.setStyleSheet(
            f"QPushButton {{ border: none; color: {accent}; text-align: left; padding: 8px;"
            f" background: transparent; }}"
        )
        self.lay.addWidget(b)
        return b

    def open_under(self, anchor: QWidget, width: int = 280) -> None:
        parent = self.parentWidget()
        assert parent is not None
        pos = anchor.mapTo(parent, anchor.rect().bottomLeft())
        width = max(width, min(parent.width() - 32, 360))
        self.setFixedWidth(width)
        self.adjustSize()
        x = max(16, min(pos.x(), parent.width() - width - 16))
        self.move(x, pos.y() + 6)
        self.raise_()
        self.show()
        self.setFocus()

    def eventFilter(self, obj: QObject, e: QEvent) -> bool:
        if self.isVisible() and e.type() == QEvent.Type.MouseButtonPress:
            pos = self.mapFromGlobal(e.globalPosition().toPoint())  # type: ignore[attr-defined]
            if not self.rect().contains(pos):
                self.hide()
        return False

    def hideEvent(self, e: QHideEvent) -> None:
        # fokus wraca do okna bez powodu "z klawiatury" - bez obwódki na sąsiednim przycisku
        parent = self.parentWidget()
        focus = QApplication.focusWidget()
        if parent is not None and focus is not None and (focus is self or self.isAncestorOf(focus)):
            parent.setFocus(Qt.FocusReason.OtherFocusReason)
        super().hideEvent(e)

    def keyPressEvent(self, e: QKeyEvent) -> None:
        if e.key() == Qt.Key.Key_Escape:
            self.hide()
        else:
            super().keyPressEvent(e)

    def sizeHint(self) -> QSize:
        return super().sizeHint()


class FlowLayout(QLayout):
    """Elementy obok siebie; gdy brak miejsca - kolejny wiersz (np. podpowiedzi nazw)."""

    def __init__(self, spacing: int = 6) -> None:
        super().__init__()
        self._items: list[QLayoutItem] = []
        self._gap = spacing
        self.setContentsMargins(0, 0, 0, 0)

    def addItem(self, item: QLayoutItem) -> None:
        self._items.append(item)

    def count(self) -> int:
        return len(self._items)

    def itemAt(self, index: int) -> QLayoutItem | None:
        return self._items[index] if 0 <= index < len(self._items) else None

    def takeAt(self, index: int) -> QLayoutItem | None:
        return self._items.pop(index) if 0 <= index < len(self._items) else None

    def expandingDirections(self) -> Qt.Orientation:
        return Qt.Orientation(0)

    def hasHeightForWidth(self) -> bool:
        return True

    def heightForWidth(self, width: int) -> int:
        return self._place(QRect(0, 0, width, 0), apply=False)

    def setGeometry(self, rect: QRect) -> None:
        super().setGeometry(rect)
        self._place(rect, apply=True)

    def sizeHint(self) -> QSize:
        return self.minimumSize()

    def minimumSize(self) -> QSize:
        size = QSize()
        for item in self._items:
            size = size.expandedTo(item.minimumSize())
        return size

    def _place(self, rect: QRect, apply: bool) -> int:
        x, y, row_h = rect.x(), rect.y(), 0
        for item in self._items:
            hint = item.sizeHint()
            if x > rect.x() and x + hint.width() > rect.right() + 1:
                x, y, row_h = rect.x(), y + row_h + self._gap, 0
            if apply:
                item.setGeometry(QRect(QPoint(x, y), hint))
            x += hint.width() + self._gap
            row_h = max(row_h, hint.height())
        return y + row_h - rect.y()


class SaveNote(QLabel):
    """Informacja przy edycji: zmiany zapisują się od razu; po zmianie krótko „Zapisano”."""

    IDLE = "Zmiany zapisują się automatycznie"

    def __init__(self, t: Theme) -> None:
        super().__init__()
        self.t = t
        self.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.setAccessibleName("Stan zapisu")
        self._timer = QTimer(self)
        self._timer.setSingleShot(True)
        self._timer.timeout.connect(self._idle)
        self._idle()

    def _show(self, text: str, color: str) -> None:
        self.setText(f"<span style='font-family:\"{th.ICON_FONT}\"'>{G_CHECK}</span>&nbsp; {text}")
        self.setStyleSheet(f"color: {color}; font-size: 13px; padding: 4px;")

    def _idle(self) -> None:
        self._show(self.IDLE, self.t.text_2)

    def saved(self) -> None:
        self._show("Zapisano", self.t.accent)
        self._timer.start(2000)

    def retheme(self, t: Theme) -> None:
        self.t = t
        self._idle()


def show_pending(layout: QLayout) -> None:
    """Pokazuje od razu elementy dodane do układu (Qt robi to z opóźnieniem, a do tego czasu
    układ liczy się bez nich)."""
    for i in range(layout.count()):
        item = layout.itemAt(i)
        if item is None:
            continue
        w = item.widget()
        if w is not None and w.isHidden():
            if not w.testAttribute(Qt.WidgetAttribute.WA_WState_ExplicitShowHide):
                w.show()
        elif (inner := item.layout()) is not None:
            show_pending(inner)


class _Ghost(QWidget):
    """Zrzut starej zawartości, który gaśnie nad nową."""

    def __init__(self, parent: QWidget) -> None:
        super().__init__(parent)
        self.pix = QPixmap()
        self.opacity = 1.0
        self.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
        self.hide()

    def paintEvent(self, _e: QEvent) -> None:
        if not self.pix.isNull():
            p = QPainter(self)
            p.setOpacity(self.opacity)
            p.drawPixmap(0, 0, self.pix)


class SmoothBox(QWidget):
    """Kontener, którego zawartość podmienia się płynnie: wysokość rośnie albo maleje
    (treść pod spodem spływa, a nie skacze), a stara zawartość przenika w nową."""

    DURATION = 220

    def __init__(self, spacing: int = 8) -> None:
        super().__init__()
        self.lay = QVBoxLayout(self)
        self.lay.setContentsMargins(0, 0, 0, 0)
        self.lay.setSpacing(spacing)
        self.ghost = _Ghost(self)
        self.anim = QVariantAnimation(self)
        self.anim.setEasingCurve(ease(th.EASE_IN))
        self.anim.valueChanged.connect(self._step)
        self.anim.finished.connect(self._done)
        self._from = self._to = 0

    def natural_height(self) -> int:
        """Wysokość nowej zawartości - po nałożeniu stylu (czcionki, odstępy) na nowe elementy
        i po tym, jak elementy zależne od szerokości (zawijanie, przełączniki) się ułożą."""
        show_pending(self.lay)  # nowe elementy Qt pokazuje dopiero w następnej klatce
        for child in self.findChildren(QWidget):
            child.ensurePolished()
        width = max(1, self.width())

        def measure() -> int:
            self.lay.invalidate()
            self.lay.activate()
            if self.lay.hasHeightForWidth():
                return int(self.lay.totalHeightForWidth(width))
            return int(self.lay.totalSizeHint().height())

        height = measure()
        for _ in range(3):  # ułożenie na próbę - elementy dopasowują się do szerokości
            self.lay.setGeometry(QRect(0, 0, width, height))
            again = measure()
            if again == height:
                break
            height = again
        return height

    def swap(self, rebuild: Callable[[], None], background: str) -> None:
        """rebuild() wypełnia self.lay od nowa (stara zawartość usuwa się sama)."""
        if not (Motion.enabled and self.isVisible() and self.height() > 0):
            rebuild()
            return
        self.anim.stop()
        old_h = self.height()
        shot = self.grab()
        pix = QPixmap(shot.size())
        pix.setDevicePixelRatio(shot.devicePixelRatio())
        pix.fill(QColor(background))  # stara treść na tle karty - nowa nie prześwituje
        painter = QPainter(pix)
        painter.drawPixmap(0, 0, shot)
        painter.end()
        rebuild()
        new_h = self.natural_height()
        self.ghost.pix, self.ghost.opacity = pix, 1.0
        self.ghost.setGeometry(0, 0, self.width(), old_h)
        self.ghost.raise_()
        self.ghost.show()
        self._from, self._to = old_h, new_h
        self.setFixedHeight(old_h)
        self.anim.setStartValue(0.0)
        self.anim.setEndValue(1.0)
        self.anim.setDuration(self.DURATION)
        self.anim.start()

    def _step(self, value: object) -> None:
        p = float(value)  # type: ignore[arg-type]
        self.setFixedHeight(round(self._from + (self._to - self._from) * p))
        self.ghost.opacity = max(0.0, 1.0 - p * 1.6)  # stara treść znika w pierwszej części
        self.ghost.update()

    def _done(self) -> None:
        self.ghost.hide()
        self.ghost.pix = QPixmap()
        self.setMinimumHeight(0)
        self.setMaximumHeight(16777215)
