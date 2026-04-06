"""macOS global hotkey backend using pynput GlobalHotKeys."""

from __future__ import annotations

import logging
from typing import Callable

from .base import HotkeyBackend

log = logging.getLogger(__name__)

_SPECIAL_KEYS = {
    "cmd",
    "ctrl",
    "alt",
    "shift",
    "space",
    "tab",
    "esc",
    "enter",
    "return",
    "backspace",
    "delete",
    "up",
    "down",
    "left",
    "right",
    "f1",
    "f2",
    "f3",
    "f4",
    "f5",
    "f6",
    "f7",
    "f8",
    "f9",
    "f10",
    "f11",
    "f12",
}


def _to_pynput(hotkey: str) -> str:
    """Convert 'cmd+shift+space' → '<cmd>+<shift>+<space>'."""
    parts = hotkey.lower().split("+")
    return "+".join(f"<{p}>" if p in _SPECIAL_KEYS else p for p in parts)


class MacOSHotkey(HotkeyBackend):
    def __init__(self) -> None:
        self._listener = None

    def register(self, hotkey: str, callback: Callable) -> None:
        from pynput import keyboard

        if self._listener:
            self._listener.stop()
        combo = _to_pynput(hotkey)
        log.info("Registering hotkey: %s → %s", hotkey, combo)
        self._listener = keyboard.GlobalHotKeys({combo: callback})
        self._listener.start()

    def unregister_all(self) -> None:
        if self._listener:
            self._listener.stop()
            self._listener = None
