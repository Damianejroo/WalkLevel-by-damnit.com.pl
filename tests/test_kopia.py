"""Kopia danych: eksport z filtrami, sprawdzanie pliku, import, przywracanie."""

from __future__ import annotations

import json
import zipfile
from pathlib import Path

import pytest

from bieznia import dane, kopia
from bieznia.historia import History, Session

PNG = kopia.PNG_MAGIC + b"ikona"


@pytest.fixture
def world(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> tuple[dane.Store, History]:
    monkeypatch.chdir(tmp_path)
    store = dane.Store.create("Szalony Biegacz", "Morski", {"walk_speed": 1.5})
    a = store.person
    dane.ICONS_DIR.mkdir(parents=True)
    (dane.ICONS_DIR / "cp.png").write_bytes(PNG)
    cp = store.add_profile("Cyberpunk 2077", icon_file="cp.png", color="Banan", copy_from=a.profile)
    store.set("walk_speed", 2.2)
    store.add_profile("Stardew Valley", copy_from=a.profile)
    b = store.add_person("Nocna Wędrowniczka", "Orchidea")
    history = History(Path("dane/historia.db"))
    history.save(Session("s1", a.id, a.name, cp.id, cp.name, 100.0, 700.0, 1000, 0.5, 600))
    history.save(
        Session("s2", a.id, a.name, a.profiles[0].id, "Domyślny", 50.0, 90.0, 30, 0.01, 40)
    )
    history.save(
        Session("s3", b.id, b.name, b.profiles[0].id, "Domyślny", 10.0, 610.0, 900, 0.4, 600)
    )
    store.switch_person(a.id)
    yield store, history
    history.close()


def ids(store: dane.Store, person: str) -> dict[str, str]:
    return {p.name: p.id for p in next(x for x in store.people if x.name == person).profiles}


def test_export_respects_person_profile_and_content_filters(
    world: tuple[dane.Store, History],
) -> None:
    store, history = world
    a = store.people[0]
    choice = kopia.ExportChoice(
        people={a.id: {ids(store, a.name)["Cyberpunk 2077"]}}, settings=True, results=True
    )
    out = Path("kopia.bieznia")
    summary = kopia.export(store, history, choice, out)
    assert (summary.people, summary.profiles, summary.sessions) == (1, 1, 1)
    with zipfile.ZipFile(out) as zf:
        names = set(zf.namelist())
        people = json.loads(zf.read("osoby.json"))["osoby"]
        sessions = zf.read("historia.jsonl").decode().splitlines()
    assert {"manifest.json", "osoby.json", "historia.jsonl", "ikony/cp.png"} <= names
    assert [p["nazwa"] for p in people[0]["profile"]] == ["Cyberpunk 2077"]
    assert len(sessions) == 1 and json.loads(sessions[0])["kroki"] == 1000
    assert not any(n.startswith("logi/") for n in names)


def test_export_results_only_and_logs(world: tuple[dane.Store, History]) -> None:
    store, history = world
    Path("logs").mkdir()
    Path("logs/bieznia.log").write_text("linia\n", encoding="utf-8")
    choice = kopia.ExportChoice(
        people={p.id: None for p in store.people}, settings=False, results=True, logs=True
    )
    out = Path("wyniki.bieznia")
    kopia.export(store, history, choice, out)
    with zipfile.ZipFile(out) as zf:
        people = json.loads(zf.read("osoby.json"))["osoby"]
        assert "logi/bieznia.log" in zf.namelist()
    assert "ustawienia" not in people[0]["profile"][0]  # bez ustawień sterowania
    with pytest.raises(kopia.CopyError):
        kopia.export(store, history, kopia.ExportChoice(people={}), out)


def full_export(store: dane.Store, history: History, path: str = "pelna.bieznia") -> Path:
    out = Path(path)
    kopia.export(store, history, kopia.ExportChoice(people={p.id: None for p in store.people}), out)
    return out


def test_read_rejects_foreign_and_broken_files(
    world: tuple[dane.Store, History], tmp_path: Path
) -> None:
    store, history = world
    good = kopia.read(full_export(store, history))
    assert len(good.people) == 2 and len(good.sessions) == 3 and "cp.png" in good.icons
    evil = tmp_path / "zly.bieznia"
    with zipfile.ZipFile(evil, "w") as zf:
        zf.writestr("manifest.json", json.dumps({"format": kopia.FORMAT, "wersja": 1}))
        zf.writestr("../../windows/system32/x.dll", b"MZ")
    with pytest.raises(kopia.CopyError, match="nieznane"):
        kopia.read(evil)
    newer = tmp_path / "nowszy.bieznia"
    with zipfile.ZipFile(newer, "w") as zf:
        zf.writestr("manifest.json", json.dumps({"format": kopia.FORMAT, "wersja": 99}))
        zf.writestr("osoby.json", "{}")
    with pytest.raises(kopia.CopyError, match="nowszej"):
        kopia.read(newer)
    broken = tmp_path / "zepsuty.bieznia"
    broken.write_bytes(b"to nie zip")
    with pytest.raises(kopia.CopyError, match="uszkodzony"):
        kopia.read(broken)


def test_import_on_another_computer_adds_people_profiles_icons_and_results(
    world: tuple[dane.Store, History], tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    store, history = world
    file = full_export(store, history).resolve()
    other = tmp_path / "inny"
    other.mkdir()
    monkeypatch.chdir(other)
    mine = dane.Store.create("Cicha Tropicielka", "Fiolet", {})
    mine_history = History(Path("dane/historia.db"))
    preview = kopia.read(file)
    choice = kopia.ImportChoice(people={p.id for p in preview.people})
    report = kopia.apply_import(mine, mine_history, preview, choice, Path("dane/historia.db"))
    assert report.people_added == 2 and report.sessions_added == 3
    assert [p.name for p in mine.people] == [
        "Cicha Tropicielka",
        "Szalony Biegacz",
        "Nocna Wędrowniczka",
    ]
    cp = next(p for p in mine.people[1].profiles if p.name == "Cyberpunk 2077")
    assert cp.icon_file and (dane.ICONS_DIR / cp.icon_file).read_bytes() == PNG
    assert cp.settings["walk_speed"] == 2.2
    assert mine.person.name == "Cicha Tropicielka"  # aktywna osoba bez zmian
    assert report.backup is not None and (report.backup / "osoby.json").exists()
    mine_history.close()


def test_import_merge_keeps_or_replaces_profile_settings(world: tuple[dane.Store, History]) -> None:
    store, history = world
    file = full_export(store, history)
    a = store.people[0]
    cp = next(p for p in a.profiles if p.name == "Cyberpunk 2077")
    store.switch_profile(cp.id)
    store.set("walk_speed", 9.9)  # zmiana po eksporcie
    preview = kopia.read(file)
    keep = kopia.ImportChoice(people={a.id}, keep_my_settings=True)
    r1 = kopia.apply_import(store, history, preview, keep, Path("dane/historia.db"))
    assert r1.people_merged == 1 and r1.sessions_added == 0  # te same sesje - bez dublowania
    assert cp.settings["walk_speed"] == 9.9
    replace = kopia.ImportChoice(people={a.id})
    kopia.apply_import(store, history, preview, replace, Path("dane/historia.db"))
    assert cp.settings["walk_speed"] == 2.2
    as_new = kopia.ImportChoice(people={a.id}, new_person={a.id})
    r3 = kopia.apply_import(store, history, preview, as_new, Path("dane/historia.db"))
    assert r3.people_added == 1
    assert store.people[-1].name == "Szalony Biegacz 2" and store.people[-1].id != a.id
    assert r3.sessions_added == 2  # wyniki skopiowane pod nową osobę


def test_restore_brings_back_data_from_before_import(world: tuple[dane.Store, History]) -> None:
    store, history = world
    file = full_export(store, history)
    preview = kopia.read(file)
    a = store.people[0]
    report = kopia.apply_import(
        store,
        history,
        preview,
        kopia.ImportChoice(people={a.id}, new_person={a.id}),
        Path("dane/historia.db"),
    )
    assert len(store.people) == 3
    backup = kopia.latest_backup()
    assert backup == report.backup
    kopia.restore(backup, store.path, Path("dane/historia.db"))
    again = dane.Store.load(store.path)
    assert again is not None and len(again.people) == 2
