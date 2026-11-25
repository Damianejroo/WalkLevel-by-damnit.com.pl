"""Okno aplikacji i przejścia: ramka okna, strony z zasłoną (przenikanie zrzutów),
nagłówki stron. Bez logiki aplikacji - tylko jak okno wygląda i się porusza."""

from __future__ import annotations

import os
import sys
import time
from pathlib import Path
from typing import Any

from PySide6.QtCore import QCoreApplication, QEvent, QRectF, Qt, QVariantAnimation
from PySide6.QtGui import (
    QCloseEvent,
    QColor,
    QFont,
    QIcon,
    QImage,
    QPainter,
    QPaintEvent,
    QPixmap,
    QWheelEvent,
)
from PySide6.QtWidgets import (
    QHBoxLayout,
    QScrollBar,
    QStackedWidget,
    QVBoxLayout,
    QWidget,
)

from bieznia import (
    TITLE,
)
from bieznia.gui import theme as th
from bieznia.gui.theme import Theme
from bieznia.gui.widgets import (
    Motion,
    ease,
    icon_button,
    label,
)

NO_FOCUS_ENV = "BIEZNIA_BEZ_FOKUSU"
TEST_SCREEN_ENV = "BIEZNIA_EKRAN_TESTOW"


def app_icon(t: Theme) -> QIcon:
    png = Path(__file__).with_name("ikona.png")
    if png.exists():
        return QIcon(str(png))
    pix = QPixmap(64, 64)
    pix.fill(Qt.GlobalColor.transparent)
    p = QPainter(pix)
    p.setRenderHint(QPainter.RenderHint.Antialiasing)
    p.setBrush(QColor(t.accent))
    p.setPen(Qt.PenStyle.NoPen)
    p.drawRoundedRect(0, 0, 64, 64, 14, 14)
    font = QFont(th.ICON_FONT)
    font.setPixelSize(38)
    p.setFont(font)
    p.setPen(QColor(t.on_accent))
    p.drawText(pix.rect(), Qt.AlignmentFlag.AlignCenter, th.ICONS["walk"])
    p.end()
    return QIcon(pix)


def tab_pressed() -> bool:
    """Czy klawisz Tab jest teraz wciśnięty (poza Windows: zakładamy, że tak)."""
    if sys.platform != "win32":
        return True
    from bieznia.winapi import is_key_down

    return bool(is_key_down(0x09))


class Window(QWidget):
    def __init__(self, t: Theme, on_close: Any) -> None:
        super().__init__()
        self.t = t
        self.on_close = on_close
        self.backdrop = False
        self.setWindowTitle(TITLE)
        self.setWindowIcon(app_icon(t))
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        self.setMinimumSize(420, 640)
        if os.environ.get(NO_FOCUS_ENV) == "1":  # testy i zrzuty: okno nie zabiera fokusu
            self.setAttribute(Qt.WidgetAttribute.WA_ShowWithoutActivating)
            self.setWindowFlag(Qt.WindowType.WindowDoesNotAcceptFocus)
        self.tab_at = float("-inf")  # kiedy ostatnio fokus przeszedł klawiszem Tab

    def focusNextPrevChild(self, next: bool) -> bool:
        """Tab i Shift+Tab z każdej kontrolki dochodzą tutaj - stąd obwódka fokusu wie, że
        fokus zmieniła klawiatura. Qt woła to samo, gdy chowa widok z fokusem (zmiana strony
        po kliknięciu) - wtedy Tab nie jest wciśnięty i obwódki nie ma."""
        if tab_pressed():
            self.tab_at = time.monotonic()
        return super().focusNextPrevChild(next)

    def retheme(self, t: Theme) -> None:
        self.t = t
        if sys.platform == "win32" and self.isVisible():
            ok = th.apply_backdrop(int(self.winId()), t.name == "dark")  # pasek tytułu też
            self.backdrop = self.backdrop or ok
        self.update()

    def showEvent(self, e: Any) -> None:
        if not self.backdrop:
            self.backdrop = th.apply_backdrop(int(self.winId()), self.t.name == "dark")
            self.update()
        super().showEvent(e)

    def paintEvent(self, _e: QPaintEvent) -> None:
        p = QPainter(self)
        bg = QColor(self.t.bg)
        if self.backdrop:
            bg.setAlphaF(self.t.backdrop_alpha)  # przez tło prześwituje materiał Windows
        p.fillRect(self.rect(), bg)

    def closeEvent(self, e: QCloseEvent) -> None:
        if self.on_close():
            e.accept()
        else:
            e.ignore()


