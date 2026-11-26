"""Fixtury modułu zgłoszeń (reports.rs) z prawdziwej implementacji w Pythonie.

Uruchomienie z katalogu głównego repozytorium:
    set PYTHONPATH=. && .venv\\Scripts\\python.exe walklevel\\tools\\fixtures\\zgloszenia.py

Czasy w logach i granice okresu są zapisane jako lokalny czas "RRRR-MM-DD GG:MM:SS",
a czasy modyfikacji plików jako przesunięcie w sekundach od tej chwili - wynik nie zależy
od strefy czasowej ani od dnia uruchomienia testu. Bajty idą jako base64.
"""

from __future__ import annotations

import asyncio
import base64
import io
import json
import os
import tempfile
import types
import urllib.error
import uuid
import zipfile
from datetime import datetime
from pathlib import Path
from typing import Any

from bieznia import zgloszenia
from bieznia.ble import diagnostics

OUT = Path(__file__).resolve().parents[2] / "crates" / "core" / "tests" / "fixtures" / "zgloszenia"
FMT = "%Y-%m-%d %H:%M:%S"
NOW_LOCAL = "2026-11-20 12:00:00"
NOW = datetime.strptime(NOW_LOCAL, FMT).timestamp()
VERSION = "0.6.0"
zgloszenia.__version__ = VERSION
HOME = r"C:\Users\Jan Kowalski"


def b64(data: bytes) -> str:
    return base64.b64encode(data).decode()


def local(ago_s: float) -> str:
    return datetime.fromtimestamp(NOW - ago_s).strftime(FMT)


def set_home(home: str) -> None:
    Path.home = classmethod(lambda cls: Path(home))  # type: ignore[method-assign]


def write_files(root: Path, files: list[dict[str, Any]]) -> None:
    for f in files:
        path = root / f["name"]
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(base64.b64decode(f["data"]))
        at = NOW - f["ago_s"]
        os.utime(path, (at, at))


def file(name: str, data: bytes | str, ago_s: float = 60) -> dict[str, Any]:
    raw = data.encode("utf-8") if isinstance(data, str) else data
    return {"name": name, "data": b64(raw), "ago_s": ago_s}


# --- scrub ---------------------------------------------------------------------------
def scrub_cases() -> list[dict[str, Any]]:
    inputs = [
        (HOME, "plik C:\\Users\\Jan Kowalski\\AppData\\x.txt", ""),
        (HOME, "plik C:/Users/Jan Kowalski/AppData/x.txt", ""),
        (HOME, '{"p": "C:\\\\Users\\\\Jan Kowalski\\\\dane"}', ""),
        (HOME, "moja AA:BB:CC:DD:EE:FF obca 11-22-33-44-55-66", "aa-bb-cc-dd-ee-ff"),
        (HOME, "moja aa-bb-cc-dd-ee-ff i AA:BB:CC:DD:EE:FF", " AA:BB:CC:DD:EE:FF "),
        (HOME, "AA:BB:CC:DD:EE:FF:00 za długi", ""),
        (HOME, "xAA:BB:CC:DD:EE:FF litera przed", ""),
        (HOME, "1AA:BB:CC:DD:EE:FF cyfra przed", ""),
        (HOME, "AA:BB:CC:DD:EE:FFG litera po", ""),
        (HOME, "-AA:BB:CC:DD:EE:FF myślnik przed", ""),
        (HOME, "AA:BB:CC:DD:EE:FF- myślnik po", ""),
        (HOME, "mieszane aa:bb-cc:dd-ee:ff", ""),
        (HOME, "AA:BB:CC:DD:EE:FF,11:22:33:44:55:66;01:02:03:04:05:06", "11:22:33:44:55:66"),
        (HOME, "AA:BB:CC:DD:EE:FF 11:22:33:44:55:66", ""),
        (HOME, "Bieżnia ąęś 0A:1B:2C:3D:4E:5F żółć", "0a:1b:2c:3d:4e:5f"),
        (HOME, "AA:BB:CC:DD:EE niepełny", ""),
        (HOME, "AABBCCDDEEFF bez separatorów", ""),
        (HOME, "AA::BB:CC:DD:EE:FF podwójny", ""),
        ("C:\\", "krótki C:\\Users i C:\\ zostaje", ""),
        (r"D:\Profile\ola", "D:\\Profile\\ola\\x i D:/Profile/ola/y i D:\\\\Profile\\\\ola", ""),
        (HOME, "", ""),
    ]
    out = []
    for home, text, keep in inputs:
        set_home(home)
        data = text.encode("utf-8")
        out.append(
            {"home": home, "keep": keep, "input": b64(data), "output": b64(zgloszenia.scrub(data, keep))}
        )
    set_home(HOME)
    return out


