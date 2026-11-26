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
               "gra_publiczna": false, "wiarygodna": true } ],
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
- `tryb`, `gra_id`, `gra`, `gra_publiczna` są puste do czasu nowej wersji regulaminu z grami;
  potem właściciel dostaje je zawsze (sekcja „Gry na koncie”).
- `uuid` ma postać z kreskami (canonical_uuid); lokalnie id to 32 znaki hex bez kresek.

## Zasady klienta

- Pobrane sesje przypisać do osoby powiązanej z tym kontem (pole `osoba` z serwera nic nie znaczy
  lokalnie); `osoba_nazwa` = nazwa tej osoby; profil gry - sekcja „Gry na koncie”.
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

## Gry na koncie (0.6.1, kontrakt od HUB; serwer wdroży przy publikacji regulaminu z grami)

Decyzja Damiana: nazwa gry jest zapisywana na serwerze ZAWSZE (widzi ją tylko właściciel), inni
widzą ją tylko przy przełączniku profilu „Pokazuj tę grę na koncie online”.

Wersja z grami: `GET /api/bieznia/regulamin` oddaje obok `wersja` pole `poziomy` - wersję
regulaminu, od której serwer zapisuje pola gier i pokazuje poziomy ("" - jeszcze nie). Klient
czyta je przy rejestracji, logowaniu i każdym odświeżeniu konta (start, co dobę) do `terms_games`
w konto.json (brak odpowiedzi - zostaje stara wartość).

Wysyłka (`POST /api/bieznia/sesje`, `upload.rs` `session_payload`):
- Pola gier tylko, gdy `poziomy` niepuste, a zaakceptowana wersja `terms` >= `poziomy`
  (`games_allowed()`, wersje to daty RRRR-MM-DD); bez tego sesja idzie bez `tryb`, `gra_id`,
  `gra`, `gra_publiczna`.
- `tryb` (`gra`/`trening`) zawsze; dla sesji z profilem gry (`gra_id` = id profilu, 10 znaków hex)
  także `gra` (nazwa profilu, pojedyncze spacje, do 40 znaków) i `gra_publiczna` = przełącznik
  profilu (domyślnie false po stronie serwera).
- Paczki po 100 (`api.rs` `BATCH`).
- Serwer nadpisuje sesję TYLKO nowszym `zmieniono_utc_s`; zapisana wersja nowsza albo równa ->
  sesja pominięta, uuid wraca w `pominiete`. Klient loguje ich liczbę (info) i oznacza je jako
  wysłane (nowsza wersja przyjdzie synchronizacją) - bez pętli.
- Dlatego każda zmiana sesji, która ma pójść jeszcze raz, dostaje `zmieniono_utc_s` = teraz:
  zmiana widoczności konta (`set_visibility` -> `mark_all_for_resend`: wszystkie sesje osoby
  z bieżni), ukrycie online jednej sesji (`set_hidden_online`), przełącznik „Pokazuj tę grę”
  (sesje tego profilu, w tym ostatnia - serwer bierze `gra_publiczna` z ostatniej sesji gry),
  ponowna wysyłka po regulaminie (niżej). Bez akceptacji regulaminu z grami sesje idą, ale bez
  pól gier.
- `przyjeto_utc_s` w `GET sesje` klient pomija. „Pobierz moje dane” (`GET moje-dane`, z `postep`,
  `osiagniecia`, `usuniete_sesje`) zapisuje całą odpowiedź bez zmian.

Ponowna wysyłka po akceptacji nowej wersji regulaminu (serwer dziś ignoruje pola gier, więc
historia sprzed akceptacji ich nie ma; serwer nadpisuje sesję nowszym `zmieniono_utc_s`):
- Pierwsza wysyłka z `games_allowed()`, gdy `resent_terms` w konto.json (wersja, po której
  historia już poszła z grami) jest starsze niż `poziomy` (`resend_due()`), woła
  `history.mark_for_resend(osoba, None, teraz)`: `zmieniono_utc_s = teraz` dla sesji osoby z bieżni
  z niepustym profilem. `unsent` zwraca je znowu (wpis w `wyslane` jest starszy), idą paczkami po
  100, potem `resent_terms = terms`. Raz na zmianę `poziomy` po stronie serwera.
- Działa też, gdy regulamin zaakceptowano na innym komputerze (wersja przychodzi z profilu przy
  odświeżeniu) - każdy komputer wysyła raz swoją historię.
- Wybór podniesienia `zmieniono_utc_s` zamiast czyszczenia `wyslane`: przy samym czyszczeniu
  sesje poszłyby z tym samym czasem, a serwer nadpisuje tylko nowszym `zmieniono_utc_s` - pola
  gier by nie doszły. Skutek dla konfliktów: podniesiona wersja jest najnowsza, więc nadpisze na serwerze
  zmiany tej sesji z innego komputera, których ten jeszcze nie pobrał (np. ukrycie online) -
  dlatego aplikacja pobiera zmiany przy starcie (synchronizacja od razu, wysyłka po 20 s).
  Inne komputery pobiorą podniesione sesje jak zwykłą zmianę (bez duplikatów, po uuid).
- Sesji BEZ profilu (pobranych wcześniej z innego komputera, gdy gra nie była znana) nie
  podnosimy: ten komputer nie wie o nich nic nowego, a nowszy czas nadpisałby pełniejszą wersję
  wysłaną przez ich komputer.
- Zmiana przełącznika „Pokazuj tę grę” podnosi tak samo tylko sesje tego profilu
  (`mark_for_resend(osoba, profil, teraz)`), żeby serwer przyjął nowe `gra_publiczna`.

Pobieranie (`GET`, `sync.rs` `session_from_server`) - właściciel dostaje zawsze `tryb`, `gra_id`,
`gra`, `gra_publiczna`:
- `gra_id` znany lokalnie (profil osoby) -> ten profil i jego lokalna nazwa.
- `gra_id` nieznany, a jest `gra` -> profil `gra_id` z nazwą `gra`; aplikacja zakłada osobie
  powiązanej z kontem profil gry z tym samym id i nazwą (domyślne ustawienia, ikona i kolor,
  „Pokazuj tę grę” = `gra_publiczna`; `SyncReport.new_games` -> `Store::add_synced_profile`,
  wycinek `people` odświeżony).
- Bez tego: profil tej samej sesji w historii, jeśli był; `gra_id` bez nazwy -> ten id z nazwą
  „gra z innego komputera”; sama `gra` -> profil pusty z tą nazwą; brak `gra_id` i `gra` ->
  „gra z innego komputera”. Zły format `gra_id` = brak.
- `tryb` z serwera (`trening`/`gra`); pusty (serwer sprzed regulaminu) -> tryb lokalnej sesji
  albo `gra`.
