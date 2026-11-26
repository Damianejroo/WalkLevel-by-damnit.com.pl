"""Fixtury modułu backup (kopia danych .bieznia) z prawdziwej implementacji 0.5.x (bieznia/kopia.py).

Uruchamiać z katalogu głównego repozytorium (PYTHONPATH=.):
    .venv\\Scripts\\python.exe walklevel\\tools\\fixtures\\kopia.py

Python -> Rust: kopie zrobione przez kopia.export (pełna, same wyniki z logami) i ręcznie
złożona kopia w wersji 1 (0.3.x: czasy start / koniec, dziesięć pól). Do każdej zapisany jest
wynik kopia.read (odczyt.json), źródłowy osoby.json (do porównania eksportu Rust) i wyniki
importów (import.json).

Rust -> Python: plik z_rust.bieznia tworzy test Rust
    cargo test -p walklevel-core --test backup_fixtures -- --ignored generate_rust_copy
a ten skrypt czyta go przez kopia.read i importuje (z_rust_odczyt.json). Kolejność: najpierw
test Rust (gdy z_rust.bieznia trzeba odświeżyć), potem ten skrypt, potem zwykłe testy.
"""

from __future__ import annotations

import json
import os
import tempfile
import zipfile
from pathlib import Path
from typing import Any

from bieznia import dane, kopia
from bieznia.historia import COLUMN_NAMES, History, Session

OUT = Path(__file__).resolve().parents[2] / "crates" / "core" / "tests" / "fixtures" / "kopia"
PNG = kopia.PNG_MAGIC + b"ikona-cyberpunk"


def dump(name: str, data: object) -> None:
    text = json.dumps(data, indent=1, ensure_ascii=False) + "\n"
    (OUT / name).write_text(text, "utf-8", newline="\n")


def preview_dict(p: kopia.Preview) -> dict[str, Any]:
    return {
        "appVersion": p.app_version,
        "hasSettings": p.has_settings,
        "hasResults": p.has_results,
        "describe": kopia.describe(p),
        "people": [x.to_dict() for x in p.people],
        "sessions": [dict(zip(COLUMN_NAMES, s.row(), strict=True)) for s in p.sessions],
        "icons": {k: v.hex() for k, v in sorted(p.icons.items())},
    }


def store_dict(store: dane.Store) -> dict[str, Any]:
    """Wynik importu bez losowych id: nazwy osób, profile (nazwa, kolor, ustawienia, ikona)."""
    return {
        "active": store.person.name,
        "people": [
            {
                "name": p.name,
                "color": p.color,
                "profiles": [
                    {
                        "name": pr.name,
                        "color": pr.color,
                        "glyph": pr.glyph,
                        "settings": pr.settings,
                        "icon": (
                            (dane.ICONS_DIR / pr.icon_file).read_bytes().hex()
                            if pr.icon_file
                            else None
                        ),
                    }
                    for pr in p.profiles
                ],
            }
            for p in store.people
        ],
    }


def report_dict(r: kopia.ImportReport) -> dict[str, int]:
    return {
        "peopleAdded": r.people_added,
        "peopleMerged": r.people_merged,
        "profilesAdded": r.profiles_added,
        "profilesUpdated": r.profiles_updated,
        "sessionsAdded": r.sessions_added,
    }


def build_world() -> tuple[dane.Store, History]:
    store = dane.Store.create("Szalony Biegacz", "Morski", {"walk_speed": 1.5, "theme": "dark"})
    a = store.person
    dane.ICONS_DIR.mkdir(parents=True)
    (dane.ICONS_DIR / "cp.png").write_bytes(PNG)
    cp = store.add_profile(
        "Cyberpunk 2077", icon_file="cp.png", color="Banan", copy_from=a.profile
    )
    store.set("walk_speed", 2.2)
    store.add_profile("Stardew Valley", glyph="", copy_from=a.profile)
    b = store.add_person("Nocna Wędrowniczka", "Orchidea")
    store.set("large_text", True)
    history = History(Path("dane/historia.db"))
    s1 = Session("s1", a.id, a.name, cp.id, cp.name, 1764000000.25, 1764000600.5, 1000, 0.5, 600)
    s1.device_model, s1.driver, s1.firmware = "KS-NACH-X21C", "kingsmith", "1.2.3"
    s1.app_version, s1.visibility, s1.tz_offset_min = "0.5.1", "znajomi", 60
    history.save(s1)
    s2 = Session("s2", a.id, a.name, a.profiles[0].id, "Domyślny", 1764100000.0, 1764100040.0)
    s2.steps, s2.distance_km, s2.active_s, s2.source, s2.mode = 30, 0.01, 40.0, "symulator", "trening"
    history.save(s2)
    history.save(
        Session("s3", b.id, b.name, b.profiles[0].id, "Domyślny", 1764200000.0, 1764200600.0,
                900, 0.4, 600.0, visibility="publiczna")
    )
    store.switch_person(a.id)
    return store, history


