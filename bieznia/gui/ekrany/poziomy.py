"""Poziomy i osiągnięcia: karta gracza na ekranie głównym, lista osiągnięć, szlak i popupy.

Stan liczony z historii osoby (bez symulatora) przy starcie i po każdym zapisie sesji.
Raz zdobyte osiągnięcia zapisują się w historii i nie znikają.
"""

from __future__ import annotations

import contextlib
import logging
import time
from datetime import datetime
from typing import TYPE_CHECKING, Any

from PySide6.QtCore import QPoint, QPointF, Qt, QTimer
from PySide6.QtGui import QColor, QPainter, QPaintEvent, QPen
from PySide6.QtWidgets import QHBoxLayout, QScrollArea, QVBoxLayout, QWidget

from bieznia import poziomy
from bieznia.gui.odznaki import (
    RARITY_COLORS,
    RARITY_NAMES,
    Hex,
    Panel,
    XPBar,
    big_num,
    caps,
    hex_path,
    name_label,
    rank_rarity,
    soft,
)
from bieznia.gui.popupy import PopupData, PopupStack, play_sound
from bieznia.gui.theme import Theme
from bieznia.gui.widgets import button, label

if TYPE_CHECKING:
    from bieznia.gui.app import App

log = logging.getLogger(__name__)

MAX_POPUPS_AT_ONCE = 4  # więcej nowych naraz - jeden zbiorczy popup


def fmt_int(n: float) -> str:
    return f"{n:,.0f}".replace(",", " ")


def fmt_km(km: float) -> str:
    if km >= 100:
        return fmt_int(km)
    return f"{km:.1f}".replace(".", ",").removesuffix(",0")


