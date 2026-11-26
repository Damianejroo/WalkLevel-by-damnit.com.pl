# WalkLevel 0.6.0: słownik tekstów interfejsu

Jedyne źródło tekstów dla nowego interfejsu (Tauri 2 + Svelte 5). Powstało z pełnego przeglądu
tekstów wersji 0.5.2 w `bieznia/gui/`, `bieznia/konto/`, `bieznia/zgloszenia.py`,
`bieznia/aktualizacje.py`, `bieznia/kopia.py`, `bieznia/klawisze.py`, `bieznia/pad.py`,
`bieznia/ble/` i `bieznia/sterowniki/`, a także `packaging/instalator.iss`.

Kolumny w tabelach:

- **Klucz**: identyfikator w stylu `ekran.element` (po polsku, bez znaków diakrytycznych). W kodzie
  Svelte tekst pobiera się po kluczu, np. `t('glowny.status.szukanie.tytul', { model })`.
- **Obecny tekst**: tekst z 0.5.2 (zmienne zapisane już nowymi nazwami). „(nowy)” oznacza tekst,
  którego dziś nie ma, a jest potrzebny (brakujący stan, brakująca podpowiedź).
- **Nowy tekst**: tekst do wstawienia w 0.6.0.
- **Uwaga**: dlaczego zmiana, gdzie tekst się pokazuje, uwagi dla programisty.

---

## 0. Zasady i konwencje

### 0.1. Zasady pisania

1. **Co się dzieje i co zrobić.** Każdy komunikat o problemie ma dwie części: stan („Bluetooth jest
   wyłączony.”) i działanie („Włącz go w Start > Ustawienia > Bluetooth i urządzenia.”).
2. **Bez rodzaju gramatycznego.** Nie piszemy „zalogowałeś/aś”, „gotowy”, „Twoim znajomym”.
   Używamy trybu rozkazującego („Kliknij”), czasu teraźniejszego i przyszłego („Masz”,
   „zalogujesz się”), form bezosobowych („Wysłano”, „Zapisano”). Aplikacja też nie ma rodzaju:
   zamiast „Nie znalazłem” piszemy „Nie ma pliku gry w tym folderze.”, zamiast „Wysłaliśmy”
   piszemy „Wysłano”.
3. **Krótko.** Jedno zdanie, gdy wystarczy. Podpowiedź pod przełącznikiem: najwyżej dwa zdania.
4. **Bez wykrzykników i emoji.** Także w podziękowaniach.
5. **Miejsca pełną ścieżką.** Zawsze nazwy dokładnie jak na ekranie, rozdzielone „ > ”, np.
   „Ustawienia > Gry i sterowanie > Sterowanie”. Nazwy kategorii i kart są w sekcji 9 i trzeba ich
   pilnować: zmiana nazwy na ekranie = zmiana we wszystkich ścieżkach (wyszukać klucz `sciezka.`).
6. **Bez żargonu.** Nie używamy słów: fokus, token, firmware, dBm, JSON, ZIP, SHA, popup, log w
   tekstach dla gracza (wyjątki: ekran Dziennik i raport diagnostyczny, które czyta autor).
   Zamienniki są w słowniczku 0.3.
7. **Nazwa aplikacji: WalkLevel.** Bez „Bieżnia” jako nazwy aplikacji i bez „Bieżnia Gamingowa”.
   Słowo „bieżnia” oznacza tylko urządzenie.
8. **Hasło bez zmian:** „Chodzisz w realu, idziesz w grze” (klucz `app.haslo`).
9. **Zwracanie się do gracza:** „Ty”, „Twój”, „Ciebie” wielką literą w całym interfejsie (tak jak
   w 0.5.2). Teksty z `definicje.json` (osiągnięcia) mają dziś małą literę, patrz sekcja 28.

### 0.2. Typografia i liczby

| Zasada | Przykład | Uwaga |
|---|---|---|
| Myślnik „ - ” między zdaniami zastępujemy kropką, dwukropkiem albo przecinkiem | „Opis jest za krótki. Dopisz jeszcze 5 znaków.” | W 0.5.2 prawie każdy komunikat łączył dwie myśli „ - ”, co czyta się jak skrót myślowy. |
| Zakresy: półpauza bez spacji | „1,0–6,0 km/h”, „3–24 znaki” | |
| Ułamki z przecinkiem | „2,5 km/h”, „12,34 km” | Formatowanie w jednej funkcji `liczba(x, miejsca)`. |
| Tysiące z twardą spacją | „12 000 kroków” | `Intl.NumberFormat('pl-PL')`, ale z wymuszoną spacją także dla liczb 4-cyfrowych (pl-PL domyślnie jej nie wstawia). |
| Trwające czynności kończy wielokropek „…” | „Łączę z bieżnią…” | Jeden znak `…`, nie trzy kropki. |
| Przyciski otwierające dalszy wybór kończy „…” | „Eksportuj…” | Tylko gdy po kliknięciu trzeba jeszcze coś wybrać. |
| Cudzysłów polski | „Tylko ja” | |
| Separator w jednej linii: „ · ” | „Pad · 4 stopnie · do 6,0 km/h” | Tylko w podpisach i podsumowaniach, nie w zdaniach. |
| Nazwy klawiszy jak na klawiaturze | „Ctrl+Shift+F8”, „Spacja”, „Strzałka w prawo” | Nazwy w sekcji 24. |

### 0.3. Słowniczek (jedno słowo na jedną rzecz)

| Pojęcie | Używamy | Nie używamy | Uwaga |
|---|---|---|---|
| Ruchoma część bieżni | taśma | pas | |
| Włączenie sterowania postacią/taśmą | sterowanie (włączone/wyłączone) | | |
| Skrót włączający sterowanie | klawisz włączania | skrót start/stop, toggle | Rola przycisku przypisywania: „Klawisz włączania”. |
| Próg wychylenia gałki w trybie Pad | stopień (Stopień 1, Stopień 2…) | skok | „Skok” w grach to skok postaci, a aplikacja ostrzega przed przyciskiem „skoku”. Myli się. |
| Stałe tempa w trybie Trening | poziom (Poziom 1–4) | | Kolizja z poziomem XP jest akceptowalna, bo zawsze stoi przy słowie „Trening”. |
| Poziom za kilometry | poziom, ranga, XP | level | |
| Okienko osiągnięcia w rogu ekranu | powiadomienie | popup | |
| Zapis jednego chodzenia | sesja | trening (w znaczeniu sesji) | |
| Ustawienia dla jednej gry | profil gry | profil (bez dopowiedzenia w tekstach ogólnych) | Profil modelu bieżni to „profil bieżni”. |
| Osoba korzystająca z aplikacji na tym komputerze | osoba | użytkownik, gracz (w tekstach UI) | |
| Konto na serwerze | konto online | | |
| Tryb, w którym gra nie dostaje klawiszy | tryb próbny | podgląd, dry | Patrz `ustawienia.chodzenie.klawisze_w_grze`. |
| Wersja oprogramowania bieżni | wersja oprogramowania bieżni | firmware | |
| Siła sygnału Bluetooth | sygnał: silny / średni / słaby | dBm | Progi: ≥ -60 silny, -61…-75 średni, < -75 słaby. Wartość w dBm tylko w raporcie. |
| Pliki dziennika | dziennik (dla gracza), logi (w zgłoszeniu, gdzie gracz wie, co wysyła) | | |
| Plik kopii danych | plik kopii WalkLevel | archiwum ZIP, JSON | |

### 0.4. Zmienne i odmiana

- Zmienne w nawiasach klamrowych: `{kroki}`, `{km}`, `{kmh}`, `{model}` itd. Lista nazw w 0.5.
- Odmiana po liczbie: `{zmienna:forma1/forma2-4/forma5}`, np. `{kroki} {kroki:krok/kroki/kroków}`.
  Reguła polska: 1 → forma1; końcówka 2–4 (bez 12–14) → forma2-4; reszta → forma5. Funkcja
  `odmiana(n, f1, f2, f5)` w jednym miejscu.
- Liczby wstawiane do tekstu są już sformatowane (patrz 0.2). `{kmh}` zawsze z jednym miejscem po
  przecinku, `{km}` z dwoma na ekranie głównym i w sesji, z jednym w podsumowaniach.
- `\n` w nowym tekście oznacza złamanie wiersza (np. w dymku).

### 0.5. Nazwy zmiennych

| Zmienna | Znaczenie |
|---|---|
| `{kroki}` | liczba kroków |
| `{km}` | dystans w km |
| `{kmh}`, `{min_kmh}`, `{max_kmh}` | prędkość, zakres prędkości |
| `{czas}` | czas ruchu sformatowany („1 h 05 min”, „12 min”) |
| `{model}` | nazwa modelu bieżni („KingSmith X21”) |
| `{nazwa_bt}` | nazwa Bluetooth urządzenia („KS-NACH-MXG”) |
| `{sygnal}` | „silny”, „średni” albo „słaby” |
| `{osoba}` | nazwa osoby na tym komputerze |
| `{profil}` | nazwa profilu gry |
| `{tryb}` | nazwa trybu: Chodzenie, Klawisze, Pad, Trening |
| `{pseudonim}`, `{email}` | dane konta online |
| `{wersja}`, `{nowa_wersja}` | numer wersji aplikacji |
| `{klawisz_wlaczania}` | np. „F8” |
| `{klawisz_chodu}`, `{klawisz_biegu}` | np. „W”, „Shift” |
| `{klawisz_szybciej}`, `{klawisz_wolniej}` | klawisze symulatora i treningu (np. „F7”, „F6”) |
| `{przycisk_pada}` | np. „Panel dotykowy” |
| `{pad}` | nazwa pada („DualSense (Bluetooth)”) |
| `{procent}` | liczba procent bez znaku % |
| `{stopien}`, `{poziom}` | numer stopnia pada, numer poziomu treningu albo poziomu XP |
| `{xp}`, `{xp_max}` | punkty doświadczenia |
| `{ranga}`, `{opis_rangi}` | nazwa i opis rangi |
| `{liczba}`, `{razem}` | liczby ogólne |
| `{data}`, `{godzina}`, `{dzien}` | „26.11.2026”, „18:40”, „Dziś”/„Wczoraj”/„24.11” |
| `{sekundy}`, `{minuty}`, `{dni}` | liczby czasu |
| `{kod}`, `{numer}` | kod zaproszenia, numer zgłoszenia |
| `{plik}`, `{folder}` | nazwa pliku, ścieżka folderu |
| `{szczegoly}` | techniczny dopisek z błędu (zawsze na końcu, w nawiasie) |

---

## 1. Aplikacja i okno

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `app.nazwa` | WalkLevel | WalkLevel | |
| `app.haslo` | (tylko w mailach: Chodzisz w realu, idziesz w grze) | Chodzisz w realu, idziesz w grze | Bez zmian. Do okna „O aplikacji” i ekranu pierwszego uruchomienia. |
| `app.wersja` | wersja {wersja} | wersja {wersja} | Pod nazwą na pasku. |
| `app.osoba_i_wersja` | {osoba} · wersja {wersja} | {osoba} · wersja {wersja} | Gdy na komputerze jest więcej niż jedna osoba. |
| `app.o_aplikacji` | WalkLevel {wersja} · © 2026 {autor} | WalkLevel {wersja} · © 2026 {autor} | |
| `app.link.strona` | Strona autora | Strona autora | |
| `app.link.prywatnosc` | Prywatność | Polityka prywatności | Pełna nazwa dokumentu, jak przy rejestracji. |
| `app.link.kod` | Kod | Kod źródłowy | „Kod” myli się z kodem zaproszenia. |
| `app.wroc` | Wróć | Wróć | Przycisk w nagłówku podstron. |
| `app.zapis.auto` | Zmiany zapisują się automatycznie | Zmiany zapisują się same | |
| `app.zapis.zapisano` | Zapisano | Zapisano | Krótki znacznik po zmianie. |
| `app.zapis.dostepnosc` | Stan zapisu | Stan zapisu | Nazwa dla czytnika ekranu. |
| `app.zamykanie.tasma` | (nowy) | Zatrzymuję taśmę i zamykam WalkLevel… | Dziś tylko w logu („Zamykanie: zatrzymano taśmę”). Pokazać, gdy zamknięcie czeka na stop taśmy. |

## 2. Ekran główny: pasek górny

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `glowny.pasek.osoba` | Zmień osobę | Zmień osobę | Podpowiedź i nazwa dostępności awatara. |
| `glowny.pasek.aktualizuj` | Aktualizuj | Aktualizuj | |
| `glowny.pasek.aktualizuj_do` | Aktualizuj do wersji {nowa_wersja} | Zainstaluj wersję {nowa_wersja} | Podpowiedź, gdy jest nowa wersja. |
| `glowny.pasek.aktualizuj_niedostepne` | Aktualizacje działają w wersji zainstalowanej | Aktualizacje działają tylko w wersji zainstalowanej instalatorem. | Wersja deweloperska. |
| `glowny.pasek.zglos` | Zgłoś problem | Zgłoś problem | |
| `glowny.pasek.online` | Znajomi i ranking | Znajomi i ranking | Widoczne tylko z kontem online. |
| `glowny.pasek.historia` | Historia kroków | Historia kroków | |
| `glowny.pasek.ustawienia` | Ustawienia | Ustawienia | |
| `glowny.osoby.dzis` | dziś {kroki} kroków | dziś {kroki} {kroki:krok/kroki/kroków} | Lista osób pod awatarem. Dziś brak odmiany („dziś 1 kroków”). |
| `glowny.osoby.dodaj` | Dodaj osobę | Dodaj osobę | |
| `glowny.osoby.zarzadzaj` | Zarządzaj osobami | Zarządzaj osobami | Otwiera Ustawienia > Konto i osoby. |

## 3. Ekran główny: karta połączenia z bieżnią

Karta ma tytuł, podtytuł i przycisk. W 0.5.2 podtytuł w stanie „łączenie” brzmiał „Bieżnia włączona,
telefon rozłączony”, co wyglądało jak stan, a było warunkiem. Po połączeniu podtytuł pokazywał
techniczny opis sterownika (np. „KS-NACH-MXG (KingSmith, usługa fed7, tablica 2)”).

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `glowny.status.brak.tytul` | Bieżnia nie połączona | Bieżnia nie jest połączona | Brak zapamiętanej bieżni. |
| `glowny.status.brak.podtytul` | Włącz bieżnię i kliknij Szukaj bieżni | Włącz bieżnię i kliknij Szukaj bieżni. | |
| `glowny.status.brak.akcja` | Szukaj bieżni | Szukaj bieżni | |
| `glowny.status.rozlaczona.tytul` | (nowy; dziś „Bieżnia nie połączona”) | Rozłączono z {model} | Bieżnia zapamiętana, ale połączenie zakończone przyciskiem Rozłącz. |
| `glowny.status.rozlaczona.podtytul` | (nowy) | Kliknij Połącz, żeby połączyć ponownie. | |
| `glowny.status.rozlaczona.akcja` | (nowy; dziś „Szukaj bieżni”) | Połącz | Łączy z zapamiętaną bieżnią bez skanowania. |
| `glowny.status.skanowanie.tytul` | Szukam urządzeń Bluetooth… | Szukam bieżni w pobliżu… | |
| `glowny.status.skanowanie.podtytul` | Bieżnia włączona, telefon rozłączony | Bieżnia musi być włączona, a jej aplikacja w telefonie zamknięta. | |
| `glowny.status.skanowanie.akcja` | Szukaj bieżni | Anuluj | W 0.5.2 przycisk „Szukaj bieżni” w trakcie szukania. |
| `glowny.status.szukanie.tytul` | Szukam {nazwa_bt}… | Szukam bieżni {model}… | Zapamiętana bieżnia, przed połączeniem. Nazwa modelu zamiast nazwy Bluetooth. |
| `glowny.status.szukanie.podtytul` | Bieżnia włączona, telefon rozłączony | Bieżnia musi być włączona, a jej aplikacja w telefonie zamknięta. | |
| `glowny.status.szukanie.akcja` | Anuluj | Anuluj | |
| `glowny.status.laczenie.tytul` | Łączę z {nazwa_bt}… | Łączę z {model}… | |
| `glowny.status.laczenie.podtytul` | Bieżnia włączona, telefon rozłączony | To trwa zwykle kilka sekund. | |
| `glowny.status.laczenie.akcja` | Anuluj | Anuluj | |
| `glowny.status.polaczono.tytul` | Połączono: {model} | Połączono z {model} | |
| `glowny.status.polaczono.podtytul` | {opis techniczny sterownika} | Taśma {min_kmh}–{max_kmh} km/h | Zamiast opisu sterownika. Opis techniczny tylko w Dzienniku. |
| `glowny.status.polaczono.akcja` | Rozłącz | Rozłącz | |
| `glowny.status.czekam.tytul` | Połączono: {model} | Połączono z {model} | Połączono, ale od kilku sekund brak danych. |
| `glowny.status.czekam.podtytul` | Czekam na dane z bieżni | Czekam na dane z bieżni. Jeśli to trwa, wyłącz i włącz bieżnię. | |
| `glowny.status.ponawiam.tytul` | Ponawiam połączenie | Łączę ponownie… | Podtytuł: jeden z tekstów `ble.powod.*`. |
| `glowny.status.ponawiam.akcja` | Rozłącz | Przerwij | Przerywa ponawianie, bieżnia zostaje zapamiętana. |
| `glowny.status.bt_wylaczony.tytul` | Bluetooth wyłączony | Bluetooth jest wyłączony | |
| `glowny.status.bt_wylaczony.podtytul` | Włącz Bluetooth w Windows | Włącz go w Start > Ustawienia > Bluetooth i urządzenia. | |
| `glowny.status.bt_wylaczony.akcja` | Spróbuj ponownie | Spróbuj ponownie | |
| `glowny.status.nieobslugiwane.tytul` | To urządzenie nie jest bieżnią | To urządzenie nie jest obsługiwaną bieżnią | |
| `glowny.status.nieobslugiwane.podtytul` | Wybierz inne albo zapisz raport | Wybierz inne urządzenie. Jeśli to bieżnia, zgłoś ją w Ustawienia > Pomoc i diagnostyka > Zgłoś problem. | |
| `glowny.status.nieobslugiwane.akcja` | Szukaj bieżni | Szukaj bieżni | |
| `glowny.status.symulator.tytul` | Symulator bieżni | Symulator bieżni jest włączony | |
| `glowny.status.symulator.podtytul` | {klawisz_wolniej} wolniej, {klawisz_szybciej} szybciej | {klawisz_szybciej} przyspiesza, {klawisz_wolniej} zwalnia. Sesje nie trafiają na konto online. | |
| `glowny.status.symulator.akcja` | Wyłącz symulator | Wyłącz symulator | |

## 4. Bluetooth: powody, lista urządzeń, start i stop taśmy

### 4.1. Powody problemów z połączeniem (podtytuł karty w stanie „Łączę ponownie…”)

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `ble.powod.bt_wylaczony` | Bluetooth w Windows jest wyłączony albo niedostępny - włącz go | Bluetooth jest wyłączony albo niedostępny. Włącz go w Start > Ustawienia > Bluetooth i urządzenia. | |
| `ble.powod.nie_widze` | Nie widzę bieżni - czy jest włączona i odłączona od telefonu? | Nie widać bieżni. Włącz ją i zamknij jej aplikację w telefonie. | Jeden tekst zamiast dwóch prawie takich samych (worker.py:121 i :375). |
| `ble.powod.timeout` | Bieżnia nie odpowiedziała na czas | Bieżnia nie odpowiada. Jeśli to się powtarza, wyłącz ją i włącz ponownie. | |
| `ble.powod.rozlaczyla` | Bieżnia się rozłączyła - łączę ponownie… | Bieżnia się rozłączyła. Łączę ponownie… | |
| `ble.powod.inny` | Błąd Bluetooth: {szczegoly} | Problem z Bluetooth. Łączę ponownie… ({szczegoly}) | Techniczny opis w nawiasie na końcu. |
| `ble.powod.nieobslugiwane` | To urządzenie nie wygląda na bieżnię (brak usługi KingSmith ani FTMS). Zapisz raport diagnostyczny i wyślij go. | To urządzenie nie wygląda na bieżnię. Jeśli to bieżnia, zgłoś ją w Ustawienia > Pomoc i diagnostyka > Zgłoś problem. | Bez nazw usług. |
| `ble.rozlaczono` | Rozłączono | Rozłączono | |
| `ble.wybierz_z_listy` | Wybierz bieżnię z listy | Wybierz bieżnię z listy. | Po skanowaniu bez zapamiętanej bieżni. |
| `ble.stop_nie_zdazyl` | Stop taśmy nie zdążył się wysłać | Nie udało się zatrzymać taśmy. Zatrzymaj ją pilotem. | Dziś tylko w logu; przy zamykaniu aplikacji gracz musi to zobaczyć (powiadomienie systemowe). |
| `ble.stop_niepotwierdzony` | Bieżnia nie potwierdziła stopu - zatrzymaj ją pilotem albo kluczem | Bieżnia nie potwierdziła zatrzymania. Zatrzymaj taśmę pilotem albo wyjmij klucz bezpieczeństwa. | Pokazać jako komunikat, nie tylko w logu. |
| `ble.predkosc_nieprzyjeta` | Bieżnia nie przyjęła {kmh} km/h po {liczba} próbach | Bieżnia nie przyjmuje prędkości {kmh} km/h. Sprawdź, czy taśma jedzie, i spróbuj jeszcze raz. | Dziś tylko w logu. Pokazać jako komunikat. |

### 4.2. Ekran „Wybierz bieżnię” (lista urządzeń)

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `bieznia.lista.tytul` | Wybierz bieżnię | Wybierz bieżnię | |
| `bieznia.lista.szukam` | Szukam urządzeń Bluetooth (ok. 6 s)… | Szukam bieżni w pobliżu (ok. 6 s)… | |
| `bieznia.lista.znalezione` | Znalezione bieżnie: {liczba}, inne urządzenia: {razem} | Bieżnie: {liczba}. Inne urządzenia: {razem}. | |
| `bieznia.lista.szukaj_ponownie` | Szukaj ponownie | Szukaj ponownie | |
| `bieznia.lista.bieznia_podpis` | {model} · sygnał {rssi} dBm | {model} · sygnał {sygnal} | Gdy model nieznany: „Bieżnia · sygnał {sygnal}”. |
| `bieznia.lista.bieznia_ogolna` | Bieżnia | Bieżnia | |
| `bieznia.lista.inne_naglowek` | Inne urządzenia w pobliżu | Inne urządzenia w pobliżu | |
| `bieznia.lista.inne_podpis` | sygnał {rssi} dBm | sygnał {sygnal} | |
| `bieznia.lista.bez_nazwy` | (bez nazwy) | Urządzenie bez nazwy | |
| `bieznia.lista.automatycznie` | (tylko log: Znaleziono jedną bieżnię - łączę automatycznie) | Znaleziono jedną bieżnię. Łączę… | Krótki komunikat, żeby gracz wiedział, czemu lista zniknęła. |
| `bieznia.lista.bt_wylaczony.tytul` | Bluetooth jest wyłączony | Bluetooth jest wyłączony | |
| `bieznia.lista.bt_wylaczony.krok1` | 1. Otwórz Start > Ustawienia > Bluetooth i urządzenia. | 1. Otwórz Start > Ustawienia > Bluetooth i urządzenia. | |
| `bieznia.lista.bt_wylaczony.krok2` | 2. Włącz Bluetooth. | 2. Włącz Bluetooth. | |
| `bieznia.lista.bt_wylaczony.krok3` | 3. Wróć tutaj i kliknij Szukaj ponownie. | 3. Wróć tutaj i kliknij Szukaj ponownie. | |
| `bieznia.lista.brak.tytul` | Nie widzę bieżni | Nie widać żadnej bieżni | |
| `bieznia.lista.brak.krok1` | 1. Włącz bieżnię (pilotem albo przyciskiem). | 1. Włącz bieżnię pilotem albo przyciskiem. | |
| `bieznia.lista.brak.krok2` | 2. Rozłącz telefon: zamknij aplikację bieżni (KS Fit, Urevo) albo wyłącz w telefonie Bluetooth. Bieżnia przyjmuje jedno połączenie naraz. | 2. Zamknij aplikację bieżni w telefonie (np. KS Fit, Urevo) albo wyłącz w nim Bluetooth. Bieżnia łączy się tylko z jednym urządzeniem naraz. | |
| `bieznia.lista.brak.krok3` | 3. Sprawdź, czy Bluetooth w Windows jest włączony. | 3. Sprawdź, czy Bluetooth jest włączony: Start > Ustawienia > Bluetooth i urządzenia. | |
| `bieznia.lista.brak.krok4` | 4. Kliknij Szukaj ponownie. | 4. Kliknij Szukaj ponownie. | |
| `bieznia.lista.blad_skanu` | {powód z ble.powod.*} | {jeden z ble.powod.*} | Pod tytułem listy, gdy skan się nie udał. |

