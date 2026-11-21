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


def combo(scan: int, name: str, vk: int, *mods: str, extended: bool = False) -> Key:
    return Key(scan, extended, name, vk, tuple(mods))


def test_toggle_combinations_allowed() -> None:
    for key in (
        combo(0x42, "F8", 0x77, "ctrl", "shift"),  # Ctrl+Shift+F8
        combo(0x42, "F8", 0x77, "ctrl"),
        combo(0x42, "F8", 0x77, "alt"),
        combo(0x42, "F8", 0x77, "shift"),
        combo(0x30, "B", 0x42, "ctrl", "shift"),  # Ctrl+Shift+B
        combo(0x22, "G", 0x47, "ctrl", "alt"),  # Ctrl+Alt+G - nie jest polską literą AltGr
    ):
        assert problem(WALK_DEFAULT, RUN_DEFAULT, key) is None, key.label
    assert combo(0x42, "F8", 0x77, "ctrl", "shift").label == "Ctrl+Shift+F8"


def test_toggle_combinations_that_collide_are_rejected() -> None:
    cases = {
        combo(0x42, "F8", 0x77, "win"): "Windows",
        combo(0x0F, "Tab", 0x09, "alt"): "Alt+Tab",
        combo(0x3E, "F4", 0x73, "alt"): "Alt+F4",
        combo(0x0F, "Tab", 0x09, "shift"): "Steam",
        combo(0x2C, "Z", 0x5A, "alt"): "NVIDIA",
        combo(0x01, "Esc", 0x1B, "ctrl", "shift"): "Esc",
        combo(0x1F, "S", 0x53, "ctrl", "alt"): "AltGr",  # ś
        combo(0x12, "E", 0x45, "ctrl", "alt"): "AltGr",  # ę
        combo(0x10, "Q", 0x51, "shift"): "wielką literę",
        combo(0x2E, "C", 0x43, "ctrl"): "kopiuje",
        Key(0x58, False, "F12", 0x7B): "F12",
        combo(0x1D, "Ctrl", 0xA2, "shift"): "razem z Ctrl",
    }
    for key, reason in cases.items():
        found = problem(WALK_DEFAULT, RUN_DEFAULT, key)
        assert found is not None and reason in found, (key.label, found)
    # klawisz chodu z modyfikatorem nadal koliduje z chodem
    assert problem(WALK_DEFAULT, RUN_DEFAULT, combo(0x11, "W", 0x57, "ctrl")) is not None


def test_combination_survives_saving() -> None:
    key = combo(0x42, "F8", 0x77, "shift", "ctrl")
    assert key.mods == ("shift", "ctrl")  # kolejność z konstruktora...
    saved = Key.from_dict(key.to_dict(), TOGGLE_DEFAULT)
    assert saved.mods == ("ctrl", "shift")  # ...po odczycie zawsze Ctrl, Alt, Shift
    assert Key.from_dict(TOGGLE_DEFAULT.to_dict(), WALK_DEFAULT) == TOGGLE_DEFAULT
    assert "mods" not in TOGGLE_DEFAULT.to_dict()  # pojedynczy klawisz zapisuje się jak w 0.2
