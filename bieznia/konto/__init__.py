"""Konto online - OPCJONALNY dodatek. Bez logowania aplikacja działa w całości lokalnie, nic
nie wysyła, a funkcji online w ogóle nie widać.

Konto = osoba w aplikacji (jedno konto, jedna osoba; profile gier dalej dowolne). Zalogowanych
może być kilka osób naraz, obok osób bez konta. Token każdej osoby osobno w Menedżerze
poświadczeń - albo tylko w pamięci, gdy „Zapamiętaj logowanie” jest wyłączone.

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
from collections.abc import Callable
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
from bieznia.konto.sejf import TARGET, TokenStore, default_store

log = logging.getLogger("bieznia")

STATE_FILE = Path("dane") / "konto.json"
VISIBILITY = ("prywatna", "znajomi", "publiczna")
PROFILE_VISIBILITY = ("ukryty", "znajomi", "publiczny")
GAMES_VISIBILITY = ("ja", "znajomi", "wszyscy")
GAME_NAME_MAX = 40
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
    remember: bool = True  # token w Menedżerze poświadczeń (inaczej tylko do zamknięcia)
    terms_current: str = ""  # obowiązująca wersja regulaminu (z serwera)
    games_visibility: str = "znajomi"  # kto widzi podział wyników na gry

    @property
    def terms_outdated(self) -> bool:
        """Serwer ma nowszy regulamin niż ten zaakceptowany - karta „zaakceptuj”."""
        return bool(self.terms_current) and self.terms != self.terms_current

    @property
    def games_allowed(self) -> bool:
        """Nazwy gier wychodzą dopiero po akceptacji wersji regulaminu, która je opisuje
        (serwer zna mechanizm wersji, a konto ma wersję aktualną)."""
        return bool(self.terms_current) and self.terms == self.terms_current

    @property
    def logged_in(self) -> bool:
        return bool(self.user_id)


def _state_from(raw: dict[str, Any]) -> AccountState:
    known = {f.name for f in fields(AccountState)}
    state = AccountState(**{k: v for k, v in raw.items() if k in known})
    if state.visibility not in VISIBILITY:
        state.visibility = "znajomi"
    if state.profile_visibility not in PROFILE_VISIBILITY:
        state.profile_visibility = "znajomi"
    return state


def load_state(path: Path = STATE_FILE) -> AccountState:
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return AccountState()
    if "konta" in raw:
        first = next(iter(raw["konta"]), None)
        return _state_from(first) if isinstance(first, dict) else AccountState()
    return _state_from(raw)


def load_states(path: Path = STATE_FILE) -> list[AccountState]:
    """Wszystkie konta z pliku (format 2: lista; format 1 z wcześniejszej wersji: jedno)."""
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return []
    items = raw.get("konta") if isinstance(raw, dict) and "konta" in raw else [raw]
    states = [_state_from(r) for r in items or [] if isinstance(r, dict)]
    return [s for s in states if s.logged_in and s.person]


def save_state(state: AccountState, path: Path = STATE_FILE) -> None:
    save_states([state] if state.logged_in else [], path)


def save_states(states: list[AccountState], path: Path = STATE_FILE) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(".tmp")
    data = {"wersja": 2, "konta": [asdict(s) for s in states]}
    tmp.write_text(json.dumps(data, ensure_ascii=False, indent=1), encoding="utf-8")
    tmp.replace(path)


def vault_target(person: str) -> str:
    return f"{TARGET}/{person}" if person else TARGET


def session_payload(
    s: Session, visibility: str, game: bool = False, details: bool | None = None
) -> dict[str, Any]:
    """Sesja do wysłania.

    details=True (zaakceptowana wersja regulaminu z grami i poziomami): tryb (gra/trening)
    i stały identyfikator gry - zawsze, bo liczą się z nich osiągnięcia za gry.
    game=True: do tego czytelna nazwa gry (przełącznik „Pokazuj tę grę na koncie online”)."""
    if details is None:
        details = game
    row = dict(zip(COLUMN_NAMES, s.row(), strict=True))
    out = {k: row[k] for k in UPLOAD_FIELDS}
    out["uuid"] = canonical_uuid(s.id)
    out["widocznosc"] = visibility
    if details:
        out["tryb"] = s.mode if s.mode in ("gra", "trening") else "gra"
        if re.fullmatch(r"[0-9a-f]{10}", s.profile or ""):
            out["gra_id"] = s.profile
            if game and s.profile_name.strip():
                out["gra"] = " ".join(s.profile_name.split())[:GAME_NAME_MAX]
    return out


class Account:
    """Konto: logowanie, wysyłka sesji i dane online. Metody sieciowe - z wątku w tle."""

    def __init__(
        self,
        base_url: str,
        store: TokenStore | None = None,
        state_path: Path = STATE_FILE,
        opener: Opener = _default_opener,
        state: AccountState | None = None,
        saver: Callable[[], None] | None = None,
        token: str = "",
    ) -> None:
        self.state_path = state_path
        self.state = state if state is not None else load_state(state_path)
        self.saver = saver
        self.store = store if store is not None else default_store()
        self.enabled = bool(base_url.strip())
        token = token or (self.store.get() if self.state.remember else "")
        self.client = Client(base_url or "https://127.0.0.1", token, opener)
        self.lock = threading.Lock()  # wysyłka sesji nie może iść dwa razy naraz
        if self.state.logged_in and not self.client.token:
            self._forget()  # brak tokenu (bez zapamiętania albo inny użytkownik Windows)

    def set_remember(self, on: bool) -> None:
        """Zapamiętaj logowanie: token do sejfu (albo z niego usunięty - tylko w pamięci)."""
        self.state.remember = on
        if on and self.client.token:
            self.store.set(self.client.token)
        elif not on:
            self.store.clear()
        self.save()

    def _keep_token(self, token: str) -> None:
        self.client.token = token
        if self.state.remember:
            self.store.set(token)
        else:
            self.store.clear()

    # --- logowanie -------------------------------------------------------------------------

    def _signed_in(self, auth: Auth, person: str) -> User:
        self._keep_token(auth.token)
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
        version = self.client.terms_version() or TERMS_VERSION  # obowiązująca z serwera
        auth = self.client.register(code, email.strip(), password, name, version)
        self.state.terms = version
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
        self._keep_token(auth.token)  # stary token przestał działać

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
            if profile.get("widocznosc_gier") in GAMES_VISIBILITY:
                self.state.games_visibility = str(profile["widocznosc_gier"])
            if profile.get("regulamin_aktualny"):
                self.state.terms_current = str(profile["regulamin_aktualny"])
            if profile.get("regulamin"):
                self.state.terms = str(profile["regulamin"])
            if "verified" in profile:  # po kliknięciu linku z maila
                self.state.verified = bool(profile["verified"])
            self.save()
        return user

    def _forget(self) -> None:
        self.store.clear()
        self.client.token = ""
        person, remember = self.state.person, self.state.remember
        self.state = AccountState(person=person, remember=remember)
        self.save()

    def save(self) -> None:
        if self.saver is not None:
            self.saver()
        else:
            save_state(self.state, self.state_path)

    # --- wysyłka ---------------------------------------------------------------------------

    def upload(self, history: History, shared_games: Callable[[str], bool] | None = None) -> int:
        """Nowe i zmienione sesje powiązanej osoby. Przerwana wysyłka - dokończy następna.
        shared_games(profil_id): czy nazwę tej gry wolno pokazać (przełącznik przy profilu)."""
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
            games = state.games_allowed and shared_games is not None
            payload = [
                session_payload(
                    s,
                    "prywatna" if s.id in hidden else state.visibility,
                    game=games and shared_games is not None and shared_games(s.profile),
                    details=games,
                )
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

    def accept_terms(self, history: History) -> None:
        """Akceptacja obowiązującego regulaminu; sesje idą jeszcze raz (np. z nazwami gier)."""
        version = self.state.terms_current
        if not version:
            return
        self.client.update_profile(terms=version)
        self.state.terms = version
        self.save()
        history.forget_sent()

    def set_games_visibility(self, value: str) -> None:
        if value not in GAMES_VISIBILITY:
            raise ValueError(value)
        self.client.update_profile(games_visibility=value)
        self.state.games_visibility = value
        self.save()

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


class Accounts:
    """Konta wszystkich osób: osoba -> konto (jedno na osobę). Zapis w dane/konto.json."""

    def __init__(
        self,
        base_url: str,
        path: Path = STATE_FILE,
        opener: Opener = _default_opener,
        store_for: Callable[[str], TokenStore] | None = None,
        legacy_store: TokenStore | None = None,
    ) -> None:
        self.base_url = base_url
        self.path = path
        self.opener = opener
        self.store_for = store_for or (lambda person: default_store(vault_target(person)))
        # token wersji z jednym kontem (wspólna nazwa w sejfie) - tylko przy prawdziwym sejfie
        if legacy_store is None and store_for is None:
            legacy_store = default_store()
        self.enabled = bool(base_url.strip())
        self.items: dict[str, Account] = {}
        self._loading = True  # bez zapisu pliku, zanim wszystkie konta są wczytane
        for state in load_states(path):
            store = self.store_for(state.person)
            if state.remember and not store.get() and legacy_store is not None:
                token = legacy_store.get()  # wersja z jednym kontem: token pod wspólną nazwą
                if token:
                    store.set(token)
                    legacy_store.clear()
            account = self._make(state, store)
            if account.state.logged_in:
                self.items[state.person] = account
        self._loading = False

    def _make(
        self, state: AccountState, store: TokenStore | None = None, token: str = ""
    ) -> Account:
        return Account(
            self.base_url,
            store if store is not None else self.store_for(state.person),
            self.path,
            self.opener,
            state=state,
            saver=self.save,
            token=token,
        )

    def get(self, person: str) -> Account:
        """Konto osoby (wylogowane, jeśli osoba jeszcze nie ma konta)."""
        account = self.items.get(person)
        if account is None:
            account = self._make(AccountState(person=person))
            self.items[person] = account
        return account

    def blank(self) -> Account:
        """Konto tymczasowe (logowanie jako nowa osoba) - przypinane do osoby po sukcesie."""
        from bieznia.konto.sejf import MemoryStore

        return Account(
            self.base_url,
            MemoryStore(),
            self.path,
            self.opener,
            state=AccountState(),
            saver=lambda: None,
        )

    def attach(self, account: Account, person: str) -> Account:
        """Zalogowane konto tymczasowe -> konto osoby (token do jej miejsca w sejfie)."""
        state = account.state
        state.person = person
        attached = self._make(state, token=account.client.token)
        attached._keep_token(account.client.token)
        self.items[person] = attached
        self.save()
        return attached

    def logged_in(self) -> list[Account]:
        return [a for a in self.items.values() if a.state.logged_in]

    def person_of(self, user_id: str) -> str:
        for person, account in self.items.items():
            if account.state.logged_in and account.state.user_id == user_id:
                return person
        return ""

    def forget_person(self, person: str) -> None:
        """Osoba usunięta z aplikacji - jej konto zostaje na serwerze, tu wylogowane."""
        account = self.items.pop(person, None)
        if account is not None and account.state.logged_in:
            account.store.clear()
        self.save()

    def save(self) -> None:
        if not self._loading:
            save_states([a.state for a in self.logged_in()], self.path)
