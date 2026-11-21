"""Kolory osób i profili gier: 20 par odcieni (ciemny motyw, jasny motyw).

Każdy odcień przechodzi kontrast 4,5:1 z tłem motywu i z tekstem na kolorze
(liczone z czystego koloru: na ciemnym rozjaśniany, na jasnym przyciemniany).
Mięta to dotychczasowy kolor aplikacji.
"""

from __future__ import annotations

import colorsys
from dataclasses import dataclass


@dataclass(frozen=True)
class Kolor:
    name: str
    hue: int  # stopnie, do dopasowania koloru ikony
    dark: str
    light: str

    def for_theme(self, theme_name: str) -> str:
        """Odcień pod motyw - do tekstu i akcentów (kontrast na tle okna)."""
        return self.dark if theme_name == "dark" else self.light

    @property
    def swatch(self) -> str:
        """Żywy kolor do kółek palety, awatarów i wykresów - taki sam w obu motywach,
        żeby 20 kolorów było naprawdę różnych (jasny motyw przyciemnia je tylko w tekście)."""
        return self.dark


KOLORY: tuple[Kolor, ...] = (
    Kolor("Czerwień", 0, "#fa3838", "#e70606"),
    Kolor("Koral", 10, "#f34523", "#dc2e0c"),
    Kolor("Pomarańcz", 25, "#f96b06", "#bf5305"),
    Kolor("Morela", 32, "#df8620", "#a56318"),
    Kolor("Banan", 52, "#f9d806", "#847303"),
    Kolor("Limonka", 80, "#a4ec13", "#587f0a"),
    Kolor("Zieleń", 118, "#26df20", "#178613"),
    Kolor("Szmaragd", 145, "#19e66f", "#0f8540"),
    Kolor("Mięta", 158, "#8fc2b0", "#2f5d50"),
    Kolor("Morski", 174, "#13ecd6", "#0b8276"),
    Kolor("Niebo", 196, "#0db5f2", "#097ba5"),
    Kolor("Błękit", 212, "#2384f3", "#0c6fe1"),
    Kolor("Chaber", 226, "#5c7ff2", "#1346ec"),
    Kolor("Indygo", 242, "#7c78f0", "#2019e6"),
    Kolor("Fiolet", 258, "#956df3", "#5413ec"),
    Kolor("Lawenda", 270, "#a16dd5", "#7f39c6"),
    Kolor("Orchidea", 292, "#cf46e5", "#c01fd8"),
    Kolor("Fuksja", 312, "#f312c6", "#d30bab"),
    Kolor("Róż", 330, "#f42f91", "#df0c75"),
    Kolor("Malina", 345, "#f0426e", "#e21246"),
)
DOMYSLNY = "Mięta"
_BY_NAME = {k.name: k for k in KOLORY}

# Tekst na kolorze: ciemny na jasnych odcieniach (ciemny motyw), biały na ciemnych (jasny).
ON_DARK_THEME = "#0f1f1a"
ON_LIGHT_THEME = "#ffffff"
# Bieg, gdy kolor profilu jest blisko bursztynu biegu (żółcie, pomarańcze).
RUN_ALT = _BY_NAME["Malina"]


def kolor(name: str | None) -> Kolor:
    return _BY_NAME.get(name or "", _BY_NAME[DOMYSLNY])


def names() -> list[str]:
    return [k.name for k in KOLORY]


def clashes_with_run(k: Kolor) -> bool:
    """Żółcie i pomarańcze zlewałyby się z bursztynem strefy biegu."""
    return 10 <= k.hue <= 70


def nearest(hue: float) -> Kolor:
    """Kolor palety najbliższy odcieniowi (Mięta pomijana - to kolor ikon bez koloru)."""

    def dist(k: Kolor) -> float:
        d = abs(k.hue - hue) % 360
        return min(d, 360 - d)

    return min((k for k in KOLORY if k.name != DOMYSLNY), key=dist)


def from_rgb_pixels(pixels: list[tuple[int, int, int, int]]) -> Kolor:
    """Kolor z pikseli ikony (r, g, b, alfa): najczęstszy nasycony odcień -> najbliższy z palety.

    Gdy kolorowych pikseli jest mniej niż 8% (szara, czarno-biała ikona) - Mięta.
    """
    bins = [0.0] * 36
    opaque = colored = 0
    for r, g, b, a in pixels:
        if a < 128:
            continue
        opaque += 1
        h, light, s = colorsys.rgb_to_hls(r / 255, g / 255, b / 255)
        if s < 0.28 or light < 0.12 or light > 0.92:
            continue
        colored += 1
        bins[int(h * 36) % 36] += s * (1 - abs(light - 0.5))
    if not opaque or colored / opaque < 0.08:
        return kolor(DOMYSLNY)
    return nearest((bins.index(max(bins)) + 0.5) * 10)
