"""Kontrolki okna. Animacje tylko tam, gdzie pokazują zmianę stanu albo ruch taśmy."""

from __future__ import annotations

import contextlib
import math
from collections.abc import Iterable

from PySide6.QtCore import (
    Property,
    QEasingCurve,
    QEvent,
    QObject,
    QPointF,
    QPropertyAnimation,
    QRectF,
    QSize,
    Qt,
    QTimer,
    QVariantAnimation,
    Signal,
)
from PySide6.QtGui import (
    QColor,
    QFocusEvent,
    QFont,
    QFontMetricsF,
    QKeyEvent,
    QKeySequence,
    QPainter,
    QPainterPath,
    QPen,
    QWheelEvent,
)
from PySide6.QtWidgets import (
    QAbstractButton,
    QApplication,
    QBoxLayout,
    QButtonGroup,
    QFrame,
    QGridLayout,
    QLabel,
    QLayout,
    QPushButton,
    QSizePolicy,
    QSlider,
    QVBoxLayout,
    QWidget,
)

from bieznia.gui import theme as th
from bieznia.gui.theme import Theme
from bieznia.klawisze import Key, key_from_native


def ease(points: tuple[float, float, float, float]) -> QEasingCurve:
    curve = QEasingCurve(QEasingCurve.Type.BezierSpline)
    x1, y1, x2, y2 = points
    curve.addCubicBezierSegment(QPointF(x1, y1), QPointF(x2, y2), QPointF(1, 1))
    return curve


def tabular(font: QFont) -> QFont:
    """Cyfry o stałej szerokości - zmieniająca się prędkość nie skacze."""
    with contextlib.suppress(AttributeError, TypeError):
        font.setFeature(QFont.Tag("tnum"), 1)
    return font


def fmt_num(value: float, digits: int = 1) -> str:
    return f"{value:.{digits}f}".replace(".", ",")


class Motion:
    """Globalny przełącznik animacji (systemowe „ogranicz ruch”)."""

    enabled = True


# --- fokus -----------------------------------------------------------------


class FocusRing(QWidget):
    """Obwódka fokusu 2 px w kolorze akcentu, 2 px od kontrolki. Tylko przy klawiaturze."""

    KEYBOARD = (
        Qt.FocusReason.TabFocusReason,
        Qt.FocusReason.BacktabFocusReason,
        Qt.FocusReason.ShortcutFocusReason,
    )

    def __init__(self, window: QWidget, t: Theme) -> None:
        super().__init__(window)
        self.t = t
        self.target: QWidget | None = None
        self.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
        self.hide()
        app = QApplication.instance()
        if app is not None:
            app.installEventFilter(self)

    def eventFilter(self, obj: QObject, event: QEvent) -> bool:
        kind = event.type()
        if (
            kind == QEvent.Type.FocusIn
            and isinstance(obj, QWidget)
            and obj.window() is self.window()
        ):
            reason = event.reason()  # type: ignore[attr-defined]
            if reason in self.KEYBOARD:
                self.target = obj
                self.follow()
        elif kind == QEvent.Type.MouseButtonPress:
            self.hide()
        elif kind in (QEvent.Type.Move, QEvent.Type.Resize) and obj is self.target:
            self.follow()
        return False

    def follow(self) -> None:
        target = self.target
        if target is None or not target.isVisible():
            self.hide()
            return
        top_left = target.mapTo(self.window(), target.rect().topLeft())
        grow = th.FOCUS_W + th.FOCUS_OFFSET
        self.setGeometry(
            top_left.x() - grow,
            top_left.y() - grow,
            target.width() + 2 * grow,
            target.height() + 2 * grow,
        )
        self.show()
        self.raise_()

    def paintEvent(self, _e: QEvent) -> None:
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        p.setPen(QPen(QColor(self.t.accent), th.FOCUS_W))
        r = QRectF(self.rect()).adjusted(1, 1, -1, -1)
        radius = th.RADIUS + th.FOCUS_OFFSET + th.FOCUS_W / 2
        p.drawRoundedRect(r, radius, radius)


