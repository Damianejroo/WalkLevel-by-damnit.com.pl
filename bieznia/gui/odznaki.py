"""Elementy gamingowego wyglądu WalkLevel: heksagonalne odznaki z własnymi znakami, pasek XP
z podziałką, karty z gradientem stopnia i podpisy w stylu HUD.

Znaki na odznakach są rysowane w kodzie (proste kształty) - bez cudzych grafik.
"""

from __future__ import annotations

import math

from PySide6.QtCore import QPointF, QRectF, Qt, Signal
from PySide6.QtGui import (
    QBrush,
    QColor,
    QFont,
    QKeyEvent,
    QLinearGradient,
    QMouseEvent,
    QPainter,
    QPainterPath,
    QPaintEvent,
    QPen,
    QPolygonF,
    QRadialGradient,
)
from PySide6.QtWidgets import QFrame, QLabel, QWidget

from bieznia.gui.theme import Theme

NUM = "Bahnschrift"  # krój cyfr i podpisów (jest w Windows 10 i 11)
ACCENTS_ABOVE = "ÓŚĆŃŹŻ"  # wielkie litery z kreską/kropką nad literą
RARITY_COLORS = {
    "pospolite": "#9aa5b1",
    "rzadkie": "#4aa3ff",
    "epickie": "#b26bff",
    "heroiczne": "#ff5470",
    "legendarne": "#ffa133",
}
RARITY_NAMES = {
    "pospolite": "Pospolite",
    "rzadkie": "Rzadkie",
    "epickie": "Epickie",
    "heroiczne": "Heroiczne",
    "legendarne": "Legendarne",
}


def num_font(px: int, bold: bool = True) -> QFont:
    f = QFont(NUM)
    f.setPixelSize(max(1, px))
    f.setWeight(QFont.Weight.Bold if bold else QFont.Weight.DemiBold)
    return f


def rank_rarity(level: int) -> str:
    """Kolor odznaki poziomu: im wyżej, tym rzadszy stopień."""
    if level < 10:
        return "pospolite"
    if level < 25:
        return "rzadkie"
    if level < 50:
        return "epickie"
    if level < 80:
        return "heroiczne"
    return "legendarne"


# --- odznaka heksagonalna ---------------------------------------------------------------------


def hex_points(c: QPointF, r: float) -> list[QPointF]:
    return [
        QPointF(
            c.x() + r * math.cos(math.radians(-90 + 60 * i)),
            c.y() + r * math.sin(math.radians(-90 + 60 * i)),
        )
        for i in range(6)
    ]


def hex_path(c: QPointF, r: float) -> QPainterPath:
    p = QPainterPath()
    p.addPolygon(QPolygonF([*hex_points(c, r), hex_points(c, r)[0]]))
    return p


