"""
Generic OpenAI-compatible provider.
Used for OpenRouter, OpenAI, LM Studio, Jan.ai, Groq, etc.
"""
from __future__ import annotations

import logging
from typing import AsyncIterator, Any

import httpx

from .base import BaseProvider

log = logging.getLogger(__name__)


class GenericOpenAIProvider(BaseProvider):
    """Streams from any OpenAI-compatible /v1/chat/completions endpoint."""

    def __init__(self, config: dict, base_url: str | None = None) -> None:
        super().__init__(config)
        self._base_url = (
            base_url
            or config.get("base_url", "http://localhost:1234/v1")
        ).rstrip("/")
        self._api_key = config.get("api_key") or "none"
        self._model = config.get("model", "gpt-3.5-turbo")

    def is_available(self) -> bool:
        return bool(self._base_url)

    async def stream(self, prompt: str, system: str = "") -> AsyncIterator[str]:
        messages: list[dict[str, Any]] = []
        if system:
            messages.append({"role": "system", "content": system})
        messages.append({"role": "user", "content": prompt})

        headers = {
            "Authorization": f"Bearer {self._api_key}",
            "Content-Type": "application/json",
        }
        payload = {
            "model": self._model,
            "messages": messages,
            "stream": True,
        }

        async with httpx.AsyncClient(timeout=60) as client:
            async with client.stream(
                "POST",
                f"{self._base_url}/chat/completions",
                json=payload,
                headers=headers,
            ) as response:
                response.raise_for_status()
                async for line in response.aiter_lines():
                    if not line.startswith("data: "):
                        continue
                    data = line[6:]
                    if data.strip() == "[DONE]":
                        break
                    try:
                        import json
                        chunk = json.loads(data)
                        delta = chunk["choices"][0]["delta"]
                        content = delta.get("content", "")
                        if content:
                            yield content
                    except Exception as e:
                        log.debug("Failed to parse SSE chunk: %s — %s", data, e)
