"""Fixtury modułu levels (port bieznia/poziomy): historie sesji -> wynik evaluate z Pythona.

Uruchamianie z katalogu głównego repozytorium:
    PYTHONPATH=. .venv/Scripts/python.exe walklevel/tools/fixtures/levels.py

Wejścia mają krótkie liczby dziesiętne (czasy w pełnych sekundach, km do 3 miejsc), żeby
parser JSON po stronie Rusta odczytał je bit w bit tak samo jak Python.
"""

from __future__ import annotations

import json
import random
from dataclasses import fields
from pathlib import Path
from typing import Any

from bieznia import poziomy
from bieznia.historia import Session

OUT = Path("walklevel/crates/core/tests/fixtures/levels")
T0 = 1_767_225_600.0  # 2026-01-01 00:00 UTC
DAY = 86400.0
HOUR = 3600.0

CAMEL = {
    "person_name": "personName",
    "profile_name": "profileName",
    "distance_km": "distanceKm",
    "active_s": "activeS",
    "device_model": "deviceModel",
    "device_profile": "deviceProfile",
    "app_version": "appVersion",
    "tz_offset_min": "tzOffsetMin",
}

_counter = 0


def sess(
    start: float,
    duration: float,
    km: float,
    steps: int | None = None,
    *,
    active: float | None = None,
    game: str = "g1",
    **kw: Any,
) -> Session:
    """Sesja z rozsądnymi domyślnymi: kroki 0,75 m, ruch = czas trwania."""
    global _counter
    _counter += 1
    if steps is None:
        steps = round(km * 1000 / 0.75)
    return Session(
        id=f"{_counter:032x}",
        person="p1",
        person_name="Olek",
        profile=game,
        profile_name=game.upper(),
        start=float(start),
        end=float(start + duration),
        steps=steps,
        distance_km=km,
        active_s=duration if active is None else active,
        **kw,
    )


def session_json(s: Session) -> dict[str, Any]:
    return {CAMEL.get(f.name, f.name): getattr(s, f.name) for f in fields(s)}


def walk_json(w: poziomy.Walk) -> dict[str, Any]:
    return {
        "start": w.start,
        "end": w.end,
        "km": w.km,
        "steps": w.steps,
        "game": w.game,
        "mode": w.mode,
        "tzOffsetMin": w.tz_offset_min,
        "day": w.day,
        "hour": w.hour,
    }


def case(
    name: str,
    sessions: list[Session],
    *,
    now: float,
    friends: int = 0,
    referrals: int = 0,
    earned_before: dict[str, float] | None = None,
    rest_per_week: int = 1,
    defs_path: Path = poziomy.DEFINITIONS,
) -> dict[str, Any]:
    defs = poziomy.load(defs_path)
    walks = poziomy.walks_from_sessions(sessions)
    st = poziomy.evaluate(
        defs,
        walks,
        friends=friends,
        referrals=referrals,
        earned_before=earned_before,
        now=now,
        rest_per_week=rest_per_week,
    )
    nxt = st.next_rank
    return {
        "name": name,
        "input": {
            "sessions": [session_json(s) for s in sessions],
            "now": now,
            "friends": friends,
            "referrals": referrals,
            "earnedBefore": earned_before or {},
            "restPerWeek": rest_per_week,
        },
        "expected": {
            "walks": [walk_json(w) for w in walks],
            "km": st.km,
            "xp": st.xp,
            "level": st.level,
            "rank": st.rank.name,
            "nextRank": nxt.name if nxt else None,
            "levelFloor": st.level_floor,
            "levelCeiling": st.level_ceiling,
            "levelFraction": st.level_fraction,
            "kmToNextLevel": st.km_to_next_level,
            "progress": [
                {
                    "id": p.achievement.id,
                    "earnedAt": p.earned_at,
                    "current": p.current,
                    "target": p.target,
                    "fraction": p.fraction,
                }
                for p in st.progress.values()
            ],
            "earned": [p.achievement.id for p in st.earned],
            "closest": [p.achievement.id for p in st.closest()],
            "closest10": [p.achievement.id for p in st.closest(10)],
        },
    }


def at(day: int, hour: float = 18, minute: float = 0, tz_min: int = 0) -> float:
    """Czas UTC dla lokalnej godziny w danym dniu (strefa w minutach od UTC)."""
    return T0 + day * DAY + hour * HOUR + minute * 60 - tz_min * 60


# --- przypadki -------------------------------------------------------------------------------


