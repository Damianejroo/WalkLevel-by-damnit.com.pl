"""Wyjścia mostu: co robimy z wyliczonym sterowaniem."""

from __future__ import annotations

from typing import Protocol

from bieznia.klawisze import RUN_DEFAULT, WALK_DEFAULT, Key
from bieznia.mapping import Control


class Output(Protocol):
    def apply(self, control: Control) -> None:
        """Ustawia stan wyjścia zgodnie ze sterowaniem (wywoływane co klatkę)."""

    def release_all(self) -> None:
        """Puszcza wszystko (pauza, błąd, wyjście)."""


def create_output(kind: str, walk: Key = WALK_DEFAULT, run: Key = RUN_DEFAULT) -> Output:
    if kind == "dry":
        from bieznia.outputs.dry import DryOutput

        return DryOutput()
    if kind == "keys":
        from bieznia.outputs.keyboard import KeyboardOutput

        return KeyboardOutput(walk, run)
    raise ValueError(f"Nieznane wyjście {kind!r} (dozwolone: dry, keys)")
