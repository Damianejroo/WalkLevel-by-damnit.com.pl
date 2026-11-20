"""Nadzorca u testera: uruchamia most, co kilka sekund sprawdza GitHuba,
po nowym commicie zatrzymuje most, aktualizuje kod i uruchamia go ponownie.

Uruchamiany w pętli przez start.bat:
    uv run --locked --no-dev python -m bieznia.supervisor [argumenty mostu]

Kody wyjścia: 0 = koniec (Ctrl+C), 3 = start.bat ma uruchomić nadzorcę od nowa
(zmienił się jego własny kod albo zależności - `uv run` je wtedy zsynchronizuje).
"""

from __future__ import annotations

import contextlib
import logging
import os
import re
import signal
import subprocess
import sys
import time
from pathlib import Path

from bieznia.config import load_config
from bieznia.logs import setup_logging
from bieznia.updater import Git, GitError, Status, UpdateResult, check_and_update
from bieznia.winapi import release_movement_keys

# Nadzorca celowo importuje mało modułów: zmiana któregokolwiek z nich wymaga
# jego przeładowania, zmiana reszty kodu - tylko restartu mostu.

log = logging.getLogger("nadzorca")

EXIT_RELAUNCH = 3
RESUME_ENV = "BIEZNIA_RESUME"
STOP_TIMEOUT_S = 5.0
HEALTHY_AFTER_S = 30.0
BACKOFF_START_S = 2.0
BACKOFF_MAX_S = 60.0
# Zmiana tych plików wymaga nowego środowiska (uv run) albo nowego nadzorcy.
RELAUNCH_FILES = {"pyproject.toml", "uv.lock", ".python-version"}
# Tylko zmiany tych ścieżek restartują most (README, testy, CI itp. - nie).
RESTART_PREFIXES = ("bieznia/", "config.toml")
SAFE_NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._/-]*$")


def own_files(root: Path) -> set[str]:
    """Pliki repo, z których zbudowany jest działający nadzorca (ich zmiana = przeładowanie)."""
    files = set(RELAUNCH_FILES)
    for name, module in list(sys.modules.items()):
        path = getattr(module, "__file__", None)
        if not (name == "bieznia" or name.startswith("bieznia.")) or not path:
            continue
        with contextlib.suppress(ValueError):  # moduł spoza repo
            files.add(Path(path).resolve().relative_to(root.resolve()).as_posix())
    return files


def kill_tree(proc: subprocess.Popen[bytes]) -> None:
    """Zabija proces razem z dziećmi.

    python.exe z .venv to tylko launcher - prawdziwy interpreter jest jego dzieckiem.
    Samo proc.kill() zostawiłoby osierocony most, który mógłby dalej trzymać W.
    """
    taskkill = Path(os.environ.get("SYSTEMROOT", r"C:\Windows")) / "System32" / "taskkill.exe"
    if sys.platform == "win32" and taskkill.exists():
        subprocess.run(
            [str(taskkill), "/T", "/F", "/PID", str(proc.pid)], capture_output=True, check=False
        )
    else:
        proc.kill()
    with contextlib.suppress(subprocess.TimeoutExpired):
        proc.wait(timeout=STOP_TIMEOUT_S)


def needs_app_restart(changed: list[str]) -> bool:
    return any(path.startswith(RESTART_PREFIXES) for path in changed)


