"""Historia kroków: zakres, liczby, kroki dziennie (wykres), udział profili/osób, ostatnie sesje."""

from __future__ import annotations

import logging
from datetime import date, datetime, timedelta
from typing import TYPE_CHECKING, Any

from PySide6.QtCore import QPointF, QRectF, Qt
from PySide6.QtGui import QColor, QFont, QPainter
from PySide6.QtWidgets import QHBoxLayout, QLabel, QPushButton, QVBoxLayout, QWidget

from bieznia import paleta
from bieznia.gui import komponenty as k
from bieznia.gui.ekrany.ustawienia import section
from bieznia.gui.theme import Theme
from bieznia.gui.widgets import (
    HoldButton,
    Segmented,
    StatTile,
    TileGrid,
    fmt_num,
    icon_button,
    label,
)
from bieznia.historia import Totals

log = logging.getLogger("bieznia")

if TYPE_CHECKING:
    from bieznia.gui.app import App

DAYS_SHORT = ("pn", "wt", "śr", "cz", "pt", "sb", "nd")
RANGES = {"day": 1, "week": 7, "month": 30, "all": 0}


def steps_text(n: int) -> str:
    return f"{n:,}".replace(",", " ")


def time_text(seconds: float) -> str:
    m = int(seconds // 60)
    return f"{m // 60} h {m % 60:02d} min" if m >= 60 else f"{m} min"


def axis_step(peak: float) -> tuple[int, int]:
    """Okrągła podziałka osi (1, 2, 5 x 10^n), najwyżej 5 przedziałów -> (krok, przedziały)."""
    peak = max(peak, 100)
    base = 1
    while True:
        for m in (1, 2, 5):
            step = m * base
            lines = -(-int(peak) // step)
            if lines <= 5:
                return step, max(lines, 2)
        base *= 10


def thousands(v: float) -> str:
    """5000 -> "5", 1500 -> "1,5" (bez zbędnego ",0")."""
    return f"{v / 1000:.0f}" if v % 1000 == 0 else fmt_num(v / 1000)


class DayBars(QWidget):
    """Kroki dziennie: słupki ułożone z kolorów profili (albo osób)."""

    def __init__(
        self,
        t: Theme,
        days: list[tuple[str, dict[str, int]]],
        colors: dict[str, str],
        hourly: bool = False,
    ) -> None:
        super().__init__()
        self.t, self.days, self.colors, self.hourly = t, days, colors, hourly
        self.setFixedHeight(190)
        self.setAccessibleName("Kroki w ciągu dnia" if hourly else "Kroki dziennie")

    def paintEvent(self, _e: Any) -> None:
        t = self.t
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        w, h = self.width(), self.height()
        left, bottom, top = 46, 22, 8
        ph = h - bottom - top
        peak = max([sum(v.values()) for _d, v in self.days] + [1])
        step, lines = axis_step(peak)
        top_val = step * lines
        small = QFont(self.font())
        small.setPixelSize(11)
        p.setFont(small)
        for i in range(lines + 1):
            v = step * i
            y = top + ph * (1 - v / top_val)
            p.setPen(QColor(t.line))
            p.drawLine(QPointF(left, y), QPointF(w, y))
            p.setPen(QColor(t.text_2))
            text = "0" if v == 0 else (f"{thousands(v)} tys." if v >= 1000 else f"{v:.0f}")
            p.drawText(
                QRectF(0, y - 8, left - 8, 16),
                Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter,
                text,
            )
        n = len(self.days)
        slot = (w - left) / max(1, n)
        bar_w = min(30.0, slot * 0.6)
        today = f"{datetime.now().hour:02d}" if self.hourly else date.today().isoformat()
        for i, (day, parts) in enumerate(self.days):
            x = left + slot * i + (slot - bar_w) / 2
            y = top + ph
            for name, v in sorted(parts.items(), key=lambda kv: -kv[1]):
                hh = ph * v / top_val
                p.setPen(Qt.PenStyle.NoPen)
                p.setBrush(QColor(self.colors.get(name, t.accent)))
                p.drawRoundedRect(QRectF(x, y - hh, bar_w, max(1.0, hh - 1.5)), 3, 3)
                y -= hh
            if self.hourly:
                now_hour = int(today)
                if day == today or (i % 3 == 0 and abs(i - now_hour) > 1):
                    p.setPen(QColor(t.text if day == today else t.text_2))
                    p.drawText(
                        QRectF(left + slot * i - 12, h - bottom + 4, slot + 24, 16),
                        Qt.AlignmentFlag.AlignCenter,
                        "teraz" if day == today else f"{int(day)}:00",
                    )
                continue
            if n <= 14 or i % max(1, n // 7) == 0:
                d = datetime.fromisoformat(day)
                txt = (
                    "dziś"
                    if day == today
                    else (DAYS_SHORT[d.weekday()] if n <= 7 else f"{d.day}.{d.month}")
                )
                p.setPen(QColor(t.text if day == today else t.text_2))
                p.drawText(
                    QRectF(left + slot * i - 10, h - bottom + 4, slot + 20, 16),
                    Qt.AlignmentFlag.AlignCenter,
                    txt,
                )


class ShareBar(QWidget):
    def __init__(self, t: Theme, share: float, color: str) -> None:
        super().__init__()
        self.t, self.share, self.color = t, share, color
        self.setFixedHeight(6)

    def paintEvent(self, _e: Any) -> None:
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(QColor(self.t.surface_2))
        p.drawRoundedRect(QRectF(0, 0, self.width(), 6), 3, 3)
        p.setBrush(QColor(self.color))
        p.drawRoundedRect(QRectF(0, 0, self.width() * max(0.0, min(1.0, self.share)), 6), 3, 3)


class HistoryScreen:
    def open_history(self: App) -> None:
        self.close_popups()
        if self.tracker is not None:
            self.tracker.flush()  # bieżąca sesja też w historii
        if getattr(self, "history_built", "") != self.history_stamp():
            self.replace_page(self.HISTORY, self.build_history_page())
            self.history_built = self.history_stamp()
        self.pages.go(self.HISTORY)

    def history_stamp(self: App) -> str:
        """Wszystko, co widać w historii - bez zmian widok nie buduje się od nowa."""
        store = self.store
        return repr(
            (
                self.history.version() if self.history else None,
                self.history_scope,
                self.history_range,
                date.today().isoformat(),
                self.t.name,
                self.t.accent,
                [
                    (
                        p.id,
                        p.name,
                        p.color,
                        [(q.id, q.name, q.color, q.icon_file) for q in p.profiles],
                    )
                    for p in store.people
                ]
                if store
                else None,
                store.active_person if store else None,
            )
        )

    def history_today(self: App, person_id: str) -> int:
        if self.history is None:
            return 0
        if self.tracker is not None:
            self.tracker.flush()  # bieżąca sesja też się liczy
        start = datetime.combine(date.today(), datetime.min.time()).timestamp()
        return self.history.totals(start, person_id).steps

    def build_history_page(self: App) -> QWidget:
        t = self.t
        page, lay = self.sticky_page("Historia kroków")
        store, history = self.store, self.history
        if history is None or store is None:
            lay.addWidget(label("Historia pojawi się po utworzeniu pierwszej osoby.", "muted"))
            lay.addStretch(1)
            return page
        many = len(store.people) > 1
        if many:
            scope = Segmented(
                [("me", "Moja"), ("all", "Wszyscy")], self.history_scope, "Czyja historia"
            )
            scope.changed.connect(lambda v: self.set_history(scope=v))
            lay.addWidget(scope)
        rng = Segmented(
            [("day", "Dziś"), ("week", "7 dni"), ("month", "30 dni"), ("all", "Wszystko")],
            self.history_range,
            "Zakres",
        )
        rng.changed.connect(lambda v: self.set_history(range_=v))
        lay.addWidget(rng)
        everyone = many and self.history_scope == "all"
        person = None if everyone else store.person.id
        days = RANGES[self.history_range]
        since = 0.0
        if days:
            first = date.today() - timedelta(days=days - 1)
            since = datetime.combine(first, datetime.min.time()).timestamp()
        totals = history.totals(since, person)
        tiles = [
            StatTile("Kroki"),
            StatTile("Dystans"),
            StatTile("Czas ruchu"),
            StatTile("Średnio"),
        ]
        for tile, v in zip(
            tiles,
            (
                steps_text(totals.steps),
                f"{fmt_num(totals.distance_km, 2)} km",
                time_text(totals.active_s),
                f"{fmt_num(totals.avg_kmh)} km/h",
            ),
            strict=True,
        ):
            tile.set(v)
        lay.addWidget(TileGrid(*tiles))
        key = "person" if everyone else "profile"
        colors = self.history_colors(key)
        chart_days = days if days > 1 else 7
        if self.history_range == "all":
            chart_days = 30
        hourly = self.history_range == "day"
        c, cl = section("Kroki w ciągu dnia" if hourly else "Kroki dziennie")
        series = history.by_hour(person, key) if hourly else history.by_day(chart_days, person, key)
        if any(parts for _d, parts in series):
            cl.addWidget(DayBars(t, series, colors, hourly=hourly))
            legend = QHBoxLayout()
            legend.setSpacing(10)
            names = sorted({n for _d, parts in series for n in parts})
            for nm in names[:6]:
                dot = QLabel()
                dot.setFixedSize(10, 10)
                dot.setStyleSheet(f"background: {colors.get(nm, t.accent)}; border-radius: 5px;")
                legend.addWidget(dot)
                legend.addWidget(label(nm if len(nm) < 16 else nm[:14] + "…", "small"))
            legend.addStretch(1)
            cl.addLayout(legend)
        else:
            cl.addWidget(
                label(
                    "Brak kroków w tym okresie. Wejdź na bieżnię - wyniki pojawią się tu same.",
                    "muted",
                )
            )
        lay.addWidget(c)
        groups = history.by_group(since, person, key)
        if groups:
            c, gl = section("Osoby" if everyone else "Profile gier")
            total = sum(g.steps for _n, g in groups) or 1
            for name, g in groups:
                gl.addWidget(
                    self.history_group_row(name, g, total, colors.get(name, t.accent), key)
                )
            lay.addWidget(c)
        recent = history.sessions(since, person, limit=8)
        if recent:
            c, sl = section("Ostatnie sesje")
            linked = {a.state.person for a in self.accounts.logged_in()}
            online = bool(linked) and self.history is not None
            hidden = self.history.hidden_online() if online and self.history else set()
            for s in recent:
                when = datetime.fromtimestamp(s.start)
                day = (
                    "Dziś"
                    if when.date() == date.today()
                    else (
                        "Wczoraj"
                        if when.date() == date.today() - timedelta(days=1)
                        else f"{when:%d.%m}"
                    )
                )
                who = f"{s.person_name} · " if everyone else ""
                holder = QPushButton()
                holder.setObjectName("row")
                holder.setCursor(Qt.CursorShape.PointingHandCursor)
                holder.setAccessibleName(f"Sesja {day} {when:%H:%M} - szczegóły")
                holder.setToolTip("Szczegóły sesji")
                holder.clicked.connect(lambda _=False, sid=s.id: self.open_session(sid))
                row = QHBoxLayout(holder)
                row.setContentsMargins(8, 6, 8, 6)
                row.setSpacing(10)
                row.addWidget(k.picture(self.history_icon(s.profile, s.profile_name, 32), 32))
                texts = QVBoxLayout()
                texts.setSpacing(0)
                n = label(f"{day} {when:%H:%M} · {who}{s.profile_name}")
                n.setObjectName("strong")
                texts.addWidget(n)
                texts.addWidget(
                    label(
                        f"{steps_text(s.steps)} kroków · {time_text(s.active_s)}"
                        f" · {fmt_num(s.avg_kmh)} km/h",
                        "small",
                    )
                )
                row.addLayout(texts, 1)
                if online and s.person in linked and s.source == "bieznia":
                    row.addWidget(self.online_toggle(s.id, s.id in hidden))
                for child in holder.findChildren(QLabel):
                    child.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
                holder.setMinimumHeight(56)
                sl.addWidget(holder)
            lay.addWidget(c)
        lay.addStretch(1)
        return page

    # --- szczegóły sesji -------------------------------------------------------------------

    def open_session(self: App, session_id: str) -> None:
        self.session_open = session_id
        self.replace_page(self.SESSION, self.build_session_page())
        self.pages.go(self.SESSION)

    def build_session_page(self: App) -> QWidget:
        s = self.history.session(self.session_open) if self.history else None
        page, lay = self.sticky_page("Sesja", back=self.HISTORY)
        if s is None:
            lay.addWidget(label("Tej sesji już nie ma.", "muted"))
            lay.addStretch(1)
            return page
        start, end = datetime.fromtimestamp(s.start), datetime.fromtimestamp(s.end)
        c, cl = section(f"{start:%d.%m.%Y} · {start:%H:%M}-{end:%H:%M}")
        cl.addWidget(label(f"{s.person_name} · {s.profile_name}", "small"))
        cadence = s.steps / (s.active_s / 60) if s.active_s >= 30 else 0.0
        values = (
            ("Kroki", steps_text(s.steps)),
            ("Dystans", f"{fmt_num(s.distance_km, 2)} km"),
            ("Czas ruchu", time_text(s.active_s)),
            ("Średnio", f"{fmt_num(s.avg_kmh)} km/h"),
            ("Kroki/min", f"{cadence:.0f}" if cadence else "-"),
            ("Cała sesja", time_text(max(0.0, s.end - s.start))),
        )
        tiles = []
        for title, value in values:
            tile = StatTile(title)
            tile.set(value)
            tiles.append(tile)
        cl.addWidget(TileGrid(*tiles[:3]))
        cl.addWidget(TileGrid(*tiles[3:]))
        lay.addWidget(c)

        c, cl = section("Skąd")
        source = "Symulator bieżni" if s.source == "symulator" else (s.device_model or "Bieżnia")
        details = [source]
        if s.firmware:
            details.append(f"firmware {s.firmware}")
        if s.app_version:
            details.append(f"aplikacja {s.app_version}")
        cl.addWidget(label(" · ".join(details), "small"))
        owner = self.accounts.items.get(s.person)
        online = owner is not None and owner.state.logged_in and self.history is not None
        if online and s.source == "bieznia":
            assert self.history is not None
            hidden = s.id in self.history.hidden_online()
            sent = self.history.was_sent(s.id)
            state = (
                "ukryta (tylko u Ciebie)"
                if hidden
                else ("na koncie" if sent else "czeka na wysyłkę")
            )
            row = QHBoxLayout()
            row.addWidget(label(f"Konto online: {state}", "small"), 1)
            row.addWidget(self.online_toggle(s.id, hidden))
            cl.addLayout(row)
        elif s.source == "symulator":
            cl.addWidget(label("Sesje z symulatora nie idą na konto online.", "small"))
        lay.addWidget(c)

        c, cl = section("Usuń")
        cl.addWidget(
            label(
                "Usuwa sesję z historii na tym komputerze"
                + (" i z konta online." if online else ".")
                + " Tego nie da się cofnąć - przytrzymaj przycisk.",
                "small",
            )
        )
        delete = HoldButton("Usuń sesję", self.t)
        delete.confirmed.connect(lambda: self.delete_session(s.id))
        cl.addWidget(delete)
        lay.addWidget(c)
        lay.addStretch(1)
        return page

    def delete_session(self: App, session_id: str) -> None:
        history = self.history
        if history is None:
            return
        tracker = self.tracker
        if tracker is not None and tracker.current is not None and tracker.current.id == session_id:
            tracker.current = None  # trwająca sesja - kończymy bez zapisu
        gone = history.session(session_id)
        history.delete(session_id)
        owner = self.accounts.items.get(gone.person) if gone is not None else None
        if owner is not None and owner.state.logged_in:
            self.account_job(
                "delete_session",
                lambda: owner.delete_session(session_id),
                person=owner.state.person,
            )
        log.info("Usunięto sesję z historii")
        self.toast("Sesja usunięta.")
        self.replace_page(self.HISTORY, self.build_history_page())
        self.history_built = self.history_stamp()
        self.go_back(self.HISTORY)

    def online_toggle(self: App, session_id: str, hidden: bool) -> QPushButton:
        """Oko przy sesji: ukryta zostaje u Ciebie, a na koncie online nikt jej nie widzi."""
        btn = icon_button("eye", "")
        btn.setCheckable(True)

        def show(is_hidden: bool) -> None:
            btn.setChecked(is_hidden)
            text = "Ukryta online - kliknij, żeby pokazać" if is_hidden else "Ukryj online"
            btn.setToolTip(text)
            btn.setAccessibleName(text)
            btn.setStyleSheet("" if is_hidden else f"color: {self.t.accent};")

        def flip() -> None:
            if self.history is None:
                return
            now_hidden = session_id not in self.history.hidden_online()
            self.history.set_hidden_online(session_id, now_hidden)
            show(now_hidden)
            self.account_upload_now()  # zmiana widoczności od razu na serwerze

        show(hidden)
        btn.clicked.connect(flip)
        return btn

    def set_history(self: App, scope: str | None = None, range_: str | None = None) -> None:
        if scope:
            self.history_scope = scope
        if range_:
            self.history_range = range_
        self.replace_page(self.HISTORY, self.build_history_page())
        self.history_built = self.history_stamp()

    def history_colors(self: App, key: str) -> dict[str, str]:
        store = self.store
        out: dict[str, str] = {}
        if store is None:
            return out
        if key == "person":
            for p in store.people:
                out[p.name] = paleta.kolor(p.color).swatch
        else:
            for pr in store.person.profiles:
                out[pr.name] = paleta.kolor(pr.color).swatch
        return out

    def history_icon(self: App, profile_id: str, name: str, size: int) -> Any:
        store = self.store
        if store is not None:
            for person in store.people:
                for p in person.profiles:
                    if p.id == profile_id:
                        return self.profile_pixmap(p, size)
        return k.profile_pixmap(self.t, size, paleta.DOMYSLNY, k.G_PAD, None)

    def history_group_row(
        self: App, name: str, g: Totals, total: int, color: str, key: str
    ) -> QWidget:
        t = self.t
        row = QWidget()
        rl = QHBoxLayout(row)
        rl.setContentsMargins(0, 4, 0, 4)
        rl.setSpacing(12)
        store = self.store
        if key == "person" and store is not None:
            person = next((p for p in store.people if p.name == name), None)
            pix = k.avatar_pixmap(t, 36, name, person.color if person else paleta.DOMYSLNY)
        else:
            profile = (
                next((p for p in store.person.profiles if p.name == name), None) if store else None
            )
            pix = (
                self.profile_pixmap(profile, 36)
                if profile
                else k.profile_pixmap(t, 36, paleta.DOMYSLNY, k.G_PAD, None)
            )
        rl.addWidget(k.picture(pix, 36))
        mid = QVBoxLayout()
        mid.setSpacing(4)
        top = QHBoxLayout()
        n = label(name)
        n.setObjectName("strong")
        top.addWidget(n, 1)
        v = label(f"{steps_text(g.steps)} kroków")
        v.setAlignment(Qt.AlignmentFlag.AlignRight)
        top.addWidget(v)
        mid.addLayout(top)
        mid.addWidget(ShareBar(t, g.steps / total, color))
        mid.addWidget(
            label(
                f"{fmt_num(g.distance_km, 1)} km · {time_text(g.active_s)}"
                f" · {round(g.steps / total * 100)}%",
                "small",
            )
        )
        rl.addLayout(mid, 1)
        return row
