@echo off
rem Start mostu z automatyczna aktualizacja z GitHuba.
rem Argumenty sa przekazywane do mostu, np.:  start.bat --output keys
rem
rem Cala logika jest w jednym bloku w nawiasach: cmd.exe wczytuje go w calosci
rem przed wykonaniem, wiec aktualizacja moze bezpiecznie podmienic ten plik w trakcie.
setlocal
cd /d "%~dp0"
(
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
  uv run --locked --no-dev python -m bieznia.supervisor %*
  if errorlevel 1 pause
  exit /b
)
