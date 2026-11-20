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