# --- podstawowe kontrolki ---------------------------------------------------


def card() -> QFrame:
    frame = QFrame()
    frame.setObjectName("card")
    return frame


def label(text: str = "", name: str = "") -> QLabel:
    """Napis zawijany do szerokości okna - długi tekst nie rozpycha układu."""
    lbl = QLabel(text)
    lbl.setWordWrap(True)
    if name:
        lbl.setObjectName(name)
    return lbl


def button(text: str, kind: str = "", tip: str = "") -> QPushButton:
    b = QPushButton(text)
    if kind:
        b.setObjectName(kind)
    b.setFixedHeight(th.CONTROL_H)
    b.setCursor(Qt.CursorShape.PointingHandCursor)
    if tip:
        b.setToolTip(tip)
    b.setAccessibleName(tip or text)
    return b


def icon_button(icon: str, name: str) -> QPushButton:
    """Przycisk z ikoną zawsze ma nazwę (podpowiedź + nazwa dla czytników ekranu)."""
    b = button(th.ICONS[icon], "icon", name)
    b.setFixedSize(th.CONTROL_H, th.CONTROL_H)
    return b


class Switch(QAbstractButton):
    """Przełącznik: pole trafienia 56x44, animacja gałki pokazuje zmianę stanu."""

    def __init__(self, t: Theme, name: str, checked: bool = False) -> None:
        super().__init__()
        self.t = t
        self.setCheckable(True)
        self.setChecked(checked)
        self.setAccessibleName(name)
        self.setToolTip(name)
        self.setCursor(Qt.CursorShape.PointingHandCursor)
        self.setFocusPolicy(Qt.FocusPolicy.StrongFocus)
        self.setFixedSize(56, th.CONTROL_H)
        self._pos = 1.0 if checked else 0.0
        self._anim = QPropertyAnimation(self, b"knob", self)
        self._anim.setDuration(160)
        self._anim.setEasingCurve(ease(th.EASE_IN))
        self.toggled.connect(self._animate)

    def _animate(self, on: bool) -> None:
        self._anim.stop()
        if not Motion.enabled:
            self.set_knob(1.0 if on else 0.0)
            return
        self._anim.setStartValue(self._pos)
        self._anim.setEndValue(1.0 if on else 0.0)
        self._anim.start()

    def get_knob(self) -> float:
        return self._pos

    def set_knob(self, v: float) -> None:
        self._pos = v
        self.update()

    knob = Property(float, get_knob, set_knob)

    def set_silently(self, on: bool) -> None:
        self.blockSignals(True)
        self.setChecked(on)
        self.blockSignals(False)
        self.set_knob(1.0 if on else 0.0)

    def sizeHint(self) -> QSize:
        return QSize(56, th.CONTROL_H)

    def paintEvent(self, _e: QEvent) -> None:
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        t, k = self.t, self._pos
        track = QRectF(2, 7, 52, 30)
        off_bg, on_bg = QColor(t.bg), QColor(t.accent)
        fill = QColor(
            round(off_bg.red() + (on_bg.red() - off_bg.red()) * k),
            round(off_bg.green() + (on_bg.green() - off_bg.green()) * k),
            round(off_bg.blue() + (on_bg.blue() - off_bg.blue()) * k),
        )
        p.setPen(QPen(QColor(t.accent if k > 0.5 else t.control), 2))
        p.setBrush(fill)
        p.drawRoundedRect(track, 15, 15)
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(QColor(t.on_accent if k > 0.5 else t.text))
        p.drawEllipse(QRectF(7 + k * 22, 12, 20, 20))


