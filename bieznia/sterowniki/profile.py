"""Profile modeli bieżni - DANE (JSON), nie kod.

Profil mówi, jak rozpoznać model po nazwie Bluetooth, którym modułem z nim rozmawiać i
czym różni się od ustawień domyślnych modułu (zakres prędkości, odliczanie, pauza...).
Wbudowane leżą w bieznia/sterowniki/profile/, dodatkowe (np. pobrane) w dane/profile/.
Profil nigdy nie zawiera kodu - nowe sterowniki przychodzą zwykłą aktualizacją aplikacji.
"""

from __future__ import annotations

import json
import logging
from dataclasses import dataclass, fields
from pathlib import Path
from typing import Any

from bieznia.sterowniki.kontrakt import Capabilities

log = logging.getLogger("bieznia.ble")

FORMAT = 1
BUILTIN_DIR = Path(__file__).resolve().parent / "profile"
USER_DIR = Path("dane") / "profile"
DRIVERS = ("kingsmith", "ftms")

# Pola możliwości, które profil może ustawić, i ich typy.
_CAP_TYPES: dict[str, type] = {
    f.name: (bool if isinstance(f.default, bool) else float)
    for f in fields(Capabilities)
    if f.name not in ("incline", "control")
}
_LIMITS = {
    "speed_min": (0.1, 30.0),
    "speed_max": (0.5, 30.0),
    "speed_step": (0.01, 1.0),
    "start_speed": (0.1, 30.0),
    "start_delay_s": (0.0, 15.0),
    "telemetry_s": (0.1, 10.0),
    "min_command_gap_s": (0.0, 5.0),
}


class ProfileError(ValueError):
    """Plik profilu nie spełnia formatu - pomijamy go (z wpisem w logu)."""


@dataclass(frozen=True)
class ModelProfile:
    id: str
    name: str
    driver: str
    match: tuple[str, ...]  # początki nazw Bluetooth (wielkość liter bez znaczenia)
    capabilities: tuple[tuple[str, Any], ...] = ()
    options: tuple[tuple[str, Any], ...] = ()  # ustawienia modułu, np. tablica szyfru
    source: str = "wbudowany"

    def apply(self, base: Capabilities) -> Capabilities:
        from dataclasses import replace

        return replace(base, **dict(self.capabilities))

    def option(self, key: str, default: Any = None) -> Any:
        return dict(self.options).get(key, default)

    def matches(self, device_name: str) -> int:
        """Długość najdłuższego pasującego początku nazwy (0 = nie pasuje)."""
        upper = device_name.upper()
        return max((len(m) for m in self.match if upper.startswith(m.upper())), default=0)


def parse_profile(data: Any, source: str = "wbudowany") -> ModelProfile:
    if not isinstance(data, dict):
        raise ProfileError("profil musi być obiektem JSON")
    if data.get("format") != FORMAT:
        raise ProfileError(f"nieznany format profilu {data.get('format')!r}")
    pid, name, driver = data.get("id"), data.get("name"), data.get("driver")
    if not isinstance(pid, str) or not pid or not pid.replace("-", "").isalnum():
        raise ProfileError("brak poprawnego id (litery, cyfry, myślniki)")
    if not isinstance(name, str) or not name.strip():
        raise ProfileError("brak nazwy modelu")
    if driver not in DRIVERS:
        raise ProfileError(f"nieznany moduł {driver!r} (dozwolone: {', '.join(DRIVERS)})")
    match = data.get("match", [])
    if not isinstance(match, list) or not all(isinstance(m, str) and m for m in match):
        raise ProfileError("match: lista początków nazw Bluetooth")
    caps: dict[str, Any] = {}
    for key, value in (data.get("capabilities") or {}).items():
        kind = _CAP_TYPES.get(key)
        if kind is None:
            raise ProfileError(f"nieznana możliwość {key!r}")
        if kind is bool:
            if not isinstance(value, bool):
                raise ProfileError(f"{key}: oczekiwano true/false")
        else:
            if isinstance(value, bool) or not isinstance(value, int | float):
                raise ProfileError(f"{key}: oczekiwano liczby")
            lo, hi = _LIMITS.get(key, (0.0, 100.0))
            if not lo <= value <= hi:
                raise ProfileError(f"{key}: {value} poza zakresem {lo}-{hi}")
            value = float(value)
        caps[key] = value
    if caps.get("speed_min", 0.0) >= caps.get("speed_max", 99.0):
        raise ProfileError("speed_min musi być mniejsze od speed_max")
    options = data.get("options") or {}
    if not isinstance(options, dict) or not all(
        isinstance(v, str | int | float | bool) for v in options.values()
    ):
        raise ProfileError("options: tylko proste wartości")
    return ModelProfile(
        id=pid,
        name=name.strip(),
        driver=driver,
        match=tuple(match),
        capabilities=tuple(sorted(caps.items())),
        options=tuple(sorted(options.items())),
        source=source,
    )


def _load_dir(directory: Path, source: str) -> dict[str, ModelProfile]:
    found: dict[str, ModelProfile] = {}
    if not directory.is_dir():
        return found
    for path in sorted(directory.glob("*.json")):
        try:
            profile = parse_profile(json.loads(path.read_text(encoding="utf-8")), source)
        except (OSError, ValueError) as exc:
            log.warning("Profil bieżni %s pominięty: %s", path.name, exc)
            continue
        found[profile.id] = profile
    return found


_cache: list[ModelProfile] | None = None


def all_profiles(reload: bool = False) -> list[ModelProfile]:
    """Wbudowane + z folderu danych (ten sam id w danych zastępuje wbudowany)."""
    global _cache
    if _cache is None or reload:
        merged = _load_dir(BUILTIN_DIR, "wbudowany")
        merged.update(_load_dir(USER_DIR, "dodatkowy"))
        _cache = list(merged.values())
    return _cache


def profile_for(device_name: str, driver: str | None = None) -> ModelProfile | None:
    """Najlepiej pasujący profil (najdłuższy początek nazwy), opcjonalnie tylko dla modułu."""
    best: tuple[int, ModelProfile] | None = None
    for profile in all_profiles():
        if driver is not None and profile.driver != driver:
            continue
        score = profile.matches(device_name or "")
        if score and (best is None or score > best[0]):
            best = (score, profile)
    return best[1] if best else None