def v1_copy(path: Path) -> None:
    """Kopia z 0.3.x: wersja 1, sesje z dziesięcioma polami i czasami start / koniec."""
    manifest = {
        "format": kopia.FORMAT,
        "wersja": 1,
        "aplikacja": "0.3.2",
        "utworzono": "2025-12-01T10:00:00",
        "zawartosc": {"ustawienia": True, "wyniki": True, "logi": False},
    }
    people = {
        "osoby": [
            {
                "id": "a1b2c3d4e5",
                "nazwa": "Stary Piechur",
                "kolor": "Niebo",
                "wyglad": {"theme": "light"},
                "aktywny_profil": "p000000001",
                "profile": [
                    {"id": "p000000001", "nazwa": "Domyślny", "kolor": "Mięta",
                     "glif": "", "ustawienia": {"walk_speed": 1.8}},
                ],
            }
        ]
    }
    sessions = [
        {"id": "0123456789abcdef0123456789abcdef", "osoba": "a1b2c3d4e5",
         "osoba_nazwa": "Stary Piechur", "profil": "p000000001", "profil_nazwa": "Domyślny",
         "start": 1735000000.0, "koniec": 1735000900.0, "kroki": 1200, "dystans_km": 0.75,
         "ruch_s": 880.0},
    ]
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        zf.writestr("manifest.json", json.dumps(manifest, ensure_ascii=False, indent=1))
        zf.writestr("osoby.json", json.dumps(people, ensure_ascii=False, indent=1))
        zf.writestr("historia.jsonl", "".join(json.dumps(s) + "\n" for s in sessions))


def import_into_fresh(file: Path, where: Path) -> dict[str, Any]:
    """Import całego pliku na „innym komputerze” (nowa osoba Cicha Tropicielka)."""
    where.mkdir()
    os.chdir(where)
    mine = dane.Store.create("Cicha Tropicielka", "Fiolet", {})
    history = History(Path("dane/historia.db"))
    preview = kopia.read(file)
    choice = kopia.ImportChoice(people={p.id for p in preview.people})
    report = kopia.apply_import(mine, history, preview, choice, Path("dane/historia.db"))
    out = {"report": report_dict(report), "store": store_dict(mine), "sessions": history.count()}
    history.close()
    return out


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp_s:
        tmp = Path(tmp_s)
        src = tmp / "zrodlo"
        src.mkdir()
        os.chdir(src)
        store, history = build_world()
        Path("logs").mkdir()
        Path("logs/bieznia.log").write_bytes(b"2026-11-26 10:00:00 INFO    bieznia: start\n")
        everyone = {p.id: None for p in store.people}
        kopia.export(store, history, kopia.ExportChoice(people=everyone), OUT / "pelna.bieznia")
        kopia.export(
            store,
            history,
            kopia.ExportChoice(people=everyone, settings=False, logs=True),
            OUT / "wyniki.bieznia",
        )
        (OUT / "zrodlo_osoby.json").write_text(store.path.read_text("utf-8"), "utf-8", newline="\n")
        v1_copy(OUT / "v1.bieznia")
        dump(
            "odczyt.json",
            {
                name: preview_dict(kopia.read(OUT / name))
                for name in ("pelna.bieznia", "wyniki.bieznia", "v1.bieznia")
            },
        )

        # import u siebie: łączenie z zachowaniem, z zastąpieniem i jako nowa osoba
        a = store.people[0]
        cp = next(p for p in a.profiles if p.name == "Cyberpunk 2077")
        store.switch_profile(cp.id)
        store.set("walk_speed", 9.9)
        preview = kopia.read(OUT / "pelna.bieznia")
        steps = []
        for label, choice in (
            ("keep", kopia.ImportChoice(people={a.id}, keep_my_settings=True)),
            ("replace", kopia.ImportChoice(people={a.id})),
            ("asNew", kopia.ImportChoice(people={a.id}, new_person={a.id})),
        ):
            r = kopia.apply_import(store, history, preview, choice, Path("dane/historia.db"))
            steps.append({"label": label, "report": report_dict(r), "store": store_dict(store),
                          "sessions": history.count()})
        history.close()
        imports = {
            "fresh": {
                name: import_into_fresh(OUT / name, tmp / f"inny-{name}")
                for name in ("pelna.bieznia", "wyniki.bieznia", "v1.bieznia")
            },
            "own": steps,
        }
        dump("import.json", imports)

        rust = OUT / "z_rust.bieznia"
        if rust.exists():
            dump(
                "z_rust_odczyt.json",
                {
                    "read": preview_dict(kopia.read(rust)),
                    "import": import_into_fresh(rust, tmp / "inny-rust"),
                },
            )
        else:
            print("Brak z_rust.bieznia - najpierw test Rust generate_rust_copy")
        os.chdir(tmp.parent)


if __name__ == "__main__":
    main()
