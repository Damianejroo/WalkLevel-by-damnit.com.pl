"""Progi kontrastu z zasad Warsztatu - dla obu motywów."""

import pytest

from bieznia.gui.theme import DARK, LIGHT, Theme, contrast


@pytest.mark.parametrize("t", [DARK, LIGHT], ids=lambda t: t.name)
def test_contrast_thresholds(t: Theme) -> None:
    for background in (t.bg, t.surface):
        assert contrast(t.text, background) >= 7.0
        assert contrast(t.text_2, background) >= 4.5
        assert contrast(t.control, background) >= 3.0  # ramka kontrolki
        assert contrast(t.danger, background) >= 4.5
        assert contrast(t.run, background) >= 4.5
        assert contrast(t.accent, background) >= 3.0  # łuk prędkościomierza, przełącznik
    assert contrast(t.on_accent, t.accent) >= 4.5
    assert contrast(t.contrast_fg, t.contrast_bg) >= 7.0
    assert contrast(t.text, t.surface_2) >= 7.0  # tekst na najechanym przycisku


def test_contrast_math() -> None:
    assert contrast("#000000", "#ffffff") == pytest.approx(21.0)
    assert contrast("#777777", "#777777") == pytest.approx(1.0)
