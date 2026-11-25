"""Klient serwera kont (PocketBase + trasy /api/bieznia/*). Opcjonalny dodatek do aplikacji.

Bezpieczeństwo po stronie klienta:
- tylko HTTPS ze zwykłą weryfikacją certyfikatu (bez wyjątków; http wyłącznie do 127.0.0.1
  w testach), przekierowanie na inny host przerywa żądanie,
- token tylko w nagłówku Authorization, nigdy w adresie; hasło nie jest nigdzie zapisywane,
- treści żądań i odpowiedzi nie trafiają do logów (tylko metoda, ścieżka, kod i czas),
- 401 = sesja wygasła -> LoggedOut (okno czyści token i pokazuje logowanie, bez pętli),
- 426 = serwer wymaga nowszej aplikacji -> UpdateRequired,
- nagłówek X-Bieznia-Wersja w każdym żądaniu.
"""

from __future__ import annotations

import json
import logging
import time
import urllib.error
import urllib.request
from collections.abc import Callable
from dataclasses import dataclass
from typing import Any, Protocol
from urllib.parse import quote, urlencode, urlparse

from bieznia import __version__

log = logging.getLogger("bieznia")

TIMEOUT_S = 15
MAX_RESPONSE = 2 * 1024 * 1024
BATCH = 100  # sesji w jednym żądaniu (limit treści serwera: 64 KB)


class KontoError(Exception):
    """Komunikat dla użytkownika (po polsku, bez szczegółów technicznych)."""

    def __init__(self, message: str, status: int = 0, fields: dict[str, str] | None = None):
        super().__init__(message)
        self.status = status
        self.fields = fields or {}


class LoggedOut(KontoError):
    """Token nieważny albo wygasł - trzeba zalogować się ponownie."""


class UpdateRequired(KontoError):
    """Serwer nie przyjmuje tej wersji aplikacji (poprawka bezpieczeństwa)."""


class Response(Protocol):
    status: int

    def read(self, n: int = -1) -> bytes: ...

    def geturl(self) -> str: ...

    def __enter__(self) -> Response: ...

    def __exit__(self, *args: object) -> None: ...


Opener = Callable[[urllib.request.Request, float], Response]


def _default_opener(request: urllib.request.Request, timeout: float) -> Response:
    return urllib.request.urlopen(request, timeout=timeout)  # type: ignore[no-any-return]


def check_base_url(url: str) -> str:
    """Adres serwera: https (http tylko do 127.0.0.1 - serwer testowy na tym komputerze)."""
    url = url.strip().rstrip("/")
    parsed = urlparse(url)
    local = parsed.hostname in ("127.0.0.1", "localhost")
    if not parsed.hostname or not (parsed.scheme == "https" or (parsed.scheme == "http" and local)):
        raise KontoError("Adres serwera kont musi zaczynać się od https://")
    if parsed.username or parsed.password or parsed.query or parsed.fragment:
        raise KontoError("Niepoprawny adres serwera kont.")
    return url


@dataclass(frozen=True)
class User:
    id: str
    name: str
    email: str
    verified: bool

    @classmethod
    def from_record(cls, r: dict[str, Any]) -> User:
        return cls(
            id=str(r.get("id", "")),
            name=str(r.get("name", "")),
            email=str(r.get("email", "")),
            verified=bool(r.get("verified", False)),
        )


@dataclass(frozen=True)
class Auth:
    token: str
    user: User


_MESSAGES = {
    0: "Brak połączenia z serwerem kont - sprawdź internet.",
    400: "Serwer nie przyjął danych.",
    403: "Brak dostępu - potwierdź adres e-mail (link w wiadomości).",
    404: "Nie znaleziono.",
    413: "Za dużo danych naraz.",
    429: "Za dużo prób - odczekaj chwilę i spróbuj ponownie.",
}


def friendly(status: int, body: Any, path: str = "/api/bieznia/") -> str:
    """Komunikat z serwera albo nasz - nigdy surowy błąd. Trasy /api/bieznia/* mówią po
    polsku ({message}); wbudowane trasy PocketBase po angielsku - te mapujemy po kodzie."""
    if not path.startswith("/api/bieznia/"):
        if status == 400 and path.endswith("auth-with-password"):
            return "Nieprawidłowy e-mail lub hasło."
        if status == 429:
            return "Za dużo prób - spróbuj za minutę."
        body = None
    elif status == 404:
        return "Ta funkcja będzie dostępna wkrótce."
    if isinstance(body, dict):
        message = body.get("message")
        if isinstance(message, str) and message.strip() and len(message) < 200:
            return message.strip()
    if status >= 500:
        return "Serwer kont ma chwilowy problem - spróbuj później."
    return _MESSAGES.get(status, f"Serwer odpowiedział błędem {status}.")


