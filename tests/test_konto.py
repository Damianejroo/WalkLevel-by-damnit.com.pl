"""Konto online (opcjonalne): logowanie, sejf na token, wysyłka sesji - na wirtualnym serwerze."""

from __future__ import annotations

import io
import json
import time
import urllib.error
from pathlib import Path
from typing import Any

import pytest

from bieznia import konto
from bieznia.historia import History, Session
from bieznia.konto.api import Client, KontoError, LoggedOut, UpdateRequired, check_base_url
from bieznia.konto.sejf import MemoryStore

BASE = "https://bieznia.example.pl"


class Reply(io.BytesIO):
    def __init__(self, body: Any, url: str, status: int = 200) -> None:
        super().__init__(json.dumps(body).encode() if body is not None else b"")
        self.status = status
        self.url = url

    def geturl(self) -> str:
        return self.url


class FakeServer:
    """Zachowuje się jak PocketBase z trasami /api/bieznia/* z uzgodnionego kontraktu."""

    def __init__(self) -> None:
        self.users: dict[str, dict[str, Any]] = {}
        self.tokens: dict[str, str] = {}  # token -> id
        self.sessions: dict[str, dict[str, Any]] = {}
        self.requests: list[tuple[str, str, Any, dict[str, str]]] = []
        self.too_old = False
        self.issued = 0

    def __call__(self, request: Any, _timeout: float) -> Reply:
        url = request.full_url
        path = url[len(BASE) :].split("?")[0]
        body = json.loads(request.data) if request.data else None
        headers = dict(request.header_items())
        self.requests.append((request.get_method(), path, body, headers))
        if self.too_old:
            return self.error(url, 426, "Zaktualizuj aplikację.")
        user = self.users_by_token(headers.get("Authorization", ""))
        method = request.get_method()
        if path == "/api/bieznia/rejestracja":
            if body.get("kod") != "KOD-START":
                return self.error(url, 400, "Kod zaproszenia jest nieważny albo już użyty.")
            uid = f"u{len(self.users) + 1}"
            self.users[uid] = {
                "id": uid,
                "email": body["email"],
                "name": body["name"],
                "password": body["password"],
                "verified": False,
                "regulamin": body.get("regulamin"),
            }
            return self.auth(url, self.users[uid])
        if path == "/api/collections/users/auth-with-password":
            for u in self.users.values():
                if u["email"] == body["identity"] and u["password"] == body["password"]:
                    return self.auth(url, u)
            return self.error(url, 400, "Failed to authenticate.")  # jak PocketBase
        if user is None:
            return self.error(url, 401, "brak tokenu")
        if path == "/api/collections/users/auth-refresh":
            return self.auth(url, user)
        if path == "/api/bieznia/konto" and method == "DELETE":
            if body.get("password") != user["password"]:
                return self.error(url, 400, "Nieprawidłowe hasło.")
            self.users.pop(user["id"], None)
            return Reply(None, url, 204)
        if path == "/api/bieznia/haslo":
            if body.get("oldPassword") != user["password"]:
                return self.error(url, 400, "Nieprawidłowe obecne hasło.")
            user["password"] = body["password"]
            self.tokens.clear()  # zmiana hasła unieważnia wszystkie tokeny
            return self.auth(url, user)
        if path == "/api/bieznia/profil" and method == "GET":
            mine = {k: user[k] for k in ("id", "email", "name", "verified")}
            return Reply({**mine, "widocznosc_profilu": "publiczny"}, url)
        if path == "/api/bieznia/kod-rejestracji":
            return Reply({"kod": "NOWY-1234", "wygasa_utc_s": 0}, url)
        if path.startswith("/api/bieznia/profil/"):
            week = {"kroki": 15200, "dystans_km": 10.1, "ruch_s": 9000, "sesje": 6}
            total = {"kroki": 99000, "dystans_km": 66.0, "ruch_s": 60000, "sesje": 40}
            friend = {"id": "u9", "name": "Kot_Bury", "od_utc_s": 1.7e9, "znajomy": True}
            model = {"bieznia_model": "Urevo SpaceWalk 5L"}
            return Reply({**friend, **model, "tydzien": week, "razem": total}, url)
        if path == "/api/bieznia/sesje":
            assert len(body["sesje"]) <= 100
            rejected = []
            for s in body["sesje"]:
                if len(s["uuid"]) != 36 or s["uuid"] != s["uuid"].lower():
                    rejected.append({"uuid": s["uuid"], "powod": "zły format uuid"})
                elif s["kroki"] > 300000:
                    rejected.append({"uuid": s["uuid"], "powod": "kroki poza zakresem"})
                else:
                    self.sessions[s["uuid"]] = s
            saved = len(body["sesje"]) - len(rejected)
            return Reply({"zapisane": saved, "pominiete": [], "odrzucone": rejected}, url)
        if path == "/api/bieznia/stan":
            return Reply(None, url, 204)
        if path == "/api/bieznia/znajomi":
            return Reply(
                [
                    {
                        "id": "u9",
                        "name": "Kot_Bury",
                        "na_zywo": {"kmh": 3.2, "kroki": 840},
                        "tydzien": {"kroki": 15200, "ruch_s": 9000},
                    },
                    {
                        "id": "u8",
                        "name": "Szybka-Ania",
                        "na_zywo": None,
                        "tydzien": {"kroki": 8100, "ruch_s": 4200},
                    },
                ],
                url,
            )
        if path == "/api/bieznia/zaproszenia":
            invite = {"id": "z1", "osoba": {"id": "u5", "name": "Nowy_Gracz"}, "utworzono_utc_s": 0}
            return Reply({"przychodzace": [invite], "wyslane": []}, url)
        if path == "/api/bieznia/statystyki":
            return Reply({"kroki": 12345, "dystans_km": 8.4, "ruch_s": 7200}, url)
        if path == "/api/bieznia/ranking":
            places = [
                {"miejsce": 1, "name": "Kot_Bury", "kroki": 15200, "dystans_km": 10.1},
                {"miejsce": 2, "name": user["name"], "kroki": 12345, "dystans_km": 8.4, "ja": True},
            ]
            return Reply(
                {"zakres": "tydzien", "miejsca": places},
                url,
            )
        if path == "/api/bieznia/osoby":
            return Reply(
                {
                    "osoby": [
                        {
                            "id": "u7",
                            "name": "Maraton_Max",
                            "tydzien": {"kroki": 30100},
                            "miejsce": 1,
                        }
                    ],
                    "strona": 1,
                    "nastepna": False,
                },
                url,
            )
        if path == "/api/bieznia/zaproszenie" and method == "POST":
            if body and body.get("do"):
                return Reply(None, url, 204)
            return Reply({"kod": "ABCD-1234", "wygasa_utc_s": 0}, url)
        if path.startswith("/api/bieznia/zaproszenie/"):
            return Reply({"znajomy": {"id": "u9", "name": "Kot_Bury"}}, url)
        if path == "/api/bieznia/profil" and method == "PATCH":
            if body.get("name"):
                user["name"] = body["name"]
            return Reply({k: user[k] for k in ("id", "email", "name", "verified")}, url)
        return self.error(url, 404, "nie ma")

    def users_by_token(self, token: str) -> dict[str, Any] | None:
        uid = self.tokens.get(token)
        return self.users.get(uid) if uid else None

    def auth(self, url: str, user: dict[str, Any]) -> Reply:
        self.issued += 1
        token = f"tok-{user['id']}-{self.issued}"
        self.tokens[token] = user["id"]
        record = {k: user[k] for k in ("id", "email", "name", "verified")}
        return Reply({"token": token, "record": record}, url)

    def error(self, url: str, code: int, message: str) -> Any:
        payload = io.BytesIO(json.dumps({"message": message, "data": {}}).encode())
        raise urllib.error.HTTPError(url, code, message, {}, payload)  # type: ignore[arg-type]