### 4.3. Start i stop taśmy z aplikacji (podpowiedź na karcie sterowania)

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `tasma.uruchamiam` | Uruchamiam bieżnię… | Uruchamiam taśmę… | |
| `tasma.uruchamiam_odliczanie` | Uruchamiam bieżnię (odliczanie ok. {sekundy} s)… | Uruchamiam taśmę. Bieżnia odlicza ok. {sekundy} s… | |
| `tasma.rusza` | Bieżnia rusza. | Taśma rusza. | |
| `tasma.zatrzymana` | Bieżnia zatrzymana. | Taśma zatrzymana. | |
| `tasma.odmowa` | Bieżnia nie przyjęła startu. Obudź ją pilotem albo sprawdź, czy klucz bezpieczeństwa jest na miejscu, i spróbuj jeszcze raz. | Bieżnia nie przyjęła startu. Sprawdź klucz bezpieczeństwa, obudź bieżnię pilotem i włącz sterowanie jeszcze raz. | Kolejność czynności jak w praktyce. |
| `tasma.bez_odpowiedzi` | Wysłano start, ale bieżnia nie odpowiedziała. Jeśli taśma nie rusza, uruchom ją pilotem. | Bieżnia nie odpowiedziała na start. Jeśli taśma stoi, uruchom ją pilotem. | |
| `tasma.nieobslugiwane` | Ta bieżnia nie obsługuje startu z aplikacji - użyj pilota. | Ta bieżnia nie pozwala na start z komputera. Uruchom taśmę pilotem. | |
| `tasma.brak_polaczenia` | Brak połączenia z bieżnią - start i stop tylko pilotem. | Bieżnia nie jest połączona. Taśmę uruchomisz i zatrzymasz tylko pilotem. | |

## 5. Ekran główny: karta poziomu

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `glowny.poziom.dostepnosc` | Poziom i osiągnięcia | Poziom i osiągnięcia. Kliknij, żeby zobaczyć szczegóły. | Nazwa dla czytnika ekranu. |
| `glowny.poziom.naglowek` | POZIOM {poziom} | POZIOM {poziom} | Wersaliki to styl, w danych zwykła pisownia „Poziom {poziom}” i CSS `text-transform`. |
| `glowny.poziom.xp` | {xp} / {xp_max} XP | {xp} / {xp_max} XP | |
| `glowny.poziom.xp_max` | {xp} XP | {xp} XP | Najwyższy poziom. |
| `glowny.poziom.do_nastepnego` | Jeszcze {km} km do poziomu {poziom} | Jeszcze {km} km do poziomu {poziom} | |
| `glowny.poziom.najwyzszy` | Najwyższy poziom. Szacunek. | Najwyższy poziom. Szacunek. | |
| `glowny.poziom.licznik` | {liczba}/{razem} | {liczba}/{razem} | Liczba osiągnięć przy pucharze. |
| `glowny.poziom.podpowiedz` | {ranga}: {opis_rangi} | {ranga}: {opis_rangi} | |
| `glowny.poziom.symulator` | (nowy) | Poziomy rosną tylko za kilometry na prawdziwej bieżni. | Pokazać na karcie, gdy włączony jest symulator. Dziś brak informacji, dlaczego XP nie rośnie. |

## 6. Ekran główny: wskaźnik prędkości

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `glowny.wskaznik.dostepnosc` | Prędkość taśmy | Prędkość taśmy | |
| `glowny.wskaznik.jednostka` | km/h taśmy | km/h | Pod liczbą. „taśmy” jest w nazwie karty. |
| `glowny.wskaznik.opis_dostepnosc` | {kmh} km/h, {podpis} | {kmh} km/h, {podpis} | |
| `glowny.wskaznik.wylaczone` | Wyłączone - {klawisz_wlaczania} włącza | Sterowanie wyłączone. {klawisz_wlaczania} włącza. | |
| `glowny.wskaznik.brak_danych` | Brak danych z bieżni | Brak danych z bieżni | Tryb Chodzenie, bieżnia niepołączona. |
| `glowny.wskaznik.czekam` | Czekam na dane | Czekam na dane z bieżni… | |
| `glowny.wskaznik.chodzenie.stoisz` | Stoisz | Stoisz | |
| `glowny.wskaznik.chodzenie.chod` | Chód · gra dostaje {klawisz_chodu} | Chód · gra dostaje {klawisz_chodu} | |
| `glowny.wskaznik.chodzenie.bieg` | Bieg · gra dostaje {klawisz_biegu} + {klawisz_chodu} | Bieg · gra dostaje {klawisz_biegu} + {klawisz_chodu} | |
| `glowny.wskaznik.chodzenie.proba` | {podpis} (podgląd) | Chód · tryb próbny, gra nic nie dostaje | Zastępuje dopisek „(podgląd)”. Dla biegu: „Bieg · tryb próbny, gra nic nie dostaje”. |
| `glowny.wskaznik.klawisze.bez` | Bez klawisza · taśma {kmh} km/h | Bez klawisza · taśma {kmh} km/h | |
| `glowny.wskaznik.klawisze.chod` | Chód · {klawisz_chodu} · taśma {kmh} km/h | Chód · {klawisz_chodu} · taśma {kmh} km/h | |
| `glowny.wskaznik.klawisze.bieg` | Bieg · {klawisz_chodu} + {klawisz_biegu} · taśma {kmh} km/h | Bieg · {klawisz_chodu} + {klawisz_biegu} · taśma {kmh} km/h | |
| `glowny.wskaznik.pad.brak` | Brak pada - {tasma} | Nie widać pada · {tasma} | `{tasma}` = `glowny.wskaznik.tasma_stoi` albo `glowny.wskaznik.tasma_kmh`. |
| `glowny.wskaznik.pad.stan` | Gałka {procent}% - skok {stopien} - {tasma} | Gałka {procent}% · stopień {stopien} · {tasma} | |
| `glowny.wskaznik.tasma_stoi` | taśma stoi | taśma stoi | |
| `glowny.wskaznik.tasma_kmh` | taśma {kmh} km/h | taśma {kmh} km/h | |
| `glowny.wskaznik.trening` | Trening · poziom {poziom} · taśma {kmh} km/h | Trening · poziom {poziom} · taśma {kmh} km/h | |

### 6.1. Żetony stref pod wskaźnikiem

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `glowny.strefy.stoisz` | Stoisz / poniżej {kmh} | Stoisz / poniżej {kmh} km/h | |
| `glowny.strefy.chod` | Chód · {klawisz_chodu} / od {kmh} km/h | Chód · {klawisz_chodu} / od {kmh} km/h | |
| `glowny.strefy.bieg` | Bieg · {klawisz_biegu}+{klawisz_chodu} / od {kmh} km/h | Bieg · {klawisz_biegu} + {klawisz_chodu} / od {kmh} km/h | Spacje wokół „+” jak wszędzie indziej. |
| `glowny.strefy.bez_klawisza` | Bez klawisza / {kmh} km/h | Bez klawisza / {kmh} km/h | Tryb Klawisze. |
| `glowny.strefy.klawisz` | {klawisz_chodu} / {kmh} km/h | {klawisz_chodu} / {kmh} km/h | |
| `glowny.strefy.klawisz_bieg` | {klawisz_biegu} + {klawisz_chodu} / {kmh} km/h | {klawisz_chodu} + {klawisz_biegu} / {kmh} km/h | Ta sama kolejność co we wskaźniku trybu Klawisze. |
| `glowny.strefy.poziom` | Poziom {poziom} / {kmh} km/h | Poziom {poziom} / {kmh} km/h | Trening. |
| `glowny.strefy.puszczona` | Puszczona / {kmh} km/h | Gałka puszczona / {kmh} km/h | Pad, przy 5 stopniach skrót „0%”. |
| `glowny.strefy.stopien` | {procent}% / {kmh} km/h | od {procent}% / {kmh} km/h | |
| `glowny.strefy.stop` | stop | stop | Wartość, gdy puszczona gałka zatrzymuje taśmę. |

### 6.2. Pasek treningu

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `glowny.trening.poziom` | {poziom}\n{kmh} km/h | {poziom}\n{kmh} km/h | Przycisk poziomu. |
| `glowny.trening.poziom_dostepnosc` | Poziom {poziom}: {kmh} km/h | Poziom {poziom}: {kmh} km/h. Kliknij, żeby ustawić. | |
| `glowny.trening.stop` | Stop | Stop | |
| `glowny.trening.stop_dostepnosc` | Stop - zatrzymaj taśmę | Zatrzymaj taśmę | |

## 7. Ekran główny: liczniki

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `glowny.licznik.kroki` | Kroki | Kroki | |
| `glowny.licznik.kadencja` | Kroki/min | Kroki na minutę | „Kroki/min” w kafelku, pełna nazwa dla czytnika ekranu. |
| `glowny.licznik.dystans` | Dystans | Dystans | |
| `glowny.licznik.czas` | Czas ruchu | Czas ruchu | |
| `glowny.licznik.brak` | - | – | Brak wartości. |
| `glowny.licznik.km` | {km} km | {km} km | |

## 8. Ekran główny: karta sterowania

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `glowny.sterowanie.profil` | {profil} | {profil} | Nazwa profilu gry, z rozwijaną listą, gdy profili jest więcej. |
| `glowny.sterowanie.profil_dostepnosc` | Profil: {profil}. Kliknij, żeby zmienić | Profil gry: {profil}. Kliknij, żeby zmienić. | |
| `glowny.sterowanie.bez_profilu` | Sterowanie | Sterowanie | Gdy nie ma jeszcze osoby. |
| `glowny.sterowanie.zarzadzaj` | Zarządzaj profilami | Zarządzaj profilami gier | Otwiera Ustawienia > Gry i sterowanie. |
| `glowny.sterowanie.przelacznik` | Sterowanie w grze | Sterowanie | Przełącznik. W trybie Trening gra nic nie dostaje, więc „w grze” mylił. |
| `glowny.sterowanie.klawisz_dostepnosc` | Klawisz włączania sterowania - kliknij, żeby zmienić | Klawisz włączania: {klawisz_wlaczania}. Kliknij, żeby zmienić. | |
| `glowny.sterowanie.klawisz_podpowiedz` | Kliknij, żeby zmienić klawisz | Kliknij, żeby zmienić klawisz | |
| `glowny.sterowanie.opis.tasma` | uruchamia i zatrzymuje taśmę | uruchamia i zatrzymuje taśmę | Podpis przy klawiszu, gdy start i stop taśmy jest włączony. |
| `glowny.sterowanie.opis.bez_tasmy` | włącza i wyłącza sterowanie | włącza i wyłącza sterowanie | |
| `glowny.sterowanie.opis.z_padem` | (albo pad: {przycisk_pada}) {opis} | {opis} · na padzie: {przycisk_pada} | Nawias na początku zdania był nieczytelny. |
| `glowny.sterowanie.opis.trening` | {opis} · {klawisz_szybciej} szybciej, {klawisz_wolniej} wolniej | {opis} · {klawisz_szybciej} szybciej, {klawisz_wolniej} wolniej | |
| `glowny.sterowanie.opis.klawisze` | {opis} · {klawisz_chodu} przyspiesza | {opis} · {klawisz_chodu} przyspiesza | |
| `glowny.sterowanie.opis.pad` | {opis} · gałka ustawia tempo | {opis} · gałka ustawia tempo | |
| `glowny.sterowanie.opis.chodzenie` | {opis} · działa też w grze | {opis} · działa też w grze | |

### 8.1. Kafelki trybów

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `tryb.chodzenie.nazwa` | Chodzenie | Chodzenie | |
| `tryb.chodzenie.opis` | ruch = W w grze | idziesz, postać idzie | Zapis z „=” był zagadką. Klawisz może nie być W. |
| `tryb.klawisze.nazwa` | Klawisze | Klawisze | |
| `tryb.klawisze.opis` | W rusza taśmę | {klawisz_chodu} rusza taśmę | Z prawdziwym klawiszem. |
| `tryb.pad.nazwa` | Pad | Pad | |
| `tryb.pad.opis` | gałka = tempo | gałka ustawia tempo | |
| `tryb.trening.nazwa` | Trening | Trening | |
| `tryb.trening.opis` | 4 poziomy tempa | 4 stałe tempa, bez gry | |
| `tryb.dostepnosc` | Tryb: {tryb} - {opis} | Tryb {tryb}: {opis} | |
| `tryb.wybrany` | {tryb} (wybrany). {opis} | {tryb}, wybrany. {opis} | |

### 8.2. Podpowiedź pod kafelkami trybów

To tu w 0.5.1 był tekst „Podgląd: nic nie jest naciskane - Klawiatura w Ustawieniach.”.
W 0.5.2 tryb Trening pokazywał podpowiedź trybu Chodzenie (błąd w `render_output_hint`: brak gałęzi
dla Treningu), dlatego dochodzi osobny tekst.

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `glowny.podpowiedz.chodzenie` | Gdy idziesz, aplikacja trzyma {klawisz_chodu} w aktywnym oknie gry, a przy szybszym chodzie {klawisz_biegu} + {klawisz_chodu}. | Idź po bieżni, a postać pójdzie razem z Tobą: gra dostaje {klawisz_chodu}, a przy szybszym tempie {klawisz_biegu} + {klawisz_chodu}. Okno gry musi być na wierzchu. | Ostatnie zdanie to częsta przyczyna „postać nie idzie”. |
| `glowny.podpowiedz.chodzenie_proba` | Tryb próbny: aplikacja nie naciska klawiszy w grze. Żeby postać szła, w Ustawieniach > Gry i sterowanie ustaw Wyjście na Klawiatura. | Tryb próbny: gra nie dostaje klawiszy, więc postać stoi. Żeby szła, włącz Ustawienia > Gry i sterowanie > Sterowanie > Klawisze w grze. | Nazwa opcji zgodna z `ustawienia.chodzenie.klawisze_w_grze`. |
| `glowny.podpowiedz.klawisze` | Trzymaj {klawisz_chodu}, żeby przyspieszyć, {klawisz_chodu} + {klawisz_biegu} - bieg. | Trzymaj {klawisz_chodu}, a taśma przyspieszy. {klawisz_chodu} + {klawisz_biegu} to bieg. Puść klawisz, a taśma zwolni. | |
| `glowny.podpowiedz.pad` | Wychyl lewą gałkę - im dalej, tym szybciej. Skoki zmienisz w Ustawieniach. | Wychyl lewą gałkę: im dalej, tym szybciej jedzie taśma. Stopnie ustawisz w Ustawienia > Gry i sterowanie > Sterowanie. | |
| `glowny.podpowiedz.trening` | (nowy; dziś tekst trybu Chodzenie) | Kliknij poziom albo użyj {klawisz_szybciej} i {klawisz_wolniej}. Gra nie dostaje klawiszy. | |

## 9. Ustawienia: kategorie i ścieżki

Nazwy kategorii i kart są celem ścieżek „Ustawienia > …” we wszystkich tekstach.

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `ustawienia.tytul` | Ustawienia | Ustawienia | |
| `sciezka.ustawienia` | Ustawienia | Ustawienia | |
| `ustawienia.kat.osoby.nazwa` | Konto i osoby | Konto i osoby | |
| `ustawienia.kat.osoby.opis` | Osoby, konto online, kto widzi wyniki | Osoby, konto online, kto widzi wyniki | |
| `ustawienia.kat.sterowanie.nazwa` | Gry i sterowanie | Gry i sterowanie | |
| `ustawienia.kat.sterowanie.opis` | Profile gier, tryb sterowania, klawisz włączania | Profile gier, tryb sterowania, klawisz włączania | |
| `ustawienia.kat.bieznia.nazwa` | Bieżnia | Bieżnia | |
| `ustawienia.kat.bieznia.opis` | Start i stop taśmy, symulator, obsługiwane modele | Twoja bieżnia, start i stop taśmy, symulator | |
| `ustawienia.kat.poziomy.nazwa` | Poziomy i powiadomienia | Poziomy i powiadomienia | |
| `ustawienia.kat.poziomy.opis` | Popupy osiągnięć nad grą, dźwięk | Powiadomienia o osiągnięciach nad grą, dźwięk | |
| `ustawienia.kat.wyglad.nazwa` | Wygląd i okno | Wygląd i okno | |
| `ustawienia.kat.wyglad.opis` | Motyw, kolor, tekst, okno, przycisk na ekranie | Motyw, kolor, wielkość tekstu, okno, przycisk na ekranie | |
| `ustawienia.kat.dane.nazwa` | Dane i kopia | Dane i kopia | |
| `ustawienia.kat.dane.opis` | Eksport, import, folder danych | Kopia danych, przeniesienie na inny komputer | |
| `ustawienia.kat.pomoc.nazwa` | Pomoc i diagnostyka | Pomoc i diagnostyka | |
| `ustawienia.kat.pomoc.opis` | Zgłoś problem, logi, aktualizacje | Zgłoś problem, dziennik, aktualizacje | |
| `ustawienia.wartosc.osoby` | {osoba} · konto {pseudonim} | {osoba} · konto {pseudonim} | Wartość przy zwiniętej kategorii. |
| `ustawienia.wartosc.sterowanie` | {profil} · {tryb} · {klawisz_wlaczania} | {profil} · {tryb} · {klawisz_wlaczania} | |
| `ustawienia.wartosc.bieznia_symulator` | symulator | symulator | |
| `ustawienia.wartosc.bieznia_brak` | nie wybrano | nie wybrano | |
| `ustawienia.wartosc.bieznia` | {nazwa_bt} | {model} | Nazwa modelu zamiast nazwy Bluetooth. |
| `ustawienia.wartosc.poziomy_wl` | popupy włączone | powiadomienia włączone | |
| `ustawienia.wartosc.poziomy_wyl` | bez popupów | powiadomienia wyłączone | |
| `ustawienia.wartosc.motyw` | jak Windows / jasny / ciemny | jak Windows / jasny / ciemny | |
| `ustawienia.wartosc.pomoc` | wersja {wersja} | wersja {wersja} | |

## 10. Ustawienia > Konto i osoby > Osoby

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `ustawienia.osoby.tytul` | Osoby | Osoby | |
| `ustawienia.osoby.opis` | Każda osoba ma swoje profile gier, ustawienia i historię kroków. | Każda osoba ma własne profile gier, ustawienia i historię kroków. | |
| `ustawienia.osoby.wiersz` | {liczba} prof. gier · dziś {kroki} kroków · konto {pseudonim} | {liczba} {liczba:profil gry/profile gier/profili gier} · dziś {kroki} {kroki:krok/kroki/kroków} · konto {pseudonim} | Bez skrótu „prof.”, z odmianą. Część „· konto …” tylko z kontem. |
| `ustawienia.osoby.dodaj` | Dodaj osobę | Dodaj osobę | |
| `ustawienia.osoby.dodaj_z_kontem` | Dodaj osobę z kontem online | Dodaj osobę z kontem online | |
| `ustawienia.osoby.edytuj` | Edytuj: {osoba} | Edytuj: {osoba} | Ikona ołówka w wierszu. |

## 11. Ustawienia > Gry i sterowanie > Profile gier oraz edycja profilu

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `ustawienia.profile.tytul` | Profile gier · {osoba} | Profile gier · {osoba} | |
| `ustawienia.profile.opis` | Ustawienia sterowania i kolor zapisują się w zaznaczonym profilu. | Sterowanie i kolor zapisują się w zaznaczonym profilu gry. | |
| `ustawienia.profile.nowy` | Nowy profil | Nowy profil gry | |
| `ustawienia.profile.podsumowanie.pad` | Pad · {liczba} skoki · do {kmh} km/h | Pad · {liczba} {liczba:stopień/stopnie/stopni} · do {kmh} km/h | Dziś zawsze „skoki”, także przy 5. |
| `ustawienia.profile.podsumowanie.klawisze` | Klawisze · {kmh} / {kmh_bieg} km/h | Klawisze · chód {kmh} km/h, bieg {kmh_bieg} km/h | |
| `ustawienia.profile.podsumowanie.trening` | Trening · {min_kmh}-{max_kmh} km/h | Trening · {min_kmh}–{max_kmh} km/h | |
| `ustawienia.profile.podsumowanie.chodzenie` | Chodzenie · chód od {kmh} km/h | Chodzenie · chód od {kmh} km/h | |
| `profil.tytul_edycja` | Edytuj profil | Edytuj profil gry | |
| `profil.tytul_nowy` | Nowy profil | Nowy profil gry | |
| `profil.gra` | Gra | Gra | |
| `profil.wskaz_folder` | Wskaż folder gry | Wskaż folder gry… | Otwiera okno wyboru. |
| `profil.wskaz_plik_okno` | Wskaż plik gry | Wskaż plik gry | Tytuł okna systemowego. |
| `profil.filtr_plikow` | Gry (*.exe *.lnk);;Wszystkie pliki (*) | Gry (*.exe, *.lnk) / Wszystkie pliki | Filtr okna wyboru (format zależny od API Tauri). |
| `profil.wskaz_opis` | Wskaż folder gry - aplikacja weźmie z pliku gry nazwę, ikonę i kolor. | Wskaż folder gry, a WalkLevel weźmie z niej nazwę, ikonę i kolor. | |
| `profil.wskaz_podpowiedz` | Wybierz plik .exe albo skrót | Wybierz plik gry (.exe) albo jej skrót. | |
| `profil.z_gry` | Ikona i kolor wzięte z pliku gry - możesz je zmienić niżej. | Ikona i kolor są z pliku gry. Możesz je zmienić niżej. | |
| `profil.zmien` | Zmień | Zmień | |
| `profil.brak_pliku` | Nie znalazłem pliku gry w tym miejscu. | Nie ma pliku gry w tym folderze. Wskaż folder z plikiem .exe gry. | Bez rodzaju i z działaniem. |
| `profil.nazwa` | Nazwa profilu | Nazwa profilu | |
| `profil.nazwa_przyklad` | Nazwa gry, np. Cyberpunk 2077 | Np. Cyberpunk 2077 | Placeholder. |
| `profil.online` | Pokazuj tę grę na koncie online | Pokazuj tę grę na koncie online | |
| `profil.online_opis` | Gdy wyłączone, sesje idą na konto bez nazwy gry | Gdy wyłączone, sesje trafiają na konto bez nazwy gry. | |
| `profil.kolor` | Kolor | Kolor | |
| `profil.ikona` | Ikona | Ikona | |
| `profil.duplikuj` | Duplikuj | Duplikuj | |
| `profil.usun` | Usuń profil | Usuń profil | Przycisk z przytrzymaniem. |
| `profil.zacznij_od` | Zacznij od | Zacznij od | |
| `profil.zacznij_kopia` | Kopia obecnego | Kopii obecnego | Druga forma pasuje do „Zacznij od”. |
| `profil.zacznij_domyslne` | Domyślne | Ustawień domyślnych | |
| `profil.zacznij_opis` | Kopia obecnego bierze tryb, klawisze, skoki pada i prędkości z profilu {profil}. | Kopia przejmie tryb, klawisze, stopnie pada i prędkości z profilu {profil}. | |
| `profil.utworz` | Utwórz i włącz profil | Utwórz profil i przełącz na niego | |
| `profil.domyslna_nazwa` | Profil | Profil | Nazwa nowego profilu, gdy pole puste. |
| `profil.domyslny` | Domyślny | Domyślny | Nazwa pierwszego profilu. |
| `profil.kopia_nazwa` | {profil} (kopia) | {profil} (kopia) | |
| `profil.ikona.pad` … `profil.ikona.serce` | Pad, Tarcza, Ogień, Gwiazda, Auto, Samolot, Świat, Flaga, Robot, Klocek, Iskry, Serce | bez zmian | Nazwy ikon dla czytnika ekranu: `Ikona {nazwa}`. |