class Segmented(QFrame):
    changed = Signal(str)

    def __init__(self, options: Iterable[tuple[str, str]], current: str, name: str) -> None:
        super().__init__()
        self.setObjectName("segbox")
        self.setAccessibleName(name)
        self.setFixedHeight(th.CONTROL_H)
        # Segmenty obok siebie; gdy naprawdę brakuje miejsca - jeden pod drugim.
        lay = QBoxLayout(QBoxLayout.Direction.LeftToRight, self)
        lay.setContentsMargins(3, 3, 3, 3)
        lay.setSpacing(2)
        lay.setSizeConstraint(QLayout.SizeConstraint.SetNoConstraint)
        self.box = lay
        self.group = QButtonGroup(self)
        self.buttons: dict[str, QPushButton] = {}
        for key, text in options:
            b = QPushButton(text)
            b.setObjectName("seg")
            b.setCheckable(True)
            b.setCursor(Qt.CursorShape.PointingHandCursor)
            b.setFixedHeight(th.CONTROL_H - 10)
            b.setAccessibleName(f"{name}: {text}")
            b.clicked.connect(lambda _=False, k=key: self.changed.emit(k))
            self.group.addButton(b)
            self.buttons[key] = b
            lay.addWidget(b)
        self.set_value(current)

    def set_value(self, key: str) -> None:
        if key in self.buttons:
            self.buttons[key].setChecked(True)

    def needed_width(self) -> int:
        widths = [b.sizeHint().width() for b in self.buttons.values()]
        return sum(widths) + self.box.spacing() * (len(widths) - 1) + 6 + 4

    def minimumSizeHint(self) -> QSize:
        widest = max(b.minimumSizeHint().width() for b in self.buttons.values())
        return QSize(widest + 10, th.CONTROL_H)

    def resizeEvent(self, e: QEvent) -> None:
        horizontal = self.width() >= self.needed_width()
        want = QBoxLayout.Direction.LeftToRight if horizontal else QBoxLayout.Direction.TopToBottom
        if self.box.direction() != want:
            self.box.setDirection(want)
            rows = 1 if horizontal else len(self.buttons)
            self.setFixedHeight(th.CONTROL_H + (rows - 1) * (th.CONTROL_H - 8))
        super().resizeEvent(e)  # type: ignore[arg-type]


class HoldButton(QPushButton):
    """Akcja niebezpieczna: przytrzymaj ok. 1 s zamiast okna z pytaniem."""

    confirmed = Signal()
    HOLD_MS = 1000

    def __init__(self, text: str, t: Theme) -> None:
        super().__init__(text)
        self.t = t
        self.setFixedHeight(th.CONTROL_H)
        self.setCursor(Qt.CursorShape.PointingHandCursor)
        self.setAccessibleName(f"{text} (przytrzymaj)")
        self.setToolTip("Przytrzymaj, żeby potwierdzić")
        self.retheme(t)
        self._progress = 0.0
        self._anim = QVariantAnimation(self)
        self._anim.setDuration(self.HOLD_MS)
        self._anim.setStartValue(0.0)
        self._anim.setEndValue(1.0)
        self._anim.valueChanged.connect(self._set_progress)
        self._anim.finished.connect(self._done)
        self.pressed.connect(self._anim.start)
        self.released.connect(self._cancel)

    def retheme(self, t: Theme) -> None:
        self.t = t
        self.setStyleSheet(f"color: {t.danger}; border-color: {t.danger};")

    def _set_progress(self, v: float) -> None:
        self._progress = float(v)
        self.update()

    def _cancel(self) -> None:
        self._anim.stop()
        self._set_progress(0.0)

    def _done(self) -> None:
        self._set_progress(0.0)
        self.confirmed.emit()

    def paintEvent(self, e: QEvent) -> None:
        super().paintEvent(e)  # type: ignore[arg-type]
        if self._progress <= 0:
            return
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        fill = QColor(self.t.danger)
        fill.setAlphaF(0.22)
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(fill)
        r = QRectF(self.rect()).adjusted(2, 2, -2, -2)
        r.setWidth(r.width() * self._progress)
        p.drawRoundedRect(r, th.RADIUS - 2, th.RADIUS - 2)


