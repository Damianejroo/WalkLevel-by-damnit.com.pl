"""Wysyłanie klawiszy: zawartość zdarzenia SendInput i prawdziwy test na polu tekstowym."""

from __future__ import annotations

import ctypes
import sys
import time

import pytest

from bieznia.winapi import (
    INJECT_TAG,
    INPUT_KEYBOARD,
    KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP,
    KEYEVENTF_SCANCODE,
    key_input,
    send_scancode,
)


def test_key_event_carries_the_key() -> None:
    # Regresja z 0.2.1/0.2.2: klawisz był zapisany obok unii, Windows dostawał wScan = 0.
    e = key_input(0x11, down=True)
    assert e.type == INPUT_KEYBOARD
    assert e.u.ki.wScan == 0x11
    assert e.u.ki.dwFlags == KEYEVENTF_SCANCODE
    assert e.u.ki.dwExtraInfo == INJECT_TAG
    up = key_input(0x48, down=False, extended=True)
    assert up.u.ki.wScan == 0x48
    assert up.u.ki.dwFlags == KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP | KEYEVENTF_EXTENDEDKEY


@pytest.mark.skipif(sys.platform != "win32", reason="SendInput tylko na Windows")
def test_send_input_really_types_into_a_text_field() -> None:
    """Prawdziwe naciśnięcie W trafia do pola tekstowego (wymaga pulpitu Windows)."""
    from PySide6.QtWidgets import QApplication, QLineEdit

    app = QApplication.instance() or QApplication([])
    if QApplication.platformName() != "windows":
        pytest.skip("wymaga prawdziwego okna Windows (QT_QPA_PLATFORM=windows)")
    field = QLineEdit()
    field.show()
    field.activateWindow()
    field.raise_()
    field.setFocus()
    for _ in range(20):
        app.processEvents()
        time.sleep(0.02)
    if ctypes.windll.user32.GetForegroundWindow() != int(field.winId()):
        field.close()
        pytest.skip("okno testowe nie dostało pierwszego planu")
    send_scancode(0x11, True)
    send_scancode(0x11, False)
    for _ in range(20):
        app.processEvents()
        time.sleep(0.02)
    typed = field.text()
    field.close()
    assert typed == "w"
