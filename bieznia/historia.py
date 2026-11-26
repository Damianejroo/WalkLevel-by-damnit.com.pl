"""Historia kroków: sesje osób i profili gier w lokalnej bazie SQLite (dane/historia.db).

Sesja trwa od pierwszego ruchu taśmy do 2 minut bez ruchu (albo zmiany osoby/profilu,
albo zamknięcia aplikacji). Kroki liczone z licznika bieżni (z obsługą jego zerowania),
dystans i czas ruchu - jak w kafelkach na ekranie głównym. Zapis co minutę.

Pod przyszłą synchronizację (etap 2): id to UUIDv7 (sortuje się po czasie, ponowna wysyłka
nie dubluje), czasy w sekundach UTC (`*_utc_s`), jednostki w nazwach kolumn, wersja schematu
w PRAGMA user_version (migracje poniżej). Przy sesji: model bieżni, sterownik, firmware,
wersja aplikacji, źródło (bieżnia / symulator) i widoczność - domyślnie prywatna.
"""

from __future__ import annotations

import logging
import os
import sqlite3
import time
from dataclasses import dataclass, fields
from datetime import datetime, timedelta
from pathlib import Path
from typing import Any

log = logging.getLogger("bieznia")

DB_FILE = Path("dane") / "historia.db"
IDLE_END_S = 120.0
SAVE_EVERY_S = 60.0
MIN_SESSION_S = 10.0  # krótsze nie trafiają do historii (przypadkowy ruch taśmy)
DELETED_PERSON = "Usunięte osoby"

SCHEMA_VERSION = 5  # 3: wysłane i ukryte online; 4: tryb, osiągnięcia, poziom; 5: odrzucone
VISIBILITY = ("prywatna", "znajomi", "publiczna")
SOURCES = ("bieznia", "symulator")

# Kolumny w kolejności pól Session (nazwa kolumny, definicja). Wersja 1 (0.3.x) miała
# pierwsze dziesięć, a czasy nazywały się start / koniec.
COLUMNS: tuple[tuple[str, str], ...] = (
    ("id", "TEXT PRIMARY KEY"),
    ("osoba", "TEXT NOT NULL"),
    ("osoba_nazwa", "TEXT NOT NULL"),
    ("profil", "TEXT NOT NULL"),
    ("profil_nazwa", "TEXT NOT NULL"),
    ("start_utc_s", "REAL NOT NULL"),
    ("koniec_utc_s", "REAL NOT NULL"),
    ("kroki", "INTEGER NOT NULL DEFAULT 0"),
    ("dystans_km", "REAL NOT NULL DEFAULT 0"),
    ("ruch_s", "REAL NOT NULL DEFAULT 0"),
    ("bieznia_model", "TEXT NOT NULL DEFAULT ''"),
    ("sterownik", "TEXT NOT NULL DEFAULT ''"),
    ("bieznia_profil", "TEXT NOT NULL DEFAULT ''"),
    ("firmware", "TEXT NOT NULL DEFAULT ''"),
    ("wersja_aplikacji", "TEXT NOT NULL DEFAULT ''"),
    ("zrodlo", "TEXT NOT NULL DEFAULT 'bieznia'"),
    ("widocznosc", "TEXT NOT NULL DEFAULT 'prywatna'"),
    ("strefa_min", "INTEGER NOT NULL DEFAULT 0"),
    ("zmieniono_utc_s", "REAL NOT NULL DEFAULT 0"),
    ("tryb", "TEXT NOT NULL DEFAULT 'gra'"),  # gra albo trening (tryb Trening, bez gry)
)
COLUMN_NAMES = tuple(name for name, _ in COLUMNS)
_SELECT = f"SELECT {', '.join(COLUMN_NAMES)} FROM sesje"
_PLACEHOLDERS = ",".join("?" * len(COLUMNS))
_INSERT = f"INTO sesje ({', '.join(COLUMN_NAMES)}) VALUES ({_PLACEHOLDERS})"

SCHEMA = f"""
CREATE TABLE IF NOT EXISTS sesje (
    {", ".join(f"{name} {kind}" for name, kind in COLUMNS)}
);
CREATE INDEX IF NOT EXISTS sesje_osoba_start ON sesje (osoba, start_utc_s);
CREATE TABLE IF NOT EXISTS wyslane (
    id TEXT PRIMARY KEY,
    zmieniono_utc_s REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS odrzucone (
    id TEXT PRIMARY KEY,
    zmieniono_utc_s REAL NOT NULL,
    powod TEXT NOT NULL,
    ponow_utc_s REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS ukryte_online (
    id TEXT PRIMARY KEY
);
CREATE TABLE IF NOT EXISTS osiagniecia (
    osoba TEXT NOT NULL,
    id TEXT NOT NULL,
    zdobyte_utc_s REAL NOT NULL,
    wersja INTEGER NOT NULL,
    PRIMARY KEY (osoba, id)
);
CREATE TABLE IF NOT EXISTS poziom_osoby (
    osoba TEXT PRIMARY KEY,
    poziom INTEGER NOT NULL
);
"""