class StatusDot(QWidget):
    """Kropka stanu połączenia. Pulsuje tylko w trakcie szukania (pokazuje, że coś trwa)."""

    def __init__(self, t: Theme) -> None:
        super().__init__()
        self.t = t
        self.setFixedSize(16, 16)
        self.color = QColor(t.text_2)
        self._phase = 0.0
        self._timer = QTimer(self)
        self._timer.setInterval(50)
        self._timer.timeout.connect(self._tick)

    def set_state(self, color: str, busy: bool) -> None:
        self.color = QColor(color)
        if busy and Motion.enabled:
            self._timer.start()
        else:
            self._timer.stop()
            self._phase = 0.0
        self.update()

    def _tick(self) -> None:
        self._phase = (self._phase + 0.05) % 1.0
        self.update()

    def paintEvent(self, _e: QEvent) -> None:
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        c = QColor(self.color)
        c.setAlphaF(0.35 + 0.65 * (0.5 + 0.5 * math.cos(self._phase * 2 * math.pi)))
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(c)
        p.drawEllipse(QRectF(3, 3, 10, 10))


# --- wizualizacja ruchu -----------------------------------------------------


class Gauge(QWidget):
    """Prędkościomierz: łuk 240°, znaczniki progów chodu i biegu, liczba na środku."""

    def __init__(self, t: Theme) -> None:
        super().__init__()
        self.t = t
        self.setFixedHeight(184)
        self.setMinimumWidth(260)
        self.max_speed = 10.0
        self.walk_from = 0.8
        self.run_from = 6.3
        self.mode = "idle"  # idle | walk | run | offline
        self.caption = "Brak danych"
        self._value = 0.0
        self._anim = QVariantAnimation(self)
        self._anim.setDuration(420)
        self._anim.setEasingCurve(ease(th.EASE_IN))
        self._anim.valueChanged.connect(self._set_value)
        self.setAccessibleName("Prędkość taśmy")

    def _set_value(self, v: float) -> None:
        self._value = float(v)
        self.update()

    def set_speed(self, speed: float, mode: str, caption: str) -> None:
        changed = (mode, caption) != (self.mode, self.caption)
        self.mode, self.caption = mode, caption
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

    def _angle(self, speed: float) -> float:
        frac = max(0.0, min(1.0, speed / self.max_speed))
        return 210.0 - 240.0 * frac  # stopnie, 0 = godzina 3, przeciwnie do wskazówek

    def paintEvent(self, _e: QEvent) -> None:
        t = self.t
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        side = min(self.width(), self.height() * 1.12)
        rect = QRectF((self.width() - side) / 2 + 34, 30, side - 68, side - 68)
        center = rect.center()
        radius = rect.width() / 2

        # tor
        p.setPen(QPen(QColor(t.line), 12, Qt.PenStyle.SolidLine, Qt.PenCapStyle.RoundCap))
        p.drawArc(rect, 210 * 16, -240 * 16)
        # wartość
        color = {"walk": t.accent, "run": t.run}.get(self.mode, t.text_2)
        span = self._angle(self._value) - 210.0
        if self._value > 0.05:
            p.setPen(QPen(QColor(color), 12, Qt.PenStyle.SolidLine, Qt.PenCapStyle.RoundCap))
            p.drawArc(rect, 210 * 16, round(span * 16))
        # znaczniki progów
        small = QFont(p.font())
        small.setPixelSize(12)
        p.setFont(small)
        for speed, text in ((self.walk_from, "chód"), (self.run_from, "bieg")):
            a = math.radians(self._angle(speed))
            inner = QPointF(
                center.x() + (radius - 16) * math.cos(a), center.y() - (radius - 16) * math.sin(a)
            )
            outer = QPointF(
                center.x() + (radius + 8) * math.cos(a), center.y() - (radius + 8) * math.sin(a)
            )
            p.setPen(QPen(QColor(t.text_2), 2))
            p.drawLine(inner, outer)
            lbl = QPointF(
                center.x() + (radius + 24) * math.cos(a), center.y() - (radius + 24) * math.sin(a)
            )
            p.drawText(
                QRectF(lbl.x() - 24, lbl.y() - 9, 48, 18), Qt.AlignmentFlag.AlignCenter, text
            )

        # liczba
        big = tabular(QFont(p.font()))
        big.setPixelSize(round(radius * 0.52))
        big.setWeight(QFont.Weight.DemiBold)
        p.setFont(big)
        p.setPen(QColor(t.text))
        number = fmt_num(self._value)
        h = QFontMetricsF(big).height()
        p.drawText(
            QRectF(rect.left(), center.y() - h * 0.62, rect.width(), h),
            Qt.AlignmentFlag.AlignCenter,
            number,
        )
        unit = QFont(p.font())
        unit.setPixelSize(14)
        unit.setWeight(QFont.Weight.Normal)
        p.setFont(unit)
        p.setPen(QColor(t.text_2))
        p.drawText(
            QRectF(rect.left(), center.y() + h * 0.30, rect.width(), 20),
            Qt.AlignmentFlag.AlignCenter,
            "km/h",
        )
        mode_font = QFont(p.font())
        mode_font.setPixelSize(16)
        mode_font.setWeight(QFont.Weight.DemiBold)
        p.setFont(mode_font)
        p.setPen(QColor(color if self.mode in ("walk", "run") else t.text_2))
        p.drawText(
            QRectF(0, center.y() + radius * 0.52, self.width(), 24),  # cała szerokość karty
            Qt.AlignmentFlag.AlignCenter,
            self.caption,
        )


