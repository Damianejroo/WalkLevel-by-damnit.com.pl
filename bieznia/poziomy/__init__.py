"""Poziomy, rangi i osiągnięcia WalkLevel liczone z historii sesji.

Definicje (progi, teksty, tabela 100 poziomów) są w definicje.json - jeden plik z wersją,
wspólny dla aplikacji i serwera. Tu tylko liczenie: czyste funkcje bez Qt i bez bazy.

Zasady (te same co na serwerze):
- liczą się sesje z bieżni, bez symulatora;
- dzień sesji to lokalny dzień jej POCZĄTKU (strefa zapisana przy sesji);
- seria: dzień liczy się od 2 km, jeden dzień wolny na 7 nie przerywa serii;
- XP = 100 za km + mała premia za każde zdobyte osiągnięcie;
- raz zdobytego osiągnięcia nie odbieramy (pilnuje tego zapis w historii, nie ta funkcja).
"""

from __future__ import annotations

import bisect
import json
import time
from collections import defaultdict
from collections.abc import Iterable
from dataclasses import dataclass, field
from functools import cache
from pathlib import Path
from typing import Any

DEFINITIONS = Path(__file__).with_name("definicje.json")
DAY_S = 86400
GAME_MODE = "gra"
TRAINING_MODE = "trening"


@dataclass(frozen=True)
class Achievement:
    id: str
    kind: str  # dystans, sesja_km, dzien_kroki, seria, gra_pierwsza, gry_km, gra_km, ...
    rarity: str
    mark: str  # znak na odznace (rysowany w kodzie)
    name: str
    flavor: str  # klimat: zdanie ze świata gry albo książki
    condition: str  # suchy warunek
    params: dict[str, Any] = field(default_factory=dict, hash=False, compare=False)

    @property
    def km(self) -> float | None:
        """Próg szlaku (dystans łącznie) - tylko dla osiągnięć typu dystans."""
        return float(self.params["km"]) if self.kind == "dystans" else None


@dataclass(frozen=True)
class Rank:
    level: int  # od którego poziomu
    name: str
    about: str


@dataclass(frozen=True)
class Definitions:
    version: int
    xp_per_km: int
    rarity_xp: dict[str, int]
    rarities: tuple[str, ...]
    levels: tuple[int, ...]  # XP potrzebne do poziomu n (indeks n - 1)
    ranks: tuple[Rank, ...]
    achievements: tuple[Achievement, ...]

    def by_id(self, achievement_id: str) -> Achievement | None:
        return next((a for a in self.achievements if a.id == achievement_id), None)

    @property
    def trail(self) -> list[Achievement]:
        """Przystanki szlaku od najbliższego."""
        return sorted((a for a in self.achievements if a.km is not None), key=lambda a: a.km or 0)

    @property
    def max_level(self) -> int:
        return len(self.levels)

    def level_of(self, xp: float) -> int:
        return max(1, bisect.bisect_right(self.levels, xp))

    def xp_for(self, level: int) -> int:
        level = max(1, min(level, self.max_level))
        return self.levels[level - 1]

    def rank(self, level: int) -> Rank:
        current = self.ranks[0]
        for r in self.ranks:
            if r.level <= level:
                current = r
        return current

    def next_rank(self, level: int) -> Rank | None:
        return next((r for r in self.ranks if r.level > level), None)


@cache
def load(path: Path = DEFINITIONS) -> Definitions:
    data = json.loads(path.read_text(encoding="utf-8"))
    common = ("id", "typ", "rzadkosc", "znak", "nazwa", "klimat", "warunek")
    achievements = tuple(
        Achievement(
            id=a["id"],
            kind=a["typ"],
            rarity=a["rzadkosc"],
            mark=a["znak"],
            name=a["nazwa"],
            flavor=a["klimat"],
            condition=a["warunek"],
            params={k: v for k, v in a.items() if k not in common},
        )
        for a in data["osiagniecia"]
    )
    return Definitions(
        version=int(data["wersja"]),
        xp_per_km=int(data["xp_za_km"]),
        rarity_xp={str(k): int(v) for k, v in data["xp_za_rzadkosc"].items()},
        rarities=tuple(data["rzadkosci"]),
        levels=tuple(int(x) for x in data["poziomy_xp"]),
        ranks=tuple(Rank(int(r["od_poziomu"]), r["nazwa"], r["opis"]) for r in data["rangi"]),
        achievements=achievements,
    )