def simple() -> dict[str, Any]:
    s = [sess(at(0), 3600, 1.5), sess(at(1), 3600, 4.5), sess(at(2), 3600, 0.3)]
    return case("proste", s, now=T0 + 3 * DAY)


def streak_with_rest() -> dict[str, Any]:
    s = [sess(at(i), 3600, 2.5) for i in range(8) if i != 3]
    return case("seria_z_dniem_wolnym", s, now=T0 + 7 * DAY + HOUR)


def streak_broken() -> dict[str, Any]:
    s = [sess(at(i), 3600, 2.5) for i in (0, 1, 2, 5, 6, 7, 8)]
    return case("seria_przerwana", s, now=T0 + 8 * DAY + HOUR)


def streak_old() -> dict[str, Any]:
    s = [sess(at(i), 3600, 2.5) for i in range(4)] + [sess(at(4), 3600, 1.0)]
    return case("seria_dawna", s, now=T0 + 20 * DAY)


def streak_rest_two() -> dict[str, Any]:
    # wolne w dniach 2 i 5 (dwa na 7), potem 9 i 11; dzień 13 jeszcze trwa
    days = [d for d in range(14) if d not in (2, 5, 9, 11)]
    s = [sess(at(d), 3600, 2.2) for d in days]
    one = case("seria_jeden_wolny", s, now=T0 + 13 * DAY + 2 * HOUR)
    two = case("seria_dwa_wolne", s, now=T0 + 13 * DAY + 2 * HOUR, rest_per_week=2)
    zero = case("seria_bez_wolnych", s, now=T0 + 13 * DAY + 2 * HOUR, rest_per_week=0)
    return [one, two, zero]  # type: ignore[return-value]


def boundaries() -> dict[str, Any]:
    s: list[Session] = []
    # wiarygodność na granicach
    s.append(sess(at(0, 1), 900, 5.0, 0))  # dokładnie 20 km/h, bez kroków
    s.append(sess(at(0, 2), 900, 5.001, 0))  # ponad 20 km/h
    s.append(sess(at(0, 3), 600, 2.0, 2500))  # dokładnie 250 kroków/min, krok 0,8 m
    s.append(sess(at(0, 3, 30), 600, 2.0, 2501))  # ponad 250 kroków/min
    s.append(sess(at(0, 5), 1800, 0.3, 1000))  # krok 0,3 m
    s.append(sess(at(0, 5, 40), 1800, 1.3, 1000))  # krok 1,3 m
    s.append(sess(at(0, 6, 20), 1800, 1.31, 1000))  # krok za długi
    s.append(sess(at(0, 7), 1800, 0.29, 1000))  # krok za krótki
    s.append(sess(at(0, 8), 600, 0.0, 0))  # bez dystansu, z ruchem
    s.append(sess(at(0, 8, 20), 600, 0.0, 300))  # kroki bez dystansu
    s.append(sess(at(0, 8, 40), 600, -1.0, 0))  # ujemny dystans
    s.append(sess(at(0, 9), 600, 1.0, active=0))  # bez ruchu
    s.append(sess(at(0, 9, 20), 600, 1.0, source="symulator"))
    s.append(sess(at(0, 9, 40), 600, 0.8, mode=""))  # pusty tryb = gra
    # nakładanie: start równo z końcem poprzedniej liczy się, wcześniejszy nie
    s.append(sess(at(0, 10), 1200, 1.0))
    s.append(sess(at(0, 10, 20), 600, 1.0))  # start == koniec poprzedniej
    s.append(sess(at(0, 10, 29) + 59, 600, 1.0))  # start sekundę przed końcem
    # okna godzin: 4:00 i 23:00 w środku, 7:00 i 4:00 (noc) poza
    s.append(sess(at(1, 7), 1200, 0.5))
    s.append(sess(at(1, 3, 59) + 59, 600, 0.4))  # 3:59:59 -> noc
    s.append(sess(at(2, 4), 1200, 0.5))  # 4:00 -> rano
    s.append(sess(at(2, 23), 1200, 0.5))  # 23:00 -> noc
    # dzień lokalny z innych stref: 23:30 w UTC+1 to jeszcze ten sam dzień lokalny
    s.append(sess(at(3, 23, 30, tz_min=60), 1200, 1.0, tz_offset_min=60))
    s.append(sess(at(4, 0, 10, tz_min=-300), 1200, 1.0, tz_offset_min=-300))
    s.append(sess(at(4, 23, 50, tz_min=-300), 1200, 1.0, tz_offset_min=-300))
    # sumy 0,1 km - 10 x 0,1 = 0,9999999999999999 (tolerancja progu 1 km)
    for i in range(10):
        s.append(sess(at(6, 6 + i), 120, 0.1, 130))
    # dzienne kroki: dokładnie 7000
    s.append(sess(at(7, 8), 1800, 2.0, 3000))
    s.append(sess(at(7, 12), 1800, 2.5, 4000))
    # dzień serii dokładnie 2 km z trzech kawałków
    s.append(sess(at(8, 8), 900, 0.7))
    s.append(sess(at(8, 9), 900, 0.7))
    s.append(sess(at(8, 10), 900, 0.6))
    return case("granice", s, now=at(9, 12))


