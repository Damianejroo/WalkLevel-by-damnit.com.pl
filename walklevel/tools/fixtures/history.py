"""Fixtury modułu history: bazy historia.db z prawdziwych wersji Pythona i oczekiwane wyniki.

Uruchamiać z katalogu głównego repozytorium (PYTHONPATH=.):
    .venv\\Scripts\\python.exe walklevel\\tools\\fixtures\\history.py

Stare bazy (schemat 0, 2, 3, 4) tworzy kod historia.py z commitów tych wersji (git show),
a oczekiwany stan po migracji liczy bieżący bieznia.historia na kopii każdej bazy.
Baza v5 z danymi i wynikami zapytań (dni, godziny, grupy, wysyłka) - bieżący kod.
Daty w listopadzie 2026 (bez zmiany czasu w oknie 30 dni) - test Rust używa zapisanej strefy.
tracker.json: zdarzenia SessionTracker i stan po każdym z nich; sums.json: sum() co do bitu.

Tylko nowe fixtury (bez przebudowy baz): ... history.py tracker
"""

from __future__ import annotations

import json
import random
import shutil
import struct
import sqlite3
import subprocess
import sys
import tempfile
import types
from datetime import datetime
from pathlib import Path
from typing import Any

from bieznia import historia as cur

OUT = Path("walklevel/crates/core/tests/fixtures/history")

# commit -> numer schematu, który ta wersja zapisywała
OLD = {
    "v0": "613751e",  # 0.3.0: dziesięć kolumn, start / koniec, bez user_version
    "v2": "d05c36e",  # bieżnie jako moduły: kolumny *_utc_s, źródło sesji
    "v3": "31fad04",  # konto online: wyslane, ukryte_online
    "v4": "e66ac74",  # 0.5.0: tryb, osiagniecia, poziom_osoby
}

CAMEL = {
    "id": "id",
    "person": "person",
    "person_name": "personName",
    "profile": "profile",
    "profile_name": "profileName",
    "start": "start",
    "end": "end",
    "steps": "steps",
    "distance_km": "distanceKm",
    "active_s": "activeS",
    "device_model": "deviceModel",
    "driver": "driver",
    "device_profile": "deviceProfile",
    "firmware": "firmware",
    "app_version": "appVersion",
    "source": "source",
    "visibility": "visibility",
    "tz_offset_min": "tzOffsetMin",
    "updated": "updated",
    "mode": "mode",
}


def load_old(commit: str) -> types.ModuleType:
    code = subprocess.run(
        ["git", "show", f"{commit}:bieznia/historia.py"],
        check=True,
        capture_output=True,
    ).stdout.decode("utf-8")
    name = f"historia_{commit}"
    mod = types.ModuleType(name)
    sys.modules[name] = mod
    exec(compile(code, name, "exec"), mod.__dict__)
    return mod


def ts(*args: int) -> float:
    return datetime(*args).timestamp()


def sess(s: cur.Session) -> dict[str, Any]:
    return {CAMEL[k]: v for k, v in s.__dict__.items()}


def totals(t: cur.Totals) -> dict[str, Any]:
    return {"steps": t.steps, "distanceKm": t.distance_km, "activeS": t.active_s}


