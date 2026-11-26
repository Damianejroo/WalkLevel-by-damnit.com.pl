"""Fixtury modułu updates (Rust) z prawdziwej implementacji bieznia/aktualizacje.py.

Uruchom z katalogu głównego repozytorium:
    set PYTHONPATH=.
    .venv\\Scripts\\python.exe walklevel\\tools\\fixtures\\updates.py
"""

from __future__ import annotations

import hashlib
import io
import json
import tempfile
import urllib.error
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
    "\x1fv1.2.3\x1c",
    "\u00a00.5.2\u3000",
    "1.2.3\n",
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
    "tag_ze_spacjami": release([asset()], tag_name=" v0.3.0 "),
    "opis_z_separatorami": release([asset()], body="\x1f Notatki \x1c\n"),
    "opis_liczba": release([asset()], body=5),
    "rozmiar_zero": release([asset(size=0)]),
    "rozmiar_prawda": release([asset(size=True)]),
    "rozmiar_tekst_ze_spacjami": release([asset(size=" 12 ")]),
    "rozmiar_z_plusem": release([asset(size="+7")]),
    "suma_liczba": release([asset(digest=12345)]),
    "suma_pusta": release([asset(digest="")]),
    "pusty_adres": release([asset(browser_download_url="")]),
    "ftp": release([asset(browser_download_url="ftp://github.com/Bieznia-Setup-0.3.0.exe")]),
    "z_loginem": release([asset(browser_download_url="https://u:p@github.com/x/Bieznia-Setup-0.3.0.exe")]),
    "kropka_na_koncu_hosta": release([asset(browser_download_url="https://github.com./x")]),
    "schemat_wielkimi": release([asset(browser_download_url="HTTPS://github.com/x")]),
    "host_kodowany_procentami": release([asset(browser_download_url="https://git%68ub.com/x")]),
    "ukosnik_wsteczny": release([asset(browser_download_url="https://github.com\\@evil.example/x")]),
    "bez_ukosnikow": release([asset(browser_download_url="https:github.com/x")]),
    "trzy_ukosniki": release([asset(browser_download_url="https:///github.com/x")]),
    "spacja_na_poczatku": release([asset(browser_download_url="  https://github.com/x")]),
    "spacja_na_koncu_hosta": release([asset(browser_download_url="https://github.com ")]),
    "tab_w_adresie": release([asset(browser_download_url="https://git\thub.com/x")]),
    "podobny_host": release([asset(browser_download_url="https://github.com.evil.example/x")]),
}


class FakeResponse(io.BytesIO):
    def __init__(self, data: bytes, url: str) -> None:
        super().__init__(data)
        self.url = url

    def geturl(self) -> str:
        return self.url

    def __enter__(self) -> FakeResponse:
        return self

    def __exit__(self, *args: object) -> None:
        pass


def opener_for(body: bytes | None = None, status: int | None = None, final_url: str | None = None) -> Any:
    def opener(request: Any, timeout: float) -> FakeResponse:
        if status is not None:
            raise urllib.error.HTTPError(request.full_url, status, "blad", None, None)  # type: ignore[arg-type]
        if body is None:
            raise OSError("brak sieci")
        return FakeResponse(body, final_url or request.full_url)

    return opener


GOOD = json.dumps(release([asset()]), ensure_ascii=False).encode()
FETCH: dict[str, dict[str, Any]] = {
    "ok": {"body": GOOD.decode()},
    "bom": {"body": "\ufeff" + GOOD.decode()},
    "nie_json": {"body": "<html>"},
    "lista": {"body": "[1, 2]"},
    # "pad" spacji wstawionych przed ostatnim znakiem treści (żeby fixtura była mała)
    "w_limicie_1mb": {"body": GOOD.decode(), "pad": 1_000_000 - len(GOOD)},
    "obciety_ponad_1mb": {"body": GOOD.decode(), "pad": 1_000_000 - len(GOOD) + 1},
    "brak_sieci": {},
    "404": {"status": 404},
    "403": {"status": 403},
    "500": {"status": 500},
}


def fetched(case: dict[str, Any]) -> dict[str, Any]:
    body = None
    if "body" in case:
        text = case["body"]
        text = text[:-1] + " " * case.get("pad", 0) + text[-1:]
        body = text.encode("utf-8")
    try:
        r = akt.fetch_latest(opener_for(body, case.get("status")))
    except akt.UpdateError as exc:
        return {"error": str(exc)}
    return {"version": r.version}


PAYLOAD_REPEAT = 60_000  # b"MZ" + b"instalator" * N = 600002 B, czyli 3 kawałki po 256 KiB
DOWNLOADS: dict[str, dict[str, Any]] = {
    "ok": {},
    "rozmiar_nieznany": {"size": 0},
    "dopisany_bajt": {"extra": "x"},
    "za_duzy": {"size": 300_000},
    "tuz_w_limicie": {"size": 600002 - 1024},
    "tuz_ponad_limit": {"size": 600002 - 1025},
    "przekierowanie_poza_github": {"final_url": "https://evil.example/x.exe"},
    "przekierowanie_na_cdn": {"final_url": "https://release-assets.githubusercontent.com/a/b"},
    "blad_http": {"status": 500},
    "brak_sieci": {"network": True},
    "zly_adres_wydania": {"url": "http://github.com/x/Bieznia-Setup-0.3.0.exe"},
}


def downloaded(case: dict[str, Any]) -> dict[str, Any]:
    payload = b"MZ" + b"instalator" * PAYLOAD_REPEAT
    data = payload + case.get("extra", "").encode()
    release_ = akt.Release(
        version="0.3.0",
        notes="",
        url=case.get("url", URL),
        sha256=hashlib.sha256(payload).hexdigest(),
        size=case.get("size", len(payload)),
        name="Bieznia-Setup-0.3.0.exe",
    )
    calls: list[list[int]] = []
    body = None if case.get("network") else data
    with tempfile.TemporaryDirectory() as tmp:
        try:
            path = akt.download(
                release_,
                Path(tmp),
                opener_for(body, case.get("status"), case.get("final_url")),
                lambda done, total: calls.append([done, total]),
            )
            result: dict[str, Any] = {"file": path.name, "ok": path.read_bytes() == payload}
        except akt.UpdateError as exc:
            result = {"error": str(exc)}
        result["files"] = sorted(p.name for p in Path(tmp).iterdir())
    result["progress"] = calls
    return result


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
    fetch = [{"case": k, **v, **fetched(v)} for k, v in FETCH.items()]
    downloads = {
        "payload_repeat": PAYLOAD_REPEAT,
        "cases": [{"case": k, **v, **downloaded(v)} for k, v in DOWNLOADS.items()],
    }
    (OUT / "versions.json").write_text(json.dumps(versions, ensure_ascii=False, indent=1) + "\n", "utf-8")
    (OUT / "releases.json").write_text(json.dumps(releases, ensure_ascii=False, indent=1) + "\n", "utf-8")
    (OUT / "fetch.json").write_text(json.dumps(fetch, ensure_ascii=False, indent=1) + "\n", "utf-8")
    (OUT / "download.json").write_text(json.dumps(downloads, ensure_ascii=False, indent=1) + "\n", "utf-8")


if __name__ == "__main__":
    main()