def glyph(p: QPainter, name: str, c: QPointF, s: float, color: QColor) -> None:
    """Znak w środku odznaki; s = połowa boku pola znaku."""
    p.save()
    p.translate(c)
    p.scale(s, s)  # współrzędne -1..1
    pen = QPen(
        color, 0.16, Qt.PenStyle.SolidLine, Qt.PenCapStyle.RoundCap, Qt.PenJoinStyle.RoundJoin
    )
    p.setPen(pen)
    p.setBrush(Qt.BrushStyle.NoBrush)
    fill = QBrush(color)

    def poly(pts: list[tuple[float, float]], filled: bool = True) -> None:
        p.setBrush(fill if filled else Qt.BrushStyle.NoBrush)
        p.drawPolygon(QPolygonF([QPointF(x, y) for x, y in pts]))
        p.setBrush(Qt.BrushStyle.NoBrush)

    if name == "stopy":
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(fill)
        for dx, dy in ((-0.38, 0.25), (0.38, -0.3)):
            p.drawEllipse(QRectF(dx - 0.22, dy - 0.2, 0.44, 0.62))
            for i, (tx, ty) in enumerate(((-0.17, -0.36), (-0.02, -0.44), (0.14, -0.4))):
                rr = 0.085 - i * 0.008
                p.drawEllipse(QPointF(dx + tx, dy + ty), rr, rr)
    elif name == "kopula":  # bariera nad kolonią
        p.drawLine(QPointF(-0.95, 0.55), QPointF(0.95, 0.55))
        p.drawArc(QRectF(-0.8, -0.85, 1.6, 2.8), 0, 180 * 16)
        p.setPen(QPen(color, 0.09))
        p.drawArc(QRectF(-0.45, -0.85, 0.9, 2.8), 0, 180 * 16)
        poly(
            [
                (-0.05, -0.15),
                (0.12, -0.15),
                (-0.02, 0.12),
                (0.1, 0.12),
                (-0.12, 0.42),
                (-0.03, 0.16),
                (-0.14, 0.16),
            ]
        )
    elif name == "helm":  # hełm z rogami
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(fill)
        path = QPainterPath()
        path.moveTo(-0.55, 0.45)
        path.cubicTo(-0.6, -0.45, 0.6, -0.45, 0.55, 0.45)
        path.closeSubpath()
        p.drawPath(path)
        for sgn in (-1, 1):
            horn = QPainterPath()
            horn.moveTo(sgn * 0.45, 0.05)
            horn.cubicTo(sgn * 0.85, 0.0, sgn * 0.95, -0.45, sgn * 0.8, -0.85)
            horn.cubicTo(sgn * 0.75, -0.45, sgn * 0.6, -0.25, sgn * 0.4, -0.2)
            p.drawPath(horn)
        p.setBrush(QColor(0, 0, 0, 90))
        p.drawRect(QRectF(-0.07, -0.05, 0.14, 0.55))
        p.setBrush(fill)
        p.drawRect(QRectF(-0.62, 0.42, 1.24, 0.14))
    elif name == "miecz":
        p.rotate(45)
        poly([(-0.1, 0.3), (-0.1, -0.75), (0.0, -0.95), (0.1, -0.75), (0.1, 0.3)])
        p.drawLine(QPointF(-0.35, 0.32), QPointF(0.35, 0.32))
        p.drawLine(QPointF(0, 0.35), QPointF(0, 0.72))
        p.setBrush(fill)
        p.drawEllipse(QPointF(0, 0.82), 0.1, 0.1)
    elif name == "dycha":
        p.setFont(num_font(1))
        f = QFont(NUM)
        f.setPixelSize(100)
        f.setWeight(QFont.Weight.Bold)
        p.scale(0.012, 0.012)
        p.setFont(f)
        p.setPen(color)
        p.drawText(QRectF(-100, -70, 200, 140), Qt.AlignmentFlag.AlignCenter, "10")
    elif name.startswith("wieniec"):  # maraton
        p.drawArc(QRectF(-0.75, -0.75, 1.5, 1.5), -200 * 16, 170 * 16)
        p.drawArc(QRectF(-0.75, -0.75, 1.5, 1.5), 30 * 16, 170 * 16)
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(fill)
        for side in (-1, 1):
            for k in range(4):
                ang = math.radians(200 - k * 38) if side < 0 else math.radians(-20 + k * 38)
                x, y = 0.75 * math.cos(ang), -0.75 * math.sin(ang)
                p.drawEllipse(QPointF(x * 1.0, y * 1.0), 0.13, 0.13)
        f = QFont(NUM)
        f.setPixelSize(100)
        f.setWeight(QFont.Weight.Bold)
        p.save()
        p.scale(0.0075, 0.0075)
        p.setFont(f)
        p.setPen(color)
        p.drawText(QRectF(-100, -70, 200, 140), Qt.AlignmentFlag.AlignCenter, name[7:])
        p.restore()
    elif name == "licznik":  # setka
        p.drawArc(QRectF(-0.85, -0.7, 1.7, 1.7), 0, 180 * 16)
        for k in range(7):
            ang = math.radians(180 - k * 30)
            p.drawLine(
                QPointF(0.85 * math.cos(ang), 0.15 - 0.85 * math.sin(ang)),
                QPointF(0.62 * math.cos(ang), 0.15 - 0.62 * math.sin(ang)),
            )
        p.setPen(QPen(color, 0.2, Qt.PenStyle.SolidLine, Qt.PenCapStyle.RoundCap))
        p.drawLine(QPointF(0, 0.15), QPointF(0.5, -0.38))
        p.setBrush(fill)
        p.drawEllipse(QPointF(0, 0.15), 0.14, 0.14)
        p.drawLine(QPointF(-0.6, 0.62), QPointF(0.6, 0.62))
    elif name == "luki":  # trzej łowcy - trzy strzały
        for dx in (-0.45, 0.0, 0.45):
            p.drawLine(QPointF(dx, 0.85), QPointF(dx, -0.55))
            poly([(dx - 0.18, -0.45), (dx, -0.9), (dx + 0.18, -0.45)])
            p.drawLine(QPointF(dx, 0.85), QPointF(dx - 0.15, 0.6))
            p.drawLine(QPointF(dx, 0.85), QPointF(dx + 0.15, 0.6))
    elif name == "mur":
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(fill)
        for row in range(4):
            y = -0.7 + row * 0.38
            off = 0.0 if row % 2 == 0 else 0.25
            x = -0.9 - off
            while x < 0.9:
                x0, x1 = max(x, -0.9), min(x + 0.46, 0.9)
                if x1 - x0 > 0.08:
                    p.drawRect(QRectF(x0 + 0.02, y, x1 - x0 - 0.04, 0.32))
                x += 0.5
        p.drawRect(QRectF(-0.9, -0.95, 0.3, 0.22))
        p.drawRect(QRectF(-0.15, -0.95, 0.3, 0.22))
        p.drawRect(QRectF(0.6, -0.95, 0.3, 0.22))
    elif name == "lisc":  # elfi liść
        path = QPainterPath()
        path.moveTo(0, -0.95)
        path.cubicTo(0.75, -0.45, 0.6, 0.45, 0, 0.75)
        path.cubicTo(-0.6, 0.45, -0.75, -0.45, 0, -0.95)
        p.setBrush(fill)
        p.setPen(Qt.PenStyle.NoPen)
        p.drawPath(path)
        p.setPen(QPen(QColor(0, 0, 0, 110), 0.08))
        p.drawLine(QPointF(0, -0.7), QPointF(0, 0.95))
        for k in range(3):
            y = -0.35 + k * 0.3
            p.drawLine(QPointF(0, y + 0.12), QPointF(-0.3, y - 0.05))
            p.drawLine(QPointF(0, y + 0.12), QPointF(0.3, y - 0.05))
        p.setPen(QPen(color, 0.12))
        p.drawLine(QPointF(0, 0.7), QPointF(0, 0.95))
    elif name == "gora":  # samotna góra ze smokiem nad nią (skrzydła)
        poly([(-0.95, 0.75), (-0.15, -0.45), (0.15, -0.1), (0.35, -0.3), (0.95, 0.75)])
        p.setPen(QPen(color, 0.11))
        path = QPainterPath()
        path.moveTo(-0.45, -0.7)
        path.quadTo(-0.2, -0.95, 0.0, -0.72)
        path.quadTo(0.2, -0.95, 0.45, -0.7)
        p.drawPath(path)
    elif name == "wulkan":
        poly([(-0.95, 0.8), (-0.3, -0.35), (0.3, -0.35), (0.95, 0.8)])
        p.setPen(Qt.PenStyle.NoPen)
        lava = QColor("#ff6a2a")
        lava.setAlpha(color.alpha())
        p.setBrush(lava)
        p.drawPolygon(
            QPolygonF(
                [
                    QPointF(-0.3, -0.35),
                    QPointF(0.3, -0.35),
                    QPointF(0.12, 0.05),
                    QPointF(0.0, -0.1),
                    QPointF(-0.15, 0.2),
                ]
            )
        )
        p.setBrush(fill)
        for x, y, rr in ((-0.2, -0.65, 0.11), (0.15, -0.8, 0.09), (0.35, -0.55, 0.07)):
            p.drawEllipse(QPointF(x, y), rr, rr)
    elif name == "pudlo":  # kurier - paczka na plecach
        poly(
            [(-0.7, -0.35), (0.0, -0.7), (0.7, -0.35), (0.7, 0.45), (0.0, 0.8), (-0.7, 0.45)], False
        )
        p.drawLine(QPointF(-0.7, -0.35), QPointF(0.0, 0.0))
        p.drawLine(QPointF(0.7, -0.35), QPointF(0.0, 0.0))
        p.drawLine(QPointF(0.0, 0.0), QPointF(0.0, 0.8))
        p.drawLine(QPointF(-0.35, -0.52), QPointF(0.35, -0.17))
    elif name == "wglab":  # do środka Ziemi
        p.drawEllipse(QPointF(0, 0), 0.85, 0.85)
        p.drawArc(QRectF(-0.85, -0.3, 1.7, 0.6), 0, 180 * 16)
        p.setBrush(fill)
        p.drawEllipse(QPointF(0, 0.15), 0.25, 0.25)
        p.drawLine(QPointF(0, -0.95), QPointF(0, -0.25))
        poly([(-0.15, -0.4), (0, -0.2), (0.15, -0.4)])
    elif name == "kostka":
        top = [(0, -0.9), (0.8, -0.45), (0, 0.0), (-0.8, -0.45)]
        left = [(-0.8, -0.45), (0, 0.0), (0, 0.9), (-0.8, 0.45)]
        right = [(0.8, -0.45), (0, 0.0), (0, 0.9), (0.8, 0.45)]
        p.setPen(Qt.PenStyle.NoPen)
        for pts, alpha in ((top, 255), (left, 170), (right, 110)):
            col = QColor(color)
            col.setAlpha(int(color.alpha() * alpha / 255))
            p.setBrush(col)
            p.drawPolygon(QPolygonF([QPointF(x, y) for x, y in pts]))
    elif name == "biegacz":  # ślad biegu przez kontynent
        path = QPainterPath()
        path.moveTo(-0.9, 0.6)
        path.cubicTo(-0.4, -0.4, 0.2, 0.7, 0.85, -0.55)
        p.setPen(QPen(color, 0.14, Qt.PenStyle.DashLine, Qt.PenCapStyle.RoundCap))
        p.drawPath(path)
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(fill)
        p.drawEllipse(QPointF(-0.9, 0.6), 0.14, 0.14)
        poly([(0.6, -0.75), (0.98, -0.65), (0.75, -0.3)])
    elif name == "glob":
        p.drawEllipse(QPointF(0, 0), 0.88, 0.88)
        p.drawEllipse(QRectF(-0.38, -0.88, 0.76, 1.76))
        p.drawLine(QPointF(-0.88, 0), QPointF(0.88, 0))
        p.drawArc(QRectF(-0.8, -0.75, 1.6, 0.7), 180 * 16, 180 * 16)
        p.drawArc(QRectF(-0.8, 0.05, 1.6, 0.7), 0, 180 * 16)
    elif name == "plomien":
        path = QPainterPath()
        path.moveTo(0, -0.95)
        path.cubicTo(0.2, -0.45, 0.8, -0.2, 0.6, 0.35)
        path.cubicTo(0.5, 0.75, 0.2, 0.9, 0, 0.9)
        path.cubicTo(-0.4, 0.9, -0.7, 0.6, -0.65, 0.2)
        path.cubicTo(-0.6, -0.1, -0.3, -0.2, -0.25, -0.5)
        path.cubicTo(0.05, -0.3, 0.05, -0.6, 0, -0.95)
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(fill)
        p.drawPath(path)
        p.setBrush(QColor(0, 0, 0, 80))
        p.drawEllipse(QPointF(0, 0.45), 0.25, 0.35)
    elif name == "pad":
        path = QPainterPath()
        path.addRoundedRect(QRectF(-0.9, -0.45, 1.8, 0.85), 0.35, 0.35)
        path.addEllipse(QPointF(-0.6, 0.35), 0.32, 0.42)
        path.addEllipse(QPointF(0.6, 0.35), 0.32, 0.42)
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(fill)
        p.drawPath(path.simplified())
        hole = QColor(0, 0, 0, 150)
        p.setBrush(hole)
        p.drawRect(QRectF(-0.62, -0.13, 0.36, 0.1))
        p.drawRect(QRectF(-0.49, -0.26, 0.1, 0.36))
        for x, y in ((0.45, -0.08), (0.62, -0.22), (0.62, 0.06), (0.79, -0.08)):
            p.drawEllipse(QPointF(x, y), 0.07, 0.07)
    elif name == "portale":  # wiele światów
        for x, y in ((-0.38, 0.22), (0.38, 0.22), (0.0, -0.42)):
            p.drawEllipse(QPointF(x, y), 0.46, 0.46)
    elif name == "dom":
        poly([(-0.85, 0.0), (0.0, -0.8), (0.85, 0.0)])
        p.drawRect(QRectF(-0.6, 0.0, 1.2, 0.8))
        p.setBrush(fill)
        p.drawRect(QRectF(-0.15, 0.35, 0.3, 0.45))
    elif name == "ksiezyc":
        path = QPainterPath()
        path.addEllipse(QPointF(0, 0), 0.8, 0.8)
        cut = QPainterPath()
        cut.addEllipse(QPointF(0.38, -0.25), 0.68, 0.68)
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(fill)
        p.drawPath(path.subtracted(cut))
    elif name == "slonce":
        p.setBrush(fill)
        p.drawEllipse(QPointF(0, 0.25), 0.38, 0.38)
        for k in range(5):
            ang = math.radians(180 - k * 45)
            p.drawLine(
                QPointF(0.58 * math.cos(ang), 0.25 - 0.58 * math.sin(ang)),
                QPointF(0.88 * math.cos(ang), 0.25 - 0.88 * math.sin(ang)),
            )
        p.drawLine(QPointF(-0.95, 0.72), QPointF(0.95, 0.72))
    elif name == "druzyna":
        p.setBrush(fill)
        p.setPen(Qt.PenStyle.NoPen)
        for x, sc in ((-0.42, 0.8), (0.42, 0.8), (0, 1.0)):
            p.drawEllipse(QPointF(x, -0.35 * sc), 0.22 * sc, 0.22 * sc)
            path = QPainterPath()
            path.addRoundedRect(
                QRectF(x - 0.36 * sc, 0.0, 0.72 * sc, 0.75 * sc), 0.3 * sc, 0.3 * sc
            )
            p.drawPath(path)
    elif name == "fala":  # rzeka
        for k in range(3):
            y = -0.5 + k * 0.5
            path = QPainterPath()
            path.moveTo(-0.9, y)
            path.cubicTo(-0.6, y - 0.3, -0.3, y + 0.3, 0.0, y)
            path.cubicTo(0.3, y - 0.3, 0.6, y + 0.3, 0.9, y)
            p.drawPath(path)
    elif name == "portal":  # rama z obsydianu
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(fill)
        for x in (-0.7, 0.5):
            p.drawRect(QRectF(x, -0.95, 0.2, 1.9))
        p.drawRect(QRectF(-0.7, -0.95, 1.4, 0.2))
        p.drawRect(QRectF(-0.7, 0.75, 1.4, 0.2))
        inner = QColor(color)
        inner.setAlpha(int(color.alpha() * 0.45))
        p.setBrush(inner)
        p.drawRect(QRectF(-0.5, -0.75, 1.0, 1.5))
        p.setPen(QPen(color, 0.08))
        p.setBrush(Qt.BrushStyle.NoBrush)
        p.drawArc(QRectF(-0.3, -0.45, 0.6, 0.9), 30 * 16, 260 * 16)
    elif name == "brama":  # brama w skale z gwiazdą
        path = QPainterPath()
        path.moveTo(-0.6, 0.85)
        path.lineTo(-0.6, -0.15)
        path.arcTo(QRectF(-0.6, -0.75, 1.2, 1.2), 180, -180)
        path.lineTo(0.6, 0.85)
        p.drawPath(path)
        p.drawLine(QPointF(0, -0.15), QPointF(0, 0.85))
        p.setBrush(fill)
        p.setPen(Qt.PenStyle.NoPen)
        star = []
        for k in range(10):
            rr = 0.24 if k % 2 == 0 else 0.1
            ang = math.radians(-90 + 36 * k)
            star.append(QPointF(rr * math.cos(ang), -0.3 + rr * math.sin(ang)))
        p.drawPolygon(QPolygonF(star))
    elif name == "drzwi":  # okrągłe drzwi norki
        p.drawEllipse(QPointF(0, 0.0), 0.78, 0.78)
        p.setBrush(fill)
        p.setPen(Qt.PenStyle.NoPen)
        p.drawEllipse(QPointF(0, 0.0), 0.55, 0.55)
        p.setBrush(QColor(0, 0, 0, 120))
        p.drawEllipse(QPointF(0.0, 0.0), 0.12, 0.12)
    elif name == "celownik":
        p.drawEllipse(QPointF(0, 0), 0.6, 0.6)
        for a1, b1 in (
            ((0, -0.95), (0, -0.3)),
            ((0, 0.3), (0, 0.95)),
            ((-0.95, 0), (-0.3, 0)),
            ((0.3, 0), (0.95, 0)),
        ):
            p.drawLine(QPointF(*a1), QPointF(*b1))
        p.setBrush(fill)
        p.drawEllipse(QPointF(0, 0), 0.08, 0.08)
    elif name == "cytadela":  # pięć ramion wokół pierścienia
        p.drawEllipse(QPointF(0, -0.1), 0.25, 0.25)
        for k in range(5):
            ang = math.radians(90 + k * 72)
            p.drawLine(
                QPointF(0.25 * math.cos(ang), -0.1 + 0.25 * math.sin(ang)),
                QPointF(0.92 * math.cos(ang), -0.1 + 0.92 * math.sin(ang)),
            )
    elif name == "gwiazda":  # stacja bojowa z czaszą
        p.setBrush(fill)
        p.setPen(Qt.PenStyle.NoPen)
        path = QPainterPath()
        path.addEllipse(QPointF(0, 0), 0.85, 0.85)
        cut = QPainterPath()
        cut.addEllipse(QPointF(-0.32, -0.3), 0.24, 0.24)
        p.drawPath(path.subtracted(cut))
        p.setBrush(QColor(0, 0, 0, 110))
        p.drawRect(QRectF(-0.85, -0.04, 1.7, 0.08))
        p.drawEllipse(QPointF(-0.32, -0.3), 0.08, 0.08)
    elif name == "dysk":
        p.drawEllipse(QRectF(-0.95, -0.25, 1.9, 0.6))
        p.setBrush(fill)
        p.drawEllipse(QRectF(-0.35, -0.55, 0.7, 0.45))
        p.setBrush(Qt.BrushStyle.NoBrush)
        for x in (-0.5, 0.0, 0.5):
            p.drawLine(QPointF(x, 0.4), QPointF(x * 0.9, 0.85))
    elif name == "pierscien":
        p.setPen(QPen(color, 0.22))
        p.drawEllipse(QRectF(-0.9, -0.45, 1.8, 0.9))
        p.setPen(QPen(color, 0.08))
        p.drawEllipse(QRectF(-0.55, -0.25, 1.1, 0.5))
    elif name == "planeta":
        p.drawEllipse(QPointF(0, 0), 0.8, 0.8)
        p.setBrush(fill)
        for x, y, rr in ((-0.3, -0.25, 0.16), (0.3, 0.1, 0.22), (-0.15, 0.4, 0.1)):
            p.drawEllipse(QPointF(x, y), rr, rr)
    elif name == "wieza":
        poly([(-0.45, 0.9), (-0.45, -0.5), (0.45, -0.5), (0.45, 0.9)])
        for x in (-0.45, -0.1, 0.25):
            p.setBrush(fill)
            p.drawRect(QRectF(x, -0.8, 0.2, 0.3))
        p.setBrush(QColor(0, 0, 0, 120))
        p.setPen(Qt.PenStyle.NoPen)
        p.drawRoundedRect(QRectF(-0.15, 0.35, 0.3, 0.55), 0.15, 0.15)
    elif name == "drzewo":
        p.setBrush(fill)
        p.setPen(Qt.PenStyle.NoPen)
        for x, y, rr in ((0, -0.4, 0.42), (-0.4, -0.05, 0.35), (0.4, -0.05, 0.35)):
            p.drawEllipse(QPointF(x, y), rr, rr)
        p.drawRect(QRectF(-0.1, 0.1, 0.2, 0.8))
    elif name == "kolo":
        p.drawEllipse(QPointF(0, 0), 0.82, 0.82)
        for k in range(8):
            ang = math.radians(k * 45)
            p.drawLine(QPointF(0, 0), QPointF(0.82 * math.cos(ang), 0.82 * math.sin(ang)))
        p.setBrush(fill)
        p.drawEllipse(QPointF(0, 0), 0.16, 0.16)
    elif name == "czerw":  # paszcza czerwia z zębami
        p.drawEllipse(QPointF(0, 0), 0.85, 0.85)
        p.setBrush(fill)
        p.setPen(Qt.PenStyle.NoPen)
        for k in range(12):
            ang = math.radians(k * 30)
            tip = QPointF(0.35 * math.cos(ang), 0.35 * math.sin(ang))
            base1 = QPointF(0.78 * math.cos(ang - 0.18), 0.78 * math.sin(ang - 0.18))
            base2 = QPointF(0.78 * math.cos(ang + 0.18), 0.78 * math.sin(ang + 0.18))
            p.drawPolygon(QPolygonF([tip, base1, base2]))
    elif name == "radiacja":  # znak promieniowania: trzy skrzydła wokół środka
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(fill)
        middle = QPainterPath()
        middle.addEllipse(QPointF(0, 0), 0.3, 0.3)
        for k in range(3):
            wing = QPainterPath()
            wing.moveTo(0, 0)
            wing.arcTo(QRectF(-0.9, -0.9, 1.8, 1.8), 60 + k * 120, 60)
            wing.closeSubpath()
            p.drawPath(wing.subtracted(middle))
        p.drawEllipse(QPointF(0, 0), 0.18, 0.18)
    elif name == "korona":
        poly(
            [
                (-0.85, 0.5),
                (-0.85, -0.4),
                (-0.42, 0.0),
                (0.0, -0.7),
                (0.42, 0.0),
                (0.85, -0.4),
                (0.85, 0.5),
            ]
        )
        p.drawLine(QPointF(-0.85, 0.72), QPointF(0.85, 0.72))
    p.restore()


