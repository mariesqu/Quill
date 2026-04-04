"""Generic OpenAI-compatible endpoint (LM Studio, Jan.ai, Groq, etc.)"""
from __future__ import annotations

from .generic import GenericOpenAIProvider


class GenericProvider(GenericOpenAIProvider):
    """Alias for use with provider: generic in config."""
    pass
