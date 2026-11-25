"""Dźwięki popupów, syntetyzowane od zera (bez próbek i cudzych nagrań).

    uv run python packaging/dzwieki.py

- osiagniecie.wav: dwa jasne „dzwoneczki” w górę (kwarta) i krótki błysk wysokich tonów,
- awans.wav: szybkie arpeggio w górę i trzymany akord durowy na końcu.

Pliki trafiają do bieznia/gui/dzwieki (paczka instalatora bierze je z datas w bieznia.spec).
"""

from __future__ import annotations

import math
import random
import struct
import wave
from pathlib import Path

RATE = 44100
PEAK = 0.45  # ok. -7 dBFS - słychać nad grą, ale nie zagłusza
OUT = Path(__file__).resolve().parent.parent / "bieznia" / "gui" / "dzwieki"


def note(name: str) -> float:
    """„C5” -> częstotliwość (A4 = 440 Hz)."""
    steps = {"C": -9, "D": -7, "E": -5, "F": -4, "G": -2, "A": 0, "B": 2}
    semis = steps[name[0]] + 12 * (int(name[-1]) - 4)
    if "#" in name:
        semis += 1
    return 440.0 * 2 ** (semis / 12)


def bell(buf: list[float], start: float, freq: float, length: float, gain: float) -> None:
    """Dzwoneczek: ton podstawowy i lekko nieharmoniczne alikwoty, długie wybrzmienie."""
    partials = ((1.0, 1.0, 1.0), (2.0, 0.35, 1.6), (2.76, 0.18, 2.4), (4.07, 0.08, 3.5))
    s0 = int(start * RATE)
    for i in range(int(length * RATE)):
        t = i / RATE
        attack = min(1.0, t / 0.004)
        v = 0.0
        for ratio, amp, decay in partials:
            v += amp * math.exp(-t * decay * 4.0) * math.sin(2 * math.pi * freq * ratio * t)
        if s0 + i < len(buf):
            buf[s0 + i] += gain * attack * v


def chip(buf: list[float], start: float, freq: float, length: float, gain: float) -> None:
    """Krótki ton „z gry”: miękki prostokąt (kilka nieparzystych harmonicznych), szybkie zejście."""
    s0 = int(start * RATE)
    for i in range(int(length * RATE)):
        t = i / RATE
        env = min(1.0, t / 0.003) * math.exp(-t * 9.0)
        v = sum(math.sin(2 * math.pi * freq * k * t) / k for k in (1, 3, 5, 7))
        if s0 + i < len(buf):
            buf[s0 + i] += gain * env * v


def pad(buf: list[float], start: float, freqs: list[float], length: float, gain: float) -> None:
    """Trzymany akord z delikatnym vibrato, łagodne narastanie i wybrzmienie."""
    s0 = int(start * RATE)
    n = int(length * RATE)
    for i in range(n):
        t = i / RATE
        env = min(1.0, t / 0.02) * math.exp(-t * 3.2)
        vib = 1 + 0.004 * math.sin(2 * math.pi * 5.5 * t) * min(1.0, t / 0.15)
        v = 0.0
        for f in freqs:
            v += math.sin(2 * math.pi * f * vib * t) + 0.3 * math.sin(4 * math.pi * f * vib * t)
        if s0 + i < len(buf):
            buf[s0 + i] += gain * env * v / len(freqs)


def sparkle(buf: list[float], start: float, length: float, gain: float, seed: int) -> None:
    """Błysk: kilka bardzo krótkich, wysokich, cichych tonów rozsianych w czasie."""
    rng = random.Random(seed)
    for k in range(7):
        at = start + k * length / 7 + rng.uniform(0, 0.015)
        f = rng.choice((note("E7"), note("B7"), note("G#7"), note("E8")))
        bell(buf, at, f, 0.18, gain * (1 - k / 9))


def fade_out(buf: list[float], seconds: float) -> None:
    n = int(seconds * RATE)
    for i in range(n):
        buf[-n + i] *= 1 - i / n


def save(name: str, buf: list[float]) -> Path:
    peak = max(abs(x) for x in buf) or 1.0
    OUT.mkdir(parents=True, exist_ok=True)
    path = OUT / name
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(b"".join(struct.pack("<h", int(x / peak * PEAK * 32767)) for x in buf))
    return path


def achievement() -> list[float]:
    buf = [0.0] * int(0.85 * RATE)
    bell(buf, 0.0, note("B5"), 0.8, 0.8)
    bell(buf, 0.09, note("E6"), 0.75, 1.0)
    sparkle(buf, 0.12, 0.3, 0.12, seed=7)
    fade_out(buf, 0.08)
    return buf


def level_up() -> list[float]:
    buf = [0.0] * int(1.15 * RATE)
    for k, n in enumerate(("C5", "E5", "G5", "C6")):
        chip(buf, k * 0.065, note(n), 0.2, 0.32)
    pad(buf, 0.26, [note("C6"), note("E6"), note("G6"), note("C7")], 0.88, 0.9)
    bell(buf, 0.26, note("C7"), 0.6, 0.35)
    sparkle(buf, 0.3, 0.4, 0.1, seed=11)
    fade_out(buf, 0.1)
    return buf


if __name__ == "__main__":
    for name, buf in (("osiagniecie.wav", achievement()), ("awans.wav", level_up())):
        path = save(name, buf)
        print(f"{path} ({len(buf) / RATE:.2f} s, {path.stat().st_size // 1024} KB)")