@pytest.fixture
def server() -> FakeServer:
    return FakeServer()


@pytest.fixture
def account(server: FakeServer, tmp_path: Path) -> konto.Account:
    return konto.Account(BASE, MemoryStore(), tmp_path / "konto.json", opener=server)


def signed_in(account: konto.Account, person: str = "p1") -> konto.Account:
    account.register("KOD-START", "ola@example.pl", "dlugie-haslo-123", "Ola_77", person)
    return account


def hexid(sid: str) -> str:
    """Krótka nazwa sesji w teście -> 32 znaki hex jak prawdziwe id."""
    return sid if len(sid) == 32 else sid.encode().hex().ljust(32, "0")[:32]


def uuid_of(sid: str) -> str:
    return konto.canonical_uuid(hexid(sid))


def session(
    sid: str, person: str = "p1", source: str = "bieznia", updated: float = 10.0
) -> Session:
    s = Session(hexid(sid), person, "Ola", "g1", "Cyberpunk 2077", 100.0, 700.0, 900, 0.6, 600)
    s.source = source
    s.updated = updated
    return s


def test_names_emails_and_passwords_are_checked_before_sending() -> None:
    assert konto.check_name("Ola_77") == "" and konto.check_name("Żaneta-2") == ""
    assert konto.check_name("Pan Kot.2") == ""  # spacja i kropka dozwolone
    assert konto.check_name("ab") and konto.check_name("a" * 25)
    assert konto.check_name(" Ola") and konto.check_name("Admin")  # brzegi i zarezerwowane
    assert konto.check_name("\u041ela")  # cyrylickie O - podszywanie się
    assert konto.check_email("ola@example.pl") == "" and konto.check_email("ola@")
    assert konto.check_password("krótkie")


