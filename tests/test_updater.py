"""Testy aktualizacji na prawdziwych, tymczasowych repozytoriach git."""

import subprocess
from pathlib import Path

import pytest

from bieznia.supervisor import needs_app_restart, own_files
from bieznia.updater import Git, Status, check_and_update

GIT_CFG = [
    "-c",
    "user.name=test",
    "-c",
    "user.email=test@example.com",
    "-c",
    "commit.gpgsign=false",
]


def git(repo: Path, *args: str) -> str:
    proc = subprocess.run(
        ["git", *GIT_CFG, *args], cwd=repo, capture_output=True, text=True, check=True
    )
    return proc.stdout.strip()


def commit(repo: Path, name: str, content: str) -> str:
    (repo / name).write_text(content, encoding="utf-8")
    git(repo, "add", name)
    git(repo, "commit", "-q", "-m", f"zmiana {name}")
    return git(repo, "rev-parse", "HEAD")


@pytest.fixture
def repos(tmp_path: Path) -> tuple[Path, Path]:
    """(dev, tester): dev wypycha do origin, tester to klon jak u Olka."""
    origin = tmp_path / "origin.git"
    dev = tmp_path / "dev"
    tester = tmp_path / "tester"
    subprocess.run(["git", "init", "-q", "--bare", "-b", "main", str(origin)], check=True)
    subprocess.run(["git", "clone", "-q", str(origin), str(dev)], check=True)
    git(dev, "checkout", "-q", "-b", "main")
    commit(dev, "app.py", "v1")
    git(dev, "push", "-q", "origin", "main")
    subprocess.run(["git", "clone", "-q", str(origin), str(tester)], check=True)
    return dev, tester


def test_up_to_date(repos: tuple[Path, Path]) -> None:
    _, tester = repos
    assert check_and_update(Git(tester)).status is Status.UP_TO_DATE


def test_fast_forward_update(repos: tuple[Path, Path]) -> None:
    dev, tester = repos
    new = commit(dev, "app.py", "v2")
    git(dev, "push", "-q", "origin", "main")

    result = check_and_update(Git(tester))

    assert result.status is Status.UPDATED
    assert result.new == new
    assert result.changed == ["app.py"]
    assert (tester / "app.py").read_text(encoding="utf-8") == "v2"


def test_local_changes_are_never_overwritten(repos: tuple[Path, Path]) -> None:
    dev, tester = repos
    commit(dev, "app.py", "v2")
    git(dev, "push", "-q", "origin", "main")
    (tester / "app.py").write_text("lokalna poprawka", encoding="utf-8")

    result = check_and_update(Git(tester))

    assert result.status is Status.SKIPPED
    assert (tester / "app.py").read_text(encoding="utf-8") == "lokalna poprawka"


def test_untracked_files_do_not_block_update(repos: tuple[Path, Path]) -> None:
    dev, tester = repos
    commit(dev, "app.py", "v2")
    git(dev, "push", "-q", "origin", "main")
    (tester / "config.local.toml").write_text("[output]\n", encoding="utf-8")

    assert check_and_update(Git(tester)).status is Status.UPDATED


def test_force_push_is_not_followed(repos: tuple[Path, Path]) -> None:
    dev, tester = repos
    git(dev, "commit", "-q", "--amend", "-m", "przepisana historia")
    git(dev, "push", "-q", "--force", "origin", "main")

    result = check_and_update(Git(tester))

    assert result.status is Status.SKIPPED
    assert (tester / "app.py").read_text(encoding="utf-8") == "v1"


def test_other_branch_is_skipped(repos: tuple[Path, Path]) -> None:
    _, tester = repos
    git(tester, "checkout", "-q", "-b", "eksperyment")
    assert check_and_update(Git(tester)).status is Status.SKIPPED


def test_unreachable_remote_is_an_error(repos: tuple[Path, Path], tmp_path: Path) -> None:
    _, tester = repos
    git(tester, "remote", "set-url", "origin", str(tmp_path / "nie-ma"))
    assert check_and_update(Git(tester)).status is Status.ERROR


@pytest.mark.parametrize(
    ("changed", "expected"),
    [
        (["bieznia/mapping.py"], True),
        (["config.toml"], True),
        (["README.md", "tests/test_mapping.py", ".github/workflows/ci.yml"], False),
        (["Uruchom.bat"], False),
    ],
)
def test_only_relevant_changes_restart_app(changed: list[str], expected: bool) -> None:
    assert needs_app_restart(changed) is expected


def test_supervisor_knows_its_own_files() -> None:
    root = Path(__file__).resolve().parents[1]
    files = own_files(root)
    assert "bieznia/supervisor.py" in files
    assert "bieznia/updater.py" in files
    assert "uv.lock" in files


def test_supervisor_sees_itself_when_run_as_main() -> None:
    # python -m bieznia.supervisor: moduł nadzorcy jest w sys.modules jako "__main__"
    import subprocess as sp
    import sys

    root = Path(__file__).resolve().parents[1]
    code = (
        "import runpy, sys; sys.argv=['x'];"
        "g = runpy.run_module('bieznia.supervisor', run_name='not_main', alter_sys=True);"
        "print(sorted(g['own_files'](__import__('pathlib').Path.cwd())))"
    )
    out = sp.run([sys.executable, "-c", code], cwd=root, capture_output=True, text=True, check=True)
    assert "bieznia/supervisor.py" in out.stdout
