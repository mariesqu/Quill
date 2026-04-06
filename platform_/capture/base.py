"""Abstract base for text capture backends."""

from __future__ import annotations

from abc import ABC, abstractmethod
from typing import Optional


class CaptureBackend(ABC):
    @abstractmethod
    def get_selected_text(self) -> Optional[str]:
        """
        Return currently selected text, or None if nothing is selected.
        Must never raise — catch all exceptions internally.
        Must restore clipboard state if it was modified.
        """
        ...