def test_register_login_and_token_only_in_vault(
    account: konto.Account, server: FakeServer, tmp_path: Path
) -> None:
    signed_in(account)
    assert account.state.logged_in and account.state.name == "Ola_77"
    assert server.users["u1"]["regulamin"] == konto.TERMS_VERSION
    saved = (tmp_path / "konto.json").read_text(encoding="utf-8")
    assert "tok-" not in saved and "dlugie-haslo" not in saved  # ani token, ani hasło w pliku
    assert account.store.get().startswith("tok-")
    headers = server.requests[-1][3]
    assert headers["X-bieznia-wersja"]  # wersja aplikacji w każdym żądaniu


def test_bad_input_never_reaches_the_server(account: konto.Account, server: FakeServer) -> None:
    with pytest.raises(KontoError, match="Pseudonim"):
        account.register("KOD-START", "ola@example.pl", "dlugie-haslo-123", "x", "p1")
    with pytest.raises(KontoError, match="kod"):
        account.register("", "ola@example.pl", "dlugie-haslo-123", "Ola_77", "p1")
    assert not server.requests


def test_expired_token_logs_out_without_retry_loop(
    account: konto.Account, server: FakeServer
) -> None:
    signed_in(account)
    server.tokens.clear()  # serwer unieważnił tokeny (np. zmiana hasła = wyloguj wszędzie)
    before = len(server.requests)
    with pytest.raises(LoggedOut):
        account.refresh()
    assert len(server.requests) == before + 1
    assert not account.state.logged_in and account.store.get() == ""


def test_upload_is_idempotent_and_sends_only_linked_person(
    account: konto.Account, server: FakeServer, tmp_path: Path
) -> None:
    history = History(tmp_path / "h.db")
    for s in (
        session("a"),
        session("b"),
        session("c", person="p2"),
        session("d", source="symulator"),
    ):
        history.save(s)
    signed_in(account)
    assert account.upload(history) == 2
    assert set(server.sessions) == {uuid_of("a"), uuid_of("b")}  # bez p2 i symulatora
    sent = server.sessions[uuid_of("a")]
    assert (
        "osoba" not in sent and "profil_nazwa" not in sent and "Cyberpunk" not in json.dumps(sent)
    )
    assert sent["widocznosc"] == "znajomi"
    assert account.upload(history) == 0  # nic nowego - nic nie idzie
    changed = history.sessions()[0]
    changed.steps += 10
    history.save(changed)  # sesja trwa dalej - zmieniona idzie jeszcze raz (upsert)
    assert account.upload(history) == 1


