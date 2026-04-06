"""Abstract base for text replacement (paste-back) backends."""
from __future__ import annotations

from abc import ABC, abstractmethod


class ReplaceBackend(ABC):
    @abstractmethod
    def paste_text(self, text: str) -> bool:
        """
        Write text to clipboard, trigger the OS paste shortcut in the previously
        focused window, then restore the original clipboard contents.
        Must restore clipboard even if paste fails (use try/finally).
        Returns True on success, False on failure.
        """
        ...
