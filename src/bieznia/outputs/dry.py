"""Wyjście „na sucho”: niczego nie naciska, tylko zapisuje zmiany stanu do logu."""

from __future__ import annotations

import logging

from bieznia.mapping import Control

log = logging.getLogger(__name__)


class DryOutput:
    def __init__(self) -> None:
        self._state: tuple[bool, bool] = (False, False)

    def apply(self, control: Control) -> None:
        state = (control.walk, control.run)
        if state != self._state:
            log.info("[dry] W=%s Shift=%s (%.1f km/h)", *state, control.speed)
            self._state = state

    def release_all(self) -> None:
        self.apply(Control.idle())
