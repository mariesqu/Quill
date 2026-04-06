"""Linux global hotkey backend using pynput GlobalHotKeys."""
from __future__ import annotations

import logging
from typing import Callable

from .base import HotkeyBackend
from .macos import _to_pynput  # same conversion logic

log = logging.getLogger(__name__)


class LinuxHotkey(HotkeyBackend):
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
