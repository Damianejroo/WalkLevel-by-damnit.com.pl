# Bieżnia → gra

Most, który zamienia chodzenie po bieżni **WalkingPad X21** na sterowanie w grach na Windows:
chód to **W**, bieg to **Shift+W** (później także analogowa gałka pada).

> **Etap:** szkielet. Działa symulator bieżni i odtwarzanie nagrań, prawdziwa bieżnia
> (Bluetooth / QZ) dojdzie w kolejnych krokach.

## Dla testera (Olek): instalacja raz na zawsze

1. Otwórz **PowerShell** (zwykły, nie jako administrator) i zainstaluj dwa programy
   z oficjalnego źródła Microsoftu (winget):

   ```powershell
   winget install --id Git.Git -e
   winget install --id astral-sh.uv -e
   ```

   Zamknij i otwórz PowerShell ponownie, żeby je widział.

2. Pobierz repozytorium (najlepiej do folderu użytkownika, **nie** do OneDrive):

   ```powershell
   cd $HOME
   git clone https://github.com/Damianejroo/bieznia-olek.git
   ```

3. Wejdź do folderu `bieznia-olek` i kliknij dwa razy **`start.bat`**.
   Za pierwszym razem `uv` sam pobierze Pythona – to chwilę potrwa.

Od teraz **zawsze uruchamiasz tylko `start.bat`**. Sam pobiera nowe wersje z GitHuba
(co ok. 10 s) i restartuje most. Nic nie musisz aktualizować ręcznie.

### Obsługa

| Klawisz | Co robi |
| --- | --- |
| **F8** | włącza / wyłącza most (startuje **wyłączony**) |
| F6 / F7 | tylko w symulatorze: wolniej / szybciej |
| Ctrl+C w oknie konsoli | zamyka wszystko |

Domyślnie most pracuje „na sucho” (`dry`): tylko pisze w konsoli, co by nacisnął.
Żeby naprawdę naciskał klawisze w grze:

```powershell
.\start.bat --output keys
```

Inne przydatne opcje: `--record` (nagrywa odczyty do folderu `nagrania/`), `--replay PLIK`
(odtwarza nagranie), `--enabled` (startuje od razu włączony).

Stałe zmiany ustawień: utwórz obok plik `config.local.toml` i wpisz tylko to, co chcesz zmienić,
np.:

```toml
[output]
type = "keys"
```

**Nie edytuj plików z repo** – przy lokalnych zmianach aktualizacja się wstrzyma
(i powie o tym w konsoli). Logi są w folderze `logs/`, możesz je podesłać przy problemach.

### Gdy gra nie reaguje na klawisze

Windows nie pozwala zwykłemu programowi naciskać klawiszy w programie uruchomionym jako
administrator. Jeśli gra (albo jej launcher) działa jako admin, most też musiałby – wtedy
daj znać, zamiast od razu uruchamiać go jako administrator.

## Dla programisty

```powershell
uv sync                         # środowisko z narzędziami dev
uv run python -m bieznia        # most z symulatorem (F6/F7/F8)
uv run pytest                   # testy
uv run ruff check; uv run ruff format
```

Struktura:

- `src/bieznia/mapping.py` – prędkość → W / Shift / gałka (wygładzanie, histereza)
- `src/bieznia/sources/` – źródła odczytów: `simulator`, `replay` (tu dojdzie bieżnia)
- `src/bieznia/outputs/` – wyjścia: `dry`, `keyboard` (tu dojdzie pad)
- `src/bieznia/supervisor.py` + `updater.py` – nadzorca z auto-aktualizacją u testera
- `start.bat` – punkt startu u testera

Każdy push na `main` trafia do testera w ciągu ok. 10 s, więc **na `main` wypychaj tylko kod,
który się uruchamia**. Eksperymenty rób na innych gałęziach. Nie rób force-pusha na `main`:
nadzorca go nie zastosuje (celowo) i tester zostanie na starej wersji.

Zmiana wersji Pythona (`.python-version`) wymaga, żeby tester zamknął i uruchomił `start.bat`
ponownie.

## Bezpieczeństwo

Automatyczna aktualizacja oznacza, że **kod z gałęzi `main` tego repo uruchamia się na komputerze
testera**. Dlatego:

- pisać do repo może tylko właściciel (konto z włączonym 2FA); cudze pull requesty nic
  nie zmieniają, dopóki nie zostaną zmergowane,
- nadzorca robi wyłącznie **fast-forward** z `origin/main`: nie kasuje lokalnych zmian,
  nie wykonuje force-pusha, nie pyta o hasła,
- zależności są przypięte w `uv.lock` (z sumami kontrolnymi), instalowane przez `uv --locked`;
  obecnie most nie ma żadnych zależności poza biblioteką standardową Pythona,
- most działa bez uprawnień administratora i **tylko czyta** dane z bieżni – nigdy nie zmienia
  jej prędkości,
- most startuje wyłączony, a po utracie danych z bieżni (2 s) albo po awarii puszcza W i Shift
  (robi to też nadzorca, gdyby most padł),
- skróty F6–F8 są sprawdzane globalnie, ale program nie czyta ani nie zapisuje innych klawiszy.

## Licencja

MIT – zobacz [LICENSE](LICENSE).