def test_hidden_session_goes_private_and_visibility_change_resends(
    account: konto.Account, server: FakeServer, tmp_path: Path
) -> None:
    history = History(tmp_path / "h.db")
    history.save(session("a"))
    history.save(session("b"))
    signed_in(account)
    account.upload(history)
    history.set_hidden_online(hexid("a"), True)
    assert account.upload(history) == 1
    assert server.sessions[uuid_of("a")]["widocznosc"] == "prywatna"
    account.set_visibility(history, "publiczna")
    assert account.upload(history) == 2
    assert server.sessions[uuid_of("b")]["widocznosc"] == "publiczna"
    assert server.sessions[uuid_of("a")]["widocznosc"] == "prywatna"  # ukryta zostaje ukryta


def test_no_upload_without_login_or_when_turned_off(
    account: konto.Account, server: FakeServer, tmp_path: Path
) -> None:
    history = History(tmp_path / "h.db")
    history.save(session("a"))
    assert account.upload(history) == 0 and not server.requests
    signed_in(account)
    account.state.upload = False
    assert account.upload(history) == 0


def test_delete_account_needs_password(account: konto.Account, server: FakeServer) -> None:
    signed_in(account)
    with pytest.raises(KontoError):
        account.delete_account("zle-haslo-000")
    assert server.users and account.state.logged_in
    account.delete_account("dlugie-haslo-123")
    assert not server.users and not account.state.logged_in and account.store.get() == ""


def test_server_can_demand_newer_app(account: konto.Account, server: FakeServer) -> None:
    server.too_old = True
    with pytest.raises(UpdateRequired, match="Zaktualizuj"):
        account.client.friends.__self__.token = "x"
        account.client.friends()


def test_only_https_and_no_redirect_to_other_hosts() -> None:
    with pytest.raises(KontoError):
        check_base_url("http://bieznia.example.pl")
    with pytest.raises(KontoError):
        check_base_url("https://user:pass@bieznia.example.pl")
    assert check_base_url("http://127.0.0.1:8090/") == "http://127.0.0.1:8090"

    def elsewhere(request: Any, _t: float) -> Reply:
        return Reply({"token": "x"}, "https://zly.example/api")

    client = Client(BASE, opener=elsewhere)
    with pytest.raises(KontoError, match="przekierował"):
        client.login("a@b.pl", "x" * 12)


def test_no_connection_gives_friendly_message() -> None:
    def offline(request: Any, _t: float) -> Reply:
        raise urllib.error.URLError("brak sieci")

    with pytest.raises(KontoError, match="internet"):
        Client(BASE, opener=offline).login("a@b.pl", "x" * 12)


def test_request_bodies_are_not_logged(
    account: konto.Account, caplog: pytest.LogCaptureFixture
) -> None:
    import logging

    with caplog.at_level(logging.DEBUG):
        signed_in(account)
    assert "dlugie-haslo" not in caplog.text and "tok-" not in caplog.text
    assert "rejestracja -> 200" in caplog.text  # tylko ścieżka i kod


def test_history_v3_keeps_sent_and_hidden_tables(tmp_path: Path) -> None:
    history = History(tmp_path / "h.db")
    assert history.schema_version() == 3
    history.save(session("a"))
    history.set_hidden_online("a", True)
    assert history.hidden_online() == {"a"}
    history.close()
    again = History(tmp_path / "h.db")
    assert again.hidden_online() == {"a"} and again.unsent("p1")[0].id == hexid("a")
    assert time.time() > 0


def session_id(n: int) -> str:
    return f"{n:032x}"


def test_uuid_goes_lowercase_with_dashes_and_rejected_do_not_loop(
    account: konto.Account, server: FakeServer, tmp_path: Path
) -> None:
    assert konto.canonical_uuid("0192ABCD0000700080000000000000AA") == (
        "0192abcd-0000-7000-8000-0000000000aa"
    )
    history = History(tmp_path / "h.db")
    history.save(session(session_id(1)))
    bad = session(session_id(2))
    bad.steps = 999_999
    history.save(bad)
    signed_in(account)
    assert account.upload(history) == 1  # zła sesja nie wywraca paczki
    assert account.upload(history) == 0  # i nie wraca w kółko


def test_builtin_pocketbase_errors_are_polish(account: konto.Account) -> None:
    signed_in(account)
    with pytest.raises(KontoError, match="Nieprawidłowy e-mail lub hasło"):
        account.login("ola@example.pl", "zle-haslo-000", "p1")


