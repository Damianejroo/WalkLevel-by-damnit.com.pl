"""Historia kroków: sesje, zerowanie licznika bieżni, podział na osoby i profile."""

from __future__ import annotations

from datetime import datetime
from pathlib import Path

import pytest

from bieznia.historia import IDLE_END_S, History, Session, SessionTracker


@pytest.fixture
def history(tmp_path: Path) -> History:
    h = History(tmp_path / "historia.db")
    yield h
    h.close()


class Clock:
    def __init__(self, t: float) -> None:
        self.t = t

    def __call__(self) -> float:
        return self.t


def walk(
    tracker: SessionTracker, clock: Clock, seconds: int, start_steps: int, spm: int = 100
) -> int:
    """Symulacja chodu: odczyt co sekundę, licznik kroków bieżni rośnie."""
    steps = start_steps
    tracker.update(0.0, steps, getattr(tracker, "_dist", 0.0), getattr(tracker, "_act", 0.0))
    for _ in range(seconds):
        clock.t += 1
        steps += spm // 60 + (1 if clock.t % 3 == 0 else 0)
        tracker._dist = getattr(tracker, "_dist", 0.0) + 2.5 / 3600
        tracker._act = getattr(tracker, "_act", 0.0) + 1
        tracker.update(2.5, steps, tracker._dist, tracker._act)
    return steps


def test_session_counts_steps_and_ends_after_idle(history: History) -> None:
    clock = Clock(datetime(2026, 11, 21, 18, 0).timestamp())
    t = SessionTracker(history, clock)
    t.set_owner("p1", "Szalony Biegacz", "g1", "Cyberpunk 2077")
    end_steps = walk(t, clock, 300, start_steps=1000)
    assert t.current is not None and t.current.steps == end_steps - 1000
    for _ in range(int(IDLE_END_S) + 2):  # taśma stoi
        clock.t += 1
        t.update(0.0, end_steps, t._dist, t._act)
    assert t.current is None
    [s] = history.sessions()
    assert s.profile_name == "Cyberpunk 2077" and s.steps == end_steps - 1000
    assert s.distance_km == pytest.approx(300 * 2.5 / 3600)
    assert s.avg_kmh == pytest.approx(2.5)


def test_treadmill_counter_reset_does_not_lose_steps(history: History) -> None:
    clock = Clock(1_800_000_000.0)
    t = SessionTracker(history, clock)
    t.set_owner("p1", "A", "g1", "PEAK")
    walk(t, clock, 60, start_steps=5000)
    before = t.current.steps if t.current else 0
    walk(t, clock, 60, start_steps=0)  # bieżnia wyzerowała licznik (nowa sesja na bieżni)
    assert t.current is not None and t.current.steps > before
    assert t.current.steps < before * 2 + 10  # bez skoku o 5000


def test_switching_profile_or_person_splits_the_session(history: History) -> None:
    clock = Clock(1_800_000_000.0)
    t = SessionTracker(history, clock)
    t.set_owner("p1", "A", "g1", "PEAK")
    steps = walk(t, clock, 120, 0)
    t.set_owner("p1", "A", "g2", "Elden Ring")
    steps = walk(t, clock, 120, steps)
    t.set_owner("p2", "B", "g9", "Elden Ring")
    walk(t, clock, 120, steps)
    t.finish()
    names = sorted((s.person_name, s.profile_name) for s in history.sessions())
    assert names == [("A", "Elden Ring"), ("A", "PEAK"), ("B", "Elden Ring")]


def test_short_accidental_moves_are_not_saved(history: History) -> None:
    clock = Clock(1_800_000_000.0)
    t = SessionTracker(history, clock)
    t.set_owner("p1", "A", "g1", "PEAK")
    clock.t += 1
    t.update(0.5, None, 0.0001, 1)
    t.finish()
    assert history.count() == 0


def test_aggregations_by_day_and_group(history: History) -> None:
    day = datetime(2026, 11, 21, 18, 0).timestamp()
    rows = [
        Session("a", "p1", "A", "g1", "Cyberpunk 2077", day, day + 600, 1000, 0.5, 600),
        Session("b", "p1", "A", "g2", "PEAK", day - 86400, day - 86400 + 600, 400, 0.2, 300),
        Session("c", "p2", "B", "g3", "PEAK", day, day + 600, 700, 0.4, 500),
    ]
    for s in rows:
        history.save(s)
    days = history.by_day(7, now=day + 3600)
    assert len(days) == 7 and days[-1][1] == {"Cyberpunk 2077": 1000, "PEAK": 700}
    assert days[-2][1] == {"PEAK": 400}
    mine = history.by_group(0, person="p1")
    assert [name for name, _ in mine] == ["Cyberpunk 2077", "PEAK"]
    people = history.by_group(0, key="person")
    assert people[0][0] == "A" and people[0][1].steps == 1400
    assert history.totals(person="p2").steps == 700


def test_import_merge_skips_duplicates_and_deleted_person_keeps_results(history: History) -> None:
    s = Session("a", "p1", "A", "g1", "PEAK", 1.0, 2.0, 10, 0.1, 60)
    assert history.merge([s]) == 1
    assert history.merge([s, Session("b", "p1", "A", "g1", "PEAK", 3.0, 4.0, 5, 0.1, 60)]) == 1
    history.forget_person("p1")
    assert {x.person_name for x in history.sessions()} == {"Usunięte osoby"}
    history.clear("p1")
    assert history.count() == 0


def test_today_by_hour_splits_long_sessions(history: History) -> None:
    from datetime import datetime

    day = datetime(2026, 11, 25)
    start = datetime(2026, 11, 25, 18, 30).timestamp()
    history.save(Session("h1", "p", "P", "g", "Gra", start, start + 3600, 1200, 1.0, 3600))
    hours = dict(history.by_hour(now=datetime(2026, 11, 25, 21, 0).timestamp()))
    assert len(hours) == 24 and day.day == 25
    assert hours["18"] == {"Gra": 600} and hours["19"] == {"Gra": 600}
    assert hours["20"] == {}