def tenths() -> dict[str, Any]:
    """Same sumy 0,1 km: 10 x 0,1 = 0,9999999999999999 - próg 1 km z tolerancją."""
    s = [sess(at(0, 6 + i), 120, 0.1, 130) for i in range(10)]
    s += [sess(at(1, 6 + i), 300, 0.1, 130) for i in range(20)]  # 2 km dnia z 20 kawałków
    return case("sumy_dziesiatych", s, now=at(2, 1))


def day_cap() -> dict[str, Any]:
    s: list[Session] = []
    s.append(sess(at(0, 0, 10), 3 * HOUR, 15.0, 20000))
    s.append(sess(at(0, 1), HOUR, 4.0, 6000))  # nakłada się
    s.append(sess(at(0, 4), 6 * HOUR, 30.0, 40000))  # razem 9 h -> liczy się 5 h
    s.append(sess(at(0, 11), HOUR, 5.0, 7000))  # ponad 8 h -> 0 km, ale gra i godzina są
    s.append(sess(at(1, 0), 8 * HOUR, 40.0, 53000))  # dokładnie 8 h
    s.append(sess(at(1, 9), 1800, 2.0, 2700, game="g2"))  # ponad limit
    s.append(sess(at(2, 23), 2 * HOUR, 8.0, 10000))
    return case("limit_8h", s, now=at(3, 8))


def empty() -> list[dict[str, Any]]:
    eb = {"km1": T0, "nieznane": T0 + 1, "seria365": T0 + 5}
    return [
        case("pusta", [], now=T0),
        case("pusta_znajomi", [], now=T0 + 77, friends=3, referrals=1, earned_before=eb),
        case("same_niewiarygodne", [sess(at(0), 600, 1.0, source="symulator")], now=T0 + DAY),
    ]


def earned_before_min() -> dict[str, Any]:
    s = [sess(at(i), 3600, 4.0) for i in range(5)]
    eb = {"km1": T0 - 1000, "km2": at(4) + 9999, "s10": T0 + 5, "gra100": T0 + 7}
    return case("zdobyte_wczesniej", s, now=at(5), friends=1, earned_before=eb)


def games_and_training() -> dict[str, Any]:
    s = [
        sess(at(0), 3600, 10.0, game="a"),
        sess(at(1), 3600, 10.0, game="b"),
        sess(at(2), 3600, 9.0, game="c"),
        sess(at(3), 3600, 5.0, game="c", mode="trening"),
        sess(at(4), 3600, 1.0, game="c"),
        sess(at(5), 3600, 9.999, game="d"),
        sess(at(6), 3600, 0.001, game="d"),
        sess(at(7), 3600, 10.0, game="e"),
    ]
    return case("gry_i_trening", s, now=at(8))


def long_history(seed: int, years: int, name: str) -> dict[str, Any]:
    rnd = random.Random(seed)
    games = ["skyrim", "wow", "gothic", "minecraft", "witcher", "stalker", "zelda"]
    s: list[Session] = []
    day = 0
    pause_until = -1
    for day in range(years * 365):
        if day < pause_until:
            continue
        if rnd.random() < 0.01:
            pause_until = day + rnd.randint(2, 20)  # urlop
            continue
        if rnd.random() < 0.15:
            continue
        tz = 120 if 90 <= day % 365 <= 300 else 60
        hour = rnd.randint(0, 22)
        minute = rnd.randint(0, 59)
        start = at(day, hour, minute, tz_min=tz)
        for _ in range(rnd.choice((1, 1, 1, 2, 2, 3))):
            km = round(rnd.uniform(0.3, 9.0), 3)
            kmh = rnd.uniform(2.5, 6.5)
            active = round(km / kmh * 3600)
            steps = round(km * 1000 / rnd.uniform(0.55, 0.95))
            if rnd.random() < 0.05:
                steps = 0  # bieżnia bez licznika kroków
            extra: dict[str, Any] = {"tz_offset_min": tz, "game": rnd.choice(games)}
            r = rnd.random()
            if r < 0.12:
                extra["mode"] = "trening"
            elif r < 0.14:
                extra["source"] = "symulator"
            elif r < 0.16:
                km = round(km * 6, 3)  # za szybko
            duration = active + rnd.randint(0, 600)
            s.append(sess(start, duration, km, steps, active=active, **extra))
            start += duration + rnd.randint(-120, 3600)  # czasem nakładka
    rnd.shuffle(s)  # kolejność wejścia nie ma znaczenia
    eb = {"znajomy": T0 + 3 * DAY, "km8047": at(years * 365 + 400)}
    return case(name, s, now=at(years * 365, 9), friends=2, earned_before=eb)


