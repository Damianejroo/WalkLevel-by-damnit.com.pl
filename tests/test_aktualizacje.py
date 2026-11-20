"""Aktualizacja z GitHub Releases: wybór wydania i zabezpieczenia pobierania."""

from __future__ import annotations

import hashlib
import io
import json
from pathlib import Path
from typing import Any

import pytest

from bieznia import aktualizacje as akt

PAYLOAD = b"MZ" + b"instalator" * 1000
SHA = hashlib.sha256(PAYLOAD).hexdigest()
URL = "https://github.com/Damianejroo/WalkingPad-X21-PC-Gaming-by-damnit.com.pl/releases/download/v0.3.0/Bieznia-Setup-0.3.0.exe"


def release_json(**asset: Any) -> dict[str, Any]:
    base = {
        "name": "Bieznia-Setup-0.3.0.exe",
        "browser_download_url": URL,
        "digest": f"sha256:{SHA}",
        "size": len(PAYLOAD),
    }
    base.update(asset)
    return {
        "tag_name": "v0.3.0",
        "body": "Poprawki połączenia",
        "assets": [{"name": "Bieznia-Setup-0.3.0.exe.sha256"}, base],
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


def opener_for(data: bytes, final_url: str = URL) -> akt.Opener:
    def opener(request: Any, timeout: float) -> FakeResponse:
        return FakeResponse(data, final_url)

    return opener


def test_versions() -> None:
    assert akt.parse_version("v1.2.10") == (1, 2, 10)
    assert akt.is_newer("0.3.0", "0.2.9")
    assert not akt.is_newer("0.2.0", "0.2.0")
    assert akt.is_newer("0.0.1", "dev")  # kod uruchomiony z repo jest zawsze najstarszy


def test_release_is_parsed() -> None:
    r = akt.parse_release(release_json())
    assert (r.version, r.sha256, r.name) == ("0.3.0", SHA, "Bieznia-Setup-0.3.0.exe")


@pytest.mark.parametrize(
    ("asset", "message"),
    [
        ({"digest": None}, "sumy"),
        ({"browser_download_url": "https://zly-serwer.example/Bieznia-Setup-0.3.0.exe"}, "adres"),
        ({"browser_download_url": URL.replace("https", "http")}, "adres"),
    ],
)
def test_suspicious_releases_are_rejected(asset: dict[str, Any], message: str) -> None:
    with pytest.raises(akt.UpdateError, match=message):
        akt.parse_release(release_json(**asset))


def test_release_without_installer() -> None:
    with pytest.raises(akt.UpdateError, match="instalatora"):
        akt.parse_release({"tag_name": "v1.0.0", "assets": [{"name": "inne.zip"}]})


def test_fetch_latest_uses_api() -> None:
    seen: list[str] = []

    def opener(request: Any, timeout: float) -> FakeResponse:
        seen.append(request.full_url)
        return FakeResponse(json.dumps(release_json()).encode(), request.full_url)

    assert akt.fetch_latest(opener).version == "0.3.0"
    assert seen == [akt.API_LATEST]


def test_network_error_is_friendly() -> None:
    def opener(request: Any, timeout: float) -> FakeResponse:
        raise OSError("brak sieci")

    with pytest.raises(akt.UpdateError, match="internet"):
        akt.fetch_latest(opener)


def test_download_verifies_checksum(tmp_path: Path) -> None:
    release = akt.parse_release(release_json())
    path = akt.download(release, tmp_path, opener_for(PAYLOAD))
    assert path.read_bytes() == PAYLOAD


def test_bad_checksum_leaves_nothing(tmp_path: Path) -> None:
    release = akt.parse_release(release_json())
    with pytest.raises(akt.UpdateError, match="Suma"):
        akt.download(release, tmp_path, opener_for(PAYLOAD + b"x"))
    assert list(tmp_path.iterdir()) == []


def test_redirect_outside_github_is_rejected(tmp_path: Path) -> None:
    release = akt.parse_release(release_json())
    with pytest.raises(akt.UpdateError, match="adres"):
        akt.download(release, tmp_path, opener_for(PAYLOAD, "https://evil.example/x.exe"))
    assert list(tmp_path.iterdir()) == []


def test_oversized_download_is_stopped(tmp_path: Path) -> None:
    release = akt.parse_release(release_json(size=10))
    with pytest.raises(akt.UpdateError, match="większy"):
        akt.download(release, tmp_path, opener_for(PAYLOAD))
    assert list(tmp_path.iterdir()) == []


def test_no_releases_yet_is_explained() -> None:
    import urllib.error

    def opener(request: Any, timeout: float) -> FakeResponse:
        raise urllib.error.HTTPError(akt.API_LATEST, 404, "Not Found", None, None)  # type: ignore[arg-type]

    with pytest.raises(akt.UpdateError, match="żadnego wydania"):
        akt.fetch_latest(opener)
