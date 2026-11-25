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
import urllib.error
import urllib.parse
import urllib.request
import uuid
import zipfile
from datetime import datetime
from pathlib import Path

from bieznia import APP_NAME, CONTACT, __version__

MAX_LOG_BYTES = 1_000_000  # z każdego pliku logu bierzemy tylko koniec
RECENT_S = 6 * 3600  # domyślny okres logów w zgłoszeniu: ostatnie 6 godzin
PERIODS = (
    (3600, "1 godzina"),
    (6 * 3600, "6 godzin"),
    (24 * 3600, "24 godziny"),
    (7 * 86400, "7 dni"),
)
DATED = re.compile(rb"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})")
# Linie, które trafiają do podsumowania - wyniki poleceń, połączenia, błędy.
SUMMARY_LINE = re.compile(
    r"Podsumowanie sesji|Start:|Stop:|Prędkość [0-9.]+:|nie przyjęła|nie zareagowała|"
    r"Połączenie |rozłączyła|Telemetria|Bieżnia \d|WalkLevel \d|Zaktualizowano|"
    r"WARNING|ERROR|CRITICAL"
)
MAX_REPORT_BYTES = 2 * 1024 * 1024  # limit serwera zgłoszeń
TAIL_LIMITS = (MAX_LOG_BYTES, 300_000, 80_000)  # za duża paczka - krótsze końcówki logów
# adresy MAC urządzeń Bluetooth (cudze maskujemy, zostaje tylko wybrana bieżnia)
MAC = re.compile(rb"(?i)(?<![0-9a-f:-])(?:[0-9a-f]{2}[:-]){5}[0-9a-f]{2}(?![0-9a-f:-])")
MASKED_MAC = b"XX:XX:XX:XX:XX:XX"
TIMEOUT_S = 30


class ReportError(Exception):
    """Komunikat dla użytkownika."""


def _tail(path: Path, limit: int = MAX_LOG_BYTES) -> bytes:
    with path.open("rb") as f:
        f.seek(0, os.SEEK_END)
        size = f.tell()
        f.seek(max(0, size - limit))
        return f.read()


def recent_logs(logs: Path, now: float | None = None, period_s: float = RECENT_S) -> list[Path]:
    """Bieżące pliki logu i ich starsze części (np. bluetooth.log.1) zmienione w okresie."""
    if not logs.exists():
        return []
    now = time.time() if now is None else now
    files = sorted(logs.glob("*.log"))
    for old in sorted(logs.glob("*.log.[0-9]")):
        if now - old.stat().st_mtime <= period_s:
            files.append(old)
    return files


def tail_since(path: Path, since: float, limit: int = MAX_LOG_BYTES) -> bytes:
    """Końcówka logu od chwili since. Linie bez daty (np. traceback) idą za poprzednią.
    Stary log bez dat: cały koniec pliku, jeśli plik zmienił się w okresie."""
    data = _tail(path, limit)
    lines = data.split(b"\n")
    dated = False
    for i, line in enumerate(lines):
        m = DATED.match(line)
        if not m:
            continue
        dated = True
        try:
            at = datetime.strptime(m.group(1).decode(), "%Y-%m-%d %H:%M:%S").timestamp()
        except ValueError:
            continue
        if at >= since:
            return b"\n".join(lines[i:])
    if dated:
        return b""
    return data if path.stat().st_mtime >= since else b""


def summary_text(logs: list[Path], since: float = 0.0) -> str:
    """Najważniejsze linie z logów aplikacji - od najstarszych do najnowszych."""
    lines: list[str] = []
    for path in sorted(
        (p for p in logs if p.name.startswith("bieznia.log")),
        key=lambda p: p.stat().st_mtime,
    ):
        text = tail_since(path, since).decode("utf-8", errors="replace")
        lines += [line for line in text.splitlines() if SUMMARY_LINE.search(line)]
    counts = {
        "polecenia przyjęte": sum("przyjęt" in line for line in lines),
        "bez reakcji": sum("nie przyjęła" in line or "nie zareagowała" in line for line in lines),
        "rozłączenia": sum("rozłączyła" in line for line in lines),
        "błędy": sum(" ERROR " in line or " CRITICAL " in line for line in lines),
    }
    head = " · ".join(f"{k}: {v}" for k, v in counts.items())
    return f"{head}\n\n" + "\n".join(lines[-2000:]) + "\n"


def report_files(root: Path = Path(), period_s: float = RECENT_S) -> list[Path]:
    logs = root / "logs"
    files = recent_logs(logs, period_s=period_s)
    reports = sorted(logs.glob("diagnostyka-*.txt")) if logs.exists() else []
    if reports:
        files.append(reports[-1])
    settings = root / "dane" / "ustawienia.json"
    if settings.exists():
        files.append(settings)
    return files


def _mac(value: str) -> str:
    return value.strip().upper().replace("-", ":")


