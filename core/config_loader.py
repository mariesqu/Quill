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
_USER_CONFIG = _ROOT / "config" / "user.yaml"
_MODES_CONFIG = _ROOT / "config" / "modes.yaml"


def _deep_merge(base: dict, override: dict) -> dict:
    """Recursively merge override into base."""
    result = base.copy()
    for k, v in override.items():
        if k in result and isinstance(result[k], dict) and isinstance(v, dict):
            result[k] = _deep_merge(result[k], v)
        else:
            result[k] = v
    return result


def load_config() -> dict[str, Any]:
    """Load and merge configuration. Never returns api_key in logs."""
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
        if platform.system() == "Darwin":
            config["hotkey"] = "cmd+shift+space"
        else:
            config["hotkey"] = "ctrl+shift+space"

    return config


def load_modes() -> dict[str, Any]:
    """Load built-in + user-defined modes."""
    with open(_MODES_CONFIG) as f:
        data = yaml.safe_load(f) or {}
    modes = data.get("modes", {})

    # Merge custom modes from user.yaml if present
    if _USER_CONFIG.exists():
        with open(_USER_CONFIG) as f:
            user = yaml.safe_load(f) or {}
        custom = user.get("custom_modes", {})
        modes.update(custom)

    return modes


def save_user_config(updates: dict[str, Any]) -> None:
    """Persist user-specific settings to config/user.yaml."""
    existing: dict = {}
    if _USER_CONFIG.exists():
        with open(_USER_CONFIG) as f:
            existing = yaml.safe_load(f) or {}

    merged = _deep_merge(existing, updates)

    _USER_CONFIG.parent.mkdir(parents=True, exist_ok=True)
    with open(_USER_CONFIG, "w") as f:
        yaml.dump(merged, f, default_flow_style=False, allow_unicode=True)
