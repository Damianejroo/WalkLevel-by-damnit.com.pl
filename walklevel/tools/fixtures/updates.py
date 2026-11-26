"""Fixtury modułu updates (Rust) z prawdziwej implementacji bieznia/aktualizacje.py.

Uruchom z katalogu głównego repozytorium:
    set PYTHONPATH=.
    .venv\\Scripts\\python.exe walklevel\\tools\\fixtures\\updates.py
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

from bieznia import aktualizacje as akt

OUT = Path("walklevel/crates/core/tests/fixtures/updates")

SHA = hashlib.sha256(b"MZ" + b"instalator" * 1000).hexdigest()
URL = "https://github.com/Damianejroo/WalkingPad-X21-PC-Gaming-by-damnit.com.pl/releases/download/v0.3.0/Bieznia-Setup-0.3.0.exe"

VERSIONS = [
    "v1.2.10",
    "1.2.3",
    " v0.5.1 ",
    "dev",
    "",
    "1.2",
    "1.2.3.4",
    "vv1.2.3",
    "V1.2.3",
    "v01.002.0003",
    "1.2.3-beta",
    "0.6.0",
]

PAIRS = [
    ("0.3.0", "0.2.9"),
    ("0.2.0", "0.2.0"),
    ("0.0.1", "dev"),
    ("v0.6.0", "0.5.1"),
    ("0.5.1", "0.6.0"),
    ("1.0.0", "0.99.99"),
    ("0.10.0", "0.9.0"),
    ("dev", "dev"),
    ("zły", "0.0.0"),
]


def asset(**over: Any) -> dict[str, Any]:
    base: dict[str, Any] = {
        "name": "Bieznia-Setup-0.3.0.exe",
        "browser_download_url": URL,
        "digest": f"sha256:{SHA}",
        "size": 10010,
    }
    base.update(over)
    return base


def release(assets: list[Any], **over: Any) -> dict[str, Any]:
    data: dict[str, Any] = {"tag_name": "v0.3.0", "body": "  Poprawki połączenia\n", "assets": assets}
    data.update(over)
    return data


RELEASES: dict[str, dict[str, Any]] = {
    "zwykle": release([{"name": "Bieznia-Setup-0.3.0.exe.sha256"}, asset()]),
    "bez_v_w_tagu": release([asset()], tag_name="0.4.1"),
    "wielkie_litery_sumy": release([asset(digest=f"sha256:{SHA.upper()}")]),
    "bez_opisu": release([asset()], body=None),
    "pusty_opis": release([asset()], body=""),
    "bez_rozmiaru": release([asset(size=None)]),
    "rozmiar_tekstem": release([asset(size="123")]),
    "rozmiar_ulamkowy": release([asset(size=12.7)]),
    "pierwszy_pasujacy": release(
        [
            {"name": "inne.zip"},
            asset(name="Bieznia-Setup-0.3.1.exe"),
            asset(name="Bieznia-Setup-0.3.2.exe", digest=None),
        ]
    ),
    "brak_sumy": release([asset(digest=None)]),
    "suma_nie_sha256": release([asset(digest=f"md5:{SHA}")]),
    "suma_za_krotka": release([asset(digest=f"sha256:{SHA[:-1]}")]),
    "obcy_serwer": release([asset(browser_download_url="https://zly-serwer.example/Bieznia-Setup-0.3.0.exe")]),
    "http": release([asset(browser_download_url=URL.replace("https", "http"))]),
    "brak_adresu": release([asset(browser_download_url=None)]),
    "drugi_host": release(
        [asset(browser_download_url="https://objects.githubusercontent.com/x/Bieznia-Setup-0.3.0.exe")]
    ),
    "host_wielkimi": release([asset(browser_download_url="https://GitHub.com/x/Bieznia-Setup-0.3.0.exe")]),
    "z_portem": release([asset(browser_download_url="https://github.com:443/x/Bieznia-Setup-0.3.0.exe")]),
    "bez_instalatora": {"tag_name": "v1.0.0", "assets": [{"name": "inne.zip"}]},
    "zla_nazwa": release([asset(name="bieznia-setup-0.3.0.exe"), asset(name="Bieznia-Setup-0.3.exe")]),
    "bez_zasobow": {"tag_name": "v1.0.0"},
    "zly_tag": release([asset()], tag_name="najnowsza"),
    "bez_tagu": release([asset()], tag_name=None),
}


def parsed(data: dict[str, Any]) -> dict[str, Any]:
    try:
        r = akt.parse_release(data)
    except akt.UpdateError as exc:
        return {"error": str(exc)}
    return {
        "release": {
            "version": r.version,
            "notes": r.notes,
            "url": r.url,
            "sha256": r.sha256,
            "size": r.size,
            "name": r.name,
        }
    }


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    versions = {
        "parse": [{"text": t, "version": list(akt.parse_version(t))} for t in VERSIONS],
        "newer": [{"candidate": a, "current": b, "newer": akt.is_newer(a, b)} for a, b in PAIRS],
        "api_latest": akt.API_LATEST,
    }
    releases = [{"case": k, "input": v, **parsed(v)} for k, v in RELEASES.items()]
    (OUT / "versions.json").write_text(json.dumps(versions, ensure_ascii=False, indent=1) + "\n", "utf-8")
    (OUT / "releases.json").write_text(json.dumps(releases, ensure_ascii=False, indent=1) + "\n", "utf-8")


if __name__ == "__main__":
    main()
