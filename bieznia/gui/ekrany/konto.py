"""Konto online (opcjonalne): logowanie, zakładanie konta, znajomi, ranking, osoby z kontem.

Bez logowania funkcji online nie widać - w Ustawieniach jest tylko krótka karta z opisem.
Wszystko sieciowe idzie w wątku w tle (run_task), wynik wraca do kolejki zadań okna, więc
okno nie zamiera przy wolnym internecie. Ostatnie dane z serwera trzymamy w pamięci - widok
pokazuje je od razu, a świeże podmienia po chwili.
"""

from __future__ import annotations

import contextlib
import logging
import time
from collections.abc import Callable
from typing import TYPE_CHECKING, Any

from PySide6.QtCore import Qt, QUrl
from PySide6.QtGui import QDesktopServices
from PySide6.QtWidgets import QHBoxLayout, QLabel, QLineEdit, QVBoxLayout, QWidget

from bieznia import konto
from bieznia.gui import theme as th
from bieznia.gui.ekrany.historia import steps_text, time_text
from bieznia.gui.ekrany.ustawienia import section
from bieznia.gui.widgets import HoldButton, Segmented, Switch, button, fmt_num, label

if TYPE_CHECKING:
    from bieznia.gui.app import App

log = logging.getLogger("bieznia")

UPLOAD_EVERY_S = 120.0
LIVE_EVERY_S = 5.0
REFRESH_EVERY_S = 24 * 3600.0  # token ważny 7 dni - odświeżany przy starcie i co dobę
TABS = [("friends", "Znajomi"), ("ranking", "Ranking"), ("people", "Osoby"), ("me", "Konto")]
VISIBILITY_LABELS = [("prywatna", "Tylko ja"), ("znajomi", "Znajomi"), ("publiczna", "Wszyscy")]
PROFILE_LABELS = [("ukryty", "Ukryty"), ("znajomi", "Znajomi"), ("publiczny", "Wszyscy")]
SENT_TEXT = (
    "Na serwer trafia: e-mail, pseudonim i sesje jednej osoby z tego komputera (data, kroki,"
    " dystans, czas ruchu, model bieżni). Nie trafia: nazwy osób i gier, ustawienia, kolory,"
    " sesje z symulatora. Bez konta nic nie jest wysyłane."
)


def field(placeholder: str, secret: bool = False) -> QLineEdit:
    edit = QLineEdit()
    edit.setPlaceholderText(placeholder)
    edit.setAccessibleName(placeholder)
    edit.setFixedHeight(th.CONTROL_H)
    if secret:
        edit.setEchoMode(QLineEdit.EchoMode.Password)
    return edit