def level_100() -> dict[str, Any]:
    s: list[Session] = []
    for day in range(6 * 365):
        km = 6.5 if day % 50 else 12.0
        s.append(sess(at(day, 6, 30), round(km / 5.0 * 3600), km, game=f"g{day % 4}"))
    s.append(sess(at(6 * 365, 0), 10 * HOUR, 45.0, 60000))  # 10 h -> liczy się 8 h
    return case("poziom_100", s, now=at(6 * 365 + 1, 12))


def definitions(path: Path = poziomy.DEFINITIONS) -> dict[str, Any]:
    d = poziomy.load(path)
    xps = [0, 1, 49, 50, 51, 200, 399, 400, 960, 961, 962, 4996, 99_999, 947_099, 947_100]
    xps += [2_000_000, -5, 0.5, 960.999]
    return {
        "version": d.version,
        "xpPerKm": d.xp_per_km,
        "rarityXp": d.rarity_xp,
        "rarities": list(d.rarities),
        "levels": list(d.levels),
        "ranks": [{"level": r.level, "name": r.name, "about": r.about} for r in d.ranks],
        "achievements": [
            {
                "id": a.id,
                "kind": a.kind,
                "rarity": a.rarity,
                "mark": a.mark,
                "name": a.name,
                "flavor": a.flavor,
                "condition": a.condition,
                "params": a.params,
                "km": a.km,
            }
            for a in d.achievements
        ],
        "trail": [a.id for a in d.trail],
        "maxLevel": d.max_level,
        "levelOf": [[x, d.level_of(x)] for x in xps],
        "xpFor": [[n, d.xp_for(n)] for n in (-3, 0, 1, 2, 50, 100, 101, 500)],
        "rankFor": [[n, d.rank(n).name] for n in range(0, 102)],
        "nextRankFor": [
            [n, r.name if (r := d.next_rank(n)) else None] for n in range(0, 102)
        ],
    }


# --- inny plik definicji (np. nowsza wersja z serwera) --------------------------------------

OTHER = OUT / "inne"

OTHER_DEFINITIONS: dict[str, Any] = {
    "wersja": 3.0,  # liczby z kropką - Python czyta je przez int()
    "opis": "Definicje testowe",
    "xp_za_km": 37.0,
    "xp_za_rzadkosc": {"a": 7, "b": 13.0},  # "c" bez premii
    "rzadkosci": ["a", "b", "c"],
    "poziomy_xp": [0, 0, 50, 50, 200, 400.0],  # równe progi
    "rangi": [
        {"od_poziomu": 3, "nazwa": "Trzeci", "opis": "od 3"},  # pierwsza ranga nie od 1
        {"od_poziomu": 5.0, "nazwa": "Piąty", "opis": "od 5"},
    ],
    "nowe_pole": [1, 2],
    "osiagniecia": [
        {"id": "k1", "typ": "dystans", "km": 1, "rzadkosc": "a"},
        {"id": "k05", "typ": "dystans", "km": 0.5, "rzadkosc": "c"},
        {"id": "k3", "typ": "dystans", "km": 3.0, "rzadkosc": "b", "ikona": "nowa"},
        {"id": "s3dni", "typ": "seria", "dni": 2, "km_dziennie": 3.0, "rzadkosc": "b"},
        {"id": "s4dni", "typ": "seria", "dni": 4, "rzadkosc": "a"},  # domyślnie 2 km
        {"id": "gry2a", "typ": "gry_km", "gier": 2, "km": 1, "rzadkosc": "a"},
        {"id": "gry2b", "typ": "gry_km", "gier": 2, "km": 3, "rzadkosc": "b"},
        {"id": "gry3a", "typ": "gry_km", "gier": 3, "km": 1, "rzadkosc": "b"},  # ten sam próg km
        {"id": "jedna", "typ": "gra_km", "km": 2.5, "rzadkosc": "a"},
        {"id": "zawsze", "typ": "start_miedzy", "od": 5, "do": 5, "rzadkosc": "a"},
        {"id": "pozno", "typ": "start_miedzy", "od": 22.9, "do": 1.2, "rzadkosc": "c"},
        {"id": "kroki", "typ": "dzien_kroki", "kroki": 1000.5, "rzadkosc": "a"},
        {"id": "sesja2", "typ": "sesja_km", "km": 2, "rzadkosc": "b"},
        {"id": "znaj2", "typ": "znajomi", "ile": 2, "rzadkosc": "a"},
        {"id": "pol0", "typ": "polecenia", "ile": 0, "rzadkosc": "b"},  # zawsze zdobyte
        {"id": "nowe", "typ": "cos_nowego", "ile": 5, "rzadkosc": "a"},  # nieznany typ
        {"id": "tren", "typ": "trening", "rzadkosc": "c"},
        {"id": "pierwsza", "typ": "gra_pierwsza", "rzadkosc": "a"},
    ],
}
for _a in OTHER_DEFINITIONS["osiagniecia"]:
    _a.update(znak="s", nazwa=_a["id"].upper(), klimat="Klimat.", warunek="Warunek.")


