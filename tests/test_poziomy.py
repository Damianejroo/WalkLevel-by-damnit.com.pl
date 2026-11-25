"""Poziomy i osiągnięcia: definicje, liczenie z historii, migracja starej bazy."""

from __future__ import annotations

import itertools
import sqlite3
from pathlib import Path

from bieznia import poziomy
from bieznia.historia import SCHEMA_VERSION, History, Session
from bieznia.poziomy import Walk, evaluate

DAY = 86400.0
T0 = 1_767_225_600.0  # 2026-01-01 00:00 UTC


def defs() -> poziomy.Definitions:
    return poziomy.load()


def walk(
    day: int, km: float, steps: int = 0, game: str = "g1", mode: str = "gra", hour: int = 18
) -> Walk:
    start = T0 + day * DAY + hour * 3600
    return Walk(start, start + 3600, km, steps, game, mode, 0)


def test_definitions_are_consistent() -> None:
    d = defs()
    assert d.max_level == 100
    assert list(d.levels) == sorted(d.levels) and d.levels[0] == 0
    assert len(set(d.levels)) == 100  # każdy poziom wymaga więcej XP
    ranks = [r.level for r in d.ranks]
    assert ranks == sorted(set(ranks)) and ranks[0] == 1 and ranks[-1] == 100
    ids = [a.id for a in d.achievements]
    assert len(ids) == len(set(ids))
    kinds = {
        "dystans",
        "sesja_km",
        "dzien_kroki",
        "seria",
        "gra_pierwsza",
        "gry_km",
        "gra_km",
        "trening",
        "start_miedzy",
        "znajomi",
        "polecenia",
    }
    for a in d.achievements:
        assert a.kind in kinds, a.id
        assert a.rarity in d.rarities, a.id
        assert a.name and a.flavor and a.condition
        assert "eś " not in a.flavor and not a.flavor.endswith("eś.")  # bezosobowo
    trail = [a.km for a in d.trail]
    assert trail == sorted(trail)
    gaps = [b - a for a, b in itertools.pairwise(trail) if b <= 2865]
    assert max(gaps) <= 152  # do Mordoru przystanek co ok. 30 dni przy 5 km dziennie


def test_level_100_after_about_five_years_at_5_km_a_day() -> None:
    d = defs()
    xp_km_only = 5 * 365 * 5 * d.xp_per_km
    assert 0.85 * d.xp_for(100) < xp_km_only < 1.1 * d.xp_for(100)


def test_distance_thresholds_and_time_of_unlock() -> None:
    d = defs()
    walks = [walk(0, 1.5), walk(1, 4.5), walk(2, 0.3)]
    s = evaluate(d, walks, now=T0 + 3 * DAY)
    assert s.progress["km1"].earned_at == walks[0].end
    assert s.progress["km2"].earned_at == walks[1].end  # 1,5 + 4,5 = 6 km
    assert s.progress["km57"].earned_at == walks[1].end
    assert not s.progress["km113"].earned
    assert abs(s.km - 6.3) < 1e-9
    bonus = sum(d.rarity_xp[p.achievement.rarity] for p in s.progress.values() if p.earned)
    assert s.xp == int(6.3 * 100) + bonus
    assert s.level == d.level_of(s.xp)


def test_simulator_sessions_do_not_count() -> None:
    real = Session("a", "p", "P", "g", "G", T0, T0 + 1800, 4000, 3.0, 1800)
    sim = Session("b", "p", "P", "g", "G", T0, T0 + 600, 1000, 30.0, 600, source="symulator")
    walks = poziomy.walks_from_sessions([real, sim])
    assert [w.km for w in walks] == [3.0]


def test_streak_allows_one_rest_day_in_seven() -> None:
    d = defs()
    # dni 0-7 z przerwą w dniu 3: seria 8 dni kalendarzowych
    walks = [walk(i, 2.5) for i in range(8) if i != 3]
    s = evaluate(d, walks, now=T0 + 7 * DAY + 3600)
    assert s.progress["seria3"].earned
    assert s.progress["seria7"].earned
    assert s.progress["seria7"].current == 8


def test_two_rest_days_in_a_row_break_the_streak() -> None:
    d = defs()
    walks = [walk(i, 2.5) for i in (0, 1, 2, 5, 6, 7, 8)]
    s = evaluate(d, walks, now=T0 + 8 * DAY + 3600)
    assert not s.progress["seria7"].earned
    assert s.progress["seria3"].earned
    assert s.progress["seria7"].current == 4  # od dnia 5


def test_short_day_does_not_count_and_old_streak_is_not_current() -> None:
    d = defs()
    walks = [walk(i, 2.5) for i in range(4)] + [walk(4, 1.0)]
    s = evaluate(d, walks, now=T0 + 20 * DAY)
    assert s.progress["seria3"].earned
    assert s.progress["seria7"].current == 0  # seria dawno przerwana


def test_daily_steps_session_distance_and_time_windows() -> None:
    d = defs()
    walks = [
        walk(0, 3.2, steps=4000, hour=5),
        walk(0, 1.0, steps=3500, hour=20),
        walk(1, 1.0, steps=100, hour=23),
    ]
    s = evaluate(d, walks, now=T0 + 2 * DAY)
    assert s.progress["d7"].earned_at == walks[1].end  # 7 500 kroków w dniu 0
    assert s.progress["s3"].earned_at == walks[0].end
    assert s.progress["rano"].earned_at == walks[0].end
    assert s.progress["noc"].earned_at == walks[2].end


