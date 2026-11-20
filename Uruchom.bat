@echo off
rem Bieznia - uruchomienie bez instalacji. Wystarczy kliknac dwa razy.
rem Pierwszy raz pobiera narzedzia do .narzedzia\ (tylko ten folder, nic w systemie).
rem Potem sam aktualizuje sie z GitHuba i restartuje aplikacje.
rem
rem Aktualizacja moze podmienic ten plik w trakcie dzialania. Dlatego wszystko, co
rem dzieje sie po powrocie z nadzorcy, siedzi w jednym bloku w nawiasach (cmd wczytuje
rem go w calosci przed wykonaniem), a petla to "goto", ktore szuka etykiety od nowa.
rem Nie uzywaj tu "for /l" jako petli - "exit /b" jej nie przerywa i cmd sie zawiesza.
setlocal
cd /d "%~dp0"
set "TOOLS=%~dp0.narzedzia"

rem BIEZNIA_SYSTEM_TOOLS=1 - uzyj git i uv z systemu (programista, CI)
if defined BIEZNIA_SYSTEM_TOOLS goto gotowe
if not exist "%TOOLS%\uv\uv.exe" goto przygotuj
if not exist "%TOOLS%\git\cmd\git.exe" goto przygotuj
if not exist ".git" goto przygotuj
goto narzedzia

:przygotuj
echo Pierwsze uruchomienie: pobieram narzedzia (ok. 55 MB, tylko raz)...
"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "%~dp0narzedzia\bootstrap.ps1"
if errorlevel 1 (
  pause
  exit /b 1
)
echo Pierwszy start aplikacji pobierze jeszcze Pythona i biblioteki (ok. 150 MB, tylko raz).

:narzedzia
set "PATH=%TOOLS%\uv;%TOOLS%\git\cmd;%PATH%"
set "UV_PYTHON_INSTALL_DIR=%TOOLS%\python"
set "UV_CACHE_DIR=%TOOLS%\cache"
set "UV_PYTHON_PREFERENCE=only-managed"

:gotowe
where git >nul 2>nul || (
  echo [BLAD] Nie znaleziono programu git.
  pause
  exit /b 1
)
where uv >nul 2>nul || (
  echo [BLAD] Nie znaleziono programu uv.
  pause
  exit /b 1
)
set "BIEZNIA_RESUME="
set "GIT_TERMINAL_PROMPT=0"

:petla
(
  uv run --locked --no-dev python -m bieznia.supervisor %*
  if errorlevel 3 if not errorlevel 4 (
    set "BIEZNIA_RESUME=1"
    goto petla
  )
  if errorlevel 1 (
    echo.
    echo [BLAD] Aplikacja nie dziala. Za 30 s pobiore poprawke z GitHuba i sprobuje ponownie.
    echo        Zamknij okno, zeby przerwac.
    rem Pelna sciezka: "timeout" z Git Basha w PATH to inny program. ping dziala tez bez konsoli.
    "%SystemRoot%\System32\PING.EXE" -n 31 127.0.0.1 >nul
    git -c credential.helper= -c http.lowSpeedLimit=1000 -c http.lowSpeedTime=20 pull --ff-only --quiet
    set "BIEZNIA_RESUME="
    goto petla
  )
  exit /b 0
)
