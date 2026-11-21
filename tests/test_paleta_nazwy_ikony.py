"""Paleta 20 kolorów, losowe nazwy osób i ikony z gier."""

from __future__ import annotations

import os
import random
from pathlib import Path

import pytest

from bieznia import ikony, nazwy, paleta
from bieznia.gui.theme import DARK, LIGHT, contrast

# --- paleta ------------------------------------------------------------------------------


@pytest.mark.parametrize("k", paleta.KOLORY, ids=lambda k: k.name)
def test_every_colour_keeps_contrast_in_both_themes(k: paleta.Kolor) -> None:
    for bg in (DARK.bg, DARK.surface):
        assert contrast(k.dark, bg) >= 4.5
    assert contrast(paleta.ON_DARK_THEME, k.dark) >= 4.5
    for bg in (LIGHT.bg, LIGHT.surface):
        assert contrast(k.light, bg) >= 4.5
    assert contrast(paleta.ON_LIGHT_THEME, k.light) >= 4.5


def test_palette_has_20_distinct_colours_and_mint_is_app_default() -> None:
    assert len({k.name for k in paleta.KOLORY}) == 20
    mint = paleta.kolor("Mięta")
    assert (mint.dark, mint.light) == (DARK.accent, LIGHT.accent)
    assert paleta.kolor("nie ma takiego").name == "Mięta"


def test_icon_colour_matching() -> None:
    yellow = [(250, 215, 0, 255)] * 50 + [(20, 20, 20, 255)] * 50
    assert paleta.from_rgb_pixels(yellow).name == "Banan"
    red = [(220, 30, 30, 255)] * 60
    assert paleta.from_rgb_pixels(red).name == "Czerwień"
    gray = [(128, 128, 128, 255)] * 100 + [(200, 0, 0, 255)] * 3  # prawie bez koloru
    assert paleta.from_rgb_pixels(gray).name == "Mięta"
    assert paleta.from_rgb_pixels([]).name == "Mięta"


def test_yellow_and_orange_profiles_change_run_colour() -> None:
    assert paleta.clashes_with_run(paleta.kolor("Banan"))
    assert paleta.clashes_with_run(paleta.kolor("Pomarańcz"))
    assert not paleta.clashes_with_run(paleta.kolor("Błękit"))


# --- nazwy -------------------------------------------------------------------------------


def test_names_have_matching_gender() -> None:
    masc = {m for m, _f in nazwy.ADJECTIVES}
    fem = {f for _m, f in nazwy.ADJECTIVES}
    for name in nazwy.all_names():
        adj, noun = name.split(" ", 1)
        if noun in nazwy.NOUNS_M:
            assert adj in masc, name
        else:
            assert noun in nazwy.NOUNS_F and adj in fem, name
    assert len(nazwy.all_names()) == len(set(nazwy.all_names())) == 396


def test_random_name_avoids_taken_ones() -> None:
    rng = random.Random(1)
    taken = set(nazwy.all_names()[:-1])
    assert nazwy.random_name(taken, rng) == nazwy.all_names()[-1]
    everything = set(nazwy.all_names())
    assert nazwy.random_name(everything, rng).endswith(" 2")
    assert len(set(nazwy.suggestions(3, rng=rng))) == 3


# --- ikony -------------------------------------------------------------------------------


def make_game(root: Path) -> Path:
    game = root / "Baldurs Gate 3"
    (game / "bin").mkdir(parents=True)
    (game / "Redist").mkdir()
    (game / "bin" / "bg3.exe").write_bytes(b"x" * 5_000_000)
    (game / "bin" / "CrashReporter.exe").write_bytes(b"x" * 9_000_000)
    (game / "Redist" / "vc_redist.x64.exe").write_bytes(b"x")
    (game / "unins000.exe").write_bytes(b"x")
    (game / "Launcher").mkdir()
    (game / "Launcher" / "LariLauncher.exe").write_bytes(b"x" * 1000)
    return game


def test_pick_exe_skips_installers_and_crash_reporters(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(ikony, "_icon_count", lambda _p: 1)
    game = make_game(tmp_path)
    found = ikony.resolve(game)
    assert found is not None
    assert found.exe.name == "bg3.exe"
    assert found.name == "Baldurs Gate 3"
    # wskazany sam plik w bin - nazwa i tak z folderu gry
    direct = ikony.resolve(game / "bin" / "bg3.exe")
    assert direct is not None and direct.name == "Baldurs Gate 3"
    assert ikony.resolve(tmp_path / "nie ma") is None


@pytest.mark.skipif(os.name != "nt", reason="ikony z plików .exe tylko na Windows")
def test_icon_from_real_windows_exe(tmp_path: Path) -> None:
    from PySide6.QtWidgets import QApplication

    QApplication.instance() or QApplication([])
    notepad = Path(os.environ.get("WINDIR", r"C:\Windows")) / "System32" / "notepad.exe"
    out = tmp_path / "ikona.png"
    assert ikony.extract_icon(notepad, out)
    assert out.exists() and out.stat().st_size > 100
    assert ikony.icon_color(out).name in paleta.names()
