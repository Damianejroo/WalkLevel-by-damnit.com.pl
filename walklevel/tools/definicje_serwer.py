"""Plik definicji dla serwera kont (HUB) z definicji aplikacji: same progi, bez tekstów.

Uruchamianie z katalogu głównego repozytorium:
    python walklevel/tools/definicje_serwer.py

Czyta walklevel/docs/definicje-v2.json, zapisuje walklevel/docs/definicje-v2-serwer.json.
Zgodność obu plików pilnuje test `server_file_matches_app_definitions` w
walklevel/crates/core/tests/levels_fixtures.rs.
"""

from __future__ import annotations

import json
from pathlib import Path

SRC = Path("walklevel/docs/definicje-v2.json")
OUT = Path("walklevel/docs/definicje-v2-serwer.json")

# typ -> pole progu w definicjach aplikacji (prog); dodatkowe parametry przechodzą pod swoją nazwą
PROG = {
    "dystans": "km",
    "sesja_km": "km",
    "gra_km": "km",
    "gry_km": "km",
    "dzien_kroki": "kroki",
    "seria": "dni",
    "znajomi": "ile",
    "polecenia": "ile",
}
EXTRA = {"gry_km": ["gier"], "seria": ["km_dziennie"], "start_miedzy": ["od", "do"]}
TYPES = {*PROG, "gra_pierwsza", "trening", "start_miedzy"}


def server(d: dict) -> dict:
    out = []
    for a in d["osiagniecia"]:
        typ = a["typ"]
        if typ not in TYPES:
            raise SystemExit(f"typ {typ} ({a['id']}) nieznany serwerowi")
        row = {"id": a["id"], "typ": typ}
        if typ in PROG:
            row["prog"] = a[PROG[typ]]
        for key in EXTRA.get(typ, []):
            row[key] = a[key]
        row["rz"] = a["rzadkosc"]
        out.append(row)
    zasady = {k: v for k, v in d["zasady"].items() if isinstance(v, (int, float))}
    return {
        "wersja": d["wersja"],
        "xp_za_km": d["xp_za_km"],
        "xp_za_rzadkosc": d["xp_za_rzadkosc"],
        "poziomy_xp": d["poziomy_xp"],
        "zasady": zasady,
        "osiagniecia": out,
    }


def main() -> None:
    d = json.loads(SRC.read_text(encoding="utf-8"))
    text = json.dumps(server(d), ensure_ascii=False, indent=1) + "\n"
    OUT.write_bytes(text.encode("utf-8"))
    print(f"zapisano {OUT} ({len(d['osiagniecia'])} osiągnięć)")


if __name__ == "__main__":
    main()