def ago(at: float, now: float | None = None) -> str:
    now = time.time() if now is None else now
    days = int((now - at) // 86400)
    if days <= 0:
        return "dzisiaj"
    if days == 1:
        return "wczoraj"
    if days < 14:
        return f"{days} dni temu"
    return datetime.fromtimestamp(at).strftime("%d.%m.%Y")


def progress_label(p: poziomy.Progress) -> str:
    a = p.achievement
    if a.kind in ("dystans", "sesja_km", "gra_km"):
        return f"{fmt_km(p.current)} / {fmt_km(p.target)} KM"
    if a.kind == "dzien_kroki":
        return f"{fmt_int(p.current)} / {fmt_int(p.target)} KROKÓW"
    if a.kind == "seria":
        return f"{int(p.current)} / {int(p.target)} DNI"
    if a.kind == "gry_km":
        return f"{int(p.current)} / {int(p.target)} GRY"
    return ""


class Rail(QWidget):
    """Kawałek pionowej ścieżki szlaku z węzłem: done / here / next."""

    def __init__(self, t: Theme, kind: str, color: str, top: bool, bottom: bool) -> None:
        super().__init__()
        self.t, self.kind, self.color, self.top, self.bottom = t, kind, color, top, bottom
        self.setFixedWidth(26)

    def paintEvent(self, _e: QPaintEvent) -> None:
        t = self.t
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        x, h = self.width() / 2, self.height()
        lit, dim = QColor(t.accent), QColor(t.control)
        if self.top:
            p.setPen(QPen(lit if self.kind in ("done", "here") else dim, 3))
            p.drawLine(QPointF(x, 0), QPointF(x, h / 2))
        if self.bottom:
            p.setPen(QPen(lit if self.kind == "done" else dim, 3))
            p.drawLine(QPointF(x, h / 2), QPointF(x, h))
        if self.kind == "here":
            p.setPen(Qt.PenStyle.NoPen)
            for rr, al in ((11, 50), (8, 90)):
                g = QColor(lit)
                g.setAlpha(al)
                p.setBrush(g)
                p.drawEllipse(QPointF(x, h / 2), rr, rr)
            p.setBrush(lit)
            p.drawEllipse(QPointF(x, h / 2), 5, 5)
        else:
            done = self.kind == "done"
            p.setPen(QPen(QColor(self.color) if done else dim, 2))
            p.setBrush(QColor(self.color) if done else QColor(t.surface_2))
            p.drawPath(hex_path(QPointF(x, h / 2), 7))
        p.end()


class LevelsScreen:
    """Mixin aplikacji: poziomy, rangi i osiągnięcia."""

    levels_status: poziomy.Status | None = None
    levels_key: tuple[Any, ...] | None = None
    popups: PopupStack | None = None
    level_card: Panel | None = None

    # --- liczenie ---------------------------------------------------------------------

    def level_definitions(self: App) -> poziomy.Definitions:
        return poziomy.load()

    def levels_person(self: App) -> str:
        return self.store.person.id if self.store is not None else ""

    def level_friends(self: App) -> int:
        friends = getattr(self, "online", {}).get("friends")
        return len(friends) if isinstance(friends, list) else 0

    def compute_levels(self: App, person: str) -> poziomy.Status | None:
        history = self.history
        if history is None or not person:
            return None
        defs = self.level_definitions()
        walks = poziomy.walks_from_sessions(history.sessions(person=person))
        earned = history.earned(person)
        return poziomy.evaluate(defs, walks, friends=self.level_friends(), earned_before=earned)

    def levels_tick(self: App) -> None:
        """Wołane z pętli okna co kilka sekund: przelicza, gdy historia albo osoba się zmieniły."""
        history = self.history
        if history is None:
            return
        try:
            key = (self.levels_person(), history.version())
        except Exception:  # baza chwilowo niedostępna - spróbujemy później
            return
        if key != getattr(self, "levels_key", None):
            self.levels_key = key
            self.refresh_levels(popups=True)

    def refresh_levels(self: App, popups: bool = True) -> None:
        person = self.levels_person()
        try:
            status = self.compute_levels(person)
        except Exception:  # poziomy nie mogą zatrzymać sterowania
            log.exception("Poziomy i osiągnięcia")
            return
        self.levels_status = status
        history = self.history
        if status is None or history is None:
            self.render_level_card()
            return
        defs = status.definitions
        stored = history.earned(person)
        new = [p for p in status.earned if p.achievement.id not in stored]
        if new:
            history.add_earned(
                person, {p.achievement.id: p.earned_at or time.time() for p in new}, defs.version
            )
        shown = history.shown_level(person)
        history.set_shown_level(person, status.level)
        self.render_level_card()
        if popups and bool(self.settings.get("achievement_popups", True)):
            if shown is None:  # pierwsze liczenie: jedno podsumowanie zamiast stosu
                if status.earned or status.level > 1:
                    self.show_history_summary(status)
            else:
                self.show_new(status, new, shown)

    # --- popupy -----------------------------------------------------------------------

    def popup_stack(self: App) -> PopupStack:
        if getattr(self, "popups", None) is None:
            self.popups = PopupStack(self.t)
        assert self.popups is not None
        return self.popups

    def achievement_popup(self: App, p: poziomy.Progress) -> PopupData:
        a = p.achievement
        defs = self.level_definitions()
        return PopupData(
            kicker=f"OSIĄGNIĘCIE · {RARITY_NAMES[a.rarity].upper()}",
            title=a.name,
            line=f"+{fmt_int(defs.rarity_xp.get(a.rarity, 0))} XP  ·  {a.condition}",
            color=RARITY_COLORS[a.rarity],
            rarity=a.rarity,
            mark=a.mark,
        )

    def level_popup(self: App, status: poziomy.Status) -> PopupData:
        rank = status.rank
        new_rank = rank.level == status.level
        return PopupData(
            kicker=f"AWANS · POZIOM {status.level}",
            title=rank.name if new_rank else f"Poziom {status.level}",
            line=(
                f"Nowa ranga · następny poziom za {fmt_km(status.km_to_next_level)} km"
                if new_rank
                else f"Następny poziom za {fmt_km(status.km_to_next_level)} km"
            ),
            color=self.t.accent,
            rarity=rank_rarity(status.level),
            badge_text=str(status.level),
            progress=status.level_fraction,
        )

    def show_new(
        self: App, status: poziomy.Status, new: list[poziomy.Progress], shown: int
    ) -> None:
        items: list[PopupData] = []
        if len(new) > MAX_POPUPS_AT_ONCE:
            best = max(new, key=lambda p: status.definitions.rarities.index(p.achievement.rarity))
            items.append(self.achievement_popup(best))
            items.append(
                PopupData(
                    kicker="OSIĄGNIĘCIA",
                    title=f"+{len(new) - 1} kolejnych",
                    line="Wszystkie na liście osiągnięć",
                    color=self.t.accent,
                    rarity="rzadkie",
                    mark="korona",
                )
            )
        else:
            new.sort(key=lambda p: p.earned_at or 0)
            items.extend(self.achievement_popup(p) for p in new)
        if status.level > shown:
            items.append(self.level_popup(status))
        if not items:
            return
        stack = self.popup_stack()
        for item in items:
            stack.push(item)
        if bool(self.settings.get("achievement_sound", False)):
            play_sound("level" if status.level > shown else "achievement")

    def show_history_summary(self: App, status: poziomy.Status) -> None:
        n = len(status.earned)
        word = (
            "osiągnięcie"
            if n == 1
            else "osiągnięcia"
            if 2 <= n % 10 <= 4 and not 12 <= n % 100 <= 14
            else "osiągnięć"
        )
        self.popup_stack().push(
            PopupData(
                kicker="Z TWOJEJ HISTORII",
                title=f"Poziom {status.level} · {status.rank.name}",
                line=f"{n} {word} · {fmt_km(status.km)} km przebyte",
                color=self.t.accent,
                rarity=rank_rarity(status.level),
                badge_text=str(status.level),
            )
        )

    def show_sample_popup(self: App) -> None:
        defs = self.level_definitions()
        a = next(x for x in defs.achievements if x.id == "km483")
        self.popup_stack().push(self.achievement_popup(poziomy.Progress(a, time.time())))
        sound = bool(self.settings.get("achievement_sound", False))
        if sound:
            play_sound("achievement")
        status = self.levels_status
        if status is None:
            return

        def level_up() -> None:  # po chwili przykład awansu (i jego dźwięk)
            with contextlib.suppress(RuntimeError):
                self.popup_stack().push(self.level_popup(status))
                if sound:
                    play_sound("level")

        QTimer.singleShot(1400, level_up)

    # --- karta gracza na ekranie głównym -------------------------------------------------

    def build_level_card(self: App) -> Panel:
        t = self.t
        card = Panel(t, t.accent, 70, stripe=False)
        card.make_clickable("Poziom i osiągnięcia")
        card.clicked.connect(self.open_achievements)
        lay = QHBoxLayout(card)
        lay.setContentsMargins(10, 6, 14, 8)
        lay.setSpacing(12)
        self.level_hex = Hex(t, "pospolite", size=62, text="1")
        lay.addWidget(self.level_hex)
        col = QVBoxLayout()
        col.setSpacing(2)
        top = QHBoxLayout()
        self.level_caption = caps("POZIOM 1", t.accent, 12, 2.0)
        top.addWidget(self.level_caption)
        top.addStretch(1)
        self.level_xp = caps("", t.text_2, 12, 0.6)
        top.addWidget(self.level_xp)
        col.addLayout(top)
        self.level_rank = name_label("", 18)
        col.addWidget(self.level_rank)
        self.level_bar = XPBar(t, 0.0, t.accent, 12)
        col.addWidget(self.level_bar)
        self.level_hint = soft(t, "")
        col.addWidget(self.level_hint)
        lay.addLayout(col, 1)
        trophies = QVBoxLayout()
        trophies.setSpacing(0)
        trophies.addWidget(
            Hex(t, "legendarne", "korona", size=40), 0, Qt.AlignmentFlag.AlignHCenter
        )
        self.level_count = caps("0/0", t.text, 13, 0.5)
        trophies.addWidget(self.level_count, 0, Qt.AlignmentFlag.AlignHCenter)
        lay.addLayout(trophies)
        self.level_card = card
        return card

    def render_level_card(self: App) -> None:
        card = getattr(self, "level_card", None)
        if card is None:
            return
        status = getattr(self, "levels_status", None)
        card.setVisible(status is not None)
        if status is None:
            return
        level = status.level
        t = self.t
        card.color = t.accent  # kolor profilu mógł się zmienić
        self.level_bar.color = t.accent
        style = self.level_caption.styleSheet()  # caps(): styl zaczyna się od koloru
        self.level_caption.setStyleSheet(f"color: {t.accent};" + style.split(";", 1)[1])
        self.level_hex.rarity = rank_rarity(level)
        self.level_hex.text = str(level)
        self.level_hex.update()
        self.level_caption.setText(f"POZIOM {level}")
        if level >= status.definitions.max_level:
            self.level_xp.setText(f"{fmt_int(status.xp)} XP")
            self.level_hint.setText("Najwyższy poziom. Szacunek.")
        else:
            self.level_xp.setText(f"{fmt_int(status.xp)} / {fmt_int(status.level_ceiling)} XP")
            self.level_hint.setText(
                f"Jeszcze {fmt_km(status.km_to_next_level)} km do poziomu {level + 1}"
            )
        self.level_rank.setText(status.rank.name)
        self.level_bar.set_value(status.level_fraction)
        got = len(status.earned)
        self.level_count.setText(f"{got}/{len(status.progress)}")
        card.setToolTip(f"{status.rank.name}: {status.rank.about}")

    # --- osiągnięcia --------------------------------------------------------------------

    def achievement_row(self: App, p: poziomy.Progress, compact: bool = False) -> Panel:
        t = self.t
        a = p.achievement
        color = RARITY_COLORS[a.rarity]
        row = Panel(t, color if p.earned else None, 46)
        lay = QHBoxLayout(row)
        lay.setContentsMargins(12, 8, 14, 9)
        lay.setSpacing(12)
        badge = Hex(t, a.rarity, a.mark, not p.earned, None if p.earned else p.fraction, size=58)
        lay.addWidget(badge, 0, Qt.AlignmentFlag.AlignTop)
        col = QVBoxLayout()
        col.setSpacing(1)
        head = QHBoxLayout()
        head.addWidget(name_label(a.name, 16), 1)
        head.addWidget(
            caps(RARITY_NAMES[a.rarity].upper(), color, 13, 1.4), 0, Qt.AlignmentFlag.AlignTop
        )
        col.addLayout(head)
        if not compact:
            col.addWidget(soft(t, a.flavor, italic=True))
        col.addWidget(soft(t, a.condition))
        label = progress_label(p)
        if not p.earned and label:
            bar = QHBoxLayout()
            bar.addWidget(XPBar(t, p.fraction, color, 8, ticks=1), 1)
            bar.addSpacing(8)
            bar.addWidget(caps(label, t.text_2, 12, 0.3))
            col.addLayout(bar)
        defs = self.level_definitions()
        foot = [f"+{fmt_int(defs.rarity_xp.get(a.rarity, 0))} XP"]
        if p.earned and p.earned_at:
            foot.insert(0, f"zdobyte {ago(p.earned_at)}")
        col.addWidget(caps("  ·  ".join(foot), t.text_2, 12, 0.3, bold=False))
        lay.addLayout(col, 1)
        row.setAccessibleName(
            f"{a.name}: {'zdobyte' if p.earned else 'do zdobycia'}. {a.condition}"
        )
        return row

    def rarity_strip(self: App, status: poziomy.Status) -> Panel:
        t = self.t
        strip = Panel(t, None)
        lay = QHBoxLayout(strip)
        lay.setContentsMargins(12, 8, 12, 8)
        for rarity in status.definitions.rarities:
            items = [p for p in status.progress.values() if p.achievement.rarity == rarity]
            got = sum(1 for p in items if p.earned)
            box = QHBoxLayout()
            box.setSpacing(6)
            box.addWidget(Hex(t, rarity, locked=got == 0, size=34, text=str(got)))
            txt = QVBoxLayout()
            txt.setSpacing(0)
            txt.addWidget(caps(RARITY_NAMES[rarity].upper(), RARITY_COLORS[rarity], 10, 1.2))
            txt.addWidget(caps(f"{got} z {len(items)}", t.text_2, 11, 0.3))
            box.addLayout(txt)
            lay.addLayout(box, 1)
        return strip

    def levels_page_key(self: App, kind: str) -> tuple[Any, ...] | None:
        """Odcisk stanu strony osiągnięć/szlaku: ten sam - strona gotowa, bez przebudowy."""
        status = self.levels_status
        if status is None:
            return None
        return (
            kind,
            self.levels_person(),
            round(status.km, 2),
            status.xp,
            tuple(sorted(p.achievement.id for p in status.earned)),
            self.t.name,
            self.t.accent,
            datetime.now().date().isoformat(),  # „zdobyte wczoraj” zmienia się z dniem
        )

    def cached_levels_page(self: App, kind: str, index: int) -> bool:
        key = self.levels_page_key(kind)
        cache: dict[str, Any] = self.__dict__.setdefault("levels_pages", {})
        if key is not None and cache.get(kind) == key:
            self.pages.go(index)
            return True
        return False

    def remember_levels_page(self: App, kind: str) -> None:
        self.__dict__.setdefault("levels_pages", {})[kind] = self.levels_page_key(kind)

    def open_achievements(self: App) -> None:
        self.refresh_levels(popups=False)
        status = self.levels_status
        if status is None:
            return
        if self.cached_levels_page("achievements", self.ACHIEVEMENTS):
            return
        got = len(status.earned)
        page, lay = self.sticky_page(f"Osiągnięcia · {got} z {len(status.progress)}")
        lay.addWidget(self.rarity_strip(status))
        trail_btn = button(f"Twój szlak · {fmt_km(status.km)} km")
        trail_btn.clicked.connect(self.open_trail)
        lay.addWidget(trail_btn)
        earned = status.earned
        closest = status.closest(3)
        if earned:
            lay.addWidget(label("Ostatnio zdobyte", "section"))
            for p in earned[:3]:
                lay.addWidget(self.achievement_row(p))
        if closest:
            lay.addWidget(label("Najbliżej", "section"))
            for p in closest:
                lay.addWidget(self.achievement_row(p))
        shown = {p.achievement.id for p in [*earned[:3], *closest]}
        rest = [p for p in status.progress.values() if p.achievement.id not in shown]
        rest.sort(key=lambda p: (not p.earned, -p.fraction))
        if rest:
            lay.addWidget(label("Pozostałe", "section"))
            for p in rest:
                lay.addWidget(self.achievement_row(p, compact=not p.earned))
        lay.addStretch(1)
        self.replace_page(self.ACHIEVEMENTS, page)
        self.remember_levels_page("achievements")
        self.pages.go(self.ACHIEVEMENTS)

    # --- szlak -------------------------------------------------------------------------------

    def trail_row(
        self: App,
        km: float,
        p: poziomy.Progress | None,
        kind: str,
        top: bool,
        bottom: bool,
        next_km: float = 0.0,
    ) -> QWidget:
        t = self.t
        w = QWidget()
        row = QHBoxLayout(w)
        row.setContentsMargins(0, 0, 0, 0)
        row.setSpacing(10)
        km_lbl = caps(f"{fmt_km(km)} km", t.text if kind != "next" else t.text_2, 14, 0.3)
        km_lbl.setFixedWidth(84)
        km_lbl.setAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
        row.addWidget(km_lbl)
        if p is None:
            row.addWidget(Rail(t, "here", t.accent, top, bottom))
            box = Panel(t, t.accent, 60, stripe=True)
            bl = QHBoxLayout(box)
            bl.setContentsMargins(12, 6, 12, 6)
            bl.addWidget(caps("TU JESTEŚ", t.accent, 12, 1.8))
            bl.addStretch(1)
            if next_km > 0:
                left = next_km - km
                bl.addWidget(
                    caps(
                        f"następny przystanek za {fmt_km(left)} km"
                        f" · ok. {max(1, round(left / 5))} dni przy 5 km dziennie",
                        t.text_2,
                        12,
                        0.3,
                        bold=False,
                    )
                )
            row.addWidget(box, 1)
            w.setFixedHeight(44)
            return w
        a = p.achievement
        color = RARITY_COLORS[a.rarity]
        row.addWidget(Rail(t, kind, color, top, bottom))
        box = Panel(t, color if kind == "done" else None, 40)
        bl = QHBoxLayout(box)
        bl.setContentsMargins(10, 5, 12, 5)
        bl.setSpacing(10)
        bl.addWidget(Hex(t, a.rarity, a.mark, kind != "done", size=44))
        txt = QVBoxLayout()
        txt.setSpacing(0)
        hd = QHBoxLayout()
        hd.addWidget(name_label(a.name, 15), 1)
        hd.addWidget(caps(RARITY_NAMES[a.rarity].upper(), color, 13, 1.2))
        txt.addLayout(hd)
        txt.addWidget(soft(t, a.flavor, italic=kind == "done"))
        bl.addLayout(txt, 1)
        row.addWidget(box, 1)
        return w

    def scroll_trail_to(self: App, row: QWidget | None) -> None:
        """„Tu jesteś” mniej więcej w jednej trzeciej wysokości okna - widać, co już za Tobą
        i co przed Tobą."""
        page = self.pages.widget(self.TRAIL)
        area = page.findChild(QScrollArea) if page is not None else None
        inner = area.widget() if area is not None else None
        if row is None or area is None or inner is None:
            return
        with contextlib.suppress(RuntimeError):
            y = row.mapTo(inner, QPoint(0, 0)).y()
            bar = area.verticalScrollBar()
            bar.setValue(max(0, min(bar.maximum(), y - area.viewport().height() // 3)))

    def open_trail(self: App) -> None:
        status = self.levels_status
        if status is None:
            return
        if self.cached_levels_page("trail", self.TRAIL):
            return
        t = self.t
        page, lay = self.sticky_page("Twój szlak", back=self.ACHIEVEMENTS)
        trail = status.definitions.trail
        done = sum(1 for a in trail if (a.km or 0) <= status.km)
        head = Panel(t, t.accent, 50, stripe=False)
        hl = QHBoxLayout(head)
        hl.setContentsMargins(14, 8, 14, 8)
        hl.addWidget(big_num(f"{fmt_km(status.km)} km", t.text, 24))
        hl.addWidget(soft(t, f"przebyte łącznie · {done} z {len(trail)} przystanków szlaku"), 1)
        lay.addWidget(head)
        lay.setSpacing(4)
        here_placed = False
        here_row: QWidget | None = None
        upcoming = [a for a in trail if (a.km or 0) > status.km]
        next_km = float(upcoming[0].km or 0) if upcoming else 0.0
        for i, a in enumerate(trail):
            km = float(a.km or 0)
            if not here_placed and km > status.km:
                here_row = self.trail_row(status.km, None, "here", i > 0, True, next_km)
                lay.addWidget(here_row)
                here_placed = True
            kind = "done" if km <= status.km else "next"
            lay.addWidget(
                self.trail_row(
                    km, status.progress[a.id], kind, i > 0 or here_placed, i < len(trail) - 1
                )
            )
        if not here_placed:
            here_row = self.trail_row(status.km, None, "here", True, False)
            lay.addWidget(here_row)
        lay.addStretch(1)
        self.replace_page(self.TRAIL, page)
        self.remember_levels_page("trail")
        QTimer.singleShot(0, lambda: self.scroll_trail_to(here_row))
        self.pages.go(self.TRAIL)


class OnlineLevels:
    """Poziom i ranga przy osobach na ekranach online. Serwer odsyła pole „poziom” (liczba);
    przy własnych wierszach, dopóki serwer go nie liczy, pokazujemy poziom z aplikacji."""

    def row_level(self: App, row: dict[str, Any], mine: bool = False) -> int | None:
        value = row.get("poziom")
        if isinstance(value, (int, float)) and not isinstance(value, bool) and value >= 1:
            return min(int(value), poziomy.load().max_level)
        status = self.levels_status
        return status.level if mine and status is not None else None

    def level_chip(self: App, level: int, size: int = 34) -> Hex:
        badge = Hex(self.t, rank_rarity(level), size=size, text=str(level))
        rank = poziomy.load().rank(level)
        badge.setToolTip(f"Poziom {level} · {rank.name}")
        badge.setAccessibleName(f"Poziom {level}, ranga {rank.name}")
        return badge

    def rank_caption(self: App, level: int) -> QWidget:
        rank = poziomy.load().rank(level)
        return caps(rank.name.upper(), RARITY_COLORS[rank_rarity(level)], 11, 1.2)

    def person_achievements(self: App, lay: QVBoxLayout, p: dict[str, Any], mine: bool) -> None:
        """Zdobyte osiągnięcia osoby, od najnowszych, z „osiagniecia_lista” z serwera. Przy
        własnym profilu, dopóki serwer nie odsyła listy, z aplikacji."""
        from bieznia.gui.ekrany.ustawienia import section

        defs = poziomy.load()
        got: list[poziomy.Progress] = []
        items = p.get("osiagniecia_lista")
        if isinstance(items, list):
            for item in items:
                a = defs.by_id(str(item.get("id", ""))) if isinstance(item, dict) else None
                if a is None:
                    continue
                at = item.get("zdobyte_utc_s")
                ok = isinstance(at, (int, float)) and not isinstance(at, bool) and at > 0
                got.append(poziomy.Progress(a, earned_at=float(at) if ok else time.time()))
        elif mine and self.levels_status is not None:
            got = [pr for pr in self.levels_status.progress.values() if pr.earned]
        else:
            return
        got.sort(key=lambda pr: pr.earned_at or 0.0, reverse=True)
        c, cl = section(f"Osiągnięcia · {len(got)} z {len(defs.achievements)}")
        if not got:
            cl.addWidget(label("Jeszcze bez osiągnięć.", "small"))
        limit = 4
        rows: list[QWidget] = []
        for i, pr in enumerate(got):
            row = self.achievement_row(pr, compact=True)
            cl.addWidget(row)  # najpierw rodzic: widoczny widżet bez rodzica to osobne okno
            if i >= limit:
                row.hide()
            rows.append(row)
        if len(got) > limit:
            more = button(f"Pokaż wszystkie ({len(got)})")

            def expand() -> None:
                for row in rows:
                    row.setVisible(True)
                more.hide()

            more.clicked.connect(expand)
            cl.addWidget(more)
        lay.addWidget(c)

    def level_panel(
        self: App, level: int, xp: int | None = None, earned: int | None = None
    ) -> Panel:
        """Karta poziomu na profilu osoby (i własnym)."""
        t = self.t
        defs = poziomy.load()
        rank = defs.rank(level)
        color = RARITY_COLORS[rank_rarity(level)]
        card = Panel(t, color, 55, stripe=False)
        lay = QHBoxLayout(card)
        lay.setContentsMargins(12, 10, 16, 12)
        lay.setSpacing(14)
        lay.addWidget(self.level_chip(level, 70))
        col = QVBoxLayout()
        col.setSpacing(3)
        col.addWidget(caps(f"POZIOM {level}", color, 12, 2.0))
        col.addWidget(name_label(rank.name, 20))
        if xp is not None:
            lo = defs.xp_for(level)
            hi = defs.xp_for(level + 1) if level < defs.max_level else lo
            col.addWidget(XPBar(t, 1.0 if hi <= lo else (xp - lo) / (hi - lo), color, 12))
        info = [rank.about]
        if earned is not None:
            info.append(f"osiągnięcia: {earned}")
        col.addWidget(soft(t, " · ".join(info)))
        lay.addLayout(col, 1)
        return card