def test_wrong_invitation_code_is_refused(account: konto.Account) -> None:
    with pytest.raises(KontoError, match="Kod zaproszenia"):
        account.register("ZLY-KOD", "ola@example.pl", "dlugie-haslo-123", "Ola_77", "p1")
    assert not account.state.logged_in


def test_change_password_keeps_session_with_new_token(
    account: konto.Account, server: FakeServer
) -> None:
    signed_in(account)
    old_token = account.store.get()
    account.change_password("dlugie-haslo-123", "nowe-dlugie-haslo-9")
    assert account.store.get() != old_token and account.client.token == account.store.get()
    assert account.client.friends()  # nowy token działa
    with pytest.raises(KontoError, match="Hasło"):
        account.change_password("nowe-dlugie-haslo-9", "krotkie")


def test_refresh_takes_profile_visibility_from_server(account: konto.Account) -> None:
    signed_in(account)
    account.refresh()
    assert account.state.profile_visibility == "publiczny"


def stores() -> tuple[dict[str, MemoryStore], Any]:
    made: dict[str, MemoryStore] = {}

    def store_for(person: str) -> MemoryStore:
        return made.setdefault(person, MemoryStore())

    return made, store_for


def test_each_person_has_own_account_and_vault_entry(server: FakeServer, tmp_path: Path) -> None:
    vault, store_for = stores()
    path = tmp_path / "konto.json"
    accounts = konto.Accounts(BASE, path, opener=server, store_for=store_for)
    accounts.get("p1").register("KOD-START", "a@example.pl", "dlugie-haslo-123", "Ala", "p1")
    accounts.get("p2").register("KOD-START", "b@example.pl", "dlugie-haslo-123", "Bob", "p2")
    assert {a.state.name for a in accounts.logged_in()} == {"Ala", "Bob"}
    assert vault["p1"].get() != vault["p2"].get()  # osobne tokeny
    assert accounts.person_of(accounts.get("p2").state.user_id) == "p2"
    again = konto.Accounts(BASE, path, opener=server, store_for=store_for)  # po restarcie
    assert {a.state.person for a in again.logged_in()} == {"p1", "p2"}
    again.forget_person("p1")
    assert [a.state.person for a in again.logged_in()] == ["p2"] and vault["p1"].get() == ""


def test_without_remember_login_lasts_until_closing(server: FakeServer, tmp_path: Path) -> None:
    vault, store_for = stores()
    path = tmp_path / "konto.json"
    accounts = konto.Accounts(BASE, path, opener=server, store_for=store_for)
    acc = accounts.get("p1")
    acc.state.remember = False
    acc.register("KOD-START", "a@example.pl", "dlugie-haslo-123", "Ala", "p1")
    assert acc.client.token and vault["p1"].get() == ""  # token tylko w pamięci
    assert konto.Accounts(BASE, path, opener=server, store_for=store_for).logged_in() == []


def test_new_person_login_attaches_to_new_person(server: FakeServer, tmp_path: Path) -> None:
    vault, store_for = stores()
    accounts = konto.Accounts(BASE, tmp_path / "k.json", opener=server, store_for=store_for)
    temp = accounts.blank()
    temp.register("KOD-START", "a@example.pl", "dlugie-haslo-123", "Ala", "")
    attached = accounts.attach(temp, "nowa")
    assert attached.state.person == "nowa" and attached.client.token == vault["nowa"].get()
    assert accounts.get("nowa") is attached


def test_single_account_file_from_previous_version_moves_over(
    server: FakeServer, tmp_path: Path
) -> None:
    path = tmp_path / "konto.json"
    old = {"user_id": "u1", "name": "Ala", "email": "a@example.pl", "person": "p1"}
    path.write_text(json.dumps(old), encoding="utf-8")
    legacy = MemoryStore()
    legacy.set("tok-stary")
    vault, store_for = stores()
    accounts = konto.Accounts(BASE, path, opener=server, store_for=store_for, legacy_store=legacy)
    assert [a.state.name for a in accounts.logged_in()] == ["Ala"]
    assert vault["p1"].get() == "tok-stary" and legacy.get() == ""
