"""Raport diagnostyczny Bluetooth - jeden plik tekstowy do wysłania, gdy coś nie działa.

Zawiera: listę urządzeń z okolicy, usługi i charakterystyki bieżni, surowe
powiadomienia (hex) i próbę rozmowy protokołem KingSmith. Bez danych osobowych
poza nazwami i adresami urządzeń Bluetooth w pobliżu.
"""

from __future__ import annotations

import asyncio
import platform
import sys
import time
from datetime import datetime
from importlib import metadata
from pathlib import Path
from typing import Any

from bieznia import __version__
from bieznia.ble.drivers import KingSmithDriver, find_kingsmith_channels, looks_like_treadmill

LISTEN_S = 12.0


async def run_diagnostics(path: Path, address: str | None) -> Path:
    from bleak import BleakClient, BleakScanner

    lines: list[str] = []
    t0 = time.monotonic()

    def out(text: str = "") -> None:
        lines.append(text)

    def stamp() -> str:
        return f"{time.monotonic() - t0:7.2f}s"

    out(f"Raport Bieżnia {__version__} - {datetime.now():%Y-%m-%d %H:%M:%S}")
    out(
        f"Windows {platform.version()} | Python {sys.version.split()[0]}"
        f" | bleak {metadata.version('bleak')}"
    )
    out()
    try:
        out("== Skan (8 s) ==")
        found = await BleakScanner.discover(timeout=8.0, return_adv=True)
        for dev, adv in sorted(found.values(), key=lambda da: -da[1].rssi):
            name = adv.local_name or dev.name or "(bez nazwy)"
            mark = "  <-- bieżnia?" if looks_like_treadmill(name, adv.service_uuids) else ""
            out(f"{dev.address}  {adv.rssi:4d} dBm  {name}{mark}")
            if adv.service_uuids:
                out(f"    usługi: {', '.join(adv.service_uuids)}")
            for company, data in adv.manufacturer_data.items():
                out(f"    producent 0x{company:04x}: {data.hex(' ')}")
        if address is None:
            candidates = [
                dev
                for dev, adv in found.values()
                if looks_like_treadmill(adv.local_name or dev.name, adv.service_uuids)
            ]
            address = candidates[0].address if candidates else None
        out()
        if address is None:
            out("Nie znaleziono bieżni - brak połączenia do zbadania.")
            return _save(path, lines)

        out(f"== Połączenie z {address} ==")
        device = await BleakScanner.find_device_by_address(address, timeout=10.0)
        if device is None:
            out("Urządzenie zniknęło przed połączeniem.")
            return _save(path, lines)
        async with BleakClient(
            device, timeout=20.0, winrt={"use_cached_services": False}
        ) as client:
            for service in client.services:
                out(f"usługa {service.uuid}  {service.description}")
                for char in service.characteristics:
                    out(f"    char {char.uuid}  [{', '.join(char.properties)}]  {char.description}")
            out()
            out(f"== Powiadomienia przez {LISTEN_S:.0f} s ==")

            def raw(char: Any, data: bytearray) -> None:
                out(f"{stamp()} << {char.uuid[:8]}  {bytes(data).hex(' ')}")

            channels = find_kingsmith_channels(client.services)
            channel_notify = {id(ch.notify) for ch in channels}
            for service in client.services:
                for char in service.characteristics:
                    if id(char) in channel_notify:
                        continue
                    if {"notify", "indicate"} & set(char.properties):
                        try:
                            await client.start_notify(char, raw)
                        except Exception as exc:
                            out(f"    nie da się subskrybować {char.uuid}: {exc}")

            if channels:
                out("Protokół KingSmith: kanały " + ", ".join(ch.label for ch in channels))

                def reading(speed: float, steps: int | None) -> None:
                    out(f"{stamp()} ODCZYT prędkość={speed} km/h kroki={steps}")

                driver = KingSmithDriver(client, channels, device.name or "", reading)
                original = driver._handle

                def handle(text: str) -> None:
                    out(f"{stamp()} tekst: {text}")
                    original(text)

                driver._handle = handle  # type: ignore[method-assign]
                await driver.start()
                for _ in range(int(LISTEN_S / 0.75)):
                    await driver.poll()
                    await asyncio.sleep(0.75)
                out(f"Wynik: {driver.describe()}")
            else:
                await asyncio.sleep(LISTEN_S)
    except Exception as exc:
        out(f"BŁĄD: {type(exc).__name__}: {exc}")
    return _save(path, lines)


def _save(path: Path, lines: list[str]) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return path