class Belt(QWidget):
    """Taśma bieżni: paski przesuwają się z prędkością proporcjonalną do taśmy."""

    PX_PER_KMH = 14.0

    def __init__(self, t: Theme) -> None:
        super().__init__()
        self.t = t
        self.setFixedHeight(28)
        self.speed = 0.0
        self.offset = 0.0
        self._timer = QTimer(self)
        self._timer.setInterval(33)
        self._timer.timeout.connect(self._tick)
        self.setAccessibleName("Animacja taśmy")

    def set_speed(self, speed: float) -> None:
        self.speed = speed
        running = speed > 0.05 and Motion.enabled
        if running and not self._timer.isActive():
            self._timer.start()
        elif not running and self._timer.isActive():
            self._timer.stop()
            self.update()

    def _tick(self) -> None:
        self.offset = (self.offset + self.speed * self.PX_PER_KMH * 0.033) % 24
        self.update()

    def paintEvent(self, _e: QEvent) -> None:
        t = self.t
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        r = QRectF(self.rect()).adjusted(1, 1, -1, -1)
        path = QPainterPath()
        path.addRoundedRect(r, r.height() / 2, r.height() / 2)
        p.setClipPath(path)
        p.fillRect(r, QColor(t.bg))
        stripe = QColor(t.control if self.speed > 0.05 else t.line)
        p.setPen(QPen(stripe, 3))
        x = -self.offset
        while x < r.width() + 24:
            p.drawLine(QPointF(x + 8, r.top() + 6), QPointF(x, r.bottom() - 6))
            x += 24
        p.setClipping(False)
        p.setPen(QPen(QColor(t.control), 2))
        p.setBrush(Qt.BrushStyle.NoBrush)
        p.drawPath(path)


