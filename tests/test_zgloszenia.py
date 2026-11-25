"""Zgłoś problem: zawartość paczki, limity i bezpieczny adres wysyłki."""

from __future__ import annotations

import json
import zipfile
from pathlib import Path

import pytest

from bieznia import zgloszenia


def make_data(root: Path) -> None:
    logs = root / "logs"
    logs.mkdir()
    (logs / "bieznia.log").write_text("start\n" + "x" * 50 + "\nkoniec\n", encoding="utf-8")
    (logs / "bluetooth.log").write_bytes(b"A" * (zgloszenia.MAX_LOG_BYTES + 500) + b"OSTATNIE")
    (logs / "diagnostyka-20260101-100000.txt").write_text("stary", encoding="utf-8")
    (logs / "diagnostyka-20260102-100000.txt").write_text("nowy", encoding="utf-8")
    (root / "dane").mkdir()
    (root / "dane" / "ustawienia.json").write_text('{"run_on": 6.3}', encoding="utf-8")
    (root / "prywatny.txt").write_text("nie wysyłać", encoding="utf-8")


def test_report_contains_only_logs_and_settings(tmp_path: Path) -> None:
    make_data(tmp_path)
    path = zgloszenia.build_report("Bieżnia się rozłącza", "nick#1", tmp_path)
    with zipfile.ZipFile(path) as zf:
        names = sorted(zf.namelist())
        info = json.loads(zf.read("zgloszenie.json"))
        tail = zf.read("logs/bluetooth.log")
    assert names == [
        "dane/ustawienia.json",
        "logs/bieznia.log",
        "logs/bluetooth.log",
        "logs/diagnostyka-20260102-100000.txt",  # tylko najnowszy raport
        "podsumowanie.txt",
        "zgloszenie.json",
    ]
    assert info["opis"] == "Bieżnia się rozłącza" and info["kontakt"] == "nick#1"
    assert len(tail) == zgloszenia.MAX_LOG_BYTES and tail.endswith(b"OSTATNIE")


def test_report_works_without_any_logs(tmp_path: Path) -> None:
    path = zgloszenia.build_report("pusto", root=tmp_path)
    with zipfile.ZipFile(path) as zf:
        assert zf.namelist() == ["zgloszenie.json"]


def test_only_https_endpoint(tmp_path: Path) -> None:
    path = zgloszenia.build_report("x", root=tmp_path)
    with pytest.raises(zgloszenia.ReportError, match="https"):
        zgloszenia.send_report(path, "http://example.com/zgloszenia", "x")


def test_multipart_body(tmp_path: Path) -> None:
    file = tmp_path / "a.zip"
    file.write_bytes(b"PK123")
    body, content_type = zgloszenia._multipart({"opis": "błąd"}, file)
    boundary = content_type.split("boundary=")[1]
    assert body.startswith(f"--{boundary}\r\n".encode())
    assert 'name="opis"\r\n\r\nbłąd\r\n'.encode() in body
    assert b'filename="a.zip"' in body and b"PK123" in body
    assert body.endswith(f"--{boundary}--\r\n".encode())


def test_mailto_link_is_encoded(tmp_path: Path) -> None:
    link = zgloszenia.mailto_link("Nie działa & stop", tmp_path / "zgloszenie-1.zip")
    assert link.startswith("mailto:kontakt@damnit.com.pl?subject=")
    assert "&" not in link.split("body=")[1]  # znak & z opisu zakodowany
    assert "zgloszenie-1.zip" in link


def test_report_adds_recent_rotated_logs_and_summary(tmp_path: Path) -> None:
    import os
    import time

    logs = tmp_path / "logs"
    logs.mkdir()
    (logs / "bieznia.log").write_text(
        "10:00:01 INFO    bieznia.ble: Start: przyjęty po 1.6 s (próba 1)\n"
        "10:00:02 INFO    bieznia: Okno gotowe po 0.4 s\n"
        "10:05:00 WARNING bieznia.ble: Taśma nie przyjęła 2.5 km/h (jedzie 1.0) - ponawiam (1)\n"
        "10:30:00 INFO    bieznia.ble: Podsumowanie sesji: 30 min\n",
        encoding="utf-8",
    )
    (logs / "bluetooth.log").write_text("nowy\n", encoding="utf-8")
    fresh = logs / "bluetooth.log.1"
    fresh.write_text("sprzed godziny\n", encoding="utf-8")
    stale = logs / "bluetooth.log.2"
    stale.write_text("sprzed tygodnia\n", encoding="utf-8")
    week_ago = time.time() - 7 * 86400
    os.utime(stale, (week_ago, week_ago))
    path = zgloszenia.build_report("test", root=tmp_path)
    with zipfile.ZipFile(path) as zf:
        names = set(zf.namelist())
        summary = zf.read("podsumowanie.txt").decode("utf-8")
    assert "logs/bluetooth.log.1" in names
    assert "logs/bluetooth.log.2" not in names  # starszy niż 6 godzin
    assert "Start: przyjęty" in summary and "Podsumowanie sesji" in summary
    assert "Okno gotowe" not in summary  # tylko ważne zdarzenia
    assert "polecenia przyjęte: 1" in summary and "bez reakcji: 1" in summary


