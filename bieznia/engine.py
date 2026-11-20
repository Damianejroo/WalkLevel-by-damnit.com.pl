"""Silnik mostu: źródło -> mapowanie -> wyjście. Jedno wywołanie tick() na klatkę.

Używany przez okno (gui) i tryb bez okna (headless). Wszystko w jednym wątku.
"""

from __future__ import annotations

import logging
from collections import deque
from dataclasses import dataclass, field

from bieznia.mapping import Control, MappingConfig, SpeedMapper
from bieznia.outputs import Output, create_output
from bieznia.recorder import Recorder
from bieznia.sources import Reading, Source

log = logging.getLogger("bieznia")

HISTORY_S = 60.0


@dataclass
class Snapshot:
    """Stan do pokazania w interfejsie."""

    enabled: bool = False
    output: str = "dry"
    control: Control = field(default_factory=Control.idle)
    steps: int | None = None
    cadence: float | None = None  # kroki na minutę
    live: bool = False  # czy dane z bieżni są świeże
    distance_km: float = 0.0
    active_s: float = 0.0  # czas ruchu
    # (czas, km/h) z ostatniej minuty; 20 klatek/s -> do 1200 punktów
    history: deque[tuple[float, float]] = field(default_factory=lambda: deque(maxlen=1300))


class Bridge:
    def __init__(self, mapping: MappingConfig, output: str = "dry", enabled: bool = False) -> None:
        self.mapping = mapping
        self.mapper = SpeedMapper(mapping)
        self.output_kind = output
        self.output: Output = create_output(output)
        self.source: Source | None = None
        self.recorder: Recorder | None = None
        self.state = Snapshot(enabled=enabled, output=output)
        self._last_reading: Reading | None = None
        self._last_tick: float | None = None
        self._steps_window: deque[tuple[float, int]] = deque(maxlen=20)

    # --- ustawienia ---------------------------------------------------------

    def set_source(self, source: Source | None) -> None:
        if self.source is not None:
            self.source.close()
        self.source = source
        self._go_idle()

    def set_output(self, kind: str) -> None:
        if kind == self.output_kind:
            return
        self.output.release_all()
        self.output = create_output(kind)
        self.output_kind = self.state.output = kind
        log.info("Wyjście: %s", "klawiatura (W / Shift)" if kind == "keys" else "podgląd")

    def set_mapping(self, mapping: MappingConfig) -> None:
        self.mapping = mapping
        self.mapper = SpeedMapper(mapping)

    def set_enabled(self, enabled: bool) -> None:
        if enabled == self.state.enabled:
            return
        self.state.enabled = enabled
        log.info("Sterowanie %s", "WŁĄCZONE" if enabled else "WYŁĄCZONE")
        if not enabled:
            self.output.release_all()

    def toggle(self) -> bool:
        self.set_enabled(not self.state.enabled)
        return self.state.enabled

    def set_recording(self, recorder: Recorder | None) -> None:
        if self.recorder is not None:
            self.recorder.close()
        self.recorder = recorder

    # --- pętla --------------------------------------------------------------

    def _go_idle(self) -> None:
        self.mapper.reset()
        self._last_reading = None
        self.state.control = Control.idle()
        self.state.live = False
        self.state.cadence = None

    def _update_cadence(self, reading: Reading) -> None:
        if reading.steps is None:
            return
        self._steps_window.append((reading.t, reading.steps))
        first_t, first_steps = self._steps_window[0]
        if reading.t - first_t >= 3.0 and reading.steps >= first_steps:
            self.state.cadence = (reading.steps - first_steps) / (reading.t - first_t) * 60

    def tick(self, now: float, actions: set[str] | frozenset[str] = frozenset()) -> Snapshot:
        """Jedna klatka. Rzuca SourceFinished, gdy źródło się skończy."""
        if "toggle" in actions:
            self.toggle()
        dt = 0.0 if self._last_tick is None else min(1.0, now - self._last_tick)
        self._last_tick = now

        reading = self.source.poll(now, set(actions)) if self.source else None
        if reading is not None:
            self._last_reading = reading
            self.state.control = self.mapper.update(reading.speed_kmh)
            self.state.live = True
            self.state.steps = reading.steps
            self._update_cadence(reading)
            if self.recorder:
                self.recorder.write(reading)
        elif (
            self._last_reading is not None
            and now - self._last_reading.t > self.mapping.stale_after_s
        ):
            log.warning(
                "Brak danych z bieżni od %.1f s - zatrzymuję postać", now - self._last_reading.t
            )
            self._go_idle()

        control = self.state.control
        if control.walk:
            self.state.distance_km += control.speed * dt / 3600
            self.state.active_s += dt
        self.state.history.append((now, control.speed))
        while self.state.history and now - self.state.history[0][0] > HISTORY_S:
            self.state.history.popleft()

        if self.state.enabled:
            self.output.apply(control)
        return self.state

    def shutdown(self) -> None:
        self.output.release_all()
        if self.source is not None:
            self.source.close()
        self.set_recording(None)
