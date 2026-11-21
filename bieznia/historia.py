"""Historia kroków: sesje osób i profili gier w lokalnej bazie SQLite (dane/historia.db).

Sesja trwa od pierwszego ruchu taśmy do 2 minut bez ruchu (albo zmiany osoby/profilu,
albo zamknięcia aplikacji). Kroki liczone z licznika bieżni (z obsługą jego zerowania),
dystans i czas ruchu - jak w kafelkach na ekranie głównym. Zapis co minutę.
"""

from __future__ import annotations

import logging
import sqlite3
import time
import uuid
from dataclasses import dataclass
from datetime import datetime, timedelta
from pathlib import Path
from typing import Any

log = logging.getLogger("bieznia")

DB_FILE = Path("dane") / "historia.db"
IDLE_END_S = 120.0
SAVE_EVERY_S = 60.0
MIN_SESSION_S = 10.0  # krótsze nie trafiają do historii (przypadkowy ruch taśmy)
DELETED_PERSON = "Usunięte osoby"

SCHEMA = """
CREATE TABLE IF NOT EXISTS sesje (
    id TEXT PRIMARY KEY,
    osoba TEXT NOT NULL,
    osoba_nazwa TEXT NOT NULL,
    profil TEXT NOT NULL,
    profil_nazwa TEXT NOT NULL,
    start REAL NOT NULL,
    koniec REAL NOT NULL,
    kroki INTEGER NOT NULL DEFAULT 0,
    dystans_km REAL NOT NULL DEFAULT 0,
    ruch_s REAL NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS sesje_osoba_start ON sesje (osoba, start);
"""


@dataclass
class Session:
    id: str
    person: str
    person_name: str
    profile: str
    profile_name: str
    start: float  # czas uniksowy
    end: float
    steps: int = 0
    distance_km: float = 0.0
    active_s: float = 0.0

    @property
    def avg_kmh(self) -> float:
        return self.distance_km / (self.active_s / 3600) if self.active_s > 0 else 0.0

    def row(self) -> tuple[Any, ...]:
        return (
            self.id,
            self.person,
            self.person_name,
            self.profile,
            self.profile_name,
            self.start,
            self.end,
            self.steps,
            self.distance_km,
            self.active_s,
        )

    @classmethod
    def from_row(cls, r: tuple[Any, ...]) -> Session:
        return cls(*r)


@dataclass(frozen=True)
class Totals:
    steps: int
    distance_km: float
    active_s: float

    @property
    def avg_kmh(self) -> float:
        return self.distance_km / (self.active_s / 3600) if self.active_s > 0 else 0.0


