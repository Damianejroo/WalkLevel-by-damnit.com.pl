"""Wyjście klawiaturowe przez SendInput: chód = klawisz chodu, bieg = klawisz biegu + chodu.

Klawisz jest wciśnięty przez cały czas chodu (puszczany, gdy stoisz). Jak na prawdziwej
klawiaturze trzymany klawisz chodu powtarza się po chwili - w Notatniku widać wtedy
„wwww”, a gry, które czytają zdarzenia zamiast stanu klawisza, też widzą ciągły ruch.
Domyślnie W i Shift; oba można zmienić w oknie (np. strzałka w górę, Ctrl).
"""

from __future__ import annotations

import logging
import time
from collections.abc import Callable

from bieznia.klawisze import RUN_DEFAULT, WALK_DEFAULT, Key
from bieznia.mapping import Control
from bieznia.winapi import send_scancode

log = logging.getLogger(__name__)

REPEAT_DELAY_S = 0.35  # jak domyślne opóźnienie powtarzania w Windows
REPEAT_EVERY_S = 0.05  # ok. 20 powtórzeń na sekundę


class KeyboardOutput:
    def __init__(
        self,
        walk: Key = WALK_DEFAULT,
        run: Key = RUN_DEFAULT,
        clock: Callable[[], float] = time.monotonic,
    ) -> None:
        self.walk = walk
        self.run = run
        self.clock = clock
        self._down: dict[Key, bool] = {walk: False, run: False}
        self._walk_since = 0.0
        self._last_repeat = 0.0
        self._failing = False

    def _send(self, key: Key, down: bool) -> bool:
        try:
            send_scancode(key.scan, down, key.extended)
        except OSError as exc:
            # Np. ekran blokady, okno UAC albo Ctrl+Alt+Del - spróbujemy w następnej klatce.
            if not self._failing:
                log.warning("Windows odrzucił naciśnięcie klawisza: %s", exc)
                self._failing = True
            return False
        if self._failing:
            log.info("Naciskanie klawiszy znów działa")
            self._failing = False
        return True

    def _set(self, key: Key, down: bool) -> None:
        if self._down[key] == down:
            return
        if self._send(key, down):
            self._down[key] = down
            if key == self.walk and down:
                self._walk_since = self._last_repeat = self.clock()

    def _repeat_walk(self) -> None:
        now = self.clock()
        held = now - self._walk_since
        if held >= REPEAT_DELAY_S and now - self._last_repeat >= REPEAT_EVERY_S:
            self._send(self.walk, True)  # kolejne „wciśnięcie” bez puszczania = powtórzenie
            self._last_repeat = now

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
        if self._down[self.walk]:
            self._repeat_walk()

    def release_all(self) -> None:
        self.apply(Control.idle())
        # Bezpiecznik: puść oba klawisze także wtedy, gdy zapamiętany stan się rozjechał.
        for key in (self.walk, self.run):
            try:
                send_scancode(key.scan, False, key.extended)
            except OSError as exc:
                log.warning("Nie udało się puścić klawisza %s: %s", key.name, exc)