### 11.1. Wybór koloru

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `kolor.przycisk` | Kolor | Kolor | Otwiera 20 kolorów. |
| `kolor.dostepnosc` | Kolor profilu: {kolor} | Kolor profilu: {kolor} | |
| `kolor.probka` | Kolor {kolor} | Kolor {kolor} | |
| `kolor.z_ikony` | · dobrany z ikony gry | · dobrany z ikony gry | |
| `kolor.recznie` | · wybrany ręcznie (z ikony: {kolor}) | · wybrany ręcznie (z ikony: {kolor}) | |
| `ustawienia.wyglad.kolor` | Kolor · zapisuje się w profilu {profil} | Kolor · zapisuje się w profilu gry {profil} | |

## 12. Ustawienia > Gry i sterowanie > Sterowanie

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `ustawienia.sterowanie.tytul` | Sterowanie · {profil} | Sterowanie · {profil} | |
| `ustawienia.sterowanie.wybor` | Ustawienia trybu | Tryb | Nazwa przełącznika trybów dla czytnika ekranu. |
| `ustawienia.sterowanie.aktywny` | ✓ to aktywny tryb. Wybierz inny, żeby ustawić go zawczasu. | Ten tryb działa teraz. Możesz też wybrać inny i ustawić go na później. | Bez znaku ✓ w treści (ikona w przełączniku wystarczy). |
| `ustawienia.sterowanie.nieaktywny` | Aktywny jest tryb {tryb} - te ustawienia zadziałają po zmianie trybu. | Teraz działa tryb {tryb_aktywny}. Te ustawienia zadziałają po przełączeniu na tryb {tryb}. | |
| `ustawienia.sterowanie.wlacz_tryb` | Włącz {tryb} | Przełącz na {tryb} | |

### 12.1. Tryb Chodzenie

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `ustawienia.chodzenie.klawisz_chodu` | Chód | Klawisz chodu | Rola przycisku przypisywania. |
| `ustawienia.chodzenie.klawisz_biegu` | Bieg | Klawisz biegu | |
| `ustawienia.chodzenie.klawisze_w_grze` | Wyjście: Podgląd / Klawiatura | Klawisze w grze: Włączone / Tryb próbny | Główna zmiana zgłoszona przez Damiana. „Wyjście” i „Podgląd” nic nie mówiły graczowi. |
| `ustawienia.chodzenie.klawisze_w_grze_opis` | (nowy) | Tryb próbny: aplikacja pokazuje, co by nacisnęła, ale gra nic nie dostaje. Do sprawdzenia progów bez ruszania postacią. | Pod przełącznikiem. |
| `ustawienia.chodzenie.chod_od` | Chód od | Chód od | Pole z km/h. |
| `ustawienia.chodzenie.bieg_od` | Bieg od | Bieg od | |
| `ustawienia.chodzenie.histereza` | Bieg wyłącza się {kmh} km/h niżej, żeby Shift nie migał. | Bieg wyłącza się {kmh} km/h poniżej progu, żeby {klawisz_biegu} nie włączał się i nie wyłączał co chwilę. | Z prawdziwym klawiszem zamiast „Shift”. |
| `ustawienia.chodzenie.szybkie_stop` | Szybkie zatrzymanie postaci | Szybkie zatrzymanie postaci | |
| `ustawienia.chodzenie.szybkie_stop_opis` | Gdy taśma gwałtownie hamuje | Postać staje od razu, gdy taśma gwałtownie zwalnia. | |
| `ustawienia.chodzenie.szybkie_stop_dostepnosc` | Szybkie zatrzymanie | Szybkie zatrzymanie postaci | |

### 12.2. Tryb Klawisze

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `ustawienia.klawisze.klawisz_chodu` | Chód | Klawisz chodu | |
| `ustawienia.klawisze.klawisz_biegu` | Bieg | Klawisz biegu | |
| `ustawienia.klawisze.bez` | Bez klawisza / taśma zwalnia do | Bez klawisza / Tempo, gdy nie trzymasz klawisza i po wyłączeniu sterowania. | Pole z km/h przy „Klawisz puszczony: Wolne tempo” (0.6.1). |
| `ustawienia.klawisze.chod` | {klawisz_chodu} / chód | {klawisz_chodu} / tempo chodu | |
| `ustawienia.klawisze.bieg` | {klawisz_biegu} + {klawisz_chodu} / bieg | {klawisz_chodu} + {klawisz_biegu} / tempo biegu | |
| `ustawienia.klawisze.opis` | (nowy) | Trzymany klawisz ustawia prędkość taśmy. Gra też dostaje te klawisze. | Dziś tryb nie ma żadnego opisu w Ustawieniach. Jeśli gra nie dostaje klawiszy w tym trybie, drugie zdanie usunąć (sprawdzić w kodzie 0.6.0). |

### 12.3. Tryb Pad

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `ustawienia.pad.brak` | Nie widzę pada. Podłącz DualSense albo pada Xbox (kablem albo Bluetooth). | Nie widać pada. Podłącz DualSense, DualShock 4 albo pad Xbox kablem lub przez Bluetooth. | |
| `ustawienia.pad.stan` | {pad}: wychylenie {procent}%, skok {stopien}. | {pad}: wychylenie {procent}%, stopień {stopien}. | |
| `ustawienia.pad.drabinka_opis` | Przeciągnij kółka, żeby zmienić, od jakiego wychylenia działa skok. Trójkąt to gałka teraz. | Przeciągnij kółko, żeby ustawić, od jakiego wychylenia gałki zaczyna się stopień. Trójkąt pokazuje, gdzie jest gałka teraz. | |
| `ustawienia.pad.drabinka_dostepnosc` | Progi skoków pada | Progi stopni pada | |
| `ustawienia.pad.liczba` | Liczba skoków | Liczba stopni | |
| `ustawienia.pad.puszczona` | Gałka puszczona | Gałka puszczona | |
| `ustawienia.pad.puszczona.stop` | Taśma staje | Taśma staje | |
| `ustawienia.pad.puszczona.wolno` | Skok 1 | Tempo stopnia 1 | Taśma jedzie dalej najwolniejszym tempem. |
| `ustawienia.pad.puszczona.zero` | 0 km/h (test) | 0 km/h (próba) | |
| `ustawienia.pad.odliczanie` | Ta bieżnia po zatrzymaniu rusza dopiero po ok. {sekundy} s odliczania. „Skok 1” trzyma taśmę w ruchu - gałka działa od razu. | Ta bieżnia po zatrzymaniu rusza dopiero po ok. {sekundy} s. Wybierz „Tempo stopnia 1”, żeby taśma nie stawała i gałka działała od razu. | |
| `ustawienia.pad.stopien` | Skok {stopien} / od {procent}% wychylenia | Stopień {stopien} / od {procent}% wychylenia | |
| `ustawienia.pad.stopien_puszczona` | Gałka puszczona | Gałka puszczona | Pierwszy wiersz listy prędkości. |

### 12.4. Tryb Trening

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `ustawienia.trening.opis` | Zwykłe sterowanie bieżnią z komputera. {klawisz_wlaczania} rusza i zatrzymuje, {klawisz_szybciej} i {klawisz_wolniej} zmieniają poziom, a na ekranie głównym poziom wybierzesz kliknięciem. Gra nie dostaje klawiszy. | Sterowanie bieżnią z komputera, bez gry. {klawisz_wlaczania} uruchamia i zatrzymuje taśmę, {klawisz_szybciej} i {klawisz_wolniej} zmieniają poziom. Poziom wybierzesz też kliknięciem na ekranie głównym. | |
| `ustawienia.trening.poziom` | Poziom {poziom} | Poziom {poziom} | Pole z km/h. |

### 12.5. Pole liczby (wspólne)

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `pole.mniej` | {nazwa}: mniej | {nazwa}: mniej | Dostępność. |
| `pole.wiecej` | {nazwa}: więcej | {nazwa}: więcej | |
| `pole.wpisz` | {nazwa}: wpisz wartość | {nazwa}: wpisz wartość | |
| `pole.wpisz_podpowiedz` | Kliknij i wpisz wartość, np. 2,3 | Kliknij i wpisz wartość, np. 2,3 | |
| `pole.kmh` | {kmh} km/h | {kmh} km/h | |

## 13. Ustawienia > Gry i sterowanie > Klawisz włączania sterowania

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `ustawienia.wlaczanie.tytul` | Klawisz włączania sterowania | Klawisz włączania sterowania | |
| `ustawienia.wlaczanie.rola` | Start/stop | Klawisz włączania | Rola przycisku przypisywania. |
| `ustawienia.wlaczanie.opis` | Kliknij i naciśnij klawisz albo kombinację, np. Ctrl+Shift+F8. Działa też w grze. Bez skrótów Windows (Win, Alt+Tab, Alt+F4) i nakładek (Shift+Tab, Alt+Z). | Kliknij przycisk niżej i naciśnij klawisz albo kombinację, np. Ctrl+Shift+F8. Działa też w trakcie gry. Skróty Windows i nakładek (np. Alt+Tab, Shift+Tab) nie zadziałają. | |
| `ustawienia.wlaczanie.pad_naglowek` | Albo przycisk na padzie (DualSense, DualShock, Xbox) | Albo przycisk na padzie (DualSense, DualShock, Xbox) | |
| `ustawienia.wlaczanie.pad_przypisz` | Przypisz przycisk pada | Przypisz przycisk pada | |
| `ustawienia.wlaczanie.pad_czekam` | Naciśnij przycisk na padzie… | Naciśnij przycisk na padzie… | |
| `ustawienia.wlaczanie.pad_podpowiedz` | Przycisk pada działa jak klawisz włączania - też w grze. Najlepiej taki, którego gra nie używa: panel dotykowy, Create albo kliknięcie gałki. | Przycisk pada działa jak klawisz włączania, także w trakcie gry. Wybierz taki, którego gra nie używa, np. panel dotykowy, Create albo wciśnięcie gałki. | |
| `ustawienia.wlaczanie.pad_usun` | Usuń | Usuń | |
| `ustawienia.wlaczanie.pad_ostrzezenie` | {przycisk_pada} to w wielu grach skok albo akcja - rozważ inny przycisk. | {przycisk_pada} w wielu grach to skok albo akcja. Lepiej wybierz inny przycisk. | Komunikat po przypisaniu Krzyżyka/Kółka/A/B. |
| `ustawienia.tempo.tytul` | (nowy) | Klawisze szybciej i wolniej | Karta pod kartą klawisza włączania. |
| `ustawienia.tempo.opis` | (nowy) | W trybie Trening zmieniają poziom, z symulatorem jego prędkość. Działają też w trakcie gry. Kombinacje są dozwolone, np. Ctrl+F7. | |
| `ustawienia.tempo.szybciej` | (nowy) | Szybciej | Rola przycisku przypisywania („Szybciej: F7”). |
| `ustawienia.tempo.wolniej` | (nowy) | Wolniej | Jw. („Wolniej: F6”). |
| `ustawienia.wlaczanie.zmiana_profilu` | Zmiana profilu na karcie sterowania | Zmiana profilu gry na ekranie głównym | |
| `ustawienia.wlaczanie.zmiana_profilu_opis` | Lista profili po kliknięciu nazwy (przy jednym profilu i tak jej nie ma) | Kliknięcie nazwy profilu otwiera listę profili. Działa, gdy profili jest więcej niż jeden. | |

### 13.1. Przycisk przypisywania klawisza (wspólny)

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `klawisz.przycisk` | {rola}: {klawisz} | {rola}: {klawisz} | |
| `klawisz.czekam` | {rola}: naciśnij klawisz… | {rola}: naciśnij klawisz… | |
| `klawisz.czekam_kombinacja` | {rola}: {wcisniete}+… | {rola}: {wcisniete}+… | Trzymane modyfikatory. |
| `klawisz.podpowiedz` | Kliknij i naciśnij nowy klawisz (Esc anuluje) | Kliknij i naciśnij nowy klawisz. Esc anuluje. | |
| `klawisz.dostepnosc` | {rola}: {klawisz}. Kliknij, żeby zmienić klawisz | {rola}: {klawisz}. Kliknij, żeby zmienić. | |

### 13.2. Klawisze, których nie da się użyć (`klawisze.py`)

Format całego komunikatu: `klawisz.blad.ogolny` z jednym powodem.

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `klawisz.blad.ogolny` | Nie można użyć tego klawisza: {powod}. | Tego klawisza nie da się użyć: {powod}. Wybierz inny. | |
| `klawisz.powod.wolniej` | F6 to skrót symulatora | {klawisz_wolniej} zwalnia symulator i trening | Klawisz wolniej z Ustawień (domyślnie F6). Do 0.6.0 klucz `klawisz.powod.f6` ze stałym F6. |
| `klawisz.powod.szybciej` | F7 to skrót symulatora | {klawisz_szybciej} przyspiesza symulator i trening | Jw. dla klawisza szybciej (domyślnie F7). |
| `klawisz.powod.esc` | Esc anuluje wybór klawisza | Esc anuluje wybór klawisza | |
| `klawisz.powod.f12` | F12 jest zarezerwowany przez Windows | F12 jest zajęty przez Windows | |
| `klawisz.powod.printscreen` | Print Screen robi zrzut ekranu w Windows | Print Screen robi zrzut ekranu | |
| `klawisz.powod.alt_tab` | Alt+Tab przełącza okna | Alt+Tab przełącza okna | |
| `klawisz.powod.alt_f4` | Alt+F4 zamyka okno | Alt+F4 zamyka okno | |
| `klawisz.powod.alt_esc` | Alt+Esc przełącza okna | Alt+Esc przełącza okna | |
| `klawisz.powod.alt_spacja` | Alt+Spacja otwiera menu okna | Alt+Spacja otwiera menu okna | |
| `klawisz.powod.alt_enter` | Alt+Enter przełącza pełny ekran w grach | Alt+Enter przełącza pełny ekran w grach | |
| `klawisz.powod.alt_z` | Alt+Z otwiera nakładkę NVIDIA | Alt+Z otwiera nakładkę NVIDIA | |
| `klawisz.powod.alt_f1` | Alt+F1 to zrzut ekranu NVIDIA | Alt+F1 robi zrzut ekranu w NVIDIA | |
| `klawisz.powod.alt_f9` | Alt+F9 nagrywa w NVIDIA | Alt+F9 nagrywa w NVIDIA | |
| `klawisz.powod.alt_f10` | Alt+F10 zapisuje powtórkę NVIDIA | Alt+F10 zapisuje powtórkę w NVIDIA | |
| `klawisz.powod.shift_tab` | Shift+Tab otwiera nakładkę Steam | Shift+Tab otwiera nakładkę Steam | |
| `klawisz.powod.ctrl_esc` | Ctrl+Esc otwiera menu Start | Ctrl+Esc otwiera menu Start | |
| `klawisz.powod.ctrl_shift_esc` | Ctrl+Shift+Esc otwiera Menedżer zadań | Ctrl+Shift+Esc otwiera Menedżera zadań | Dopełniacz. |
| `klawisz.powod.ctrl_alt_del` | Ctrl+Alt+Delete to skrót systemowy | Ctrl+Alt+Delete jest zajęty przez Windows | |
| `klawisz.powod.ctrl_c` | Ctrl+C kopiuje | Ctrl+C kopiuje | Analogicznie Ctrl+V wkleja, Ctrl+X wycina, Ctrl+Z cofa, Ctrl+Y ponawia, Ctrl+A zaznacza wszystko, Ctrl+S zapisuje. |
| `klawisz.blad.win` | Skróty z klawiszem Windows są zarezerwowane przez system. | Skrótów z klawiszem Windows nie da się użyć. Wybierz inną kombinację. | |
| `klawisz.blad.pojedynczy` | Wybierz klawisz razem z Ctrl, Alt albo Shift, np. Ctrl+Shift+F8. | Ten klawisz zadziała tylko z Ctrl, Alt albo Shift, np. Ctrl+Shift+F8. | |
| `klawisz.blad.zajety` | {powod} - wybierz inną kombinację. | {powod}. Wybierz inną kombinację. | |
| `klawisz.blad.altgr` | {klawisz} to AltGr - pisze polskie znaki. Wybierz inną kombinację. | {klawisz} działa jak AltGr i pisze polskie znaki. Wybierz inną kombinację. | |
| `klawisz.blad.shift_litera` | Shift + litera pisze wielką literę - dodaj Ctrl albo Alt, np. Ctrl+Shift. | Shift z literą pisze wielką literę. Dodaj Ctrl albo Alt, np. Ctrl+Shift+K. | |
| `klawisz.blad.te_same` | Chód i bieg muszą mieć różne klawisze. | Chód i bieg muszą mieć różne klawisze. | |
| `klawisz.blad.wlaczanie_koliduje` | Start/stop musi mieć inny klawisz niż chód i bieg. | Klawisz włączania musi być inny niż klawisze chodu i biegu. | |
| `klawisz.blad.wlaczanie_niedozwolony` | Tego klawisza nie da się użyć jako start/stop - wybierz inny. | Tego klawisza nie da się użyć jako klawisza włączania. Wybierz inny. | |
| `klawisz.blad.tempo_te_same` | (nowy) | Szybciej i wolniej muszą mieć różne klawisze. | |
| `klawisz.blad.tempo_koliduje` | (nowy) | Klawisze szybciej i wolniej muszą być inne niż klawisze włączania, chodu i biegu. | Ten sam klawisz z innym modyfikatorem też się liczy. |
| `klawisz.blad.tempo_niedozwolony` | (nowy) | Tego klawisza nie da się użyć do zmiany prędkości. Wybierz inny. | Klawisz bez kodu wirtualnego. |
| `klawisz.blad.chod_z_modyfikatorem` | Chód i bieg to pojedyncze klawisze - bez Ctrl, Alt i Shift. | Chód i bieg to pojedyncze klawisze, bez kombinacji z Ctrl, Alt albo Shift. | Sam Shift jako klawisz biegu jest dozwolony (domyślny), zabronione są tylko kombinacje. Stare zdanie sugerowało, że Shift w ogóle odpada. |
| `klawisz.kod` | Kod {liczba} | Klawisz nr {liczba} | Klawisz bez nazwy. |

## 14. Ustawienia > Bieżnia

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `ustawienia.bieznia.tytul` | Taśma i połączenie | Twoja bieżnia | |
| `ustawienia.bieznia.opis` | {model} ({nazwa_bt})\n{min_kmh}-{max_kmh} km/h · rozruch ok. {sekundy} s · firmware {wersja_bt} | {model}\nTaśma {min_kmh}–{max_kmh} km/h · start ok. {sekundy} s · oprogramowanie {wersja_bt} | Nazwę Bluetooth pokazać w podpowiedzi po najechaniu, nie w treści. |
| `ustawienia.bieznia.symulator_opis` | Symulator bieżni · {min_kmh}-{max_kmh} km/h | Symulator bieżni · {min_kmh}–{max_kmh} km/h | |
| `ustawienia.bieznia.brak` | Brak zapamiętanej bieżni | Nie wybrano bieżni. Kliknij Szukaj bieżni na ekranie głównym. | |
| `ustawienia.bieznia.start_stop` | Klawisz uruchamia i zatrzymuje taśmę | {klawisz_wlaczania} uruchamia i zatrzymuje taśmę | |
| `ustawienia.bieznia.start_stop_opis` | Wyłącz, jeśli wolisz startować tylko pilotem | Wyłącz, jeśli taśmę uruchamiasz tylko pilotem. | |
| `ustawienia.bieznia.start_stop_dostepnosc` | Start i stop bieżni | Start i stop taśmy | |
| `ustawienia.bieznia.symulator` | Symulator bieżni | Symulator bieżni | |
| `ustawienia.bieznia.symulator_przelacznik_opis` | Bez bieżni: {klawisz_wolniej} wolniej, {klawisz_szybciej} szybciej | Próba bez bieżni: {klawisz_szybciej} przyspiesza, {klawisz_wolniej} zwalnia. Sesje z symulatora nie dają XP i nie trafiają na konto online. | Gracz nie wiedział, czemu nie rośnie poziom. |
| `ustawienia.bieznia.modele` | Obsługiwane bieżnie… | Obsługiwane bieżnie… | |
| `ustawienia.bieznia.modele_podpowiedz` | Lista modeli, pobieranie nowych i dodawanie profilu z pliku | Lista modeli, pobieranie nowych i dodawanie profilu bieżni z pliku | |
| `ustawienia.bieznia.zapomnij` | Zapomnij bieżnię | Zapomnij bieżnię | Przycisk z przytrzymaniem. |