# --- liczenie ---------------------------------------------------------------------------------


@dataclass(frozen=True)
class Walk:
    """To, czego potrzebuje liczenie, z jednej sesji historii."""

    start: float  # UTC
    end: float
    km: float
    steps: int
    game: str  # identyfikator profilu (gry)
    mode: str = GAME_MODE
    tz_offset_min: int = 0

    @property
    def day(self) -> int:
        """Lokalny dzień początku sesji (liczba dni od 1970-01-01)."""
        return int((self.start + self.tz_offset_min * 60) // DAY_S)

    @property
    def hour(self) -> int:
        return int(((self.start + self.tz_offset_min * 60) % DAY_S) // 3600)


MAX_KMH = 20.0  # średnio szybciej to nie chód na bieżni
MAX_STEPS_PER_MIN = 250
STEP_M = (0.3, 1.3)  # rozsądna długość kroku
DAY_MOTION_CAP_S = 8 * 3600  # ponad 8 h ruchu dziennie nic się nie liczy


def reliable(s: Any) -> bool:
    """Te same zasady co na serwerze: z bieżni, z ruchem, w ludzkim tempie. Bieżnia, która nie
    liczy kroków (0), nie jest sprawdzana pod kątem kroków."""
    if getattr(s, "source", "bieznia") != "bieznia":
        return False
    active = float(s.active_s)
    km = float(s.distance_km)
    steps = int(s.steps)
    if active <= 0 or km < 0:
        return False
    if km / (active / 3600) > MAX_KMH:
        return False
    if steps > 0:
        if steps / (active / 60) > MAX_STEPS_PER_MIN:
            return False
        step = km * 1000 / steps
        if km > 0 and not STEP_M[0] <= step <= STEP_M[1]:
            return False
    return True


def walks_from_sessions(sessions: Iterable[Any]) -> list[Walk]:
    """Sesje historii -> lista do liczenia, tak samo jak na serwerze: tylko wiarygodne sesje
    z bieżni, sesja zaczynająca się przed końcem poprzedniej jest pomijana, a ruch ponad 8 h
    w dniu lokalnym nie daje dystansu ani kroków (proporcjonalnie)."""
    ordered = sorted((s for s in sessions if reliable(s)), key=lambda s: (s.start, s.end))
    out: list[Walk] = []
    last_end = float("-inf")
    used: dict[int, float] = defaultdict(float)
    for s in ordered:
        start, end = float(s.start), float(s.end)
        if start < last_end:
            continue
        last_end = end
        tz = int(s.tz_offset_min)
        day = int((start + tz * 60) // DAY_S)
        active = float(s.active_s)
        allowed = max(0.0, DAY_MOTION_CAP_S - used[day])
        share = 1.0 if active <= allowed else allowed / active
        used[day] += active
        out.append(
            Walk(
                start=start,
                end=end,
                km=max(0.0, float(s.distance_km)) * share,
                steps=int(max(0, int(s.steps)) * share),
                game=str(s.profile),
                mode=str(getattr(s, "mode", GAME_MODE) or GAME_MODE),
                tz_offset_min=tz,
            )
        )
    out.sort(key=lambda w: (w.end, w.start))
    return out


@dataclass
class Progress:
    achievement: Achievement
    earned_at: float | None = None  # koniec sesji, w której padł próg (UTC)
    current: float = 0.0
    target: float = 1.0

    @property
    def earned(self) -> bool:
        return self.earned_at is not None

    @property
    def fraction(self) -> float:
        if self.earned:
            return 1.0
        return max(0.0, min(1.0, self.current / self.target)) if self.target > 0 else 0.0


@dataclass
class Status:
    definitions: Definitions
    km: float
    xp: int
    level: int
    progress: dict[str, Progress]

    @property
    def rank(self) -> Rank:
        return self.definitions.rank(self.level)

    @property
    def next_rank(self) -> Rank | None:
        return self.definitions.next_rank(self.level)

    @property
    def level_floor(self) -> int:
        return self.definitions.xp_for(self.level)

    @property
    def level_ceiling(self) -> int:
        if self.level >= self.definitions.max_level:
            return self.level_floor
        return self.definitions.xp_for(self.level + 1)

    @property
    def level_fraction(self) -> float:
        span = self.level_ceiling - self.level_floor
        return 1.0 if span <= 0 else max(0.0, min(1.0, (self.xp - self.level_floor) / span))

    @property
    def km_to_next_level(self) -> float:
        return max(0.0, (self.level_ceiling - self.xp) / self.definitions.xp_per_km)

    @property
    def earned(self) -> list[Progress]:
        """Zdobyte, od najnowszego."""
        got = [p for p in self.progress.values() if p.earned]
        got.sort(key=lambda p: p.earned_at or 0.0, reverse=True)
        return got

    def closest(self, limit: int = 3) -> list[Progress]:
        """Niezdobyte najbliżej celu (największy ułamek)."""
        left = [p for p in self.progress.values() if not p.earned and p.current > 0]
        left.sort(key=lambda p: p.fraction, reverse=True)
        return left[:limit]


def _first_crossing(values: list[tuple[float, float]], target: float) -> float | None:
    """(czas, wartość narastająca) -> czas pierwszego przekroczenia progu."""
    for at, value in values:
        if value >= target - 1e-9:
            return at
    return None


def _streaks(
    day_done: dict[int, float], last_day: int, rest_per_week: int
) -> tuple[dict[int, int], int | None]:
    """Długość serii (dni kalendarzowe od jej początku) na każdy zaliczony dzień oraz początek
    serii, która trwa jeszcze po last_day (None - przerwana).

    Dzień wolny przerywa serię, gdy jest drugi z rzędu albo gdy w ostatnich 7 dniach serii
    był już inny wolny dzień (najwyżej rest_per_week wolnych na 7 dni)."""
    if not day_done:
        return {}, None
    out: dict[int, int] = {}
    start: int | None = None
    rests: list[int] = []
    for d in range(min(day_done), max(last_day, max(day_done)) + 1):
        if d in day_done:
            if start is None:
                start, rests = d, []
            out[d] = d - start + 1
            continue
        if start is None:
            continue
        recent = [r for r in rests if r > d - 7]
        if (d - 1) in rests or len(recent) >= rest_per_week:
            start, rests = None, []
        else:
            rests.append(d)
    return out, start


def evaluate(
    defs: Definitions,
    walks: list[Walk],
    *,
    friends: int = 0,
    referrals: int = 0,
    earned_before: dict[str, float] | None = None,
    now: float | None = None,
    rest_per_week: int = 1,
) -> Status:
    """Stan gracza z jego sesji. earned_before: zapisane wcześniej zdobycze (nie odbieramy ich,
    nawet gdy sesja zniknie z historii)."""
    now = time.time() if now is None else now
    earned_before = earned_before or {}

    total = 0.0
    cumulative: list[tuple[float, float]] = []
    best_session: list[tuple[float, float]] = []
    day_steps: dict[int, int] = defaultdict(int)
    day_steps_at: list[tuple[float, int]] = []
    day_km: dict[int, float] = defaultdict(float)
    day_done_at: dict[int, float] = {}
    game_km: dict[str, float] = defaultdict(float)
    best_game: list[tuple[float, float]] = []
    games_over: dict[float, list[tuple[float, float]]] = defaultdict(list)
    first_game: float | None = None
    first_training: float | None = None
    windows: dict[str, float] = {}
    best_km = 0.0
    best_day = 0
    gry = [a for a in defs.achievements if a.kind == "gry_km"]
    min_km_day = max(
        (float(a.params.get("km_dziennie", 2.0)) for a in defs.achievements if a.kind == "seria"),
        default=2.0,
    )

    for w in walks:
        total += w.km
        cumulative.append((w.end, total))
        best_km = max(best_km, w.km)
        best_session.append((w.end, best_km))
        day_steps[w.day] += w.steps
        best_day = max(best_day, day_steps[w.day])
        day_steps_at.append((w.end, best_day))
        day_km[w.day] += w.km
        if day_km[w.day] >= min_km_day and w.day not in day_done_at:
            day_done_at[w.day] = w.end
        if w.mode == TRAINING_MODE:
            if first_training is None:
                first_training = w.end
        else:
            if first_game is None:
                first_game = w.end
            game_km[w.game] += w.km
            best_game.append((w.end, max(game_km.values())))
            for a in gry:
                need = float(a.params["km"])
                games_over[need].append((w.end, sum(1 for v in game_km.values() if v >= need)))
        for a in defs.achievements:
            if a.kind == "start_miedzy" and a.id not in windows:
                lo, hi = int(a.params["od"]), int(a.params["do"])
                inside = lo <= w.hour < hi if lo < hi else (w.hour >= lo or w.hour < hi)
                if inside:
                    windows[a.id] = w.end

    # dzisiejszy dzień jeszcze trwa, więc nie jest dniem wolnym: seria trwa, jeśli przetrwała
    # do wczoraj
    today = int((now + (walks[-1].tz_offset_min if walks else 0) * 60) // DAY_S)
    streak, open_start = _streaks(day_done_at, today - 1, rest_per_week)
    alive = 0
    if open_start is not None:
        alive = streak[max(day_done_at)]

    progress: dict[str, Progress] = {}
    for a in defs.achievements:
        p = Progress(a)
        if a.kind == "dystans":
            p.target, p.current = float(a.params["km"]), total
            p.earned_at = _first_crossing(cumulative, p.target)
        elif a.kind == "sesja_km":
            p.target, p.current = float(a.params["km"]), best_km
            p.earned_at = _first_crossing(best_session, p.target)
        elif a.kind == "dzien_kroki":
            p.target, p.current = float(a.params["kroki"]), float(best_day)
            p.earned_at = _first_crossing([(t, float(v)) for t, v in day_steps_at], p.target)
        elif a.kind == "seria":
            p.target, p.current = float(a.params["dni"]), float(alive)
            hits = sorted(day_done_at[d] for d, n in streak.items() if n >= p.target)
            p.earned_at = hits[0] if hits else None
        elif a.kind == "gra_pierwsza":
            p.current = 1.0 if first_game is not None else 0.0
            p.earned_at = first_game
        elif a.kind == "trening":
            p.current = 1.0 if first_training is not None else 0.0
            p.earned_at = first_training
        elif a.kind == "gry_km":
            series = games_over.get(float(a.params["km"]), [])
            p.target = float(a.params["gier"])
            p.current = float(series[-1][1]) if series else 0.0
            p.earned_at = _first_crossing([(t, float(v)) for t, v in series], p.target)
        elif a.kind == "gra_km":
            p.target = float(a.params["km"])
            p.current = max(game_km.values(), default=0.0)
            p.earned_at = _first_crossing(best_game, p.target)
        elif a.kind == "start_miedzy":
            p.current = 1.0 if a.id in windows else 0.0
            p.earned_at = windows.get(a.id)
        elif a.kind == "znajomi":
            p.target, p.current = float(a.params["ile"]), float(friends)
            p.earned_at = now if friends >= p.target else None
        elif a.kind == "polecenia":
            p.target, p.current = float(a.params["ile"]), float(referrals)
            p.earned_at = now if referrals >= p.target else None
        if a.id in earned_before:  # nie odbieramy
            p.earned_at = (
                earned_before[a.id]
                if p.earned_at is None
                else min(p.earned_at, earned_before[a.id])
            )
        progress[a.id] = p

    bonus = sum(defs.rarity_xp.get(p.achievement.rarity, 0) for p in progress.values() if p.earned)
    xp = int(total * defs.xp_per_km) + bonus
    return Status(defs, total, xp, defs.level_of(xp), progress)
