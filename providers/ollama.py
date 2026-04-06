"""Ollama provider — fully local, privacy-first."""

from __future__ import annotations

import json
import logging
from typing import AsyncIterator

import httpx

from .base import BaseProvider

log = logging.getLogger(__name__)

OLLAMA_DEFAULT_URL = "http://localhost:11434"
DEFAULT_MODEL = "gemma3:4b"


class OllamaProvider(BaseProvider):
    def __init__(self, config: dict) -> None:
        super().__init__(config)
        self._base_url = config.get("base_url", OLLAMA_DEFAULT_URL).rstrip("/")
        self._model = config.get("model", DEFAULT_MODEL)

    def is_available(self) -> bool:
        try:
            r = httpx.get(f"{self._base_url}/api/tags", timeout=2)
            return r.status_code == 200
        except Exception:
            return False

    async def stream(self, prompt: str, system: str = "") -> AsyncIterator[str]:
        payload: dict = {
            "model": self._model,
            "prompt": prompt,
            "stream": True,
        }
        if system:
            payload["system"] = system

        async with httpx.AsyncClient(timeout=120) as client:
            async with client.stream(
                "POST",
                f"{self._base_url}/api/generate",
                json=payload,
            ) as response:
                if response.status_code >= 400:
                    body = await response.aread()
                    error_text = body.decode("utf-8", errors="replace")[:1000]  # Limit size
                    log.error("Ollama error %s: %s", response.status_code, error_text)
                    if response.status_code == 404:
                        raise RuntimeError(
                            f"Model '{self._model}' not found. Run: ollama pull {self._model}"
                        )
                    try:
                        err = json.loads(error_text).get("error", error_text)
                    except Exception:
                        err = error_text
                    raise RuntimeError(f"Ollama error {response.status_code}: {err}")

                async for line in response.aiter_lines():
                    if not line.strip():
                        continue
                    try:
                        chunk = json.loads(line)
                        text = chunk.get("response", "")
                        if text:
                            yield text
                        if chunk.get("done"):
                            break
                    except (json.JSONDecodeError, KeyError, TypeError) as e:
                        log.debug("Failed to parse Ollama chunk: %s — %s", line[:100], e)
