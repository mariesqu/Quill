"""
Generic OpenAI-compatible provider.
Used for OpenRouter, OpenAI, LM Studio, Jan.ai, Groq, etc.
"""

from __future__ import annotations

import json
import logging
from typing import AsyncIterator, Any

import httpx

from .base import BaseProvider

log = logging.getLogger(__name__)


def _mask_key(key: str) -> str:
    """Mask API key for safe logging."""
    if not key or len(key) < 8:
        return "***"
    return f"{key[:4]}…{key[-4:]}"


def _friendly_error(status_code: int, body: str, model: str) -> str:
    """Return a human-readable error message from an API error response."""
    # Try to extract the API's own message first
    api_msg = ""
    try:
        data = json.loads(body)
        api_msg = data.get("error", {}).get("message") or data.get("message") or ""
    except Exception:
        api_msg = body.strip()

    if status_code == 400:
        detail = api_msg or "The request was rejected by the API."
        return f"Bad request (400): {detail}"
    if status_code == 401:
        return "Invalid API key (401). Check your key in Settings → AI Provider."
    if status_code == 402:
        return "Insufficient credits (402). Top up your account or switch to a free model."
    if status_code == 403:
        return f"Access denied (403). Your key may not have access to model '{model}'."
    if status_code == 404:
        return f"Model not found (404): '{model}'. Check the model name in Settings → AI Provider."
    if status_code == 429:
        return (
            "Rate limit reached (429). Wait a moment and try again, or switch to a different model."
        )
    if status_code >= 500:
        return (
            f"Provider server error ({status_code}). The API is having issues — try again shortly."
        )
    return f"API error {status_code}: {api_msg or 'Unknown error'}"


class GenericOpenAIProvider(BaseProvider):
    """Streams from any OpenAI-compatible /v1/chat/completions endpoint."""

    def __init__(self, config: dict, base_url: str | None = None) -> None:
        super().__init__(config)
        self._base_url = (
            base_url if base_url is not None else config.get("base_url", "http://localhost:1234/v1")
        ).rstrip("/")
        self._api_key = config.get("api_key") or ""
        self._model = config.get("model", "gpt-3.5-turbo")
        self._custom_headers = self._parse_custom_headers(config.get("custom_headers", ""))
        # Subclasses (OpenAI, OpenRouter) override _chat_path. For generic/custom
        # endpoint the user provides the full URL — we don't append anything.
        self._endpoint_url = f"{self._base_url}{self._chat_path()}"

    def _chat_path(self) -> str:
        """Path appended to base_url. Empty for custom endpoints (user gives full URL)."""
        return ""

    @staticmethod
    def _parse_custom_headers(raw) -> dict[str, str]:
        """Parse custom headers from either a dict or 'Header: value' string."""
        if isinstance(raw, dict):
            # Already a dict (from YAML mapping)
            return {str(k): str(v) for k, v in raw.items() if k}
        if not raw or not isinstance(raw, str):
            return {}
        headers: dict[str, str] = {}
        for line in raw.strip().splitlines():
            line = line.strip()
            if ":" in line:
                key, _, value = line.partition(":")
                key = key.strip()
                value = value.strip()
                if key:
                    headers[key] = value
        return headers

    def is_available(self) -> bool:
        return bool(self._base_url)

    async def stream(self, prompt: str, system: str = "") -> AsyncIterator[str]:
        messages: list[dict[str, Any]] = []
        if system:
            messages.append({"role": "system", "content": system})
        messages.append({"role": "user", "content": prompt})

        headers = {
            "Content-Type": "application/json",
        }
        # Only add Bearer token if an API key is configured
        if self._api_key:
            headers["Authorization"] = f"Bearer {self._api_key}"
        # Merge custom headers (can override Authorization for custom auth schemes)
        if self._custom_headers:
            headers.update(self._custom_headers)
            log.info(
                "Custom headers applied: %s",
                ", ".join(self._custom_headers.keys()),
            )
        else:
            log.warning("No custom headers configured")
        payload = {
            "model": self._model,
            "messages": messages,
            "stream": True,
        }

        async with httpx.AsyncClient(timeout=60, follow_redirects=False) as client:
            async with client.stream(
                "POST",
                self._endpoint_url,
                json=payload,
                headers=headers,
            ) as response:
                if response.status_code in (301, 302, 307, 308):
                    location = response.headers.get("location", "unknown")
                    raise RuntimeError(
                        f"API redirected ({response.status_code}) to {location}. "
                        "Check your Base URL and authentication headers (custom headers may not be reaching the server)."
                    )
                if response.status_code >= 400:
                    # Read the error body before raising so we can surface a useful message
                    body = await response.aread()
                    body_text = body.decode("utf-8", errors="replace")
                    log.error(
                        "API error %s from %s: %s", response.status_code, self._base_url, body_text
                    )
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