def test_report_hides_profile_path_and_other_bluetooth_devices(tmp_path: Path) -> None:
    home = str(Path.home())
    logs = tmp_path / "logs"
    logs.mkdir()
    (logs / "bieznia.log").write_text(
        f"plik {home}/AppData/x.txt\nmoja AA:BB:CC:DD:EE:FF obca 11-22-33-44-55-66\n",
        encoding="utf-8",
    )
    (tmp_path / "dane").mkdir()
    (tmp_path / "dane" / "ustawienia.json").write_text(
        '{"device_address": "aa:bb:cc:dd:ee:ff"}', encoding="utf-8"
    )
    path = zgloszenia.build_report("x", root=tmp_path)
    with zipfile.ZipFile(path) as zf:
        text = zf.read("logs/bieznia.log").decode()
    assert home not in text and "%USERPROFILE%/AppData" in text
    assert "AA:BB:CC:DD:EE:FF" in text  # wybrana bieżnia zostaje
    assert "11-22-33-44-55-66" not in text and "XX:XX:XX:XX:XX:XX" in text


def test_too_big_report_uses_shorter_log_tails(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    import os

    logs = tmp_path / "logs"
    logs.mkdir()
    (logs / "bieznia.log").write_bytes(os.urandom(zgloszenia.MAX_LOG_BYTES))  # nie kompresuje się
    monkeypatch.setattr(zgloszenia, "MAX_REPORT_BYTES", 500_000)
    path = zgloszenia.build_report("x", root=tmp_path)
    assert path.stat().st_size <= 500_000


class FakeResponse:
    def __init__(self, body: bytes, status: int = 200) -> None:
        self.body, self.status = body, status

    def read(self) -> bytes:
        return self.body

    def __enter__(self) -> FakeResponse:
        return self

    def __exit__(self, *_a: object) -> None:
        return None


def test_send_report_returns_number_and_maps_errors(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    import io
    import urllib.error

    path = zgloszenia.build_report("x", root=tmp_path)
    sent: list[object] = []

    def ok(request: object, timeout: float) -> FakeResponse:
        sent.append(request)
        return FakeResponse(b'{"id": "42"}')

    monkeypatch.setattr(zgloszenia.urllib.request, "urlopen", ok)
    assert zgloszenia.send_report(path, "https://example.com/z", "opis") == "42"
    request = sent[0]
    assert request.get_header("User-agent").startswith("Bieznia/")  # type: ignore[attr-defined]
    assert b'name="paczka"' in request.data  # type: ignore[attr-defined]
    assert b'name="okres"' not in request.data  # type: ignore[attr-defined]
    zgloszenia.send_report(path, "https://example.com/z", "opis", period_s=6 * 3600)
    body = sent[-1].data  # type: ignore[attr-defined]
    crlf = bytes([13, 10])
    assert b'name="okres"' + crlf + crlf + b"6" + crlf in body  # okres w godzinach

    def fail(code: int, body: bytes) -> None:
        def raise_(request: object, timeout: float) -> FakeResponse:
            raise urllib.error.HTTPError("https://example.com/z", code, "x", {}, io.BytesIO(body))  # type: ignore[arg-type]

        monkeypatch.setattr(zgloszenia.urllib.request, "urlopen", raise_)

    fail(400, b'{"message": "Brak opisu."}')
    with pytest.raises(zgloszenia.ReportError, match="Brak opisu"):
        zgloszenia.send_report(path, "https://example.com/z", "opis")
    fail(413, b"<html>too large</html>")
    with pytest.raises(zgloszenia.ReportError, match="za duża"):
        zgloszenia.send_report(path, "https://example.com/z", "opis")
    fail(503, b"{}")
    with pytest.raises(zgloszenia.ReportError, match="Zapisz paczkę"):
        zgloszenia.send_report(path, "https://example.com/z", "opis")


def test_report_takes_only_the_chosen_period(tmp_path: Path) -> None:
    import time
    from datetime import datetime

    logs = tmp_path / "logs"
    logs.mkdir()
    now = time.time()

    def stamp(ago_s: float) -> str:
        return datetime.fromtimestamp(now - ago_s).strftime("%Y-%m-%d %H:%M:%S")

    (logs / "bieznia.log").write_text(
        f"{stamp(3 * 86400)} INFO  stare\n"
        f"{stamp(5 * 3600)} INFO  piec godzin temu\n"
        f"{stamp(1800)} ERROR swieze\nTraceback: dalsza linia\n",
        encoding="utf-8",
    )
    one_hour = zgloszenia.build_report("x", root=tmp_path, period_s=3600)
    with zipfile.ZipFile(one_hour) as zf:
        text = zf.read("logs/bieznia.log").decode()
        info = json.loads(zf.read("zgloszenie.json"))
    assert "swieze" in text and "Traceback" in text
    assert "piec godzin" not in text and "stare" not in text
    assert info["okres_logow_h"] == 1.0
    week = zgloszenia.build_report("x", root=tmp_path, period_s=7 * 86400)
    with zipfile.ZipFile(week) as zf:
        assert "stare" in zf.read("logs/bieznia.log").decode()
