"""Sterowniki bieżni: kontrakt, rejestr modułów i rozpoznawanie modelu.

Rdzeń aplikacji używa tylko tego modułu i kontraktu (kontrakt.py). Moduły: kingsmith
(WalkingPad), ftms (standard Bluetooth, np. Urevo) i symulator. Model rozpoznajemy po
usługach GATT (który moduł) i nazwie Bluetooth (który profil - dane w profile/*.json).
"""

from __future__ import annotations

from collections.abc import Iterable

from bieznia.sterowniki.ftms import protokol as ftms
from bieznia.sterowniki.ftms.sterownik import DEFAULT_CAPS as FTMS_CAPS
from bieznia.sterowniki.ftms.sterownik import FtmsDriver, has_ftms
from bieznia.sterowniki.kingsmith.sterownik import DEFAULT_CAPS as KINGSMITH_CAPS
from bieznia.sterowniki.kingsmith.sterownik import (
    KingSmithDriver,
    find_channels,
    is_kingsmith_service,
)
from bieznia.sterowniki.kontrakt import (
    Capabilities,
    DeviceInfo,
    Driver,
    GattClient,
    ReadingCallback,
    UnsupportedDevice,
)
from bieznia.sterowniki.profile import ModelProfile, all_profiles, profile_for
from bieznia.sterowniki.symulator import CAPS as SIMULATOR_CAPS
from bieznia.sterowniki.symulator import SimulatorDriver

_DEFAULTS = {"kingsmith": KINGSMITH_CAPS, "ftms": FTMS_CAPS}


def make_driver(client: GattClient, device_name: str, on_reading: ReadingCallback) -> Driver:
    """Sterownik dla połączonej bieżni: moduł po usługach GATT, profil po nazwie."""
    services = list(client.services)
    channels = find_channels(services)
    if channels:
        profile = profile_for(device_name, "kingsmith")
        return KingSmithDriver(client, channels, device_name, on_reading, profile)
    if has_ftms(services):
        profile = profile_for(device_name, "ftms")
        return FtmsDriver(client, services, device_name, on_reading, profile)
    raise UnsupportedDevice(
        "To urządzenie nie wygląda na bieżnię (brak usługi KingSmith ani FTMS). "
        "Zapisz raport diagnostyczny i wyślij go."
    )


def known_capabilities(device_name: str, simulated: bool = False) -> Capabilities:
    """Możliwości przed połączeniem (z profilu po zapamiętanej nazwie) - okno nie czeka
    na bieżnię, żeby pokazać właściwy zakres prędkości."""
    if simulated:
        return SIMULATOR_CAPS
    profile = profile_for(device_name)
    if profile is None:
        return KINGSMITH_CAPS if device_name.upper().startswith("KS-") else Capabilities()
    return profile.apply(_DEFAULTS[profile.driver])


def model_name(device_name: str) -> str:
    """Czytelna nazwa modelu z profilu (URTM054 -> Urevo SpaceWalk 5L)."""
    profile = profile_for(device_name)
    if profile is None:
        return device_name
    if profile.id == "urevo":  # ogólny profil marki: model zostaje z nazwy Bluetooth
        return f"Urevo {device_name}"
    return profile.name


def looks_like_treadmill(name: str | None, service_uuids: Iterable[str] = ()) -> bool:
    """Do sortowania wyników skanowania: znany model (profil) albo usługa bieżni."""
    if name and profile_for(name) is not None:
        return True
    upper = (name or "").upper()
    if upper.startswith(("KS-", "WALKINGPAD")):
        return True
    return any(u.lower() == ftms.FTMS_SERVICE or is_kingsmith_service(u) for u in service_uuids)


__all__ = [
    "Capabilities",
    "DeviceInfo",
    "Driver",
    "FtmsDriver",
    "KingSmithDriver",
    "ModelProfile",
    "SimulatorDriver",
    "UnsupportedDevice",
    "all_profiles",
    "known_capabilities",
    "looks_like_treadmill",
    "make_driver",
    "model_name",
    "profile_for",
]
