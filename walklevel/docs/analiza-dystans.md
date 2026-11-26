# Analiza: kroki, dystans i wartości liczone z ruchu bieżni

Stan na: WalkLevel 0.5.2 (gałąź `wydanie-0.5.2`, kod Pythona w `bieznia/`).
Cel: opis obecnego działania, błędy i ryzyka, dokładne zasady dla rdzenia 0.6.0 w Ruście
(`walklevel/crates/core`) i przypadki testowe, które ten rdzeń ma spełnić.

Źródła danych użyte w analizie (tylko odczyt):

- kod: `bieznia/sterowniki/**`, `bieznia/ble/worker.py`, `bieznia/engine.py`, `bieznia/mapping.py`,
  `bieznia/historia.py`, `bieznia/poziomy/__init__.py`, `bieznia/gui/**`;
- nagrania ramek: `tests/dane/x21-ramki.txt` (KingSmith X21, 2026-11-20),
  `tests/dane/urevo-5l-ramki.txt` (Urevo SpaceWalk 5L, wycinek);
- log `%LOCALAPPDATA%\Bieznia\logs\bluetooth.log`: pełna sesja Urevo z 2026-11-24 21:07,
  2208 surowych ramek Treadmill Data (37 min), oraz `logs/bluetooth.log` w repozytorium
  (ponowne połączenie Urevo 2026-11-25 19:44);
- bazy historii: `D:\Dev\bieznia-olek\dane\historia.db` (6 sesji) i
  `%LOCALAPPDATA%\Bieznia\dane\historia.db` (8 sesji), obie w schemacie 4.

---

## 1. Podsumowanie w pięciu punktach

1. **Dystans i czas ruchu liczy aplikacja, a nie bieżnia.** Obie bieżnie podają własny licznik
   dystansu (co 10 m) i czasu, ale sterowniki go wyrzucają. Aplikacja liczy prędkość × czas
   **tylko wtedy, gdy mapowanie gry uznaje, że postać idzie** (`control.walk`). Przez to dystans
   zależy od ustawienia „Chód od” i od wykrywania hamowania. Przy „Chód od 2,6 km/h” powtórka
   prawdziwej sesji Urevo daje 1380 m zamiast 1900 m (−27%).
2. **Kroki zawsze pochodzą z licznika bieżni** (KingSmith `RunningSteps`, Urevo: pole spoza
   standardu FTMS, bit 13 flag). Aplikacja niczego nie szacuje. Bieżnia bez licznika daje 0 kroków.
3. **Na prawdziwych danych długość kroku jest realistyczna:** 0,48–0,61 m w sesjach z Urevo,
   0,46 m w nagraniu X21, a z licznika Urevo 0,56 m przy 2,5 km/h i 0,76 m przy 5 km/h.
   Wyjątki to dwie krótkie sesje (0,96 m i 0,43 m) oraz sesja z 0 krokami: starsza wersja nie
   czytała kroków Urevo, a w logu jest 3061 kroków, które przepadły.
4. **Obsługa zerowania liczników jest zbyt naiwna:** każdy spadek licznika uznaje za zerowanie i dolicza całą
   nową wartość. Grozi to fantomowymi krokami, np. po przełączeniu z symulatora na bieżnię, przy
   chwilowym błędzie odczytu albo po odtworzeniu kopii. Nie ma progów wiarygodności ani obsługi
   przepełnienia.
5. **Kafelki na ekranie mieszają trzy różne źródła.** „Kroki” pokazują surowy licznik bieżni, który
   bywa niezerowy już przy połączeniu. „Dystans” i „Czas ruchu” to sumy od uruchomienia aplikacji.
   W historii zapisują się z kolei wartości sesji. Na serwer (podgląd na żywo) idzie surowy licznik
   i czas od uruchomienia aplikacji.

---

## 2. Obecne działanie: przepływ danych

```
bieżnia --BLE--> sterownik (kingsmith | ftms | symulator)
                  on_reading(km/h, kroki|None)          kontrakt.py:15
        --> BleWorker._on_reading: Reading(t=monotonic, speed, steps)   worker.py:246-247
        --> kolejka --> QueueSource.poll (tylko najnowszy odczyt na klatkę)
        --> Bridge.tick (20 klatek/s)                   engine.py:123-158
              mapper.update(speed) -> Control(walk, run, speed)   mapping.py:98-121
              cadence z okna 20 odczytów                engine.py:115-121
              distance_km += speed*dt/3600, active_s += dt  TYLKO gdy control.walk  engine.py:148-151
        --> App.tick co 5 klatek (4x/s), tylko gdy state.live   gui/app.py:1327-1342
              kafelki: Kroki = state.steps (surowy licznik), Kroki/min, Dystans, Czas
              SessionTracker.update(c.speed, state.steps, state.distance_km, state.active_s)
                 przyrosty liczników aplikacji -> Session -> SQLite co 60 s   historia.py:484-513
```

Kontrakt sterownika przenosi **tylko prędkość i kroki** (`ReadingCallback = Callable[[float, int | None], None]`,
`bieznia/sterowniki/kontrakt.py:15`). Dystansu, czasu ani kalorii z bieżni nie da się przekazać dalej.

### 2.1. KingSmith (WalkingPad X21 / R2 / G1 / X23)

Protokół tekstowy: base64 z tablicą podstawień. Ramka z danymi wygląda tak (odszyfrowana z
`tests/dane/x21-ramki.txt`, tablica v5):

```
props netled 0 ControlMode 1 CurrentSpeed 2.3 RunningTotalTime 169 RunningSteps 232 RunningDistance 100 BurnCalories 6800
```

| Pole | Jednostka i rozdzielczość (z nagrania) | Co robi aplikacja |
|---|---|---|
| `CurrentSpeed` | km/h, 0,1 (przy `unit 0`) | bierze, `max(0, v)`; `runState 0` → 0 (`kingsmith/sterownik.py:369-372`) |
| `RunningSteps` | kroki, liczba całkowita, ramka co ~2 s | bierze (`kingsmith/sterownik.py:373-374`) |
| `RunningDistance` | metry, **skok 10 m** (0, 10, 20 …) | **pomija** |
| `RunningTotalTime` | sekundy, 1 s | **pomija** |
| `BurnCalories` | nieznana jednostka (~+100 na 2 s przy 2 km/h) | pomija |
| `unit` | 0 = metryczne (w nagraniu tylko w ramce startowej z `Max 12.0`) | **pomija** (patrz ryzyko B9) |

