# PyInstaller: Bieznia.exe (folder, bez konsoli). Budowanie: uv run python packaging/zbuduj.py
# Odchudzone: z Qt zostaje tylko Core/Gui/Widgets i to, czego te moduły naprawdę używają.
# ruff: noqa
import os
from pathlib import Path

from PyInstaller.utils.win32.versioninfo import (
    FixedFileInfo,
    StringFileInfo,
    StringStruct,
    StringTable,
    VarFileInfo,
    VarStruct,
    VSVersionInfo,
)

ROOT = Path(SPECPATH).parent
VERSION = os.environ.get("BIEZNIA_VERSION", "0.0.0")
NUMS = tuple(int(x) for x in VERSION.split(".")) + (0,)

# Moduły Qt i biblioteki, których aplikacja nie używa (ok. 40 MB mniej).
EXCLUDES = [
    "PySide6.QtNetwork", "PySide6.QtQml", "PySide6.QtQuick", "PySide6.QtQuickWidgets",
    "PySide6.QtSvg", "PySide6.QtSvgWidgets", "PySide6.QtPdf", "PySide6.QtOpenGL",
    "PySide6.QtOpenGLWidgets", "PySide6.QtDBus", "PySide6.QtConcurrent", "PySide6.QtSql",
    "PySide6.QtTest", "PySide6.QtXml", "PySide6.QtPrintSupport",
    "tkinter", "unittest", "pydoc", "doctest", "lib2to3", "pytest", "mypy", "ruff",
]
DROP = (
    "opengl32sw.dll", "Qt6Network", "Qt6Svg", "Qt6Pdf", "Qt6Qml", "Qt6Quick", "Qt6OpenGL",
    "libcrypto-3-x64.dll", "libssl-3-x64.dll",  # kopia OpenSSL dla QtNetwork
    "/translations/", "\\translations\\",
    "plugins/tls", "plugins\\tls", "plugins/networkinformation", "plugins\\networkinformation",
    "plugins/iconengines", "plugins\\iconengines", "plugins/generic", "plugins\\generic",
    "qminimal", "qoffscreen", "qdirect2d",
)
KEEP_IMAGE_PLUGINS = ("qico",)


def keep(entry):
    name = entry[0].replace("\\", "/")
    if any(d.replace("\\", "/") in name for d in DROP):
        return False
    if "plugins/imageformats/" in name:
        return any(k in name for k in KEEP_IMAGE_PLUGINS)
    return True


a = Analysis(
    [str(ROOT / "bieznia" / "__main__.py")],
    pathex=[str(ROOT)],
    datas=[
        (str(ROOT / "config.toml"), "."),
        (str(ROOT / "build" / "wersja.txt"), "."),
        (str(ROOT / "bieznia" / "gui" / "ikona.png"), "bieznia/gui"),
        # profile modeli bieżni (dane JSON - zakresy prędkości, odliczanie, nazwy)
        (str(ROOT / "bieznia" / "sterowniki" / "profile" / "*.json"), "bieznia/sterowniki/profile"),
        # poziomy, rangi i osiągnięcia (definicje z wersją, wspólne z serwerem)
        (str(ROOT / "bieznia" / "poziomy" / "definicje.json"), "bieznia/poziomy"),
        # dźwięki popupów (osiągnięcie, awans) - generuje packaging/dzwieki.py
        (str(ROOT / "bieznia" / "gui" / "dzwieki" / "*.wav"), "bieznia/gui/dzwieki"),
    ],
    hiddenimports=["bleak.backends.winrt.scanner", "bleak.backends.winrt.client"],
    excludes=EXCLUDES,
    noarchive=False,
)
a.binaries = [b for b in a.binaries if keep(b)]
a.datas = [d for d in a.datas if keep(d)]
pyz = PYZ(a.pure)

version_info = VSVersionInfo(
    ffi=FixedFileInfo(filevers=NUMS, prodvers=NUMS),
    kids=[
        StringFileInfo([
            StringTable("041504B0", [
                StringStruct("CompanyName", "damnit.com.pl Damian Skrzypiec"),
                StringStruct("FileDescription", "WalkLevel by damnit.com.pl"),
                StringStruct("FileVersion", VERSION),
                StringStruct("InternalName", "Bieznia"),
                StringStruct("LegalCopyright", "© 2026 damnit.com.pl Damian Skrzypiec, GPL-3.0"),
                StringStruct("OriginalFilename", "Bieznia.exe"),
                StringStruct("ProductName", "WalkLevel"),
                StringStruct("ProductVersion", VERSION),
            ])
        ]),
        VarFileInfo([VarStruct("Translation", [0x0415, 1200])]),
    ],
)

exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name="Bieznia",
    icon=str(ROOT / "packaging" / "bieznia.ico"),
    version=version_info,
    console=False,
    upx=False,  # UPX częściej wywołuje fałszywe alarmy antywirusów
)
coll = COLLECT(exe, a.binaries, a.datas, name="Bieznia", upx=False)
