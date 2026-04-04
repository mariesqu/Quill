"""macOS Accessibility permission check and prompt."""
from __future__ import annotations

import subprocess


def check_accessibility_permission() -> bool:
    """Return True if Quill (or the current process) has Accessibility API access."""
    try:
        result = subprocess.run(
            ["osascript", "-e",
             'tell application "System Events" to return name of first process'],
            capture_output=True,
            timeout=2
        )
        return result.returncode == 0
    except Exception:
        return False


def open_accessibility_settings() -> None:
    """Open the Accessibility pane in System Settings (works on macOS 13+)."""
    subprocess.run([
        "open",
        "x-apple.systempreferences:"
        "com.apple.preference.security?Privacy_Accessibility"
    ])
