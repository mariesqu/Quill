"""OpenRouter provider (default). Free tier with many models."""

from __future__ import annotations

import json
import logging
from typing import AsyncIterator

import httpx

from .generic import GenericOpenAIProvider, _friendly_error

log = logging.getLogger(__name__)

OPENROUTER_BASE = "https://openrouter.ai/api/v1"
DEFAULT_MODEL = "google/gemma-3-27b-it"


class OpenRouterProvider(GenericOpenAIProvider):
    def __init__(self, config: dict) -> None:
        super().__init__(
            config,
            base_url=config.get("base_url", OPENROUTER_BASE),
        )
        self._model = config.get("model", DEFAULT_MODEL)

    def _chat_path(self) -> str:
        return "/chat/completions"

    def is_available(self) -> bool:
        return bool(self.config.get("api_key"))

    async def stream(self, prompt: str, system: str = "") -> AsyncIterator[str]:
        # OpenRouter requires HTTP-Referer for rate limiting attribution
        messages = []
        if system:
            messages.append({"role": "system", "content": system})
        messages.append({"role": "user", "content": prompt})

        headers = {
            "Authorization": f"Bearer {self._api_key}",
            "Content-Type": "application/json",
            "HTTP-Referer": "https://github.com/mariesqu/Quill",
            "X-Title": "Quill",
        }
        payload = {
            "model": self._model,
            "messages": messages,
            "stream": True,
        }

        async with httpx.AsyncClient(timeout=60) as client:
            async with client.stream(
                "POST",
                self._endpoint_url,
                json=payload,
                headers=headers,
            ) as response:
                if response.status_code >= 400:
                    body = await response.aread()
                    body_text = body.decode("utf-8", errors="replace")
                    log.error("OpenRouter API error %s: %s", response.status_code, body_text)
                    raise RuntimeError(
                        _friendly_error(response.status_code, body_text, self._model)
                    )

                async for line in response.aiter_lines():
                    if not line.startswith("data: "):
                        continue
                    data = line[6:]
                    if data.strip() == "[DONE]":
                        break
                    try:
                        chunk = json.loads(data)
                        choices = chunk.get("choices", [])
                        if not choices:
                            continue
                        delta = choices[0].get("delta", {})
                        content = delta.get("content", "")
                        if content:
                            yield content
                    except (json.JSONDecodeError, KeyError, IndexError, TypeError) as e:
                        log.debug("Failed to parse SSE chunk: %s — %s", data[:100], e)