class Hex(QWidget):
    """Odznaka: heksagon w kolorze stopnia, poświata i znak; zablokowana - ciemna, a postęp
    świeci po obwodzie."""

    def __init__(
        self,
        t: Theme,
        rarity: str,
        mark: str = "",
        locked: bool = False,
        progress: float | None = None,
        size: int = 58,
        text: str | None = None,
    ) -> None:
        super().__init__()
        self.t = t
        self.rarity = rarity
        self.mark = mark
        self.locked = locked
        self.progress = progress
        self.text = text
        self.setFixedSize(size, size)

    def paintEvent(self, _e: QPaintEvent) -> None:
        t = self.t
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        c = QPointF(self.width() / 2, self.height() / 2)
        r = self.width() * 0.40
        base = QColor(RARITY_COLORS.get(self.rarity, RARITY_COLORS["pospolite"]))
        if not self.locked:
            p.setPen(Qt.PenStyle.NoPen)
            for i in range(6, 0, -1):  # poświata
                g = QColor(base)
                g.setAlpha(16)
                p.setBrush(g)
                p.drawPath(hex_path(c, r + i * self.width() * 0.012))
            grad = QLinearGradient(c.x(), c.y() - r, c.x(), c.y() + r)
            grad.setColorAt(0, base.lighter(135))
            grad.setColorAt(0.55, base)
            grad.setColorAt(1, base.darker(190))
            p.setBrush(grad)
            p.setPen(QPen(base.lighter(160), max(1.5, self.width() * 0.03)))
            p.drawPath(hex_path(c, r))
            p.setPen(QPen(QColor(255, 255, 255, 60), 1))
            p.setBrush(Qt.BrushStyle.NoBrush)
            p.drawPath(hex_path(c, r * 0.82))
            ink = QColor("#ffffff")
        else:
            p.setBrush(QColor(t.surface_2))
            p.setPen(QPen(QColor(t.control), max(1.5, self.width() * 0.035)))
            p.drawPath(hex_path(c, r))
            if self.progress:
                pts = [*hex_points(c, r), hex_points(c, r)[0]]
                path = QPainterPath(pts[0])
                for q in pts[1:]:
                    path.lineTo(q)
                n = 90
                part = [path.pointAtPercent(min(1.0, k / n * self.progress)) for k in range(n + 1)]
                pen = QPen(base, max(2.0, self.width() * 0.05))
                pen.setCapStyle(Qt.PenCapStyle.RoundCap)
                p.setPen(pen)
                p.drawPolyline(QPolygonF(part))
            ink = QColor(t.text_2)
            ink.setAlpha(150)
        if self.text is not None:
            p.setFont(num_font(int(r * (0.95 if len(self.text) < 3 else 0.75))))
            box = QRectF(c.x() - r, c.y() - r, 2 * r, 2 * r)
            p.setPen(QColor(0, 0, 0, 120))
            p.drawText(box.translated(0, 2), Qt.AlignmentFlag.AlignCenter, self.text)
            p.setPen(ink)
            p.drawText(box, Qt.AlignmentFlag.AlignCenter, self.text)
        elif self.mark:
            if not self.locked:
                glyph(p, self.mark, c + QPointF(0, 1.5), r * 0.52, QColor(0, 0, 0, 110))
            glyph(p, self.mark, c, r * 0.52, ink)
        p.end()