### 14.1. Ekran „Obsługiwane bieżnie”

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `modele.tytul` | Obsługiwane bieżnie | Obsługiwane bieżnie | |
| `modele.opis` | Bieżnię rozpoznajemy po nazwie Bluetooth. Profil modelu to dane: zakres prędkości, odliczanie przy starcie, nazwa - nie program. Nowy model tego samego rodzaju działa od razu po pobraniu profilu. | WalkLevel rozpoznaje bieżnię po nazwie Bluetooth. Profil bieżni opisuje zakres prędkości i czas startu. Nowy model działa od razu po pobraniu jego profilu. | Zdanie „nie program” to wiedza dla programisty. |
| `modele.rodzaj.kingsmith` | KingSmith | KingSmith | |
| `modele.rodzaj.ftms` | Bluetooth FTMS | Standard Bluetooth FTMS | |
| `modele.wbudowany` | w aplikacji | wbudowany | |
| `modele.zakres` | {min_kmh}-{max_kmh} km/h | {min_kmh}–{max_kmh} km/h | |
| `modele.marka_reszta` | pozostałe modele marki | pozostałe modele marki | |
| `modele.rozruch` | rozruch ok. {sekundy} s | start ok. {sekundy} s | |
| `modele.nazwy` | nazwy {lista} | nazwy Bluetooth: {lista} | |
| `modele.twoja` | {model} · Twoja bieżnia | {model} · Twoja bieżnia | |
| `modele.usun` | Usuń | Usuń | |
| `modele.usun_dostepnosc` | Usuń profil {model} | Usuń profil bieżni {model} | |
| `modele.dodaj` | Dodaj z pliku… | Dodaj z pliku… | |
| `modele.dodaj_podpowiedz` | Profil .json od autora albo innego użytkownika | Plik profilu bieżni od autora albo innej osoby | |
| `modele.dodaj_okno` | Dodaj profil bieżni | Dodaj profil bieżni | Tytuł okna systemowego. |
| `modele.dodaj_filtr` | Profil bieżni (*.json) | Profil bieżni (*.json) | |
| `modele.sprawdz` | Sprawdź nowe modele | Sprawdź nowe modele | |
| `modele.sprawdzam` | Sprawdzam listę modeli na GitHubie… | Sprawdzam listę modeli… | |
| `modele.blad` | Nie udało się pobrać listy modeli. | Nie udało się pobrać listy modeli. {powod} | `{powod}` z `modele.powod.*`. |
| `modele.nowe` | Nowe modele: {lista} | Nowe modele: {lista}. | |
| `modele.zaktualizowane` | Zaktualizowane: {lista} | Zaktualizowane: {lista}. | |
| `modele.odrzucone` | Odrzucone (nie przeszły sprawdzenia): {liczba}. | Pominięte, bo plik był uszkodzony: {liczba}. | |
| `modele.aktualne` | Masz wszystkie znane modele. | Masz wszystkie znane modele. | |
| `modele.dodano` | Dodano: {model}. | Dodano profil bieżni {model}. | |
| `modele.usunieto` | Profil usunięty. | Usunięto profil bieżni. | |
| `modele.powod.adres` | Nieoczekiwany adres - przerwano dla bezpieczeństwa. | Pobieranie przerwane dla bezpieczeństwa (nieznany adres). | |
| `modele.powod.przekierowanie` | Serwer przekierował poza GitHub - przerwano. | Pobieranie przerwane dla bezpieczeństwa (nieznany adres). | Jeden tekst dla obu przypadków. |
| `modele.powod.brak_listy` | Lista modeli nie jest jeszcze opublikowana - spróbuj po aktualizacji aplikacji. | Lista modeli jest jeszcze niedostępna. Spróbuj po aktualizacji WalkLevel. | |
| `modele.powod.kod` | Serwer odpowiedział błędem {kod}. | Serwer nie odpowiada poprawnie. Spróbuj później. (kod {kod}) | |
| `modele.powod.internet` | Brak internetu albo GitHub nie odpowiada. | Brak połączenia z internetem. Sprawdź połączenie i spróbuj ponownie. | |
| `modele.powod.za_duzy` | Plik jest za duży jak na profil bieżni - odrzucono. | To nie jest profil bieżni: plik jest za duży. | |
| `modele.powod.format` | Nieznany format listy modeli - zaktualizuj aplikację. | Ta lista modeli wymaga nowszej wersji. Zaktualizuj WalkLevel. | |
| `modele.powod.zly_wpis` | Lista modeli zawiera niepoprawny wpis - przerwano. | Lista modeli jest uszkodzona. Spróbuj później. | |
| `modele.plik.nieczytelny` | Nie da się odczytać pliku. | Nie da się odczytać tego pliku. | Dodawanie z pliku. |
| `modele.plik.za_duzy` | Plik jest za duży jak na profil bieżni. | To nie jest profil bieżni: plik jest za duży. | |
| `modele.plik.zly` | To nie jest poprawny profil bieżni: {szczegoly} | To nie jest poprawny profil bieżni. ({szczegoly}) | Szczegóły z walidacji (`sterowniki/profile.py`) zostają techniczne, w nawiasie. |

## 15. Ustawienia > Poziomy i powiadomienia

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `ustawienia.poziomy.tytul` | Popupy osiągnięć i awansów | Powiadomienia o osiągnięciach i awansach | |
| `ustawienia.poziomy.wlacz` | Popupy na ekranie | Powiadomienia nad grą | |
| `ustawienia.poziomy.wlacz_opis` | Osiągnięcia i awanse w prawym dolnym rogu, także nad grą. Nie zabierają fokusu, kliknięcia przechodzą do gry | Pojawiają się w prawym dolnym rogu, także nad grą. Nie przerywają gry: klawisze i kliknięcia dalej trafiają do gry. | Bez słowa „fokus”. |
| `ustawienia.poziomy.dzwiek` | Dźwięk przy popupie | Dźwięk powiadomienia | |
| `ustawienia.poziomy.dzwiek_opis` | Krótki dźwięk przy zdobyciu | Krótki dźwięk przy nowym osiągnięciu i awansie. | |
| `ustawienia.poziomy.przyklad` | Pokaż przykładowy popup | Pokaż przykładowe powiadomienie | |

## 16. Ustawienia > Wygląd i okno

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `ustawienia.wyglad.tytul` | Motyw i okno | Motyw i okno | |
| `ustawienia.wyglad.motyw` | Motyw | Motyw | |
| `ustawienia.wyglad.motyw.system` | Jak Windows | Jak Windows | |
| `ustawienia.wyglad.motyw.jasny` | Jasny | Jasny | |
| `ustawienia.wyglad.motyw.ciemny` | Ciemny | Ciemny | |
| `ustawienia.wyglad.duzy_tekst` | Duży tekst | Duży tekst | |
| `ustawienia.wyglad.duzy_tekst_opis` | Wszystkie napisy większe o 15% | Wszystkie napisy są o 15% większe. | |
| `ustawienia.wyglad.na_wierzchu` | Zawsze na wierzchu | Zawsze na wierzchu | |
| `ustawienia.wyglad.na_wierzchu_opis` | Okno nie chowa się pod grą | Okno WalkLevel nie chowa się pod grą. | |
| `ustawienia.wyglad.na_wierzchu_blad` | (tylko log) | Nie udało się ustawić okna na wierzchu. | Komunikat, gdy Windows odmówi. |
| `ustawienia.wyglad.przycisk` | Przycisk na ekranie | Przycisk na ekranie | |
| `ustawienia.wyglad.przycisk_opis` | Klik włącza i wyłącza sterowanie, prawy przycisk otwiera okno | Mały przycisk nad grą. Kliknięcie włącza i wyłącza sterowanie, prawy przycisk myszy otwiera okno WalkLevel. | |
| `ustawienia.wyglad.zmiana_osoby` | Zmiana osoby na pasku | Zmiana osoby na pasku | |
| `ustawienia.wyglad.zmiana_osoby_opis` | Awatar z listą osób (przy jednej osobie i tak go nie ma) | Awatar na górnym pasku otwiera listę osób. Widać go, gdy osób jest więcej niż jedna. | |

### 16.1. Przycisk na ekranie (dymek nad grą)

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `dymek.tytul_okna` | WalkLevel - sterowanie | WalkLevel · sterowanie | |
| `dymek.dostepnosc` | Sterowanie w grze: {stan} | Sterowanie: {stan} | `{stan}` = włączone / wyłączone. |
| `dymek.podpowiedz` | Sterowanie: {stan}\nKliknij - {wyłącz/włącz}\nPrawy przycisk - okno Bieżni\nPrzeciągnij - przesuń | Sterowanie {stan}\nKliknij, żeby {wyłączyć/włączyć}\nPrawy przycisk otwiera okno WalkLevel\nPrzeciągnij, żeby przesunąć | Usunięta stara nazwa „okno Bieżni”. |
| `dymek.stan.wl` | włączone | włączone | |
| `dymek.stan.wyl` | wyłączone | wyłączone | |

## 17. Ustawienia > Dane i kopia, eksport i import

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `ustawienia.dane.tytul` | Kopia danych | Kopia danych | |
| `ustawienia.dane.opis` | Przenieś osoby, profile gier i wyniki na inny komputer albo zrób kopię. | Zapisz osoby, profile gier i wyniki do pliku, żeby mieć kopię albo przenieść je na inny komputer. | |
| `ustawienia.dane.eksport` | Eksportuj… | Zapisz kopię… | Prostsze słowa. |
| `ustawienia.dane.import` | Importuj… | Wczytaj kopię… | |
| `ustawienia.dane.przywroc` | Przywróć dane sprzed ostatniego importu | Przywróć dane sprzed ostatniego wczytania kopii | Przycisk z przytrzymaniem. |
| `ustawienia.dane.folder` | Folder danych | Otwórz folder danych | |
| `ustawienia.dane.folder_podpowiedz` | Osoby, profile, historia i ustawienia na tym komputerze | Tu WalkLevel trzyma osoby, profile, historię i ustawienia. | |

### 17.1. Zapis kopii (eksport)

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `kopia.eksport.tytul` | Eksport danych | Zapis kopii danych | |
| `kopia.eksport.osoby` | Osoby i profile gier | Osoby i profile gier | |
| `kopia.eksport.profile_wybrane` | {liczba} z {razem} profili gier | {liczba} z {razem} profili gier | |
| `kopia.eksport.co` | Co zapisać | Co zapisać | |
| `kopia.eksport.ustawienia` | Ustawienia i profile gier / tryby, klawisze, skoki pada, prędkości, ikony, kolory | Ustawienia i profile gier / tryby, klawisze, stopnie pada, prędkości, ikony, kolory | |
| `kopia.eksport.wyniki` | Wyniki / historia kroków: sesje, kroki, dystans, czas | Wyniki / historia kroków: sesje, kroki, dystans, czas | |
| `kopia.eksport.logi` | Logi / dziennik aplikacji i Bluetooth - do zgłoszenia problemu | Dziennik / zapis działania aplikacji i Bluetooth, przydaje się przy zgłoszeniu problemu | |
| `kopia.eksport.zakres` | Zakres wyników | Zakres wyników | |
| `kopia.eksport.zakres.wszystko` | Wszystko | Wszystko | |
| `kopia.eksport.zakres.rok` | Rok | Ostatni rok | |
| `kopia.eksport.zakres.30` | 30 dni | Ostatnie 30 dni | |
| `kopia.eksport.podsumowanie` | Plik: {liczba} os., {profile} profili gier, {sesje} sesji | W pliku: {liczba} {liczba:osoba/osoby/osób}, {profile} {profile:profil gry/profile gier/profili gier}, {sesje} {sesje:sesja/sesje/sesji} | Bez skrótu „os.”, z odmianą. |
| `kopia.eksport.zapisz` | Zapisz plik kopii… | Zapisz plik kopii… | |
| `kopia.eksport.opis_pliku` | Plik .bieznia to archiwum ZIP z danymi w JSON - można je otworzyć i sprawdzić. | Plik kopii możesz otworzyć jak zwykłe archiwum ZIP i sprawdzić, co jest w środku. | Patrz uwaga o rozszerzeniu niżej. |
| `kopia.eksport.okno` | Zapisz kopię danych | Zapisz kopię danych | Tytuł okna systemowego. |
| `kopia.filtr` | Kopia Bieżni (*.bieznia) | Kopia WalkLevel (*.walklevel, *.bieznia) | Stara nazwa. Decyzja dla Damiana: nowe pliki z rozszerzeniem `.walklevel`, wczytywanie obu. Jeśli rozszerzenie zostaje, filtr „Kopia WalkLevel (*.bieznia)”. |
| `kopia.eksport.gotowe` | Zapisano: {liczba} os., {profile} profili, {sesje} sesji. | Zapisano kopię: {liczba} {liczba:osoba/osoby/osób}, {profile} {profile:profil gry/profile gier/profili gier}, {sesje} {sesje:sesja/sesje/sesji}. | |
| `kopia.blad.brak_osoby` | Zaznacz co najmniej jedną osobę. | Zaznacz co najmniej jedną osobę. | |
| `kopia.blad.brak_tresci` | Zaznacz, co zapisać: ustawienia, wyniki albo logi. | Zaznacz, co zapisać: ustawienia, wyniki albo dziennik. | |

### 17.2. Wczytanie kopii (import)

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `kopia.import.okno` | Wybierz kopię danych | Wybierz plik kopii | |
| `kopia.import.tytul` | Import danych | Wczytanie kopii danych | |
| `kopia.import.plik` | Plik | Plik | |
| `kopia.import.plik_opis` | z wersji {wersja} · {liczba} os. · {profile} profili · {sesje} sesji | z wersji {wersja} · {liczba} {liczba:osoba/osoby/osób} · {profile} {profile:profil gry/profile gier/profili gier} · {sesje} {sesje:sesja/sesje/sesji} | |
| `kopia.import.co` | Co zaimportować | Co wczytać | |
| `kopia.import.osoba_jest` | już jest na tym komputerze | już jest na tym komputerze | |
| `kopia.import.osoba_nowa` | nowa osoba | nowa osoba | |
| `kopia.import.osoba_opis` | {stan} · {profile} prof. · {sesje} sesji | {stan} · {profile} {profile:profil/profile/profili} · {sesje} {sesje:sesja/sesje/sesji} | |
| `kopia.import.ustawienia` | Ustawienia i profile gier / tryby, klawisze, prędkości, ikony, kolory | Ustawienia i profile gier / tryby, klawisze, prędkości, ikony, kolory | |
| `kopia.import.wyniki` | Wyniki / historia kroków | Wyniki / historia kroków | |
| `kopia.import.konflikt` | {osoba} już jest | Osoba {osoba} już jest na tym komputerze | |
| `kopia.import.konflikt_dostepnosc` | Konflikt osoby | Co zrobić z osobą {osoba} | |
| `kopia.import.polacz` | Połącz | Połącz | |
| `kopia.import.jako_nowa` | Dodaj jako nową | Dodaj jako nową osobę | |
| `kopia.import.polacz_opis` | Połącz: wyniki się dodają (bez dublowania tych samych sesji), profile o tej samej nazwie dostają ustawienia z pliku, nowe profile dochodzą. | Połącz: wyniki z pliku się dodadzą (te same sesje nie powtórzą się), profile o tej samej nazwie dostaną ustawienia z pliku, a nowe profile dojdą. | |
| `kopia.import.zachowaj` | Zachowaj moje ustawienia profili / Z pliku weź tylko nowe profile i wyniki | Zachowaj moje ustawienia profili / Z pliku weź tylko nowe profile i wyniki | |
| `kopia.import.wczytaj` | Importuj | Wczytaj | |
| `kopia.import.kopia_przed` | Przed importem aplikacja robi kopię obecnych danych - można ją przywrócić w Ustawieniach. | Przed wczytaniem WalkLevel zapisze kopię obecnych danych. Przywrócisz ją w Ustawienia > Dane i kopia. | |
| `kopia.import.gotowe` | Zaimportowano: nowe osoby {liczba}, połączone {razem}, profile +{profile}, sesje +{sesje}. | Wczytano kopię. Nowe osoby: {liczba}, połączone: {razem}, nowe profile: {profile}, nowe sesje: {sesje}. | |
| `kopia.import.blad` | Import nie powiódł się: {szczegoly} | Nie udało się wczytać kopii. {powod} | `{powod}` z `kopia.blad.*`. Dane na komputerze zostają bez zmian (sprawdzić w kodzie). |
| `kopia.import.przywrocono` | Przywrócono dane sprzed importu. | Przywrócono dane sprzed wczytania kopii. | |
| `kopia.blad.brak_pliku` | Plik nie istnieje albo jest za duży. | Tego pliku nie ma albo jest za duży na kopię WalkLevel. | |
| `kopia.blad.nieznane_pliki` | To nie jest plik kopii Bieżni (nieznane pliki w środku). | To nie jest plik kopii WalkLevel. | Stara nazwa usunięta. |
| `kopia.blad.nie_kopia` | To nie jest plik kopii Bieżni. | To nie jest plik kopii WalkLevel. | |
| `kopia.blad.nowsza` | Plik pochodzi z nowszej wersji aplikacji - zaktualizuj Bieżnię. | Ta kopia pochodzi z nowszej wersji WalkLevel. Zaktualizuj aplikację i spróbuj ponownie. | |
| `kopia.blad.uszkodzony` | Plik kopii jest uszkodzony. | Plik kopii jest uszkodzony. Spróbuj innej kopii. | |
| `kopia.nazwa_duplikatu` | {nazwa} {liczba} | {nazwa} {liczba} | Gdy nazwa osoby albo profilu się powtarza. |

## 18. Ustawienia > Pomoc i diagnostyka

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `ustawienia.pomoc.tytul` | Diagnostyka | Pomoc | |
| `ustawienia.pomoc.zglos` | Zgłoś problem… | Zgłoś problem… | |
| `ustawienia.pomoc.logi_bt` | Szczegółowe logi Bluetooth | Szczegółowy dziennik Bluetooth | |
| `ustawienia.pomoc.logi_bt_opis` | Rozmowa z bieżnią w logach - pomaga naprawić połączenie. Bez klawiszy i danych z gier. | Zapisuje dokładnie, co aplikacja wymienia z bieżnią. Włącz, gdy połączenie sprawia kłopoty, i wyślij zgłoszenie. Klawisze i dane z gier nie są zapisywane. | |
| `ustawienia.pomoc.nagrywanie` | Nagrywaj przejścia | Nagrywaj odczyty z bieżni | Funkcja dla autora. Rozważyć ukrycie w wersji instalowanej. |
| `ustawienia.pomoc.nagrywanie_opis` | Odczyty trafiają do folderu nagrania | Zapisuje prędkość i kroki do pliku w folderze nagrania. Potrzebne tylko na prośbę autora. | |
| `ustawienia.pomoc.dziennik` | Dziennik | Dziennik zdarzeń | |
| `ustawienia.pomoc.folder_logow` | Folder logów | Otwórz folder dziennika | |
| `ustawienia.pomoc.aktualizacje_tytul` | Aktualizacje i o aplikacji | Aktualizacje i informacje | |
| `ustawienia.pomoc.sprawdz` | Sprawdź aktualizacje | Sprawdź aktualizacje | |

### 18.1. Aktualizacje (`aktualizacje.py` i pasek)

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `aktualizacja.sprawdzam` | Sprawdzam aktualizacje… | Sprawdzam aktualizacje… | |
| `aktualizacja.pobieram` | Pobieram {nowa_wersja}: {procent}% | Pobieram wersję {nowa_wersja}: {procent}% | |
| `aktualizacja.dostepna_krotko` | Dostępna wersja {nowa_wersja} | Dostępna wersja {nowa_wersja} | Pod nazwą na pasku. |
| `aktualizacja.dostepna` | Dostępna nowa wersja {nowa_wersja} - kliknij Aktualizuj. | Jest nowa wersja {nowa_wersja}. Kliknij Aktualizuj na górnym pasku. | Ikona pobierania ma podpis „Aktualizuj”. |
| `aktualizacja.najnowsza_krotko` | Masz najnowszą wersję | Masz najnowszą wersję | |
| `aktualizacja.najnowsza` | Masz najnowszą wersję ({wersja}). | Masz najnowszą wersję ({wersja}). | |
| `aktualizacja.nieudana_krotko` | Aktualizacja nieudana | Aktualizacja się nie udała | Pod nazwą na pasku; pełny powód w Ustawienia > Pomoc i diagnostyka. |
| `aktualizacja.instaluje` | Instaluję {nowa_wersja} - uruchomię się ponownie | Instaluję wersję {nowa_wersja}. WalkLevel za chwilę uruchomi się ponownie. | |
| `aktualizacja.instalator_blad` | Nie udało się uruchomić instalatora | Nie udało się uruchomić instalatora. Pobierz go ze strony autora. | |
| `aktualizacja.powod.adres` | Nieoczekiwany adres pobierania - przerwano dla bezpieczeństwa. | Pobieranie przerwane dla bezpieczeństwa (nieznany adres). | |
| `aktualizacja.powod.numer` | Wydanie na GitHubie nie ma poprawnego numeru wersji. | Nowa wersja jest źle opisana na serwerze. Spróbuj później. | |
| `aktualizacja.powod.suma_brak` | Wydanie nie ma sumy kontrolnej - nie instaluję. | Nowej wersji nie da się sprawdzić, więc nie zostanie zainstalowana. Spróbuj później. | |
| `aktualizacja.powod.brak_instalatora` | Wydanie nie zawiera instalatora. | Nowa wersja nie ma jeszcze instalatora. Spróbuj później. | |
| `aktualizacja.powod.brak_wydan` | Na GitHubie nie ma jeszcze żadnego wydania. | Brak wersji do pobrania. | |
| `aktualizacja.powod.kod` | GitHub odpowiedział błędem {kod} - spróbuj później. | Serwer aktualizacji nie odpowiada poprawnie. Spróbuj później. (kod {kod}) | |
| `aktualizacja.powod.internet` | Nie udało się sprawdzić aktualizacji - sprawdź internet. | Nie udało się sprawdzić aktualizacji. Sprawdź połączenie z internetem. | |
| `aktualizacja.powod.odpowiedz` | Nieoczekiwana odpowiedź GitHuba. | Serwer aktualizacji odpowiedział niezrozumiale. Spróbuj później. | |
| `aktualizacja.powod.za_duzy` | Plik aktualizacji jest większy niż zapowiadany - przerwano. | Pobrany plik się nie zgadza, więc został odrzucony. Spróbuj ponownie. | Oba przypadki tym samym tekstem. |
| `aktualizacja.powod.suma` | Suma kontrolna się nie zgadza - plik odrzucony. | Pobrany plik się nie zgadza, więc został odrzucony. Spróbuj ponownie. | |
| `aktualizacja.powod.przerwane` | Pobieranie przerwane - spróbuj ponownie. | Pobieranie zostało przerwane. Spróbuj ponownie. | |
| `aktualizacja.powod.system` | Aktualizacja działa tylko na Windows. | Aktualizacja działa tylko w Windows. | |
| `aktualizacja.zaktualizowano` | (tylko log: Zaktualizowano z {wersja} do {nowa_wersja}) | Zaktualizowano WalkLevel do wersji {nowa_wersja}. | Komunikat po pierwszym uruchomieniu nowej wersji. |

### 18.2. Dziennik zdarzeń

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `dziennik.tytul` | Dziennik | Dziennik zdarzeń | |
| `dziennik.opis` | Gdy coś nie działa: zapisz raport diagnostyczny i wyślij plik z folderu logs. | Gdy coś nie działa, najprościej użyć Ustawienia > Pomoc i diagnostyka > Zgłoś problem. Raport diagnostyczny pomaga, gdy bieżnia się nie łączy. | Zamiast „wyślij plik z folderu logs” (nie wiadomo, dokąd). |
| `dziennik.dostepnosc` | Dziennik zdarzeń | Dziennik zdarzeń | |
| `dziennik.folder` | Otwórz folder logów | Otwórz folder dziennika | |
| `dziennik.raport` | Zapisz raport diagnostyczny | Zapisz raport diagnostyczny | |
| `dziennik.raport_trwa` | Zbieram raport (ok. 30 s)… | Zbieram raport, to potrwa ok. 30 s… | |
| `dziennik.raport_gotowy` | Raport: Raport gotowy | Raport zapisany. Dołączy się sam do następnego zgłoszenia. | Dziś „Raport: Raport gotowy” (zdublowane słowo). |
| `dziennik.raport_blad` | Raport: Raport diagnostyczny nie powiódł się | Nie udało się zebrać raportu. Sprawdź, czy Bluetooth jest włączony. | |
| `dziennik.raport_brak_biezni` | (treść raportu: Nie znaleziono bieżni - brak połączenia do zbadania.) | Raport nie znalazł bieżni. Włącz ją i zamknij jej aplikację w telefonie. | Ostatni wiersz raportu widoczny jako stan. |

