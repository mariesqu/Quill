"""Abstract base for AI provider backends."""

from __future__ import annotations

from abc import ABC, abstractmethod
from typing import AsyncIterator


class BaseProvider(ABC):
    def __init__(self, config: dict) -> None:
        self.config = config

    @abstractmethod
    async def stream(self, prompt: str, system: str = "") -> AsyncIterator[str]:
        """Yield streamed response text chunks."""
        ...

    @abstractmethod
    def is_available(self) -> bool:
        """Return True if this provider is configured and likely reachable."""
        ...
