"""Konto online - OPCJONALNY dodatek. Bez logowania aplikacja działa w całości lokalnie, nic
nie wysyła, a funkcji online w ogóle nie widać.

Po zalogowaniu: statystyki z serwera, znajomi (zaproszenia), lista osób z kontem, ranking,
stan na żywo dla znajomych (tylko po włączeniu). Na serwer idą wyłącznie sesje JEDNEJ osoby
powiązanej z kontem - bez nazw osób i profili gier, bez sesji z symulatora. Stan konta
(bez tokenu) w dane/konto.json, token w Menedżerze poświadczeń Windows (sejf.py).
"""

from __future__ import annotations

import contextlib
import json
import logging
import re
import threading
from dataclasses import asdict, dataclass, fields
from pathlib import Path
from typing import Any

from bieznia.historia import COLUMN_NAMES, History, Session
from bieznia.konto.api import (
    Auth,
    Client,
    KontoError,
    LoggedOut,
    Opener,
    UpdateRequired,
    User,
    _default_opener,
)
from bieznia.konto.sejf import TokenStore, default_store

log = logging.getLogger("bieznia")

STATE_FILE = Path("dane") / "konto.json"
VISIBILITY = ("prywatna", "znajomi", "publiczna")
PROFILE_VISIBILITY = ("ukryty", "znajomi", "publiczny")
TERMS_VERSION = "2026-11-25"  # data regulaminu i polityki prywatności konta online
NAME_MIN, NAME_MAX = 3, 24
# Pseudonim jak na serwerze: łacinka z polskimi literami, cyfry, spacja, _, - i kropka, bez
# spacji na brzegach (bez cyrylicy: jej litery wyglądają jak łacińskie - podszywanie się).
NAME_RE = re.compile(r"^[A-Za-z0-9 _.\-ĄĆĘŁŃÓŚŹŻąćęłńóśźż]+$")
RESERVED_NAMES = frozenset({"admin", "administrator", "moderator", "damnit", "bieznia", "bieżnia"})
EMAIL_RE = re.compile(r"^[^@\s]{1,64}@[^@\s]{1,190}\.[^@\s]{2,24}$")
PASSWORD_MIN = 10

# Pola sesji, które idą na serwer (bez osoby i profilu gry - to zostaje u Ciebie).
UPLOAD_FIELDS = (
    "start_utc_s",
    "koniec_utc_s",
    "kroki",
    "dystans_km",
    "ruch_s",
    "bieznia_model",
    "sterownik",
    "bieznia_profil",
    "firmware",
    "wersja_aplikacji",
    "zrodlo",
    "strefa_min",
    "zmieniono_utc_s",
)

__all__ = [
    "Account",
    "AccountState",
    "KontoError",
    "LoggedOut",
    "UpdateRequired",
    "check_email",
    "check_name",
    "check_password",
]


def check_name(name: str) -> str:
    """Pusty tekst = poprawny; inaczej podpowiedź do pola."""
    if name != name.strip():
        return "Pseudonim: bez spacji na początku i na końcu."
    if not NAME_MIN <= len(name) <= NAME_MAX:
        return f"Pseudonim: od {NAME_MIN} do {NAME_MAX} znaków."
    if not NAME_RE.match(name):
        return "Pseudonim: litery, cyfry, spacja, _, - i kropka."
    if name.lower() in RESERVED_NAMES:
        return "Ten pseudonim jest zarezerwowany - wybierz inny."
    return ""


def check_code(code: str) -> str:
    return "" if len(code.strip()) >= 4 else "Wpisz kod zaproszenia (dostaniesz go od znajomego)."


def canonical_uuid(session_id: str) -> str:
    """Zapis dla serwera: małe litery z myślnikami (36 znaków). Lokalnie 32 znaki hex."""
    h = session_id.replace("-", "").lower()
    if len(h) != 32:
        return session_id
    return f"{h[:8]}-{h[8:12]}-{h[12:16]}-{h[16:20]}-{h[20:]}"


def check_email(email: str) -> str:
    return "" if EMAIL_RE.match(email.strip()) else "Wpisz poprawny adres e-mail."


def check_password(password: str) -> str:
    if len(password) < PASSWORD_MIN:
        return f"Hasło: co najmniej {PASSWORD_MIN} znaków."
    return ""


@dataclass
class AccountState:
    """Stan konta zapisany lokalnie (bez tokenu i bez hasła)."""

    user_id: str = ""
    name: str = ""
    email: str = ""
    verified: bool = False
    person: str = ""  # lokalna osoba, której sesje idą na konto
    upload: bool = True
    visibility: str = "znajomi"  # widoczność wysyłanych sesji (ukryte zawsze prywatne)
    live: bool = False  # stan na żywo dla znajomych
    profile_visibility: str = "znajomi"  # czy jestem na liście osób z kontem
    terms: str = ""  # wersja zaakceptowanego regulaminu

    @property
    def logged_in(self) -> bool:
        return bool(self.user_id)


def load_state(path: Path = STATE_FILE) -> AccountState:
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return AccountState()
    known = {f.name for f in fields(AccountState)}
    state = AccountState(**{k: v for k, v in raw.items() if k in known})
    if state.visibility not in VISIBILITY:
        state.visibility = "znajomi"
    if state.profile_visibility not in PROFILE_VISIBILITY:
        state.profile_visibility = "znajomi"
    return state


