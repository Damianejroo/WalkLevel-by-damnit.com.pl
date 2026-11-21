"""Ikona i nazwa profilu z gry: folder (albo .exe, albo skrót .lnk) -> główny .exe -> ikona PNG.

Tylko odczyt plików gry. Ikona trafia do folderu danych osoby jako PNG 128 px.
"""

from __future__ import annotations

import ctypes
import logging
import os
import re
import sys
from dataclasses import dataclass
from pathlib import Path

from bieznia import paleta

log = logging.getLogger("bieznia.ikony")

MAX_DEPTH = 4
ICON_PX = 128
# Pliki i foldery, które na pewno nie są grą.
SKIP = re.compile(
    r"unins|setup|install|redist|vc_?redist|dxsetup|prereq|crash|report|easyanticheat|eac_|"
    r"battleye|be(service|launcher)|helper|updater|dotnet|cefprocess|webhelper|unitycrashhandler|"
    r"quicksfv|oalinst|physx|touchup|cleanup|benchmark|editor|server|dedicated",
    re.IGNORECASE,
)
# Foldery techniczne, z których nazwa profilu nie powinna się brać.
GENERIC_DIRS = {"bin", "x64", "x86", "win64", "win32", "binaries", "game", "_retail_", "release"}


@dataclass(frozen=True)
class FoundGame:
    exe: Path
    name: str  # proponowana nazwa profilu (z nazwy folderu gry)


def _icon_count(path: Path) -> int:
    if sys.platform != "win32":
        return 1
    shell32 = ctypes.WinDLL("shell32")
    shell32.ExtractIconExW.restype = ctypes.c_uint
    return int(shell32.ExtractIconExW(str(path), -1, None, None, 0))


def _norm(s: str) -> str:
    return re.sub(r"[^a-z0-9]", "", s.lower())


def pick_exe(folder: Path, max_depth: int = MAX_DEPTH) -> Path | None:
    """Najbardziej prawdopodobny plik gry: z ikoną, nazwą jak folder, płytko, duży."""
    best: tuple[float, Path] | None = None
    base = _norm(folder.name)
    for root, dirs, files in os.walk(folder):
        depth = len(Path(root).relative_to(folder).parts)
        if depth >= max_depth:
            dirs[:] = []
        dirs[:] = [d for d in dirs if not SKIP.search(d) and not d.startswith(".")]
        for f in files:
            if not f.lower().endswith(".exe") or SKIP.search(f):
                continue
            path = Path(root) / f
            try:
                if _icon_count(path) == 0:
                    continue
                size_mb = path.stat().st_size / 1e6
            except OSError:
                continue
            name = _norm(path.stem.replace("-Win64-Shipping", "").replace("_Win64", ""))
            score = 0.0
            if name and (name in base or base in name):
                score += 6
            elif name and base and name[:4] == base[:4]:
                score += 2
            score -= depth * 1.2
            score += min(size_mb, 200) / 50
            if "launcher" in path.stem.lower():
                score -= 1.5
            if best is None or score > best[0]:
                best = (score, path)
    return best[1] if best else None


def game_name(path: Path) -> str:
    """Nazwa profilu z folderu gry (bez folderów technicznych typu bin, x64)."""
    folder = path if path.is_dir() else path.parent
    while folder.name.lower() in GENERIC_DIRS and folder.parent != folder:
        folder = folder.parent
    return folder.name or path.stem


def resolve(path: Path) -> FoundGame | None:
    """Folder, plik .exe albo skrót .lnk -> plik gry i proponowana nazwa."""
    if path.suffix.lower() == ".lnk":
        from PySide6.QtCore import QFileInfo

        target = QFileInfo(str(path)).symLinkTarget()
        if not target:
            return None
        path = Path(target)
    if path.is_dir():
        exe = pick_exe(path)
        return FoundGame(exe, game_name(path)) if exe else None
    if path.suffix.lower() == ".exe" and path.exists():
        return FoundGame(path, game_name(path))
    return None


def extract_icon(exe: Path, out_png: Path, size: int = ICON_PX) -> bool:
    """Ikona z pliku .exe -> PNG (puste brzegi przycięte, pikselowe ikony zostają ostre)."""
    from PySide6.QtCore import QFileInfo, QRect, QSize, Qt
    from PySide6.QtGui import QImage, QPixmap
    from PySide6.QtWidgets import QFileIconProvider

    icon = QFileIconProvider().icon(QFileInfo(str(exe)))
    pix = icon.pixmap(QSize(256, 256))
    if pix.isNull():
        return False
    img = pix.toImage().convertToFormat(QImage.Format.Format_ARGB32)
    xs: list[int] = []
    ys: list[int] = []
    for y in range(img.height()):
        for x in range(img.width()):
            if img.pixelColor(x, y).alpha() > 16:
                xs.append(x)
                ys.append(y)
    if not xs:
        return False
    side = max(max(xs) - min(xs), max(ys) - min(ys)) + 1
    cx, cy = (min(xs) + max(xs)) // 2, (min(ys) + max(ys)) // 2
    cropped = QPixmap.fromImage(img.copy(QRect(cx - side // 2, cy - side // 2, side, side)))
    mode = (
        Qt.TransformationMode.FastTransformation
        if side <= 64
        else Qt.TransformationMode.SmoothTransformation
    )
    out = cropped.scaled(size, size, Qt.AspectRatioMode.KeepAspectRatio, mode)
    out_png.parent.mkdir(parents=True, exist_ok=True)
    return bool(out.save(str(out_png), "PNG"))


def icon_color(png: Path) -> paleta.Kolor:
    """Kolor profilu z ikony: najczęstszy nasycony odcień -> najbliższy z palety."""
    from PySide6.QtGui import QImage

    img = QImage(str(png)).convertToFormat(QImage.Format.Format_ARGB32)
    pixels: list[tuple[int, int, int, int]] = []
    for y in range(0, img.height(), 2):
        for x in range(0, img.width(), 2):
            c = img.pixelColor(x, y)
            pixels.append((c.red(), c.green(), c.blue(), c.alpha()))
    return paleta.from_rgb_pixels(pixels)