class Cover(QWidget):
    """Przenikanie między dwoma zrzutami okna na czas zmiany.

    capture() zapamiętuje to, co widać; pod spodem nowy widok buduje się, układa i przewija;
    reveal() robi zrzut gotowego wyniku i miesza oba obrazy piksel w piksel (z tą samą
    przezroczystością co okno nad tłem Windows), po czym znika. Żywe widżety nigdy nie jeżdżą
    po ekranie i nic nie mruga - także odcień półprzezroczystego tła okna.
    """

    def __init__(self, window: QWidget, area: QWidget) -> None:
        super().__init__(window)
        self.area = area  # obszar przykrywany (widoki pod paskiem tytułu)
        self.old = QPixmap()
        self.new = QPixmap()
        self.progress = 0.0
        self.band: tuple[int, int, int] | None = None  # (góra karty, wys. stara, wys. nowa)
        self.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
        self.anim = QVariantAnimation(self)
        self.anim.setStartValue(0.0)
        self.anim.setEndValue(1.0)
        self.anim.setEasingCurve(ease(th.EASE_SMOOTH))
        self.anim.valueChanged.connect(self._step)
        self.anim.finished.connect(self._done)
        self.hide()

    def _grab(self) -> QPixmap:
        window = self.parentWidget()
        assert window is not None
        return window.grab(self.area.geometry())

    def capture(self, _background: str = "") -> None:
        """Zrzut obecnego wyglądu - razem z ewentualnym trwającym przenikaniem."""
        self.anim.stop()
        self.old = self._grab()
        self.new = QPixmap()
        self.progress = 0.0
        self.band = None
        self.setGeometry(self.area.geometry())
        self.raise_()
        self.show()

    def reveal(self, ms: int, _shift: int = 0, snappy: bool = False) -> None:
        """snappy: krzywa „wejścia” - większość zmiany od razu (np. motyw), zamiast łagodnego
        startu, przy którym całe okno długo wisi w pół drogi między starym a nowym."""
        if not self.isVisible():
            return
        self.anim.setEasingCurve(ease(th.EASE_IN if snappy else th.EASE_SMOOTH))
        if ms <= 0 or not Motion.enabled:
            self._done()
            return
        self.hide()  # zrzut nowego stanu bez przykrycia
        self.new = self._grab()
        self.show()
        self.raise_()
        self.anim.stop()
        self.anim.setDuration(ms)
        self.anim.start()

    def morph(
        self, top: int, old_h: int, new_h: int, ms: int, span: tuple[int, int] | None = None
    ) -> None:
        """Zmiana wysokości fragmentu (np. karty od y=top) bez przeliczania układu w klatkach.

        Nad fragmentem okno zostaje żywe (np. jadące podświetlenie przełącznika); fragment
        przenika ze starego w nowy i odsłania się do nowej wysokości; wszystko pod nim
        zjeżdża jako obraz. Każda klatka to tylko kopiowanie obrazów - płynnie także przy
        dużych widokach.
        """
        self.anim.setEasingCurve(ease(th.EASE_SMOOTH))
        if not self.isVisible():
            return
        if ms <= 0 or not Motion.enabled:
            self._done()
            return
        self.hide()
        self.new = self._grab()
        # wiersze, które się nie zmieniły, zostają żywe (bez przenikania) - animacja dotyczy
        # dopiero pierwszego wiersza, który wygląda inaczej
        # wiersze pod zmianą (np. kolejne karty) tylko zjeżdżają - liczą się do części „pod
        # fragmentem”, nie do przenikania (inaczej stare i nowe położenie nakładają się)
        rows = RowCompare(self.old, self.new, span)
        same = rows.head(top, min(old_h, new_h))
        top, old_h, new_h = top + same, old_h - same, new_h - same
        tail = rows.tail(top, old_h, new_h)
        self.band = (top, old_h - tail, new_h - tail)
        self.show()
        self.raise_()
        self.anim.stop()
        self.anim.setDuration(ms)
        self.anim.start()

    def _step(self, value: Any) -> None:
        self.progress = float(value)
        if self.band is not None:
            self.update(0, self.band[0], self.width(), self.height() - self.band[0])
        else:
            self.update()

    def _done(self) -> None:
        self.anim.stop()
        self.hide()
        self.old = QPixmap()
        self.new = QPixmap()
        self.band = None

    def _paint_band(self, p: QPainter) -> None:
        assert self.band is not None
        top, h0, h1 = self.band
        width, height = self.width(), self.height()
        prog = self.progress
        h = round(h0 + (h1 - h0) * prog)
        dpr = self.old.devicePixelRatio()

        def src(y: float, rows: float) -> QRectF:  # wiersze obrazu (piksele urządzenia)
            return QRectF(0, y * dpr, width * dpr, rows * dpr)

        p.setCompositionMode(QPainter.CompositionMode.CompositionMode_Source)
        # tło karty pod fragmentem: wiersz tuż nad nim (sama karta) rozciągnięty w dół
        p.drawPixmap(QRectF(0, top, width, h), self.new, src(top - 1, 1))
        # stary obraz w pełni, nowy na nim z kryciem p (tryb Source = (1-p)*stary + p*nowy):
        # jasność nigdy nie spada w połowie - wcześniej stary gasł, zanim pojawił się nowy,
        # i cały fragment na chwilę ciemniał (mignięcie przy każdym kliknięciu)
        # wiersze wspólne dla obu wysokości: stary w pełni, nowy z kryciem p; wiersze, które
        # znikają (zwijanie) - stary gaśnie do tła karty; wiersze, które dochodzą - nowy z tła
        both = min(h, h0, h1)
        p.drawPixmap(QRectF(0, top, width, both), self.old, src(top, both))
        if h0 > both and h > both:
            gone = min(h, h0) - both
            p.setOpacity(1.0 - prog)
            p.drawPixmap(QRectF(0, top + both, width, gone), self.old, src(top + both, gone))
        shown = min(h, h1)
        p.setOpacity(prog)
        p.drawPixmap(QRectF(0, top, width, shown), self.new, src(top, shown))
        p.setOpacity(1.0)
        # wszystko pod fragmentem: przesuwa się razem z jego dolną krawędzią
        below, start = (self.old, top + h0) if h1 > h0 else (self.new, top + h1)
        rows = height - start
        p.drawPixmap(QRectF(0, top + h, width, rows), below, src(start, rows))

    def paintEvent(self, _e: QPaintEvent) -> None:
        if self.old.isNull():
            return
        if self.band is not None and not self.new.isNull():
            self._paint_band(QPainter(self))
            return
        # stary obraz zastępuje okno, nowy na nim z kryciem p: tryb Source z kryciem to
        # dokładnie (1-p)*stary + p*nowy, także przezroczystość tła okna; rysowane wprost
        # na ekran - bez tworzenia nowego obrazu w każdej klatce
        p = QPainter(self)
        p.setCompositionMode(QPainter.CompositionMode.CompositionMode_Source)
        p.drawPixmap(0, 0, self.old)
        if not self.new.isNull() and self.new.size() == self.old.size() and self.progress > 0:
            p.setOpacity(self.progress)
            p.drawPixmap(0, 0, self.new)