class XPBar(QWidget):
    """Pasek z podziałką i świecącym końcem."""

    def __init__(
        self, t: Theme, value: float, color: str, height: int = 14, ticks: int = 10
    ) -> None:
        super().__init__()
        self.t = t
        self.value = value
        self.color = color
        self.ticks = ticks
        self.setFixedHeight(height)

    def set_value(self, value: float, color: str | None = None) -> None:
        self.value = value
        if color:
            self.color = color
        self.update()

    def paintEvent(self, _e: QPaintEvent) -> None:
        t = self.t
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        r = QRectF(self.rect()).adjusted(1, 1, -1, -1)
        rad = 3
        p.setPen(QPen(QColor(t.control), 1))
        p.setBrush(QColor(t.bg))
        p.drawRoundedRect(r, rad, rad)
        col = QColor(self.color)
        w = r.width() * max(0.0, min(1.0, self.value))
        if w > 0:
            fr = QRectF(r.x(), r.y(), w, r.height())
            g = QLinearGradient(fr.topLeft(), fr.topRight())
            g.setColorAt(0, col.darker(150))
            g.setColorAt(1, col.lighter(125))
            p.setPen(Qt.PenStyle.NoPen)
            p.setBrush(g)
            p.drawRoundedRect(fr, rad, rad)
            p.setBrush(QColor(255, 255, 255, 55))
            p.drawRect(QRectF(fr.x() + 1, fr.y() + 1, max(0.0, fr.width() - 2), fr.height() * 0.35))
            radius = r.height() * 1.6
            glow = QRadialGradient(QPointF(fr.right(), fr.center().y()), radius)
            gc = QColor(col.lighter(150))
            gc.setAlpha(170)
            glow.setColorAt(0, gc)
            glow.setColorAt(1, QColor(0, 0, 0, 0))
            p.setBrush(glow)
            p.drawEllipse(QPointF(fr.right(), fr.center().y()), radius, radius)
        if self.ticks > 1:
            p.setPen(QPen(QColor(t.bg), 2))
            for k in range(1, self.ticks):
                x = r.x() + r.width() * k / self.ticks
                p.drawLine(QPointF(x, r.y() + 1), QPointF(x, r.bottom() - 1))
        p.end()


