"""Windows global hotkey backend using the `keyboard` library."""
from __future__ import annotations

import logging
from typing import Callable

from .base import HotkeyBackend

log = logging.getLogger(__name__)


class WindowsHotkey(HotkeyBackend):
    def __init__(self) -> None:
        self._registered: list[str] = []

    def register(self, hotkey: str, callback: Callable) -> None:
        import keyboard
        keyboard.add_hotkey(hotkey, callback)
        self._registered.append(hotkey)
        log.info("Registered hotkey: %s", hotkey)

    def unregister_all(self) -> None:
        import keyboard
        for hk in self._registered:
            try:
                keyboard.remove_hotkey(hk)
            except Exception:
                pass
        self._registered.clear()