class CalmScrollBar(QScrollBar):
    """Pasek przewijania, który nie rysuje się, gdy nie ma czego przewijać - także gdy do
    przewinięcia zostaje tylko dolny margines (cała treść jest już widoczna)."""

    MARGIN_ONLY = 16

    def margin_only(self) -> bool:
        return self.maximum() - self.minimum() <= self.MARGIN_ONLY

    def paintEvent(self, e: QPaintEvent) -> None:
        if not self.margin_only():
            super().paintEvent(e)

    def wheelEvent(self, e: QWheelEvent) -> None:
        """Kółko nie przesuwa strony o sam margines (treść cała widać, pasek się nie rysuje)."""
        if self.margin_only():
            self.setValue(self.minimum())
            e.accept()
            return
        super().wheelEvent(e)


class RowCompare:
    """Porównanie wierszy dwóch zrzutów (w zakresie kolumn span - bez paska przewijania, który
    zmienia się razem z wysokością strony). Wyniki w px logicznych, z 1 px zapasu."""

    def __init__(self, old: QPixmap, new: QPixmap, span: tuple[int, int] | None = None) -> None:
        self.ok = not old.isNull() and not new.isNull() and old.size() == new.size()
        self.dpr = old.devicePixelRatio() or 1.0
        if not self.ok:
            return
        a = old.toImage().convertToFormat(QImage.Format.Format_ARGB32)
        b = new.toImage().convertToFormat(QImage.Format.Format_ARGB32)
        self.rows = a.height()
        self.line = a.bytesPerLine()
        x0, x1 = (0, a.width()) if span is None else span
        self.x0 = max(0, int(x0 * self.dpr)) * 4  # ARGB32: 4 bajty na piksel
        self.x1 = min(a.width(), int(x1 * self.dpr)) * 4
        self.a, self.b = a, b  # obrazy żyją, póki żyją widoki bajtów
        self.bits_a = memoryview(a.constBits())
        self.bits_b = memoryview(b.constBits())

    def equal(self, ya: int, yb: int) -> bool:
        ra = slice(ya * self.line + self.x0, ya * self.line + self.x1)
        rb = slice(yb * self.line + self.x0, yb * self.line + self.x1)
        return self.bits_a[ra] == self.bits_b[rb]

    def head(self, top: int, limit: int) -> int:
        """Ile wierszy od y=top jest takich samych na obu zrzutach."""
        if not self.ok or limit <= 0:
            return 0
        first = max(0, int(top * self.dpr))
        last = min(self.rows, int((top + limit) * self.dpr))
        y = first
        while y < last and self.equal(y, y):
            y += 1
        return max(0, int((y - first) / self.dpr) - 1)

    def tail(self, top: int, old_h: int, new_h: int) -> int:
        """Ile wierszy na dole fragmentu jest takich samych, licząc od dołu: stara część
        kończy się na top+old_h, nowa na top+new_h (wiersze pod zmianą tylko się przesunęły)."""
        if not self.ok:
            return 0
        end_a = int((top + old_h) * self.dpr)
        end_b = int((top + new_h) * self.dpr)
        limit = int(min(old_h, new_h) * self.dpr)
        n = 0
        while n < limit:
            ya, yb = end_a - 1 - n, end_b - 1 - n
            # wiersz poza oknem na którymś zrzucie: nie widać go, więc tylko się przesuwa
            if ya < self.rows and yb < self.rows and not self.equal(ya, yb):
                break
            n += 1
        return max(0, int(n / self.dpr) - 1)


