"""Nadzorca u testera: uruchamia most, co kilka sekund sprawdza GitHuba,
po nowym commicie zatrzymuje most, aktualizuje kod i uruchamia go ponownie.

Uruchamiany przez start.bat:
    uv run --locked --no-dev python -m bieznia.supervisor [argumenty mostu]
"""

from __future__ import annotations

import contextlib
import logging
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

from bieznia.config import load_config
from bieznia.logs import setup_logging
from bieznia.updater import Git, Status, UpdateResult, check_and_update
from bieznia.winapi import release_movement_keys

# Uwaga: nadzorca importuje celowo mało modułów. Zmiana któregokolwiek z nich
# wymusza przeładowanie nadzorcy (dodatkowy proces), zmiana reszty – tylko restart mostu.

log = logging.getLogger("nadzorca")

STOP_TIMEOUT_S = 5.0
HEALTHY_AFTER_S = 30.0
BACKOFF_START_S = 2.0
BACKOFF_MAX_S = 60.0
ALWAYS_RELAUNCH = {"pyproject.toml", "uv.lock", ".python-version"}


def own_files(root: Path) -> set[str]:
    """Pliki repo, z których zbudowany jest działający nadzorca (ich zmiana = przeładowanie)."""
    files = set(ALWAYS_RELAUNCH)
    for name, module in list(sys.modules.items()):
        path = getattr(module, "__file__", None)
        if not (name == "bieznia" or name.startswith("bieznia.")) or not path:
            continue
        with contextlib.suppress(ValueError):  # moduł spoza repo
            files.add(Path(path).resolve().relative_to(root.resolve()).as_posix())
    return files


class Supervisor:
    def __init__(self, root: Path, app_args: list[str]) -> None:
        self.root = root
        self.app_args = app_args
        update_cfg = load_config(root)["update"]
        self.remote = str(update_cfg["remote"])
        self.branch = str(update_cfg["branch"])
        self.interval = float(update_cfg["interval_s"])
        self.git = Git(root)
        self.child: subprocess.Popen[bytes] | None = None
        self.child_started = 0.0
        self.restart_at: float | None = 0.0
        self.backoff = BACKOFF_START_S
        self.last_message = ""

    # --- proces mostu -------------------------------------------------------

    def start_child(self) -> None:
        cmd = [sys.executable, "-m", "bieznia", *self.app_args]
        # Własna grupa procesów: możemy wysłać mu CTRL_BREAK bez zatrzymywania siebie.
        flags = getattr(subprocess, "CREATE_NEW_PROCESS_GROUP", 0)
        self.child = subprocess.Popen(cmd, cwd=self.root, creationflags=flags)
        self.child_started = time.monotonic()
        self.restart_at = None

    def stop_child(self) -> None:
        child, self.child = self.child, None
        if child is None:
            return
        if child.poll() is None:
            log.info("Zatrzymuję most…")
            try:
                if hasattr(signal, "CTRL_BREAK_EVENT"):
                    os.kill(child.pid, signal.CTRL_BREAK_EVENT)
                else:
                    child.terminate()
                child.wait(timeout=STOP_TIMEOUT_S)
            except (subprocess.TimeoutExpired, OSError):
                log.warning("Most nie zamknął się sam – zabijam proces")
                child.kill()
                child.wait()
        self.failsafe_release()

    @staticmethod
    def failsafe_release() -> None:
        # Gdyby most zginął w trakcie trzymania W/Shift, puść je za niego.
        try:
            release_movement_keys()
        except OSError as exc:
            log.warning("Nie udało się puścić klawiszy: %s", exc)

    def watch_child(self, now: float) -> None:
        if self.child is None or (rc := self.child.poll()) is None:
            return
        self.child = None
        self.failsafe_release()
        if rc == 0:
            log.info("Most zakończył pracę. Czekam na aktualizacje (Ctrl+C = wyjście).")
            return
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

    def sync_dependencies(self) -> None:
        cmd = ["uv", "sync", "--locked", "--no-dev", "--inexact", "--quiet"]
        proc = subprocess.run(cmd, cwd=self.root, check=False)
        if proc.returncode != 0:
            log.error(
                "uv sync nie powiódł się (kod %s) – zależności mogą być nieaktualne",
                proc.returncode,
            )

    def relaunch(self) -> int:
        log.info("Zmienił się kod nadzorcy – uruchamiam go ponownie")
        cmd = ["uv", "run", "--locked", "--no-dev", "python", "-m", "bieznia.supervisor"]
        proc = subprocess.Popen([*cmd, *self.app_args], cwd=self.root)
        while True:
            try:
                return proc.wait()
            except KeyboardInterrupt:
                continue  # Ctrl+C dostaje też nowy nadzorca – czekamy, aż sam się zamknie

    def apply_update(self, result: UpdateResult) -> bool:
        """Zwraca True, gdy trzeba przeładować samego nadzorcę."""
        log.info("Nowa wersja: %s (%d plików)", result.message, len(result.changed))
        if set(result.changed) & own_files(self.root):
            return True
        self.stop_child()
        if {"pyproject.toml", "uv.lock"} & set(result.changed):
            self.sync_dependencies()
        self.backoff = BACKOFF_START_S
        self.restart_at = 0.0
        return False

    # --- pętla --------------------------------------------------------------

    def run(self) -> int:
        log.info(
            "Nadzorca: śledzę %s/%s co %.0f s, wersja %s",
            self.remote,
            self.branch,
            self.interval,
            self.git.out("log", "-1", "--format=%h %s"),
        )
        next_check = 0.0
        try:
            while True:
                now = time.monotonic()
                if now >= next_check:
                    next_check = now + self.interval
                    result = check_and_update(self.git, self.remote, self.branch)
                    self.report(result)
                    if result.status is Status.UPDATED and self.apply_update(result):
                        self.stop_child()
                        return self.relaunch()
                self.watch_child(now)
                if self.child is None and self.restart_at is not None and now >= self.restart_at:
                    self.start_child()
                time.sleep(0.5)
        except KeyboardInterrupt:
            log.info("Ctrl+C – kończę")
            self.stop_child()
            return 0


def main() -> int:
    root = Path.cwd()
    setup_logging("nadzorca", root)
    return Supervisor(root, sys.argv[1:]).run()


if __name__ == "__main__":
    raise SystemExit(main())
