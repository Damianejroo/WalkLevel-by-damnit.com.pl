"""Wybór klawiszy chodu i biegu."""

from bieznia.klawisze import (
    RUN_DEFAULT,
    TOGGLE_DEFAULT,
    WALK_DEFAULT,
    Key,
    key_from_native,
    problem,
)


def test_native_scan_codes_map_to_named_keys() -> None:
    assert key_from_native(0x148, "Up") == Key(0x48, True, "Strzałka w górę")
    assert key_from_native(0x11, "W") == Key(0x11, False, "W")
    assert key_from_native(0x1D, "Ctrl") == Key(0x1D, False, "Ctrl")
    assert key_from_native(0x11D, "Ctrl") == Key(0x1D, True, "Prawy Ctrl")
    assert key_from_native(0x00, "?") is None


def test_problems_are_explained() -> None:
    assert problem(WALK_DEFAULT, RUN_DEFAULT) is None
    assert "różne" in (problem(WALK_DEFAULT, WALK_DEFAULT) or "")
    assert "F6" in (problem(Key(0x40, False, "F6"), RUN_DEFAULT) or "")
    # klawisz start/stop nie może być klawiszem chodu ani biegu (aplikacja sama go naciska)
    assert "Start/stop" in (problem(Key(0x42, False, "F8"), RUN_DEFAULT) or "")
    assert "Start/stop" in (problem(WALK_DEFAULT, RUN_DEFAULT, Key(0x11, False, "W", 0x57)) or "")
    f9 = Key(0x43, False, "F9", 0x78)
    assert problem(Key(0x42, False, "F8"), RUN_DEFAULT, f9) is None  # F8 wolny po zmianie
    assert "inny" in (problem(WALK_DEFAULT, RUN_DEFAULT, Key(0x43, False, "F9")) or "")  # bez vk


def test_saved_keys_round_trip_and_fallback() -> None:
    up = Key(0x48, True, "Strzałka w górę")
    assert Key.from_dict(up.to_dict(), WALK_DEFAULT) == up
    assert Key.from_dict(None, WALK_DEFAULT) == WALK_DEFAULT
    assert Key.from_dict({"scan": 9999}, RUN_DEFAULT) == RUN_DEFAULT
    assert Key.from_dict(TOGGLE_DEFAULT.to_dict(), WALK_DEFAULT) == TOGGLE_DEFAULT
    # zapis sprzed wersji z kodem wirtualnym
    assert Key.from_dict({"scan": 0x11, "name": "W"}, RUN_DEFAULT) == Key(0x11, False, "W", 0)
