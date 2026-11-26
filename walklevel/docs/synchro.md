# Synchronizacja sesji z serwera (kontrakt od HUB, 26.11.2026)

WDROŻONE na produkcji 26.11.2026 (zgoda Damiana w sesji HUB, test serwera 129/129). Klient i tak
ma znosić 404 (starszy serwer): po cichu pominąć pobieranie, zalogować raz i dalej tylko wysyłać.

Uwagi od HUB po wdrożeniu:
- Zmiany z ostatnich ~2 s nie są jeszcze oddawane (sesja wysłana przed chwilą przyjdzie przy
  następnym pobraniu) - to nie błąd.
- Gdy nie ma nic nowego, `kursor` w odpowiedzi jest taki sam jak wysłany (albo pusty przy pustym
  koncie i braku kursora) - wtedy niczego nie zapisywać.
- 400 = nieprawidłowy kursor -> wyczyścić kursor i zrobić pełne pobranie.

## Trasa

`GET /api/bieznia/sesje?kursor=<z poprzedniej odpowiedzi>&limit=500` - token konta, tylko własne
sesje. Bez `kursor` = od początku (pełne pobranie). `limit` 1-500, domyślnie 500.

Odpowiedź 200:

```json
{
  "sesje": [ { "uuid": "...", "osoba": "...", "start_utc_s": 0, "koniec_utc_s": 0, "kroki": 0,
               "dystans_km": 0, "ruch_s": 0, "bieznia_model": "", "sterownik": "", "bieznia_profil": "",
               "firmware": "", "wersja_aplikacji": "", "zrodlo": "bieznia", "widocznosc": "znajomi",
               "strefa_min": 60, "zmieniono_utc_s": 0, "tryb": "", "gra_id": "", "gra": "",
               "wiarygodna": true } ],
  "usuniete": [ { "uuid": "...", "usunieto_utc_s": 0 } ],
  "kursor": "<nieprzezroczysty napis>",
  "wiecej": true
}
```

- Kolejność: zmiany na serwerze rosnąco, sesje i nagrobki w jednym strumieniu; `limit` liczy obie
  listy łącznie.
- Kursor jest nieprzezroczysty: nie parsować, nie składać. Zapamiętać go dopiero PO zapisaniu
  odpowiedzi w lokalnej bazie. Tę samą stronę można pobrać drugi raz (idempotentnie).
- `wiecej: true` - od razu pytać dalej tym kursorem.
- `tryb`, `gra_id`, `gra` są puste do czasu nowej wersji regulaminu z grami (wtedy pusty tryb = "gra").
- `uuid` ma postać z kreskami (canonical_uuid); lokalnie id to 32 znaki hex bez kresek.

## Zasady klienta

- Pobrane sesje przypisać do osoby powiązanej z tym kontem (pole `osoba` z serwera nic nie znaczy
  lokalnie); `osoba_nazwa` = nazwa tej osoby; profil gry nieznany lokalnie -> profil pusty, nazwa
  „gra z innego komputera” (albo `gra`, jeśli przyszła).
- Konflikt: wersja z serwera zastępuje lokalną, gdy jej `zmieniono_utc_s` > lokalne; przy równości
  zostaje lokalna. Pobranych sesji nie odsyłać (oznaczyć jako wysłane z ich `zmieniono_utc_s`).
- Nagrobek (`usuniete`): usunąć sesję lokalnie (z historii, kolejki wysyłki i odrzuconych).
- 410 `{message, data: {pelna: true}}`: kursor starszy niż przechowywanie nagrobków (90 dni).
  Pełne pobranie bez kursora, potem usunąć lokalnie wszystko, co było wysłane, a czego serwer już nie
  zwrócił. Niewysłane zostawić i wysłać.
- Ponowna wysyłka usuniętej sesji (stara kolejka z innego komputera) wraca w „odrzucone” z powodem
  „Sesja została usunięta na innym komputerze.” - usunąć ją lokalnie, nie ponawiać.
- Częstotliwość: po zalogowaniu, przy starcie i co 5 minut; przy `wiecej` od razu dalej
  (limit serwera 300 zapytań/min - z zapasem).
- Kursor przechowywać per konto (konto.json albo osobny plik w dane/), wylogowanie/zmiana konta
  kasuje kursor.
- Po pobraniu zmian: przeliczyć poziomy i osiągnięcia (popupy tylko dla nowo zdobytych, nie dla
  historii z innego komputera - jak „Z Twojej historii” przy pierwszym liczeniu).
