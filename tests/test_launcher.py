"""Skrypty startowe dla testera: to, co Olek klika jako pierwsze, musi być poprawne."""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BAT = ROOT / "Uruchom.bat"
PS1 = ROOT / "narzedzia" / "bootstrap.ps1"


def test_launcher_runs_bootstrap_and_supervisor() -> None:
    text = BAT.read_text(encoding="ascii")
    assert r"narzedzia\bootstrap.ps1" in text
    assert "python -m bieznia.supervisor" in text
    assert "BIEZNIA_SYSTEM_TOOLS" in text
    assert "UV_PYTHON_PREFERENCE=only-managed" in text  # Python tylko z folderu aplikacji
    assert "for /l" not in text.split("rem Nie uzywaj")[1].split("\n", 1)[1]


def test_launcher_has_crlf_line_endings() -> None:
    data = BAT.read_bytes()
    assert data.count(b"\n") == data.count(b"\r\n"), "cmd.exe źle czyta .bat z końcami LF"


def test_scripts_are_plain_ascii() -> None:
    # PowerShell 5.1 czyta skrypty bez BOM jako ANSI, cmd.exe ma stronę kodową OEM.
    BAT.read_bytes().decode("ascii")
    PS1.read_bytes().decode("ascii")


def test_bootstrap_pins_versions_and_checksums() -> None:
    text = PS1.read_text(encoding="ascii")
    urls = re.findall(r"Url\s*=\s*'([^']+)'", text)
    shas = re.findall(r"Sha\s*=\s*'([^']+)'", text)
    assert len(urls) == len(shas) == 2
    assert all(u.startswith("https://github.com/") and "/download/" in u for u in urls)
    assert all(re.fullmatch(r"[0-9a-f]{64}", s) for s in shas)
    assert "latest" not in text.lower()