class Panel(QFrame):
    """Karta z delikatnym gradientem w kolorze stopnia (albo akcentu) i paskiem z lewej.
    Po make_clickable: kursor, fokus z klawiatury i sygnał clicked."""

    clicked = Signal()

    def __init__(
        self, t: Theme, color: str | None, strength: int = 46, stripe: bool = True
    ) -> None:
        super().__init__()
        self.t = t
        self.color = color
        self.strength = strength
        self.stripe = stripe
        self.hover = False
        self.clickable = False

    def make_clickable(self, name: str) -> None:
        self.clickable = True
        self.setCursor(Qt.CursorShape.PointingHandCursor)
        self.setFocusPolicy(Qt.FocusPolicy.StrongFocus)
        self.setAccessibleName(name)
        self.setToolTip(name)

    def mouseReleaseEvent(self, e: QMouseEvent) -> None:
        if (
            self.clickable
            and e.button() == Qt.MouseButton.LeftButton
            and self.rect().contains(e.position().toPoint())
        ):
            self.clicked.emit()
        super().mouseReleaseEvent(e)

    def keyPressEvent(self, e: QKeyEvent) -> None:
        if self.clickable and e.key() in (Qt.Key.Key_Return, Qt.Key.Key_Enter, Qt.Key.Key_Space):
            self.clicked.emit()
            return
        super().keyPressEvent(e)

    def enterEvent(self, e: object) -> None:
        self.hover = True
        self.update()

    def leaveEvent(self, e: object) -> None:
        self.hover = False
        self.update()

    def paintEvent(self, _e: QPaintEvent) -> None:
        t = self.t
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        r = QRectF(self.rect()).adjusted(0.5, 0.5, -0.5, -0.5)
        lit = self.clickable and self.hover
        p.setPen(QPen(QColor(t.control if lit else t.line), 1))
        p.setBrush(QColor(t.surface_2 if lit else t.surface))
        p.drawRoundedRect(r, 10, 10)
        if self.color:
            col = QColor(self.color)
            g = QLinearGradient(r.topLeft(), r.topRight())
            c0 = QColor(col)
            c0.setAlpha(self.strength)
            g.setColorAt(0, c0)
            g.setColorAt(0.6, QColor(0, 0, 0, 0))
            p.setPen(Qt.PenStyle.NoPen)
            p.setBrush(g)
            p.drawRoundedRect(r, 10, 10)
            if self.stripe:
                p.setBrush(col)
                path = QPainterPath()
                path.addRoundedRect(r, 10, 10)
                clip = QPainterPath()
                clip.addRect(QRectF(r.x(), r.y(), 4, r.height()))
                p.drawPath(path.intersected(clip))
        p.end()


