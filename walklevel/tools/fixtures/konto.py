"""Fixtury modułu konta (account/) z prawdziwej implementacji w Pythonie (bieznia/konto).

Uruchomienie z katalogu głównego repozytorium:
    set PYTHONPATH=. && .venv\\Scripts\\python.exe walklevel\\tools\\fixtures\\konto.py

Zapisuje crates/core/tests/fixtures/konto/konto.json: sprawdzanie pól (pseudonim, e-mail,
hasło, kod), canonical_uuid, komunikaty błędów (friendly), błędy pól PocketBase, adresy
serwera, sesje do wysłania (session_payload) oraz zapis i odczyt dane/konto.json.
"""

from __future__ import annotations

import json
import tempfile
from dataclasses import asdict
from pathlib import Path
from typing import Any

from bieznia import konto
from bieznia.historia import Session
from bieznia.konto.api import KontoError, _field_errors, check_base_url, friendly

OUT = Path(__file__).resolve().parents[2] / "crates" / "core" / "tests" / "fixtures" / "konto"

NAMES = [
    "Ola_77", "Żaneta-2", "Pan Kot.2", "ab", "abc", "a" * 24, "a" * 25, " Ola", "Ola ",
    "Admin", "BIEŻNIA", "damnit", "Оla", "Ola!", "Ola\n", "", "ŁÓDŹ_ąę", "x.y-z",
    "Ćma 12345678901234567890",
]
EMAILS = [
    "ola@example.pl", "ola@", " ola@example.pl ", "ola@example", "o la@example.pl",
    "a@b.cd", "a@b.c", "a" * 64 + "@x.pl", "a" * 65 + "@x.pl", "ola@@example.pl",
    "ola@ex.ample.pl", "żółw@przykład.pl", "", "a@b." + "c" * 24, "a@b." + "c" * 25,
]
PASSWORDS = ["krótkie", "dziesiec10", "123456789", "ąęśćżźółńą", "dlugie-haslo-123", ""]
CODES = ["", "abc", "abcd", "  ab  ", " KOD-START ", "ąęść"]
UUIDS = [
    "0192ABCD0000700080000000000000AA", "0192abcd-0000-7000-8000-0000000000aa", "abc",
    "0192abcd0000700080000000000000aa", "", "0192ABCD-0000-7000-8000-0000000000AA",
]
BASES = [
    "https://bieznia.example.pl", "https://bieznia.example.pl/", " https://x.pl// ",
    "http://bieznia.example.pl", "http://127.0.0.1:8090/", "http://localhost:8090",
    "https://user:pass@bieznia.example.pl", "https://x.pl/?a=1", "https://x.pl/#a",
    "ftp://x.pl", "https://", "", "x.pl", "https://x.pl/api",
]
PATHS = [
    "/api/bieznia/sesje", "/api/collections/users/auth-with-password",
    "/api/collections/users/auth-refresh", "/api/bieznia/rejestracja",
]
STATUSES = [0, 400, 401, 403, 404, 410, 413, 426, 429, 500, 502, 418]
BODIES: list[Any] = [
    None,
    {"message": "Kod zaproszenia jest nieważny albo już użyty.", "data": {}},
    {"message": "   "},
    {"message": "x" * 200},
    {"message": "  Spacje dookoła  "},
    {"message": 5},
    {"data": {"email": {"message": "Adres zajęty.", "code": "x"}}},
    ["lista"],
]


def session(**kw: Any) -> Session:
    s = Session("0192abcd0000700080000000000000aa", "p1", "Ola", "aaaaaaaaaa",
                "  Cyberpunk   2077 ", 100.0, 700.5, 900, 0.6, 600.0)
    s.device_model, s.driver, s.device_profile = "KingSmith X21", "kingsmith", "x21"
    s.firmware, s.app_version, s.tz_offset_min, s.updated = "1.2", "0.5.1", 60, 712.25
    for k, v in kw.items():
        setattr(s, k, v)
    return s