- Dane przychodzą jako ramki `props` co ok. 2 s (subskrypcja KS Fit `servers getProp 1 9 15 2 10 11 12 13 14`
  albo odpytywanie QZ `servers getProp 1 2 7 12 23 24 31`, `kingsmith/protokol.py:48-51`).
- Na odpytywanie co 0,75 s bieżnia odpowiada krótkim `servers 0`. Każde takie potwierdzenie wysyła
  odczyt z **ostatnią** prędkością i **ostatnimi** krokami, dopóki od ramki z danymi nie minęło 6 s
  (`kingsmith/sterownik.py:363-364`, `PROPS_TIMEOUT_S`).
- Ramki „niepewne” (różne tablice szyfru dają różny tekst) są pomijane w całości (`kingsmith/sterownik.py:351-352`).
- Ramki częściowe: w nagraniu `RunningSteps` jest w 83 z 98 ramek `props`. Brak pola oznacza
  „bez zmian” (zostaje `last_steps`), co jest poprawne.
- W nagraniu X21 kroki zaczynają rosnąć ok. 4 s po ruszeniu taśmy (31,3 s: prędkość 1,6; 35,3 s: 2 kroki),
  a dystans pierwszy raz pokazuje 10 m po 24 s.
- Kadencja z licznika X21: 230 kroków w 163 s, czyli **84,7/min**. Licznik daje 100 m, a całka z
  prędkości 110,2 m. **Długość kroku: 0,43–0,48 m** przy 2,0–3,0 km/h.

### 2.2. FTMS (Urevo SpaceWalk 5L i inne bieżnie FTMS)

Treadmill Data (0x2ACD), powiadomienie co ~1 s. Dekoder: `bieznia/sterowniki/ftms/protokol.py:72-124`.
Urevo wysyła flagi `0x259C` i ramki o długości 26 bajtów:

| Bajty | Flaga | Pole (FTMS) | Typ, rozdzielczość | Przykład `9c25 fa00 860100 2800 0000 0000 0000 2500 0000 00 00 fb02 2803 00` |
|---|---|---|---|---|
| 0–1 | – | flagi | uint16 | `0x259C` |
| 2–3 | bit0 = 0 | prędkość chwilowa | uint16, 0,01 km/h | 250 → 2,50 km/h |
| 4–6 | bit2 | dystans całkowity | **uint24, 1 m** (Urevo: skok 10 m) | 390 m |
| 7–10 | bit3 | nachylenie, kąt rampy | sint16 0,1 %, sint16 0,1° | 4,0 %, 0 |
| 11–14 | bit4 | przewyższenie +/− | uint16 0,1 m ×2 | 0, 0 |
| 15–19 | bit7 | energia: suma, na h, na min | uint16 kcal, uint16, uint8 | 37 kcal |
| 20 | bit8 | tętno | uint8 | 0 |
| 21–22 | bit10 | czas od startu | uint16, 1 s | 763 s |
| 23–24 | bit13 (RFU w FTMS 1.0) | **kroki Urevo** | uint16 (założenie) | 808 |
| 25 | – | nieopisany bajt | – | zawsze 0 we wszystkich 2208 ramkach |

- Sterownik przekazuje dalej tylko prędkość i kroki (`ftms/sterownik.py:169`). `distance_m` i `elapsed_s`
  są dekodowane, ale nieużywane.
- Ramka bez prędkości (bit 0 „More Data” = 1) jest odrzucana w całości jako „nie do odczytania”
  (`ftms/sterownik.py:155-158`). Bieżnia, która dzieli dane na dwa powiadomienia, zgubi drugą część.
- Bajt 25 nie należy do żadnego znanego pola. Kroki mogą być w rzeczywistości uint24, a bajt 25
  byłby wtedy ich najstarszym bajtem. Dla wartości < 65 536 nie ma różnicy.
- Pomiary z pełnej sesji z 2026-11-24 (2208 ramek):
  - przyrosty dystansu tylko 0 albo 10 m, czasu 0 albo 1 s, ani jednego spadku żadnego licznika;
  - licznik bieżni: 390 → 2290 m (**1900 m**), kroki 808 → 3869 (**3061**), czas 763 → 2941 s (2178 s);
  - całka prędkości po czasie ścianowym: 1915,7 m (+0,8%), po czasie bieżni 1905,3 m;
  - zegar bieżni chodzi ok. 1,6% wolniej niż zegar komputera (2178 s wobec 2214 s);
  - kadencja i krok według prędkości (z licznika kroków Urevo):

    | km/h | czas | kadencja | długość kroku |
    |---|---|---|---|
    | 2,5 | 760 s | 74/min | 0,56 m |
    | 3,0 | 672 s | 83/min | 0,60 m |
    | 3,5 | 192 s | 90/min | 0,65 m |
    | 4,0 | 509 s | 95/min | 0,70 m |
    | 5,0 | 44 s | 110/min | 0,76 m |

    Kadencja rośnie z prędkością prawie liniowo (≈ 37 + 14,6·v), co potwierdza, że bit 13 to kroki.
- Pauza (`08 02`) **zachowuje** liczniki: w `tests/dane/urevo-5l-ramki.txt` dystans 10 m i czas 7 s
  zostają po zatrzymaniu. Ponowne połączenie BLE też ich nie zeruje: o 19:44:33 pierwsza ramka
  pokazuje 1220 m, 2422 kroki i 2199 s z trwającego treningu. Nie wiadomo, co robi pełny stop (`08 01`)
  i wyłączenie zasilania.
- Przy połączeniu liczniki bywają niezerowe (390 m / 808 kroków / 763 s), bo trening na bieżni
  zaczął się wcześniej. Obecny tracker bierze pierwszą wartość jako punkt odniesienia i niczego nie dolicza.

### 2.3. Symulator

`bieznia/sterowniki/symulator.py:69-82`: taśma przyspiesza o 1 km/h na sekundę, ramka co 0,75 s.
Powyżej 0,4 km/h kroki rosną z kadencją `60 + 15·v` na minutę (`symulator.py:77-78`). Symulator
nie ma licznika dystansu, więc dystans liczy aplikacja tak jak dla prawdziwej bieżni. Długość kroku
z tego wzoru: 0,22 m przy 1 km/h, 0,37 m przy 2 km/h, 0,48 m przy 3 km/h i 0,67 m przy 6 km/h.
Przy wolnym chodzie jest za krótka, poniżej progu 0,3 m z `reliable()`. Sesje z symulatora mają
`zrodlo = 'symulator'`, nie trafiają do poziomów (`poziomy/__init__.py:158`) ani na serwer
(`historia.py:237`), więc ten błąd nie wpływa na wyniki.

