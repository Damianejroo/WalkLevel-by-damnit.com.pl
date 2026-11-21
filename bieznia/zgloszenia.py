"""Zgłoś problem: paczka z opisem i logami, wysyłana do autora albo dołączana do e-maila.

Co trafia do paczki (to samo pokazuje okno przed wysłaniem):
- opis problemu i opcjonalny kontakt wpisane przez użytkownika,
- wersja aplikacji, wersja Windows, ustawienia aplikacji (progi, wybrana bieżnia),
- logi aplikacji i Bluetooth z ostatnich 6 godzin (także starsze pliki logu),
- podsumowanie.txt: najważniejsze zdarzenia wyciągnięte z logów (polecenia, błędy, sesje),
- ostatni raport diagnostyczny.
Nic nie jest wysyłane bez kliknięcia Wyślij.
"""

from __future__ import annotations

import json
import os
import platform
import re
import subprocess
import sys
import time
import urllib.parse
import urllib.request
import uuid
import zipfile
from datetime import datetime
from pathlib import Path

from bieznia import APP_NAME, CONTACT, __version__

MAX_LOG_BYTES = 1_000_000  # z każdego pliku logu bierzemy tylko koniec
RECENT_S = 6 * 3600  # starsze pliki logu (bluetooth.log.1 ...) z ostatnich 6 godzin
# Linie, które trafiają do podsumowania - wyniki poleceń, połączenia, błędy.
SUMMARY_LINE = re.compile(
    r"Podsumowanie sesji|Start:|Stop:|Prędkość [0-9.]+:|nie przyjęła|nie zareagowała|"
    r"Połączenie |rozłączyła|Telemetria|Bieżnia \d|Zaktualizowano|WARNING|ERROR|CRITICAL"
)
MAX_REPORT_BYTES = 8 * 1024 * 1024
TIMEOUT_S = 30


class ReportError(Exception):
    """Komunikat dla użytkownika."""


def _tail(path: Path, limit: int = MAX_LOG_BYTES) -> bytes:
    with path.open("rb") as f:
        f.seek(0, os.SEEK_END)
        size = f.tell()
        f.seek(max(0, size - limit))
        return f.read()


def recent_logs(logs: Path, now: float | None = None) -> list[Path]:
    """Bieżące pliki logu i ich starsze części (np. bluetooth.log.1), jeśli są świeże."""
    if not logs.exists():
        return []
    now = time.time() if now is None else now
    files = sorted(logs.glob("*.log"))
    for old in sorted(logs.glob("*.log.[0-9]")):
        if now - old.stat().st_mtime <= RECENT_S:
            files.append(old)
    return files


def summary_text(logs: list[Path]) -> str:
    """Najważniejsze linie z logów aplikacji - od najstarszych do najnowszych."""
    lines: list[str] = []
    for path in sorted(
        (p for p in logs if p.name.startswith("bieznia.log")),
        key=lambda p: p.stat().st_mtime,
    ):
        text = _tail(path).decode("utf-8", errors="replace")
        lines += [line for line in text.splitlines() if SUMMARY_LINE.search(line)]
    counts = {
        "polecenia przyjęte": sum("przyjęt" in line for line in lines),
        "bez reakcji": sum("nie przyjęła" in line or "nie zareagowała" in line for line in lines),
        "rozłączenia": sum("rozłączyła" in line for line in lines),
        "błędy": sum(" ERROR " in line or " CRITICAL " in line for line in lines),
    }
    head = " · ".join(f"{k}: {v}" for k, v in counts.items())
    return f"{head}\n\n" + "\n".join(lines[-2000:]) + "\n"


def report_files(root: Path = Path()) -> list[Path]:
    logs = root / "logs"
    files = recent_logs(logs)
    reports = sorted(logs.glob("diagnostyka-*.txt")) if logs.exists() else []
    if reports:
        files.append(reports[-1])
    settings = root / "dane" / "ustawienia.json"
    if settings.exists():
        files.append(settings)
    return files


