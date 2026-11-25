"""Wspólne dla testów: okna testowe nie zabierają fokusu i otwierają się na drugim ekranie
(jeśli jest) - można dalej pracować obok."""

import os

os.environ.setdefault("BIEZNIA_BEZ_FOKUSU", "1")
os.environ.setdefault("BIEZNIA_EKRAN_TESTOW", "1")

import urllib.request
from typing import Any

import pytest


@pytest.fixture(autouse=True)
def no_real_network(monkeypatch: pytest.MonkeyPatch) -> None:
    """Testy nigdy nie łączą się z prawdziwymi serwerami (np. zgłoszenia trafiłyby do autora).
    Test, który potrzebuje odpowiedzi, podmienia urlopen sam."""

    def blocked(*_a: Any, **_k: Any) -> Any:
        raise OSError("Sieć zablokowana w testach")

    monkeypatch.setattr(urllib.request, "urlopen", blocked)


@pytest.fixture(autouse=True)
def no_real_sound(monkeypatch: pytest.MonkeyPatch) -> list[tuple[Any, int]]:
    """Testy nie grają dźwięków; zagrane trafiają na listę (do sprawdzenia w teście)."""
    played: list[tuple[Any, int]] = []
    try:
        import winsound
    except ImportError:
        return played
    monkeypatch.setattr(winsound, "PlaySound", lambda sound, flags: played.append((sound, flags)))
    return played
