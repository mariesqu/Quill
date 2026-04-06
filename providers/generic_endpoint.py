"""Generic OpenAI-compatible endpoint (LM Studio, Jan.ai, Groq, etc.)"""
from __future__ import annotations

from .generic import GenericOpenAIProvider


class GenericProvider(GenericOpenAIProvider):
    """Generic OpenAI-compatible endpoint provider (LM Studio, Jan.ai, Groq, etc.)."""

    def is_available(self) -> bool:
        return bool(self._base_url)