### 18.3. Zgłoś problem (`app.py`, `zgloszenia.py`)

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `zgloszenie.tytul` | Zgłoś problem | Zgłoś problem | |
| `zgloszenie.wstep` | Opisz, co się stało. Razem z opisem trafią logi, żeby autor mógł szybko znaleźć przyczynę. | Opisz, co się stało. Razem z opisem wyślemy dziennik aplikacji, żeby autor szybciej znalazł przyczynę. | „wyślemy” jest bez rodzaju. |
| `zgloszenie.opis_etykieta` | Co się stało? | Co się stało? | |
| `zgloszenie.opis_przyklad` | Np. bieżnia rozłącza się po kilku minutach albo postać nie biegnie w grze. | Np. bieżnia rozłącza się po kilku minutach albo postać nie biegnie w grze. | |
| `zgloszenie.opis_dostepnosc` | Opis problemu | Opis problemu | |
| `zgloszenie.blad.pusty` | Opisz, co się stało - wystarczy jedno zdanie. | Opisz, co się stało. Wystarczy jedno zdanie. | Pod polem. |
| `zgloszenie.blad.krotki` | Opis jest za krótki - dopisz jeszcze {liczba} {znak/znaki/znaków}, np. co poszło nie tak. | Opis jest za krótki. Dopisz jeszcze {liczba} {liczba:znak/znaki/znaków}, np. co poszło nie tak. | |
| `zgloszenie.kontakt_etykieta` | Kontakt (opcjonalnie) | Kontakt (opcjonalnie) | |
| `zgloszenie.kontakt_przyklad` | Np. nick na Discordzie albo e-mail | Np. nick na Discordzie albo e-mail | |
| `zgloszenie.kontakt_dostepnosc` | Kontakt | Kontakt | |
| `zgloszenie.okres_etykieta` | Logi z okresu | Kiedy był problem | |
| `zgloszenie.okres_dostepnosc` | Okres logów w zgłoszeniu | Z jakiego okresu dołączyć dziennik | |
| `zgloszenie.okres.1h` | 1 godzina | W ciągu godziny | |
| `zgloszenie.okres.6h` | 6 godzin | W ciągu 6 godzin | |
| `zgloszenie.okres.24h` | 24 godziny | W ciągu doby | |
| `zgloszenie.okres.7d` | 7 dni | W ciągu tygodnia | |
| `zgloszenie.okres_opis` | Wybierz, kiedy wystąpił problem. Paczka ma najwyżej 2 MB - przy dłuższym okresie i dużych logach trafia do niej najnowsza część. | Paczka ma najwyżej 2 MB. Przy dłuższym okresie trafi do niej najnowsza część dziennika. | |
| `zgloszenie.dolaczone_tytul` | Co zostanie dołączone | Co zostanie dołączone | |
| `zgloszenie.dolaczone1` | - logi aplikacji i Bluetooth z wybranego okresu i ich podsumowanie, | dziennik aplikacji i Bluetooth z wybranego okresu, | Lista punktowana w HTML zamiast myślników w tekście. |
| `zgloszenie.dolaczone2` | - ostatni raport diagnostyczny, jeśli go zapisano, | ostatni raport diagnostyczny, jeśli jest, | |
| `zgloszenie.dolaczone3` | - wersja aplikacji i Windows, ustawienia (progi, wybrana bieżnia). | wersja aplikacji i Windows, ustawienia sterowania i model bieżni. | |
| `zgloszenie.niedolaczone` | Bez haseł, plików z komputera, wciśniętych klawiszy i historii kroków. Ścieżki z nazwą użytkownika Windows i adresy innych urządzeń Bluetooth są usuwane. | Nie dołączamy haseł, plików z komputera, naciśniętych klawiszy ani historii kroków. Nazwa użytkownika Windows i adresy innych urządzeń Bluetooth są usuwane. | |
| `zgloszenie.droga` | Zgłoszenie idzie przez serwer bieznia.damnit-live.pl prosto na skrzynkę autora; serwer nie zachowuje treści ani paczki. | Zgłoszenie trafia przez serwer {serwer} prosto na skrzynkę autora. Serwer niczego nie przechowuje. | Adres serwera jako zmienna (konfiguracja). |
| `zgloszenie.wyslij` | Wyślij zgłoszenie | Wyślij zgłoszenie | |
| `zgloszenie.przygotuj_email` | Przygotuj e-mail z logami | Przygotuj e-mail ze zgłoszeniem | Gdy brak serwera zgłoszeń. |
| `zgloszenie.zapisz` | Zapisz paczkę na dysk | Zapisz paczkę na dysk | |
| `zgloszenie.zapisz_podpowiedz` | Paczka z logami do wysłania samodzielnie, np. e-mailem | Zapisuje paczkę z dziennikiem do pliku, żeby wysłać ją samodzielnie, np. e-mailem. | |
| `zgloszenie.przygotowuje` | Przygotowuję paczkę z logami… | Przygotowuję zgłoszenie… | |
| `zgloszenie.wyslano` | Wysłano zgłoszenie nr {numer} - dziękuję! Autor dostał opis i logi. | Wysłano zgłoszenie nr {numer}. Dziękujemy, autor dostał opis i dziennik. | Bez wykrzyknika. Bez numeru: „Wysłano zgłoszenie. Dziękujemy, autor dostał opis i dziennik.” |
| `zgloszenie.email_gotowy` | Gotowe: otworzył się e-mail do autora i folder z paczką {plik}. Przeciągnij plik do wiadomości i wyślij. | Otworzył się e-mail do autora i folder z paczką {plik}. Przeciągnij plik do wiadomości i wyślij ją. | |
| `zgloszenie.zapisano` | Zapisano paczkę {plik} (folder dane/zgloszenia otworzył się obok). Możesz ją wysłać e-mailem na {email_autora}. | Zapisano paczkę {plik}. Folder z nią otworzył się obok. Wyślij ją e-mailem na {email_autora}. | |
| `zgloszenie.bez_opisu` | (bez opisu) | (bez opisu) | Treść zgłoszenia, gdy zapis paczki bez opisu. |
| `zgloszenie.blad.paczka` | Nie udało się przygotować paczki: {szczegoly} | Nie udało się przygotować zgłoszenia. Spróbuj ponownie. ({szczegoly}) | |
| `zgloszenie.blad.za_duza` | Paczka z logami jest za duża - usuń stare logi i spróbuj ponownie. | Paczka jest za duża. Wybierz krótszy okres w polu Kiedy był problem i spróbuj ponownie. | Gracz nie wie, jak „usunąć stare logi”. |
| `zgloszenie.blad.adres` | Adres zgłoszeń musi zaczynać się od https:// | Adres serwera zgłoszeń w konfiguracji jest błędny. | Błąd konfiguracji, rzadki. |
| `zgloszenie.blad.odrzucone` | Serwer odrzucił zgłoszenie (kod {kod}). | Serwer nie przyjął zgłoszenia. Zapisz paczkę na dysk i wyślij ją e-mailem. (kod {kod}) | |
| `zgloszenie.blad.za_duza_serwer` | Paczka jest za duża dla serwera - zapisz ją na dysk. | Paczka jest za duża, żeby ją wysłać. Zapisz ją na dysk i wyślij e-mailem. | |
| `zgloszenie.blad.limit` | Za dużo zgłoszeń naraz - spróbuj za godzinę albo zapisz paczkę. | Wysłano już dużo zgłoszeń. Spróbuj za godzinę albo zapisz paczkę na dysk. | |
| `zgloszenie.blad.mail` | Serwer nie wysłał teraz maila. Zapisz paczkę na dysk i wyślij ją e-mailem na {email_autora}. | Serwer nie mógł teraz wysłać zgłoszenia. Zapisz paczkę na dysk i wyślij ją e-mailem na {email_autora}. | |
| `zgloszenie.blad.internet` | Nie udało się wysłać - sprawdź internet albo zapisz paczkę. | Nie udało się wysłać zgłoszenia. Sprawdź połączenie z internetem albo zapisz paczkę na dysk. | |
| `zgloszenie.email.temat` | {app} {wersja} - zgłoszenie problemu | WalkLevel {wersja}: zgłoszenie problemu | |
| `zgloszenie.email.tresc` | {opis}\n\nPaczka z logami do dołączenia: {plik}\n(folder otworzył się obok - przeciągnij plik do tej wiadomości) | {opis}\n\nPaczka do dołączenia: {plik}\nFolder z paczką otworzył się obok. Przeciągnij plik do tej wiadomości. | |

## 19. Kreator osoby i edycja osoby

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `kreator.krok` | Krok {liczba} z 3 | Krok {liczba} z 3 | |
| `kreator.wstecz` | Wstecz | Wstecz | |
| `kreator.anuluj` | Anuluj | Anuluj | |
| `kreator.dalej` | Dalej | Dalej | |
| `kreator.gotowe` | Gotowe | Gotowe | |
| `kreator.imie.tytul_migracja` | WalkLevel ma teraz osoby | WalkLevel ma teraz osoby | Po aktualizacji z bardzo starej wersji (0.2). |
| `kreator.imie.opis_migracja` | Każda osoba ma swoje profile gier i kroki. Nazwij siebie - Twoje dotychczasowe ustawienia trafią do tej osoby. | Każda osoba ma własne profile gier i kroki. Wpisz swoją nazwę, a dotychczasowe ustawienia trafią do Twojej osoby. | |
| `kreator.imie.tytul` | Jak się nazywasz? | Jak się nazywasz? | |
| `kreator.imie.opis` | Nazwa osoby na tej bieżni. Wylosowaliśmy jedną - możesz losować dalej albo wpisać swoją. | Ta nazwa będzie widoczna w aplikacji. Jest wylosowana: losuj dalej albo wpisz własną. | Bez „wylosowaliśmy”. |
| `kreator.imie.pole` | Nazwa osoby | Nazwa osoby | |
| `kreator.imie.losuj` | Losuj inną nazwę | Losuj inną nazwę | |
| `kreator.imie.propozycje` | Albo jedna z tych: | Albo jedna z tych: | |
| `kreator.imie.kolor` | Kolor osoby | Kolor osoby | |
| `kreator.imie.domyslna` | Gracz | Gracz | Gdy pole puste. |
| `kreator.gra.tytul` | Pierwszy profil gry | Pierwszy profil gry | |
| `kreator.gra.opis` | Profil to ustawienia sterowania dla jednej gry. Kolejne dodasz kiedy chcesz. | Profil gry to ustawienia sterowania dla jednej gry. Kolejne dodasz w dowolnej chwili. | |
| `kreator.gra.wskaz` | Wskaż folder gry / Nazwa, ikona i kolor wezmą się z pliku gry | Wskaż folder gry / Nazwę, ikonę i kolor WalkLevel weźmie z pliku gry | |
| `kreator.gra.zmien` | Zmień | Zmień | |
| `kreator.gra.nazwa` | Nazwa profilu - wpisz dowolną | Nazwa profilu, dowolna | Placeholder. |
| `kreator.gra.nazwa_dostepnosc` | Nazwa profilu gry | Nazwa profilu gry | |
| `kreator.gra.kolor_z_ikony` | Kolor {kolor} dobrany z ikony. Przycisk Kolor otwiera 20 kolorów. | Kolor {kolor} jest z ikony gry. Inny wybierzesz przyciskiem Kolor. | |
| `kreator.gra.zostaw` | Zostaw moje obecne ustawienia / Profil Domyślny z dotychczasowymi ustawieniami | Zostaw moje obecne ustawienia / Profil Domyślny z dotychczasowymi ustawieniami | Migracja. |
| `kreator.gra.domyslny` | Profil Domyślny / Dla dowolnej gry - nazwę i ikonę zmienisz później | Profil Domyślny / Dla dowolnej gry. Nazwę i ikonę zmienisz później. | |
| `kreator.gra.kopiuj` | Skopiuj moje profile gier | Skopiuj moje profile gier | |
| `kreator.gra.kopiuj_od` | Skopiuj profile gier: {osoba} | Skopiuj profile gier osoby {osoba} | |
| `kreator.gra.kopiuj_opis` | Z ikonami i ustawieniami. Historia kroków nowej osoby zaczyna się od zera. | Z ikonami i ustawieniami. Historia kroków nowej osoby zacznie się od zera. | |
| `kreator.gra.brak_pliku` | Nie znalazłem pliku gry w tym miejscu. | Nie ma pliku gry w tym folderze. Wskaż folder z plikiem .exe gry. | |
| `kreator.tryb.tytul` | Jak sterujesz? | Jak chcesz sterować? | |
| `kreator.tryb.opis` | Dla profilu {profil}. Wszystko zmienisz później w Ustawieniach. | Dla profilu {profil}. Zmienisz to w Ustawienia > Gry i sterowanie. | |
| `kreator.tryb.klawisze` | Klawisze / Trzymasz W - taśma przyspiesza, puszczasz - zwalnia | Klawisze / Trzymasz W, a taśma przyspiesza. Puszczasz, a zwalnia. | |
| `kreator.tryb.pad` | Pad / Wychylenie gałki ustawia prędkość taśmy skokami | Pad / Wychylenie gałki ustawia prędkość taśmy w kilku stopniach. | |
| `kreator.tryb.chodzenie` | Chodzenie / Chodzisz po bieżni - gra dostaje W, szybciej W + Shift | Chodzenie / Chodzisz po bieżni, a postać idzie w grze. Gra dostaje W, przy szybszym tempie W + Shift. | |
| `kreator.tryb.trening` | – | Trening / Cztery stałe tempa bez gry. Poziom wybierasz kliknięciem albo F7 i F6. | Nowy (26.11): w 0.5 tryb Trening nie był do wyboru w kreatorze. |
| `kreator.tryb.wybrane` | {tryb} (wybrane) | {tryb}, wybrany | Dostępność. |
| `osoba.edycja.tytul` | Edytuj osobę | Edytuj osobę | |
| `osoba.edycja.nazwa` | Nazwa | Nazwa | |
| `osoba.edycja.kolor` | Kolor osoby | Kolor osoby | |
| `osoba.edycja.usun_opis` | Usunięcie osoby usuwa jej profile gier. Jej wyniki zostają w historii jako „Usunięte osoby”. | Usunięcie osoby usuwa jej profile gier. Jej wyniki zostaną w historii pod nazwą „Usunięte osoby”. | |
| `osoba.edycja.usun` | Usuń osobę | Usuń osobę | Przycisk z przytrzymaniem. |
| `osoba.usuniete` | Usunięte osoby | Usunięte osoby | Nazwa w historii. |

## 20. Historia kroków

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `historia.tytul` | Historia kroków | Historia kroków | |
| `historia.brak_osoby` | Historia pojawi się po utworzeniu pierwszej osoby. | Historia pojawi się, gdy dodasz pierwszą osobę. | |
| `historia.niedostepna` | (tylko log: Historia kroków niedostępna) | Historia kroków jest chwilowo niedostępna. Uruchom WalkLevel ponownie. Jeśli to nie pomoże, zgłoś problem. | Baza zablokowana albo uszkodzona. Dziś gracz nic nie widzi, a sesje się nie zapisują. |
| `historia.czyja` | Czyja historia | Czyja historia | |
| `historia.czyja.moja` | Moja | Moja | |
| `historia.czyja.wszyscy` | Wszyscy | Wszystkie osoby | |
| `historia.zakres` | Zakres | Zakres | |
| `historia.zakres.dzis` | Dziś | Dziś | |
| `historia.zakres.7` | 7 dni | 7 dni | |
| `historia.zakres.30` | 30 dni | 30 dni | |
| `historia.zakres.wszystko` | Wszystko | Wszystko | |
| `historia.suma.kroki` | Kroki | Kroki | |
| `historia.suma.dystans` | Dystans | Dystans | |
| `historia.suma.czas` | Czas ruchu | Czas ruchu | |
| `historia.suma.srednio` | Średnio | Średnie tempo | |
| `historia.wykres.dzien` | Kroki w ciągu dnia | Kroki w ciągu dnia | |
| `historia.wykres.dni` | Kroki dziennie | Kroki dziennie | |
| `historia.wykres.tys` | {liczba} tys. | {liczba} tys. | Oś wykresu. |
| `historia.wykres.dzis` | dziś | dziś | |
| `historia.pusto` | Brak kroków w tym okresie. Wejdź na bieżnię - wyniki pojawią się tu same. | Brak kroków w tym okresie. Wejdź na bieżnię, a wyniki pojawią się tu same. | |
| `historia.podzial.osoby` | Osoby | Osoby | |
| `historia.podzial.profile` | Profile gier | Profile gier | |
| `historia.podzial.wiersz` | {kroki} kroków / {km} km · {czas} · {procent}% | {kroki} {kroki:krok/kroki/kroków} / {km} km · {czas} · {procent}% | |
| `historia.sesje.tytul` | Ostatnie sesje | Ostatnie sesje | |
| `historia.sesje.dzis` | Dziś | Dziś | |
| `historia.sesje.wczoraj` | Wczoraj | Wczoraj | |
| `historia.sesje.wiersz` | {dzien} {godzina} · {osoba} · {profil} / {kroki} kroków · {czas} · {kmh} km/h | {dzien} {godzina} · {osoba} · {profil} / {kroki} {kroki:krok/kroki/kroków} · {czas} · {kmh} km/h | |
| `historia.sesje.dostepnosc` | Sesja {dzien} {godzina} - szczegóły | Sesja {dzien} {godzina}. Kliknij, żeby zobaczyć szczegóły. | |
| `historia.sesje.pusto` | (nowy) | Jeszcze nie ma sesji. Pierwsza zapisze się sama po kilku krokach na bieżni. | Pusta lista sesji. |
| `historia.czas.godziny` | {godziny} h {minuty} min | {godziny} h {minuty} min | |
| `historia.czas.minuty` | {minuty} min | {minuty} min | |

### 20.1. Szczegóły sesji

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `sesja.tytul_dostepnosc` | Szczegóły sesji | Szczegóły sesji | |
| `sesja.tytul` | Sesja | Sesja | |
| `sesja.brak` | Tej sesji już nie ma. | Tej sesji już nie ma. Mogła zostać usunięta. | |
| `sesja.kiedy` | {data} · {godzina}-{godzina_koniec} | {data} · {godzina}–{godzina_koniec} | |
| `sesja.kto` | {osoba} · {profil} | {osoba} · {profil} | |
| `sesja.kroki` | Kroki | Kroki | |
| `sesja.dystans` | Dystans | Dystans | |
| `sesja.czas` | Czas ruchu | Czas ruchu | |
| `sesja.srednio` | Średnio | Średnie tempo | |
| `sesja.kadencja` | Kroki/min | Kroki/min | |
| `sesja.wykres` | Cała sesja | Cała sesja | |
| `sesja.skad` | Skąd | Źródło | |
| `sesja.skad.symulator` | Symulator bieżni | Symulator bieżni | |
| `sesja.skad.bieznia` | Bieżnia | Bieżnia | Gdy model nieznany. |
| `sesja.skad.oprogramowanie` | firmware {wersja_bt} | oprogramowanie bieżni {wersja_bt} | |
| `sesja.skad.aplikacja` | aplikacja {wersja} | WalkLevel {wersja} | |
| `sesja.online.etykieta` | Konto online: {stan} | Konto online: {stan} | |
| `sesja.online.ukryta` | ukryta (tylko u Ciebie) | ukryta, widać ją tylko tutaj | |
| `sesja.online.wyslana` | na koncie | wysłana | |
| `sesja.online.czeka` | czeka na wysyłkę | czeka na wysłanie | |
| `sesja.online.odrzucona` | (nowy) | serwer jej nie przyjął, WalkLevel spróbuje ponownie za dobę | Od 0.5.2 odrzucone sesje są ponawiane po dobie, ale gracz tego nie widzi. |
| `sesja.online.ukryj` | Ukryj online | Ukryj na koncie online | Podpowiedź ikony oka. |
| `sesja.online.pokaz` | Ukryta online - kliknij, żeby pokazać | Ukryta na koncie online. Kliknij, żeby pokazać. | |
| `sesja.online.symulator` | Sesje z symulatora nie idą na konto online. | Sesje z symulatora nie trafiają na konto online. | |
| `sesja.usun.tytul` | Usuń | Usuń sesję | |
| `sesja.usun.opis` | Usuwa sesję z historii na tym komputerze i z konta online. Tego nie da się cofnąć - przytrzymaj przycisk. | Sesja zniknie z historii na tym komputerze i z konta online. Tego nie da się cofnąć. Przytrzymaj przycisk, żeby usunąć. | Wariant bez konta: bez „i z konta online”. |
| `sesja.usun.przycisk` | Usuń sesję | Usuń sesję | |
| `sesja.usunieta` | Sesja usunięta. | Usunięto sesję. | |

## 21. Poziomy i osiągnięcia

### 21.1. Ekran „Poziomy i osiągnięcia”

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `poziomy.tytul` | Poziomy i osiągnięcia | Poziomy i osiągnięcia | |
| `poziomy.naglowek` | POZIOM {poziom} | POZIOM {poziom} | |
| `poziomy.xp` | {xp} / {xp_max} XP | {xp} / {xp_max} XP | |
| `poziomy.do_nastepnego` | Jeszcze {km} km do poziomu {poziom} | Jeszcze {km} km do poziomu {poziom} | |
| `poziomy.ranga` | {ranga}: {opis_rangi} | {ranga}: {opis_rangi} | |
| `poziomy.ranga_dostepnosc` | Poziom {poziom}, ranga {ranga} | Poziom {poziom}, ranga {ranga} | |
| `poziomy.ranga_wiersz` | Poziom {poziom} · {ranga} | Poziom {poziom} · {ranga} | |
| `poziomy.osiagniecia_tytul` | Osiągnięcia · {liczba} z {razem} | Osiągnięcia · {liczba} z {razem} | |
| `poziomy.ostatnie` | Ostatnio zdobyte | Ostatnio zdobyte | |
| `poziomy.najblizej` | Najbliżej | Najbliżej zdobycia | |
| `poziomy.pozostale` | Pozostałe | Pozostałe | |
| `poziomy.grupa_licznik` | {liczba} z {razem} | {liczba} z {razem} | |
| `poziomy.brak_osiagniec` | Jeszcze bez osiągnięć. | Jeszcze nie ma osiągnięć. Pierwsze wpadnie po 1 km na bieżni. | Pusty stan z podpowiedzią. |
| `poziomy.pokaz_wszystkie` | Pokaż wszystkie ({liczba}) | Pokaż wszystkie ({liczba}) | |
| `poziomy.osiagniecie.xp` | +{xp} XP | +{xp} XP | |
| `poziomy.osiagniecie.zdobyte` | zdobyte {kiedy} | zdobyte {kiedy} | |
| `poziomy.osiagniecie.dni_temu` | {dni} dni temu | {dni} {dni:dzień/dni/dni} temu | Dla 1: „wczoraj”, dla 0: „dziś”. Powyżej 30 dni data „{data}”. |
| `poziomy.osiagniecie.dostepnosc` | {nazwa}: zdobyte/do zdobycia. {warunek} | {nazwa}: {zdobyte/do zdobycia}. {warunek} | |
| `poziomy.osiagniecie.do_zdobycia` | do zdobycia | do zdobycia | |
| `poziomy.postep.km` | {liczba} / {razem} KM | {liczba} / {razem} km | Jednostki małą literą; wersaliki przez CSS, jeśli styl tego wymaga. |
| `poziomy.postep.kroki` | {liczba} / {razem} KROKÓW | {liczba} / {razem} {razem:krok/kroki/kroków} | |
| `poziomy.postep.dni` | {liczba} / {razem} DNI | {liczba} / {razem} {razem:dzień/dni/dni} | |
| `poziomy.postep.gry` | {liczba} / {razem} GRY | {liczba} / {razem} {razem:gra/gry/gier} | Dziś zawsze „GRY”. |
| `poziomy.symulator` | (nowy) | XP i osiągnięcia liczą się tylko z prawdziwej bieżni, bez symulatora. | Na dole ekranu. |