def other_cases(path: Path) -> list[dict[str, Any]]:
    """Te same reguły z innym plikiem definicji, strefy z połówkami godzin."""
    out = [case("inne_pusta", [], now=T0, defs_path=path)]
    s = [
        sess(at(0, 22, 30, tz_min=330), 1800, 1.2, game="x", tz_offset_min=330),
        sess(at(0, 23, 50, tz_min=330), 1800, 2.0, game="y", tz_offset_min=330),
        sess(at(1, 0, 30, tz_min=-420), 1800, 1.0, game="x", tz_offset_min=-420),
        sess(at(2, 6), 1800, 0.4, mode="trening"),
        sess(at(3, 7), 3600, 3.1, game="z"),
        sess(at(5, 12), 3600, 3.5, game="y"),
    ]
    out.append(
        case(
            "inne_strefy",
            s,
            now=at(6, 1),
            friends=2,
            earned_before={"nowe": T0 + 3, "k3": at(9)},
            rest_per_week=2,
            defs_path=path,
        )
    )
    out.append(case("inne_max_poziom", [sess(at(0, 8), 4 * HOUR, 16.0)], now=at(1), defs_path=path))
    out.append(case("inne_rowny_prog", [sess(at(0, 8), 600, 0.01)], now=at(0, 9), defs_path=path))
    return out


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    cases: list[dict[str, Any]] = [
        simple(),
        streak_with_rest(),
        streak_broken(),
        streak_old(),
        *streak_rest_two(),  # type: ignore[misc]
        boundaries(),
        tenths(),
        day_cap(),
        *empty(),
        earned_before_min(),
        games_and_training(),
        long_history(42, 3, "dluga_3_lata"),
        long_history(7, 1, "dluga_rok"),
        level_100(),
    ]
    for c in cases:
        path = OUT / f"{c['name']}.json"
        indent = 1 if len(c["input"]["sessions"]) < 100 else None  # długie bez wcięć
        path.write_text(json.dumps(c, ensure_ascii=False, indent=indent) + "\n", encoding="utf-8")
    (OUT / "definicje.json").write_text(
        json.dumps(definitions(), ensure_ascii=False, indent=1) + "\n", encoding="utf-8"
    )
    OTHER.mkdir(parents=True, exist_ok=True)
    other = OTHER / "definicje.json"
    other.write_text(
        json.dumps(OTHER_DEFINITIONS, ensure_ascii=False, indent=1) + "\n", encoding="utf-8"
    )
    other_list = other_cases(other)
    for c in other_list:
        (OTHER / f"{c['name']}.json").write_text(
            json.dumps(c, ensure_ascii=False, indent=1) + "\n", encoding="utf-8"
        )
    (OTHER / "wynik_definicji.json").write_text(
        json.dumps(definitions(other), ensure_ascii=False, indent=1) + "\n", encoding="utf-8"
    )
    print(f"zapisano {len(cases) + len(other_list)} przypadków do {OUT}")


if __name__ == "__main__":
    main()
