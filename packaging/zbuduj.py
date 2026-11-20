"""Buduje Bieznia.exe (PyInstaller) i instalator (Inno Setup), liczy SHA-256.

Użycie:  uv run python packaging/zbuduj.py 0.2.0
Wynik:   dist/Bieznia-Setup-0.2.0.exe (+ .sha256), folder build/dist/Bieznia
Inno Setup: ISCC.exe z PATH, zmiennej ISCC albo standardowej lokalizacji.
"""

from __future__ import annotations

import hashlib
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BUILD = ROOT / "build"
DIST = ROOT / "dist"


def find_iscc() -> Path | None:
    candidates = [os.environ.get("ISCC", ""), shutil.which("ISCC") or ""]
    for base in (
        os.environ.get("PROGRAMFILES(X86)", ""),
        os.environ.get("PROGRAMFILES", ""),
        os.path.join(os.environ.get("LOCALAPPDATA", ""), "Programs"),
    ):
        for name in ("Inno Setup 7", "Inno Setup 6"):
            candidates.append(os.path.join(base, name, "ISCC.exe"))
    return next((Path(c) for c in candidates if c and Path(c).is_file()), None)


def size_mb(path: Path) -> float:
    if path.is_file():
        return path.stat().st_size / 1e6
    return sum(f.stat().st_size for f in path.rglob("*") if f.is_file()) / 1e6


def main() -> int:
    version = sys.argv[1] if len(sys.argv) > 1 else "0.0.0"
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        print("Wersja w formacie 1.2.3")
        return 2
    BUILD.mkdir(exist_ok=True)
    DIST.mkdir(exist_ok=True)
    (BUILD / "wersja.txt").write_text(version, encoding="utf-8")
    env = {**os.environ, "BIEZNIA_VERSION": version}
    subprocess.run(
        [
            sys.executable,
            "-m",
            "PyInstaller",
            "--noconfirm",
            "--clean",
            "--distpath",
            str(BUILD / "dist"),
            "--workpath",
            str(BUILD / "work"),
            str(ROOT / "packaging" / "bieznia.spec"),
        ],
        check=True,
        env=env,
        cwd=ROOT,
    )
    app = BUILD / "dist" / "Bieznia"
    print(f"Aplikacja: {app} ({size_mb(app):.0f} MB)")

    iscc = find_iscc()
    if iscc is None:
        print("Brak Inno Setup (ISCC.exe) - pomijam instalator.")
        return 0
    subprocess.run(
        [str(iscc), f"/DAppVersion={version}", str(ROOT / "packaging" / "instalator.iss")],
        check=True,
        cwd=ROOT / "packaging",
    )
    setup = DIST / f"Bieznia-Setup-{version}.exe"
    digest = hashlib.sha256(setup.read_bytes()).hexdigest()
    (DIST / f"{setup.name}.sha256").write_text(f"{digest}  {setup.name}\n", encoding="ascii")
    print(f"Instalator: {setup} ({size_mb(setup):.1f} MB), SHA-256 {digest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