def save_state(state: AccountState, path: Path = STATE_FILE) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(".tmp")
    tmp.write_text(json.dumps(asdict(state), ensure_ascii=False, indent=1), encoding="utf-8")
    tmp.replace(path)


def session_payload(s: Session, visibility: str) -> dict[str, Any]:
    row = dict(zip(COLUMN_NAMES, s.row(), strict=True))
    out = {k: row[k] for k in UPLOAD_FIELDS}
    out["uuid"] = canonical_uuid(s.id)
    out["widocznosc"] = visibility
    return out


class Account:
    """Konto: logowanie, wysyłka sesji i dane online. Metody sieciowe - z wątku w tle."""

    def __init__(
        self,
        base_url: str,
        store: TokenStore | None = None,
        state_path: Path = STATE_FILE,
        opener: Opener = _default_opener,
    ) -> None:
        self.state_path = state_path
        self.state = load_state(state_path)
        self.store = store if store is not None else default_store()
        self.enabled = bool(base_url.strip())
        self.client = Client(base_url or "https://127.0.0.1", self.store.get(), opener)
        self.lock = threading.Lock()  # wysyłka sesji nie może iść dwa razy naraz
        if self.state.logged_in and not self.client.token:
            self._forget()  # token zniknął z sejfu (inny użytkownik Windows) - wylogowany

    # --- logowanie -------------------------------------------------------------------------

    def _signed_in(self, auth: Auth, person: str) -> User:
        self.store.set(auth.token)
        self.state.user_id = auth.user.id
        self.state.name = auth.user.name
        self.state.email = auth.user.email
        self.state.verified = auth.user.verified
        if person:
            self.state.person = person
        self.save()
        return auth.user

    def register(self, code: str, email: str, password: str, name: str, person: str) -> User:
        """Konto na kod zaproszenia - od razu zalogowane (rejestracja publiczna zamknięta)."""
        checks = (check_code(code), check_email(email), check_password(password), check_name(name))
        for problem in checks:
            if problem:
                raise KontoError(problem)
        auth = self.client.register(code, email.strip(), password, name, TERMS_VERSION)
        self.state.terms = TERMS_VERSION
        return self._signed_in(auth, person)

    def login(self, email: str, password: str, person: str) -> User:
        auth = self.client.login(email.strip(), password)
        return self._signed_in(auth, person)

    def logout(self) -> None:
        if self.state.live and self.client.token:
            with contextlib.suppress(KontoError):
                self.client.clear_live()
        self._forget()

    def delete_account(self, password: str) -> None:
        """Usunięcie wymaga hasła - sam token (np. skradziony) nie skasuje konta."""
        if not password:
            raise KontoError("Wpisz hasło, żeby usunąć konto.")
        self.client.delete_account(password)
        self._forget()

    def change_password(self, old: str, new: str) -> None:
        problem = check_password(new)
        if problem:
            raise KontoError(problem)
        auth = self.client.change_password(old, new)
        self.store.set(auth.token)  # stary token przestał działać

    def refresh(self) -> User:
        """Przy starcie i co dobę: świeży token i ustawienia profilu z serwera."""
        try:
            auth = self.client.refresh()
        except LoggedOut:
            self._forget()
            raise
        user = self._signed_in(auth, "")
        with contextlib.suppress(KontoError):
            profile = self.client.my_profile()
            visibility = profile.get("widocznosc_profilu")
            if visibility in PROFILE_VISIBILITY:
                self.state.profile_visibility = str(visibility)
                self.save()
        return user

    def _forget(self) -> None:
        self.store.clear()
        self.client.token = ""
        person = self.state.person
        self.state = AccountState(person=person)
        self.save()

    def save(self) -> None:
        save_state(self.state, self.state_path)

    # --- wysyłka ---------------------------------------------------------------------------

    def upload(self, history: History) -> int:
        """Nowe i zmienione sesje powiązanej osoby. Przerwana wysyłka - dokończy następna."""
        state = self.state
        if not (state.logged_in and state.upload and state.person):
            return 0
        if not self.lock.acquire(blocking=False):
            return 0
        try:
            pending = history.unsent(state.person)
            if not pending:
                return 0
            hidden = history.hidden_online()
            payload = [
                session_payload(s, "prywatna" if s.id in hidden else state.visibility)
                for s in pending
            ]
            rejected = self.client.upload_sessions(payload)
            # odrzucone też zapisujemy jako wysłane - bez pętli; zmieniona sesja pójdzie znowu
            history.mark_sent(pending)
            for uuid, reason in rejected.items():
                log.warning("Konto: serwer odrzucił sesję %s: %s", uuid[:13], reason)
            log.info("Konto: wysłano %d sesji (odrzucone %d)", len(pending), len(rejected))
            return len(pending) - len(rejected)
        finally:
            self.lock.release()

    def delete_session(self, session_id: str) -> None:
        if self.state.logged_in:
            self.client.delete_session(canonical_uuid(session_id))

    def set_visibility(self, history: History, visibility: str) -> None:
        if visibility not in VISIBILITY:
            raise ValueError(visibility)
        self.state.visibility = visibility
        self.save()
        history.forget_sent()  # wszystkie sesje pójdą jeszcze raz - z nową widocznością

    def live(self, kmh: float, steps: int, active_s: float, session_id: str) -> None:
        if self.state.logged_in and self.state.live:
            self.client.put_live(kmh, steps, active_s, session_id)

    def clear_live(self) -> None:
        if self.state.logged_in:
            self.client.clear_live()
