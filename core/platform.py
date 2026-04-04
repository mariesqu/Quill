"""
Platform backend loader.
core/ must NEVER import OS-specific modules directly.
Always use get_backends() from this module.
"""
from __future__ import annotations

import platform as _platform
import sys
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from platform_.capture.base import CaptureBackend
    from platform_.context.base import ContextBackend
    from platform_.hotkey.base import HotkeyBackend
    from platform_.replace.base import ReplaceBackend


def get_os() -> str:
    """Return 'Windows', 'Darwin', or 'Linux'."""
    return _platform.system()


def get_backends() -> dict:
    """
    Load and return the correct backend implementations for the current OS.
    Raises PermissionError("accessibility") on macOS if Accessibility is not granted.
    """
    os_name = get_os()

    if os_name == "Windows":
        from platform_.capture.windows import WindowsCapture
        from platform_.context.windows import WindowsContext
        from platform_.hotkey.windows import WindowsHotkey
        from platform_.replace.windows import WindowsReplace
        return {
            "capture": WindowsCapture(),
            "context": WindowsContext(),
            "hotkey": WindowsHotkey(),
            "replace": WindowsReplace(),
        }

    elif os_name == "Darwin":
        from platform_.permissions.macos import check_accessibility_permission
        if not check_accessibility_permission():
            raise PermissionError("accessibility")

        from platform_.capture.macos import MacOSCapture
        from platform_.context.macos import MacOSContext
        from platform_.hotkey.macos import MacOSHotkey
        from platform_.replace.macos import MacOSReplace
        return {
            "capture": MacOSCapture(),
            "context": MacOSContext(),
            "hotkey": MacOSHotkey(),
            "replace": MacOSReplace(),
        }

    elif os_name == "Linux":
        from platform_.permissions.linux import check_xdotool
        if not check_xdotool():
            import logging
            logging.warning(
                "xdotool is not installed. Text capture will fall back to clipboard only. "
                "Install with: sudo apt install xdotool"
            )

        from platform_.capture.linux import LinuxCapture
        from platform_.context.linux import LinuxContext
        from platform_.hotkey.linux import LinuxHotkey
        from platform_.replace.linux import LinuxReplace
        return {
            "capture": LinuxCapture(),
            "context": LinuxContext(),
            "hotkey": LinuxHotkey(),
            "replace": LinuxReplace(),
        }

    else:
        raise RuntimeError(f"Unsupported OS: {os_name}")