### 2.4. Silnik i mapowanie

- `Bridge.tick` (`engine.py:127`): `dt = min(1.0, now − last_tick)`, liczone po czasie klatek, a nie
  po czasie odczytów.
- `engine.py:149-151`: `if control.walk: distance_km += control.speed*dt/3600; active_s += dt`.
  `control.walk` jest fałszywe, gdy:
  - prędkość < `stand_below` („Chód od”, domyślnie 0,8 km/h, w ustawieniach od 0,0; `mapping.py:110`);
  - włączone jest szybkie zatrzymanie (hamowanie): spadek ≥ 0,6 km/h na odczyt albo spadek blisko progu
    (`mapping.py:28-34`, `mapping.py:105-107`), a taśma dalej jedzie;
  - od ostatniego odczytu minęły ponad 3 s (`stale_after_s`, `engine.py:139-146` → `_go_idle`).
- `state.distance_km` i `state.active_s` rosną od uruchomienia aplikacji i nigdy się nie zerują
  (`engine.py:33-34`). Nie zerują ich ani zmiana osoby, ani nowa sesja.
- Kadencja (`engine.py:115-121`): okno ostatnich 20 odczytów z krokami, wynik dopiero przy rozpiętości ≥ 3 s.
  Gdy licznik spadnie, kadencja się nie aktualizuje, dopóki stare próbki nie wyjdą z okna. `_go_idle`
  zeruje kadencję, ale nie czyści okna (`engine.py:108-113`).

### 2.5. Sesja i historia (`SessionTracker`, `historia.py:443-539`)

- `update(speed, steps, distance_km, active_s)` przyjmuje **sumy aplikacji** i liczy przyrosty
  `max(0, nowe − poprzednie)` (`historia.py:487-489`).
- Kroki (`historia.py:490-494`): `d = steps − last`, gdy `steps ≥ last`, a w przeciwnym razie `d = steps`
  (zakłada zerowanie do 0). Pierwszy odczyt służy za punkt odniesienia.
- Ruch: `speed > 0.05 or d_steps > 0`. Sesja zaczyna się od pierwszego ruchu i kończy po 120 s bez
  ruchu (`IDLE_END_S`). Zapis co 60 s. Sesje krótsze niż 10 s bez kroków nie są zapisywane.
- Aplikacja woła `update` co 5 klatek i **tylko przy `state.live`** (`gui/app.py:1333-1336`).
- Zmiana osoby, profilu, trybu albo źródła (model / sterownik / symulator) kończy sesję
  (`historia.py:459-482`).

### 2.6. Wyświetlanie

| Miejsce | Wartość | Skąd |
|---|---|---|
| Ekran główny „Kroki” (`gui/app.py:1328`, `gui/ekrany/glowny.py:133-137`) | `state.steps` | **surowy licznik bieżni** |
| „Kroki/min” (`gui/app.py:1329`) | `state.cadence` | okno 20 odczytów; „-” przy 0/None |
| „Dystans” (`gui/app.py:1330`) | `state.distance_km`, 2 miejsca | **suma od uruchomienia aplikacji** |
| „Czas ruchu” (`gui/app.py:1331-1332`) | `state.active_s` jako `m:ss` | suma od uruchomienia; po godzinie np. „75:03” |
| Licznik prędkości | `c.speed` | prędkość z bieżni (FTMS 0,01, KS 0,1 km/h) |
| Szczegóły sesji (`gui/ekrany/historia.py:356-364`) | kroki, dystans, czas ruchu, średnio, kroki/min | wartości sesji; kroki/min = kroki / (ruch_s/60), gdy ruch ≥ 30 s |
| Konto „na żywo” (`gui/ekrany/konto.py:146-153`) | `speed, steps, active` | **surowy licznik i czas od uruchomienia**, choć z `session_id` |

### 2.7. Wiarygodność (`poziomy/__init__.py:149-173`)

Te same zasady obowiązują na serwerze. Sesja liczy się, gdy: źródło = `bieznia`, `ruch_s > 0`,
`km ≥ 0`, średnia ≤ 20 km/h, a przy krokach > 0 także ≤ 250 kroków/min i długość kroku
`0,3 ≤ km·1000/kroki ≤ 1,3 m` (sprawdzana tylko przy km > 0). XP = 100 za km, więc każdy błąd
dystansu od razu przekłada się na poziom.

---

## 3. Weryfikacja na danych z historii

Odczyt tylko do odczytu (`mode=ro`). Obie bazy mają `user_version = 4`. Sesje z bazy w
`%LOCALAPPDATA%` (baza w repozytorium to podzbiór tych samych sesji, z inną nazwą osoby):

| Start | Ruch | Kroki | Dystans | Krok | Kadencja | Średnio | Bieżnia | Uwagi |
|---|---|---|---|---|---|---|---|---|
| 11-21 21:29 | 0,9 min | 82 | 0,035 km | **0,43 m** | 90 | 2,31 | (brak) | krótka, granica normy |
| 11-23 12:22 | 1,5 min | 62 | 0,060 km | **0,96 m** | **41** | 2,37 | (brak) | za mało kroków (opóźnienie licznika albo stanie z boku) |
| 11-23 12:41 | 3,4 min | 222 | 0,106 km | 0,48 m | 65 | 1,88 | (brak) | OK |
| 11-23 13:20 | 5,6 min | 538 | 0,271 km | 0,50 m | 97 | 2,93 | (brak) | OK |
| 11-24 21:07 | 36,6 min | **0** | 1,916 km | – | – | 3,14 | (brak) | starsza wersja nie czytała kroków Urevo; log ma 3061 kroków i 1900 m |
| 11-25 19:07 | 35,6 min | 2355 | 1,199 km | 0,51 m | 66 | 2,02 | Urevo 5L | licznik bieżni przy 19:44: 1220 m / 2422 kroki (z przerwą ~50 s) |
| 11-25 19:44 | 11,8 min | 974 | 0,595 km | 0,61 m | 83 | 3,03 | Urevo 5L | sesja rozdzielona przez restart aplikacji |
| 11-25 19:58 | 28,3 min | 2119 | 1,187 km | 0,56 m | 75 | 2,51 | Urevo 5L | OK |

Wnioski:

