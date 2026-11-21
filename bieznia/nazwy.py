"""Losowe nazwy osób: przymiotnik + rzeczownik w zgodnym rodzaju („Szybki Zawodnik”).

Przymiotniki zapisane parami (męski, żeński), rzeczowniki osobno dla każdego rodzaju -
odmiana nie jest zgadywana z końcówek, więc nie wyjdzie „Szybki Biegaczka”.
"""

from __future__ import annotations

import random
from collections.abc import Iterable

ADJECTIVES: tuple[tuple[str, str], ...] = (
    ("Szybki", "Szybka"),
    ("Szalony", "Szalona"),
    ("Podstępny", "Podstępna"),
    ("Dzielny", "Dzielna"),
    ("Wytrwały", "Wytrwała"),
    ("Niezmordowany", "Niezmordowana"),
    ("Zwinny", "Zwinna"),
    ("Nocny", "Nocna"),
    ("Cichy", "Cicha"),
    ("Żelazny", "Żelazna"),
    ("Błyskawiczny", "Błyskawiczna"),
    ("Nieustraszony", "Nieustraszona"),
    ("Tajemniczy", "Tajemnicza"),
    ("Legendarny", "Legendarna"),
    ("Waleczny", "Waleczna"),
    ("Uparty", "Uparta"),
    ("Sprytny", "Sprytna"),
    ("Wesoły", "Wesoła"),
)
NOUNS_M: tuple[str, ...] = (
    "Zawodnik",
    "Biegacz",
    "Maszerowicz",
    "Piechur",
    "Wędrowiec",
    "Tropiciel",
    "Sprinter",
    "Spacerowicz",
    "Truchtacz",
    "Łazik",
    "Podróżnik",
    "Zdobywca",
)
NOUNS_F: tuple[str, ...] = (
    "Zawodniczka",
    "Biegaczka",
    "Maszerowiczka",
    "Piechurka",
    "Wędrowniczka",
    "Tropicielka",
    "Sprinterka",
    "Spacerowiczka",
    "Podróżniczka",
    "Zdobywczyni",
)


def all_names() -> list[str]:
    names = [f"{m} {n}" for m, _f in ADJECTIVES for n in NOUNS_M]
    names += [f"{f} {n}" for _m, f in ADJECTIVES for n in NOUNS_F]
    return names


def random_name(taken: Iterable[str] = (), rng: random.Random | None = None) -> str:
    """Nazwa, której jeszcze nikt nie ma (gdy wszystkie zajęte - z numerem)."""
    rng = rng or random.Random()
    used = set(taken)
    free = [n for n in all_names() if n not in used]
    if free:
        return rng.choice(free)
    base = rng.choice(all_names())
    i = 2
    while f"{base} {i}" in used:
        i += 1
    return f"{base} {i}"


def suggestions(
    count: int, taken: Iterable[str] = (), rng: random.Random | None = None
) -> list[str]:
    rng = rng or random.Random()
    used = set(taken)
    free = [n for n in all_names() if n not in used]
    return rng.sample(free, min(count, len(free)))
