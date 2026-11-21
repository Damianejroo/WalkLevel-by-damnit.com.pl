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