# --- tail_since ----------------------------------------------------------------------
def tail_cases(tmp: Path) -> list[dict[str, Any]]:
    dated = (
        f"{local(3 * 86400)} INFO  stare\n"
        f"{local(5 * 3600)} INFO  pięć godzin temu\n"
        f"{local(1800)} ERROR świeże\nTraceback: dalsza linia\n"
        f"{local(60)} INFO  koniec"
    )
    cases = [
        ("bieznia.log", dated, 120, local(3600), zgloszenia.MAX_LOG_BYTES),
        ("bieznia.log", dated, 120, local(7 * 86400), zgloszenia.MAX_LOG_BYTES),
        ("bieznia.log", dated, 120, local(10), zgloszenia.MAX_LOG_BYTES),
        ("bieznia.log", dated, 120, local(3600), 40),
        ("bieznia.log", dated.replace("\n", "\r\n"), 120, local(3600), zgloszenia.MAX_LOG_BYTES),
        ("stary.log", "bez dat\ndruga linia\n", 120, local(3600), zgloszenia.MAX_LOG_BYTES),
        ("stary.log", "bez dat\ndruga linia\n", 7200, local(3600), zgloszenia.MAX_LOG_BYTES),
        ("zle.log", f"2026-13-45 99:99:99 zła data\n{local(30)} dobra\n", 60, local(3600), 10_000),
        ("zle.log", "2026-13-45 99:99:99 tylko zła data\n", 60, local(3600), 10_000),
        ("cut.log", "x" * 50 + f"\n{local(30)} ok\n", 60, local(3600), 30),
        ("pusty.log", "", 60, local(3600), 10_000),
        ("bin.log", b"\xff\xfe" + f"{local(30)} \xc5\x82 ok\n".encode("latin-1"), 60, local(3600), 10_000),
    ]
    out = []
    for i, (name, data, ago, since_local, limit) in enumerate(cases):
        folder = tmp / f"tail{i}"
        folder.mkdir()
        spec = file(name, data, ago)
        write_files(folder, [spec])
        since = datetime.strptime(since_local, FMT).timestamp()
        result = zgloszenia.tail_since(folder / name, since, limit)
        out.append({"file": spec, "since": since_local, "limit": limit, "output": b64(result)})
    return out


# --- summary_text --------------------------------------------------------------------
def summary_cases(tmp: Path) -> list[dict[str, Any]]:
    app_log = (
        f"{local(4000)} INFO    bieznia.ble: Start: przyjęty po 1.6 s (próba 1)\n"
        f"{local(3500)} INFO    bieznia: Okno gotowe po 0.4 s\n"
        f"{local(3000)} WARNING bieznia.ble: Taśma nie przyjęła 2.5 km/h (jedzie 1.0) - ponawiam (1)\n"
        f"{local(2900)} INFO    bieznia.ble: Prędkość 2.5: przyjęta po 0.8 s\n"
        f"{local(2800)} INFO    bieznia.ble: Bieżnia nie zareagowała na Stop:\r\n"
        f"{local(2700)} INFO    bieznia.ble: Połączenie z AA:BB:CC:DD:EE:FF\x0cTelemetria ok\n"
        f"{local(2600)} ERROR   bieznia: coś poszło nie tak\n"
        "Traceback (most recent call last): ERROR bez spacji\n"
        f"{local(2500)} CRITICAL bieznia: koniec\n"
        f"{local(2400)} INFO    bieznia.ble: Bieżnia 2 rozłączyła się\n"
        f"{local(2300)} INFO    bieznia: WalkLevel 0.5.2 (exe)\u2028Zaktualizowano\n"
        f"{local(2200)} INFO    bieznia: Bieżnia Ⅻ\n"
        f"{local(2100)} INFO    bieznia: Prędkość ٣.٥: arabska cyfra\n"
    )
    older = f"{local(90000)} INFO bieznia: Start: przyjęty dawno\n{local(5000)} INFO Stop: ok\n"
    many = "".join(f"{local(3000 - i)} ERROR linia {i}\n" for i in range(2100))
    sets = [
        (
            [
                file("bieznia.log", app_log, 60),
                file("bieznia.log.1", older, 4000),
                file("bluetooth.log", f"{local(100)} ERROR bluetooth\n", 30),
            ],
            local(6 * 3600),
        ),
        ([file("bieznia.log", app_log, 60), file("bieznia.log.1", older, 4000)], local(100000)),
        ([file("bieznia.log", many, 60)], local(6 * 3600)),
        ([file("bluetooth.log", "Start: x\n", 60)], local(6 * 3600)),
        ([file("bieznia.log", b"\xff Start: z\xc5 b\xc5\x82\xc4dem\n", 60)], local(6 * 3600)),
    ]
    out = []
    for i, (files, since_local) in enumerate(sets):
        folder = tmp / f"summary{i}"
        folder.mkdir()
        write_files(folder, files)
        since = datetime.strptime(since_local, FMT).timestamp()
        paths = [folder / f["name"] for f in files]
        out.append(
            {"files": files, "since": since_local, "output": zgloszenia.summary_text(paths, since)}
        )
    return out