class AccountScreen:
    accounts: konto.Accounts
    account_new_person: bool
    online: dict[str, Any]
    online_tab: str
    account_mode: str
    account_status: QLabel
    account_busy: set[str]

    # --- start i pętla ---------------------------------------------------------------------

    def init_account(self: App) -> None:
        adres = str(self.config.get("konto", {}).get("adres", ""))
        try:
            self.accounts = konto.Accounts(adres)
        except konto.KontoError as exc:
            log.warning("Konto online wyłączone: %s", exc)
            self.accounts = konto.Accounts("")
        self.online = {}
        self.online_tab = "friends"
        self.account_mode = "login"
        self.account_new_person = False
        self.account_busy = set()
        self.account_upload_at = time.monotonic() + 20  # pierwsza wysyłka chwilę po starcie
        self.account_live_at = 0.0
        self.account_live_on = False
        self.account_refresh_at = time.monotonic() + REFRESH_EVERY_S
        for acc in self.accounts.logged_in():
            self.account_job("refresh", acc.refresh, quiet=True, person=acc.state.person)

    @property
    def account(self: App) -> konto.Account:
        """Konto wybranej osoby (wylogowane, gdy ta osoba nie ma konta)."""
        store = self.store
        return self.accounts.get(store.person.id if store is not None else "")

    def online_visible(self: App) -> bool:
        return self.accounts.enabled and self.account.state.logged_in

    def account_tick(self: App, now: float, speed: float, steps: int | None, active: float) -> None:
        if not self.accounts.enabled:
            return
        if now >= self.account_refresh_at:
            self.account_refresh_at = now + REFRESH_EVERY_S
            for acc in self.accounts.logged_in():
                self.account_job("refresh", acc.refresh, quiet=True, person=acc.state.person)
        if now >= self.account_upload_at and self.history is not None:
            self.account_upload_at = now + UPLOAD_EVERY_S
            self.account_upload_all()
        account = self.account  # na żywo - tylko osoba, która teraz idzie
        if not account.state.logged_in or not account.state.live or now < self.account_live_at:
            return
        self.account_live_at = now + LIVE_EVERY_S
        current = self.tracker.current if self.tracker is not None else None
        moving = current is not None and speed > 0.05
        if moving and current is not None:
            self.account_live_on = True
            sid = current.id
            self.account_job(
                "live", lambda: account.live(speed, steps or 0, active, sid), quiet=True
            )
        elif self.account_live_on:
            self.account_live_on = False
            self.account_job("live", account.clear_live, quiet=True)

    def account_job(
        self: App,
        action: str,
        func: Callable[[], Any],
        quiet: bool = False,
        person: str | None = None,
    ) -> None:
        """Zadanie sieciowe w tle; to samo zadanie tej samej osoby nie idzie drugi raz naraz."""
        if person is None:
            person = self.store.person.id if self.store is not None else ""
        key = f"{person}:{action}"
        if key in self.account_busy:
            return
        self.account_busy.add(key)

        def work() -> None:
            try:
                self.tasks.put(("konto", (action, True, func(), quiet, person)))
            except konto.KontoError as exc:
                self.tasks.put(("konto", (action, False, exc, quiet, person)))
            except Exception as exc:
                log.exception("Konto: %s", action)
                error = konto.KontoError(str(exc))
                self.tasks.put(("konto", (action, False, error, quiet, person)))

        self.run_task(work)

    def on_account_result(self: App, data: tuple[str, bool, Any, bool, str]) -> None:
        action, ok, value, quiet, person = data
        self.account_busy.discard(f"{person}:{action}")
        mine = self.store is not None and person == self.store.person.id
        if not ok:
            error: konto.KontoError = value
            if isinstance(error, konto.LoggedOut):
                acc = self.accounts.items.get(person)
                if acc is not None and acc.state.logged_in:
                    acc.logout()
                if mine:
                    self.online = {}
                name = self.person_name(person)
                self.toast(f"Logowanie konta osoby {name} wygasło - zaloguj się ponownie.")
                self.account_changed()
                return
            if not quiet and (mine or action in ("login", "register")):
                self.account_message(str(error))
            return
        if action in ("login", "register"):
            self.on_signed_in(value, person)
            return
        if not mine and action not in ("upload", "delete_session"):
            return  # wynik dla innej osoby (przełączono osobę w trakcie) - bez skutku
        if action in ("delete", "logout"):
            self.online = {}
            self.toast("Konto usunięte." if action == "delete" else "Wylogowano.")
            self.account_changed()
            self.go_back(self.SETTINGS)
            return
        if action == "refresh":
            self.account_changed(rebuild_page=False)
            return
        if action == "upload":
            if value and mine and self.pages.currentIndex() == self.ONLINE:
                self.load_online(self.online_tab)
            return
        if action in ("invite_name", "invite_user"):
            who = (value or {}).get("osoba", {}).get("name", "")
            accepted = (value or {}).get("stan") == "przyjeta"
            self.toast(
                f"{who} jest teraz Twoim znajomym."
                if accepted
                else f"Wysłano zaproszenie do {who}."
            )
            self.load_online("friends", force=True)
        elif action == "reg_code":
            self.online["reg_code"] = value
        elif action == "password":
            self.account_message("Hasło zmienione - inne urządzenia zostały wylogowane.")
        elif action == "person":
            self.online["person"] = value
            self.replace_page(self.PERSON, self.build_online_person_page())
            self.pages.go(self.PERSON)
            return
        elif action in ("answer", "remove_friend"):
            message = {"answer": "Gotowe.", "remove_friend": "Usunięto ze znajomych."}[action]
            self.toast(message)
            self.load_online("friends", force=True)
        elif action.startswith("load:"):
            self.online[action[5:]] = value
        elif action == "profile":
            self.account_message("Zapisano.")
        elif action == "export":
            self.account_message(f"Zapisano Twoje dane z serwera: {value}")
        if self.pages.currentIndex() == self.ONLINE and not action.startswith("live"):
            self.render_online()

    def person_name(self: App, person: str) -> str:
        store = self.store
        if store is not None:
            for p in store.people:
                if p.id == person:
                    return p.name
        return "?"

    def on_signed_in(self: App, signed: tuple[konto.Account, Any], person: str) -> None:
        """Zalogowano: konto przypięte do osoby (nowej albo tej, dla której logowano)."""
        acc, user = signed
        store = self.store
        if store is None:
            return
        taken = self.accounts.person_of(user.id)
        if taken and taken != person:
            if acc.state.person:  # osoba z innym kontem - to logowanie cofamy
                acc.logout()
            self.account_message(
                f"To konto jest już przypięte do osoby {self.person_name(taken)} - wybierz ją."
            )
            return
        if not person:  # nowa osoba z konta online
            from bieznia import paleta

            color = paleta.KOLORY[len(store.people) % len(paleta.KOLORY)].name
            new = store.add_person(user.name, color)
            acc = self.accounts.attach(acc, new.id)
            person = new.id
            self.apply_profile()
            log.info("Nowa osoba z konta online: %s", new.name)
        self.online = {}
        self.toast(f"Zalogowano jako {acc.state.name}.")
        self.account_upload_at = time.monotonic()  # wyniki od razu na konto
        self.account_changed()
        self.open_online()

    def account_message(self: App, text: str) -> None:
        with contextlib.suppress(RuntimeError, AttributeError):
            self.account_status.setText(text)
            return
        self.toast(text)

    def account_changed(self: App, rebuild_page: bool = True) -> None:
        """Logowanie, wylogowanie albo nowe dane konta: Ustawienia i ikona na głównym ekranie."""
        self.render_online_button()
        self.refresh_settings()
        if rebuild_page and self.pages.currentIndex() == self.ACCOUNT:
            self.replace_page(self.ACCOUNT, self.build_account_page())

    def render_online_button(self: App) -> None:
        btn = getattr(self, "online_btn", None)
        if btn is not None:
            with contextlib.suppress(RuntimeError):
                btn.setVisible(self.online_visible())

    # --- karta w Ustawieniach --------------------------------------------------------------

    def build_account_card(self: App) -> QWidget | None:
        if not self.account.enabled:
            return None
        state = self.account.state
        who = self.store.person.name if self.store is not None else ""
        c, cl = section(f"Konto online · {who} (opcjonalne)")
        if not state.logged_in:
            cl.addWidget(
                label(
                    "Bez konta wszystko działa na tym komputerze i nic nie jest wysyłane."
                    " Z kontem: wyniki online, znajomi, ranking i lista osób. Konto należy"
                    " do jednej osoby - inne osoby mogą mieć swoje konta albo żadnego.",
                    "small",
                )
            )
            row = QHBoxLayout()
            register = button("Załóż konto")
            register.clicked.connect(lambda: self.open_account("register"))
            login = button("Zaloguj się", "primary")
            login.clicked.connect(lambda: self.open_account("login"))
            row.addWidget(register)
            row.addWidget(login, 1)
            cl.addLayout(row)
            return c
        nick = label(state.name)
        nick.setObjectName("strong")
        cl.addWidget(nick)
        cl.addWidget(label(state.email, "small"))
        open_btn = button("Znajomi, ranking i ustawienia konta", "primary")
        open_btn.clicked.connect(self.open_online)
        cl.addWidget(open_btn)
        return c

    # --- logowanie i zakładanie konta ------------------------------------------------------

    def open_account(self: App, mode: str = "login", new_person: bool | None = None) -> None:
        """Logowanie / nowe konto: dla wybranej osoby albo (new_person) jako nowa osoba."""
        self.account_mode = mode
        if new_person is not None:
            self.account_new_person = new_person
        self.replace_page(self.ACCOUNT, self.build_account_page())
        self.pages.go(self.ACCOUNT)

    def build_account_page(self: App) -> QWidget:
        mode = self.account_mode
        titles = {
            "login": "Logowanie",
            "register": "Nowe konto",
            "reset": "Nie pamiętam hasła",
            "registered": "Potwierdź e-mail",
        }
        page, lay = self.sticky_page(titles.get(mode, "Konto"), back=self.SETTINGS)
        self.account_status = label("", "small")
        self.account_status.setWordWrap(True)
        c, cl = section({"login": "Zaloguj się", "register": "Załóż konto"}.get(mode, titles[mode]))
        if mode == "registered":
            cl.addWidget(
                label(
                    "Konto założone. Kliknij link w e-mailu, który właśnie wysłaliśmy (sprawdź też"
                    " spam), a potem zaloguj się. Bez potwierdzenia funkcje online są zablokowane.",
                    "small",
                )
            )
            to_login = button("Przejdź do logowania", "primary")
            to_login.clicked.connect(lambda: self.open_account("login"))
            cl.addWidget(to_login)
            lay.addWidget(c)
            lay.addStretch(1)
            return page
        code = field("Kod zaproszenia")
        if mode == "register":
            cl.addWidget(
                label(
                    "Konta zakładamy na zaproszenie: kod dostaniesz od kogoś, kto już ma konto.",
                    "small",
                )
            )
            cl.addWidget(code)
        target = (
            "Konto będzie nową osobą w aplikacji (nazwa = pseudonim)."
            if self.account_new_person
            else f"Konto dla osoby: {self.store.person.name if self.store else ''}."
        )
        cl.insertWidget(1, label(target, "small"))
        email = field("E-mail")
        email.setText("" if self.account_new_person else self.account.state.email)
        cl.addWidget(email)
        name = field("Pseudonim (widzą go znajomi)")
        password = field("Hasło", secret=True)
        repeat = field("Powtórz hasło", secret=True)
        if mode == "register":
            cl.addWidget(name)
            cl.addWidget(
                label(
                    "Pseudonim: 3-24 znaki - litery, cyfry, spacja, _, - i kropka."
                    " Nie wpisuj imienia i nazwiska.",
                    "small",
                )
            )
        cl.addWidget(password)
        terms: Switch | None = None
        if mode == "register":
            cl.addWidget(repeat)
            cl.addWidget(label(f"Hasło: co najmniej {konto.PASSWORD_MIN} znaków.", "small"))
        remember = Switch(self.t, "Zapamiętaj logowanie", True)
        self.setting_row(
            cl,
            "Zapamiętaj logowanie",
            "Hasła nie zapisujemy - tylko bezpieczny token w Windows",
            remember,
        )
        if mode == "register":
            terms = Switch(self.t, "Akceptuję regulamin i politykę prywatności")
            self.setting_row(
                cl,
                "Akceptuję regulamin i politykę prywatności",
                f"Wersja z {konto.TERMS_VERSION}",
                terms,
            )
            read = button("Przeczytaj regulamin i politykę prywatności")
            read.setToolTip(self.terms_url())
            read.clicked.connect(lambda: QDesktopServices.openUrl(QUrl(self.terms_url())))
            cl.addWidget(read)
        cl.addWidget(self.account_status)
        submit = button("Zaloguj się" if mode == "login" else "Załóż konto", "primary")
        cl.addWidget(submit)

        new_person = self.account_new_person
        person = "" if new_person or self.store is None else self.store.person.id

        def target_account() -> konto.Account:
            """Logujemy na koncie osoby; nowa osoba - konto tymczasowe, przypinane po sukcesie."""
            acc = self.accounts.blank() if new_person else self.accounts.get(person)
            acc.state.remember = remember.isChecked()
            return acc

        def go() -> None:
            if mode == "login":
                acc = target_account()
                values = (email.text(), password.text())
                self.account_job(
                    "login",
                    lambda: (acc, acc.login(values[0], values[1], person)),
                    person=person,
                )
                self.account_status.setText("Loguję…")
                return
            problem = (
                konto.check_code(code.text())
                or konto.check_name(name.text())
                or konto.check_email(email.text())
                or konto.check_password(password.text())
            )
            if not problem and password.text() != repeat.text():
                problem = "Hasła różnią się."
            if not problem and terms is not None and not terms.isChecked():
                problem = "Zaakceptuj regulamin, żeby założyć konto."
            if problem:
                self.account_status.setText(problem)
                return
            new_acc = target_account()
            form = (code.text(), email.text(), password.text(), name.text(), person)
            self.account_job("register", lambda: (new_acc, new_acc.register(*form)), person=person)
            self.account_status.setText("Zakładam konto…")

        submit.clicked.connect(go)
        for edit in (code, email, password, repeat, name):
            edit.returnPressed.connect(go)
        lay.addWidget(c)
        links = QHBoxLayout()
        if mode == "login":
            forgot = button("Nie pamiętam hasła")
            forgot.setEnabled(False)  # reset mailem ruszy, gdy serwer dostanie skrzynkę nadawczą
            forgot.setToolTip("Wkrótce - reset hasła e-mailem jeszcze nie działa")
            new = button("Załóż konto")
            new.clicked.connect(lambda: self.open_account("register"))
            links.addWidget(forgot)
            links.addWidget(new)
        else:
            have = button("Mam już konto - zaloguj się")
            have.clicked.connect(lambda: self.open_account("login"))
            links.addWidget(have)
        lay.addLayout(links)
        c2, c2l = section("Co trafia na serwer")
        c2l.addWidget(label(SENT_TEXT, "small"))
        lay.addWidget(c2)
        lay.addStretch(1)
        return page

    def terms_url(self: App) -> str:
        return str(
            self.config.get("konto", {}).get("regulamin", "https://bieznia.damnit-live.pl/bieznia")
        )

    # --- konto online: znajomi, ranking, osoby, ustawienia ---------------------------------

    def open_online(self: App) -> None:
        if not self.online_visible():
            self.open_account("login")
            return
        self.replace_page(self.ONLINE, self.build_online_page())
        self.pages.go(self.ONLINE)
        self.load_online(self.online_tab)

    def load_online(self: App, tab: str, force: bool = False) -> None:
        client = self.account.client
        loaders: dict[str, list[tuple[str, Callable[[], Any]]]] = {
            "friends": [
                ("friends", client.friends),
                ("invitations", client.invitations),
                ("stats", lambda: client.stats("tydzien")),
            ],
            "ranking": [("ranking", lambda: client.ranking(self.online.get("scope", "tydzien")))],
            "people": [
                (
                    "people",
                    lambda: client.people(
                        self.online.get("search", ""), int(self.online.get("people_page", 1))
                    ),
                )
            ],
            "me": [],
        }
        for key, func in loaders.get(tab, []):
            if force or key not in self.online or tab == self.online_tab:
                self.account_job(f"load:{key}", func, quiet=key in self.online)

    def set_online_tab(self: App, tab: str) -> None:
        self.online_tab = tab
        self.render_online()
        self.load_online(tab)

    def render_online(self: App) -> None:
        if self.pages.currentIndex() != self.ONLINE:
            return
        self.quietly(lambda: self.replace_page(self.ONLINE, self.build_online_page()))

    def build_online_page(self: App) -> QWidget:
        state = self.account.state
        page, lay = self.sticky_page(f"Konto online · {state.name}", back=self.MAIN)
        tabs = Segmented(TABS, self.online_tab, "Konto online")
        tabs.changed.connect(self.set_online_tab)
        lay.addWidget(tabs)
        self.account_status = label("", "small")
        builder = {
            "friends": self.online_friends,
            "ranking": self.online_ranking,
            "people": self.online_people,
            "me": self.online_me,
        }[self.online_tab]
        builder(lay)
        lay.addWidget(self.account_status)
        lay.addStretch(1)
        return page

    def loading(self: App, key: str) -> bool:
        return key not in self.online

    def online_friends(self: App, lay: QVBoxLayout) -> None:
        stats = self.online.get("stats") or {}
        c, cl = section("Moje wyniki online · ten tydzień")
        if self.loading("stats"):
            cl.addWidget(label("Ładuję…", "small"))
        else:
            cl.addWidget(
                label(
                    f"{steps_text(int(stats.get('kroki', 0)))} kroków · "
                    f"{fmt_num(float(stats.get('dystans_km', 0.0)), 2)} km · "
                    f"{time_text(float(stats.get('ruch_s', 0)))}",
                )
            )
        lay.addWidget(c)

        invitations = self.online.get("invitations") or {}
        incoming = list(invitations.get("przychodzace") or [])
        if incoming:
            c, cl = section("Zaproszenia do Ciebie")
            for inv in incoming:
                row = QHBoxLayout()
                who = (inv.get("osoba") or {}).get("name") or inv.get("name") or "?"
                row.addWidget(label(str(who)), 1)
                no = button("Odrzuć")
                yes = button("Przyjmij", "primary")
                iid = str(inv.get("id", ""))
                no.clicked.connect(
                    lambda _=False, i=iid: self.account_job(
                        "answer", lambda: self.account.client.answer_invitation(i, False)
                    )
                )
                yes.clicked.connect(
                    lambda _=False, i=iid: self.account_job(
                        "answer", lambda: self.account.client.answer_invitation(i, True)
                    )
                )
                row.addWidget(no)
                row.addWidget(yes)
                cl.addLayout(row)
            lay.addWidget(c)

        c, cl = section("Znajomi")
        friends = self.online.get("friends")
        if friends is None:
            cl.addWidget(label("Ładuję…", "small"))
        elif not friends:
            cl.addWidget(
                label(
                    "Jeszcze nikogo. Zaproś znajomego po pseudonimie albo zaproś na Bieżnię"
                    " kogoś, kto nie ma konta.",
                    "small",
                )
            )
        for f in friends or []:
            cl.addWidget(self.friend_row(f))
        lay.addWidget(c)

        c, cl = section("Dodaj znajomego")
        cl.addWidget(
            label("Wpisz pseudonim osoby, która ma konto. Zaproszenie musi przyjąć.", "small")
        )
        row = QHBoxLayout()
        other = field("Pseudonim znajomego")
        add = button("Zaproś", "primary")

        def invite() -> None:
            name = other.text().strip()
            if name:
                self.account_job("invite_name", lambda: self.account.client.invite_by_name(name))

        add.clicked.connect(invite)
        other.returnPressed.connect(invite)
        row.addWidget(other, 1)
        row.addWidget(add)
        cl.addLayout(row)
        sent = list(invitations.get("wyslane") or [])
        if sent:
            cl.addWidget(label("Wysłane - czekają na odpowiedź", "small"))
            for inv in sent:
                line = QHBoxLayout()
                who = (inv.get("osoba") or {}).get("name") or "?"
                line.addWidget(label(str(who)), 1)
                back = button("Wycofaj")
                iid = str(inv.get("id", ""))
                back.clicked.connect(
                    lambda _=False, i=iid: self.account_job(
                        "answer", lambda: self.account.client.withdraw_invitation(i)
                    )
                )
                line.addWidget(back)
                cl.addLayout(line)
        lay.addWidget(c)

        c, cl = section("Zaproś na Bieżnię")
        reg = self.online.get("reg_code") or {}
        if reg.get("kod"):
            new = label(f"Kod na nowe konto: {reg['kod']}")
            new.setObjectName("accent")
            new.setTextInteractionFlags(Qt.TextInteractionFlag.TextSelectableByMouse)
            cl.addWidget(new)
            cl.addWidget(
                label(
                    "Działa raz, przez 14 dni. Ta osoba od razu będzie Twoim znajomym.",
                    "small",
                )
            )
            more = button("Kolejny kod")
            more.clicked.connect(
                lambda: self.account_job("reg_code", self.account.client.registration_code)
            )
            cl.addWidget(more)
        else:
            cl.addWidget(
                label(
                    "Dla kogoś bez konta: kod do wpisania przy zakładaniu konta. Ta osoba od"
                    " razu będzie Twoim znajomym.",
                    "small",
                )
            )
            invite_new = button("Pokaż kod na nowe konto")
            invite_new.clicked.connect(
                lambda: self.account_job("reg_code", self.account.client.registration_code)
            )
            cl.addWidget(invite_new)
        lay.addWidget(c)

    def friend_row(self: App, f: dict[str, Any]) -> QWidget:
        row = QWidget()
        rl = QHBoxLayout(row)
        rl.setContentsMargins(0, 2, 0, 2)
        texts = QVBoxLayout()
        texts.setSpacing(0)
        live = f.get("na_zywo") or None
        fid = str(f.get("id", ""))
        name = button(str(f.get("name", "?")))
        name.setObjectName("link")
        name.setFlat(True)
        name.setStyleSheet(
            f"text-align: left; padding: 0; border: 0; font-weight: 600;"
            f" color: {self.t.accent if live else self.t.text};"
        )
        name.setToolTip("Pokaż profil")
        name.clicked.connect(lambda: self.open_person(fid))
        texts.addWidget(name)
        week = f.get("tydzien") or {}
        if live:
            sub = f"idzie teraz {fmt_num(float(live.get('kmh', 0)))} km/h"
            sub += f" · {steps_text(int(live.get('kroki', 0)))} kroków"
        else:
            sub = f"tydzień: {steps_text(int(week.get('kroki', 0)))} kroków"
            sub += f" · {time_text(float(week.get('ruch_s', 0)))}"
        texts.addWidget(label(sub, "small"))
        rl.addLayout(texts, 1)
        remove = HoldButton("Usuń", self.t)
        remove.confirmed.connect(
            lambda: self.account_job(
                "remove_friend", lambda: self.account.client.remove_friend(fid)
            )
        )
        rl.addWidget(remove)
        return row

    def open_person(self: App, user_id: str) -> None:
        if user_id:
            self.account_job("person", lambda: self.account.client.profile(user_id))

    def people_page(self: App, page_no: int) -> None:
        self.online["people_page"] = max(1, page_no)
        self.online.pop("people", None)
        self.render_online()
        self.load_online("people", force=True)

    def build_online_person_page(self: App) -> QWidget:
        p = self.online.get("person") or {}
        page, lay = self.sticky_page(str(p.get("name", "Profil")), back=self.ONLINE)
        c, cl = section("Profil")
        since = p.get("od_utc_s")
        if since:
            import datetime as dt

            day = dt.datetime.fromtimestamp(float(since)).strftime("%d.%m.%Y")
            cl.addWidget(label(f"Na Bieżni od {day}", "small"))
        if p.get("bieznia_model"):
            cl.addWidget(label(f"Bieżnia: {p['bieznia_model']}", "small"))
        cl.addWidget(label("Znajomy" if p.get("znajomy") else "Nie jesteście znajomymi", "small"))
        lay.addWidget(c)
        for key, title in (("tydzien", "Ostatnie 7 dni"), ("razem", "Razem")):
            totals = p.get(key) or {}
            c, cl = section(title)
            cl.addWidget(
                label(
                    f"{steps_text(int(totals.get('kroki', 0)))} kroków · "
                    f"{fmt_num(float(totals.get('dystans_km', 0.0)), 2)} km · "
                    f"{time_text(float(totals.get('ruch_s', 0)))}",
                )
            )
            if totals.get("sesje") is not None:
                cl.addWidget(label(f"sesje: {int(totals['sesje'])}", "small"))
            lay.addWidget(c)
        lay.addWidget(
            label("Widać tylko sesje, które ta osoba pokazuje Tobie (bez ukrytych).", "small")
        )
        lay.addStretch(1)
        return page

    def online_ranking(self: App, lay: QVBoxLayout) -> None:
        scope = self.online.get("scope", "tydzien")
        seg = Segmented([("tydzien", "Tydzień"), ("miesiac", "Miesiąc")], scope, "Zakres rankingu")

        def change(value: str) -> None:
            self.online["scope"] = value
            self.online.pop("ranking", None)
            self.render_online()
            self.load_online("ranking", force=True)

        seg.changed.connect(change)
        lay.addWidget(seg)
        c, cl = section("Ranking kroków")
        rows = self.online.get("ranking")
        if rows is None:
            cl.addWidget(label("Ładuję…", "small"))
        elif not rows:
            cl.addWidget(label("Na razie nikt nie ma wyników w tym okresie.", "small"))
        for r in rows or []:
            line = QHBoxLayout()
            place = label(f"{int(r.get('miejsce', 0))}.")
            place.setFixedWidth(34)
            who = label(str(r.get("name", "?")))
            who.setObjectName("accent" if r.get("ja") else "strong")
            line.addWidget(place)
            line.addWidget(who, 1)
            line.addWidget(
                label(
                    f"{steps_text(int(r.get('kroki', 0)))} kroków · "
                    f"{fmt_num(float(r.get('dystans_km', 0.0)), 1)} km",
                    "small",
                )
            )
            cl.addLayout(line)
        lay.addWidget(c)
        lay.addWidget(
            label(
                "W rankingu są tylko sesje z bieżni (bez symulatora) widoczne dla wszystkich"
                " albo dla znajomych. Ukryte sesje i profil ukryty nie wchodzą do rankingu.",
                "small",
            )
        )

    def online_people(self: App, lay: QVBoxLayout) -> None:
        row = QHBoxLayout()
        search = field("Szukaj po pseudonimie")
        search.setText(self.online.get("search", ""))
        find = button("Szukaj")

        def go() -> None:
            self.online["search"] = search.text()
            self.online["people_page"] = 1
            self.online.pop("people", None)
            self.render_online()
            self.load_online("people", force=True)

        find.clicked.connect(go)
        search.returnPressed.connect(go)
        row.addWidget(search, 1)
        row.addWidget(find)
        lay.addLayout(row)
        c, cl = section("Osoby z kontem")
        people = self.online.get("people")
        if people is None:
            cl.addWidget(label("Ładuję…", "small"))
        else:
            items = list(people.get("osoby") or [])
            if people.get("nastepna") or int(people.get("strona", 1)) > 1:
                pager = QHBoxLayout()
                page_no = int(people.get("strona", 1))
                prev = button("Poprzednie")
                prev.setEnabled(page_no > 1)
                prev.clicked.connect(lambda: self.people_page(page_no - 1))
                nxt = button("Następne")
                nxt.setEnabled(bool(people.get("nastepna")))
                nxt.clicked.connect(lambda: self.people_page(page_no + 1))
                pager.addWidget(prev)
                pager.addWidget(label(f"strona {page_no}", "small"), 1)
                pager.addWidget(nxt)
                lay.addLayout(pager)
            if not items:
                cl.addWidget(label("Nikogo nie znaleziono.", "small"))
            for p in items:
                line = QHBoxLayout()
                texts = QVBoxLayout()
                texts.setSpacing(0)
                who = label(str(p.get("name", "?")))
                who.setObjectName("strong")
                texts.addWidget(who)
                week = p.get("tydzien") or {}
                sub = f"tydzień: {steps_text(int(week.get('kroki', 0)))} kroków"
                if p.get("miejsce"):
                    sub += f" · {int(p['miejsce'])}. miejsce"
                texts.addWidget(label(sub, "small"))
                line.addLayout(texts, 1)
                if p.get("znajomy"):
                    line.addWidget(label("znajomy", "small"))
                elif p.get("id") != self.account.state.user_id:
                    invite = button("Zaproś")
                    uid = str(p.get("id", ""))
                    invite.clicked.connect(
                        lambda _=False, u=uid: self.account_job(
                            "invite_user", lambda: self.account.client.invite_user(u)
                        )
                    )
                    line.addWidget(invite)
                cl.addLayout(line)
        lay.addWidget(c)
        lay.addWidget(
            label(
                "Na liście są tylko osoby, które same ustawiły profil jako widoczny dla"
                " wszystkich. E-maili nikt nie widzi.",
                "small",
            )
        )

    def online_me(self: App, lay: QVBoxLayout) -> None:
        state, t = self.account.state, self.t
        c, cl = section("Moje wyniki na koncie")
        upload = Switch(t, "Wysyłaj moje sesje", state.upload)
        upload.toggled.connect(self.on_account_upload)
        self.setting_row(cl, "Wysyłaj moje sesje", "Co 2 minuty i po każdej sesji", upload)
        cl.addWidget(label("Kto widzi moje sesje", "small"))
        vis = Segmented(VISIBILITY_LABELS, state.visibility, "Kto widzi moje sesje")
        vis.changed.connect(self.on_account_visibility)
        cl.addWidget(vis)
        cl.addWidget(
            label("Pojedynczą sesję ukryjesz w Historii kroków (zostaje tylko u Ciebie).", "small")
        )
        live = Switch(t, "Na żywo dla znajomych", state.live)
        live.toggled.connect(self.on_account_live)
        self.setting_row(
            cl, "Na żywo dla znajomych", "Znajomi widzą, że idziesz, i Twoje tempo", live
        )
        send = button("Wyślij teraz")
        send.clicked.connect(self.account_upload_now)
        cl.addWidget(send)
        lay.addWidget(c)

        c, cl = section("Profil")
        name = field("Pseudonim")
        name.setText(state.name)
        save = button("Zapisz pseudonim")

        def rename() -> None:
            problem = konto.check_name(name.text())
            if problem:
                self.account_status.setText(problem)
                return
            new = name.text().strip()

            def work() -> Any:
                user = self.account.client.update_profile(name=new)
                self.account.state.name = user.name or new
                self.account.save()
                return user

            self.account_job("profile", work)

        save.clicked.connect(rename)
        row = QHBoxLayout()
        row.addWidget(name, 1)
        row.addWidget(save)
        cl.addLayout(row)
        cl.addWidget(label("Mój profil na liście osób z kontem", "small"))
        prof = Segmented(PROFILE_LABELS, state.profile_visibility, "Widoczność profilu")
        prof.changed.connect(self.on_profile_visibility)
        cl.addWidget(prof)
        lay.addWidget(c)

        c, cl = section("Hasło")
        old = field("Obecne hasło", secret=True)
        new = field("Nowe hasło", secret=True)
        again = field("Powtórz nowe hasło", secret=True)
        for edit in (old, new, again):
            cl.addWidget(edit)
        change = button("Zmień hasło")

        def change_password() -> None:
            if new.text() != again.text():
                self.account_status.setText("Nowe hasła różnią się.")
                return
            values = (old.text(), new.text())
            self.account_job("password", lambda: self.account.change_password(*values))

        change.clicked.connect(change_password)
        cl.addWidget(change)
        cl.addWidget(
            label(
                "Zmiana hasła wylogowuje Cię na innych komputerach (jak „wyloguj wszędzie”).",
                "small",
            )
        )
        lay.addWidget(c)

        c, cl = section("Konto")
        cl.addWidget(label(f"{state.email}", "small"))
        keep = Switch(t, "Zapamiętaj logowanie", state.remember)
        keep.toggled.connect(self.account.set_remember)
        hint = "Wyłączone - wylogowanie przy zamknięciu"
        self.setting_row(cl, "Zapamiętaj logowanie", hint, keep)
        row = QHBoxLayout()
        data = button("Pobierz moje dane")
        data.clicked.connect(self.account_export)
        out = button("Wyloguj")
        out.clicked.connect(lambda: self.account_job("logout", self.account.logout))
        row.addWidget(data)
        row.addWidget(out)
        cl.addLayout(row)
        cl.addWidget(
            label(
                "Usunięcie konta kasuje je z serwera razem z wynikami i znajomościami."
                " Wyniki na tym komputerze zostają. Wpisz hasło i przytrzymaj przycisk.",
                "small",
            )
        )
        password = field("Hasło", secret=True)
        cl.addWidget(password)
        delete = HoldButton("Usuń konto", t)
        delete.confirmed.connect(
            lambda: self.account_job("delete", lambda: self.account.delete_account(password.text()))
        )
        cl.addWidget(delete)
        lay.addWidget(c)

    # --- akcje ustawień konta --------------------------------------------------------------

    def on_account_upload(self: App, on: bool) -> None:
        self.account.state.upload = on
        self.account.save()

    def on_account_visibility(self: App, value: str) -> None:
        if self.history is not None:
            self.account.set_visibility(self.history, value)
            self.account_upload_now()

    def on_account_live(self: App, on: bool) -> None:
        self.account.state.live = on
        self.account.save()
        if not on and self.account_live_on:
            self.account_live_on = False
            self.account_job("live", self.account.clear_live, quiet=True)

    def on_profile_visibility(self: App, value: str) -> None:
        def work() -> Any:
            self.account.client.update_profile(visibility=value)
            self.account.state.profile_visibility = value
            self.account.save()

        self.account_job("profile", work)

    def account_upload_now(self: App) -> None:
        if self.tracker is not None:
            self.tracker.flush()
        self.account_upload_at = time.monotonic() + UPLOAD_EVERY_S
        self.account_upload_all()

    def account_upload_all(self: App) -> None:
        """Każda zalogowana osoba wysyła swoje sesje (osobno, swoim kontem)."""
        history = self.history
        if history is None:
            return
        if self.tracker is not None:
            self.tracker.flush()
        for acc in self.accounts.logged_in():
            self.account_job(
                "upload", lambda a=acc: a.upload(history), quiet=True, person=acc.state.person
            )

    def account_export(self: App) -> None:
        from pathlib import Path

        target = Path("dane") / f"moje-dane-konto-{time.strftime('%Y%m%d-%H%M%S')}.json"

        def work() -> Any:
            import json

            data = self.account.client.my_data()
            target.write_text(json.dumps(data, ensure_ascii=False, indent=1), encoding="utf-8")
            return str(target)

        self.account_job("export", work)
        self.account_message(f"Pobieram do {target}…")