- W sesjach dłuższych niż 3 min długość kroku mieści się w przedziale 0,48–0,61 m, a więc w normie
  0,5–0,8 m dla chodu albo tuż pod nią (przy 1,9 km/h). Przy 2–3 km/h te wartości są realistyczne.
- Dystans liczony przez aplikację zgadza się z licznikiem bieżni z dokładnością do ~1% przy domyślnych
  ustawieniach: +0,8% w sesji 11-24 i −0,6% w sesji 11-25 19:07 po odjęciu przerwy w połączeniu.
- Powtórka sesji 11-24 przez obecne `SpeedMapper` i `Bridge` przy różnych wartościach „Chód od”:

  | `stand_below` | dystans w aplikacji | czas ruchu | licznik bieżni |
  |---|---|---|---|
  | 0,0 | 1911 m | 2188 s | 1900 m |
  | 0,8 | 1911 m | 2186 s | 1900 m |
  | 1,5 | 1910 m | 2184 s | 1900 m |
  | 2,6 | **1380 m** | **1422 s** | 1900 m |

  Dystans i czas zależą więc od ustawień gry, a nie od tego, co zrobiła taśma.
- Krótkie sesje dają nierealistyczny krok (0,96 m, 0,43 m), bo dystans liczony z prędkości startuje
  od razu, a licznik kroków bieżni zaczyna rosnąć z kilkusekundowym opóźnieniem.

---

## 4. Błędy i ryzyka

Waga: **W** wysoka (błędne wyniki w historii lub na serwerze), **Ś** średnia, **N** niska.

| # | Waga | Miejsce | Problem | Skutek |
|---|---|---|---|---|
| B1 | W | `engine.py:149-151`, `mapping.py:105-110` | Dystans i czas ruchu liczą się tylko przy `control.walk`, które zależy od progu „Chód od”, histerezy biegu, wykrywania hamowania i świeżości danych. | Przy wyższym progu brakuje do 27% dystansu, a chód na X21 w zakresie 0,5–0,7 km/h daje kroki bez dystansu i czasu (sesja `ruch_s = 0` odpada w `reliable()`). Hamowanie odcina kilka metrów przy każdym stopie. Krok wychodzi za krótki, więc sesja może odpaść na progu 0,3 m. |
| B2 | W | `kingsmith/sterownik.py:369-376`, `ftms/sterownik.py:169`, `kontrakt.py:15` | Liczniki dystansu i czasu z bieżni (KS `RunningDistance` / `RunningTotalTime`, FTMS bit 2 i bit 10) są pomijane, a kontrakt ich nie przenosi. | Dystans przy przerwie w połączeniu jest tracony (B6), aplikacja rozjeżdża się z wyświetlaczem bieżni i nie ma czym sprawdzić wiarygodności. |
| B3 | W | `historia.py:493` | Każdy spadek licznika kroków jest uznawany za zerowanie i doliczana jest cała nowa wartość. Brak progu wiarygodności. | Fantomowe kroki: (a) symulator → bieżnia z licznikiem 3000 (`_last_steps` z symulatora = 50): +2950 kroków; (b) chwilowy błędny odczyt 5000 → 4990 → 5002: +4992; (c) inna bieżnia z wyższym licznikiem. Długość kroku spada wtedy poniżej 0,3 m i cała sesja odpada w `reliable()`. |
| B4 | W | `historia.py:450-453`, `gui/app.py:405` (`open_history_db` z `reload_store` po odtworzeniu kopii) | Nowy `SessionTracker` startuje z `_last_distance = 0` i `_last_active = 0`, a sumy aplikacji są już niezerowe. | Pierwsza aktualizacja wpisuje do nowej sesji cały dystans i czas od uruchomienia aplikacji. |
| B5 | Ś | `gui/app.py:1328`, `gui/ekrany/konto.py:146-153` | Kafelek „Kroki” i podgląd na żywo pokazują surowy licznik bieżni, a „Czas ruchu” na żywo to czas od uruchomienia aplikacji. | Po połączeniu z bieżnią w trakcie treningu (Urevo 808 kroków) widać 808 zamiast 0. Wartości z kafelków nie zgadzają się z historią, a znajomi online widzą inne liczby niż w zapisanej sesji. |
| B6 | Ś | `engine.py:139-146`, `worker.py:332-382` | Przy przerwie w połączeniu (np. 50 s o 19:43:42–19:44:33) aplikacja nie dolicza dystansu ani czasu, choć liczniki bieżni je mają. Kroki z przerwy są doliczane z licznika, a dystans nie. | Niespójność kroków i dystansu oraz zaniżony dystans. |
| B7 | Ś | `gui/app.py:1333` | `SessionTracker.update` jest wołany tylko przy `state.live`. Gdy dane nie płyną, licznik 120 s bezczynności nie biegnie. | Po zerwaniu połączenia sesja wisi otwarta, a gdy dane wrócą po godzinach, ta sama sesja ciągnie się dalej: „Cała sesja” trwa godziny, a `by_hour` rozkłada kroki na puste godziny. |
| B8 | Ś | `ftms/sterownik.py:155-158` | Ramka FTMS z bitem „More Data” (bez prędkości) jest wyrzucana w całości. | Na bieżniach, które dzielą Treadmill Data na dwa powiadomienia, giną dystans, czas i kroki. |
| B9 | Ś | `kingsmith/sterownik.py:369-370` | Pole `unit` jest ignorowane. Przy `unit 1` (jednostki imperialne) `CurrentSpeed` i `RunningDistance` są najpewniej w milach. | Prędkość i dystans zaniżone o czynnik 1,609. Do sprawdzenia na żywej bieżni. |
| B10 | Ś | `ftms/protokol.py:58`, `ftms/protokol.py:120-121` | Kroki Urevo czytane jako uint16 bez obsługi przepełnienia. Pauza i ponowne połączenie nie zerują liczników. | Po 65 535 krokach (≈ 40 km bez wyłączenia bieżni) licznik się przekręca, a B3 dolicza wtedy tylko wartość po przekręceniu. Ryzyko niskie, ale realne przy całodniowym chodzeniu. Ponadto bajt 25 jest nieopisany (może to być uint24). |
| B11 | N | `ftms/protokol.py:102-105` | Tempo (bit 5 i 6) czytane jako uint8 zgodnie z FTMS 1.0. Część implementacji i nowsze opisy GSS podają inną szerokość. | Bieżnia z tymi bitami przesunęłaby wszystkie dalsze pola (energia, czas, kroki). Urevo ich nie ustawia. Dekoder nie sprawdza, czy zużył całą ramkę. |
| B12 | N | `engine.py:108-121` | Okno kadencji to 20 odczytów, a nie stały czas. Nie jest czyszczone przy `_go_idle` ani przy zerowaniu, a przy spadku licznika kadencja zamarza. Na KingSmith potwierdzenia co 0,75 s dodają próbki ze starym licznikiem. | Kadencja skacze (kroki X21 zmieniają się co 2 s), po zatrzymaniu przez ~15 s pokazuje malejące liczby, a po zerowaniu pokazuje starą wartość. |
| B13 | N | `engine.py:127` | `dt` liczone po klatkach i obcięte do 1 s. Integracja nie korzysta ze znaczników czasu odczytów. | Przy przyciętym oknie albo zawieszeniu UI dystans ginie (do 1 s na klatkę). Błąd zwykle pomijalny. |
| B14 | N | `symulator.py:77-78` | Kadencja `60 + 15·v` daje krok 0,22 m przy 1 km/h. Brak licznika dystansu i czasu. | Symulator nie ćwiczy ścieżki „licznik z bieżni”, a jego sesje miałyby nierealistyczny krok. |
| B15 | N | `poziomy/__init__.py:151-172` | Próg długości kroku obejmuje też bardzo krótkie sesje, w których licznik kroków bieżni startuje z opóźnieniem (patrz 0,96 m). | Krótkie sesje z małą liczbą kroków mogą odpaść. Zasady muszą zostać takie same jak na serwerze, więc tylko odnotowujemy. |
| B16 | N | `gui/app.py:1331-1332` | Czas ruchu `m:ss` bez godzin. | „75:03” zamiast „1:15:03”. |
| B17 | N | `kingsmith/sterownik.py:363-364` | Potwierdzenia `servers` dają „odczyt” z ostatnią prędkością do 6 s po ramce z danymi. | Dobre do podtrzymania „żywej” bieżni, ale do liczenia lepiej rozdzielić ramkę z danymi i sygnał życia (patrz R1). |
| B18 | N | `historia.py:510` | Sesja kończy się 120 s po ostatnim ruchu, ale `end = now`, więc dopisuje się też te 2 minuty bez ruchu. | „Cała sesja” jest dłuższa o czas oczekiwania. Kosmetyka (np. 11-25 19:44: 13,9 min zamiast ~12). |