# --- recent_logs ---------------------------------------------------------------------
def recent_cases(tmp: Path) -> list[dict[str, Any]]:
    files = [
        file("bieznia.log", "a", 60),
        file("bluetooth.log", "b", 10 * 86400),
        file("bieznia.log.1", "c", 3600),
        file("bieznia.log.2", "d", 8 * 3600),
        file("bluetooth.log.1", "e", 100),
        file("bluetooth.log.10", "f", 100),
        file("a.log.x", "g", 100),
        file("Zeta.LOG", "h", 100),
        file("alfa.Log.3", "i", 100),
        file("diagnostyka-20260101-100000.txt", "j", 100),
        file("notatki.txt", "k", 100),
        file(".log", "l", 100),
    ]
    folder = tmp / "recent" / "logs"
    folder.mkdir(parents=True)
    write_files(folder, files)
    out = []
    for period in (3600, 6 * 3600, 24 * 3600):
        found = zgloszenia.recent_logs(folder, now=NOW, period_s=period)
        out.append({"period_s": period, "output": [p.name for p in found]})
    return [{"files": files, "cases": out}]


# --- build_report --------------------------------------------------------------------
def report_cases(tmp: Path) -> list[dict[str, Any]]:
    app_log = (
        f"{local(3 * 86400)} INFO  stare C:\\Users\\Jan Kowalski\\x\n"
        f"{local(1800)} INFO    bieznia.ble: Start: przyjęty po 1.6 s, bieżnia AA:BB:CC:DD:EE:FF\n"
        f"{local(1700)} ERROR   bieznia: obca 11:22:33:44:55:66 w C:/Users/Jan Kowalski/AppData\n"
        "Traceback: dalsza linia\n"
    )
    trees = [
        (
            [
                file("logs/bieznia.log", app_log, 60),
                file("logs/bluetooth.log", f"{local(100)} DEBUG << 11-22-33-44-55-66\n", 30),
                file("logs/bluetooth.log.1", f"{local(5000)} DEBUG stary\n", 5000),
                file("logs/bluetooth.log.2", f"{local(9 * 86400)} DEBUG bardzo stary\n", 9 * 86400),
                file("logs/diagnostyka-20260101-100000.txt", "stary raport", 100),
                file("logs/diagnostyka-20260102-100000.txt", "nowy AA:BB:CC:DD:EE:FF 01:02:03:04:05:06", 100),
                file("dane/ustawienia.json", '{"device_address": "aa-bb-cc-dd-ee-ff", "run_on": 6.3}', 100),
                file("dane/osoby.json", "{}", 100),
                file("prywatny.txt", "nie wysyłać", 100),
            ],
            "  Bieżnia się rozłącza  ",
            " nick#1 ",
            6 * 3600,
        ),
        (
            [
                file("logs/bieznia.log", app_log, 60),
                file("dane/ustawienia.json", "nie json", 100),
            ],
            "x",
            "",
            3600,
        ),
        (
            [file("logs/bieznia.log", app_log, 60), file("logs/bieznia.log.1", "bez dat\n", 100)],
            "tydzień",
            "",
            7 * 86400,
        ),
        ([file("logs/bluetooth.log", "bez dat\n", 60)], "tylko bluetooth", "", 5580),
        ([], "pusto", "", 6 * 3600),
    ]
    real_time = zgloszenia.time.time
    zgloszenia.time.time = lambda: NOW  # type: ignore[assignment]
    out = []
    try:
        for i, (files, description, contact, period) in enumerate(trees):
            root = tmp / f"report{i}"
            root.mkdir()
            write_files(root, files)
            path = zgloszenia.build_report(description, contact, root, period)
            with zipfile.ZipFile(path) as zf:
                entries = {name: b64(zf.read(name)) for name in zf.namelist()}
                info = json.loads(zf.read("zgloszenie.json"))
            out.append(
                {
                    "files": files,
                    "description": description,
                    "contact": contact,
                    "period_s": period,
                    "entries": entries,
                    "info": {k: info[k] for k in ("opis", "kontakt", "wersja", "okres_logow_h")},
                    "info_keys": list(info),
                }
            )
    finally:
        zgloszenia.time.time = real_time  # type: ignore[assignment]
    return out


