"""Bieżnia: wyszukiwanie, połączenie, symulator, stan połączenia i zdarzenia z wątku
Bluetooth. Okno pyta o możliwości bieżni (kontrakt sterownika), nie o model."""

from __future__ import annotations

import logging
from datetime import datetime
from pathlib import Path
from typing import TYPE_CHECKING

from PySide6.QtWidgets import (
    QVBoxLayout,
    QWidget,
)

from bieznia import (
    __version__,
    historia,
)
from bieznia.ble.worker import BleEvent, FoundDevice, Link
from bieznia.gui.okno import header, page_layout
from bieznia.gui.widgets import (
    DeviceRow,
    button,
    card,
    label,
)
from bieznia.sources.ble_source import QueueSource
from bieznia.sterowniki import Capabilities, DeviceInfo, known_capabilities, model_name

if TYPE_CHECKING:
    from bieznia.gui.app import App

log = logging.getLogger("bieznia")


class TreadmillScreen:
    device: DeviceInfo | None
    devices: list[FoundDevice]

    def build_devices(self: App) -> QWidget:
        page = QWidget()
        lay = page_layout(page)
        lay.addLayout(header("Wybierz bieżnię", lambda: self.pages.go(self.MAIN, back=True)))
        self.devices_status = label("", "muted")
        self.devices_status.setFixedHeight(24)
        lay.addWidget(self.devices_status)
        self.devices_box = QVBoxLayout()
        self.devices_box.setSpacing(8)
        self.devices_box.setContentsMargins(0, 0, 0, 0)
        holder = QWidget()
        holder.setObjectName("page")
        holder.setLayout(self.devices_box)
        lay.addWidget(self.scroll(holder), 1)
        rescan = button("Szukaj ponownie", "primary")
        rescan.clicked.connect(self.scan)
        lay.addWidget(rescan)
        return page

    def set_tracker_source(self: App) -> None:
        """Sesje w historii podpisane bieżnią (model, sterownik, firmware) albo symulatorem."""
        tracker = getattr(self, "tracker", None)
        if tracker is None or not hasattr(self, "caps"):
            return  # okno w budowie - wywołane jeszcze raz, gdy wiadomo, jaka bieżnia
        device = getattr(self, "device", None)
        simulated = bool(self.settings["simulator"])
        if device is not None and device.simulated == simulated:
            model, driver, profile, firmware = (
                device.model,
                device.driver,
                device.profile,
                device.firmware,
            )
        elif simulated:
            model, driver, profile, firmware = "Symulator bieżni", "symulator", "symulator", ""
        else:
            name = self.settings["device_name"]
            model, driver, profile, firmware = (model_name(name) if name else ""), "", "", ""
        tracker.set_source(
            historia.SessionSource(
                model=model,
                driver=driver,
                profile=profile,
                firmware=firmware,
                app_version=__version__,
                source="symulator" if simulated else "bieznia",
            )
        )

    def apply_source(self: App) -> None:
        """Odczyty zawsze z wątku Bluetooth - symulator to tam zwykły sterownik."""
        self.bridge.set_source(QueueSource(self.worker.readings))

    def on_simulator(self: App, on: bool) -> None:
        self.change("simulator", on)
        self.device = None
        self.caps = known_capabilities(self.settings["device_name"], on)
        self.set_tracker_source()
        if on:
            self.worker.connect_simulator()
        elif self.settings["device_address"]:
            self.worker.connect(self.settings["device_address"], self.settings["device_name"])
        else:
            self.worker.disconnect()
        log.info("Symulator %s", "włączony" if on else "wyłączony")
        self.render_link()

    def scan(self: App) -> None:
        self.devices = []
        self.render_devices(scanning=True)
        self.pages.go(self.DEVICES)
        self.worker.scan()

    def connect_device(self: App, device: FoundDevice) -> None:
        self.settings["device_address"] = device.address
        self.settings["device_name"] = device.name
        if self.settings["simulator"]:
            self.settings["simulator"] = False  # wybrana prawdziwa bieżnia wygrywa z symulatorem
        self.device = None
        self.caps = known_capabilities(device.name)
        self.set_tracker_source()
        self.save()
        self.worker.connect(device.address, device.name)
        if self.pages.currentIndex() == self.DEVICES:
            self.pages.go(self.MAIN, back=True)
        log.info("Wybrano bieżnię %s", device.name)
        self.render_device_label()

    def forget_device(self: App) -> None:
        self.worker.disconnect()
        self.settings["device_address"] = self.settings["device_name"] = ""
        self.save()
        self.auto_connect = True
        log.info("Bieżnia zapomniana")
        self.render_link()

    def on_status_action(self: App) -> None:
        if self.settings["simulator"]:
            self.on_simulator(False)
        elif self.link in (Link.STREAMING, Link.CONNECTED, Link.CONNECTING, Link.RETRYING) or (
            self.link is Link.SEARCHING and self.settings["device_address"]
        ):
            self.worker.disconnect()
        elif self.settings["device_address"] and self.link is Link.BLUETOOTH_OFF:
            self.worker.connect(self.settings["device_address"], self.settings["device_name"])
        else:
            self.scan()

    def diagnose(self: App) -> None:
        path = Path("logs") / f"diagnostyka-{datetime.now():%Y%m%d-%H%M%S}.txt"
        self.diag_status.setText("Zbieram raport (ok. 30 s)…")
        self.worker.diagnose(path)

    def on_belt_event(self: App, result: str) -> None:
        messages = {
            "started": "Bieżnia rusza.",
            "stopped": "Bieżnia zatrzymana.",
            "refused": "Bieżnia nie przyjęła startu. Obudź ją pilotem albo sprawdź, czy klucz"
            " bezpieczeństwa jest na miejscu, i spróbuj jeszcze raz.",
            "unknown": "Wysłano start, ale bieżnia nie odpowiedziała. Jeśli taśma nie rusza,"
            " uruchom ją pilotem.",
            "unsupported": "Ta bieżnia nie obsługuje startu z aplikacji - użyj pilota.",
            "not_connected": "Brak połączenia z bieżnią - start i stop tylko pilotem.",
        }
        log.info("Taśma: %s", result)
        self.render_output_hint(messages.get(result, ""))

    def render_link(self: App) -> None:
        if self.window is None:
            return
        t, primary = self.t, False
        if self.device is not None and not self.device.simulated:
            name = self.device.model
        else:
            name = model_name(self.settings["device_name"]) or "bieżnia"
        if self.settings["simulator"]:
            keys = self.config["hotkeys"]
            color, busy, title = t.accent, False, "Symulator bieżni"
            sub, action = (
                f"{keys['sim_slower']} wolniej, {keys['sim_faster']} szybciej",
                "Wyłącz symulator",
            )
        elif self.link is Link.STREAMING:
            stale = not self.bridge.state.live
            color, busy = (t.run if stale else t.accent), stale
            title = f"Połączono: {name}"
            sub = "Czekam na dane z bieżni" if stale else self.link_text
            action = "Rozłącz"
        elif self.link in (Link.SEARCHING, Link.CONNECTING, Link.CONNECTED):
            color, busy, title, sub = (
                t.accent,
                True,
                self.link_text,
                "Bieżnia włączona, telefon rozłączony",
            )
            action = "Anuluj" if self.settings["device_address"] else "Szukaj bieżni"
        elif self.link is Link.RETRYING:
            color, busy, title, sub, action = (
                t.run,
                True,
                "Ponawiam połączenie",
                self.link_text,
                "Rozłącz",
            )
        elif self.link is Link.BLUETOOTH_OFF:
            color, busy, title = t.danger, False, "Bluetooth wyłączony"
            sub, action, primary = "Włącz Bluetooth w Windows", "Spróbuj ponownie", True
        elif self.link is Link.UNSUPPORTED:
            color, busy, title = t.danger, False, "To urządzenie nie jest bieżnią"
            sub, action, primary = "Wybierz inne albo zapisz raport", "Szukaj bieżni", True
        else:
            color, busy, title = t.text_2, False, "Bieżnia nie połączona"
            sub, action, primary = "Włącz bieżnię i kliknij Szukaj bieżni", "Szukaj bieżni", True
        self.dot.set_state(color, busy)
        if self.status_title.text() != title:
            self.status_title.setText(title)
        if self.status_sub.text() != sub:
            self.status_sub.setText(sub)
        if self.status_action.text() != action:
            self.status_action.setText(action)
            self.status_action.setAccessibleName(action)
        kind = "primary" if primary else ""
        if self.status_action.objectName() != kind:
            self.status_action.setObjectName(kind)
            self.status_action.style().unpolish(self.status_action)
            self.status_action.style().polish(self.status_action)
        self.render_device_label()

    def render_devices(self: App, scanning: bool = False) -> None:
        while self.devices_box.count():
            item = self.devices_box.takeAt(0)
            if item is not None and item.widget() is not None:
                item.widget().deleteLater()  # type: ignore[union-attr]
        if scanning:
            self.devices_status.setText("Szukam urządzeń Bluetooth (ok. 6 s)…")
            self.devices_box.addStretch(1)
            return
        treadmills = [d for d in self.devices if d.treadmill]
        others = [d for d in self.devices if not d.treadmill]
        self.devices_status.setText(
            f"Znalezione bieżnie: {len(treadmills)}, inne urządzenia: {len(others)}"
            if self.devices
            else ""
        )
        if not treadmills:
            empty = card()
            el = QVBoxLayout(empty)
            el.setContentsMargins(14, 12, 14, 14)
            steps: tuple[str, ...]
            if self.link is Link.BLUETOOTH_OFF:
                title, steps = (
                    "Bluetooth jest wyłączony",
                    (
                        "1. Otwórz Start > Ustawienia > Bluetooth i urządzenia.",
                        "2. Włącz Bluetooth.",
                        "3. Wróć tutaj i kliknij Szukaj ponownie.",
                    ),
                )
            else:
                title, steps = (
                    "Nie widzę bieżni",
                    (
                        "1. Włącz bieżnię (pilotem albo przyciskiem).",
                        "2. Rozłącz telefon: zamknij aplikację bieżni (KS Fit, Urevo)"
                        " albo wyłącz w telefonie Bluetooth."
                        " Bieżnia przyjmuje jedno połączenie naraz.",
                        "3. Sprawdź, czy Bluetooth w Windows jest włączony.",
                        "4. Kliknij Szukaj ponownie.",
                    ),
                )
            el.addWidget(label(title, "title"))
            for line in steps:
                step = label(line, "muted")
                step.setWordWrap(True)
                el.addWidget(step)
            self.devices_box.addWidget(empty)
        for device in treadmills:
            model = model_name(device.name)
            kind = model if model != device.name else "Bieżnia"
            row = DeviceRow(device.name, f"{kind} · sygnał {device.rssi} dBm", self.t)
            row.clicked.connect(lambda _=False, d=device: self.connect_device(d))
            self.devices_box.addWidget(row)
        if others:
            self.devices_box.addWidget(label("Inne urządzenia w pobliżu", "section"))
            for device in others[:12]:
                row = DeviceRow(device.name, f"sygnał {device.rssi} dBm", self.t)
                row.clicked.connect(lambda _=False, d=device: self.connect_device(d))
                self.devices_box.addWidget(row)
        self.devices_box.addStretch(1)

    def handle_event(self: App, event: BleEvent) -> None:
        if event.kind == "link" and event.link is not None:
            self.link, self.link_text = event.link, event.text
            self.render_link()
        elif event.kind == "devices":
            self.devices = list(event.devices)
            treadmills = [d for d in self.devices if d.treadmill]
            if self.auto_connect and len(treadmills) == 1:
                self.auto_connect = False
                log.info("Znaleziono jedną bieżnię - łączę automatycznie")
                self.connect_device(treadmills[0])
            else:
                self.auto_connect = False
                self.render_devices()
                if self.pages.currentIndex() != self.WIZARD:  # konfiguratora nie przerywamy
                    self.pages.go(self.DEVICES)
        elif event.kind == "scan_failed":
            if event.link is not None:
                self.link, self.link_text = event.link, event.text
                self.render_link()
            self.auto_connect = False
            self.render_devices()
            if self.pages.currentIndex() != self.WIZARD:
                self.pages.go(self.DEVICES)
            self.devices_status.setText(event.text)
        elif event.kind == "belt":
            self.on_belt_event(event.text)
        elif event.kind == "device" and event.device is not None and event.caps is not None:
            self.on_device(event.device, event.caps)
        elif event.kind == "report":
            self.diag_status.setText(f"Raport: {event.text}")
            log.info("Raport diagnostyczny: %s", event.text)

    def on_device(self: App, device: DeviceInfo, caps: Capabilities) -> None:
        """Bieżnia (albo symulator) powiedziała, czym jest i co potrafi."""
        if device.simulated != bool(self.settings["simulator"]):
            return  # spóźnione zdarzenie po przełączeniu symulatora
        changed = caps != self.caps
        self.device, self.caps = device, caps
        self.set_tracker_source()
        log.info(
            "Bieżnia: %s, %.1f-%.1f km/h%s",
            device.model,
            caps.speed_min,
            caps.speed_max,
            f", rozruch ok. {caps.start_delay_s:.0f} s" if caps.start_delay_s >= 1 else "",
        )
        if changed:
            self.refresh_settings()  # zakres prędkości w Ustawieniach - tylko gdy się zmienił
        self.render_link()
