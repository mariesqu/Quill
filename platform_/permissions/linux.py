"""Linux dependency checks."""

from __future__ import annotations

import shutil


def check_xdotool() -> bool:
    """Return True if xdotool is available on PATH."""
    return shutil.which("xdotool") is not None


def check_xclip() -> bool:
    """Return True if xclip is available on PATH."""
    return shutil.which("xclip") is not None