class Sparkline(QWidget):
    """Prędkość z ostatniej minuty + linia progu biegu."""

    def __init__(self, t: Theme) -> None:
        super().__init__()
        self.t = t
        self.setFixedHeight(40)
        self.points: list[tuple[float, float]] = []
        self.max_speed = 10.0
        self.run_from = 6.3
        self.setAccessibleName("Wykres prędkości z ostatniej minuty")

    def set_points(self, points: list[tuple[float, float]]) -> None:
        self.points = points
        self.update()

    def paintEvent(self, _e: QEvent) -> None:
        t = self.t
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        w, h = self.width(), self.height()

        def y(speed: float) -> float:
            return h - 4 - (h - 8) * max(0.0, min(1.0, speed / self.max_speed))

        p.setPen(QPen(QColor(t.line), 1, Qt.PenStyle.DashLine))
        p.drawLine(QPointF(0, y(self.run_from)), QPointF(w, y(self.run_from)))
        small = QFont(p.font())
        small.setPixelSize(12)
        p.setFont(small)
        p.setPen(QColor(t.text_2))
        p.drawText(
            QRectF(0, y(self.run_from) - 16, w, 14),
            Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignBottom,
            "bieg",
        )
        p.drawText(
            QRectF(0, 0, w, 14),
            Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignTop,
            "ostatnia minuta",
        )
        if len(self.points) < 2:
            return
        now = self.points[-1][0]
        path = QPainterPath()
        for i, (ts, speed) in enumerate(self.points):
            x = w - (now - ts) / 60.0 * w
            if i == 0:
                path.moveTo(x, y(speed))
            else:
                path.lineTo(x, y(speed))
        p.setPen(QPen(QColor(t.accent), 2))
        p.drawPath(path)


class StatTile(QFrame):
    def __init__(self, title: str) -> None:
        super().__init__()
        self.setObjectName("card")
        lay = QVBoxLayout(self)
        lay.setContentsMargins(12, 10, 12, 10)
        lay.setSpacing(2)
        self.title = label(title, "small")
        self.value = label("-", "statValue")
        self.value.setWordWrap(False)  # liczba z jednostką zawsze w jednej linii
        self.value.setFont(tabular(self.value.font()))
        lay.addWidget(self.title)
        lay.addWidget(self.value)
        self.setAccessibleName(title)

    def set(self, text: str) -> None:
        if self.value.text() != text:
            self.value.setText(text)
            self.setAccessibleDescription(text)


class DeviceRow(QPushButton):
    """Wiersz na liście urządzeń: nazwa, rodzaj, siła sygnału."""

    def __init__(self, name: str, detail: str, t: Theme) -> None:
        super().__init__()
        self.setObjectName("row")
        self.setFixedHeight(60)
        self.setCursor(Qt.CursorShape.PointingHandCursor)
        self.setAccessibleName(f"{name}, {detail}")
        lay = QVBoxLayout(self)
        lay.setContentsMargins(14, 8, 14, 8)
        lay.setSpacing(0)
        title = label(name)
        title.setStyleSheet("font-weight: 600;")
        sub = label(detail, "small")
        for w in (title, sub):
            w.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
            lay.addWidget(w)


class ElidedLabel(QLabel):
    """Napis w jednej linii, skracany wielokropkiem, gdy brakuje miejsca.

    Nie rozpycha okna; pełna treść jest w podpowiedzi i w nazwie dla czytników ekranu.
    """

    def __init__(self, text: str = "", name: str = "") -> None:
        super().__init__()
        if name:
            self.setObjectName(name)
        self._full = ""
        self.setSizePolicy(QSizePolicy.Policy.Ignored, QSizePolicy.Policy.Preferred)
        self.setMinimumWidth(40)
        self.setText(text)

    def text(self) -> str:
        return self._full

    def setText(self, text: str) -> None:
        self._full = text
        self.setAccessibleName(text)
        self._elide()

    def _elide(self) -> None:
        shown = self.fontMetrics().elidedText(self._full, Qt.TextElideMode.ElideRight, self.width())
        self.setToolTip(self._full if shown != self._full else "")
        QLabel.setText(self, shown)

    def resizeEvent(self, e: QEvent) -> None:
        super().resizeEvent(e)  # type: ignore[arg-type]
        self._elide()

    def changeEvent(self, e: QEvent) -> None:
        super().changeEvent(e)
        if e.type() in (QEvent.Type.FontChange, QEvent.Type.StyleChange):
            self._elide()


