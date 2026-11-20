"""Bezpieczna aktualizacja z GitHuba: tylko fast-forward, nigdy nie nadpisuje lokalnych zmian."""

from __future__ import annotations

import os
import subprocess
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path

GIT_TIMEOUT_S = 60


class Status(Enum):
    UP_TO_DATE = "up_to_date"
    UPDATED = "updated"
    SKIPPED = "skipped"  # celowo nie aktualizujemy (lokalne zmiany, inna gałąź, ...)
    ERROR = "error"  # np. brak internetu


@dataclass
class UpdateResult:
    status: Status
    message: str = ""
    old: str = ""
    new: str = ""
    changed: list[str] = field(default_factory=list)


class GitError(RuntimeError):
    pass


class Git:
    def __init__(self, repo: Path) -> None:
        self.repo = repo
        # Nigdy nie pytaj o hasło w konsoli – fetch ma się udać albo szybko polec.
        self.env = {**os.environ, "GIT_TERMINAL_PROMPT": "0", "GCM_INTERACTIVE": "never"}

    def __call__(self, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
        proc = subprocess.run(
            ["git", *args],
            cwd=self.repo,
            env=self.env,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=GIT_TIMEOUT_S,
            check=False,
        )
        if check and proc.returncode != 0:
            raise GitError(f"git {' '.join(args)}: {proc.stderr.strip() or proc.stdout.strip()}")
        return proc

    def out(self, *args: str) -> str:
        return self(*args).stdout.strip()


def check_and_update(git: Git, remote: str = "origin", branch: str = "main") -> UpdateResult:
    """Pobiera remote/branch i przesuwa lokalną gałąź, jeśli to czysty fast-forward."""
    try:
        current = git.out("rev-parse", "--abbrev-ref", "HEAD")
        if current != branch:
            return UpdateResult(Status.SKIPPED, f"jesteś na gałęzi {current!r}, śledzę {branch!r}")

        git("fetch", "--quiet", "--no-tags", remote, branch)
        old = git.out("rev-parse", "HEAD")
        new = git.out("rev-parse", "FETCH_HEAD")
        if old == new:
            return UpdateResult(Status.UP_TO_DATE, old=old, new=new)

        if git("merge-base", "--is-ancestor", old, new, check=False).returncode != 0:
            return UpdateResult(
                Status.SKIPPED,
                "lokalna historia rozjechała się z GitHubem (nowe lokalne commity albo force-push)"
                " – nie aktualizuję, żeby niczego nie skasować",
                old=old,
                new=new,
            )

        if git.out("status", "--porcelain", "--untracked-files=no"):
            return UpdateResult(
                Status.SKIPPED,
                "są lokalne zmiany w plikach z repo – cofnij je (git restore .), żeby wrócić"
                " do aktualizacji",
                old=old,
                new=new,
            )

        git("merge", "--ff-only", "--quiet", new)
        changed = git.out("diff", "--name-only", old, new).splitlines()
        return UpdateResult(
            Status.UPDATED, git.out("log", "-1", "--format=%h %s"), old, new, changed
        )
    except (GitError, subprocess.TimeoutExpired, OSError) as exc:
        return UpdateResult(Status.ERROR, str(exc))