def caps(text: str, color: str, px: int = 12, spacing: float = 1.5, bold: bool = True) -> QLabel:
    """Podpis wersalikami z odstępami (jak w HUD-ach gier)."""
    lb = QLabel(text)
    f = num_font(px, bold=bold)
    f.setLetterSpacing(QFont.SpacingType.AbsoluteSpacing, spacing)
    lb.setFont(f)
    lb.setStyleSheet(
        f"color: {color}; background: transparent; font-family: '{NUM}'; font-size: {px}px;"
        f" font-weight: {700 if bold else 400};"
    )
    if any(ch in ACCENTS_ABOVE for ch in text):
        # Bahnschrift liczy wysokość bez kresek nad wielkimi literami (Ó, Ś, Ż...) - bez tego
        # zapasu QLabel ucina kreskę i „KROKÓW” wygląda jak „KROKOW”
        lb.setContentsMargins(0, max(2, px // 4), 0, 0)
    return lb


def big_num(text: str, color: str, px: int = 22) -> QLabel:
    lb = QLabel(text)
    lb.setFont(num_font(px))
    lb.setStyleSheet(
        f"color: {color}; background: transparent; font-family: '{NUM}';"
        f" font-size: {px}px; font-weight: 700;"
    )
    return lb


def name_label(text: str, px: int = 17, color: str | None = None) -> QLabel:
    lb = QLabel(text)
    lb.setWordWrap(True)
    tint = f" color: {color};" if color else ""
    lb.setStyleSheet(f"background: transparent; font-size: {px}px; font-weight: 700;{tint}")
    return lb


def soft(t: Theme, text: str, italic: bool = False) -> QLabel:
    lb = QLabel(text)
    lb.setWordWrap(True)
    style = "italic" if italic else "normal"
    color = t.text if italic else t.text_2
    lb.setStyleSheet(
        f"color: {color}; background: transparent; font-size: 13px; font-style: {style};"
    )
    return lb
