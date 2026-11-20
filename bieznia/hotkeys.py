"""Globalne skróty: wykrywa moment wciśnięcia klawisza, także gdy gra ma fokus.

Sprawdza wyłącznie klawisze z konfiguracji, niczego nie zapisuje.
"""

from __future__ import annotations

from collections.abc import Callable

from bieznia.winapi import is_key_down

VK_CODES = {f"F{i}": 0x6F + i for i in range(1, 13)}


class Hotkeys:
    def __init__(
        self, bindings: dict[str, str | int], key_down: Callable[[int], bool] = is_key_down
    ) -> None:
        """bindings: akcja -> nazwa klawisza ("F8") albo kod wirtualny (0x77)."""
        self._vk: dict[str, int] = {}
        for action, key in bindings.items():
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
        self._was_down = {action: key_down(vk) for action, vk in self._vk.items()}

    def poll(self) -> set[str]:
        """Zwraca akcje, których klawisz został właśnie wciśnięty."""
        pressed: set[str] = set()
        for action, vk in self._vk.items():
            down = self._key_down(vk)
            if down and not self._was_down[action]:
                pressed.add(action)
            self._was_down[action] = down
        return pressed