def build_report(description: str, contact: str = "", root: Path = Path()) -> Path:
    """Tworzy dane/zgloszenia/zgloszenie-....zip i zwraca jego ścieżkę."""
    stamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    out_dir = root / "dane" / "zgloszenia"
    out_dir.mkdir(parents=True, exist_ok=True)
    path = out_dir / f"zgloszenie-{stamp}.zip"
    info = {
        "opis": description.strip(),
        "kontakt": contact.strip(),
        "wersja": __version__,
        "windows": platform.platform(),
        "python": sys.version.split()[0],
        "czas": datetime.now().isoformat(timespec="seconds"),
    }
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        zf.writestr("zgloszenie.json", json.dumps(info, ensure_ascii=False, indent=2))
        files = report_files(root)
        logs = [f for f in files if ".log" in f.name]
        if logs:
            zf.writestr("podsumowanie.txt", summary_text(logs))
        for file in files:
            zf.writestr(f"{file.parent.name}/{file.name}", _tail(file))
    if path.stat().st_size > MAX_REPORT_BYTES:
        path.unlink()
        raise ReportError("Paczka z logami jest za duża - usuń stare logi i spróbuj ponownie.")
    return path


def _multipart(fields: dict[str, str], file: Path) -> tuple[bytes, str]:
    boundary = uuid.uuid4().hex
    crlf = "\r\n"
    parts: list[bytes] = []
    for name, value in fields.items():
        head = f'--{boundary}{crlf}Content-Disposition: form-data; name="{name}"{crlf}{crlf}'
        parts.append(f"{head}{value}{crlf}".encode())
    head = (
        f"--{boundary}{crlf}"
        f'Content-Disposition: form-data; name="plik"; filename="{file.name}"{crlf}'
        f"Content-Type: application/zip{crlf}{crlf}"
    )
    parts.append(head.encode() + file.read_bytes() + crlf.encode())
    parts.append(f"--{boundary}--{crlf}".encode())
    return b"".join(parts), f"multipart/form-data; boundary={boundary}"


def send_report(report: Path, endpoint: str, description: str, contact: str = "") -> None:
    """Wysyła paczkę metodą POST (multipart) na adres HTTPS autora."""
    if urllib.parse.urlparse(endpoint).scheme != "https":
        raise ReportError("Adres zgłoszeń musi zaczynać się od https://")
    body, content_type = _multipart(
        {
            "wersja": __version__,
            "opis": description.strip()[:4000],
            "kontakt": contact.strip()[:200],
        },
        report,
    )
    request = urllib.request.Request(
        endpoint,
        data=body,
        method="POST",
        headers={"Content-Type": content_type, "User-Agent": f"Bieznia/{__version__}"},
    )
    try:
        with urllib.request.urlopen(request, timeout=TIMEOUT_S) as response:
            if response.status >= 300:
                raise ReportError(f"Serwer odrzucił zgłoszenie (kod {response.status}).")
    except ReportError:
        raise
    except Exception as exc:
        raise ReportError("Nie udało się wysłać - sprawdź internet albo wyślij e-mailem.") from exc


def mailto_link(description: str, report: Path) -> str:
    subject = f"{APP_NAME} {__version__} - zgłoszenie problemu"
    body = (
        f"{description.strip()}\n\n"
        f"Paczka z logami do dołączenia: {report.name}\n"
        f"(folder otworzył się obok - przeciągnij plik do tej wiadomości)\n"
    )
    query = urllib.parse.urlencode({"subject": subject, "body": body}, quote_via=urllib.parse.quote)
    return f"mailto:{CONTACT}?{query}"


def open_email(description: str, report: Path) -> None:
    """Otwiera program pocztowy z gotową wiadomością i pokazuje paczkę w Eksploratorze."""
    if sys.platform != "win32":
        return
    os.startfile(mailto_link(description, report))
    subprocess.Popen(["explorer", f"/select,{report.resolve()}"])