def table_info(db: sqlite3.Connection) -> dict[str, list[list[Any]]]:
    names = [
        r[0]
        for r in db.execute("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
    ]
    return {
        n: [[r[1], r[2], r[3], r[4], r[5]] for r in db.execute(f"PRAGMA table_info({n})")]
        for n in names
    }


def dump_tables(db: sqlite3.Connection) -> dict[str, Any]:
    return {
        "wyslane": [list(r) for r in db.execute("SELECT * FROM wyslane ORDER BY id")],
        "odrzucone": [list(r) for r in db.execute("SELECT * FROM odrzucone ORDER BY id")],
        "ukryte_online": [r[0] for r in db.execute("SELECT id FROM ukryte_online ORDER BY id")],
        "osiagniecia": [
            list(r) for r in db.execute("SELECT * FROM osiagniecia ORDER BY osoba, id")
        ],
        "poziom_osoby": [list(r) for r in db.execute("SELECT * FROM poziom_osoby ORDER BY osoba")],
    }


def make_old(key: str, commit: str, path: Path) -> None:
    m = load_old(commit)
    h = m.History(path)
    base = ts(2026, 11, 20, 18, 0)
    if key == "v0":
        rows = [
            m.Session("a0", "p1", "Ala", "g1", "PEAK", base, base + 600, 1000, 0.5, 600.0),
            m.Session("b0", "p1", "Ala", "g2", "Cyberpunk 2077", base + 900, base + 1500, 7, 0.0, 9),
            m.Session("c0", "p2", "Bartek", "g3", "Żółć ąęś", base - 86400, base - 86000, 400, 0.25, 380),
        ]
        for s in rows:
            h.save(s)
    else:
        src = m.SessionSource("KS-F0", "kingsmith", "walkingpad", "1.2.3", "0.4.0", "bieznia")
        sim = m.SessionSource("", "", "", "", "0.4.0", "symulator")
        a = m.Session("a" + key, "p1", "Ala", "g1", "PEAK", base, base + 600, 1000, 0.5, 600.0)
        a.with_source(src)
        a.tz_offset_min = 60
        b = m.Session("b" + key, "p1", "Ala", "g2", "Elden Ring", base + 900, base + 1500, 300, 0.2, 500.0)
        b.with_source(sim)
        b.visibility = "znajomi"
        c = m.Session("c" + key, "p2", "Bartek", "g3", "PEAK", base - 86400, base - 86000, 400, 0.25, 380.0)
        if key == "v4":
            c.mode = "trening"
        for s in (a, b, c):
            h.save(s)
        h.db.execute("UPDATE sesje SET zmieniono_utc_s = koniec_utc_s + 5")
        h.db.commit()
        if key in ("v3", "v4"):
            [sent] = [s for s in h.sessions() if s.id == "a" + key]
            h.mark_sent([sent])
            h.set_hidden_online("b" + key, True)
        if key == "v4":
            h.add_earned("p1", {"pierwsze-kroki": base + 10, "1000-krokow": base + 20}, 1)
            h.set_shown_level("p1", 3)
    h.close()


def migrated(path: Path) -> dict[str, Any]:
    with tempfile.TemporaryDirectory() as tmp:
        copy = Path(tmp) / "historia.db"
        shutil.copy(path, copy)
        h = cur.History(copy)
        out = {
            "schemaVersion": h.schema_version(),
            "sessions": [sess(s) for s in h.sessions()],
            "tables": table_info(h.db),
            "data": dump_tables(h.db),
            "version": list(h.version()),
        }
        h.close()
    return out


def make_v5(path: Path) -> dict[str, Any]:
    h = cur.History(path)
    day = ts(2026, 11, 25, 18, 30)
    S = cur.Session
    rows = [
        # dziś, przez dwie godziny 18:30-19:30
        S("s1", "p1", "Ala", "g1", "Gra", day, day + 3600, 1200, 1.0, 3600.0),
        # dziś krótka, kroki nieparzyste w połowie godziny (zaokrąglanie do parzystej)
        S("s2", "p1", "Ala", "g2", "PEAK", ts(2026, 11, 25, 9, 45), ts(2026, 11, 25, 10, 15), 5, 0.01, 1800.0),
        S("s3", "p1", "Ala", "g2", "PEAK", ts(2026, 11, 25, 11, 45), ts(2026, 11, 25, 12, 15), 3, 0.01, 1800.0),
        # przez północ (wczoraj -> dziś)
        S("s4", "p2", "Bartek", "g3", "Elden Ring", ts(2026, 11, 24, 23, 30), ts(2026, 11, 25, 0, 30), 1001, 0.7, 3500.0),
        # krótsza niż sekunda
        S("s5", "p2", "Bartek", "g3", "Elden Ring", ts(2026, 11, 25, 7, 10), ts(2026, 11, 25, 7, 10) + 0.4, 9, 0.0, 0.4),
        # starsze dni
        S("s6", "p1", "Ala", "g1", "Gra", ts(2026, 11, 20, 8, 0), ts(2026, 11, 20, 9, 0), 2500, 1.9, 3600.0),
        S("s7", "p1", "Ala", "g1", "Gra", ts(2026, 11, 1, 8, 0), ts(2026, 11, 1, 9, 0), 777, 0.6, 1800.0),
        S("s8", "p2", "Bartek", "g4", "PEAK", ts(2026, 10, 28, 8, 0), ts(2026, 10, 28, 8, 30), 333, 0.3, 1700.0),
        S("s9", "p1", "Ala", "g2", "PEAK", ts(2026, 9, 1, 8, 0), ts(2026, 9, 1, 8, 30), 100, 0.1, 600.0),
        # symulator (nie idzie na konto), trening
        S("s10", "p1", "Ala", "g1", "Gra", ts(2026, 11, 23, 8, 0), ts(2026, 11, 23, 8, 30), 50, 0.1, 900.0),
        S("s11", "p1", "Ala", "", "Trening", ts(2026, 11, 22, 8, 0), ts(2026, 11, 22, 8, 40), 1500, 1.2, 2400.0),
    ]
    for i, s in enumerate(rows):
        s.updated = s.end + 1
        s.tz_offset_min = 60
        s.app_version = "0.5.1"
        s.device_model, s.driver = "KS-F0", "kingsmith"
        if i == 9:
            s.source = "symulator"
        if i == 10:
            s.mode = "trening"
            s.visibility = "publiczna"
    h.merge(rows)
    by_id = {s.id: s for s in h.sessions()}
    h.mark_sent([by_id["s1"], by_id["s6"]])
    h.mark_rejected([(by_id["s7"], "Za szybko " * 30)], retry_at=day + 7200)
    h.mark_rejected([(by_id["s9"], "zła data")], retry_at=day - 100)
    h.set_hidden_online("s4", True)
    h.set_hidden_online("s5", True)
    h.set_hidden_online("s5", False)
    h.add_earned("p1", {"pierwsze-kroki": day + 1, "maraton": day + 2}, 2)
    h.add_earned("p1", {"pierwsze-kroki": day + 99}, 3)  # już zdobyte - bez zmian
    h.set_shown_level("p1", 4)
    h.set_shown_level("p2", 1)
    h.set_shown_level("p2", 2)

    now = ts(2026, 11, 25, 21, 0)
    since_week = ts(2026, 11, 19, 0, 0)
    out: dict[str, Any] = {
        "now": now,
        "tzOffsetMin": [cur.utc_offset_min(t) for t in (now, ts(2026, 10, 27, 0, 0), ts(2026, 10, 26, 0, 0))],
        "schemaVersion": h.schema_version(),
        "count": h.count(),
        "version": list(h.version()),
        "schemaSql": [
            r[0]
            for r in h.db.execute(
                "SELECT sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY name"
            )
        ],
        "tables": table_info(h.db),
        "data": dump_tables(h.db),
        "rejected": h.rejected(),
        "hiddenOnline": sorted(h.hidden_online()),
        "earned": {p: h.earned(p) for p in ("p1", "p2")},
        "shownLevel": {p: h.shown_level(p) for p in ("p1", "p2", "p3")},
        "wasSent": {i: h.was_sent(i) for i in ("s1", "s2", "s6")},
        "unsent": {
            "p1_now": [s.id for s in h.unsent("p1", now)],
            "p1_later": [s.id for s in h.unsent("p1", day + 7200)],
            "p2_now": [s.id for s in h.unsent("p2", now)],
        },
        "sessions": {
            "all": [sess(s) for s in h.sessions()],
            "week_p1": [s.id for s in h.sessions(since_week, "p1")],
            "limit3": [s.id for s in h.sessions(limit=3)],
            "profiles_g2": [s.id for s in h.sessions(profiles={"g2", "g9"})],
            "profiles_limit": [s.id for s in h.sessions(0.0, "p1", {"g1"}, 4)],
        },
        "totals": {
            "all": totals(h.totals()),
            "p1": totals(h.totals(person="p1")),
            "week_p2": totals(h.totals(since_week, "p2")),
        },
        "byDay": {
            "7_profile": h.by_day(7, now=now),
            "7_person": h.by_day(7, key="person", now=now),
            "30_p1": h.by_day(30, person="p1", now=now),
            "1_p2": h.by_day(1, person="p2", now=now),
        },
        "byHour": {
            "profile": h.by_hour(now=now),
            "person": h.by_hour(key="person", now=now),
            "p2": h.by_hour(person="p2", now=now),
        },
        "byGroup": {
            "all_profile": [[n, totals(t)] for n, t in h.by_group(0)],
            "all_person": [[n, totals(t)] for n, t in h.by_group(0, key="person")],
            "week_p1": [[n, totals(t)] for n, t in h.by_group(since_week, "p1")],
        },
    }
    h.close()
    return out


def bits(x: float) -> int:
    return struct.unpack("<Q", struct.pack("<d", float(x)))[0]


def make_sums() -> dict[str, Any]:
    """sum() z Pythona 3.12+ (Neumaier) - wejście i wynik jako bity f64, porównanie dokładne."""
    rng = random.Random(7)
    cases = [[0.1] * 10, [1e16, 1.0, -1e16], [], [2.5 / 3600] * 777]
    for _ in range(120):
        cases.append(
            [rng.uniform(0, 5) * 10 ** rng.randint(-3, 4) for _ in range(rng.randint(1, 40))]
        )
    return {
        "python": sys.version.split()[0],
        "cases": [{"in": [bits(x) for x in c], "sum": bits(sum(c))} for c in cases],
    }


def tracker_events() -> list[dict[str, Any]]:
    """Scenariusz: bez osoby, chód z zerowaniem licznika, bez licznika, próg 0.05 km/h,
    przypadkowy ruch, firmware później, symulator, trening, zmiana osoby, flush."""
    t = ts(2026, 11, 21, 18, 0) + 0.25
    ev: list[dict[str, Any]] = []
    dist = act = 0.0

    def upd(speed: float, steps: int | None, dt: float = 1.0) -> None:
        nonlocal t, dist, act
        t += dt
        if speed > 0:
            dist += speed / 3600 * dt
            act += dt
        ev.append({"op": "update", "t": t, "speed": speed, "steps": steps, "dist": dist, "act": act})

    def op(name: str, **kw: Any) -> None:
        ev.append({"op": name, "t": t, **kw})

    for i in range(15):  # bez osoby - nic nie trafia do historii
        upd(3.0, 100 + i)
    op("finish")
    op("owner", args=["p1", "Ala", "g1", "PEAK"])
    ks = {"model": "KS-F0", "driver": "kingsmith", "profile": "walkingpad", "firmware": "",
          "app_version": "0.5.2", "source": "bieznia"}
    op("source", src=ks)
    steps = 1000
    for i in range(90):
        steps += 1 + (i % 3 == 0)
        upd(2.7, steps)
    op("source", src={**ks, "firmware": "1.4.2"})  # firmware odczytany później
    for i in range(20):  # taśma stoi, ale licznik rośnie (ruch z kroków)
        steps += 1
        upd(0.0, steps)
    for _ in range(10):
        upd(0.05, steps)  # próg: 0.05 to jeszcze nie ruch
    steps = 3  # bieżnia wyzerowała licznik
    upd(2.0, steps)
    for i in range(40):
        steps += 2
        upd(2.0, steps, 1.5)
    for _ in range(30):
        upd(3.1, None)  # bieżnia bez licznika kroków
    for _ in range(125):
        upd(0.0, None)  # koniec po IDLE_END_S
    for _ in range(3):
        upd(0.6, None)  # przypadkowy ruch - za krótko i bez kroków
    for _ in range(122):
        upd(0.0, None)
    op("source", src={"model": "", "driver": "", "profile": "", "firmware": "",
                      "app_version": "0.5.2", "source": "symulator"})
    for i in range(70):
        upd(4.0, 10 + i)
    op("mode", mode="trening")
    for i in range(30):
        upd(4.0, 80 + i)
    op("flush")
    op("owner", args=["p2", "Bartek", "", "Trening"])
    for i in range(65):
        upd(5.0, 200 + i * 2)
    op("finish")
    return ev


def snapshot(tr: cur.SessionTracker, h: cur.History) -> list[Any]:
    """[zapisane sesje, kroki, km, ruch_s, start, koniec, firmware] (bez sesji - same zapisane)."""
    s = tr.current
    if s is None:
        return [h.count()]
    return [h.count(), s.steps, s.distance_km, s.active_s, s.start, s.end, s.firmware]


def make_tracker() -> dict[str, Any]:
    events = tracker_events()
    clock = {"t": events[0]["t"]}
    with tempfile.TemporaryDirectory() as tmp:
        h = cur.History(Path(tmp) / "historia.db")
        tr = cur.SessionTracker(h, lambda: clock["t"])
        states = []
        for e in events:
            clock["t"] = e["t"]
            if e["op"] == "update":
                tr.update(e["speed"], e["steps"], e["dist"], e["act"])
            elif e["op"] == "owner":
                tr.set_owner(*e["args"])
            elif e["op"] == "source":
                tr.set_source(cur.SessionSource(**{
                    "model": e["src"]["model"], "driver": e["src"]["driver"],
                    "profile": e["src"]["profile"], "firmware": e["src"]["firmware"],
                    "app_version": e["src"]["app_version"], "source": e["src"]["source"],
                }))
            elif e["op"] == "mode":
                tr.set_mode(e["mode"])
            elif e["op"] == "flush":
                tr.flush()
            elif e["op"] == "finish":
                tr.finish()
            states.append(snapshot(tr, h))
        saved = [
            {k: v for k, v in sess(s).items() if k not in ("id", "updated", "tzOffsetMin")}
            for s in sorted(h.sessions(), key=lambda s: s.start)
        ]
        h.close()
    return {"events": events, "states": states, "sessions": saved}


def write_json(name: str, data: Any) -> None:
    text = json.dumps(data, ensure_ascii=False, separators=(",", ":"))
    (OUT / name).write_text(text, encoding="utf-8")


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    write_json("tracker.json", make_tracker())
    write_json("sums.json", make_sums())
    if sys.argv[1:] == ["tracker"]:
        print("fixtury tracker i sums zapisane w", OUT)
        return
    expected: dict[str, Any] = {}
    for key, commit in OLD.items():
        db = OUT / f"{key}.db"
        db.unlink(missing_ok=True)
        make_old(key, commit, db)
        expected[key] = migrated(db)
    (OUT / "migrated.json").write_text(
        json.dumps(expected, ensure_ascii=False, indent=1), encoding="utf-8"
    )
    db = OUT / "v5.db"
    db.unlink(missing_ok=True)
    v5 = make_v5(db)
    (OUT / "v5.json").write_text(json.dumps(v5, ensure_ascii=False, indent=1), encoding="utf-8")
    for extra in OUT.glob("*.db-*"):
        extra.unlink()
    print("fixtury history zapisane w", OUT)


if __name__ == "__main__":
    main()
