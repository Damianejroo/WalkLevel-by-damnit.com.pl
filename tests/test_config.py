from pathlib import Path

from bieznia.app import apply_overrides, parse_args
from bieznia.config import load_config
from bieznia.mapping import MappingConfig

ROOT = Path(__file__).resolve().parents[1]


def test_repo_config_is_valid() -> None:
    config = load_config(ROOT)
    MappingConfig.from_dict(config["mapping"])
    assert config["output"]["type"] == "dry"  # domyślnie bezpiecznie: nic nie naciska
    assert config["output"]["start_enabled"] is False


def test_local_config_overrides_defaults(tmp_path: Path) -> None:
    (tmp_path / "config.toml").write_text(
        '[output]\ntype = "dry"\nstart_enabled = false\n', encoding="utf-8"
    )
    (tmp_path / "config.local.toml").write_text('[output]\ntype = "keys"\n', encoding="utf-8")
    config = load_config(tmp_path)
    assert config["output"] == {"type": "keys", "start_enabled": False}


def test_cli_overrides() -> None:
    config = load_config(ROOT)
    args = parse_args(["--replay", "x.jsonl", "--output", "keys", "--enabled"])
    config = apply_overrides(config, args)
    assert config["source"] == {"type": "replay", "replay_file": "x.jsonl"}
    assert config["output"]["type"] == "keys"
    assert config["output"]["start_enabled"] is True