def test_games_variety_and_training() -> None:
    d = defs()
    walks = [
        walk(0, 10, game="a"),
        walk(1, 10, game="b"),
        walk(2, 9, game="c"),
        walk(3, 5, mode="trening", game="c"),
    ]
    s = evaluate(d, walks, now=T0 + 4 * DAY)
    assert s.progress["gra1"].earned_at == walks[0].end
    assert not s.progress["gry3"].earned  # trening nie liczy się do gry
    assert s.progress["gry3"].current == 2
    assert s.progress["trening"].earned_at == walks[3].end
    s2 = evaluate(d, [*walks, walk(4, 1, game="c")], now=T0 + 5 * DAY)
    assert s2.progress["gry3"].earned


def test_earned_achievements_are_never_taken_away() -> None:
    d = defs()
    s = evaluate(d, [], earned_before={"km1": T0}, now=T0)
    assert s.progress["km1"].earned_at == T0
    assert s.xp == d.rarity_xp[d.by_id("km1").rarity]  # type: ignore[union-attr]


def test_ranks_cover_levels() -> None:
    d = defs()
    assert d.rank(1).name == d.ranks[0].name
    assert d.rank(100).name == d.ranks[-1].name
    assert d.next_rank(100) is None
    assert d.next_rank(1) == d.ranks[1]


def test_old_database_from_0_3_keeps_sessions_and_gets_achievements(tmp_path: Path) -> None:
    """Aktualizacja z 0.3.x: wszystkie sesje zostają, liczą się do osiągnięć i idą na konto."""
    path = tmp_path / "historia.db"
    db = sqlite3.connect(path)
    db.execute(
        "CREATE TABLE sesje (id TEXT PRIMARY KEY, osoba TEXT NOT NULL, osoba_nazwa TEXT NOT NULL,"
        " profil TEXT NOT NULL, profil_nazwa TEXT NOT NULL, start REAL NOT NULL,"
        " koniec REAL NOT NULL, kroki INTEGER NOT NULL DEFAULT 0,"
        " dystans_km REAL NOT NULL DEFAULT 0, ruch_s REAL NOT NULL DEFAULT 0)"
    )
    for i in range(3):
        db.execute(
            "INSERT INTO sesje VALUES (?, 'olek', 'Olek', 'p1', 'Gra', ?, ?, 5000, 4.0, 3000)",
            (f"s{i}", T0 + i * DAY, T0 + i * DAY + 3000),
        )
    db.commit()
    db.close()

    h = History(path)
    try:
        assert h.schema_version() == SCHEMA_VERSION
        sessions = h.sessions(person="olek")
        assert len(sessions) == 3
        assert all(s.mode == "gra" for s in sessions)
        s = evaluate(defs(), poziomy.walks_from_sessions(sessions), now=T0 + 3 * DAY)
        assert s.progress["km113"].earned  # 12 km z historii
        h.add_earned("olek", {p.achievement.id: p.earned_at or 0 for p in s.earned}, 1)
        assert set(h.earned("olek")) == {p.achievement.id for p in s.earned}
        assert len(h.unsent("olek")) == 3  # po zalogowaniu cała historia trafi na konto
        assert h.shown_level("olek") is None
        h.set_shown_level("olek", 4)
        assert h.shown_level("olek") == 4
    finally:
        h.close()


def test_trail_rarity_only_grows_with_distance() -> None:
    d = defs()
    tiers = [d.rarities.index(a.rarity) for a in d.trail]
    assert tiers == sorted(tiers)


def sess(
    i: int, start: float, end: float, steps: int, km: float, active: float, **kw: object
) -> Session:
    return Session(f"s{i}", "p", "P", "g", "G", start, end, steps, km, active, **kw)  # type: ignore[arg-type]


def test_unreliable_sessions_do_not_count_like_on_the_server() -> None:
    ok = sess(1, T0, T0 + 3600, 6000, 4.5, 3600)
    too_fast = sess(2, T0 + 4000, T0 + 7600, 30000, 25.0, 3600)  # 25 km/h
    too_many_steps = sess(3, T0 + 8000, T0 + 8600, 3000, 2.0, 600)  # 300 kroków na minutę
    weird_step = sess(4, T0 + 9000, T0 + 12600, 1000, 4.0, 3600)  # krok 4 m
    no_motion = sess(5, T0 + 13000, T0 + 13600, 0, 0.0, 0)
    no_steps = sess(6, T0 + 14000, T0 + 17600, 0, 4.0, 3600)  # bieżnia bez licznika kroków
    walks = poziomy.walks_from_sessions(
        [ok, too_fast, too_many_steps, weird_step, no_motion, no_steps]
    )
    assert [w.km for w in walks] == [4.5, 4.0]


def test_overlapping_session_is_skipped_and_day_is_capped_at_8_hours() -> None:
    first = sess(1, T0, T0 + 3 * 3600, 20000, 15.0, 3 * 3600)
    overlap = sess(2, T0 + 3600, T0 + 2 * 3600, 6000, 4.0, 3600)
    later = sess(3, T0 + 4 * 3600, T0 + 10 * 3600, 40000, 30.0, 6 * 3600)  # razem 9 h ruchu
    walks = poziomy.walks_from_sessions([first, overlap, later])
    assert len(walks) == 2
    assert walks[0].km == 15.0
    assert abs(walks[1].km - 25.0) < 1e-9  # z 6 h liczy się 5 h
