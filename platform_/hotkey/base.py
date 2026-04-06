"""Abstract base for global hotkey backends."""

from __future__ import annotations

from abc import ABC, abstractmethod
from typing import Callable


class HotkeyBackend(ABC):
    @abstractmethod
    def register(self, hotkey: str, callback: Callable) -> None:
        """
        Register a global hotkey.
        hotkey format: 'ctrl+shift+space' or 'cmd+shift+space'
        callback: called with no arguments on each keypress (may be from another thread).
        """
        ...

    @abstractmethod
    def unregister_all(self) -> None:
        """Unregister all hotkeys and release resources."""
        ...