def same_rows(
    old: QPixmap, new: QPixmap, top: int, limit: int, span: tuple[int, int] | None = None
) -> int:
    """Ile wierszy (px logicznych) od y=top jest identycznych na obu zrzutach."""
    return RowCompare(old, new, span).head(top, limit)


def settle_layout(widget: QWidget) -> None:
    """Układy (także pola przewijania) przeliczone od razu, a nie przy następnej klatce."""
    for _ in range(4):
        QCoreApplication.sendPostedEvents(None, QEvent.Type.LayoutRequest)
    widget.updateGeometry()


class Pages(QStackedWidget):
    """Przejście między widokami: krótki przesuw (panel), bez przenikania.

    Styl (motyw) jest na każdym widoku osobno i trafia na widok dopiero, gdy ten się pokazuje -
    zmiana motywu przestylowuje tylko widoczny ekran (kilkadziesiąt ms zamiast pół sekundy).
    """

    def __init__(self) -> None:
        super().__init__()
        self.cover: Cover | None = None
        self.background = "#000000"  # kolor tła okna pod zrzutem przejścia
        self.qss = ""
        self.qss_version = 0
        self.qss_scale: object = None
        self.currentChanged.connect(lambda i: self.style_page(self.widget(i)))

    def set_qss(self, qss: str, everywhere: bool = False) -> None:
        """everywhere: zmiana wielkości tekstu - układ każdego widoku liczy się od nowa."""
        self.qss = qss
        self.qss_version += 1
        if everywhere:
            for i in range(self.count()):
                self.style_page(self.widget(i))
        else:
            self.style_page(self.currentWidget())

    def style_page(self, page: QWidget | None) -> None:
        if page is not None and page.property("qss_version") != self.qss_version:
            page.setStyleSheet(self.qss)
            page.setProperty("qss_version", self.qss_version)

    def go(self, index: int, back: bool = False) -> None:
        """Nowy widok od razu na miejscu, stary (zrzut) przenika w niego z lekkim odjazdem."""
        if index == self.currentIndex():
            return
        cover = self.cover if (Motion.enabled and self.isVisible()) else None
        if cover is not None:
            cover.capture(self.background)
        self.setCurrentIndex(index)
        if cover is not None:
            page = self.currentWidget()
            if page is not None:
                settle_layout(page)
            cover.reveal(th.T_PANEL_IN)


def page_layout(widget: QWidget) -> QVBoxLayout:
    widget.setObjectName("page")
    lay = QVBoxLayout(widget)
    lay.setContentsMargins(th.PAD, th.PAD, th.PAD, th.PAD)
    lay.setSpacing(th.GAP)
    return lay


def header(title: str, on_back: Any) -> QHBoxLayout:
    row = QHBoxLayout()
    back = icon_button("back", "Wróć")
    back.clicked.connect(on_back)
    row.addWidget(back)
    row.addWidget(label(title, "title"))
    row.addStretch(1)
    return row