class ResponsiveRow(QWidget):
    """Przyciski obok siebie, a gdy brakuje miejsca - jeden pod drugim. Nic nie jest ucinane."""

    def __init__(self, *widgets: QWidget, spacing: int = 8) -> None:
        super().__init__()
        self.widgets = widgets
        self.box = QBoxLayout(QBoxLayout.Direction.LeftToRight, self)
        self.box.setContentsMargins(0, 0, 0, 0)
        self.box.setSpacing(spacing)
        # Układ nie wymusza minimalnej szerokości = sumy przycisków (inaczej okno nie zwęzi się).
        self.box.setSizeConstraint(QLayout.SizeConstraint.SetNoConstraint)
        for w in widgets:
            # szerokość z podziału wiersza, nie z treści - zmiana napisu nie rusza układu
            policy = w.sizePolicy()
            policy.setHorizontalPolicy(QSizePolicy.Policy.Ignored)
            w.setSizePolicy(policy)
            self.box.addWidget(w, 1)
        self.setSizePolicy(QSizePolicy.Policy.Preferred, QSizePolicy.Policy.Fixed)

    def needed_width(self) -> int:
        # przyciski dzielą wiersz po równo, więc liczy się najszerszy z nich
        widths = [w.sizeHint().width() for w in self.widgets if not w.isHidden()]
        if not widths:
            return 0
        return max(widths) * len(widths) + self.box.spacing() * (len(widths) - 1)

    def minimumSizeHint(self) -> QSize:
        widest = max((w.minimumSizeHint().width() for w in self.widgets), default=0)
        return QSize(widest, super().minimumSizeHint().height())

    def resizeEvent(self, e: QEvent) -> None:
        horizontal = self.width() >= self.needed_width()
        want = QBoxLayout.Direction.LeftToRight if horizontal else QBoxLayout.Direction.TopToBottom
        if self.box.direction() != want:
            self.box.setDirection(want)
            self.updateGeometry()
        super().resizeEvent(e)  # type: ignore[arg-type]


def key_label(event: QKeyEvent) -> str:
    """Czytelna nazwa naciśniętego klawisza (litera albo nazwa z Qt)."""
    text = event.text()
    if text and text.isprintable() and not text.isspace():
        return text.upper()
    return QKeySequence(event.key()).toString()


class KeyButton(QPushButton):
    """Przycisk z klawiszem: kliknij, naciśnij nowy klawisz (Esc anuluje)."""

    captured = Signal(object)  # klawisze.Key

    def __init__(self, role: str, key: Key) -> None:
        super().__init__()
        self.role = role
        self.key = key
        self.capturing = False
        self.setFixedHeight(th.CONTROL_H)
        self.setCursor(Qt.CursorShape.PointingHandCursor)
        self.setFocusPolicy(Qt.FocusPolicy.StrongFocus)
        self.setToolTip("Kliknij i naciśnij nowy klawisz (Esc anuluje)")
        self.clicked.connect(self.start)
        self.show_key()

    def sizeHint(self) -> QSize:
        """Stały rozmiar: najdłuższy z napisów (nasłuchiwanie, typowe nazwy klawiszy)."""
        samples = (
            f"{self.role}: naciśnij klawisz…",
            f"{self.role}: Strzałka w prawo",
            f"{self.role}: {self.key.name}",
        )
        bold = QFont(self.font())
        bold.setWeight(QFont.Weight.DemiBold)
        fm = QFontMetricsF(bold)
        return QSize(round(max(fm.horizontalAdvance(t) for t in samples)) + 40, th.CONTROL_H)

    def show_key(self) -> None:
        self.setText(f"{self.role}: {self.key.name}")
        self.setAccessibleName(f"{self.role}: {self.key.name}. Kliknij, żeby zmienić klawisz")

    def set_key(self, key: Key) -> None:
        self.key = key
        self.show_key()

    def start(self) -> None:
        if self.capturing:
            return
        self.capturing = True
        self.setText(f"{self.role}: naciśnij klawisz…")
        self.grabKeyboard()

    def stop(self) -> None:
        if self.capturing:
            self.releaseKeyboard()
            self.capturing = False
        self.show_key()

    def keyPressEvent(self, e: QKeyEvent) -> None:
        if not self.capturing:
            super().keyPressEvent(e)
            return
        if e.isAutoRepeat():
            return
        if e.key() == Qt.Key.Key_Escape:
            self.stop()
            return
        key = key_from_native(e.nativeScanCode(), key_label(e))
        self.stop()
        if key is not None:
            self.captured.emit(key)

    def focusOutEvent(self, e: QFocusEvent) -> None:
        self.stop()
        super().focusOutEvent(e)


