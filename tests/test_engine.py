"""Silnik mostu: bezpieczniki i liczniki."""

from __future__ import annotations

from bieznia.engine import Bridge
from bieznia.mapping import Control, MappingConfig
from bieznia.sources import Reading


class ListSource:
    def __init__(self) -> None:
        self.queue: list[Reading] = []

    def push(self, t: float, speed: float, steps: int | None = None) -> None:
        self.queue.append(Reading(t=t, speed_kmh=speed, steps=steps))

    def poll(self, now: float, actions: set[str]) -> Reading | None:
        return self.queue.pop(0) if self.queue else None

    def close(self) -> None:
        pass


class SpyOutput:
    def __init__(self) -> None:
        self.applied: list[bool] = []
        self.released = 0

    def apply(self, control: Control) -> None:
        self.applied.append(control.walk)

    def release_all(self) -> None:
        self.released += 1


def make(enabled: bool = True) -> tuple[Bridge, ListSource, SpyOutput]:
    bridge = Bridge(MappingConfig(smoothing_samples=1), "dry", enabled)
    spy = SpyOutput()
    bridge.output = spy
    source = ListSource()
    bridge.set_source(source)
    return bridge, source, spy


def test_disabled_bridge_never_touches_output() -> None:
    bridge, source, spy = make(enabled=False)
    source.push(0.0, 4.0)
    bridge.tick(0.0)
    assert spy.applied == []


def test_stale_data_stops_the_character() -> None:
    bridge, source, spy = make()
    source.push(0.0, 4.0)
    assert bridge.tick(0.0).control.walk
    assert bridge.tick(1.0).control.walk  # jeszcze świeże
    state = bridge.tick(3.0)  # dłużej niż stale_after_s bez odczytu
    assert not state.control.walk and not state.live
    assert spy.applied[-1] is False


def test_toggle_releases_keys() -> None:
    bridge, _source, spy = make()
    bridge.tick(0.0, {"toggle"})
    assert not bridge.state.enabled
    assert spy.released == 1


def test_distance_time_and_cadence() -> None:
    bridge, source, _ = make()
    for i in range(41):  # 10 s co 0,25 s przy 6 km/h i 2 krokach na sekundę
        source.push(i * 0.25, 6.0, steps=i // 2)
        bridge.tick(i * 0.25)
    state = bridge.state
    assert abs(state.distance_km - 6.0 * 10 / 3600) < 0.001
    assert abs(state.active_s - 10.0) < 0.01
    assert state.cadence is not None and 100 < state.cadence < 140


def test_history_keeps_last_minute() -> None:
    bridge, _source, _ = make()
    for i in range(200):
        bridge.tick(i * 0.5)
    times = [t for t, _ in bridge.state.history]
    assert times[-1] - times[0] <= 60.0
