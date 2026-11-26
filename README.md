# WalkingPad X21 PC Gaming by damnit.com.pl

**WalkLevel** to aplikacja na Windows, która zamienia chodzenie po bieżni na sterowanie
w grach: chód to **W**, bieg to **Shift + W** (oba klawisze można zmienić). Łączy się z bieżnią przez Bluetooth i sama ją
wyszukuje. Obsługiwane bieżnie: **KingSmith WalkingPad** (np. X21, X23) i bieżnie ze standardem **Bluetooth FTMS**
(np. **Urevo SpaceWalk 5L**); model rozpoznaje sama. Jest też tryb **Trening** (zwykłe sterowanie bieżnią z komputera)
i opcjonalne **konto online** (znajomi, ranking, wyniki na koncie).

Twórca: **damnit.com.pl Damian Skrzypiec** · [damnit.com.pl](https://damnit.com.pl) ·
kontakt@damnit.com.pl

## Instalacja

1. Pobierz **`Bieznia-Setup-….exe`** z [najnowszego wydania](https://github.com/Damianejroo/WalkingPad-X21-PC-Gaming-by-damnit.com.pl/releases/latest).
2. Uruchom instalator i kliknij **Instaluj** (jeden ekran, bez administratora). Po instalacji
   aplikacja otworzy się sama.
   Windows może pokazać okno „System Windows ochronił ten komputer” (aplikacja nie ma
   płatnego certyfikatu wydawcy) - kliknij **Więcej informacji > Uruchom mimo to**.
3. Później otwieraj **Bieżnię Gamingową** z menu Start albo ze skrótu na pulpicie.

Wymagania: Windows 10 lub 11 (64-bit), Bluetooth. Instalator ma ok. 20 MB.

### Pierwsze połączenie z bieżnią

- włącz bieżnię,
- rozłącz telefon: zamknij KS Fit albo wyłącz w nim Bluetooth (bieżnia przyjmuje jedno
  połączenie naraz),
- włącz Bluetooth w Windows (Start > Ustawienia > Bluetooth i urządzenia).

Aplikacja przy pierwszym starcie sama szuka bieżni. Jeśli znajdzie jedną, łączy się od razu
i zapamiętuje ją. Jeśli kilka - pokazuje listę do wyboru.

### Pierwsze uruchomienie 0.3.0

Konfigurator w trzech krokach: nazwa osoby (wylosowana - możesz losować dalej albo wpisać
swoją) i jej kolor, pierwszy profil gry (folder gry albo profil Domyślny), sposób sterowania.
Po aktualizacji z 0.2 dotychczasowe ustawienia trafiają do pierwszej osoby (kopia starych
ustawień: `dane\kopie\ustawienia-0.2.json`).

### Osoby i profile gier

- **Osoba** ma swoje profile gier, wygląd i historię kroków. Dwie osoby na jednej bieżni liczą
  kroki osobno. Zmiana osoby: awatar na górze okna (widoczny, gdy osób jest więcej niż jedna).
- **Profil gry** to zestaw ustawień sterowania z nazwą, ikoną i kolorem. Nowy profil:
  Ustawienia > Profile gier > Nowy profil - wskaż folder gry, a nazwa, ikona i kolor wezmą się
  z pliku gry (nazwę i kolor zmienisz). Zmiana profilu: nazwa profilu na karcie sterowania.
- Liczba osób i profili nie jest ograniczona. Przełączniki na ekranie głównym można ukryć
  w Ustawieniach > Wygląd i okno.

### Obsługa

| Co | Jak |
| --- | --- |
| Włącz / wyłącz sterowanie | przełącznik na karcie sterowania albo **F8** (działa też w grze) - razem z tym bieżnia rusza i staje |
| Tryb **Klawisze** (domyślny) | trzymasz W - taśma przyspiesza do prędkości chodu, W + Shift - do biegu, puszczasz - zwalnia do prędkości bez klawisza. Aplikacja niczego wtedy nie naciska, W naciskasz sam w grze |
| Tryb **Pad** | wychylenie lewej gałki (DualSense albo pad Xbox, kabel lub Bluetooth) ustawia prędkość skokami: domyślnie 0 / 25 / 50 / 100% = 1 / 1,5 / 2,5 / 4 km/h. Liczba skoków (2-5), progi (przeciąganie kółek na osi) i prędkości w Ustawieniach. Gałka puszczona: taśma staje (domyślnie, ruch gałki ją uruchamia), prędkość skoku 1 albo 0 km/h (test) |
| Tryb **Chodzenie** | chodzenie po bieżni naciska W, szybszy chód W + Shift |
| Wskaźnik | strefy (stop / chód / bieg albo skoki pada) na łuku i jako żetony pod nim - świeci aktywna |
| Ustawienia sterowania | Ustawienia pokazują tylko to, co dotyczy trybu bieżącego profilu; zapisują się w profilu |
| Historia kroków | ikona wykresu na górze okna: dziś / 7 dni / 30 dni / wszystko, moja albo wszystkich osób, podział na profile gier |
| Kopia danych | Ustawienia > Kopia danych: eksport wybranych osób, profili, ustawień, wyników i logów do pliku `.bieznia`; import z podglądem (połącz albo dodaj jako nową osobę). Przed importem robi się kopia, którą można przywrócić |
| Zmiana klawisza F8 | Ustawienia > Zaawansowane > Klawisz start/stop (nie może to być klawisz chodu ani biegu) |
| Start i stop bieżni | Ustawienia > Bieżnia (domyślnie włączone) |
| Podgląd / Klawiatura | w trybie Chodzenie: podgląd niczego nie naciska, klawiatura naciska wybrane klawisze w aktywnym oknie |
| Zmiana klawiszy | kliknij **Chód: W** albo **Bieg: Shift** i naciśnij nowy klawisz; Esc anuluje |
| Test bez bieżni | Ustawienia > Zaawansowane > Symulator bieżni, potem **F6** wolniej, **F7** szybciej |
| Przycisk na ekranie | Ustawienia > Wygląd i okno: mały przycisk zawsze na wierzchu - klik włącza i wyłącza sterowanie, prawy przycisk otwiera okno |

Sterowanie startuje zawsze **wyłączone**. Gdy bieżnia przestanie wysyłać dane (3 s), postać
się zatrzymuje. Wyłączenie sterowania najpierw puszcza klawisze (postać staje od razu), potem
zatrzymuje taśmę. Zamknięcie aplikacji przy włączonym sterowaniu też zatrzymuje taśmę.
Gdy bieżnia nie przyjmie startu, obudź ją pilotem albo sprawdź klucz bezpieczeństwa.

### Aktualizacje

Gdy wyjdzie nowa wersja, kliknij ikonę **Aktualizuj** (strzałka w dół) na górze okna albo
Ustawienia > Sprawdź aktualizacje. Aplikacja pobierze instalator z GitHuba, sprawdzi jego sumę kontrolną,
zainstaluje go i uruchomi się ponownie. Ustawienia i logi zostają. Bez kliknięcia nic się
nie pobiera ani nie instaluje.

### Gdy coś nie działa

Kliknij **Zgłoś problem** (ikona na górze okna), opisz, co się stało, i wyślij. Do zgłoszenia
trafiają logi aplikacji i Bluetooth - okno pokazuje dokładnie, co zostanie dołączone.
Zgłoszenie idzie przez serwer prosto na skrzynkę autora (serwer go nie przechowuje),
a w odpowiedzi dostajesz numer zgłoszenia. Paczkę można też zapisać na dysk.
Szczegółowe logi Bluetooth można wyłączyć w Ustawieniach > Zaawansowane.

Przy problemach z połączeniem pomaga też **Ustawienia > Zaawansowane > Dziennik > Zapisz raport diagnostyczny**
(ok. 30 s): lista urządzeń w pobliżu i surowa rozmowa z bieżnią.

### Gry

- Jeśli gra działa jako administrator, Windows nie pozwoli zwykłej aplikacji naciskać w niej
  klawiszy.
- Gry z ochroną przed oszustwami mogą traktować sztucznie naciskane klawisze jak
  automatyzację. Używaj w grach, które na to pozwalają, na własną odpowiedzialność.

### Odinstalowanie

Ustawienia Windows > Aplikacje > WalkLevel > Odinstaluj. Osoby, profile, historia kroków i logi zostają w
`%LOCALAPPDATA%\Bieznia` - usuń ten folder, jeśli chcesz pozbyć się wszystkiego.

## Poziomy i osiągnięcia

Każdy kilometr na bieżni to 100 XP. Poziomy 1-100 mają 39 rang - postaci z gier i książek,
od tych, które chodziły najmniej, po te, które przeszły najwięcej. Poziom 100 wypada po ok. 5 latach
przy 5 km dziennie. Osiągnięcia (61) w pięciu stopniach: pospolite, rzadkie, epickie, heroiczne
i legendarne - szlak dystansu przez światy z gier i książek, serie dni (jeden dzień wolny na 7 nie
przerywa serii), różne gry, tryb Trening. Liczą się sesje z bieżni, bez symulatora, także wstecz
z całej historii. Raz zdobytego osiągnięcia aplikacja nie odbiera.

Nowe osiągnięcia i awanse wyskakują w prawym dolnym rogu ekranu, także nad grą - nie zabierają
fokusu, a kliknięcia przechodzą do gry. Wyłączysz je w Ustawieniach (Poziomy i osiągnięcia).
Gra w trybie wyłącznego pełnego ekranu może je przykryć; wtedy zdobycz czeka na liście osiągnięć.

Aktualizacja ze starszej wersji przechodzi bez zmian: osoby, profile i cała historia zostają,
a poziom i osiągnięcia liczą się z niej przy pierwszym uruchomieniu.

## Prywatność

Aplikacja nie ma reklam ani analityki. Dane z bieżni, historia kroków i logi zostają na komputerze.
Z internetem łączy się, żeby sprawdzić i pobrać aktualizację albo profile bieżni z GitHuba, gdy sam
wyślesz zgłoszenie, oraz - tylko jeśli założysz konto online - z serwerem kont.
Szczegóły: [Polityka prywatności](POLITYKA-PRYWATNOSCI.md) i
[Regulamin i prywatność konta online](https://bieznia.damnit-live.pl/bieznia).

## Dla programisty

```powershell
uv sync                                   # środowisko z narzędziami
uv run python -m bieznia                  # okno (z kodu, bez instalacji)
uv run python -m bieznia --headless       # bez okna (symulator, logi w konsoli)
uv run python -m bieznia --diagnostyka    # tylko raport Bluetooth
uv run pytest; uv run ruff check; uv run ruff format; uv run mypy
uv run python packaging/zbuduj.py 0.2.0   # exe + instalator (instalator wymaga Inno Setup)
```

### Wydanie nowej wersji dla testera

```powershell
git tag -a v0.2.1 -m "Poprawione łączenie z bieżnią po wybudzeniu"
git push origin v0.2.1
```

GitHub Actions uruchamia testy, buduje `Bieznia.exe` (PyInstaller) i instalator (Inno Setup),
dodaje poświadczenie pochodzenia i publikuje wydanie z opisem z tagu (ok. 5 min). Potem
wystarczy powiedzieć testerowi, żeby kliknął **Aktualizuj**.

### Struktura

- `bieznia/ble/` - Bluetooth: protokół KingSmith, FTMS, sterowniki, wątek połączenia
  z ponawianiem, raport diagnostyczny
- `bieznia/engine.py`, `mapping.py` - prędkość > W / Shift, bezpieczniki, liczniki
- `bieznia/gui/` - okno PySide6: tokeny i motywy, kontrolki (`komponenty.py`), kontroler (`app.py`)
- `bieznia/gui/ekrany/` - ekrany: główny, ustawienia, profil gry, osoby i konfigurator, historia, kopia
- `bieznia/dane.py`, `historia.py`, `kopia.py` - osoby i profile (`dane/osoby.json`), historia kroków (SQLite `dane/historia.db`), eksport i import `.bieznia`
- `bieznia/paleta.py`, `nazwy.py`, `ikony.py` - 20 kolorów, losowe nazwy osób, ikona i kolor z pliku gry
- `bieznia/aktualizacje.py` - aktualizacja z GitHub Releases (SHA-256, tylko hosty GitHuba)
- `bieznia/zgloszenia.py` - paczka z logami i wysyłka zgłoszenia
- `packaging/` - ikona, specyfikacja PyInstaller, instalator Inno Setup, skrypt budowania

### Zgłoszenia prosto do autora

**Zgłoś problem** wysyła na adres z `config.toml` (sekcja `[zgloszenia]`, `adres`):
POST `multipart/form-data` z polami `wersja`, `opis` (do 4000 znaków), `kontakt` (do 200)
i plikiem `paczka` (ZIP, do 2 MB), nagłówek User-Agent `Bieznia/<wersja>`. Odpowiedź
`{"id": "..."}` to numer zgłoszenia. Pusty adres = e-mail na kontakt@damnit.com.pl
z paczką logów.

### Jak działa połączenie z X21

X21 mówi własnym, szyfrowanym protokołem KingSmith (opisanym w otwartym sterowniku
[QZ / qdomyos-zwift](https://github.com/cagnulein/qdomyos-zwift)): usługa `0x1234`, zapis do
`FED7`, powiadomienia z `FED8`, tekst w base64 z tablicą podstawień. Bieżnie różnią się
tablicą (7 wariantów) - aplikacja wykrywa ją z odpowiedzi bieżni i nigdy nie podaje prędkości
z niepewnie odszyfrowanej ramki. Prędkość jest odpytywana co 0,75 s, bo przy stałym tempie
bieżnia sama nic nie wysyła. Start to `props ControlMode 1` + `props runState 1` (do trzech
prób, gdy bieżnia odpowie błędem), stop to `props runState 0`, prędkość to `props CurrentSpeed 1.5` (jak w QZ). Zapasowo obsługiwany jest
standard FTMS (tylko odczyt).

## Bezpieczeństwo

- aktualizacja tylko po kliknięciu, tylko z wydań tego repozytorium i tylko z hostów GitHuba;
  instalator jest uruchamiany dopiero po zgodności sumy SHA-256 podanej przez GitHub,
- wydania budują się w GitHub Actions z przypiętych wersji (akcje po commitach, Inno Setup
  po sumie SHA-256, biblioteki z `uv.lock` z sumami) i mają poświadczenie pochodzenia
  (`gh attestation verify Bieznia-Setup-….exe --repo Damianejroo/WalkingPad-X21-PC-Gaming-by-damnit.com.pl`),
- instalacja i działanie bez administratora; aplikacja startuje z wyłączonym sterowaniem,
  wysyła do bieżni tylko start, stop i prędkość (gdy sterowanie jest włączone) i puszcza W/Shift po utracie
  danych, awarii albo zamknięciu,
- skróty (F6, F7 i klawisz start/stop) są sprawdzane globalnie, ale program nie czyta ani nie
  zapisuje innych klawiszy.

## Licencja

© 2026 damnit.com.pl Damian Skrzypiec. Licencja GPL-3.0-or-later - zobacz [LICENSE](LICENSE).
Protokół KingSmith odtworzony na podstawie sterownika z projektu
[QZ (qdomyos-zwift)](https://github.com/cagnulein/qdomyos-zwift), również na licencji GPL-3.0.
Ikona przygotowana z pomocą generatora obrazów GPT.