---

## 5. Rekomendacje dla rdzenia w Ruście

### R1. Kontrakt: pełna ramka telemetrii zamiast (prędkość, kroki)

```rust
/// Jedna ramka z bieżni. None = pole nieobecne w tej ramce (NIE zero).
pub struct Telemetry {
    pub t: f64,                    // s, zegar monotoniczny w chwili odbioru
    pub speed_kmh: Option<f64>,    // już w km/h (KS: przeliczone, gdy unit=1)
    pub distance_m: Option<u64>,   // surowy licznik bieżni
    pub steps: Option<u64>,        // surowy licznik bieżni
    pub elapsed_s: Option<u64>,    // surowy licznik bieżni
    pub kind: FrameKind,           // Data | Heartbeat (KS "servers 0") | Merged (FTMS More Data)
}
pub struct CounterSpec { pub modulus: Option<u64>, pub quantum: u64 }
pub struct TelemetrySpec {
    pub distance: Option<CounterSpec>, // FTMS: modulus 2^24, quantum 10 (Urevo); KS: None modulus, quantum 10
    pub steps: Option<CounterSpec>,    // Urevo: modulus 2^16, quantum 1; KS: None, 1; brak licznika: None
    pub elapsed: Option<CounterSpec>,  // FTMS: 2^16, 1; KS: None, 1
    pub telemetry_s: f64,              // 1.0 FTMS, 2.0 KS, 0.75 symulator
    pub speed_max_kmh: f64,
}
```

- Heartbeat podtrzymuje tylko „bieżnia żyje” (świeżość danych). Nie dodaje próbek kadencji
  i nie zmienia liczników.
- FTMS: ramki z bitem „More Data” scalamy z następną ramką z prędkością. Każde pole bierzemy z
  najnowszej ramki, która je zawiera.
- Dekoder FTMS zwraca też liczbę nieprzeczytanych bajtów na końcu. Urevo: 1 bajt, do logu raz na
  połączenie. Ucięta ramka (pole wychodzi poza dane) jest odrzucana w całości.
- KingSmith: czytamy `CurrentSpeed`, `RunningSteps`, `RunningDistance` (m) i `RunningTotalTime` (s).
  Gdy `unit == 1`: `speed_kmh = v · 1,609344`, a licznik dystansu wyłączamy (`distance: None`),
  dopóki nie potwierdzimy jednostki na żywej bieżni. Ramki niepewne pomijamy jak dziś.

### R2. Licznik z bieżni: przyrost, przepełnienie, zerowanie, błąd odczytu

Jedna implementacja dla dystansu, kroków i czasu (`treadmill/counters.rs`):

```
stan: last: Option<u64>, pending: Option<u64>, total: u64 (przyrost w sesji), last_t
limit(dt) = ceil(max_rate · max(dt, 1 s) · 1,25) + quantum + slack
  max_rate: kroki 4/s (240/min), dystans speed_max/3,6 m/s, czas 1 s/s
  slack:    kroki 5, dystans 0, czas 2

feed(raw, t):
  brak last               → last = raw, przyrost 0                      (punkt odniesienia)
  raw ≥ last:
     d = raw − last
     d ≤ limit(dt)        → total += d, last = raw, pending = None      (zwykły przyrost)
     d > limit(dt)        → przyrost 0, last = raw, log „skok licznika” (np. inna bieżnia, długa przerwa)
  raw < last:
     modulus m, last ≥ m − limit(dt) i (raw + m − last) ≤ limit(dt)
                          → total += raw + m − last, last = raw         (przepełnienie)
     raw ≤ limit(dt)      → total += raw, last = raw                    (zerowanie do 0 i dalszy ruch)
     pending = None       → pending = raw, przyrost 0, last bez zmian   (podejrzany spadek: czekamy)
     pending = Some(p)    → last = raw, pending = None, total += max(0, raw − p)  (potwierdzony nowy punkt)
  pending i raw ≥ last    → pending = None, zwykły przyrost od last     (to był jednorazowy błąd)
```