class Supervisor:
    def __init__(self, root: Path, app_args: list[str]) -> None:
        self.root = root
        self.app_args = app_args
        update_cfg = load_config(root)["update"]
        self.remote = str(update_cfg["remote"])
        self.branch = str(update_cfg["branch"])
        for value in (self.remote, self.branch):
            if not SAFE_NAME.match(value):
                raise ValueError(f"Niedozwolona nazwa remote/gałęzi w [update]: {value!r}")
        self.interval = float(update_cfg["interval_s"])
        self.git = Git(root)
        self.child: subprocess.Popen[bytes] | None = None
        self.child_started = 0.0
        self.restart_at: float | None = 0.0
        self.resume_next = False
        self.backoff = BACKOFF_START_S
        self.last_message = ""
        self.finished = False

    # --- proces mostu -------------------------------------------------------

    def start_child(self) -> None:
        log.info("Uruchamiam aplikację…")
        cmd = [sys.executable, "-m", "bieznia", *self.app_args]
        env = dict(os.environ)
        if self.resume_next:
            env[RESUME_ENV] = "1"  # po aktualizacji most wraca do stanu włączony/wyłączony
        self.resume_next = False
        # Własna grupa procesów: możemy wysłać mu CTRL_BREAK bez zatrzymywania siebie.
        flags = getattr(subprocess, "CREATE_NEW_PROCESS_GROUP", 0)
        self.child = subprocess.Popen(cmd, cwd=self.root, env=env, creationflags=flags)
        self.child_started = time.monotonic()
        self.restart_at = None

    def stop_child(self) -> None:
        child, self.child = self.child, None
        if child is None or child.poll() is not None:
            return
        log.info("Zatrzymuję most…")
        try:
            if hasattr(signal, "CTRL_BREAK_EVENT"):
                os.kill(child.pid, signal.CTRL_BREAK_EVENT)
            else:
                child.terminate()
            child.wait(timeout=STOP_TIMEOUT_S)
        except (subprocess.TimeoutExpired, OSError):
            log.warning("Most nie zamknął się sam - zabijam proces")
            kill_tree(child)
            self.failsafe_release()

    @staticmethod
    def failsafe_release() -> None:
        # Most zginął bez sprzątania - puść za niego W i Shift.
        try:
            release_movement_keys()
        except OSError as exc:
            log.warning("Nie udało się puścić klawiszy: %s", exc)

    def watch_child(self, now: float) -> None:
        if self.child is None or (rc := self.child.poll()) is None:
            return
        self.child = None
        if rc == 0:
            # Użytkownik zamknął okno (albo skończyło się nagranie) - kończymy całość.
            log.info("Aplikacja zamknięta - kończę")
            self.finished = True
            return
        self.failsafe_release()
        if now - self.child_started > HEALTHY_AFTER_S:
            self.backoff = BACKOFF_START_S
        log.error("Most padł (kod %s). Restart za %.0f s albo po aktualizacji.", rc, self.backoff)
        self.restart_at = now + self.backoff
        self.backoff = min(BACKOFF_MAX_S, self.backoff * 2)

    # --- aktualizacje -------------------------------------------------------

    def report(self, result: UpdateResult) -> None:
        if result.status in (Status.SKIPPED, Status.ERROR):
            # Nie spamuj tym samym komunikatem co 10 s.
            if result.message != self.last_message:
                level = logging.WARNING if result.status is Status.SKIPPED else logging.ERROR
                log.log(level, "Aktualizacja: %s", result.message)
            self.last_message = result.message
        else:
            self.last_message = ""

    def apply_update(self, result: UpdateResult) -> bool:
        """Zwraca True, gdy start.bat ma uruchomić nadzorcę od nowa."""
        log.info("Nowa wersja: %s (plików: %d)", result.message, len(result.changed))
        if set(result.changed) & own_files(self.root):
            self.stop_child()
            return True
        if not needs_app_restart(result.changed):
            log.info("Zmiany nie dotyczą mostu - działa dalej bez restartu")
            return False
        was_running = self.child is not None
        self.stop_child()
        self.resume_next = was_running
        self.backoff = BACKOFF_START_S
        self.restart_at = 0.0
        return False

    def describe_version(self) -> str:
        try:
            return self.git.out("log", "-1", "--format=%h %s")
        except (GitError, subprocess.TimeoutExpired, OSError) as exc:
            return f"nieznana ({exc})"

    # --- pętla --------------------------------------------------------------

    def run(self) -> int:
        log.info(
            "Nadzorca: śledzę %s/%s co %.0f s, wersja %s",
            self.remote,
            self.branch,
            self.interval,
            self.describe_version(),
        )
        # Start po przeładowaniu nadzorcy (kod 3) traktujemy jak restart po aktualizacji.
        self.resume_next = os.environ.pop(RESUME_ENV, "") == "1"
        next_check = 0.0
        try:
            while True:
                now = time.monotonic()
                if now >= next_check:
                    next_check = now + self.interval
                    result = check_and_update(self.git, self.remote, self.branch)
                    self.report(result)
                    if result.status is Status.UPDATED and self.apply_update(result):
                        log.info("Zmienił się kod nadzorcy lub zależności - przeładowuję")
                        return EXIT_RELAUNCH
                self.watch_child(now)
                if self.finished:
                    return 0
                if self.child is None and self.restart_at is not None and now >= self.restart_at:
                    self.start_child()
                time.sleep(0.5)
        except KeyboardInterrupt:
            log.info("Ctrl+C - kończę")
            self.stop_child()
            return 0


def main() -> int:
    root = Path.cwd()
    setup_logging("nadzorca", root)
    return Supervisor(root, sys.argv[1:]).run()


if __name__ == "__main__":
    raise SystemExit(main())