class History:
    def __init__(self, path: Path = DB_FILE) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        self.db = sqlite3.connect(str(path), check_same_thread=False)
        self.db.executescript(SCHEMA)
        self.db.execute("PRAGMA journal_mode=WAL")

    def close(self) -> None:
        self.db.close()

    def save(self, s: Session) -> None:
        with self.db:
            self.db.execute("INSERT OR REPLACE INTO sesje VALUES (?,?,?,?,?,?,?,?,?,?)", s.row())

    def merge(self, sessions: list[Session]) -> int:
        """Import: dodaje sesje, których jeszcze nie ma (po id). Zwraca liczbę nowych."""
        before = self.count()
        with self.db:
            self.db.executemany(
                "INSERT OR IGNORE INTO sesje VALUES (?,?,?,?,?,?,?,?,?,?)",
                [s.row() for s in sessions],
            )
        return self.count() - before

    def count(self) -> int:
        return int(self.db.execute("SELECT COUNT(*) FROM sesje").fetchone()[0])

    def sessions(
        self,
        since: float = 0.0,
        person: str | None = None,
        profiles: set[str] | None = None,
        limit: int | None = None,
    ) -> list[Session]:
        sql = "SELECT * FROM sesje WHERE start >= ?"
        args: list[Any] = [since]
        if person is not None:
            sql += " AND osoba = ?"
            args.append(person)
        sql += " ORDER BY start DESC"
        if limit:
            sql += f" LIMIT {int(limit)}"
        out = [Session.from_row(r) for r in self.db.execute(sql, args)]
        if profiles is not None:
            out = [s for s in out if s.profile in profiles]
        return out

    def totals(self, since: float = 0.0, person: str | None = None) -> Totals:
        ss = self.sessions(since, person)
        return Totals(
            sum(s.steps for s in ss), sum(s.distance_km for s in ss), sum(s.active_s for s in ss)
        )

    def by_day(
        self, days: int, person: str | None = None, key: str = "profile", now: float | None = None
    ) -> list[tuple[str, dict[str, int]]]:
        """Kroki dziennie z ostatnich `days` dni: [(dzień, {profil_lub_osoba: kroki})]."""
        today = datetime.fromtimestamp(now or time.time()).date()
        first = today - timedelta(days=days - 1)
        since = datetime.combine(first, datetime.min.time()).timestamp()
        out: dict[str, dict[str, int]] = {
            (first + timedelta(days=i)).isoformat(): {} for i in range(days)
        }
        for s in self.sessions(since, person):
            day = datetime.fromtimestamp(s.start).date().isoformat()
            name = s.profile_name if key == "profile" else s.person_name
            if day in out:
                out[day][name] = out[day].get(name, 0) + s.steps
        return list(out.items())

    def by_group(
        self, since: float, person: str | None = None, key: str = "profile"
    ) -> list[tuple[str, Totals]]:
        """[(nazwa, Totals)] posortowane od największej liczby kroków."""
        groups: dict[str, list[Session]] = {}
        for s in self.sessions(since, person):
            groups.setdefault(s.profile_name if key == "profile" else s.person_name, []).append(s)
        rows = [
            (
                name,
                Totals(
                    sum(s.steps for s in ss),
                    sum(s.distance_km for s in ss),
                    sum(s.active_s for s in ss),
                ),
            )
            for name, ss in groups.items()
        ]
        return sorted(rows, key=lambda r: -r[1].steps)

    def forget_person(self, person: str) -> None:
        """Usunięta osoba: jej wyniki zostają, podpisane jako „Usunięte osoby”."""
        with self.db:
            self.db.execute(
                "UPDATE sesje SET osoba_nazwa = ? WHERE osoba = ?", (DELETED_PERSON, person)
            )

    def clear(self, person: str | None = None) -> None:
        with self.db:
            if person is None:
                self.db.execute("DELETE FROM sesje")
            else:
                self.db.execute("DELETE FROM sesje WHERE osoba = ?", (person,))


class SessionTracker:
    """Liczy bieżącą sesję z odczytów aplikacji i zapisuje ją do historii."""

    def __init__(self, history: History, clock: Any = time.time) -> None:
        self.history = history
        self.clock = clock
        self.current: Session | None = None
        self._last_steps: int | None = None
        self._last_distance = 0.0
        self._last_active = 0.0
        self._last_move = 0.0
        self._saved_at = 0.0
        self.owner: tuple[str, str, str, str] = ("", "", "", "")  # osoba, nazwa, profil, nazwa

    def set_owner(self, person: str, person_name: str, profile: str, profile_name: str) -> None:
        """Zmiana osoby albo profilu kończy sesję poprzedniego."""
        new = (person, person_name, profile, profile_name)
        if new != self.owner:
            self.finish()
            self.owner = new

    def update(self, speed: float, steps: int | None, distance_km: float, active_s: float) -> None:
        """Wywoływane z pętli okna. distance_km i active_s to liczniki narastające aplikacji."""
        now = self.clock()
        d_dist = max(0.0, distance_km - self._last_distance)
        d_active = max(0.0, active_s - self._last_active)
        self._last_distance, self._last_active = distance_km, active_s
        d_steps = 0
        if steps is not None:
            if self._last_steps is not None:
                d_steps = steps - self._last_steps if steps >= self._last_steps else steps
            self._last_steps = steps
        moving = speed > 0.05 or d_steps > 0
        if moving:
            self._last_move = now
            if self.current is None:
                person, pname, profile, prname = self.owner
                self.current = Session(uuid.uuid4().hex, person, pname, profile, prname, now, now)
        if self.current is None:
            return
        self.current.steps += max(0, d_steps)
        self.current.distance_km += d_dist
        self.current.active_s += d_active
        self.current.end = now
        if not moving and now - self._last_move >= IDLE_END_S:
            self.finish()
        elif now - self._saved_at >= SAVE_EVERY_S:
            self._save()

    def _save(self) -> None:
        self._saved_at = self.clock()
        s = self.current
        if s is None or not s.person:
            return
        if s.end - s.start >= MIN_SESSION_S or s.steps > 0:
            self.history.save(s)

    def finish(self) -> None:
        if self.current is not None:
            self._save()
            log.info(
                "Sesja kroków: %s / %s - %d kroków, %.2f km, %.0f min",
                self.current.person_name,
                self.current.profile_name,
                self.current.steps,
                self.current.distance_km,
                self.current.active_s / 60,
            )
        self.current = None
