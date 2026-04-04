"""OpenAI direct provider."""
from __future__ import annotations

from .generic import GenericOpenAIProvider

OPENAI_BASE = "https://api.openai.com/v1"
DEFAULT_MODEL = "gpt-4o-mini"


class OpenAIProvider(GenericOpenAIProvider):
    def __init__(self, config: dict) -> None:
        super().__init__(
            config,
            base_url=config.get("base_url", OPENAI_BASE),
        )
        self._model = config.get("model", DEFAULT_MODEL)

    def is_available(self) -> bool:
        return bool(self.config.get("api_key"))
