"""Globalne skróty: wykrywa moment wciśnięcia klawisza, także gdy gra ma fokus.

Sprawdza wyłącznie klawisze z konfiguracji, niczego nie zapisuje.
"""

from __future__ import annotations

from collections.abc import Callable

from bieznia.klawisze import MOD_VK
from bieznia.winapi import is_key_down

VK_CODES = {f"F{i}": 0x6F + i for i in range(1, 13)}
Binding = str | int | tuple[int, tuple[str, ...]]


class Hotkeys:
    def __init__(
        self, bindings: dict[str, Binding], key_down: Callable[[int], bool] = is_key_down
    ) -> None:
        """bindings: akcja -> nazwa klawisza ("F8"), kod wirtualny (0x77) albo kod
        z modyfikatorami ((0x77, ("ctrl", "shift")) = Ctrl+Shift+F8)."""
        self._vk: dict[str, int] = {}
        self._mods: dict[str, tuple[int, ...]] = {}
        for action, key in bindings.items():
            if isinstance(key, tuple):
                vk, mods = key
                key = vk
                self._mods[action] = tuple(MOD_VK[m] for m in mods)
            if isinstance(key, int):
                if not 0 < key < 0x100:
                    raise ValueError(f"Zły kod klawisza {key!r} dla {action!r}")
                self._vk[action] = key
                continue
            name = key.upper()
            if name not in VK_CODES:
                raise ValueError(
                    f"Nieobsługiwany klawisz {key!r} dla {action!r} (dozwolone F1-F12)"
                )
            self._vk[action] = VK_CODES[name]
        self._key_down = key_down
        # Klawisze trzymane przy starcie nie liczą się jako wciśnięcie.
        self._was_down = {action: self._down(action) for action in self._vk}

    def _down(self, action: str) -> bool:
        """Klawisz wciśnięty razem z wymaganymi modyfikatorami (inne nie przeszkadzają -
        np. Shift trzymany do biegu nie blokuje pojedynczego F8)."""
        if not self._key_down(self._vk[action]):
            return False
        return all(self._key_down(vk) for vk in self._mods.get(action, ()))

    def poll(self) -> set[str]:
        """Zwraca akcje, których klawisz został właśnie wciśnięty."""
        pressed: set[str] = set()
        for action in self._vk:
            down = self._down(action)
            if down and not self._was_down[action]:
                pressed.add(action)
            self._was_down[action] = down
        return pressed
