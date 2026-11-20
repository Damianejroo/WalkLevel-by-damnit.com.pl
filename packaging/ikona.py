"""Generuje ikonę aplikacji: packaging/bieznia.ico (16-256 px) i bieznia/gui/ikona.png.

Źródło: packaging/ikona-zrodlo.png (wygenerowana przez GPT image generation na podstawie
opisu z kolorami aplikacji: prędkościomierz nad taśmą bieżni). Bez źródła ikona jest
rysowana w kodzie (funkcja draw) w tym samym motywie.
Uruchom: uv run python packaging/ikona.py
"""

from __future__ import annotations

import struct
import sys
from pathlib import Path

from PySide6.QtCore import QBuffer, QByteArray, QIODevice, QPointF, QRectF, Qt
from PySide6.QtGui import QColor, QGuiApplication, QImage, QPainter, QPainterPath, QPen

ROOT = Path(__file__).resolve().parents[1]
BG = "#151514"
EDGE = "#33322e"
ACCENT = "#8fc2b0"
RUN = "#e6b46e"
BELT = "#ecebe6"
SIZES = (16, 20, 24, 32, 40, 48, 64, 128, 256)


def draw(size: int) -> QImage:
    img = QImage(size, size, QImage.Format.Format_ARGB32)
    img.fill(Qt.GlobalColor.transparent)
    p = QPainter(img)
    p.setRenderHint(QPainter.RenderHint.Antialiasing)
    s = size / 256  # rysujemy w siatce 256x256
    small = size <= 24

    # tło: zaokrąglony kwadrat
    p.setPen(QPen(QColor(EDGE), max(1.0, 6 * s)) if not small else Qt.PenStyle.NoPen)
    p.setBrush(QColor(BG))
    inset = 6 * s
    p.drawRoundedRect(QRectF(inset, inset, size - 2 * inset, size - 2 * inset), 56 * s, 56 * s)

    # łuk prędkościomierza (240 stopni), wypełniony do ok. 70%
    arc = QRectF(52 * s, 40 * s, 152 * s, 152 * s)
    width = (30 if small else 22) * s
    p.setBrush(Qt.BrushStyle.NoBrush)
    p.setPen(QPen(QColor(EDGE), width, Qt.PenStyle.SolidLine, Qt.PenCapStyle.RoundCap))
    p.drawArc(arc, 210 * 16, -240 * 16)
    p.setPen(QPen(QColor(ACCENT), width, Qt.PenStyle.SolidLine, Qt.PenCapStyle.RoundCap))
    p.drawArc(arc, 210 * 16, -150 * 16)
    if not small:
        # mały znacznik progu biegu
        p.setPen(QPen(QColor(RUN), 10 * s, Qt.PenStyle.SolidLine, Qt.PenCapStyle.RoundCap))
        p.drawArc(arc, 60 * 16, -18 * 16)

    # taśma bieżni z paskami
    belt = QRectF(44 * s, 186 * s, 168 * s, 34 * s)
    path = QPainterPath()
    path.addRoundedRect(belt, 17 * s, 17 * s)
    p.setPen(Qt.PenStyle.NoPen)
    p.setBrush(QColor(BELT))
    p.drawPath(path)
    if size >= 32:
        p.setClipPath(path)
        p.setPen(QPen(QColor(BG), 8 * s))
        x = belt.left() + 14 * s
        while x < belt.right():
            p.drawLine(QPointF(x + 10 * s, belt.top() + 6 * s), QPointF(x, belt.bottom() - 6 * s))
            x += 30 * s
    p.end()
    return img


def png_bytes(img: QImage) -> bytes:
    data = QByteArray()
    buf = QBuffer(data)
    buf.open(QIODevice.OpenModeFlag.WriteOnly)
    img.save(buf, "PNG")
    return bytes(data.data())


def write_ico(path: Path, images: list[tuple[int, bytes]]) -> None:
    """ICO z obrazami PNG (format Windows Vista+), bez dodatkowych bibliotek."""
    header = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    entries, blobs = b"", b""
    for size, blob in images:
        dim = 0 if size >= 256 else size  # 0 oznacza 256
        entries += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(blob), offset)
        offset += len(blob)
        blobs += blob
    path.write_bytes(header + entries + blobs)


SOURCE = ROOT / "packaging" / "ikona-zrodlo.png"


def render(size: int) -> QImage:
    if SOURCE.exists():
        src = QImage(str(SOURCE))
        return src.scaled(
            size,
            size,
            Qt.AspectRatioMode.KeepAspectRatio,
            Qt.TransformationMode.SmoothTransformation,
        )
    return draw(size)


WIZARD_SIZES = (55, 64, 83, 110)  # obrazek w nagłówku instalatora: 100/125/150/200% DPI


def wizard_image(size: int) -> QImage:
    """BMP bez przezroczystości (Inno Setup), ikona z marginesem na białym tle okna."""
    img = QImage(size, size, QImage.Format.Format_RGB32)
    img.fill(QColor("#ffffff"))
    p = QPainter(img)
    inner = round(size * 0.9)
    offset = (size - inner) // 2
    p.drawImage(offset, offset, render(inner))
    p.end()
    return img


def main() -> int:
    _app = QGuiApplication(sys.argv)
    images = [(size, png_bytes(render(size))) for size in SIZES]
    write_ico(ROOT / "packaging" / "bieznia.ico", images)
    render(256).save(str(ROOT / "bieznia" / "gui" / "ikona.png"))
    for size in WIZARD_SIZES:
        wizard_image(size).save(str(ROOT / "packaging" / f"instalator-{size}.bmp"), "BMP")
    print("Zapisano packaging/bieznia.ico i bieznia/gui/ikona.png")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
