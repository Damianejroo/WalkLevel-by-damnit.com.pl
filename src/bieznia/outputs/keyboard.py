"""Wyjście klawiaturowe: W (chód) i Shift+W (bieg) przez SendInput."""

from __future__ import annotations

from bieznia.mapping import Control
from bieznia.winapi import SCAN_LSHIFT, SCAN_W, release_movement_keys, send_scancode


class KeyboardOutput:
    def __init__(self) -> None:
        self._down: dict[int, bool] = {SCAN_W: False, SCAN_LSHIFT: False}

    def _set(self, scan: int, down: bool) -> None:
        if self._down[scan] != down:
            send_scancode(scan, down)
            self._down[scan] = down

    def apply(self, control: Control) -> None:
        if control.run:
            # Najpierw Shift, potem W – gra od razu widzi bieg.
            self._set(SCAN_LSHIFT, True)
            self._set(SCAN_W, True)
        elif control.walk:
            self._set(SCAN_LSHIFT, False)
            self._set(SCAN_W, True)
        else:
            self._set(SCAN_W, False)
            self._set(SCAN_LSHIFT, False)

    def release_all(self) -> None:
        try:
            self.apply(Control.idle())
        finally:
            release_movement_keys()