- Przy zmianie źródła (inna bieżnia, symulator, inny sterownik), ponownym połączeniu po przerwie
  > 120 s albo nowym trackerze wszystkie liczniki wracają do stanu „brak last”. Usuwa to B3a i B4.
- Przy ponownym połączeniu z tą samą bieżnią (ten sam adres BLE i model) i przerwie ≤ 120 s liczniki
  zachowują `last`. Przyrost z przerwy jest doliczany, jeśli mieści się w `limit(dt)`, bo Urevo
  liczników nie zeruje.
- Liczników sprzed połączenia nie doliczamy (punkt odniesienia = pierwsza ramka), tak jak dziś.

### R3. Dystans: licznik bieżni jako prawda, całka z prędkości do płynnego wyświetlania

```
Całka (zawsze, niezależnie od gry):
  dla kolejnych ramek z prędkością (t0, v0) → (t1, v1):
    dt = t1 − t0
    dt ≤ 0                         → pomiń (zegar się cofnął albo duplikat)
    dt > gap = max(3 s, 3 · telemetry_s)  → przerwa: nie całkuj
    inaczej                        → I += v0 / 3,6 · dt          [m]   (wartość z lewego końca)

Licznik: C = distance.total (suma przyrostów z R2, w metrach), Q = distance.quantum (10 m)

Po każdej ramce:
  licznik dostępny:  I = clamp(I, C − Q, C + Q)
                     po przerwie (poprzedni odcinek był „gap”): I = max(I, C)
  dystans sesji D = I  (zapis i wyświetlanie), dystans_km = round(D) / 1000
  brak licznika:     D = I                                     (dystans_zrodlo = 'predkosc')
```

- Dlaczego tak: licznik bieżni zgadza się z jej wyświetlaczem i nie dryfuje, ale skacze co 10 m.
  Całka jest płynna, ale zależy od połączenia (+0,8% w danych z 11-24). Ograniczenie do
  `[C − Q, C + Q]` daje płynny odczyt i błąd ≤ 10 m względem bieżni. Na sesji z 11-24 wychodzi
  **1910 m** (licznik 1900 m, sama całka 1916 m).
- **Dystans nie zależy od `control.walk`.** Liczy się każda prędkość taśmy > 0, również poniżej
  progu chodu i w trakcie hamowania. Usuwa to B1.
- Proponowany schemat v6: kolumny `dystans_zrodlo TEXT ('licznik'|'predkosc')` i
  `kroki_zrodlo TEXT ('bieznia'|'brak')`. `dystans_km` i `kroki` zachowują dotychczasowe znaczenie,
  więc serwer i `reliable()` działają bez zmian.

### R4. Kroki

- Kroki sesji = `steps.total` z R2. Tylko z licznika bieżni, **bez szacowania**: ranking i serwer
  dostają wyłącznie zmierzone kroki.
- Bieżnia bez licznika (FTMS bez bitu 13, profil `steps=false`): `kroki = 0`, `kroki_zrodlo = 'brak'`,
  w kafelku „-”. Jeśli kiedyś pojawi się szacunek, tylko jako podpowiedź w interfejsie („≈”),
  nigdy w bazie.
- Kroki z bieżni obsługujemy w całości wzorem z R2: bez fantomów, z przepełnieniem uint16 i zerowaniem.

### R5. Czas ruchu

```
dla kolejnych ramek (t0, v0) → (t1, v1), 0 < dt ≤ gap:
    v0 > 0,05 km/h   → moving_s += dt
po przerwie (dt > gap), gdy licznik czasu bieżni dostępny:
    moving_s += min(dt, elapsed.total_po − elapsed.total_przed)
bez licznika czasu, ale z przyrostem dystansu C w przerwie:
    moving_s += min(dt, ΔC / (v0 / 3,6))   (v0 > 0,05; przy v0 = 0 nic)
```

Czas ruchu mierzymy zegarem komputera, bo zegar Urevo chodzi ~1,6% wolniej, a licznik ma
rozdzielczość 1 s. Licznik bieżni służy tylko do uzupełniania przerw. Czas ruchu też **nie zależy
od gry**.

### R6. Kadencja

- **Na żywo:** okno czasowe W = 15 s z próbek `(t, steps.total)`, tylko z ramek, które niosą kroki.
  `cadence = (S_now − S_old) / (t_now − t_old) · 60`, gdzie `S_old` to najstarsza próbka z
  `t_now − t ≤ W`. Wynik tylko wtedy, gdy rozpiętość ≥ 6 s (X21 podaje kroki co 2 s, a kwant
  1 kroku przy 6 s to ±10/min). W przeciwnym razie `None`.
- `None`, gdy prędkość ≤ 0,05 km/h od ≥ 3 s albo dane nie są świeże. Okno czyścimy przy przerwie,
  zmianie źródła i nowej sesji. Zerowanie licznika nie zakłóca kadencji, bo `steps.total` jest monotoniczne.
- **Sesja:** `kroki / (moving_s / 60)`, gdy `moving_s ≥ 30`, w przeciwnym razie „-” (jak dziś).

### R7. Prędkość i średnia

- Prędkość bierzemy z bieżni: FTMS `u16 / 100`, KS liczba z tekstu, `max(0, v)`, a `runState 0` → 0.
  Wyświetlamy z dokładnością 0,1. Do całki używamy pełnej rozdzielczości.
- Średnia sesji: `D_km / (moving_s / 3600)`, gdy `moving_s > 0` (jak `Session.avg_kmh`).
- Długość kroku (diagnostyka i log, nie interfejs): `D_m / kroki`, gdy kroki ≥ 50.

### R8. Sesja (tracker)

- Tracker trzyma własne akumulatory (R2–R6), a nie przyrosty sum aplikacji. Usuwa to B4.
- Tracker dostaje zdarzenie „tik” co sekundę **także bez danych**. Sesja kończy się, gdy przez 120 s
  nie ma ruchu (prędkość ≤ 0,05 i brak przyrostu kroków) **albo nie ma żadnych danych**. Usuwa to B7.
  `end` = czas ostatniego ruchu, a nie chwila zamknięcia (B18).
- Zmiana osoby, profilu, trybu albo źródła kończy sesję tak jak dziś. Zmiana źródła zeruje dodatkowo
  liczniki (R2).
- Kafelki na ekranie i podgląd na żywo pokazują **wartości bieżącej sesji**: kroki, dystans, czas
  ruchu i kadencję. Po zakończeniu sesji pokazują ostatnią sesję do następnego ruchu. Czas
  ruchu formatujemy jako `m:ss`, a od godziny `h:mm:ss`.