# --- wysyłka: multipart, pola, błędy -----------------------------------------------
class FakeResponse:
    def __init__(self, body: bytes, status: int = 200) -> None:
        self.body, self.status = body, status

    def read(self) -> bytes:
        return self.body

    def __enter__(self) -> FakeResponse:
        return self

    def __exit__(self, *_a: object) -> None:
        return None


def send_cases(tmp: Path) -> dict[str, Any]:
    report = tmp / "zgloszenie-1.zip"
    report.write_bytes(b"PK\x03\x04zawartosc\xff")
    fixed = uuid.UUID("0123456789abcdef0123456789abcdef")
    real_uuid4 = zgloszenia.uuid.uuid4
    zgloszenia.uuid.uuid4 = lambda: fixed  # type: ignore[assignment]
    real_urlopen = zgloszenia.urllib.request.urlopen
    try:
        body, content_type = zgloszenia._multipart(
            {"wersja": VERSION, "opis": "błąd \"cudzysłów\"\r\nlinia", "kontakt": ""}, report
        )
        multipart = {"body": b64(body), "content_type": content_type}

        sent: list[Any] = []

        def ok(reply: bytes) -> None:
            def urlopen(request: Any, timeout: float) -> FakeResponse:
                sent.append(request)
                return FakeResponse(reply)

            zgloszenia.urllib.request.urlopen = urlopen

        fields = []
        long_text = "ą" * 4100 + "  "
        for description, contact, period in [
            ("  opis  ", " kontakt ", None),
            ("opis", "", 3600),
            ("opis", "", 5400),
            ("opis", "", 9000),
            ("opis", "", 600),
            ("opis", "", 0),
            ("opis", "", -5),
            ("opis", "", 7 * 86400),
            (long_text, "k" * 300, 6 * 3600),
        ]:
            ok(b'{"id": "42"}')
            zgloszenia.send_report(report, "https://example.com/z", description, contact, period)
            fields.append(
                {
                    "description": description,
                    "contact": contact,
                    "period_s": period,
                    "body": b64(sent[-1].data),
                    "user_agent": sent[-1].get_header("User-agent"),
                }
            )

        replies = []
        for raw in [b'{"id": "42"}', b'{"id": 42}', b'{"id": null}', b"[]", b"nie json", b"{}",
                    b'{"id": true}', b'{"id": 4.0}']:
            ok(raw)
            replies.append(
                {"body": b64(raw), "id": zgloszenia.send_report(report, "https://example.com/z", "x")}
            )

        errors = []
        for code, raw in [
            (400, b'{"message": "Brak opisu."}'),
            (400, b"<html>bad</html>"),
            (413, b'{"message": "ignorowane"}'),
            (429, "{\"message\": \"Limit: 5 na godzinę.\"}".encode()),
            (429, b"{}"),
            (503, "{\"message\": \"Poczta leży.\"}".encode()),
            (503, b"{}"),
            (500, b"[1, 2]"),
            (404, b'{"message": null}'),
            (401, b'{"message": 7}'),
        ]:

            def fail(request: Any, timeout: float, code: int = code, raw: bytes = raw) -> FakeResponse:
                raise urllib.error.HTTPError("https://example.com/z", code, "x", {}, io.BytesIO(raw))  # type: ignore[arg-type]

            zgloszenia.urllib.request.urlopen = fail
            try:
                zgloszenia.send_report(report, "https://example.com/z", "x")
                message = ""
            except zgloszenia.ReportError as exc:
                message = str(exc)
            errors.append({"status": code, "body": b64(raw), "message": message})

        def broken(request: Any, timeout: float) -> FakeResponse:
            raise OSError("brak sieci")

        zgloszenia.urllib.request.urlopen = broken
        try:
            zgloszenia.send_report(report, "https://example.com/z", "x")
        except zgloszenia.ReportError as exc:
            network = str(exc)
        try:
            zgloszenia.send_report(report, "http://example.com/z", "x")
        except zgloszenia.ReportError as exc:
            not_https = str(exc)
    finally:
        zgloszenia.uuid.uuid4 = real_uuid4  # type: ignore[assignment]
        zgloszenia.urllib.request.urlopen = real_urlopen
    return {
        "report_name": report.name,
        "report": b64(report.read_bytes()),
        "boundary": fixed.hex,
        "multipart": multipart,
        "fields": fields,
        "replies": replies,
        "errors": errors,
        "network": network,
        "not_https": not_https,
    }


