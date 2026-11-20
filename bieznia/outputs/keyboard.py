"""Wyjście klawiaturowe przez SendInput: chód = klawisz chodu, bieg = klawisz biegu + chodu.

Domyślnie W i Shift; oba można zmienić w oknie (np. strzałka w górę, Ctrl).
"""

from __future__ import annotations

import logging

from bieznia.klawisze import RUN_DEFAULT, WALK_DEFAULT, Key
from bieznia.mapping import Control
from bieznia.winapi import send_scancode

log = logging.getLogger(__name__)


class KeyboardOutput:
    def __init__(self, walk: Key = WALK_DEFAULT, run: Key = RUN_DEFAULT) -> None:
        self.walk = walk
        self.run = run
        self._down: dict[Key, bool] = {walk: False, run: False}
        self._failing = False

    def _set(self, key: Key, down: bool) -> None:
        if self._down[key] == down:
            return
        try:
            send_scancode(key.scan, down, key.extended)
        except OSError as exc:
            # Np. ekran blokady, okno UAC albo Ctrl+Alt+Del - spróbujemy w następnej klatce.
            if not self._failing:
                log.warning("Windows odrzucił naciśnięcie klawisza: %s", exc)
                self._failing = True
            return
        if self._failing:
            log.info("Naciskanie klawiszy znów działa")
            self._failing = False
        self._down[key] = down

    def apply(self, control: Control) -> None:
        if control.run:
            # Najpierw klawisz biegu, potem chodu - gra od razu widzi bieg.
            self._set(self.run, True)
            self._set(self.walk, True)
        elif control.walk:
            self._set(self.run, False)
            self._set(self.walk, True)
        else:
            self._set(self.walk, False)
            self._set(self.run, False)

    def release_all(self) -> None:
        self.apply(Control.idle())
        # Bezpiecznik: puść oba klawisze także wtedy, gdy zapamiętany stan się rozjechał.
        for key in (self.walk, self.run):
            try:
                send_scancode(key.scan, False, key.extended)
            except OSError as exc:
                log.warning("Nie udało się puścić klawisza %s: %s", key.name, exc)
