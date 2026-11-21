"""Osoby i profile gier: zapis, przełączanie, kopie profili, migracja z 0.2."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from bieznia import dane

DEFAULTS = {
    "direction": "keyboard",
    "walk_speed": 1.5,
    "theme": "system",
    "device_address": "",
    "belt_control": True,
}
LEGACY = {
    "direction": "gamepad",
    "pad_idle": "stop",
    "walk_speed": 2.0,
    "theme": "dark",
    "floating_button": True,
    "device_address": "A0:24:42:30:E5:B3",
    "belt_control": True,
    "toggle_key": {"scan": 66, "extended": False, "name": "F8", "vk": 119},
    "nieznany_klucz": 1,
}


@pytest.fixture
def store(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> dane.Store:
    monkeypatch.chdir(tmp_path)
    return dane.Store.create("Szalony Biegacz", "Morski", LEGACY)


def test_migration_puts_old_settings_where_they_belong(store: dane.Store) -> None:
    assert store.shared["device_address"] == "A0:24:42:30:E5:B3"  # wspólne
    assert store.person.look == {"theme": "dark", "floating_button": True}  # wygląd osoby
    assert store.profile.name == "Domyślny"
    assert store.profile.settings == {"direction": "gamepad", "pad_idle": "stop", "walk_speed": 2.0}
    flat = store.flat(DEFAULTS)
    assert flat["walk_speed"] == 2.0 and flat["theme"] == "dark" and flat["belt_control"]
    assert flat["show_person_switch"] is True  # domyślne nowe opcje wyglądu
    assert "nieznany_klucz" not in flat


def test_store_round_trips_through_disk(store: dane.Store) -> None:
    store.add_profile("Cyberpunk 2077", icon_color="Banan", icon_file="x.png", exe=r"D:\cp.exe")
    again = dane.Store.load(store.path)
    assert again is not None
    assert [p.name for p in again.person.profiles] == ["Domyślny", "Cyberpunk 2077"]
    cp = again.profile
    assert (cp.color, cp.icon_color, cp.icon_file, cp.glyph) == ("Banan", "Banan", "x.png", None)
    raw = json.loads(store.path.read_text(encoding="utf-8"))
    assert raw["wersja"] == dane.FORMAT_VERSION


def test_settings_go_to_the_active_profile_only(store: dane.Store) -> None:
    default = store.profile
    game = store.add_profile("Elden Ring", copy_from=default)
    store.set("walk_speed", 3.0)  # zmiana w aktywnym profilu (Elden Ring)
    assert game.settings["walk_speed"] == 3.0
    assert default.settings["walk_speed"] == 2.0
    store.switch_profile(default.id)
    assert store.flat(DEFAULTS)["walk_speed"] == 2.0
    store.set("theme", "light")  # wygląd - osoba, nie profil
    assert "theme" not in default.settings and store.person.look["theme"] == "light"
    store.set("device_name", "KS-NACH-X21C")  # wspólne
    assert store.shared["device_name"] == "KS-NACH-X21C"


def test_people_have_separate_profiles_and_settings(store: dane.Store) -> None:
    first = store.person
    store.add_profile("Cyberpunk 2077", copy_from=store.profile)
    second = store.add_person("Nocna Wędrowniczka", "Orchidea", copy_profiles_from=first)
    assert store.person is second
    assert [p.name for p in second.profiles] == ["Domyślny", "Cyberpunk 2077"]
    assert {p.id for p in second.profiles}.isdisjoint({p.id for p in first.profiles})
    store.switch_profile(second.profiles[1].id)
    store.set("walk_speed", 4.0)
    store.switch_person(first.id)
    assert first.profile.settings.get("walk_speed") != 4.0  # osobne nawet dla tej samej gry
    empty = store.add_person("Cicha Tropicielka", "Fiolet")
    assert [p.name for p in empty.profiles] == ["Domyślny"]


def test_last_person_and_last_profile_cannot_be_removed(store: dane.Store) -> None:
    assert not store.remove_person(store.person.id)
    assert not store.remove_profile(store.profile.id)
    extra = store.add_profile("PEAK")
    assert store.remove_profile(extra.id)
    assert store.profile.name == "Domyślny"


def test_no_limits_on_people_and_profiles(store: dane.Store) -> None:
    for i in range(30):
        store.add_profile(f"Gra {i}")
    for i in range(12):
        store.add_person(f"Osoba {i}", "Błękit")
    again = dane.Store.load(store.path)
    assert again is not None and len(again.people) == 13
    assert len(again.people[0].profiles) == 31


def test_duplicate_profile_copies_settings_and_icon(store: dane.Store) -> None:
    dane.ICONS_DIR.mkdir(parents=True)
    (dane.ICONS_DIR / "ikona.png").write_bytes(b"png")
    game = store.add_profile("PEAK", icon_file="ikona.png", copy_from=store.profile)
    dup = store.duplicate_profile(game.id)
    assert dup is not None and dup.name == "PEAK (kopia)"
    assert dup.icon_file and dup.icon_file != "ikona.png"
    assert (dane.ICONS_DIR / dup.icon_file).read_bytes() == b"png"
    assert dup.settings == game.settings


def test_corrupted_file_is_backed_up(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.chdir(tmp_path)
    dane.STORE_FILE.parent.mkdir()
    dane.STORE_FILE.write_text("{zepsuty", encoding="utf-8")
    assert dane.Store.load() is None
    assert list(dane.BACKUP_DIR.glob("uszkodzony-*.json"))


def test_legacy_backup(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.chdir(tmp_path)
    assert dane.legacy_settings() is None  # nowa instalacja
    dane.OLD_SETTINGS.parent.mkdir()
    dane.OLD_SETTINGS.write_text(json.dumps(LEGACY), encoding="utf-8")
    assert dane.legacy_settings() == LEGACY
    backup = dane.backup_legacy()
    assert backup is not None and json.loads(backup.read_text(encoding="utf-8")) == LEGACY