def device_address(root: Path) -> str:
    """Adres wybranej bieżni z ustawień - jego nie maskujemy (przydaje się w diagnozie)."""
    try:
        data = json.loads((root / "dane" / "ustawienia.json").read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return ""
    return _mac(str(data.get("device_address") or "")) if isinstance(data, dict) else ""


def scrub(data: bytes, keep_mac: str = "") -> bytes:
    """Bez ścieżki profilu Windows (nazwy użytkownika) i adresów cudzych urządzeń Bluetooth."""
    home = str(Path.home())
    for variant in {home, home.replace("\\", "/"), home.replace("\\", "\\\\")}:
        if len(variant) > 3:
            data = data.replace(variant.encode(), b"%USERPROFILE%")
    keep = _mac(keep_mac)

    def mask(m: re.Match[bytes]) -> bytes:
        return m.group(0) if keep and _mac(m.group(0).decode()) == keep else MASKED_MAC

    return MAC.sub(mask, data)


def build_report(
    description: str, contact: str = "", root: Path = Path(), period_s: float = RECENT_S
) -> Path:
    """Tworzy dane/zgloszenia/zgloszenie-....zip i zwraca jego ścieżkę. Logi bez ścieżki
    profilu i cudzych adresów Bluetooth; za duża paczka - krótsze końcówki logów."""
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
        "okres_logow_h": round(period_s / 3600, 1),
    }
    keep = device_address(root)
    files = report_files(root, period_s)
    logs = [f for f in files if ".log" in f.name]
    since = time.time() - period_s
    for limit in TAIL_LIMITS:
        with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
            zf.writestr("zgloszenie.json", json.dumps(info, ensure_ascii=False, indent=2))
            if logs:
                zf.writestr("podsumowanie.txt", scrub(summary_text(logs, since).encode(), keep))
            for file in files:
                data = tail_since(file, since, limit) if ".log" in file.name else _tail(file, limit)
                zf.writestr(f"{file.parent.name}/{file.name}", scrub(data, keep))
        if path.stat().st_size <= MAX_REPORT_BYTES:
            return path
    path.unlink()
    raise ReportError("Paczka z logami jest za duża - usuń stare logi i spróbuj ponownie.")


def _multipart(fields: dict[str, str], file: Path) -> tuple[bytes, str]:
    boundary = uuid.uuid4().hex
    crlf = "\r\n"
    parts: list[bytes] = []
    for name, value in fields.items():
        head = f'--{boundary}{crlf}Content-Disposition: form-data; name="{name}"{crlf}{crlf}'
        parts.append(f"{head}{value}{crlf}".encode())
    head = (
        f"--{boundary}{crlf}"
        f'Content-Disposition: form-data; name="paczka"; filename="{file.name}"{crlf}'
        f"Content-Type: application/zip{crlf}{crlf}"
    )
    parts.append(head.encode() + file.read_bytes() + crlf.encode())
    parts.append(f"--{boundary}--{crlf}".encode())
    return b"".join(parts), f"multipart/form-data; boundary={boundary}"


def _server_message(exc: urllib.error.HTTPError) -> str:
    try:
        data = json.loads(exc.read().decode("utf-8", "replace"))
    except (OSError, ValueError):
        return ""
    return str(data.get("message") or "") if isinstance(data, dict) else ""


def send_report(
    report: Path,
    endpoint: str,
    description: str,
    contact: str = "",
    period_s: float | None = None,
) -> str:
    """Wysyła paczkę metodą POST (multipart) na serwer zgłoszeń. Zwraca numer zgłoszenia.

    Serwer przekazuje opis i paczkę mailem do autora i ich nie przechowuje. period_s: okres
    logów - idzie osobnym polem „okres” (godziny), bo serwer nie rozpakowuje paczki."""
    fields = {
        "wersja": __version__,
        "opis": description.strip()[:4000],
        "kontakt": contact.strip()[:200],
    }
    if period_s is not None and period_s > 0:
        fields["okres"] = str(max(1, round(period_s / 3600)))
    if urllib.parse.urlparse(endpoint).scheme != "https":
        raise ReportError("Adres zgłoszeń musi zaczynać się od https://")
    body, content_type = _multipart(fields, report)
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
            raw = response.read()
    except ReportError:
        raise
    except urllib.error.HTTPError as exc:
        message = _server_message(exc)
        if exc.code == 413:
            raise ReportError("Paczka jest za duża dla serwera - zapisz ją na dysk.") from exc
        if exc.code == 429:
            raise ReportError(
                message or "Za dużo zgłoszeń naraz - spróbuj za godzinę albo zapisz paczkę."
            ) from exc
        if exc.code == 503:
            raise ReportError(
                (message or "Serwer nie wysłał teraz maila.")
                + " Zapisz paczkę na dysk i wyślij ją e-mailem na "
                + CONTACT
                + "."
            ) from exc
        raise ReportError(message or f"Serwer odrzucił zgłoszenie (kod {exc.code}).") from exc
    except Exception as exc:
        raise ReportError("Nie udało się wysłać - sprawdź internet albo zapisz paczkę.") from exc
    try:
        data = json.loads(raw.decode("utf-8", "replace"))
    except ValueError:
        return ""
    return str(data.get("id") or "") if isinstance(data, dict) else ""


def show_in_folder(report: Path) -> None:
    """Pokazuje paczkę w Eksploratorze (zapis na dysk zamiast wysyłki)."""
    if sys.platform == "win32":
        subprocess.Popen(["explorer", f"/select,{report.resolve()}"])


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
