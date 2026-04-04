"""Tests for provider implementations."""
import pytest
from unittest.mock import AsyncMock, MagicMock, patch


# ── Ollama ────────────────────────────────────────────────────────────────────

class TestOllamaProvider:
    def _make(self, config=None):
        from providers.ollama import OllamaProvider
        return OllamaProvider(config or {"model": "gemma3:4b"})

    def test_is_available_false_when_offline(self):
        provider = self._make()
        with patch("httpx.get", side_effect=Exception("connection refused")):
            assert provider.is_available() is False

    @pytest.mark.asyncio
    async def test_stream_yields_chunks(self):
        import json
        chunks = [
            json.dumps({"response": "Hello", "done": False}),
            json.dumps({"response": " world", "done": False}),
            json.dumps({"response": "", "done": True}),
        ]

        async def mock_aiter_lines():
            for c in chunks:
                yield c

        mock_response = MagicMock()
        mock_response.raise_for_status = MagicMock()
        mock_response.aiter_lines = mock_aiter_lines

        mock_ctx = MagicMock()
        mock_ctx.__aenter__ = AsyncMock(return_value=mock_response)
        mock_ctx.__aexit__ = AsyncMock(return_value=False)

        mock_client = MagicMock()
        mock_client.stream = MagicMock(return_value=mock_ctx)

        mock_client_ctx = MagicMock()
        mock_client_ctx.__aenter__ = AsyncMock(return_value=mock_client)
        mock_client_ctx.__aexit__ = AsyncMock(return_value=False)

        provider = self._make()
        with patch("httpx.AsyncClient", return_value=mock_client_ctx):
            result = []
            async for chunk in provider.stream("test prompt"):
                result.append(chunk)

        assert result == ["Hello", " world"]


# ── Generic provider ─────────────────────────────────────────────────────────

class TestGenericProvider:
    def _make(self, config=None):
        from providers.generic import GenericOpenAIProvider
        return GenericOpenAIProvider(
            config or {"model": "test-model", "api_key": "sk-test"},
            base_url="http://localhost:1234/v1",
        )

    def test_is_available(self):
        provider = self._make()
        assert provider.is_available() is True

    def test_is_available_no_url(self):
        from providers.generic import GenericOpenAIProvider
        p = GenericOpenAIProvider({"model": "x"}, base_url="")
        assert p.is_available() is False

    @pytest.mark.asyncio
    async def test_stream_parses_sse(self):
        import json
        lines = [
            'data: ' + json.dumps({"choices": [{"delta": {"content": "Hi"}}]}),
            'data: ' + json.dumps({"choices": [{"delta": {"content": " there"}}]}),
            'data: [DONE]',
        ]

        async def mock_aiter_lines():
            for line in lines:
                yield line

        mock_response = MagicMock()
        mock_response.raise_for_status = MagicMock()
        mock_response.aiter_lines = mock_aiter_lines

        mock_ctx = MagicMock()
        mock_ctx.__aenter__ = AsyncMock(return_value=mock_response)
        mock_ctx.__aexit__ = AsyncMock(return_value=False)

        mock_client = MagicMock()
        mock_client.stream = MagicMock(return_value=mock_ctx)

        mock_client_ctx = MagicMock()
        mock_client_ctx.__aenter__ = AsyncMock(return_value=mock_client)
        mock_client_ctx.__aexit__ = AsyncMock(return_value=False)

        provider = self._make()
        with patch("httpx.AsyncClient", return_value=mock_client_ctx):
            result = []
            async for chunk in provider.stream("hello"):
                result.append(chunk)

        assert result == ["Hi", " there"]
