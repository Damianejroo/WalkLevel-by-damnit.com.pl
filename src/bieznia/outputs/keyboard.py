"""Wyjście klawiaturowe: W (chód) i Shift+W (bieg) przez SendInput."""

from __future__ import annotations

import logging

from bieznia.mapping import Control
from bieznia.winapi import SCAN_LSHIFT, SCAN_W, release_movement_keys, send_scancode

log = logging.getLogger(__name__)


class KeyboardOutput:
    def __init__(self) -> None:
        self._down: dict[int, bool] = {SCAN_W: False, SCAN_LSHIFT: False}
        self._failing = False

    def _set(self, scan: int, down: bool) -> None:
        if self._down[scan] == down:
            return
        try:
            send_scancode(scan, down)
        except OSError as exc:
            # Np. ekran blokady, okno UAC albo Ctrl+Alt+Del – spróbujemy w następnej klatce.
            if not self._failing:
                log.warning("Windows odrzucił naciśnięcie klawisza: %s", exc)
                self._failing = True
            return
        if self._failing:
            log.info("Naciskanie klawiszy znów działa")
            self._failing = False
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
        self.apply(Control.idle())
        try:
            release_movement_keys()
        except OSError as exc:
            log.warning("Nie udało się puścić klawiszy: %s", exc)