def payloads() -> list[dict[str, Any]]:
    sessions = {
        "zwykla": session(),
        "trening": session(mode="trening"),
        "dziwny_tryb": session(mode="cos"),
        "profil_nie_hex": session(profile="g1"),
        "profil_duze": session(profile="AAAAAAAAAA"),
        "pusta_nazwa": session(profile_name="   "),
        "dluga_nazwa": session(profile_name="Bardzo  długa nazwa gry " * 4),
        "symulator": session(source="symulator", steps=0, distance_km=0.0),
    }
    out = []
    for name, s in sessions.items():
        for visibility in ("prywatna", "znajomi", "publiczna"):
            for game, details in ((False, None), (True, None), (False, True), (True, False)):
                out.append(
                    {
                        "name": name,
                        "session": asdict(s),
                        "visibility": visibility,
                        "game": game,
                        "details": details,
                        "payload": konto.session_payload(s, visibility, game, details),
                    }
                )
    return out


def states() -> dict[str, Any]:
    tmp = Path(tempfile.mkdtemp())
    path = tmp / "konto.json"
    full = konto.AccountState(
        user_id="u1", name="Olek", email="olek@example.com", verified=True, person="a1b2c3d4e5",
        upload=True, visibility="znajomi", live=False, profile_visibility="znajomi",
        terms="2026-11-25", remember=True, terms_current="2026-11-25",
        games_visibility="znajomi",
    )
    other = konto.AccountState(user_id="u2", name="Żaneta", email="z@example.pl", person="p2",
                               visibility="publiczna", remember=False)
    konto.save_states([full, other], path)
    saved_two = path.read_text(encoding="utf-8")
    konto.save_states([], path)
    saved_none = path.read_text(encoding="utf-8")
    reads = []
    inputs = [
        {"user_id": "u1", "name": "Ala", "email": "a@example.pl", "person": "p1"},
        {"user_id": "u1", "person": "p1", "visibility": "zla", "profile_visibility": "zla",
         "nieznane": 5},
        {"wersja": 2, "konta": [{"user_id": "u1", "person": "p1"}, {"user_id": "", "person": "x"},
                                {"user_id": "u3", "person": ""}, 7]},
        {"wersja": 2, "konta": []},
        {"user_id": "", "person": "p1"},
    ]
    for raw in inputs:
        path.write_text(json.dumps(raw), encoding="utf-8")
        reads.append({"input": raw, "states": [asdict(s) for s in konto.load_states(path)]})
    for text in ("", "nie json", "[]"):
        path.write_text(text, encoding="utf-8")
        try:
            got: Any = [asdict(s) for s in konto.load_states(path)]
        except Exception as exc:  # noqa: BLE001 - zapis, czym kończy się Python
            got = f"wyjątek {type(exc).__name__}"
        reads.append({"text": text, "states": got})
    return {"saved_two": saved_two, "saved_none": saved_none, "reads": reads}


def base(url: str) -> dict[str, Any]:
    try:
        return {"url": url, "ok": check_base_url(url)}
    except KontoError as exc:
        return {"url": url, "error": str(exc)}


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    data = {
        "names": [{"in": n, "out": konto.check_name(n)} for n in NAMES],
        "emails": [{"in": e, "out": konto.check_email(e)} for e in EMAILS],
        "passwords": [{"in": p, "out": konto.check_password(p)} for p in PASSWORDS],
        "codes": [{"in": c, "out": konto.check_code(c)} for c in CODES],
        "uuids": [{"in": u, "out": konto.canonical_uuid(u)} for u in UUIDS],
        "bases": [base(u) for u in BASES],
        "friendly": [
            {"status": s, "body": b, "path": p, "out": friendly(s, b, p)}
            for s in STATUSES
            for b in BODIES
            for p in PATHS
        ],
        "fields": [{"body": b, "out": _field_errors(b)} for b in BODIES],
        "payloads": payloads(),
        "states": states(),
        "terms_version": konto.TERMS_VERSION,
    }
    (OUT / "konto.json").write_text(
        json.dumps(data, ensure_ascii=False, indent=1) + "\n", encoding="utf-8"
    )
    print(f"Zapisano {OUT / 'konto.json'}")


if __name__ == "__main__":
    main()
