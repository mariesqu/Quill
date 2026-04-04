"""
Quill — main orchestrator.
Runs as a Tauri sidecar: reads commands from stdin, writes events to stdout.
"""
from __future__ import annotations

import asyncio
import logging
import signal
import sys
from typing import Any

from .config_loader import load_config, load_modes
from .platform import get_backends, get_os
from .prompt_builder import build_prompt
from .streamer import (
    emit_chunk,
    emit_done,
    emit_error,
    emit_permission_required,
    emit_ready,
    emit_show_overlay,
    read_command,
    stream_to_overlay,
)

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
    stream=sys.stderr,  # stderr only; stdout is the IPC channel
)
log = logging.getLogger("quill.main")


def _modes_list(modes: dict[str, Any]) -> list[dict]:
    return [
        {"id": k, "label": v.get("label", k), "icon": v.get("icon", "")}
        for k, v in modes.items()
    ]


class QuillApp:
    def __init__(self) -> None:
        self.config = load_config()
        self.modes = load_modes()
        self.backends: dict | None = None
        self._running = True
        self._last_text: str = ""
        self._last_result: str = ""

    def _load_provider(self):
        provider_name = self.config.get("provider", "openrouter")
        if provider_name == "openrouter":
            from providers.openrouter import OpenRouterProvider
            return OpenRouterProvider(self.config)
        elif provider_name == "ollama":
            from providers.ollama import OllamaProvider
            return OllamaProvider(self.config)
        elif provider_name == "openai":
            from providers.openai import OpenAIProvider
            return OpenAIProvider(self.config)
        else:
            from providers.generic import GenericProvider
            return GenericProvider(self.config)

    def _on_hotkey(self) -> None:
        """Called from hotkey thread — schedule async work on event loop."""
        asyncio.run_coroutine_threadsafe(self._handle_hotkey(), self._loop)

    async def _handle_hotkey(self) -> None:
        try:
            text = self.backends["capture"].get_selected_text()
            if not text or not text.strip():
                log.info("Hotkey fired but no text selected.")
                return

            self._last_text = text.strip()
            context = self.backends["context"].get_active_context()

            emit_show_overlay(
                text=self._last_text,
                context=context,
                modes=_modes_list(self.modes),
            )
        except Exception as e:
            log.exception("Error handling hotkey")
            emit_error(str(e))

    async def _handle_mode_selected(self, mode: str) -> None:
        try:
            context = self.backends["context"].get_active_context()
            language = self.config.get("language", "auto")
            system, user_prompt = build_prompt(
                self._last_text, mode, self.modes, context, language
            )

            provider = self._load_provider()
            self._last_result = await stream_to_overlay(
                provider.stream(user_prompt, system)
            )
        except Exception as e:
            log.exception("Error processing mode %s", mode)
            emit_error(str(e))

    async def _handle_replace_confirmed(self) -> None:
        try:
            if self._last_result:
                self.backends["replace"].paste_text(self._last_result)
        except Exception as e:
            log.exception("Error replacing text")
            emit_error(str(e))

    async def _command_loop(self) -> None:
        while self._running:
            cmd = await read_command()
            if cmd is None:
                # stdin closed → Tauri exited
                self._running = False
                break

            cmd_type = cmd.get("type")
            log.debug("Received command: %s", cmd_type)

            if cmd_type == "mode_selected":
                await self._handle_mode_selected(cmd.get("mode", "rewrite"))
            elif cmd_type == "replace_confirmed":
                await self._handle_replace_confirmed()
            elif cmd_type == "dismissed":
                self._last_result = ""
            elif cmd_type == "ping":
                emit_ready()
            elif cmd_type == "save_config":
                from .config_loader import save_user_config
                save_user_config(cmd.get("config", {}))
                self.config = load_config()
            else:
                log.warning("Unknown command type: %s", cmd_type)

    async def run(self) -> None:
        self._loop = asyncio.get_event_loop()

        # Load backends
        try:
            self.backends = get_backends()
        except PermissionError as e:
            if str(e) == "accessibility":
                emit_permission_required("accessibility")
                # Wait for user to grant permission then retry
                await asyncio.sleep(5)
                self.backends = get_backends()
            else:
                raise

        # Register hotkey
        hotkey = self.config.get("hotkey", "ctrl+shift+space")
        self.backends["hotkey"].register(hotkey, self._on_hotkey)
        log.info("Quill ready. Hotkey: %s", hotkey)

        emit_ready()

        # Run command loop
        try:
            await self._command_loop()
        finally:
            self.backends["hotkey"].unregister_all()
            log.info("Quill stopped.")


def cli_entry() -> None:
    app = QuillApp()
    try:
        asyncio.run(app.run())
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    cli_entry()