- Reguły wiarygodności (`reliable`, `walks_from_sessions`) przenosimy 1:1, bo muszą być identyczne
  z serwerem. Nie zmieniamy ich w 0.6.0.

### R9. Symulator

- Symulator generuje pełną ramkę `Telemetry` z liczników, które przechodzą tę samą ścieżkę co prawdziwa
  bieżnia:
  - dystans: dokładny `∫v dt`, raportowany jako `floor(m / 10) · 10` (quantum 10 m);
  - kroki: kadencja `c(v) = 40 + 14·v` na minutę przy v > 0,4 km/h, czyli 82/min i krok 0,61 m przy 3 km/h,
    96/min i 0,69 m przy 4 km/h, 54/min i 0,31 m przy 1 km/h. Ten wzór jest dopasowany do danych z Urevo;
  - czas: `floor(∫[v > 0] dt)`.
- Symulator ma wstrzykiwany zegar (testy deterministyczne), a jego sesje mają `zrodlo = 'symulator'`
  jak dziś.

### R10. Do sprawdzenia na żywych bieżniach (nie blokuje 0.6.0)

- Urevo: czy pełny stop (`08 01`) albo wyłączenie zeruje liczniki; czy bajt 25 to najstarszy bajt
  kroków (uint24).
- KingSmith: czy `runState 0` (stop) zeruje `RunningSteps` i `RunningDistance`; czy `unit 1` zmienia
  jednostki dystansu; co oznacza jednostka `BurnCalories`.
- Inne bieżnie FTMS: szerokość pola tempa (bity 5 i 6) i opcjonalny odczyt Feature 0x2ACC
  (bit „Total Distance Supported”), żeby wiedzieć z góry, czy licznik dystansu będzie.

---

## 6. Przypadki testowe dla rdzenia w Ruście

Konwencje: `t` w sekundach, prędkość w km/h, dystans w m. „±” to dopuszczalna tolerancja.
Ramki FTMS to zapis szesnastkowy bez spacji.

### 6.1. Dekodowanie ramek

| # | Wejście | Oczekiwany wynik |
|---|---|---|
| D1 | FTMS `9c25fa008601002800000000000000250000000000fb02280300` | speed 2,50; distance 390; incline 4,0; energy 37; elapsed 763; steps 808; 1 bajt nadmiarowy |
| D2 | FTMS `9c25640000000000000000000000000000000000000000000000` | speed 1,00; distance 0; elapsed 0; steps 0 |
| D3 | FTMS `9c255a000a000000000000000000000000000000000700000000` | speed 0,90; distance 10; elapsed 7; steps 0 |
| D4 | D1 obcięta do 24 bajtów (bez kroków) | błąd „ucięta ramka”, żadne pole nie trafia dalej |
| D5 | FTMS flagi `0x259D` (More Data), reszta jak w D1 bez bajtów 2–3 | speed None; distance 390, elapsed 763, steps 808 scalone z następną ramką z prędkością |
| D6 | FTMS flagi `0x0080`, energia `ffff 0000 00` (+ prędkość) | energy None (0xFFFF = brak) |
| D7 | KS `props netled 0 ControlMode 1 CurrentSpeed 2.3 RunningTotalTime 169 RunningSteps 232 RunningDistance 100 BurnCalories 6800` | speed 2,3; elapsed 169; steps 232; distance 100 |
| D8 | KS `props netled 0 ControlMode 1 CurrentSpeed 2.0 RunningTotalTime 3 RunningDistance 0 BurnCalories 0` | steps None (nie 0); poprzednia wartość kroków bez zmian |
| D9 | KS `props CurrentSpeed 2.0 unit 1` | speed 3,22 (2,0 · 1,609344, zaokr. do 0,01); licznik dystansu wyłączony |
| D10 | KS `props runState 0 CurrentSpeed 1.5` | speed 0 |
| D11 | KS `servers 0` | Heartbeat: bez zmian liczników i bez próbki kadencji, świeżość odnowiona |

### 6.2. Liczniki (R2), na przykładzie kroków (rate 4/s, slack 5)

| # | Wejście (t: raw) | Oczekiwany `steps.total` |
|---|---|---|
| C1 | 0: 808 | 0 (punkt odniesienia) |
| C2 | 0: 808, 1: 809, 2: 811 | 3 |
| C3 | 0: 5000, 1: 3 | 3 (zerowanie) |
| C4 | modulus 65536; 0: 65530, 2: 4 | 10 (przepełnienie) |
| C5 | 0: 5000, 1: 4990, 2: 5002 | 2 (jednorazowy błąd odczytu zignorowany) |
| C6 | 0: 5000, 1: 4000, 2: 4001 | 1 (spadek potwierdzony, nowy punkt odniesienia) |
| C7 | 0: 1000, 1: 9000, 2: 9002 | 2 (skok 8000 w 1 s pominięty) |
| C8 | 0: 2400, 50: 2460 (ta sama bieżnia, przerwa 50 s) | 60 |
| C9 | symulator 0: 0 … 60: 50, zmiana źródła, bieżnia 61: 3000, 62: 3002 | sesja symulatora 50; nowa sesja bieżni 2 (brak +2950) |
| C10 | dystans, modulus 2^24, quantum 10, speed_max 6; 0: 16777210, 1: 5 | 11 |
| C11 | dystans; 0: 390, 1: 390, 2: 400, 3: 400 | 10 |
| C12 | czas FTMS; 0: 763, 1: 764, 2: 764, 3: 765 | 2 |
| C13 | steps None przez 10 ramek, potem 120, 121 | 1 (pierwsza wartość = punkt odniesienia) |

### 6.3. Dystans i czas ruchu (R3, R5)

