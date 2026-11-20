@echo off
rem Start mostu z automatyczna aktualizacja z GitHuba.
rem Argumenty sa przekazywane do mostu, np.:  start.bat --output keys
rem
rem Aktualizacja moze podmienic ten plik w trakcie dzialania. Dlatego wszystko, co
rem dzieje sie po powrocie z nadzorcy, siedzi w jednym bloku w nawiasach (cmd wczytuje
rem go w calosci przed wykonaniem), a petla to "goto", ktore szuka etykiety od nowa.
rem Nie uzywaj tu "for /l" jako petli - "exit /b" jej nie przerywa i cmd sie zawiesza.
setlocal
cd /d "%~dp0"
set "BIEZNIA_RESUME="
set "GIT_TERMINAL_PROMPT=0"
where git >nul 2>nul || (
  echo [BLAD] Nie znaleziono programu git. Instrukcja instalacji: README.md
  pause
  exit /b 1
)
where uv >nul 2>nul || (
  echo [BLAD] Nie znaleziono programu uv. Instrukcja instalacji: README.md
  pause
  exit /b 1
)
if not exist ".git" (
  echo [BLAD] Ten folder nie jest klonem repozytorium. Pobierz go przez git clone, nie jako ZIP.
  pause
  exit /b 1
)

:petla
(
  uv run --locked --no-dev python -m bieznia.supervisor %*
  if errorlevel 3 if not errorlevel 4 (
    set "BIEZNIA_RESUME=1"
    goto petla
  )
  if errorlevel 1 (
    echo.
    echo [BLAD] Nadzorca nie dziala. Za 30 s pobiore poprawke z GitHuba i sprobuje ponownie.
    echo        Zamknij okno, zeby przerwac.
    rem Pelna sciezka: "timeout" z Git Basha w PATH to inny program. ping dziala tez bez konsoli.
    "%SystemRoot%\System32\PING.EXE" -n 31 127.0.0.1 >nul
    git pull --ff-only --quiet
    set "BIEZNIA_RESUME="
    goto petla
  )
  exit /b 0
)
