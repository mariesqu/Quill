"""Abstract base for app context detection backends."""
from __future__ import annotations

from abc import ABC, abstractmethod


class ContextBackend(ABC):
    @abstractmethod
    def get_active_context(self) -> dict:
        """
        Return a dict with:
          app  (str): process name without extension, lowercase
          tone (str): neutral | professional | technical | casual | formal
          hint (str): human-readable e.g. 'email', 'code editor', 'browser'
        Must never raise.
        """
        ...

    @staticmethod
    def _fallback() -> dict:
        return {"app": "unknown", "tone": "neutral", "hint": "general"}
