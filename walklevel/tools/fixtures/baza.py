"""Fixtury modułu baza (paths/config/logs) z prawdziwej implementacji w Pythonie.

Uruchomienie z katalogu głównego repozytorium:
    PYTHONPATH=. .venv/Scripts/python.exe walklevel/tools/fixtures/baza.py
Wyniki: walklevel/crates/core/tests/fixtures/baza/*.json
"""

from __future__ import annotations

import copy
import json
import logging
import tempfile
import time
from logging.handlers import RotatingFileHandler
from pathlib import Path

from bieznia import logs, settings
from bieznia.config import load_config

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / "walklevel" / "crates" / "core" / "tests" / "fixtures" / "baza"


def save(name: str, data: object) -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    text = json.dumps(data, indent=2, ensure_ascii=False) + "\n"
    (OUT / name).write_text(text, encoding="utf-8", newline="\n")


def config_cases() -> None:
    cases = [
        ('[output]\ntype = "dry"\nstart_enabled = false\n', '[output]\ntype = "keys"\n'),
        (
            '[a]\nx = 1\n[a.b]\ny = 2\nz = [1, 2]\n[c]\nk = "v"\n',
            '[a.b]\ny = 3\nnowy = true\n[a]\nx = { w = 1 }\n[d]\nq = 1.5\n',
        ),
        ('[a]\nx = { w = 1 }\n', '[a]\nx = 5\n'),
        ('[konto]\nadres = "https://x"\n', None),
        ('top = 1\n[s]\nlista = [[0, 1.0], [25, 1.5]]\n', 'top = "z"\n[s]\nlista = []\n'),
    ]
    out = []
    for base, local in cases:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "config.toml").write_text(base, encoding="utf-8")
            if local is not None:
                (root / "config.local.toml").write_text(local, encoding="utf-8")
            out.append({"base": base, "local": local, "result": load_config(root)})
    save("config_merge.json", out)
    save("config_repo.json", load_config(ROOT))


def settings_cases() -> None:
    base = {"mapping": {"run_on": 6.3}, "output": {"type": "dry"}}
    cases = [
        {"run_on": 7.0, "output": "keys", "device_name": "KS-X21"},
        {"theme": "dark", "detailed_logs": False, "pad_steps": [[0, 1.0], [100, 4.0]]},
        {"walk_key": "w", "toggle_pad": "back", "floating_pos": [10, 20], "last_version": "0.5.1"},
    ]
    out = []
    for user in cases:
        out.append(
            {
                "config": base,
                "settings": user,
                "result": settings.apply_settings(copy.deepcopy(base), user),
            }
        )
    save("apply_settings.json", {"sections": settings.SECTIONS, "cases": out})


RECORDS = [
    (1_700_000_000_123_456_789, logging.INFO, "bieznia", "Start aplikacji"),
    (1_700_000_001_999_999_999, logging.WARNING, "bieznia.ble", "Zażółć gęślą jaźń"),
    (1_700_000_002_000_500_000, logging.ERROR, "walklevel_core::history", "błąd: %s"),
    (1_700_000_003_050_000_000, logging.CRITICAL, "bieznia.blad", "Nieobsłużony błąd\nlinia 2"),
    (1_700_000_004_007_000_000, logging.DEBUG, "bieznia.ble", "-> 0x02 0x53 (KingSmith)"),
]


def record(ns: int, level: int, name: str, msg: str) -> logging.LogRecord:
    rec = logging.LogRecord(name, level, __file__, 1, msg, None, None)
    rec.created = ns / 1e9
    rec.msecs = (ns % 1_000_000_000) // 1_000_000 + 0.0
    if rec.msecs == 999.0 and int(rec.created) != ns // 1_000_000_000:
        rec.msecs = 0.0
    return rec


def line_cases() -> None:
    main = logging.Formatter(logs.FORMAT, logs.FILE_DATEFMT)
    ble = logging.Formatter("%(asctime)s.%(msecs)03d %(message)s", logs.FILE_DATEFMT)
    console = logging.Formatter(logs.FORMAT, "%H:%M:%S")
    for f in (main, ble, console):
        f.converter = time.gmtime  # fixtury niezależne od strefy czasowej
    out = []
    for ns, level, name, msg in RECORDS:
        rec = record(ns, level, name, msg)
        out.append(
            {
                "ns": ns,
                "level": logging.getLevelName(level),
                "name": name,
                "msg": msg,
                "main": main.format(rec),
                "ble": ble.format(rec),
                "console": console.format(rec),
            }
        )
    save("log_lines.json", out)


MESSAGES = [
    "x" * 150,  # pierwszy wpis w pustym pliku: nigdy nie obraca, nawet gdy za długi
    "krótki",
    "Zażółć gęślą jaźń - wielobajtowe znaki",
    "linia 1\nlinia 2\nlinia 3",
    "a" * 40,
    "b" * 41,
    "c" * 42,
    "ąęśćżźółń" * 5,
    "d" * 10,
    "e" * 60,
    "f" * 60,
    "g" * 60,
    "h" * 60,
    "i" * 5,
    "j" * 118,
    "k" * 119,
    "l" * 120,
    "m",
    "n" * 30,
    "o" * 80,
]


def rotation_cases() -> None:
    out = []
    for max_bytes in (120, 200):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "test.log"
            handler = RotatingFileHandler(path, maxBytes=max_bytes, backupCount=3, encoding="utf-8")
            handler.setFormatter(logging.Formatter("%(message)s"))
            for msg in MESSAGES:
                handler.emit(logging.LogRecord("t", logging.INFO, __file__, 1, msg, None, None))
            handler.close()
            files = {}
            for name in ("test.log", "test.log.1", "test.log.2", "test.log.3", "test.log.4"):
                p = Path(tmp) / name
                if p.exists():
                    files[name] = p.read_bytes().decode("utf-8")
            out.append({"max_bytes": max_bytes, "backup_count": 3, "files": files})
    save("rotation.json", {"messages": MESSAGES, "cases": out})


if __name__ == "__main__":
    config_cases()
    settings_cases()
    line_cases()
    rotation_cases()
    print(f"Zapisano fixtury w {OUT}")