class TileGrid(QWidget):
    """Kafelki w jednym rzędzie, a w wąskim oknie w siatce 2 x 2."""

    def __init__(self, *tiles: QWidget, spacing: int = 8) -> None:
        super().__init__()
        self.tiles = tiles
        self.grid = QGridLayout(self)
        self.grid.setContentsMargins(0, 0, 0, 0)
        self.grid.setSpacing(spacing)
        self.grid.setSizeConstraint(QLayout.SizeConstraint.SetNoConstraint)
        self.columns = 0
        self._place(len(tiles))

    def _place(self, columns: int) -> None:
        if columns == self.columns:
            return
        self.columns = columns
        for tile in self.tiles:
            self.grid.removeWidget(tile)
        for i, tile in enumerate(self.tiles):
            self.grid.addWidget(tile, i // columns, i % columns)
        for c in range(len(self.tiles)):
            self.grid.setColumnStretch(c, 1 if c < columns else 0)
        self.updateGeometry()

    def needed_width(self, columns: int) -> int:
        widest = max(t.minimumSizeHint().width() for t in self.tiles)
        return widest * columns + self.grid.spacing() * (columns - 1)

    def minimumSizeHint(self) -> QSize:
        two = min(2, len(self.tiles))
        rows = -(-len(self.tiles) // two)
        tile_h = max(t.minimumSizeHint().height() for t in self.tiles)
        return QSize(self.needed_width(two), tile_h * rows + self.grid.spacing() * (rows - 1))

    def resizeEvent(self, e: QEvent) -> None:
        many = len(self.tiles)
        self._place(many if self.width() >= self.needed_width(many) else min(2, many))
        super().resizeEvent(e)  # type: ignore[arg-type]


class Slider(QSlider):
    """Suwak, którego nie zmienia przypadkowe kółko myszy przy przewijaniu strony.

    Kółko działa dopiero, gdy suwak ma ostrość (kliknięty albo wybrany klawiaturą);
    inaczej zdarzenie idzie dalej i przewija ustawienia.
    """

    def __init__(self, orientation: Qt.Orientation) -> None:
        super().__init__(orientation)
        self.setFocusPolicy(Qt.FocusPolicy.StrongFocus)  # bez fokusu od samego kółka

    def wheelEvent(self, e: QWheelEvent) -> None:
        if self.hasFocus():
            super().wheelEvent(e)
        else:
            e.ignore()


def fix_width(widget: QPushButton, texts: Iterable[str]) -> None:
    """Przycisk o stałej szerokości najdłuższego ze swoich napisów (zmienia się tylko treść)."""
    widget.ensurePolished()
    bold = QFont(widget.font())
    bold.setWeight(QFont.Weight.DemiBold)
    fm = QFontMetricsF(bold)
    widget.setFixedWidth(round(max(fm.horizontalAdvance(t) for t in texts)) + 40)