### 21.2. Szlak

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `szlak.tytul` | Twój szlak | Twój szlak | |
| `szlak.tytul_km` | Twój szlak · {km} km | Twój szlak · {km} km | |
| `szlak.km` | {km} km | {km} km | |
| `szlak.lacznie` | przebyte łącznie · {liczba} z {razem} przystanków szlaku | przebyte łącznie · {liczba} z {razem} przystanków | |
| `szlak.tu_jestes` | TU JESTEŚ | TU JESTEŚ | |
| `szlak.nastepny` | następny przystanek za {km} km · ok. {dni} dni przy 5 km dziennie | następny przystanek za {km} km, to ok. {dni} {dni:dzień/dni/dni} po 5 km dziennie | Odmiana. |

### 21.3. Powiadomienia (okienka w rogu ekranu)

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `powiadomienie.tytul_okna` | {tytul} - WalkLevel | {tytul} · WalkLevel | |
| `powiadomienie.dostepnosc` | Popup osiągnięcia | Powiadomienie o osiągnięciu | |
| `powiadomienie.osiagniecie.nad` | OSIĄGNIĘCIE · {rzadkosc} | OSIĄGNIĘCIE · {rzadkosc} | Wersaliki przez CSS. |
| `powiadomienie.osiagniecie.linia` | +{xp} XP  ·  {warunek} | +{xp} XP · {warunek} | Jedna spacja wokół kropki. |
| `powiadomienie.awans.nad` | AWANS · POZIOM {poziom} | AWANS · POZIOM {poziom} | |
| `powiadomienie.awans.tytul` | Poziom {poziom} | Poziom {poziom} | Bez nowej rangi; z nową rangą tytułem jest `{ranga}`. |
| `powiadomienie.awans.linia` | Następny poziom za {km} km | Następny poziom za {km} km | |
| `powiadomienie.awans.ranga_linia` | Nowa ranga · następny poziom za {km} km | Nowa ranga · następny poziom za {km} km | |
| `powiadomienie.wiele.nad` | OSIĄGNIĘCIA | OSIĄGNIĘCIA | |
| `powiadomienie.wiele.tytul` | +{liczba} kolejnych | I {liczba} {liczba:kolejne/kolejne/kolejnych} | „+1 kolejnych” było błędne. |
| `powiadomienie.wiele.linia` | Wszystkie na liście osiągnięć | Wszystkie są w Poziomy i osiągnięcia. | Ścieżka do ekranu. |
| `powiadomienie.historia.nad` | Z TWOJEJ HISTORII | Z TWOJEJ HISTORII | Po pierwszym przeliczeniu historii. |
| `powiadomienie.historia.tytul` | Poziom {poziom} · {ranga} | Poziom {poziom} · {ranga} | |
| `powiadomienie.historia.linia` | {liczba} {osiągnięcie/osiągnięcia/osiągnięć} · {km} km przebyte | {liczba} {liczba:osiągnięcie/osiągnięcia/osiągnięć} · {km} km za Tobą | |
| `powiadomienie.dzwiek_brak` | (tylko log) | bez tekstu | Brak dźwięku nie wymaga komunikatu. |

### 21.4. Rzadkość osiągnięć

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `rzadkosc.pospolite` | Pospolite | Pospolite | |
| `rzadkosc.rzadkie` | Rzadkie | Rzadkie | |
| `rzadkosc.epickie` | Epickie | Epickie | |
| `rzadkosc.heroiczne` | Heroiczne | Heroiczne | |
| `rzadkosc.legendarne` | Legendarne | Legendarne | |

## 22. Konto online

### 22.1. Karta w Ustawienia > Konto i osoby

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `konto.karta.tytul` | Konto online · {osoba} (opcjonalne) | Konto online · {osoba} (opcjonalne) | |
| `konto.karta.opis` | Bez konta wszystko działa na tym komputerze i nic nie jest wysyłane. Z kontem: wyniki online, znajomi, ranking i lista osób. Konto należy do jednej osoby - inne osoby mogą mieć swoje konta albo żadnego. | Bez konta wszystko działa na tym komputerze i nic nie jest wysyłane. Konto daje wyniki online, znajomych i ranking. Każda osoba może mieć własne konto albo żadnego. | |
| `konto.karta.zaloz` | Załóż konto | Załóż konto | |
| `konto.karta.zaloguj` | Zaloguj się | Zaloguj się | |
| `konto.karta.otworz` | Znajomi, ranking i ustawienia konta | Znajomi, ranking i ustawienia konta | |
| `konto.co_wysylamy.tytul` | Co trafia na serwer | Co trafia na serwer | |
| `konto.co_wysylamy` | Na serwer trafia: e-mail, pseudonim i sesje jednej osoby z tego komputera (data, kroki, dystans, czas ruchu, model bieżni). Nie trafia: nazwy osób i gier, ustawienia, kolory, sesje z symulatora. Bez konta nic nie jest wysyłane. | Na serwer trafia: e-mail, pseudonim i sesje osoby, do której należy konto (data, kroki, dystans, czas ruchu, model bieżni, nazwa gry, jeśli jest pokazywana). Nie trafiają: nazwy osób, ustawienia, kolory i sesje z symulatora. | „nazwy gier” nie trafiają było nieaktualne od podziału na gry (0.5.0). Sprawdzić z regulaminem. |

### 22.2. Logowanie, rejestracja, hasło

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `konto.logowanie.tytul` | Logowanie | Logowanie | |
| `konto.rejestracja.tytul` | Nowe konto | Nowe konto | |
| `konto.reset.tytul` | Nie pamiętam hasła | Nie pamiętam hasła | |
| `konto.potwierdz.tytul` | Potwierdź e-mail | Potwierdź e-mail | |
| `konto.tytul_ogolny` | Konto | Konto | |
| `konto.pole.email` | E-mail | E-mail | |
| `konto.pole.haslo` | Hasło | Hasło | |
| `konto.pole.haslo2` | Powtórz hasło | Powtórz hasło | |
| `konto.pole.pseudonim` | Pseudonim (widzą go znajomi) | Pseudonim (widzą go inni) | Widać go też w rankingu i na liście osób. |
| `konto.pole.kod` | Kod zaproszenia | Kod zaproszenia | |
| `konto.kod_opis` | Konta zakładamy na zaproszenie: kod dostaniesz od kogoś, kto już ma konto. | Konto założysz tylko z kodem zaproszenia. Kod da Ci ktoś, kto już ma konto. | |
| `konto.nowa_osoba_opis` | Konto będzie nową osobą w aplikacji (nazwa = pseudonim). | Konto będzie nową osobą w aplikacji, z nazwą taką jak pseudonim. | |
| `konto.dla_osoby` | Konto dla osoby: {osoba}. | Konto dla osoby {osoba}. | |
| `konto.pseudonim_zasady` | Pseudonim: 3-24 znaki - litery, cyfry, spacja, _, - i kropka. Nie wpisuj imienia i nazwiska. | Pseudonim: 3–24 znaki, litery, cyfry, spacja, podkreślnik, łącznik i kropka. Nie wpisuj imienia i nazwiska. | |
| `konto.haslo_zasady` | Hasło: co najmniej {liczba} znaków. | Hasło: co najmniej {liczba} {liczba:znak/znaki/znaków}. | |
| `konto.zapamietaj` | Zapamiętaj logowanie | Zapamiętaj logowanie | |
| `konto.zapamietaj_opis` | Hasła nie zapisujemy - tylko bezpieczny token w Windows | Hasło nie jest zapisywane. Windows przechowuje tylko klucz logowania. | Bez słowa „token”. |
| `konto.regulamin_zgoda` | Akceptuję regulamin i politykę prywatności | Akceptuję regulamin i politykę prywatności | |
| `konto.regulamin_wersja` | Wersja z {data} | Wersja z {data} | |
| `konto.regulamin_czytaj` | Przeczytaj regulamin i politykę prywatności | Przeczytaj regulamin i politykę prywatności | |
| `konto.przycisk.zaloguj` | Zaloguj się | Zaloguj się | |
| `konto.przycisk.zaloz` | Załóż konto | Załóż konto | |
| `konto.trwa.logowanie` | Loguję… | Loguję… | |
| `konto.trwa.zakladanie` | Zakładam konto… | Zakładam konto… | |
| `konto.trwa.wysylanie` | Wysyłam… | Wysyłam… | |
| `konto.link.nie_pamietam` | Nie pamiętam hasła | Nie pamiętam hasła | |
| `konto.link.zaloz` | Załóż konto | Nie masz konta? Załóż je | |
| `konto.link.mam_konto` | Mam już konto - zaloguj się | Masz już konto? Zaloguj się | |
| `konto.link.do_logowania` | Wróć do logowania | Wróć do logowania | |
| `konto.link.przejdz` | Przejdź do logowania | Przejdź do logowania | |
| `konto.rejestracja.gotowe` | Konto założone. Kliknij link w e-mailu, który właśnie wysłaliśmy (sprawdź też spam), a potem zaloguj się. Bez potwierdzenia funkcje online są zablokowane. | Konto założone. Otwórz e-mail od WalkLevel i kliknij link potwierdzający (sprawdź też spam). Do tego czasu funkcje online nie działają. | |
| `konto.reset.opis` | Wyślemy na Twój adres link do ustawienia nowego hasła (ważny 30 minut). Po zmianie hasła zalogujesz się od nowa na każdym komputerze. | Na Twój adres przyjdzie link do ustawienia nowego hasła. Link działa 30 minut. Po zmianie hasła trzeba zalogować się ponownie na każdym komputerze. | |
| `konto.reset.wyslij` | Wyślij link | Wyślij link | |
| `konto.reset.wyslano` | Jeśli konto z tym adresem istnieje, wysłaliśmy wiadomość z linkiem. Sprawdź też spam. | Jeśli konto z tym adresem istnieje, przyjdzie na nie e-mail z linkiem. Sprawdź też spam. | |
| `konto.blad.hasla_rozne` | Hasła różnią się. | Hasła się różnią. Wpisz to samo hasło w obu polach. | |
| `konto.blad.regulamin` | Zaakceptuj regulamin, żeby założyć konto. | Zaznacz akceptację regulaminu, żeby założyć konto. | |
| `konto.zalogowano` | Zalogowano jako {pseudonim}. | Zalogowano jako {pseudonim}. | |
| `konto.wylogowano` | Wylogowano. | Wylogowano. | |
| `konto.usunieto` | Konto usunięte. | Usunięto konto. | |
| `konto.wygaslo` | Logowanie konta osoby {osoba} wygasło (albo hasło zostało zmienione) - zaloguj się ponownie. | Logowanie osoby {osoba} wygasło albo hasło zostało zmienione. Zaloguj się ponownie w Ustawienia > Konto i osoby. | |
| `konto.przypiete` | To konto jest już przypięte do osoby {osoba} - wybierz ją. | To konto należy już do osoby {osoba} na tym komputerze. Przełącz się na nią. | |
| `konto.przypiete_nieznana` | ? | inna osoba | Gdy nazwy nie da się ustalić (dziś „?”). |

### 22.3. Potwierdzenie adresu e-mail

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `konto.email.niepotwierdzony` | Adres niepotwierdzony - bez tego nie odzyskasz hasła, gdy je zapomnisz. | Adres e-mail nie jest potwierdzony. Bez tego nie odzyskasz hasła. | |
| `konto.email.wyslij_link` | Wyślij link potwierdzający | Wyślij link potwierdzający | |
| `konto.email.wyslano_toast` | Wysłaliśmy link potwierdzający na {email} - kliknij go w wiadomości (sprawdź też spam). | Link potwierdzający jest w drodze na {email}. Kliknij go w wiadomości (sprawdź też spam). | |
| `konto.email.wyslano` | Wysłano link potwierdzający - sprawdź skrzynkę. | Wysłano link potwierdzający. Sprawdź skrzynkę. | |
| `konto.email.juz` | Adres e-mail jest już potwierdzony. | Adres e-mail jest już potwierdzony. | |
| `konto.email.potwierdzony` | Adres e-mail potwierdzony - możesz odzyskać hasło, gdy je zapomnisz. | Potwierdzono adres e-mail. | Krótko; reszta wynika z kontekstu. |

### 22.4. Regulamin

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `konto.regulamin.zmiana_tytul` | Zaktualizowaliśmy regulamin konta | Regulamin konta się zmienił | Bez „zaktualizowaliśmy”. |
| `konto.regulamin.zmiana_opis` | Przeczytaj zmiany i zaakceptuj, żeby korzystać z nowych funkcji (np. podziału wyników na gry). Do tego czasu nowe dane nie są wysyłane. | Przeczytaj zmiany i zaakceptuj je, żeby korzystać z nowych funkcji, np. podziału wyników na gry. Do tego czasu nowe sesje czekają na komputerze. | |
| `konto.regulamin.czytaj` | Przeczytaj | Przeczytaj | |
| `konto.regulamin.akceptuj` | Akceptuję | Akceptuję | |
| `konto.regulamin.dziekujemy` | Dziękujemy - regulamin zaakceptowany. | Dziękujemy. Regulamin zaakceptowany. | |

### 22.5. Ekran „Znajomi i ranking”: zakładki

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `online.tytul` | Konto online · {pseudonim} | Konto online · {pseudonim} | |
| `online.tytul_ogolny` | Konto online | Konto online | |
| `online.zakladka.znajomi` | Znajomi | Znajomi | |
| `online.zakladka.ranking` | Ranking | Ranking | |
| `online.zakladka.osoby` | Osoby | Osoby z kontem | „Osoby” myliło się z osobami na komputerze. |
| `online.zakladka.konto` | Konto | Moje konto | |
| `online.laduje` | Ładuję… | Wczytuję… | |
| `online.blad_ladowania` | (nowy; dziś zostaje „Ładuję…” albo pusto) | Nie udało się wczytać danych. {powod} | `{powod}` z `konto.blad.*`. Plus przycisk `online.ponow`. |
| `online.ponow` | (nowy) | Spróbuj ponownie | |

### 22.6. Zakładka Znajomi

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `znajomi.moj_profil` | Mój profil - jak widzą mnie inni | Mój profil oczami innych | Przycisk i podpowiedź. |
| `znajomi.moje_wyniki` | Moje wyniki online · ten tydzień | Moje wyniki online · ten tydzień | |
| `znajomi.moje_wyniki_wiersz` | {kroki} kroków · {km} km · {czas} | {kroki} {kroki:krok/kroki/kroków} · {km} km · {czas} | |
| `znajomi.podzial_pokaz` | Pokaż podział na gry | Pokaż podział na gry | |
| `znajomi.podzial_ukryj` | Ukryj podział na gry | Ukryj podział na gry | |
| `znajomi.zaproszenia_do_mnie` | Zaproszenia do Ciebie | Zaproszenia do Ciebie | |
| `znajomi.odrzuc` | Odrzuć | Odrzuć | |
| `znajomi.przyjmij` | Przyjmij | Przyjmij | |
| `znajomi.lista` | Znajomi | Znajomi | |
| `znajomi.pusto` | Jeszcze nikogo. Zaproś znajomego po pseudonimie albo zaproś do WalkLevel kogoś, kto nie ma konta. | Lista znajomych jest pusta. Zaproś kogoś po pseudonimie albo daj kod na nowe konto komuś, kto konta nie ma. | |
| `znajomi.idzie` | idzie teraz {kmh} km/h · {kroki} kroków | idzie teraz {kmh} km/h · {kroki} {kroki:krok/kroki/kroków} | |
| `znajomi.tydzien` | tydzień: {kroki} kroków · {czas} | w tym tygodniu: {kroki} {kroki:krok/kroki/kroków} · {czas} | |
| `znajomi.usun` | Usuń | Usuń ze znajomych | Przycisk z przytrzymaniem. |
| `znajomi.dodaj` | Dodaj znajomego | Dodaj znajomego | |
| `znajomi.dodaj_opis` | Wpisz pseudonim osoby, która ma konto. Zaproszenie musi przyjąć. | Wpisz pseudonim osoby, która ma konto. Ta osoba musi przyjąć zaproszenie. | |
| `znajomi.dodaj_pole` | Pseudonim znajomego | Pseudonim | |
| `znajomi.zapros` | Zaproś | Zaproś | |
| `znajomi.wyslane` | Wysłane - czekają na odpowiedź | Wysłane zaproszenia, czekają na odpowiedź | |
| `znajomi.wycofaj` | Wycofaj | Wycofaj | |
| `znajomi.zaproszono` | Wysłano zaproszenie do {pseudonim}. | Wysłano zaproszenie do {pseudonim}. | |
| `znajomi.przyjeto` | {pseudonim} jest teraz Twoim znajomym. | {pseudonim} jest teraz na liście Twoich znajomych. | Bez rodzaju. |
| `znajomi.odpowiedziano` | Gotowe. | Zapisano odpowiedź. | „Gotowe.” po przyjęciu albo odrzuceniu niczego nie mówiło. Lepiej osobno: „Przyjęto zaproszenie.” / „Odrzucono zaproszenie.” |
| `znajomi.usunieto` | Usunięto ze znajomych. | Usunięto ze znajomych. | |
| `znajomi.kod.tytul` | Zaproś do WalkLevel | Zaproś do WalkLevel | |
| `znajomi.kod.opis` | Dla kogoś bez konta: kod do wpisania przy zakładaniu konta. Ta osoba od razu będzie Twoim znajomym. | Dla kogoś bez konta: kod do wpisania przy zakładaniu konta. Ta osoba od razu trafi na Twoją listę znajomych. | Bez rodzaju. |
| `znajomi.kod.pokaz` | Pokaż kod na nowe konto | Pokaż kod na nowe konto | |
| `znajomi.kod.kod` | Kod na nowe konto: {kod} | Kod na nowe konto: {kod} | |
| `znajomi.kod.zasady` | Działa raz, przez 14 dni. Ta osoba od razu będzie Twoim znajomym. | Kod działa raz, przez 14 dni. | Druga część jest już w opisie. |
| `znajomi.kod.kolejny` | Kolejny kod | Nowy kod | |
| `znajomi.kod.kopiuj` | (nowy) | Kopiuj kod | Przycisk; po kliknięciu „Skopiowano kod.” (`znajomi.kod.skopiowano`). |

### 22.7. Zakładka Ranking

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `ranking.zakres` | Zakres rankingu | Zakres rankingu | |
| `ranking.zakres.tydzien` | Tydzień | Ten tydzień | |
| `ranking.zakres.miesiac` | Miesiąc | Ten miesiąc | |
| `ranking.zakres.zawsze` | Od początku | Od początku | |
| `ranking.zakres.poziomy` | Poziomy | Poziomy | |
| `ranking.tytul.poziomy` | Ranking poziomów | Ranking poziomów | |
| `ranking.tytul.kroki_zawsze` | Ranking kroków · od początku | Ranking kroków · od początku | |
| `ranking.tytul.kroki` | Ranking kroków | Ranking kroków | |
| `ranking.pusto` | Na razie nikt nie ma wyników w tym okresie. | W tym okresie nikt nie ma jeszcze wyników. | |
| `ranking.miejsce` | {liczba}. | {liczba}. | |
| `ranking.wiersz.poziom` | {km} km łącznie | {km} km łącznie | |
| `ranking.wiersz.kroki` | {kroki} kroków · {km} km | {kroki} {kroki:krok/kroki/kroków} · {km} km | |
| `ranking.opis.poziomy` | Ranking poziomów liczy wszystko od początku konta (tylko sesje wysłane na bieżąco). Tydzień i miesiąc to rankingi „na teraz”. | Ranking poziomów liczy kilometry od założenia konta. Tydzień i miesiąc pokazują bieżący okres. | Nawias „tylko sesje wysłane na bieżąco” był niezrozumiały. Jeśli to ważne (sesje sprzed konta się nie liczą), napisać wprost: „Sesje sprzed założenia konta się nie liczą.” |
| `ranking.opis.zasady` | W rankingu są tylko sesje z bieżni (bez symulatora) widoczne dla wszystkich albo dla znajomych. Ukryte sesje i profil ukryty nie wchodzą do rankingu. | W rankingu liczą się sesje z prawdziwej bieżni, widoczne dla znajomych albo wszystkich. Sesje i profile ukryte nie są liczone. | |

### 22.8. Zakładka Osoby z kontem

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `osoby_online.szukaj_pole` | Szukaj po pseudonimie | Szukaj po pseudonimie | |
| `osoby_online.szukaj` | Szukaj | Szukaj | |
| `osoby_online.tytul` | Osoby z kontem | Osoby z kontem | |
| `osoby_online.poprzednie` | Poprzednie | Poprzednia strona | |
| `osoby_online.nastepne` | Następne | Następna strona | |
| `osoby_online.strona` | strona {liczba} | strona {liczba} | |
| `osoby_online.pusto` | Nikogo nie znaleziono. | Nie ma nikogo o takim pseudonimie. Sprawdź pisownię. | Wariant bez wyszukiwania: „Na liście nie ma jeszcze nikogo.” (`osoby_online.pusto_lista`). |
| `osoby_online.wiersz` | tydzień: {kroki} kroków · {liczba}. miejsce | w tym tygodniu: {kroki} {kroki:krok/kroki/kroków} · {liczba}. miejsce | |
| `osoby_online.zapros` | Zaproś | Zaproś | |
| `osoby_online.opis` | Na liście są tylko osoby, które same ustawiły profil jako widoczny dla wszystkich. E-maili nikt nie widzi. | Na liście są tylko osoby, które pokazują swój profil wszystkim. Adresów e-mail nikt nie widzi. | |

