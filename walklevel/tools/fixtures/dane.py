"""Fixtury modułu store/icons (osoby.json, paleta, nazwy) z prawdziwej implementacji 0.5.x.

Uruchamiać z katalogu głównego repozytorium:
    PYTHONPATH=. .venv/Scripts/python.exe walklevel/tools/fixtures/dane.py

Nowe identyfikatory (losowe w Pythonie i w Rust) są zapisane jako "<nowe-id>" -
test Rust traktuje je jako dowolne 10 znaków hex.
"""

from __future__ import annotations

import json
import os
import random
import tempfile
from pathlib import Path

from bieznia import dane, nazwy, paleta

OUT = Path("walklevel/crates/core/tests/fixtures/dane").resolve()
NEW_ID = "<nowe-id>"

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


def dump(name: str, data: object) -> None:
    text = json.dumps(data, indent=1, ensure_ascii=False) + "\n"
    (OUT / name).write_text(text, "utf-8", newline="\n")


def realistic_store(tmp: Path) -> str:
    """Plik zapisany przez 0.5.x po typowym użyciu (prawdziwe losowe id)."""
    os.chdir(tmp)
    store = dane.Store.create("Szalony Biegacz", "Morski", LEGACY)
    store.set(
        "training_levels", [{"name": "Lekko", "speed": 2.5}, {"name": "Szybko", "speed": 5.0}]
    )
    store.set("window_geometry", "AdnQywADAAAAAAJTAAABLQ==")
    cp = store.add_profile(
        "Cyberpunk 2077",
        icon_color="Banan",
        icon_file="cp.png",
        exe=r"D:\Gry\Cyberpunk 2077\bin\x64\Cyberpunk2077.exe",
        copy_from=store.profile,
    )
    store.set("run_on", 5.5)
    store.update_profile(cp.id, online=False)
    store.add_profile("Minecraft", color="Zieleń", glyph="\ue7fc", activate=False)
    store.set("large_text", True)
    second = store.add_person("Nocna Wędrowniczka", "Orchidea", copy_profiles_from=store.people[0])
    store.set("theme", "light")
    store.switch_profile(second.profiles[1].id)
    store.set("walk_speed", 3.25)
    store.switch_person(store.people[0].id)
    return store.path.read_text("utf-8")


def load_cases(tmp: Path) -> list[dict]:
    real = realistic_store(tmp)
    raw_cases: list[tuple[str, str]] = [("realny_0_5", real)]
    old = {
        "aktywna": "nie-ma-takiej",
        "wspolne": {"device_name": "KS-NACH-X21C", "toggle_pad": None, "stary_klucz": 3},
        "osoby": [
            {
                "id": "aaaaaaaaaa",
                "nazwa": "Ola",
                "kolor": "Zielony",
                "aktywny_profil": "zly",
                "profile": [
                    {"id": "p1", "nazwa": "", "ustawienia": {"walk_speed": 1, "zly": 2}},
                    "nie-slownik",
                    {
                        "id": "p2",
                        "nazwa": "Gra",
                        "kolor": "Fuksja",
                        "gra_online": 0,
                        "glif": "\ue7fc",
                        "kolor_z_ikony": "Banan",
                        "exe": "C:\\g.exe",
                    },
                    {"nazwa": "Bez id", "gra_online": None},
                ],
            },
            {"id": 12345, "nazwa": 7, "wyglad": {"theme": "dark", "obcy": 1}, "profile": []},
            {"nazwa": "", "profile": [{"id": "q1"}]},
            42,
        ],
    }
    raw_cases.append(("stary_niepelny", json.dumps(old, ensure_ascii=False)))
    raw_cases.append(("bez_osob", json.dumps({"wersja": 1, "osoby": []})))
    raw_cases.append(("uszkodzony", "{zepsuty"))
    raw_cases.append(
        (
            "pytaj_falsz",
            json.dumps(
                {
                    "pytaj_przy_starcie": False,
                    "aktywna": "b",
                    "osoby": [{"id": "a"}, {"id": "b", "kolor": "Róż"}],
                },
                ensure_ascii=False,
            ),
        )
    )
    raw_cases.append(
        ("wspolne_null", json.dumps({"wspolne": None, "osoby": [{"id": "x", "wyglad": None}]}))
    )

    out = []
    for name, text in raw_cases:
        path = tmp / f"{name}.json"
        path.write_text(text, "utf-8")
        orig = dane.new_id
        dane.new_id = lambda: NEW_ID
        try:
            store = dane.Store.load(path)
        finally:
            dane.new_id = orig
        out.append(
            {
                "nazwa": name,
                "wejscie": text,
                "wynik": None if store is None else store.to_dict(),
                "flat": None if store is None else store.flat(DEFAULTS),
            }
        )
    return out