def mailto_cases() -> list[dict[str, str]]:
    out = []
    for description in ["Nie działa & stop", "  zażółć gęślą jaźń\nnowa linia?=#+%  ", "a~b_c.d-e*f'g(h)"]:
        out.append(
            {
                "description": description,
                "report": "zgloszenie-20261120-120000.zip",
                "output": zgloszenia.mailto_link(description, Path("x") / "zgloszenie-20261120-120000.zip"),
            }
        )
    return out


# --- raport diagnostyczny (bleak podmieniony) --------------------------------------
class Dev:
    def __init__(self, address: str, name: str | None) -> None:
        self.address, self.name = address, name


class Adv:
    def __init__(self, local_name: str | None, rssi: int, uuids: list[str], mfr: dict[int, bytes]) -> None:
        self.local_name, self.rssi, self.service_uuids, self.manufacturer_data = local_name, rssi, uuids, mfr


class Char:
    def __init__(self, uuid: str, properties: list[str], description: str) -> None:
        self.uuid, self.properties, self.description = uuid, properties, description


class Service:
    def __init__(self, uuid: str, description: str, characteristics: list[Char]) -> None:
        self.uuid, self.description, self.characteristics = uuid, description, characteristics


def diagnostics_cases(tmp: Path) -> list[dict[str, Any]]:
    scenario: dict[str, Any] = {}

    class Scanner:
        @staticmethod
        async def discover(timeout: float, return_adv: bool) -> dict[str, Any]:
            if scenario.get("boom"):
                raise RuntimeError("boom")
            return {d["address"]: (Dev(d["address"], d["name"]), Adv(d["local_name"], d["rssi"], d["uuids"],
                    {int(k): bytes.fromhex(v) for k, v in d["mfr"]})) for d in scenario["devices"]}

        @staticmethod
        async def find_device_by_address(address: str, timeout: float) -> Dev | None:
            if scenario.get("gone"):
                return None
            return Dev(address, "KS-ST-A1P")

    class Client:
        def __init__(self, device: Any, timeout: float, winrt: Any) -> None:
            self.services = [
                Service(s["uuid"], s["description"], [Char(*c) for c in s["chars"]]) for s in scenario["services"]
            ]

        async def __aenter__(self) -> Any:
            return self

        async def __aexit__(self, *_a: object) -> None:
            return None

        async def read_gatt_char(self, char: Char) -> bytes:
            value = scenario["reads"].get(char.uuid)
            if value is None:
                raise TimeoutError("cisza")
            return bytes.fromhex(value)

        async def start_notify(self, char: Char, callback: Any) -> None:
            data = scenario["notify"].get(char.uuid)
            if data is None:
                raise OSError("odmowa")
            callback(char, bytearray.fromhex(data))

    async def no_sleep(_s: float) -> None:
        return None

    import sys

    sys.modules["bleak"] = types.SimpleNamespace(BleakScanner=Scanner, BleakClient=Client)  # type: ignore[assignment]
    diagnostics.asyncio = types.SimpleNamespace(sleep=no_sleep)  # type: ignore[attr-defined]
    devices = [
        {"address": "11:22:33:44:55:66", "name": "Telefon", "local_name": None, "rssi": -80, "uuids": [], "mfr": []},
        {
            "address": "AA:BB:CC:DD:EE:FF",
            "name": "KS-ST-A1P",
            "local_name": "KS-ST-A1P",
            "rssi": -45,
            "uuids": ["00001826-0000-1000-8000-00805f9b34fb"],
            "mfr": [["2117", "0102ab"], ["76", ""]],
        },
        {"address": "01:02:03:04:05:06", "name": None, "local_name": None, "rssi": -101, "uuids": [], "mfr": []},
        {"address": "0A:0B:0C:0D:0E:0F", "name": "Głośnik", "local_name": "", "rssi": -45, "uuids": [], "mfr": []},
    ]
    services = [
        {
            "uuid": "0000180a-0000-1000-8000-00805f9b34fb",
            "description": "Device Information",
            "chars": [
                ["00002a26-0000-1000-8000-00805f9b34fb", ["read"], "Firmware Revision String"],
                ["00002a29-0000-1000-8000-00805f9b34fb", ["read"], "Manufacturer Name String"],
                ["00002a24-0000-1000-8000-00805f9b34fb", ["read"], "Model Number String"],
                ["00002a25-0000-1000-8000-00805f9b34fb", ["read"], "Serial Number String"],
                ["00002a27-0000-1000-8000-00805f9b34fb", ["read"], "Hardware Revision String"],
                ["00002a28-0000-1000-8000-00805f9b34fb", ["read"], "Software Revision String"],
            ],
        },
        {
            "uuid": "00001826-0000-1000-8000-00805f9b34fb",
            "description": "Fitness Machine",
            "chars": [
                ["00002ad4-0000-1000-8000-00805f9b34fb", ["read"], "Supported Speed Range"],
                ["00002acd-0000-1000-8000-00805f9b34fb", ["notify"], "Treadmill Data"],
                ["00002ad9-0000-1000-8000-00805f9b34fb", ["write", "indicate"], "Fitness Machine Control Point"],
                ["00002ada-0000-1000-8000-00805f9b34fb", ["notify"], "Fitness Machine Status"],
            ],
        },
    ]
    reads = {
        "00002a26-0000-1000-8000-00805f9b34fb": b" 1.2.3\n".hex(),
        "00002a29-0000-1000-8000-00805f9b34fb": "4b696e67536d697468",
        "00002a24-0000-1000-8000-00805f9b34fb": "c5bc",
        "00002a25-0000-1000-8000-00805f9b34fb": "2020",
        "00002a27-0000-1000-8000-00805f9b34fb": "41094201",
        "00002ad4-0000-1000-8000-00805f9b34fb": "6400b0040a00",
    }
    notify = {
        "00002acd-0000-1000-8000-00805f9b34fb": "0400a000",
        "00002ad9-0000-1000-8000-00805f9b34fb": "",
    }
    scenarios = [
        {"name": "pelny", "devices": devices, "services": services, "reads": reads, "notify": notify,
         "address": None},
        {"name": "z_adresem", "devices": devices[:1], "services": services[:1], "reads": {}, "notify": {},
         "address": "AA:BB:CC:DD:EE:FF"},
        {"name": "brak_biezni", "devices": [devices[0], devices[2]], "services": [], "reads": {}, "notify": {},
         "address": None},
        {"name": "zniknela", "devices": devices, "services": [], "reads": {}, "notify": {}, "address": None,
         "gone": True},
        {"name": "blad", "devices": [], "services": [], "reads": {}, "notify": {}, "address": None, "boom": True},
    ]
    out = []
    for i, sc in enumerate(scenarios):
        scenario.clear()
        scenario.update(sc)
        path = tmp / f"diag{i}" / "diagnostyka.txt"
        asyncio.run(diagnostics.run_diagnostics(path, sc["address"]))
        lines = path.read_text(encoding="utf-8").split("\n")
        out.append({**sc, "lines": lines[2:]})  # bez nagłówka z datą i wersjami
    return out


def main() -> None:
    set_home(HOME)
    OUT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp_dir:
        tmp = Path(tmp_dir)
        data = {
            "now": NOW_LOCAL,
            "version": VERSION,
            "home": HOME,
            "constants": {
                "max_log_bytes": zgloszenia.MAX_LOG_BYTES,
                "recent_s": zgloszenia.RECENT_S,
                "max_report_bytes": zgloszenia.MAX_REPORT_BYTES,
                "tail_limits": list(zgloszenia.TAIL_LIMITS),
                "periods": [list(p) for p in zgloszenia.PERIODS],
            },
            "scrub": scrub_cases(),
            "tail_since": tail_cases(tmp),
            "summary": summary_cases(tmp),
            "recent_logs": recent_cases(tmp),
            "reports": report_cases(tmp),
            "send": send_cases(tmp),
            "mailto": mailto_cases(),
            "diagnostics": diagnostics_cases(tmp),
        }
    (OUT / "zgloszenia.json").write_text(json.dumps(data, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"Zapisano {OUT / 'zgloszenia.json'}")


if __name__ == "__main__":
    main()