### 22.9. Zakładka Moje konto

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `moje.wyniki.tytul` | Moje wyniki na koncie | Moje wyniki na koncie | |
| `moje.wyniki.wysylaj` | Wysyłaj moje sesje | Wysyłaj moje sesje | |
| `moje.wyniki.wysylaj_opis` | Co 2 minuty i po każdej sesji | Co 2 minuty i po każdej sesji. | |
| `moje.wyniki.kto` | Kto widzi moje sesje | Kto widzi moje sesje | |
| `moje.widocznosc.ja` | Tylko ja | Tylko ja | |
| `moje.widocznosc.znajomi` | Znajomi | Znajomi | |
| `moje.widocznosc.wszyscy` | Wszyscy | Wszyscy | |
| `moje.wyniki.jedna_sesja` | Pojedynczą sesję ukryjesz w Historii kroków (zostaje tylko u Ciebie). | Pojedynczą sesję ukryjesz w Historia kroków > szczegóły sesji. Zostanie wtedy tylko na tym komputerze. | Pełna ścieżka. |
| `moje.na_zywo` | Na żywo dla znajomych | Na żywo dla znajomych | |
| `moje.na_zywo_opis` | Znajomi widzą, że idziesz, i Twoje tempo | Znajomi widzą, że idziesz, i z jaką prędkością. | |
| `moje.wyslij_teraz` | Wyślij teraz | Wyślij teraz | |
| `moje.profil.tytul` | Mój profil - jak widzą mnie inni | Mój profil oczami innych | |
| `moje.profil.etykieta` | Profil | Profil | |
| `moje.profil.pseudonim` | Pseudonim | Pseudonim | |
| `moje.profil.zapisz` | Zapisz pseudonim | Zapisz pseudonim | |
| `moje.profil.zapisano` | Zapisano. | Zapisano. | |
| `moje.profil.lista` | Mój profil na liście osób z kontem | Kto widzi mój profil | |
| `moje.profil.lista_dostepnosc` | Widoczność profilu | Widoczność profilu | |
| `moje.profil.ukryty` | Ukryty | Nikt | Opcje: Nikt / Znajomi / Wszyscy (dziś Ukryty / Znajomi / Wszyscy). |
| `moje.gry.kto` | Podział wyników na gry widzą | Kto widzi wyniki w poszczególnych grach | |
| `moje.gry.kto_dostepnosc` | Podział na gry widzą | Kto widzi podział na gry | |
| `moje.gry.opis` | Domyślnie inni widzą sumę kroków, a po kliknięciu - wyniki w poszczególnych grach. Pojedynczą grę ukryjesz w jej profilu (Pokazuj tę grę na koncie online). | Inni widzą sumę kroków, a po kliknięciu także wyniki w poszczególnych grach. Pojedynczą grę ukryjesz w Ustawienia > Gry i sterowanie > Profile gier > (profil) > Pokazuj tę grę na koncie online. | |
| `moje.haslo.tytul` | Hasło | Hasło | |
| `moje.haslo.obecne` | Obecne hasło | Obecne hasło | |
| `moje.haslo.nowe` | Nowe hasło | Nowe hasło | |
| `moje.haslo.nowe2` | Powtórz nowe hasło | Powtórz nowe hasło | |
| `moje.haslo.zmien` | Zmień hasło | Zmień hasło | |
| `moje.haslo.rozne` | Nowe hasła różnią się. | Nowe hasła się różnią. Wpisz to samo w obu polach. | |
| `moje.haslo.opis` | Zmiana hasła wylogowuje Cię na innych komputerach (jak „wyloguj wszędzie”). | Po zmianie hasła inne komputery zostaną wylogowane. | |
| `moje.haslo.zmienione` | Hasło zmienione - inne urządzenia zostały wylogowane. | Zmieniono hasło. Inne komputery zostały wylogowane. | |
| `moje.konto.tytul` | Konto | Konto | |
| `moje.konto.zapamietaj` | Zapamiętaj logowanie | Zapamiętaj logowanie | |
| `moje.konto.zapamietaj_wyl` | Wyłączone - wylogowanie przy zamknięciu | Gdy wyłączone, zamknięcie WalkLevel wyloguje z konta. | |
| `moje.konto.pobierz` | Pobierz moje dane | Pobierz moje dane z serwera | |
| `moje.konto.pobieram` | Pobieram do {plik}… | Pobieram dane do {plik}… | |
| `moje.konto.pobrano` | Zapisano Twoje dane z serwera: {plik} | Zapisano Twoje dane z serwera w pliku {plik}. | |
| `moje.konto.wyloguj` | Wyloguj | Wyloguj się | |
| `moje.konto.usun_opis` | Usunięcie konta kasuje je z serwera razem z wynikami i znajomościami. Wyniki na tym komputerze zostają. Wpisz hasło i przytrzymaj przycisk. | Usunięcie konta kasuje je z serwera razem z wynikami i znajomymi. Wyniki na tym komputerze zostaną. Wpisz hasło i przytrzymaj przycisk. | |
| `moje.konto.usun_haslo` | Hasło | Hasło | |
| `moje.konto.usun` | Usuń konto | Usuń konto | Przycisk z przytrzymaniem. |
| `moje.konto.usun_brak_hasla` | Wpisz hasło, żeby usunąć konto. | Wpisz hasło, żeby usunąć konto. | |

### 22.10. Profil osoby (widok jak u innych)

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `profil_online.tytul` | Profil | Profil | |
| `profil_online.pokaz` | Pokaż profil | Pokaż profil | |
| `profil_online.moj_opis` | To Twój profil w takim widoku, jaki dostają inni. Kto go widzi, ustawiasz w zakładce Ja (widoczność profilu i sesji). | Tak widzą Twój profil inni. Kto go widzi, ustawisz w zakładce Moje konto. | Zakładka „Ja” nie istnieje od 0.5.0 (nazywa się „Konto”). |
| `profil_online.od` | W WalkLevel od {data} | W WalkLevel od {data} | |
| `profil_online.bieznia` | Bieżnia: {model} | Bieżnia: {model} | |
| `profil_online.znajomy` | Znajomy | Na liście znajomych | Bez rodzaju. |
| `profil_online.nieznajomy` | Nie jesteście znajomymi | Nie jesteście znajomymi | |
| `profil_online.7dni` | Ostatnie 7 dni | Ostatnie 7 dni | |
| `profil_online.razem` | Razem | Razem | |
| `profil_online.wiersz` | {kroki} kroków · {km} km · {czas} | {kroki} {kroki:krok/kroki/kroków} · {km} km · {czas} | |
| `profil_online.sesje` | sesje: {liczba} | {liczba} {liczba:sesja/sesje/sesji} | |
| `profil_online.gry` | Gry · razem | Gry · razem | |
| `profil_online.gry_ukryte` | Ta osoba nie pokazuje podziału na gry. | Ta osoba nie pokazuje wyników w poszczególnych grach. | |
| `profil_online.gry_brak` | Brak danych o grach. | Brak danych o grach. | |
| `profil_online.bez_gry` | bez gry | bez nazwy gry | |
| `profil_online.gra_wiersz` | {kroki} kroków · {km} km · {czas} | {kroki} {kroki:krok/kroki/kroków} · {km} km · {czas} | |
| `profil_online.moje_ukryte` | Sesje ukryte („Tylko ja”) nie wchodzą do wyników widocznych dla innych. | Sesje ukryte („Tylko ja”) nie liczą się w wynikach widocznych dla innych. | |
| `profil_online.cudze_ukryte` | Widać tylko sesje, które ta osoba pokazuje Tobie (bez ukrytych). | Widać tylko sesje, które ta osoba Ci pokazuje. | |

### 22.11. Błędy konta (`konto/__init__.py`, `konto/api.py`, `konto/sejf.py`)

Teksty z serwera (pole `message` z tras `/api/bieznia/*`) przychodzą gotowe i są poza tym słownikiem.
Trzeba je przejrzeć osobno w kodzie serwera według tych samych zasad.

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `konto.blad.pseudonim_spacje` | Pseudonim: bez spacji na początku i na końcu. | Pseudonim nie może zaczynać się ani kończyć spacją. | |
| `konto.blad.pseudonim_dlugosc` | Pseudonim: od {min} do {max} znaków. | Pseudonim musi mieć od {min} do {max} znaków. | |
| `konto.blad.pseudonim_znaki` | Pseudonim: litery, cyfry, spacja, _, - i kropka. | Pseudonim może mieć tylko litery, cyfry, spację, podkreślnik, łącznik i kropkę. | |
| `konto.blad.pseudonim_zarezerwowany` | Ten pseudonim jest zarezerwowany - wybierz inny. | Ten pseudonim jest zajęty. Wybierz inny. | |
| `konto.blad.kod` | Wpisz kod zaproszenia (dostaniesz go od znajomego). | Wpisz kod zaproszenia. Da Ci go ktoś, kto już ma konto. | |
| `konto.blad.email` | Wpisz poprawny adres e-mail. | Wpisz poprawny adres e-mail, np. imie@poczta.pl. | |
| `konto.blad.haslo_krotkie` | Hasło: co najmniej {liczba} znaków. | Hasło musi mieć co najmniej {liczba} {liczba:znak/znaki/znaków}. | |
| `konto.blad.internet` | Brak połączenia z serwerem kont - sprawdź internet. | Brak połączenia z serwerem kont. Sprawdź połączenie z internetem. | |
| `konto.blad.400` | Serwer nie przyjął danych. | Serwer nie przyjął tych danych. Sprawdź pola i spróbuj ponownie. | |
| `konto.blad.403` | Brak dostępu - potwierdź adres e-mail (link w wiadomości). | Najpierw potwierdź adres e-mail: kliknij link w wiadomości od WalkLevel. Nowy link wyślesz w zakładce Moje konto. | |
| `konto.blad.404` | Nie znaleziono. | Nie znaleziono. | |
| `konto.blad.404_funkcja` | Ta funkcja będzie dostępna wkrótce. | Ta funkcja jeszcze nie działa na serwerze. Spróbuj później. | |
| `konto.blad.413` | Za dużo danych naraz. | Za dużo danych naraz. Spróbuj ponownie za chwilę. | |
| `konto.blad.429` | Za dużo prób - odczekaj chwilę i spróbuj ponownie. | Za dużo prób. Odczekaj chwilę i spróbuj ponownie. | |
| `konto.blad.429_logowanie` | Za dużo prób - spróbuj za minutę. | Za dużo prób logowania. Spróbuj za minutę. | |
| `konto.blad.logowanie` | Nieprawidłowy e-mail lub hasło. | Nieprawidłowy e-mail lub hasło. | |
| `konto.blad.500` | Serwer kont ma chwilowy problem - spróbuj później. | Serwer kont ma chwilowy problem. Spróbuj później. | |
| `konto.blad.kod_inny` | Serwer odpowiedział błędem {kod}. | Serwer kont nie odpowiada poprawnie. Spróbuj później. (kod {kod}) | |
| `konto.blad.niezalogowany` | Zaloguj się, żeby korzystać z funkcji online. | Zaloguj się, żeby korzystać z funkcji online. | |
| `konto.blad.sesja_wygasla` | Sesja wygasła - zaloguj się ponownie. | Logowanie wygasło. Zaloguj się ponownie. | „Sesja” znaczy w aplikacji co innego (sesja chodzenia). |
| `konto.blad.aktualizacja` | (tekst z serwera przy kodzie 426) | Ta wersja WalkLevel jest za stara dla konta online. Zaktualizuj aplikację: kliknij Aktualizuj na górnym pasku. | Tekst zastępczy, gdy serwer nie poda własnego. |
| `konto.blad.przekierowanie` | Serwer przekierował na inny adres - przerwano. | Połączenie przerwane dla bezpieczeństwa (nieznany adres). | |
| `konto.blad.za_duza_odpowiedz` | Odpowiedź serwera jest za duża - przerwano. | Serwer odpowiedział niezrozumiale. Spróbuj później. | Trzy rzadkie przypadki jednym tekstem. |
| `konto.blad.format` | Serwer odpowiedział w nieznanym formacie. | Serwer odpowiedział niezrozumiale. Spróbuj później. | |
| `konto.blad.adres_https` | Adres serwera kont musi zaczynać się od https:// | Adres serwera kont w konfiguracji jest błędny. | Błąd konfiguracji. |
| `konto.blad.adres` | Niepoprawny adres serwera kont. | Adres serwera kont w konfiguracji jest błędny. | |
| `konto.blad.sejf` | Menedżer poświadczeń niedostępny - logowanie tylko do zamknięcia | Nie da się zapamiętać logowania w tym Windows. Po zamknięciu WalkLevel trzeba będzie zalogować się ponownie. | Dziś tylko w logu. |
| `konto.sejf.opis` | WalkLevel - logowanie do konta online | WalkLevel: logowanie do konta online | Opis wpisu w Menedżerze poświadczeń Windows. |

## 23. Nazwy przycisków pada (`pad.py`)

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `pad.ps.krzyzyk` | Krzyżyk | Krzyżyk | Analogicznie: Kółko, Kwadrat, Trójkąt, Create, Options, Panel dotykowy. |
| `pad.ps.l3` | L3 (lewa gałka) | L3 (wciśnięcie lewej gałki) | |
| `pad.ps.r3` | R3 (prawa gałka) | R3 (wciśnięcie prawej gałki) | |
| `pad.xbox.ls` | LS (lewa gałka) | LS (wciśnięcie lewej gałki) | |
| `pad.xbox.rs` | RS (prawa gałka) | RS (wciśnięcie prawej gałki) | |
| `pad.xbox.view` … `pad.xbox.przycisk` | View, Menu, Xbox, A, B, X, Y, LB, RB | bez zmian | |
| `pad.strzalki` | Strzałka w górę / w dół / w lewo / w prawo | bez zmian | |
| `pad.przycisk_nr` | Przycisk {liczba} | Przycisk {liczba} | Nieznany pad. |
| `pad.nazwa.xbox` | Pad Xbox (XInput {liczba}) | Pad Xbox {liczba} | Bez „XInput”. |
| `pad.nazwa.ps` | Pad PlayStation | Pad PlayStation | |
| `pad.nazwa.ps_model` | {model} (Bluetooth/USB) | {model} (Bluetooth/USB) | DualSense, DualSense Edge, DualShock 4. |
| `pad.nazwa.ogolna` | Pad {liczba} | Pad {liczba} | |
| `pad.brak` | brak | brak | Gdy nie przypisano przycisku. |

## 24. Nazwy klawiszy (`klawisze.py`)

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `klawiatura.*` | Strzałka w górę, Strzałka w dół, Strzałka w lewo, Strzałka w prawo, Shift, Prawy Shift, Ctrl, Prawy Ctrl, Alt, Prawy Alt, Spacja, Tab, Caps Lock, Enter, Backspace, Insert, Delete, Home, End, Page Up, Page Down, Win | bez zmian | Nazwy jak na klawiaturze. |

## 25. Elementy wspólne

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `wspolne.potwierdz` | – | Na pewno? Kliknij jeszcze raz | Napis groźnego przycisku po pierwszym kliknięciu (26.11: zamiast przytrzymania, które nic nie zapowiadało). Po 4 s wraca. |
| `wspolne.potwierdz_dostepnosc` | – | {tekst} - kliknij dwa razy, żeby potwierdzić | Nazwa dostępna groźnego przycisku. |
| `wspolne.zaznaczone` | zaznaczone / niezaznaczone / częściowo | zaznaczone / niezaznaczone / zaznaczone częściowo | Dostępność pola wyboru. |
| `wspolne.edytuj` | Edytuj | Edytuj | |
| `wspolne.edytuj_co` | Edytuj: {nazwa} | Edytuj: {nazwa} | |
| `wspolne.wybrany` | {nazwa} (wybrany). {opis} | {nazwa}, wybrany. {opis} | |
| `wspolne.anuluj` | Anuluj | Anuluj | |
| `wspolne.usun` | Usuń | Usuń | |
| `wspolne.zmien` | Zmień | Zmień | |
| `wspolne.blad_nieznany` | (nowy) | Coś poszło nie tak. Spróbuj ponownie, a jeśli to wraca, zgłoś problem w Ustawienia > Pomoc i diagnostyka. | Ostatnia deska dla nieprzewidzianych błędów z Rust. Nigdy surowy komunikat wyjątku. |

## 26. Instalator (`packaging/instalator.iss`)

Jeśli 0.6.0 ma nowy instalator (Tauri/NSIS albo nowe Inno Setup), teksty:

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `instalator.tytul` | Instalacja WalkLevel | Instalacja WalkLevel | |
| `instalator.gotowe1` | WalkLevel {wersja} by damnit.com.pl zostanie zainstalowany na tym komputerze. | WalkLevel {wersja} zostanie zainstalowany na tym komputerze. | „by damnit.com.pl” to wtrącenie po angielsku; autor jest w danych wydawcy. |
| `instalator.gotowe2` | Kliknij Instaluj. Nie są potrzebne uprawnienia administratora, a po instalacji aplikacja otworzy się sama. | Kliknij Instaluj. Uprawnienia administratora nie są potrzebne, a po instalacji WalkLevel otworzy się sam. | |
| `instalator.skrot` | WalkLevel | WalkLevel | Skrót w menu Start i na pulpicie. Instalator nadal usuwa stare skróty „Bieżnia” i „Bieżnia Gamingowa” (to nazwy plików, nie tekst dla gracza). |

## 27. Domyślne nazwy i losowanie nazw

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `nazwy.osoba_domyslna` | Gracz | Gracz | |
| `nazwy.profil_domyslny` | Domyślny | Domyślny | |
| `nazwy.profil` | Profil | Profil | |
| `nazwy.losowe` | (`nazwy.py`: przymiotnik + rzeczownik w dwóch rodzajach, np. Szybki Biegacz, Szybka Biegaczka) | bez zmian | To nazwy do wyboru, nie zwroty do gracza. Rodzaj jest tu zamierzony i gracz wybiera sam. Zarezerwowana nazwa „bieżnia” zostaje. |

## 28. Treści z `bieznia/poziomy/definicje.json`

Rangi (40 pozycji: `nazwa`, `opis`) i osiągnięcia (`nazwa`, `klimat`, `warunek`) zostają w
`definicje.json`, bo plik jest wspólny dla aplikacji i serwera. Interfejs Svelte czyta je z tego
pliku, nie z tego słownika. Przegląd według tych samych zasad:

| Miejsce | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| zwracanie się do gracza | „twoimi nogami”, „twoje ślady”, „twojego zaproszenia”, „za tobą”, „znają cię” | „Twoimi nogami”, „Twoje ślady”, „Twojego zaproszenia”, „za Tobą”, „znają Cię” | Jednolicie z interfejsem (wielka litera). Zmiana tekstu nie zmienia progów, więc nie wymaga nowej `wersja`. |
| `warunek` serii | Chodź min. 2 km dziennie przez 3 dni z rzędu (1 dzień wolny na 7 nie przerywa serii). | Chodź co najmniej 2 km dziennie przez 3 dni z rzędu. Jeden dzień przerwy na tydzień nie przerywa serii. | Bez skrótu „min.” i bez nawiasu. Tak samo dla 7, 28, 80 i 365 dni. |
| `polecenie.warunek` | Ktoś założy konto z twoim kodem. | Ktoś założy konto z Twoim kodem zaproszenia. | |
| `znajomy.klimat` | Do drużyny dołączył pierwszy kompan. | bez zmian | Rodzaj dotyczy postaci z gry, nie gracza. Do decyzji Damiana, ewentualnie „W drużynie jest już pierwsza osoba.” |
| rangi | np. „Hobbit z Shire”, „Szary Pielgrzym” | bez zmian | Nazwy postaci, rodzaj jest częścią postaci. |
| pozostałe `klimat` i `warunek` | | bez zmian | Bez wykrzykników i bez rodzaju gracza, krótkie. |

## 29. Poza słownikiem (świadomie)

- **Wpisy dziennika (`log.info`, `log.warning`)** w `bieznia/ble/`, `bieznia/sterowniki/`,
  `bieznia/konto/`, `bieznia/historia.py` itd. Czyta je autor, gracz widzi je tylko na ekranie
  Dziennik zdarzeń. Zostają techniczne, po polsku. Wyjątki, które mają być też komunikatem dla
  gracza, są w słowniku (np. `ble.stop_niepotwierdzony`, `ble.predkosc_nieprzyjeta`,
  `historia.niedostepna`, `konto.blad.sejf`).
- **Raport diagnostyczny** (`ble/diagnostics.py`): plik tekstowy dla autora (skan, usługi,
  charakterystyki). Bez zmian. Gracz widzi tylko stan z `dziennik.raport_*`.
- **Opisy błędów walidacji profilu bieżni** (`sterowniki/profile.py`, np. „speed_min musi być
  mniejsze od speed_max”): trafiają do `{szczegoly}` w `modele.plik.zly`.
- **Komunikaty protokołu FTMS** (`sterowniki/ftms/protokol.py`: „przyjęte”, „nieobsługiwane”,
  „zatrzymana kluczem bezpieczeństwa”): tylko dziennik.
- **Teksty serwera kont** (`message` z `/api/bieznia/*`) i maile (`strona/maile/`): osobny
  przegląd po stronie serwera.

## 30. Najważniejsze problemy znalezione w 0.5.2

1. **Tryb próbny niezrozumiały.** „Wyjście: Podgląd / Klawiatura” i dopisek „(podgląd)” nie
   mówiły, że postać nie pójdzie. Zmiana: `ustawienia.chodzenie.klawisze_w_grze`,
   `glowny.podpowiedz.chodzenie_proba`, `glowny.wskaznik.chodzenie.proba`.
2. **Tryb Trening pokazywał podpowiedź trybu Chodzenie** (brak gałęzi w `render_output_hint`).
   Nowy tekst `glowny.podpowiedz.trening`.
3. **Stara nazwa w trzech miejscach:** „Prawy przycisk - okno Bieżni” (dymek), „Kopia Bieżni
   (*.bieznia)”, „To nie jest plik kopii Bieżni”, „zaktualizuj Bieżnię”. Do decyzji: rozszerzenie
   plików kopii `.walklevel` (z wczytywaniem `.bieznia`).
4. **Stan połączenia mylący:** podtytuł „Bieżnia włączona, telefon rozłączony” wyglądał jak stan,
   a był warunkiem; po połączeniu pokazywał opis techniczny sterownika; w trakcie skanowania
   przycisk „Szukaj bieżni” zamiast „Anuluj”; brak stanu „rozłączono, połącz ponownie”.
5. **Żargon:** fokus, token, firmware, dBm, JSON/ZIP, popup, „Nagrywaj przejścia”, „XInput”.
6. **Rodzaj gramatyczny:** „Nie znalazłem pliku gry”, „jest teraz Twoim znajomym”, „będzie Twoim
   znajomym”, „Znajomy” w profilu, „Wylosowaliśmy”, „Wysłaliśmy”, „Zaktualizowaliśmy”.
7. **Komunikaty bez działania:** m.in. „Serwer nie przyjął danych.”, „Nie znaleziono.”, „Gotowe.”
   po odpowiedzi na zaproszenie, „Raport: Raport gotowy”, „Wyślij plik z folderu logs”.
8. **Nieaktualne odwołania:** „w zakładce Ja” (zakładka nazywa się Konto), „Nie trafia: nazwy gier”
   (od 0.5.0 nazwy gier mogą trafiać na konto).
9. **Brak odmiany liczebników:** „dziś 1 kroków”, „5 skoki”, „+1 kolejnych”, „1 / 3 GRY”,
   „ok. 1 dni”.
10. **Brakujące stany:** błąd wczytywania danych online (zostawało „Ładuję…”), pusta lista sesji,
    niedostępna historia, sesja odrzucona przez serwer, brak XP z symulatora, niepotwierdzony
    stop taśmy (tylko w dzienniku).
11. **Jeden wykrzyknik:** „dziękuję!” po wysłaniu zgłoszenia.

