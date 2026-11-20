# Bieżnia

Aplikacja na Windows, która zamienia chodzenie po bieżni **WalkingPad X21** na sterowanie
w grach: chód to **W**, bieg to **Shift + W**. Łączy się z bieżnią przez Bluetooth i sama ją
wyszukuje.

Twórca: **damnit.com.pl Damian Skrzypiec** · [damnit.com.pl](https://damnit.com.pl) ·
kontakt@damnit.com.pl

## Instalacja

1. Pobierz **`Bieznia-Setup-….exe`** z [najnowszego wydania](https://github.com/Damianejroo/bieznia-olek/releases/latest).
2. Uruchom instalator. Nie wymaga uprawnień administratora.
   Windows może pokazać okno „System Windows ochronił ten komputer” (aplikacja nie ma
   płatnego certyfikatu wydawcy) - kliknij **Więcej informacji > Uruchom mimo to**.
3. Otwórz **Bieżnię** z menu Start albo ze skrótu na pulpicie.

Wymagania: Windows 10 lub 11 (64-bit), Bluetooth. Instalator ma ok. 20 MB.

### Pierwsze połączenie z bieżnią

- włącz bieżnię,
- rozłącz telefon: zamknij KS Fit albo wyłącz w nim Bluetooth (bieżnia przyjmuje jedno
  połączenie naraz),
- włącz Bluetooth w Windows (Start > Ustawienia > Bluetooth i urządzenia).

Aplikacja przy pierwszym starcie sama szuka bieżni. Jeśli znajdzie jedną, łączy się od razu
i zapamiętuje ją. Jeśli kilka - pokazuje listę do wyboru.

### Obsługa

| Co | Jak |
| --- | --- |
| Włącz / wyłącz sterowanie | przełącznik w oknie albo **F8** (działa też w grze) |
| Podgląd | nic nie jest naciskane, widać tylko, co by się stało |
| Klawiatura W / Shift | chód naciska W, bieg Shift + W w aktywnym oknie |
| Progi chodu i biegu | Ustawienia > Progi prędkości |
| Test bez bieżni | Ustawienia > Symulator bieżni, potem **F6** wolniej, **F7** szybciej |

Sterowanie startuje zawsze **wyłączone**. Gdy bieżnia przestanie wysyłać dane (2 s), postać
się zatrzymuje. Aplikacja tylko czyta bieżnię - nigdy nie zmienia jej prędkości.

### Aktualizacje

Gdy wyjdzie nowa wersja, kliknij **Aktualizuj** na dole okna (albo Ustawienia > Sprawdź
aktualizacje). Aplikacja pobierze instalator z GitHuba, sprawdzi jego sumę kontrolną,
zainstaluje go i uruchomi się ponownie. Ustawienia i logi zostają. Bez kliknięcia nic się
nie pobiera ani nie instaluje.

### Gdy coś nie działa

Kliknij **Zgłoś problem** (ikona na dole okna), opisz, co się stało, i wyślij. Do zgłoszenia
trafiają logi aplikacji i Bluetooth - okno pokazuje dokładnie, co zostanie dołączone.
Szczegółowe logi Bluetooth można wyłączyć w Ustawieniach > Dziennik i zgłoszenia.

Przy problemach z połączeniem pomaga też **Dziennik > Zapisz raport diagnostyczny**
(ok. 30 s): lista urządzeń w pobliżu i surowa rozmowa z bieżnią.

### Gry

- Jeśli gra działa jako administrator, Windows nie pozwoli zwykłej aplikacji naciskać w niej
  klawiszy.
- Gry z ochroną przed oszustwami mogą traktować sztucznie naciskane klawisze jak
  automatyzację. Używaj w grach, które na to pozwalają, na własną odpowiedzialność.

### Odinstalowanie

Ustawienia Windows > Aplikacje > Bieżnia > Odinstaluj. Ustawienia i logi zostają w
`%LOCALAPPDATA%\Bieznia` - usuń ten folder, jeśli chcesz pozbyć się wszystkiego.

## Prywatność

Aplikacja nie ma kont, reklam ani analityki. Dane z bieżni i logi zostają na komputerze.
Z internetem łączy się tylko, żeby sprawdzić i pobrać aktualizację z GitHuba, oraz gdy sam
wyślesz zgłoszenie. Szczegóły: [Polityka prywatności](POLITYKA-PRYWATNOSCI.md).

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
- `bieznia/gui/` - okno PySide6: tokeny i motywy, kontrolki, widoki
- `bieznia/aktualizacje.py` - aktualizacja z GitHub Releases (SHA-256, tylko hosty GitHuba)
- `bieznia/zgloszenia.py` - paczka z logami i wysyłka zgłoszenia
- `packaging/` - ikona, specyfikacja PyInstaller, instalator Inno Setup, skrypt budowania

### Zgłoszenia prosto do autora

Domyślnie **Zgłoś problem** przygotowuje e-mail na kontakt@damnit.com.pl z paczką logów.
Żeby zgłoszenia przychodziły automatycznie, ustaw w `config.toml` sekcję `[zgloszenia]`
`adres` na adres HTTPS, który przyjmuje POST `multipart/form-data` z polami `wersja`, `opis`,
`kontakt` i plikiem `plik` (ZIP, do 8 MB).

### Jak działa połączenie z X21

X21 mówi własnym, szyfrowanym protokołem KingSmith (opisanym w otwartym sterowniku
[QZ / qdomyos-zwift](https://github.com/cagnulein/qdomyos-zwift)): usługa `0x1234`, zapis do
`FED7`, powiadomienia z `FED8`, tekst w base64 z tablicą podstawień. Bieżnie różnią się
tablicą (7 wariantów) - aplikacja wykrywa ją z odpowiedzi bieżni i nigdy nie podaje prędkości
z niepewnie odszyfrowanej ramki. Prędkość jest odpytywana co 0,75 s, bo przy stałym tempie
bieżnia sama nic nie wysyła. Zapasowo obsługiwany jest standard FTMS.

## Bezpieczeństwo

- aktualizacja tylko po kliknięciu, tylko z wydań tego repozytorium i tylko z hostów GitHuba;
  instalator jest uruchamiany dopiero po zgodności sumy SHA-256 podanej przez GitHub,
- wydania budują się w GitHub Actions z przypiętych wersji (akcje po commitach, Inno Setup
  po sumie SHA-256, biblioteki z `uv.lock` z sumami) i mają poświadczenie pochodzenia
  (`gh attestation verify Bieznia-Setup-….exe --repo Damianejroo/bieznia-olek`),
- instalacja i działanie bez administratora; aplikacja tylko czyta bieżnię, startuje
  z wyłączonym sterowaniem i puszcza W/Shift po utracie danych, awarii albo zamknięciu,
- skróty F6-F8 są sprawdzane globalnie, ale program nie czyta ani nie zapisuje innych
  klawiszy.

## Licencja

© 2026 damnit.com.pl Damian Skrzypiec. Licencja GPL-3.0-or-later - zobacz [LICENSE](LICENSE).
Protokół KingSmith odtworzony na podstawie sterownika z projektu
[QZ (qdomyos-zwift)](https://github.com/cagnulein/qdomyos-zwift), również na licencji GPL-3.0.
Ikona przygotowana z pomocą generatora obrazów GPT.
