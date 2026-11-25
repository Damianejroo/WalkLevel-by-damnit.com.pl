"""Pobieranie profili bieżni: tylko dane z GitHuba tego repozytorium, sprawdzone sumą i formatem."""

from __future__ import annotations

import io
import json
import urllib.error
from pathlib import Path
from typing import Any

import pytest

from bieznia.sterowniki import pobieranie, profile

NEW = {
    "format": 1,
    "id": "nowa-bieznia",
    "name": "Nowa Bieżnia 9",
    "driver": "ftms",
    "match": ["NB9"],
    "capabilities": {"speed_min": 0.8, "speed_max": 6.0},
}


class FakeResponse(io.BytesIO):
    def __init__(self, data: bytes, url: str) -> None:
        super().__init__(data)
        self.url = url

    def geturl(self) -> str:
        return self.url


def server(files: dict[str, bytes], redirect: str | None = None) -> Any:
    def opener(request: Any, _timeout: float) -> FakeResponse:
        url = request.full_url
        name = url.rsplit("/", 1)[-1]
        if name not in files:
            raise urllib.error.HTTPError(url, 404, "nie ma", None, None)  # type: ignore[arg-type]
        return FakeResponse(files[name], redirect or url)

    return opener


def catalog(**extra: bytes) -> dict[str, bytes]:
    files = {p.name: p.read_bytes() for p in profile.BUILTIN_DIR.glob("*.json")}
    files.pop(pobieranie.INDEX, None)
    files.update(extra)
    index = {
        "format": 1,
        "profile": [{"plik": n, "sha256": pobieranie.sha256(d)} for n, d in sorted(files.items())],
    }
    files[pobieranie.INDEX] = json.dumps(index).encode()
    return files


@pytest.fixture(autouse=True)
def user_dir(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    folder = tmp_path / "profile"
    monkeypatch.setattr(profile, "USER_DIR", folder)
    profile.all_profiles(reload=True)
    yield folder
    monkeypatch.undo()
    profile.all_profiles(reload=True)


def test_index_in_repo_matches_builtin_profiles() -> None:
    """Po zmianie profilu: python -m bieznia.sterowniki.pobieranie (inaczej pobieranie odrzuci)."""
    saved = json.loads((profile.BUILTIN_DIR / pobieranie.INDEX).read_text(encoding="utf-8"))
    assert saved == pobieranie.build_index()


def test_new_model_is_downloaded_and_known_ones_are_skipped(user_dir: Path) -> None:
    files = catalog(**{"nowa-bieznia.json": json.dumps(NEW).encode()})
    result = pobieranie.download_profiles(server(files))
    assert result.added == ["Nowa Bieżnia 9"] and not result.rejected
    assert result.same == len(files) - 2  # wbudowane już są
    assert (user_dir / "nowa-bieznia.json").exists()
    found = profile.profile_for("NB9-1234")
    assert found is not None and found.source == "dodatkowy"
    again = pobieranie.download_profiles(server(files))
    assert not again.added and not again.updated


def test_wrong_checksum_and_bad_profile_are_rejected(user_dir: Path) -> None:
    bad = dict(NEW, driver="python")
    files = catalog(
        **{"zly.json": json.dumps(bad).encode(), "nowa-bieznia.json": json.dumps(NEW).encode()}
    )
    files["nowa-bieznia.json"] = json.dumps(dict(NEW, name="Podmieniony")).encode()  # suma nie gra
    result = pobieranie.download_profiles(server(files))
    assert sorted(result.rejected) == ["nowa-bieznia.json", "zly.json"]
    assert not list(user_dir.glob("*.json")) if user_dir.exists() else True


@pytest.mark.parametrize(
    "entry",
    [{"plik": "../../dane/osoby.json", "sha256": "0" * 64}, {"plik": "x.exe", "sha256": "0" * 64}],
)
def test_index_cannot_point_outside_profiles(entry: dict[str, str]) -> None:
    files = {pobieranie.INDEX: json.dumps({"format": 1, "profile": [entry]}).encode()}
    with pytest.raises(pobieranie.ProfileDownloadError):
        pobieranie.download_profiles(server(files))


def test_only_github_over_https(monkeypatch: pytest.MonkeyPatch) -> None:
    files = catalog()
    with pytest.raises(pobieranie.ProfileDownloadError, match="przekierował"):
        pobieranie.download_profiles(server(files, redirect="https://zly.example/indeks.json"))
    monkeypatch.setattr(pobieranie, "BASE_URL", "http://raw.githubusercontent.com/x")
    with pytest.raises(pobieranie.ProfileDownloadError, match="bezpieczeństwa"):
        pobieranie.download_profiles(server(files))


def test_missing_catalog_and_huge_file_have_clear_messages(monkeypatch: pytest.MonkeyPatch) -> None:
    with pytest.raises(pobieranie.ProfileDownloadError, match="nie jest jeszcze opublikowana"):
        pobieranie.download_profiles(server({}))
    huge = {pobieranie.INDEX: b" " * (pobieranie.MAX_BYTES + 10)}
    with pytest.raises(pobieranie.ProfileDownloadError, match="za duży"):
        pobieranie.download_profiles(server(huge))


def test_profile_from_file_and_removal(tmp_path: Path, user_dir: Path) -> None:
    good = tmp_path / "moja.json"
    good.write_text(json.dumps(NEW), encoding="utf-8")
    assert pobieranie.install_from_file(good).name == "Nowa Bieżnia 9"
    assert profile.profile_for("NB9") is not None
    assert pobieranie.remove_user_profile("nowa-bieznia")
    assert profile.profile_for("NB9") is None
    broken = tmp_path / "zly.json"
    broken.write_text('{"format": 1, "id": "x", "name": "X", "driver": "dll"}', encoding="utf-8")
    with pytest.raises(pobieranie.ProfileDownloadError, match="nie jest poprawny profil"):
        pobieranie.install_from_file(broken)