def create_case(tmp: Path) -> dict:
    os.chdir(tmp)
    orig = dane.new_id
    dane.new_id = lambda: NEW_ID
    try:
        store = dane.Store.create("Szalony Biegacz", "Morski", LEGACY, path=tmp / "nowy.json")
    finally:
        dane.new_id = orig
    return {
        "legacy": LEGACY,
        "defaults": DEFAULTS,
        "wynik": store.to_dict(),
        "flat": store.flat(DEFAULTS),
    }


def palette_cases() -> dict:
    rng = random.Random(7)
    pixel_cases = []
    for i in range(60):
        n = rng.randint(0, 200)
        if i % 5 == 0:  # prawie szare
            px = [(v, v, v, 255) for v in (rng.randint(0, 255) for _ in range(n))]
            px += [
                (rng.randint(0, 255), rng.randint(0, 255), rng.randint(0, 255), 255)
                for _ in range(rng.randint(0, 10))
            ]
        else:
            base = (rng.randint(0, 255), rng.randint(0, 255), rng.randint(0, 255))
            px = []
            for _ in range(n):
                if rng.random() < 0.6:
                    c = tuple(max(0, min(255, b + rng.randint(-25, 25))) for b in base)
                else:
                    c = (rng.randint(0, 255), rng.randint(0, 255), rng.randint(0, 255))
                px.append((*c, rng.choice((0, 100, 127, 128, 200, 255))))
        pixel_cases.append({"piksele": px, "kolor": paleta.from_rgb_pixels(px).name})
    # przypadki brzegowe: remis koszyków (wygrywa pierwszy), progi alfy, jasności i 8%
    edge = [
        [(255, 0, 0, 255)] * 10 + [(0, 0, 255, 255)] * 10,
        [(0, 0, 255, 255)] * 10 + [(255, 0, 0, 255)] * 10,
        [(255, 0, 0, 127)] * 10,
        [(255, 0, 0, 128)] * 10,
        [(31, 0, 0, 255)] * 10,
        [(255, 235, 235, 255)] * 10,
        [(200, 0, 0, 255)] * 8 + [(128, 128, 128, 255)] * 92,
        [(200, 0, 0, 255)] * 7 + [(128, 128, 128, 255)] * 93,
        [(255, 0, 5, 255)] * 5,
        [(0, 255, 255, 255)] * 5,
    ]
    for px in edge:
        pixel_cases.append({"piksele": px, "kolor": paleta.from_rgb_pixels(px).name})
    hues = [h / 2 for h in range(0, 720, 3)] + [-10.0, 365.5, 720.0]
    return {
        "kolory": [
            {"name": k.name, "hue": k.hue, "dark": k.dark, "light": k.light} for k in paleta.KOLORY
        ],
        "domyslny": paleta.DOMYSLNY,
        "piksele": pixel_cases,
        "najblizszy": [{"hue": h, "kolor": paleta.nearest(h).name} for h in hues],
        "z_biegiem": [k.name for k in paleta.KOLORY if paleta.clashes_with_run(k)],
    }


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    root = Path.cwd()
    with tempfile.TemporaryDirectory() as t:
        tmp = Path(t)
        cases = load_cases(tmp)
        created = create_case(tmp)
        os.chdir(root)
    dump("wczytanie.json", cases)
    dump("nowy.json", created)
    dump("paleta.json", palette_cases())
    dump("nazwy.json", {"wszystkie": nazwy.all_names()})


if __name__ == "__main__":
    main()