| # | Wejście | Oczekiwany wynik |
|---|---|---|
| I1 | ramki co 1,0 s, t = 0…600, stale 3,0 km/h, bez licznika | D = 500,0 m ± 0,01; moving 600 s |
| I2 | jak I1, licznik dystansu rośnie co 10 m zgodnie z floor(prawdziwy/10)·10 | D w każdej chwili w [C − 10, C + 10]; na końcu 500 m ± 10 |
| I3 | 0,5 km/h przez 360 s (ramki co 2 s), „Chód od” = 0,8 | D = 50 m ± 0,5; moving 360 s (nie zależy od gry) |
| I4 | ramki co 1 s: 3,0; 2,0; 1,0; 0,0 (hamowanie) | D = (3+2+1)/3,6 = 1,667 m; moving 3 s |
| I5 | C = 100, całka 115 | D = 110 |
| I6 | C = 100, całka 85 | D = 90 |
| I7 | ramki 0…10 s przy 3,6 km/h, następna przy t = 60 s; licznik 0 → 60 m | I po 10 s = 10 m; po przerwie D = 60; moving = 10 + min(50, Δelapsed) |
| I8 | telemetry_s 2 (KS), odstęp 5,9 s | całkowane (gap = 6 s) |
| I9 | telemetry_s 1 (FTMS), odstęp 3,5 s | przerwa, nie całkujemy |
| I10 | t: 10, 9 (zegar wstecz), 11 | odcinek 10→9 pominięty, bez wartości ujemnych |
| I11 | prędkość 0 przez 300 s | D = 0, moving 0 |
| I12 | replay `x21-ramki.txt` (v5), ograniczenie po każdej ramce | steps 230; C = 100; sama całka 110,2; D = 108,2 ± 0,5; moving 166,7 s ± 1 |
| I13 | replay pełnej sesji Urevo z 2026-11-24 21:07 (2208 ramek; do skopiowania z logu do `tests/dane/urevo-5l-sesja-37min.txt`) | steps 3061; C = 1900; D = 1910 ± 1; moving 2193 s ± 5; krok 0,62 m ± 0,01; kadencja sesji 83,7 ± 0,5 |
| I14 | replay `urevo-5l-ramki.txt` | prędkość max 4,00; C = 10; steps 0; po `status 0202` prędkość 0, liczniki bez zmian |

### 6.4. Kadencja (R6)

| # | Wejście | Oczekiwany wynik |
|---|---|---|
| K1 | kroki +3 co 2 s przez 20 s (X21) | 90/min ± 1 |
| K2 | jak K1, ale tylko 4 s danych | None (rozpiętość < 6 s) |
| K3 | po K1 taśma stoi (prędkość 0), kroki bez zmian przez 3 s | None |
| K4 | kroki 5000…5030 w 20 s, potem zerowanie 0, 1, 2 … dalej +1,5/s | po zerowaniu wciąż ~90/min (z `steps.total`), bez zamarzania |
| K5 | Urevo, 2,5 km/h stale przez 60 s (dane z I13) | 74/min ± 3 |
| K6 | heartbeat KS co 0,75 s pomiędzy ramkami | nie zmienia wyniku K1 |

### 6.5. Sesja (R8)

| # | Wejście | Oczekiwany wynik |
|---|---|---|
| S1 | ruch 300 s przy 2,5 km/h, potem 122 s stania (ramki dalej płyną) | 1 sesja; D = 208,3 m ± 10; `end` = ostatni ruch (nie +120 s) |
| S2 | ruch 300 s, potem brak jakichkolwiek ramek, tik co 1 s przez 121 s | sesja zamknięta po 120 s bez danych |
| S3 | ruch, utrata połączenia na 3 h, powrót danych i ruch | 2 osobne sesje |
| S4 | nowy tracker utworzony w trakcie działania aplikacji (odtworzenie kopii) | pierwsza sesja zaczyna od 0 m / 0 s / 0 kroków |
| S5 | Urevo: pauza 100 s (liczniki zachowane), wznowienie | jedna sesja; liczniki ciągłe |
| S6 | Urevo: przy połączeniu licznik 390 m / 808 kroków / 763 s | sesja zaczyna od 0 m / 0 kroków |
| S7 | zmiana osoby w trakcie ruchu | 2 sesje; suma kroków = przyrost licznika bieżni |
| S8 | bieżnia bez licznika kroków (FTMS bez bitu 13), 600 s przy 3 km/h | kroki 0, `kroki_zrodlo = 'brak'`; D = 500 m; kafelek „Kroki” = „-” |
| S9 | kafelki w trakcie S6 po 60 s przy 3 km/h | kroki = przyrost (np. 83), nie 891; dystans ≈ 50 m, nie suma od uruchomienia |
| S10 | sesja krótsza niż 10 s bez kroków | niezapisana (jak dziś) |

### 6.6. Wiarygodność (zgodność z serwerem, `reliable` 1:1)

| # | Sesja (źródło, km, kroki, ruch_s) | Oczekiwany wynik |
|---|---|---|
| W1 | bieznia, 1,199, 2355, 2136 | true (krok 0,51 m) |
| W2 | bieznia, 0,060, 62, 90 | true (krok 0,97 m) |
| W3 | bieznia, 0,100, 2950, 600 | false (krok 0,034 m < 0,3) |
| W4 | bieznia, 1,916, 0, 2196 | true (bez sprawdzania kroków) |
| W5 | symulator, 1,0, 1600, 1200 | false |
| W6 | bieznia, 1,0, 1500, 150 | false (24 km/h > 20) |
| W7 | bieznia, 0,5, 3000, 600 | false (300/min > 250) |
| W8 | bieznia, 0,0, 50, 0 | false (ruch_s = 0) |
| W9 | bieznia, 0,5, 300, 600 | false (krok 1,67 m > 1,3) |
| W10 | `walks_from_sessions`: dwie sesje, druga zaczyna się przed końcem pierwszej | druga pominięta |
| W11 | `walks_from_sessions`: 9 h ruchu w jednym dniu lokalnym | km i kroki obcięte proporcjonalnie do 8 h |

### 6.7. Symulator (R9)

| # | Wejście | Oczekiwany wynik |
|---|---|---|
| M1 | 3,0 km/h stale przez 60 s (po rozpędzie) | kroki 82 ± 1; licznik dystansu 50 (floor do 10 m); D = 50 m ± 0,1 |
| M2 | 1,0 km/h przez 60 s | kroki 54 ± 1; krok ≈ 0,31 m |
| M3 | 0,3 km/h | kroki nie rosną (v ≤ 0,4) |
| M4 | ten sam zegar wstrzyknięty dwa razy | identyczne ramki (determinizm) |

### 6.8. Formatowanie

| # | Wejście | Oczekiwany wynik |
|---|---|---|
| F1 | 1,2345 km | „1,23 km” |
| F2 | czas ruchu 3725 s | „1:02:05” |
| F3 | czas ruchu 125 s | „2:05” |
| F4 | kadencja None | „-” |
| F5 | prędkość 2,549 | „2,5” |
| F6 | kroki None (brak licznika) | „-” |
