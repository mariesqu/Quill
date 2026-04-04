"""Tests for core/config_loader.py"""
import os
import platform
import pytest
from pathlib import Path
import yaml

from core.config_loader import load_config, load_modes, _deep_merge


def test_deep_merge_basic():
    base = {"a": 1, "b": {"c": 2, "d": 3}}
    override = {"b": {"c": 99}, "e": 5}
    result = _deep_merge(base, override)
    assert result == {"a": 1, "b": {"c": 99, "d": 3}, "e": 5}


def test_load_config_returns_dict():
    config = load_config()
    assert isinstance(config, dict)
    assert "provider" in config
    assert "model" in config
    assert "hotkey" in config


def test_load_config_hotkey_resolved():
    config = load_config()
    # hotkey must be resolved (not None)
    assert config["hotkey"] is not None
    if platform.system() == "Darwin":
        assert config["hotkey"] == "cmd+shift+space"
    else:
        assert config["hotkey"] == "ctrl+shift+space"


def test_load_config_env_override(monkeypatch):
    monkeypatch.setenv("QUILL_PROVIDER", "ollama")
    monkeypatch.setenv("QUILL_MODEL", "gemma3:4b")
    config = load_config()
    assert config["provider"] == "ollama"
    assert config["model"] == "gemma3:4b"


def test_load_config_api_key_not_in_default(monkeypatch):
    # api_key must not be hardcoded in default.yaml
    monkeypatch.delenv("QUILL_API_KEY", raising=False)
    config = load_config()
    assert config.get("api_key") is None


def test_load_modes_returns_tuple():
    result = load_modes()
    assert isinstance(result, tuple)
    assert len(result) == 2
    modes, chains = result
    assert isinstance(modes, dict)
    assert isinstance(chains, dict)


def test_load_modes_has_required_keys():
    modes, _ = load_modes()
    required = {"rewrite", "translate", "coach"}
    assert required.issubset(set(modes.keys()))


def test_load_modes_each_has_prompt():
    modes, _ = load_modes()
    for name, cfg in modes.items():
        assert "prompt" in cfg, f"Mode '{name}' missing 'prompt'"
        assert "label" in cfg, f"Mode '{name}' missing 'label'"


def test_load_chains_has_builtin_chains():
    _, chains = load_modes()
    assert len(chains) > 0
    for chain_id, cfg in chains.items():
        assert "steps" in cfg, f"Chain '{chain_id}' missing 'steps'"
        assert isinstance(cfg["steps"], list)
        assert len(cfg["steps"]) >= 2
