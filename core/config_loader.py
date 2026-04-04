"""
Configuration loader for Quill.
Priority: env vars → config/user.yaml → config/default.yaml
"""
from __future__ import annotations

import os
import platform
from pathlib import Path
from typing import Any

import yaml

_ROOT = Path(__file__).parent.parent
_DEFAULT_CONFIG = _ROOT / "config" / "default.yaml"
_USER_CONFIG    = _ROOT / "config" / "user.yaml"
_MODES_CONFIG   = _ROOT / "config" / "modes.yaml"


def _deep_merge(base: dict, override: dict) -> dict:
    result = base.copy()
    for k, v in override.items():
        if k in result and isinstance(result[k], dict) and isinstance(v, dict):
            result[k] = _deep_merge(result[k], v)
        else:
            result[k] = v
    return result


def load_config() -> dict[str, Any]:
    with open(_DEFAULT_CONFIG) as f:
        config = yaml.safe_load(f) or {}

    if _USER_CONFIG.exists():
        with open(_USER_CONFIG) as f:
            user = yaml.safe_load(f) or {}
        config = _deep_merge(config, user)

    # Environment variable overrides
    if key := os.environ.get("QUILL_API_KEY"):
        config["api_key"] = key
    if provider := os.environ.get("QUILL_PROVIDER"):
        config["provider"] = provider
    if model := os.environ.get("QUILL_MODEL"):
        config["model"] = model
    if base_url := os.environ.get("QUILL_BASE_URL"):
        config["base_url"] = base_url

    # Resolve adaptive hotkey
    if config.get("hotkey") is None:
        config["hotkey"] = "cmd+shift+space" if platform.system() == "Darwin" else "ctrl+shift+space"

    return config


def load_modes() -> tuple[dict[str, Any], dict[str, Any]]:
    """
    Returns (modes, chains) dicts.
    Modes and chains from user.yaml are merged on top of defaults.
    """
    with open(_MODES_CONFIG) as f:
        data = yaml.safe_load(f) or {}

    modes  = data.get("modes",  {})
    chains = data.get("chains", {})

    if _USER_CONFIG.exists():
        with open(_USER_CONFIG) as f:
            user = yaml.safe_load(f) or {}
        modes.update(user.get("custom_modes",  {}))
        chains.update(user.get("custom_chains", {}))

    return modes, chains


def save_user_config(updates: dict[str, Any]) -> None:
    existing: dict = {}
    if _USER_CONFIG.exists():
        with open(_USER_CONFIG) as f:
            existing = yaml.safe_load(f) or {}

    merged = _deep_merge(existing, updates)
    _USER_CONFIG.parent.mkdir(parents=True, exist_ok=True)
    with open(_USER_CONFIG, "w") as f:
        yaml.dump(merged, f, default_flow_style=False, allow_unicode=True)
