# Bieżnia → gra

Aplikacja, która zamienia chodzenie po bieżni **WalkingPad X21** na sterowanie w grach na
Windows: chód to **W**, bieg to **Shift + W**. Łączy się z bieżnią przez Bluetooth, sama
ją wyszukuje i sama się aktualizuje.

## Dla testera: uruchomienie bez instalacji

1. Pobierz paczkę: **[bieznia-olek-main.zip](https://github.com/Damianejroo/bieznia-olek/archive/refs/heads/main.zip)**
2. Rozpakuj ją (prawy przycisk > Wyodrębnij wszystkie) do zwykłego folderu, np. Dokumenty.
   Nie do OneDrive i nie do Program Files.
3. Kliknij dwa razy **`Uruchom.bat`**. Jeśli Windows zapyta o niezweryfikowanego wydawcę,
   wybierz Uruchom (to zwykły skrypt z tej paczki).

Pierwsze uruchomienie pobiera narzędzia, Pythona i biblioteki (ok. 200 MB, jednorazowo) do
ukrytego folderu `.narzedzia` obok aplikacji. Nic nie jest instalowane w systemie i nie trzeba
uprawnień administratora. Usunięcie folderu usuwa wszystko.

Potem w ustawieniach aplikacji możesz kliknąć **Utwórz skrót na pulpicie**.

### Zanim połączysz bieżnię

- włącz bieżnię,
- rozłącz telefon: zamknij KS Fit albo wyłącz w nim Bluetooth (bieżnia przyjmuje jedno
  połączenie naraz),
- włącz Bluetooth w Windows (Start > Ustawienia > Bluetooth i urządzenia).

Aplikacja przy pierwszym starcie sama szuka bieżni. Jeśli znajdzie jedną, łączy się od razu
i zapamiętuje ją na przyszłość. Jeśli kilka - pokazuje listę do wyboru.

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

### Gdy coś nie działa

Kliknij ikonę **Dziennik** (na dole okna) > **Zapisz raport diagnostyczny** i wyślij plik
z folderu `logs` (raport `diagnostyka-…txt` oraz `bluetooth.log`). Raport zawiera listę
urządzeń Bluetooth w pobliżu i surową rozmowę z bieżnią - to wystarczy do poprawki.

Aktualizacje przychodzą same, zwykle kilka sekund po wysłaniu poprawki. Aplikacja na chwilę
się zamknie i otworzy ponownie. Jeśli nowa wersja się wysypie, okno konsoli co 30 s próbuje
pobrać poprawkę.

### Gry

- Jeśli gra działa jako administrator, Windows nie pozwoli zwykłej aplikacji naciskać w niej
  klawiszy. Wtedy daj znać, zamiast uruchamiać wszystko jako administrator.
- Gry z ochroną przed oszustwami (np. sieciowe strzelanki) mogą traktować sztucznie naciskane
  klawisze jak automatyzację. Używaj w grach, które na to pozwalają, na własną
  odpowiedzialność.

## Dla programisty

```powershell
uv sync                                   # środowisko z narzędziami
uv run python -m bieznia                  # okno
uv run python -m bieznia --headless       # bez okna (symulator, logi w konsoli)
uv run python -m bieznia --diagnostyka    # tylko raport Bluetooth
uv run pytest; uv run ruff check; uv run ruff format; uv run mypy
```

Struktura:

- `bieznia/ble/` - Bluetooth: protokół KingSmith (`kingsmith.py`), FTMS, sterowniki,
  wątek połączenia z ponawianiem, raport diagnostyczny
- `bieznia/engine.py` - źródło > mapowanie > wyjście, bezpieczniki, liczniki
- `bieznia/mapping.py` - prędkość > W / Shift / gałka (wygładzanie, histereza)
- `bieznia/gui/` - okno PySide6: tokeny i motywy (`theme.py`), kontrolki, widoki
- `bieznia/supervisor.py` + `updater.py` - nadzorca z auto-aktualizacją u testera
- `Uruchom.bat` + `narzedzia/bootstrap.ps1` - start bez instalacji

### Jak działa połączenie z X21

X21 mówi własnym, szyfrowanym protokołem KingSmith (opisanym w otwartym sterowniku
[QZ / qdomyos-zwift](https://github.com/cagnulein/qdomyos-zwift)): usługa `0x1234`, zapis do
`FED7`, powiadomienia z `FED8`, tekst w base64 z tablicą podstawień. Bieżnie różnią się
tablicą (7 wariantów) - aplikacja wykrywa ją z odpowiedzi bieżni i nigdy nie podaje prędkości
z niepewnie odszyfrowanej ramki. Prędkość jest odpytywana co 0,75 s, bo przy stałym tempie
bieżnia sama nic nie wysyła. Zapasowo obsługiwany jest standard FTMS.

### Aktualizacje u testera

Każdy push na `main` trafia do testera w ok. 10 s, więc **na `main` wypychaj tylko kod,
który się uruchamia**. Eksperymenty rób na innych gałęziach. Force-push na `main` jest
zablokowany regułą repozytorium.

| Zmienione pliki | Efekt u testera |
| --- | --- |
| `bieznia/` (poza nadzorcą), `config.toml` | restart aplikacji (stan F8 zachowany) |
| kod nadzorcy, `pyproject.toml`, `uv.lock`, `.python-version` | nadzorca od nowa, `uv` synchronizuje środowisko |
| README, testy, `.github/`, `Uruchom.bat` | nic - zadziała przy następnym starcie |

## Bezpieczeństwo

Automatyczna aktualizacja oznacza, że **kod z gałęzi `main` uruchamia się na komputerze
testera**. Dlatego:

- pisać do repo może tylko właściciel (konto z 2FA), force-push i usunięcie `main` są
  zablokowane, cudze pull requesty nic nie zmieniają, dopóki nie zostaną zmergowane,
- nadzorca pobiera tylko gałąź `main` i robi wyłącznie fast-forward: nie kasuje lokalnych
  zmian, nie stosuje przepisanej historii, nie pyta o hasła, przy zerwanym połączeniu sam
  przerywa pobieranie,
- narzędzia pobierane przy pierwszym starcie (uv, MinGit) mają przypięte wersje i sumy
  SHA-256, Python pochodzi z uv (też ze sprawdzaniem sum), biblioteki są przypięte
  w `uv.lock` z sumami kontrolnymi; projekt niczego nie buduje,
- CI ma tylko prawo odczytu, bez sekretów, akcje przypięte do konkretnych commitów,
- aplikacja działa bez administratora, tylko czyta bieżnię, startuje z wyłączonym
  sterowaniem i puszcza W/Shift po utracie danych, awarii albo zamknięciu (robi to też
  nadzorca, który w razie potrzeby zabija cały proces razem z dziećmi),
- skróty F6-F8 są sprawdzane globalnie, ale program nie czyta ani nie zapisuje innych
  klawiszy.

## Licencja

GPL-3.0-or-later - zobacz [LICENSE](LICENSE). Protokół KingSmith odtworzony na podstawie
sterownika z projektu [QZ (qdomyos-zwift)](https://github.com/cagnulein/qdomyos-zwift),
również na licencji GPL-3.0.
