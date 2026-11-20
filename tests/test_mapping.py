import pytest

from bieznia.mapping import Control, MappingConfig, SpeedMapper


def mapper(**kwargs: float) -> SpeedMapper:
    return SpeedMapper(MappingConfig(smoothing_samples=1, **kwargs))


def test_standing_presses_nothing() -> None:
    assert mapper().update(0.5) == Control(speed=0.5, stick=0.0, walk=False, run=False)


def test_walking_presses_w_only() -> None:
    c = mapper().update(3.0)
    assert c.walk and not c.run


def test_run_has_hysteresis() -> None:
    m = mapper()
    assert not m.update(6.0).run  # poniżej run_on
    assert m.update(6.3).run  # włącza się od run_on
    assert m.update(5.8).run  # nadal bieg nad run_off
    assert not m.update(5.6).run  # wyłącza się poniżej run_off
    assert not m.update(6.0).run  # i znów potrzeba run_on


def test_stopping_resets_run() -> None:
    m = mapper()
    m.update(8.0)
    assert m.update(0.0) == Control(speed=0.0, stick=0.0, walk=False, run=False)
    assert not m.update(6.0).run


def test_stick_is_linear_and_clamped() -> None:
    m = mapper(stand_below=1.0, max_speed=11.0)
    assert m.update(6.0).stick == pytest.approx(0.5)
    assert m.update(20.0).stick == 1.0


def test_smoothing_averages_last_samples() -> None:
    m = SpeedMapper(MappingConfig(smoothing_samples=3))
    m.update(3.0)
    m.update(6.0)
    assert m.update(9.0).speed == pytest.approx(6.0)
    assert m.update(9.0).speed == pytest.approx(8.0)


def test_negative_speed_is_treated_as_zero() -> None:
    assert mapper().update(-5.0).speed == 0.0


@pytest.mark.parametrize(
    "kwargs",
    [
        {"run_on": 5.0, "run_off": 5.5},
        {"stand_below": 7.0},
        {"max_speed": 6.0},
        {"smoothing_samples": 0},
    ],
)
def test_invalid_config_is_rejected(kwargs: dict[str, float]) -> None:
    with pytest.raises(ValueError):
        MappingConfig(**kwargs)


def test_from_dict_ignores_unknown_keys() -> None:
    cfg = MappingConfig.from_dict({"run_on": 7.0, "foo": 1})
    assert cfg.run_on == 7.0


def test_full_x21_range_and_zero_walk_threshold() -> None:
    m = mapper(stand_below=0.0, run_on=12.0, run_off=11.4, max_speed=12.0)
    assert not m.update(0.0).walk  # stojąca taśma nie rusza postaci
    assert m.update(0.5).walk
    assert m.update(12.0).run
    assert m.update(12.0).stick == 1.0


# --- szybkie zatrzymanie: prawdziwe sekwencje z bieżni KS-NACH-X21C (log testera) ---------
# Bieżnia podaje prędkość co ~2 s; między ramkami aplikacja co 0,75 s powtarza ostatnią
# prędkość (sygnał życia), więc symulujemy oba źródła odczytów.


def replay(frames: list[tuple[float, float]], quick: bool, walk_from: float = 1.0) -> float | None:
    """Zwraca czas, w którym postać przestała iść (albo None)."""
    m = SpeedMapper(MappingConfig(stand_below=walk_from, run_on=4.0, run_off=3.4, quick_stop=quick))
    readings: list[tuple[float, float]] = []
    for (t, v), nxt in zip(
        frames, [*frames[1:], (frames[-1][0] + 2.0, frames[-1][1])], strict=True
    ):
        hb = t
        while hb < nxt[0]:
            readings.append((hb, v))
            hb += 0.75
    walking = False
    for t, v in readings:
        c = m.update(v, t)
        if c.walk:
            walking = True
        elif walking:
            return t
    return None


STOP_1 = [(44.6, 2.3), (46.6, 2.2), (48.6, 1.2), (50.6, 1.0), (53.0, 0.0), (55.0, 0.0)]
STOP_2 = [(12.8, 3.6), (14.8, 2.7), (16.8, 1.8), (18.8, 1.6), (20.7, 1.4), (22.7, 1.0), (25.5, 0.0)]
STOP_3 = [(0.0, 1.7), (2.0, 1.7), (4.0, 1.7), (6.1, 1.0), (8.0, 0.0), (10.0, 0.0)]


@pytest.mark.parametrize(
    ("frames", "before", "after"),
    [(STOP_1, 53.0, 48.6), (STOP_2, 25.5, 14.8), (STOP_3, 8.0, 6.1)],
)
def test_quick_stop_cuts_stop_lag(
    frames: list[tuple[float, float]], before: float, after: float
) -> None:
    assert replay(frames, quick=False) == pytest.approx(before)
    assert replay(frames, quick=True) == pytest.approx(after)


def test_slowing_down_with_remote_does_not_stop() -> None:
    # pilot: 3,0 -> 2,8 -> 2,5 -> 2,5 ... (spadki 0,2-0,3 km/h)
    frames = [(0.0, 3.0), (2.0, 2.8), (4.0, 2.5), (6.0, 2.5), (8.0, 2.5)]
    assert replay(frames, quick=True) is None


def test_quick_slowdown_resumes_walking_when_speed_holds() -> None:
    m = SpeedMapper(MappingConfig(stand_below=1.0, run_on=6.3, run_off=5.7))
    assert m.update(5.0, 0.0).walk
    assert not m.update(3.5, 2.0).walk  # spadek 1,5 = hamowanie
    assert not m.update(3.5, 4.0).walk  # nadal w oknie 3 s
    assert m.update(3.5, 5.5).walk  # nie spada dalej - to było tylko zwolnienie
    assert m.update(4.0, 6.0).walk


# zatrzymania z logu wersji 0.2.6 (próg chodu testera 1,0 km/h): (ramki, stop przed, stop teraz)
STOP_SLOW = [(0.0, 1.7), (2.0, 1.7), (4.0, 1.5), (6.0, 1.0), (8.0, 0.0), (10.0, 0.0)]
STOP_RAMP = [(0.0, 2.0), (2.0, 2.0), (4.0, 1.9), (6.0, 1.0), (8.0, 0.0), (10.0, 0.0)]


def test_slow_walk_stop_is_caught_near_the_walk_threshold() -> None:
    assert replay(STOP_SLOW, quick=False) == pytest.approx(8.0)
    assert replay(STOP_SLOW, quick=True) == pytest.approx(4.0)  # 1,7 -> 1,5 przy progu 1,0


def test_ramp_stop_is_caught_on_the_big_drop() -> None:
    assert replay(STOP_RAMP, quick=True) == pytest.approx(6.0)  # 1,9 -> 1,0 (0,9 km/h)


def test_remote_steps_of_half_kmh_do_not_stop_running() -> None:
    m = SpeedMapper(MappingConfig(stand_below=1.0, run_on=4.0, run_off=3.4))
    for t, v in enumerate([5.0, 4.5, 4.0, 3.5]):  # pilot w dół po 0,5 km/h
        assert m.update(v, t * 2.0).walk