def uuid7() -> str:
    """UUID wersji 7 (RFC 9562) jako 32 znaki hex: najpierw milisekundy, potem losowe bity."""
    value = (int(time.time() * 1000) << 80) | int.from_bytes(os.urandom(10), "big")
    value = (value & ~(0xF << 76)) | (0x7 << 76)  # wersja 7
    value = (value & ~(0x3 << 62)) | (0x2 << 62)  # wariant RFC
    return f"{value:032x}"


def utc_offset_min(at: float) -> int:
    """Strefa czasowa w chwili startu sesji (minuty od UTC) - dni liczone jak u użytkownika."""
    local = datetime.fromtimestamp(at).astimezone()
    offset = local.utcoffset()
    return int(offset.total_seconds() // 60) if offset else 0


def migrate(db: sqlite3.Connection) -> None:
    """Schemat bazy do SCHEMA_VERSION. Baza z 0.3.x (bez wersji) zachowuje wszystkie sesje."""
    version = int(db.execute("PRAGMA user_version").fetchone()[0])
    exists = db.execute(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'sesje'"
    ).fetchone()
    if version >= SCHEMA_VERSION:
        return
    with db:
        if exists:
            have = {row[1] for row in db.execute("PRAGMA table_info(sesje)")}
            if "start" in have:
                db.execute("ALTER TABLE sesje RENAME COLUMN start TO start_utc_s")
                db.execute("ALTER TABLE sesje RENAME COLUMN koniec TO koniec_utc_s")
                have |= {"start_utc_s", "koniec_utc_s"}
            for name, kind in COLUMNS:
                if name not in have:
                    db.execute(f"ALTER TABLE sesje ADD COLUMN {name} {kind}")
            if version < 2:
                db.execute(
                    "UPDATE sesje SET zmieniono_utc_s = koniec_utc_s WHERE zmieniono_utc_s = 0"
                )
            log.info("Historia: baza z wersji %d przeniesiona do %d", version, SCHEMA_VERSION)
        db.executescript(SCHEMA)
        db.execute(f"PRAGMA user_version = {SCHEMA_VERSION}")


@dataclass(frozen=True)
class SessionSource:
    """Skąd sesja: bieżnia (model, sterownik, firmware) albo symulator."""

    model: str = ""
    driver: str = ""
    profile: str = ""
    firmware: str = ""
    app_version: str = ""
    source: str = "bieznia"


@dataclass
class Session:
    id: str
    person: str
    person_name: str
    profile: str
    profile_name: str
    start: float  # czas uniksowy (UTC)
    end: float
    steps: int = 0
    distance_km: float = 0.0
    active_s: float = 0.0
    device_model: str = ""
    driver: str = ""
    device_profile: str = ""
    firmware: str = ""
    app_version: str = ""
    source: str = "bieznia"
    visibility: str = "prywatna"
    tz_offset_min: int = 0
    updated: float = 0.0
    mode: str = "gra"  # gra albo trening

    @property
    def avg_kmh(self) -> float:
        return self.distance_km / (self.active_s / 3600) if self.active_s > 0 else 0.0

    def row(self) -> tuple[Any, ...]:
        return tuple(getattr(self, f.name) for f in fields(self))

    @classmethod
    def from_row(cls, r: tuple[Any, ...]) -> Session:
        return cls(*r)

    def with_source(self, src: SessionSource) -> None:
        self.device_model, self.driver, self.device_profile = src.model, src.driver, src.profile
        self.firmware, self.app_version, self.source = src.firmware, src.app_version, src.source


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
        migrate(self.db)
        self.db.execute("PRAGMA journal_mode=WAL")

    def close(self) -> None:
        self.db.close()

    def save(self, s: Session) -> None:
        s.updated = time.time()
        with self.db:
            self.db.execute(
                f"INSERT OR REPLACE {_INSERT}",
                s.row(),
            )

    def merge(self, sessions: list[Session]) -> int:
        """Import: dodaje sesje, których jeszcze nie ma (po id). Zwraca liczbę nowych."""
        before = self.count()
        with self.db:
            self.db.executemany(
                f"INSERT OR IGNORE {_INSERT}",
                [s.row() for s in sessions],
            )
        return self.count() - before

    # --- konto online (opcjonalne) ---------------------------------------------------------

    def unsent(self, person: str, now: float | None = None) -> list[Session]:
        """Sesje osoby powiązanej z kontem, których serwer jeszcze nie ma albo które się
        zmieniły od wysyłki. Bez sesji z symulatora (nie idą do rankingu ani statystyk).
        Odrzucona przez serwer czeka do terminu ponowienia (serwer mógł zmienić zasady),
        chyba że sama się zmieniła."""
        sql = (
            f"{_SELECT} WHERE osoba = ? AND zrodlo = 'bieznia' AND NOT EXISTS ("
            "SELECT 1 FROM wyslane w WHERE w.id = sesje.id"
            " AND w.zmieniono_utc_s >= sesje.zmieniono_utc_s) AND NOT EXISTS ("
            "SELECT 1 FROM odrzucone o WHERE o.id = sesje.id"
            " AND o.zmieniono_utc_s >= sesje.zmieniono_utc_s AND o.ponow_utc_s > ?)"
            " ORDER BY start_utc_s"
        )
        at = time.time() if now is None else now
        return [Session.from_row(r) for r in self.db.execute(sql, (person, at))]

    def mark_rejected(self, rejected: list[tuple[Session, str]], retry_at: float) -> None:
        """Odrzucone przez serwer: bez pętli wysyłek, ale z ponowieniem po retry_at."""
        with self.db:
            self.db.executemany(
                "INSERT OR REPLACE INTO odrzucone VALUES (?, ?, ?, ?)",
                [(s.id, s.updated, reason[:200], retry_at) for s, reason in rejected],
            )

    def rejected(self) -> dict[str, str]:
        return dict(self.db.execute("SELECT id, powod FROM odrzucone").fetchall())

    def mark_sent(self, sessions: list[Session]) -> None:
        with self.db:
            self.db.executemany(
                "INSERT OR REPLACE INTO wyslane VALUES (?, ?)",
                [(s.id, s.updated) for s in sessions],
            )
            self.db.executemany("DELETE FROM odrzucone WHERE id = ?", [(s.id,) for s in sessions])

    def forget_sent(self) -> None:
        """Inne konto, wylogowanie albo zmiana widoczności - wszystko pójdzie od nowa."""
        with self.db:
            self.db.execute("DELETE FROM wyslane")
            self.db.execute("DELETE FROM odrzucone")

    def hidden_online(self) -> set[str]:
        return {r[0] for r in self.db.execute("SELECT id FROM ukryte_online")}

    def set_hidden_online(self, session_id: str, hidden: bool) -> None:
        """Ukryta sesja zostaje u Ciebie, a online jest prywatna (nikt jej nie widzi)."""
        with self.db:
            if hidden:
                self.db.execute("INSERT OR IGNORE INTO ukryte_online VALUES (?)", (session_id,))
            else:
                self.db.execute("DELETE FROM ukryte_online WHERE id = ?", (session_id,))
            self.db.execute("DELETE FROM wyslane WHERE id = ?", (session_id,))

    # --- poziomy i osiągnięcia -----------------------------------------------------------

    def earned(self, person: str) -> dict[str, float]:
        """Zdobyte osiągnięcia osoby (id -> czas zdobycia). Raz zapisanych nie usuwamy."""
        rows = self.db.execute(
            "SELECT id, zdobyte_utc_s FROM osiagniecia WHERE osoba = ?", (person,)
        )
        return {str(r[0]): float(r[1]) for r in rows}

    def add_earned(self, person: str, items: dict[str, float], version: int) -> None:
        with self.db:
            self.db.executemany(
                "INSERT OR IGNORE INTO osiagniecia VALUES (?, ?, ?, ?)",
                [(person, k, at, version) for k, at in items.items()],
            )

    def shown_level(self, person: str) -> int | None:
        """Ostatni poziom pokazany osobie (None - jeszcze nigdy: pierwsze liczenie z historii)."""
        row = self.db.execute(
            "SELECT poziom FROM poziom_osoby WHERE osoba = ?", (person,)
        ).fetchone()
        return int(row[0]) if row else None

    def set_shown_level(self, person: str, level: int) -> None:
        with self.db:
            self.db.execute("INSERT OR REPLACE INTO poziom_osoby VALUES (?, ?)", (person, level))

    def schema_version(self) -> int:
        return int(self.db.execute("PRAGMA user_version").fetchone()[0])

    def version(self) -> tuple[int, int, float]:
        """Krótki odcisk zawartości (liczba sesji, kroki, ostatni koniec) - czy widok historii
        trzeba zbudować od nowa."""
        row = self.db.execute(
            "SELECT COUNT(*), COALESCE(SUM(kroki), 0), COALESCE(MAX(koniec_utc_s), 0) FROM sesje"
        ).fetchone()
        return int(row[0]), int(row[1]), float(row[2])

    def count(self) -> int:
        return int(self.db.execute("SELECT COUNT(*) FROM sesje").fetchone()[0])

    def sessions(
        self,
        since: float = 0.0,
        person: str | None = None,
        profiles: set[str] | None = None,
        limit: int | None = None,
    ) -> list[Session]:
        sql = f"{_SELECT} WHERE start_utc_s >= ?"
        args: list[Any] = [since]
        if person is not None:
            sql += " AND osoba = ?"
            args.append(person)
        sql += " ORDER BY start_utc_s DESC"
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

    def by_hour(
        self, person: str | None = None, key: str = "profile", now: float | None = None
    ) -> list[tuple[str, dict[str, int]]]:
        """Kroki dzisiaj godzina po godzinie: [("HH", {profil_lub_osoba: kroki})]. Sesja
        przez kilka godzin dzieli kroki proporcjonalnie do czasu w każdej godzinie."""
        today = datetime.fromtimestamp(now or time.time()).date()
        day_start = datetime.combine(today, datetime.min.time()).timestamp()
        out: list[dict[str, int]] = [{} for _ in range(24)]
        for s in self.sessions(day_start - 86400, person):
            name = s.profile_name if key == "profile" else s.person_name
            start, end = max(s.start, day_start), max(s.end, s.start)
            span = max(end - s.start, 1.0)
            for hour in range(24):
                h0 = day_start + hour * 3600
                overlap = min(end, h0 + 3600) - max(start, h0)
                if overlap <= 0:
                    continue
                share = s.steps if end - s.start < 1.0 else round(s.steps * overlap / span)
                if share:
                    out[hour][name] = out[hour].get(name, 0) + int(share)
        return [(f"{hour:02d}", parts) for hour, parts in enumerate(out)]

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

    def session(self, session_id: str) -> Session | None:
        row = self.db.execute(f"{_SELECT} WHERE id = ?", (session_id,)).fetchone()
        return Session.from_row(row) if row else None

    def delete(self, session_id: str) -> None:
        """Usuwa sesję z historii (i ślady wysyłki - nic po niej nie zostaje)."""
        with self.db:
            self.db.execute("DELETE FROM sesje WHERE id = ?", (session_id,))
            self.db.execute("DELETE FROM wyslane WHERE id = ?", (session_id,))
            self.db.execute("DELETE FROM ukryte_online WHERE id = ?", (session_id,))

    def was_sent(self, session_id: str) -> bool:
        return bool(self.db.execute("SELECT 1 FROM wyslane WHERE id = ?", (session_id,)).fetchone())

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
                self.db.execute("DELETE FROM osiagniecia")
                self.db.execute("DELETE FROM poziom_osoby")
            else:
                self.db.execute("DELETE FROM sesje WHERE osoba = ?", (person,))
                self.db.execute("DELETE FROM osiagniecia WHERE osoba = ?", (person,))
                self.db.execute("DELETE FROM poziom_osoby WHERE osoba = ?", (person,))


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
        self.origin = SessionSource()
        self.mode = "gra"

    def set_owner(self, person: str, person_name: str, profile: str, profile_name: str) -> None:
        """Zmiana osoby albo profilu kończy sesję poprzedniego."""
        new = (person, person_name, profile, profile_name)
        if new != self.owner:
            self.finish()
            self.owner = new

    def set_mode(self, mode: str) -> None:
        """Gra albo trening (tryb Trening) - zmiana kończy bieżącą sesję."""
        if mode != self.mode:
            self.finish()
            self.mode = mode

    def set_source(self, origin: SessionSource) -> None:
        """Inna bieżnia (albo symulator) to nowa sesja - wyniki nie mieszają się między nimi."""
        if (origin.model, origin.driver, origin.source) != (
            self.origin.model,
            self.origin.driver,
            self.origin.source,
        ):
            self.finish()
        self.origin = origin
        if self.current is not None and not self.current.firmware and origin.firmware:
            self.current.firmware = origin.firmware

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
                self.current = Session(uuid7(), person, pname, profile, prname, now, now)
                self.current.with_source(self.origin)
                self.current.tz_offset_min = utc_offset_min(now)
                self.current.mode = self.mode
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

    def flush(self) -> None:
        """Bieżąca sesja na dysk teraz (np. przed pokazaniem historii)."""
        if self.current is not None:
            self._save()

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