## Dodatki: Główny

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `glowny.osoby.lista` | (nowy) | Osoby na tym komputerze | Nazwa listy rozwijanej osób (czytnik ekranu). |
| `glowny.sterowanie.lista` | (nowy) | Profile gier | Nazwa listy rozwijanej profili (czytnik ekranu). |
| `glowny.sterowanie.tryby` | (nowy) | Tryb sterowania | Nazwa grupy kafelków trybu (czytnik ekranu). |
| `glowny.licznik.kadencja_krotko` | Kroki/min | Kroki/min | Tytuł kafelka; pełna nazwa dla czytnika ekranu to `glowny.licznik.kadencja`. |
| `glowny.strefy.puszczona_krotko` | 0% | 0% | Nazwa pierwszego żetonu trybu Pad przy 5 stopniach (krótsza niż „Gałka puszczona”). |
| `tryb.chodzenie.opis_proba` | (nowy) | tryb próbny, postać stoi | Opis kafelka Chodzenie w trybie próbnym (gra nie dostaje klawiszy). |
## Dodatki: Ustawienia

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `ustawienia.osoby.wiersz_bez_konta` | (nowy) | {liczba} {liczba:profil gry/profile gier/profili gier} · dziś {kroki} {kroki:krok/kroki/kroków} | Wiersz osoby bez konta online (`ustawienia.osoby.wiersz` bez części „· konto …”). |
| `ustawienia.sterowanie.aktywny_tryb` | (nowy) | aktywny tryb | Podpowiedź przy kropce przed nazwą trybu, który działa teraz (zamiast „✓”). |
| `ustawienia.wlaczanie.pad_rola` | (nowy) | Przycisk pada | Rola przycisku pada: „Przycisk pada: Panel dotykowy”. |
## Dodatki: historia

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `historia.sesje.wiersz_moja` | (nowy) | {dzien} {godzina} · {profil} / {kroki} {kroki:krok/kroki/kroków} · {czas} · {kmh} km/h | Wiersz sesji w historii jednej osoby (bez nazwy osoby). |
| `historia.wykres.teraz` | teraz | teraz | Oś wykresu godzinowego, bieżąca godzina. W 0.5.2 wpisane w kod. |
| `historia.wykres.dni_tygodnia` | pn wt śr cz pt sb nd | pn wt śr cz pt sb nd | Oś wykresu 7 dni. Skróty od poniedziałku, rozdzielone spacją. |
| `historia.wykres.slupek` | (nowy) | {kiedy} · {kroki} {kroki:krok/kroki/kroków} | Podpowiedź po najechaniu na słupek, np. „13:00 · 1 234 kroki”. |
| `historia.wykres.opis` | (nowy) | Razem {kroki} {kroki:krok/kroki/kroków}, najwięcej: {kiedy}. | Opis wykresu dla czytnika ekranu. |
| `sesja.online.kto` | (nowy) | Kto ją widzi: {kto}. Zmienisz to w Konto online > Moje konto. | `{kto}` z `moje.widocznosc.*`. Tylko dla wybranej osoby z kontem. |
| `sesja.usun.opis_lokalnie` | (nowy) | Sesja zniknie z historii na tym komputerze. Tego nie da się cofnąć. Przytrzymaj przycisk, żeby usunąć. | Wariant `sesja.usun.opis` dla osoby bez konta online. |
## Dodatki: Osiągnięcia i szlak

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `osiagniecia.tytul` | (nowy) | Osiągnięcia | Tytuł ekranu (wyglad.md 9.1: zmienna część w podpisie). |
| `osiagniecia.podpis` | (nowy) | {liczba} z {razem} zdobytych | Podpis pod tytułem. |
| `osiagniecia.dzis` | dzisiaj | dziś | Do `poziomy.osiagniecie.zdobyte`. |
| `osiagniecia.wczoraj` | wczoraj | wczoraj | Do `poziomy.osiagniecie.zdobyte`. |
| `osiagniecia.zdobyte` | zdobyte | zdobyte | Stan dla czytnika ekranu. |
| `osiagniecia.dostepnosc` | (nowy) | {nazwa}, {rzadkosc}, {stan}. {warunek} | Nazwa wiersza osiągnięcia dla czytnika ekranu (zastępuje `poziomy.osiagniecie.dostepnosc`). |
| `osiagniecia.rzadkosc_dostepnosc` | (nowy) | {rzadkosc}: {liczba} z {razem} | Komórka paska rzadkości dla czytnika ekranu. |
| `osiagniecia.lista_rzadkosci` | (nowy) | Zdobyte według rzadkości | Nazwa paska rzadkości dla czytnika ekranu. |
| `szlak.koniec` | (nowy) | Cały szlak jest już za Tobą. | Wiersz „TU JESTEŚ” po ostatnim przystanku. |
| `szlak.przystanki` | (nowy) | Przystanki szlaku | Nazwa listy dla czytnika ekranu. |
## Dodatki: online

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `online.pokaz_haslo` | (nowy) | Pokaż hasło | Przycisk z okiem w polu hasła (logowanie, rejestracja, Moje konto). |
| `ranking.ty` | (nowy) | (Ty) | Dopisek przy własnym pseudonimie w rankingu i na liście osób. |
| `online.poziom_ukryty` | (nowy) | Poziom ukryty | Podpowiedź przy neutralnym awatarze zamiast odznaki poziomu (znajomi, ranking, osoby z kontem) i wiersz w profilu osoby: serwer nie podaje poziomu bez zgody tej osoby. |
| `ranking.wiersz.kroki_slowo` | (nowy) | {kroki:krok/kroki/kroków} | Słowo pod liczbą kroków w kolumnie statystyk (liczba osobno, krojem liczb). |
| `ranking.wiersz.km` | (nowy) | {km} km | Druga linia statystyk w wierszu rankingu i gry. |
| `ranking.wiersz.lacznie` | (nowy) | łącznie | Słowo przy kilometrach w rankingu poziomów. |
| `znajomi.kod.skopiowano` | (nowy) | Skopiowano kod. | Toast po „Kopiuj kod”. |
| `osoby_online.pusto_lista` | (nowy) | Na liście nie ma jeszcze nikogo. | Lista osób bez wyszukiwania. |
| `osoby_online.wiersz_bez_miejsca` | (nowy) | w tym tygodniu: {kroki} {kroki:krok/kroki/kroków} | Osoba bez miejsca w rankingu tygodnia. |
| `osoby_online.wyslano` | (nowy) | Wysłano | Napis przycisku „Zaproś” po wysłaniu zaproszenia (lista osób, profil). |
| `moje.wyslano` | (nowy) | Wysłano {liczba} {liczba:sesję/sesje/sesji}. | Toast po „Wyślij teraz”. |
| `moje.wyslano_nic` | (nowy) | Wszystkie sesje są już na serwerze. | Toast po „Wyślij teraz”, gdy nie było czego wysłać. |
| `moje.czeka` | (nowy) | Na wysłanie {liczba:czeka/czekają/czeka} {liczba} {liczba:sesja/sesje/sesji}. | Pod „Wyślij teraz”, gdy coś czeka. |
| `moje.konto.adres` | (nowy) | Adres e-mail: {email} | Karta Konto w zakładce Moje konto. |
| `moje.konto.regulamin_wersja` | (nowy) | Zaakceptowany regulamin: wersja z {data}. | Karta Konto, obok linku do regulaminu. |
| `profil_online.brak_osiagniec` | (nowy) | Jeszcze bez osiągnięć. | Profil osoby bez zdobytych osiągnięć. |
| `profil_online.zdobyte_dzis` | (nowy) | dziś | „zdobyte dziś” w wierszu osiągnięcia na profilu. |
| `profil_online.zdobyte_wczoraj` | (nowy) | wczoraj | „zdobyte wczoraj” w wierszu osiągnięcia na profilu. |
## Dodatki: Zgłoś problem

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `zgloszenie.droga_ogolna` | (nowy) | Zgłoszenie trafia przez serwer WalkLevel prosto na skrzynkę autora. Serwer niczego nie przechowuje. | Zamiast `zgloszenie.droga`, dopóki rdzeń nie podaje adresu serwera zgłoszeń (w `statics` nie ma takiego pola). |
| `zgloszenie.droga_email` | (nowy) | Otworzy się e-mail do autora i folder z paczką. Wystarczy przeciągnąć plik do wiadomości. | Gdy brak serwera zgłoszeń (`app.reportEndpoint = false`), przy przycisku „Przygotuj e-mail ze zgłoszeniem”. |

## Dodatki: Dziennik zdarzeń

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `dziennik.podpis` | (nowy) | Na żywo · {liczba} {liczba:wpis/wpisy/wpisów} | Podpis tytułu strony; najwyżej 500 ostatnich wpisów. |
| `dziennik.pusty` | (nowy) | Dziennik jest pusty. Wpisy pojawią się tutaj, gdy coś się wydarzy. | W miejscu listy wpisów. |
| `dziennik.kopiuj` | (nowy) | Kopiuj dziennik | Przycisk: wszystkie wpisy z listy do schowka. |
| `dziennik.skopiowano` | (nowy) | Skopiowano dziennik. Możesz go wkleić do wiadomości. | Komunikat na dole okna. |
| `dziennik.kopiuj_blad` | (nowy) | Nie udało się skopiować dziennika. Zaznacz wpisy myszą i naciśnij Ctrl+C. | Komunikat na dole okna, gdy schowek jest niedostępny. |
## Dodatki: Kopia danych

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `kopia.import.wybierz` | (nowy) | Wybierz plik kopii… | Ekran wczytania bez wybranego pliku (okno wyboru zamknięte bez wyboru). |
| `kopia.import.wybierz_opis` | (nowy) | Wybierz plik kopii WalkLevel zapisany na tym albo innym komputerze. Przed wczytaniem zobaczysz, co jest w środku. | |
| `kopia.import.inny_plik` | (nowy) | Inny plik… | Przy nazwie wybranego pliku. |
| `kopia.import.osoby_sa` | (nowy) | Osoby, które już są na tym komputerze | Tytuł karty, gdy w pliku jest więcej niż jedna taka osoba (dla jednej: `kopia.import.konflikt`). |
| `kopia.import.jako_nowa_opis` | (nowy) | Dodaj jako nową osobę: osoba z pliku pojawi się obok obecnej, z numerem przy nazwie i z własnymi profilami gier oraz wynikami. | Pod `kopia.import.polacz_opis`. |
| `kopia.import.wynik.tytul` | (nowy) | Wczytano kopię | Karta po wczytaniu (zamiast samego komunikatu). |
| `kopia.import.wynik.nowe_osoby` | (nowy) | Nowe osoby | Kafelek. |
| `kopia.import.wynik.polaczone` | (nowy) | Połączone osoby | Kafelek. |
| `kopia.import.wynik.nowe_profile` | (nowy) | Nowe profile gier | Kafelek. |
| `kopia.import.wynik.nowe_sesje` | (nowy) | Nowe sesje | Kafelek. |
| `kopia.import.zakoncz` | (nowy) | Gotowe | Powrót do Ustawień po wczytaniu. |
| `kopia.import.przed.tytul` | (nowy) | Dane sprzed wczytania kopii | Karta z przywracaniem (gdy jest kopia `przed-importem-*`). |
| `kopia.import.przed.opis` | (nowy) | Zapisano je przed ostatnim wczytaniem kopii. Przywrócenie cofa to wczytanie i wszystko, co zmieniło się od tamtej chwili, także nowe sesje. | Pod nim przycisk z przytrzymaniem `kopia.import.przed.przywroc`. |
| `kopia.import.przed.przywroc` | (nowy) | Przywróć te dane | Przycisk z przytrzymaniem w karcie `kopia.import.przed.tytul` (pełny napis `ustawienia.dane.przywroc` nie mieści się w wąskim oknie). |
| `kopia.import.osoba_jest_tytul` | (nowy) | Ta osoba już jest na tym komputerze | Tytuł karty, gdy w pliku jest jedna taka osoba (tytuł w jednej linii, nazwa osoby w wierszu pod opisem). Zastępuje `kopia.import.konflikt` w tytule karty. |
## Dodatki: bieznie

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `bieznia.sygnal.silny` | (nowy) | silny | `{sygnal}` w wierszu urządzenia, rssi ≥ -60. |
| `bieznia.sygnal.sredni` | (nowy) | średni | rssi od -61 do -75. |
| `bieznia.sygnal.slaby` | (nowy) | słaby | rssi poniżej -75. |
| `bieznia.lista.twoja_podpis` | (nowy) | {model} · Twoja bieżnia · sygnał {sygnal} | Wiersz zapamiętanej bieżni na liście wyników skanowania. |
| `modele.dodatkowy` | dodany | dodatkowy | Profil bieżni spoza aplikacji (pobrany albo dodany z pliku), można go usunąć. |
## Dodatki: Profil gry

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `profil.wskaz_plik` | (nowy) | Wskaż plik gry… | Drugi przycisk obok „Wskaż folder gry…”: plik .exe albo skrót (podpowiedź `profil.wskaz_podpowiedz`). |
| `profil.folder_gry` | (nowy) | …\{folder} | Pod nazwą pliku gry: folder, w którym leży. |
| `profil.ikona_dostepnosc` | (nowy) | Ikona {nazwa} | Nazwa pola w siatce ikon dla czytnika ekranu (nazwy w `profil.ikona.*`). |
| `profil.zacznij_domyslne_opis` | (nowy) | Tryb, klawisze, stopnie pada i prędkości będą takie jak po instalacji. | Pod „Zacznij od”, gdy wybrane „Ustawień domyślnych”. |

## Dodatki: Edycja osoby

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `osoba.edycja.konto` | (nowy) | Konto online | Tytuł karty konta osoby. |
| `osoba.edycja.konto_jest` | (nowy) | Ta osoba ma konto online {pseudonim}. | |
| `osoba.edycja.konto_brak` | (nowy) | Ta osoba nie ma konta online. Bez konta wszystko działa na tym komputerze. | |
| `osoba.edycja.konto_wybierz` | (nowy) | Kontem online zarządzasz, gdy ta osoba jest wybrana. | Edycja osoby, która nie jest teraz wybrana. |
| `osoba.edycja.wybierz` | (nowy) | Wybierz tę osobę | Przełącza aplikację na tę osobę. |
## Dodatki: okna

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `dymek.podpowiedz_wl` | (nowy) | Sterowanie włączone\nKliknij, żeby wyłączyć\nPrawy przycisk otwiera okno WalkLevel\nPrzeciągnij, żeby przesunąć | Gotowa forma `dymek.podpowiedz` dla włączonego sterowania (`{wyłączyć/włączyć}` nie jest zmienną słownika). |
| `dymek.podpowiedz_wyl` | (nowy) | Sterowanie wyłączone\nKliknij, żeby włączyć\nPrawy przycisk otwiera okno WalkLevel\nPrzeciągnij, żeby przesunąć | Jw. dla wyłączonego sterowania. |
| `dymek.trening.wyzej` | (nowy) | Poziom wyżej ({klawisz_szybciej}) | Przycisk + przy kółku w trybie Trening (nazwa i podpowiedź). |
| `dymek.trening.nizej` | (nowy) | Poziom niżej ({klawisz_wolniej}) | Przycisk − w trybie Trening. |
| `dymek.trening.poziom` | (nowy) | Poziom {poziom} z {liczba} | Numer poziomu między + i − (czytnik ekranu i podpowiedź). |
| `dymek.trening.stop` | (nowy) | Stop | Przycisk pod kółkiem w trybie Trening. |
| `dymek.trening.stop_dostepnosc` | (nowy) | Zatrzymaj taśmę i wyłącz sterowanie | |

## Dodatki: rdzeń

Teksty składane w Rust (`walklevel_core::teksty::t`, ten sam słownik co interfejs), które nie miały
wcześniej wpisu.

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `glowny.wskaznik.chodzenie.proba_bieg` | (nowy) | Bieg · tryb próbny, gra nic nie dostaje | Wariant `glowny.wskaznik.chodzenie.proba` dla biegu. |
| `zgloszenie.wyslano_bez_numeru` | Wysłano zgłoszenie - dziękuję! Autor dostał opis i logi. | Wysłano zgłoszenie. Dziękujemy, autor dostał opis i dziennik. | `zgloszenie.wyslano`, gdy serwer nie podał numeru. |
| `zgloszenie.blad.mail_serwer` | {powod} Zapisz paczkę na dysk i wyślij ją e-mailem na {email_autora}. | {powod} Zapisz paczkę na dysk i wyślij ją e-mailem na {email_autora}. | `zgloszenie.blad.mail`, gdy serwer podał własny powód. |
| `zgloszenie.trwa` | Zgłoszenie już się przygotowuje. | Zgłoszenie już się przygotowuje… | Drugie kliknięcie w trakcie. |
| `ustawienia.pad.blad.liczba` | Od 2 do 5 skoków. | Liczba stopni pada: od 2 do 5. | Ochrona komendy; interfejs nie pozwala na inne. |
| `ustawienia.pad.blad.brak` | Nie ma takiego skoku. | Nie ma takiego stopnia pada. | Jw. |
| `ustawienia.pad.blad.progi` | Skoki: pierwszy od 0%, kolejne co najmniej 5 punktów dalej, najwyżej 100%, prędkość 0,5-12 km/h. | Stopnie pada: pierwszy od 0%, kolejne co najmniej 5 punktów dalej, najwyżej 100%. Prędkość 0,5–12 km/h. | Jw. |
| `ustawienia.trening.blad_predkosc` | Prędkość poziomu: 0,5-20 km/h. | Prędkość poziomu treningu: 0,5–20 km/h. | Jw. |
| `klawisz.blad.nieznany` | Tego klawisza nie da się użyć - wybierz inny. | Tego klawisza nie da się użyć. Wybierz inny. | Klawisz bez kodu Windows. |
| `klawisz.powod.ctrl_v` | Ctrl+V wkleja | Ctrl+V wkleja | Rodzeństwo `klawisz.powod.ctrl_c`. |
| `klawisz.powod.ctrl_x` | Ctrl+X wycina | Ctrl+X wycina | |
| `klawisz.powod.ctrl_z` | Ctrl+Z cofa | Ctrl+Z cofa | |
| `klawisz.powod.ctrl_y` | Ctrl+Y ponawia | Ctrl+Y ponawia | |
| `klawisz.powod.ctrl_a` | Ctrl+A zaznacza wszystko | Ctrl+A zaznacza wszystko | |
| `klawisz.powod.ctrl_s` | Ctrl+S zapisuje | Ctrl+S zapisuje | |
| `aktualizacja.trwa` | Aktualizacja już trwa. | Aktualizacja już trwa. | |
| `konto.blad.trwa` | To zadanie już trwa - chwilę. | To zadanie konta już trwa. Poczekaj chwilę. | |
| `konto.blad.zapis` | Nie udało się zapisać danych konta na tym komputerze. | Nie udało się zapisać danych konta na tym komputerze. | |
| `profil_online.brak_osoby` | Nie znalazłem tej osoby. | Nie ma już tej osoby na liście. Odśwież listę. | |
| `znajomi.przyjeto_zaproszenie` | Gotowe. | Przyjęto zaproszenie. | Za `znajomi.odpowiedziano` (uwaga tam). |
| `znajomi.odrzucono_zaproszenie` | Gotowe. | Odrzucono zaproszenie. | |
| `znajomi.wycofano` | Gotowe. | Wycofano zaproszenie. | |
| `kopia.blad.brak_wyboru` | Najpierw wskaż plik kopii. | Najpierw wybierz plik kopii. | |
| `kopia.blad.brak_przed` | Nie ma kopii sprzed importu. | Nie ma danych sprzed wczytania kopii. | |
| `kopia.blad.brak_danych` | Brak danych do zapisania. | Nie ma jeszcze danych do zapisania. Najpierw dodaj osobę. | |
| `kopia.blad.polaczenie` | Nie da się połączyć danych z kopią. | Nie udało się połączyć danych z kopią. Spróbuj ponownie. | |
| `profil.wskaz_folder_okno` | Wskaż folder gry | Wskaż folder gry | Tytuł okna systemowego (`profil.wskaz_folder` to przycisk z „…”). |
| `profil.wybierz_ponownie` | Wybierz grę jeszcze raz. | Wybierz grę jeszcze raz: wskaż jej folder albo plik. | Wybór gry wygasł. |
| `osoba.blad.ostatnia` | Ostatniej osoby nie da się usunąć. | Ostatniej osoby nie da się usunąć. | |
| `osoba.blad.brak` | Najpierw utwórz osobę. | Najpierw dodaj osobę. | |
| `profil.blad.brak` | Nie ma takiego profilu. | Nie ma takiego profilu gry. | |
| `profil.blad.ostatni` | Ostatniego profilu nie da się usunąć. | Ostatniego profilu gry nie da się usunąć. | |
| `modele.blad_folder` | Brak folderu danych na profile bieżni. | Nie da się zapisać profilu bieżni: brak folderu danych. | |
| `modele.blad_zapisu` | Nie da się zapisać profilu: {szczegoly} | Nie da się zapisać profilu bieżni. ({szczegoly}) | |
| `wspolne.blad_zapisu` | Błąd zapisu na dysku: {szczegoly} | Nie udało się zapisać na dysku. Sprawdź, czy jest wolne miejsce, i spróbuj ponownie. ({szczegoly}) | |

## Dodatki: tryb Klawisze i przycisk na ekranie (0.6.1)

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `ustawienia.klawisze.puszczony` | (nowy) | Klawisz puszczony | Grupa z wyborem w trybie Klawisze (ustawienie `key_idle`). |
| `ustawienia.klawisze.puszczony.wolno` | (nowy) | Wolne tempo | Taśma jedzie tempem „Bez klawisza” (jak do 0.6.0). |
| `ustawienia.klawisze.puszczony.stop` | (nowy) | Taśma staje | Puszczony klawisz zatrzymuje taśmę; wpisany też zamiast liczby w polu „Bez klawisza”. |
| `ustawienia.klawisze.bez_stoi` | (nowy) | Puść klawisz, a taśma stanie. Wciśnij, a ruszy. | Opis pola „Bez klawisza” przy „Taśma staje”, bieżnia z rozruchem poniżej 2 s. |
| `ustawienia.klawisze.bez_stoi_rozruch` | (nowy) | Puść klawisz, a taśma stanie. Wciśnij, a ruszy (rozruch ok. {sekundy} s). | Jw., rozruch bieżni od 2 s (`caps.startDelayS`). |
| `ustawienia.klawisze.przytrzymanie` | (nowy) | Przyspieszaj po przytrzymaniu | Grupa z wyborem (ustawienie `key_hold_s`). |
| `ustawienia.klawisze.przytrzymanie.od_razu` | (nowy) | Od razu | Bez opóźnienia (jak do 0.6.0). |
| `ustawienia.klawisze.przytrzymanie.sekundy` | (nowy) | {sekundy} s | 1, 2 albo 3 s. |
| `ustawienia.klawisze.przytrzymanie_opis` | (nowy) | Taśma przyspiesza dopiero, gdy trzymasz klawisz tyle sekund - krótkie kliknięcie nic nie zmienia. Puszczenie działa od razu. Gra dostaje klawisze bez opóźnienia. | Pod wyborem. |
| `glowny.wskaznik.klawisze.stoi` | (nowy) | Bez klawisza · taśma stoi | Podpis wskaźnika przy „Taśma staje”. |
| `glowny.podpowiedz.klawisze_stop` | (nowy) | Trzymaj {klawisz_chodu}, a taśma ruszy. Puść, a stanie. | Podpowiedź karty sterowania przy „Taśma staje” (zamiast `glowny.podpowiedz.klawisze`). |
| `dymek.stop_dostepnosc` | (nowy) | Zatrzymaj taśmę | Stop pod kółkiem poza Treningiem, gdy taśma jedzie (napis jak `dymek.trening.stop`). Włączone sterowanie się wyłącza. |

## Dodatki: pola konta online pod regulamin 2026-11-27 (0.6.1)

| Klucz | Obecny tekst | Nowy tekst | Uwaga |
|---|---|---|---|
| `profil_online.inne_gry` | (nowy) | Inne gry ({liczba}) | Ostatni wiersz podziału na gry w profilu osoby (`inne_gry`: gry bez nazw). |
| `osiagniecia.procent_graczy` | (nowy) | Ma to {procent}% graczy | Stopka osiągnięcia na ekranie Osiągnięcia (konto online, od 25 kont). |