class Client:
    def __init__(
        self,
        base_url: str,
        token: str = "",
        opener: Opener = _default_opener,
        clock: Callable[[], float] = time.monotonic,
    ) -> None:
        self.base = check_base_url(base_url)
        self.host = urlparse(self.base).hostname
        self.token = token
        self.opener = opener
        self.clock = clock

    # --- transport -------------------------------------------------------------------------

    def call(
        self,
        method: str,
        path: str,
        body: Any = None,
        query: dict[str, str] | None = None,
        auth: bool = True,
    ) -> Any:
        url = self.base + path + (f"?{urlencode(query)}" if query else "")
        headers = {
            "Accept": "application/json",
            "User-Agent": f"Bieznia/{__version__}",
            "X-Bieznia-Wersja": __version__,
        }
        data = None
        if body is not None:
            data = json.dumps(body, ensure_ascii=False).encode("utf-8")
            headers["Content-Type"] = "application/json"
        if auth:
            if not self.token:
                raise LoggedOut("Zaloguj się, żeby korzystać z funkcji online.", 401)
            headers["Authorization"] = self.token
        request = urllib.request.Request(url, data=data, headers=headers, method=method)
        began = self.clock()
        status = 0
        try:
            with self.opener(request, TIMEOUT_S) as response:
                if urlparse(response.geturl()).hostname != self.host:
                    raise KontoError("Serwer przekierował na inny adres - przerwano.")
                status = getattr(response, "status", 200)
                raw = response.read(MAX_RESPONSE + 1)
        except urllib.error.HTTPError as exc:
            status = exc.code
            try:
                payload = json.loads(exc.read(64 * 1024) or b"null")
            except (ValueError, OSError):
                payload = None
            log.info("Konto: %s %s -> %d", method, path, status)
            message = friendly(status, payload, path)
            fields = _field_errors(payload)
            if status == 401:
                self.token = ""
                raise LoggedOut("Sesja wygasła - zaloguj się ponownie.", 401) from exc
            if status == 426:
                raise UpdateRequired(message, 426) from exc
            raise KontoError(message, status, fields) from exc
        except (urllib.error.URLError, TimeoutError, OSError) as exc:
            log.info("Konto: %s %s -> brak połączenia (%s)", method, path, type(exc).__name__)
            raise KontoError(_MESSAGES[0]) from exc
        log.info("Konto: %s %s -> %d (%.1f s)", method, path, status, self.clock() - began)
        if len(raw) > MAX_RESPONSE:
            raise KontoError("Odpowiedź serwera jest za duża - przerwano.")
        if not raw:
            return None
        try:
            return json.loads(raw)
        except ValueError as exc:
            raise KontoError("Serwer odpowiedział w nieznanym formacie.") from exc

    def _auth(self, payload: Any) -> Auth:
        if not isinstance(payload, dict) or not payload.get("token"):
            raise KontoError("Serwer odpowiedział w nieznanym formacie.")
        self.token = str(payload["token"])
        return Auth(self.token, User.from_record(payload.get("record") or {}))

    # --- konto -----------------------------------------------------------------------------

    def register(self, code: str, email: str, password: str, name: str, terms: str) -> Auth:
        """Konto na kod zaproszenia (rejestracja publiczna zamknięta) - od razu zalogowane.
        Wersja zaakceptowanego regulaminu zapisuje się przy koncie."""
        payload = self.call(
            "POST",
            "/api/bieznia/rejestracja",
            {
                "kod": code.strip(),
                "email": email,
                "password": password,
                "passwordConfirm": password,
                "name": name,
                "regulamin": terms,
            },
            auth=False,
        )
        return self._auth(payload)

    def login(self, email: str, password: str) -> Auth:
        payload = self.call(
            "POST",
            "/api/collections/users/auth-with-password",
            {"identity": email, "password": password},
            auth=False,
        )
        return self._auth(payload)

    def refresh(self) -> Auth:
        return self._auth(self.call("POST", "/api/collections/users/auth-refresh"))

    def change_password(self, old: str, new: str) -> Auth:
        """Nowe hasło unieważnia wszystkie tokeny konta - zapisujemy nowy z odpowiedzi."""
        payload = self.call(
            "POST",
            "/api/bieznia/haslo",
            {"oldPassword": old, "password": new, "passwordConfirm": new},
        )
        return self._auth(payload)

    def delete_account(self, password: str) -> None:
        """Hasło sprawdza serwer - sam token (np. skradziony) nie skasuje konta."""
        self.call("DELETE", "/api/bieznia/konto", {"password": password})
        self.token = ""

    def my_profile(self) -> dict[str, Any]:
        return dict(self.call("GET", "/api/bieznia/profil") or {})

    # --- dane ------------------------------------------------------------------------------

    def upload_sessions(self, sessions: list[dict[str, Any]]) -> dict[str, str]:
        """Paczkami po BATCH. Zwraca odrzucone sesje {uuid: powód} - zła sesja nie wywraca
        paczki, a serwer zostawia nowszą wersję (wyższe zmieniono_utc_s)."""
        rejected: dict[str, str] = {}
        for i in range(0, len(sessions), BATCH):
            result = self.call("POST", "/api/bieznia/sesje", {"sesje": sessions[i : i + BATCH]})
            for item in (result or {}).get("odrzucone") or []:
                if isinstance(item, dict):
                    rejected[str(item.get("uuid", ""))] = str(item.get("powod", ""))[:120]
        return rejected

    def delete_session(self, uuid: str) -> None:
        """Sesja usunięta z konta (404 = serwer jej nie miał - też dobrze)."""
        try:
            self.call("DELETE", f"/api/bieznia/sesje/{quote(uuid, safe='')}")
        except KontoError as exc:
            if exc.status != 404:
                raise

    def put_live(self, kmh: float, steps: int, active_s: float, session_uuid: str) -> None:
        self.call(
            "PUT",
            "/api/bieznia/stan",
            {
                "kmh": round(kmh, 1),
                "kroki": int(steps),
                "ruch_s": int(active_s),
                "sesja_uuid": session_uuid,
            },
        )

    def clear_live(self) -> None:
        self.call("DELETE", "/api/bieznia/stan")

    def invite_by_name(self, name: str) -> dict[str, Any]:
        """Zaproszenie do znajomych po pseudonimie -> {stan: oczekuje|przyjeta, osoba}."""
        result = self.call("POST", "/api/bieznia/zaproszenie", {"name": name.strip()})
        return dict(result or {})

    def friends(self) -> list[dict[str, Any]]:
        return list(self.call("GET", "/api/bieznia/znajomi") or [])

    def remove_friend(self, friend_id: str) -> None:
        self.call("DELETE", f"/api/bieznia/znajomi/{quote(friend_id, safe='')}")

    def ranking(self, scope: str = "tydzien") -> list[dict[str, Any]]:
        result = self.call("GET", "/api/bieznia/ranking", query={"zakres": scope}) or {}
        if isinstance(result, list):
            return result
        return list(result.get("miejsca") or [])

    def registration_code(self) -> dict[str, Any]:
        """Kod dla nowej osoby („Zaproś na Bieżnię”): jednorazowy, 14 dni. Kto założy z niego
        konto, od razu jest znajomym zapraszającego."""
        return dict(self.call("POST", "/api/bieznia/kod-rejestracji") or {})

    def stats(self, scope: str = "tydzien") -> dict[str, Any]:
        return dict(self.call("GET", "/api/bieznia/statystyki", query={"zakres": scope}) or {})

    def my_data(self) -> Any:
        return self.call("GET", "/api/bieznia/moje-dane")

    # --- profile, osoby, zaproszenia -------------------------------------------------------

    def withdraw_invitation(self, invitation_id: str) -> None:
        self.call("POST", f"/api/bieznia/zaproszenie/{quote(invitation_id, safe='')}/wycofaj")

    def profile(self, user_id: str) -> dict[str, Any]:
        return dict(self.call("GET", f"/api/bieznia/profil/{quote(user_id, safe='')}") or {})

    def update_profile(self, name: str | None = None, visibility: str | None = None) -> User:
        body: dict[str, str] = {}
        if name is not None:
            body["name"] = name
        if visibility is not None:
            body["widocznosc_profilu"] = visibility
        return User.from_record(dict(self.call("PATCH", "/api/bieznia/profil", body) or {}))

    def people(self, search: str = "", page: int = 1) -> dict[str, Any]:
        """Osoby z kontem (tylko profile publiczne): {"strona", "nastepna", "osoby"}."""
        query = {"strona": str(max(1, page))}
        if search.strip():
            query["szukaj"] = search.strip()[:24]
        result = self.call("GET", "/api/bieznia/osoby", query=query) or {}
        if isinstance(result, list):  # serwer może zwrócić samą listę
            return {"osoby": result, "strona": page, "nastepna": False}
        return dict(result)

    def invitations(self) -> dict[str, Any]:
        """{"przychodzace": [{id, osoba: {id, name}, utworzono_utc_s}], "wyslane": [...]};
        id to identyfikator zaproszenia (do przyjęcia lub odrzucenia)."""
        return dict(self.call("GET", "/api/bieznia/zaproszenia") or {})

    def invite_user(self, user_id: str) -> dict[str, Any]:
        return dict(self.call("POST", "/api/bieznia/zaproszenie", {"do": user_id}) or {})

    def answer_invitation(self, invitation_id: str, accept: bool) -> None:
        action = "przyjmij" if accept else "odrzuc"  # nadawca: withdraw_invitation
        self.call("POST", f"/api/bieznia/zaproszenie/{quote(invitation_id, safe='')}/{action}")


def _field_errors(payload: Any) -> dict[str, str]:
    """Błędy pól z PocketBase ({data: {email: {message}}}) - do podpowiedzi przy polach."""
    out: dict[str, str] = {}
    data = payload.get("data") if isinstance(payload, dict) else None
    if isinstance(data, dict):
        for key, value in data.items():
            if isinstance(value, dict) and isinstance(value.get("message"), str):
                out[str(key)] = value["message"][:160]
    return out
