"""
Quill — main orchestrator.
Runs as a Tauri sidecar: reads commands from stdin, writes events to stdout.
"""
from __future__ import annotations

import asyncio
import logging
import sys
from typing import Any

from .config_loader import load_config, load_modes, save_user_config
from .platform import get_backends, get_os
from .prompt_builder import build_prompt, suggest_mode
from .streamer import (
    emit_chain_step,
    emit_comparison_done,
    emit_clipboard_change,
    emit_done,
    emit_error,
    emit_export_data,
    emit_favorite_toggled,
    emit_history,
    emit_permission_required,
    emit_pronunciation,
    emit_ready,
    emit_show_overlay,
    emit_smart_suggestion,
    emit_templates,
    emit_tutor_explanation,
    emit_tutor_lesson,
    read_command,
    stream_to_overlay,
)

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
    stream=sys.stderr,
)
log = logging.getLogger("quill.main")


def _modes_list(modes: dict[str, Any]) -> list[dict]:
    return [
        {"id": k, "label": v.get("label", k), "icon": v.get("icon", "")}
        for k, v in modes.items()
    ]


def _chains_list(chains: dict[str, Any]) -> list[dict]:
    return [
        {
            "id":          k,
            "label":       v.get("label", k),
            "icon":        v.get("icon", ""),
            "steps":       v.get("steps", []),
            "description": v.get("description", ""),
        }
        for k, v in chains.items()
    ]


class QuillApp:
    def __init__(self) -> None:
        self.config = load_config()
        self.modes, self.chains = load_modes()
        self.backends: dict | None = None
        self._running = True
        self._last_text: str = ""
        self._last_result: str = ""
        self._last_entry_id: int | None = None
        self._last_mode: str = ""
        self._last_language: str = "auto"

    def _load_provider(self):
        name = self.config.get("provider", "openrouter")
        if name == "openrouter":
            from providers.openrouter import OpenRouterProvider
            return OpenRouterProvider(self.config)
        elif name == "ollama":
            from providers.ollama import OllamaProvider
            return OllamaProvider(self.config)
        elif name == "openai":
            from providers.openai import OpenAIProvider
            return OpenAIProvider(self.config)
        else:
            from providers.generic import GenericOpenAIProvider
            return GenericOpenAIProvider(self.config)

    def _history_enabled(self) -> bool:
        return bool(self.config.get("history", {}).get("enabled"))

    def _tutor_enabled(self) -> bool:
        return bool(self.config.get("tutor", {}).get("enabled")) and self._history_enabled()

    def _on_hotkey(self) -> None:
        asyncio.run_coroutine_threadsafe(self._handle_hotkey(), self._loop)

    # ── Hotkey handler ────────────────────────────────────────────────────────

    async def _handle_hotkey(self) -> None:
        try:
            text = self.backends["capture"].get_selected_text()
            if not text or not text.strip():
                log.info("Hotkey fired but no text selected.")
                return

            self._last_text = text.strip()
            context = self.backends["context"].get_active_context()

            # Smart mode suggestion (always computed, UI decides whether to show it)
            suggested_mode, suggestion_reason = suggest_mode(self._last_text, context)
            emit_smart_suggestion(suggested_mode, suggestion_reason)

            emit_show_overlay(
                text=self._last_text,
                context=context,
                modes=_modes_list(self.modes),
                chains=_chains_list(self.chains),
            )
        except Exception as e:
            log.exception("Error handling hotkey")
            emit_error(str(e))

    # ── Mode execution ────────────────────────────────────────────────────────

    async def _run_single_mode(
        self,
        text: str,
        mode: str,
        language: str,
        extra_instruction: str = "",
    ) -> str:
        context  = self.backends["context"].get_active_context()
        persona  = self.config.get("persona", {})
        system, user_prompt = build_prompt(
            text, mode, self.modes, context, language, persona, extra_instruction
        )
        provider = self._load_provider()
        result   = await stream_to_overlay(provider.stream(user_prompt, system))
        return result

    async def _handle_mode_selected(
        self,
        mode: str,
        language: str | None = None,
        extra_instruction: str = "",
    ) -> None:
        try:
            effective_language = language or self.config.get("language", "auto")
            self._last_mode     = mode
            self._last_language = effective_language

            result = await self._run_single_mode(
                self._last_text, mode, effective_language, extra_instruction
            )
            self._last_result = result

            # Save to history
            entry_id = None
            if self._history_enabled():
                from .history import save_entry
                context = self.backends["context"].get_active_context()
                entry_id = save_entry(
                    original=self._last_text,
                    output=result,
                    mode=mode,
                    language=effective_language,
                    app_hint=context.get("hint", ""),
                    persona_tone=self.config.get("persona", {}).get("tone", "natural"),
                )
            self._last_entry_id = entry_id
            emit_done(result, entry_id)

            # Auto-explain if tutor is configured that way
            if self._tutor_enabled() and self.config.get("tutor", {}).get("auto_explain"):
                await self._handle_tutor_explain(entry_id)

        except Exception as e:
            log.exception("Error processing mode %s", mode)
            emit_error(str(e))

    async def _handle_chain_selected(
        self,
        chain_id: str,
        language: str | None = None,
        extra_instruction: str = "",
    ) -> None:
        """Run a chain of modes sequentially."""
        try:
            if chain_id not in self.chains:
                emit_error(f"Unknown chain: {chain_id!r}")
                return

            chain    = self.chains[chain_id]
            steps    = chain.get("steps", [])
            language = language or self.config.get("language", "auto")
            text     = self._last_text

            for i, mode in enumerate(steps):
                emit_chain_step(i + 1, len(steps), mode)
                text = await self._run_single_mode(
                    text, mode, language,
                    extra_instruction if i == 0 else ""  # instruction only on first step
                )

            self._last_result   = text
            self._last_mode     = f"chain:{chain_id}"
            self._last_language = language

            entry_id = None
            if self._history_enabled():
                from .history import save_entry
                context  = self.backends["context"].get_active_context()
                entry_id = save_entry(
                    original=self._last_text,
                    output=text,
                    mode=f"chain:{chain_id}",
                    language=language,
                    app_hint=context.get("hint", ""),
                    persona_tone=self.config.get("persona", {}).get("tone", "natural"),
                )
            self._last_entry_id = entry_id
            emit_done(text, entry_id)

        except Exception as e:
            log.exception("Error running chain %s", chain_id)
            emit_error(str(e))

    # ── Replace ───────────────────────────────────────────────────────────────

    async def _handle_replace_confirmed(self) -> None:
        try:
            if self._last_result:
                self.backends["replace"].paste_text(self._last_result)
        except Exception as e:
            log.exception("Error replacing text")
            emit_error(str(e))

    # ── Retry ─────────────────────────────────────────────────────────────────

    async def _handle_retry(self, extra_instruction: str = "") -> None:
        if not self._last_mode or not self._last_text:
            emit_error("Nothing to retry.")
            return
        if self._last_mode.startswith("chain:"):
            chain_id = self._last_mode[6:]
            await self._handle_chain_selected(chain_id, self._last_language, extra_instruction)
        else:
            await self._handle_mode_selected(self._last_mode, self._last_language, extra_instruction)

    # ── AI Tutor ──────────────────────────────────────────────────────────────

    async def _handle_tutor_explain(self, entry_id: int | None = None) -> None:
        try:
            from .tutor import build_explain_prompt
            system, user_prompt = build_explain_prompt(
                original=self._last_text,
                output=self._last_result,
                mode=self._last_mode,
                language=self._last_language,
            )
            provider = self._load_provider()
            explanation = ""
            async for chunk in provider.stream(user_prompt, system):
                explanation += chunk
            emit_tutor_explanation(explanation, entry_id)

            if entry_id and self._history_enabled():
                from .history import save_tutor_explanation
                save_tutor_explanation(entry_id, explanation)

        except Exception as e:
            log.exception("Error generating tutor explanation")
            emit_error(str(e))

    async def _handle_generate_lesson(self, period: str = "daily") -> None:
        try:
            from .history import get_stats, save_lesson, get_latest_lesson
            from .tutor import build_lesson_prompt

            stats  = get_stats(days=1 if period == "daily" else 7)
            system, user_prompt = build_lesson_prompt(stats, period)
            provider = self._load_provider()
            lesson = ""
            async for chunk in provider.stream(user_prompt, system):
                lesson += chunk
            emit_tutor_lesson(lesson, period)
            save_lesson(period, lesson, stats.get("top_language", ""))

        except Exception as e:
            log.exception("Error generating lesson")
            emit_error(str(e))

    async def _handle_get_history(self, limit: int = 30, language: str | None = None) -> None:
        try:
            from .history import get_recent
            entries = get_recent(limit=limit, language=language)
            emit_history(entries)
        except Exception as e:
            log.exception("Error fetching history")
            emit_error(str(e))

    # ── Favorites ─────────────────────────────────────────────────────────────

    async def _handle_toggle_favorite(self, entry_id: int) -> None:
        try:
            from .history import toggle_favorite
            favorited = toggle_favorite(entry_id)
            emit_favorite_toggled(entry_id, favorited)
        except Exception as e:
            log.exception("Error toggling favorite")
            emit_error(str(e))

    # ── Export history ────────────────────────────────────────────────────────

    async def _handle_export_history(self, fmt: str = "json") -> None:
        try:
            from .history import get_all_entries
            entries = get_all_entries()
            emit_export_data(entries, fmt)
        except Exception as e:
            log.exception("Error exporting history")
            emit_error(str(e))

    # ── Comparison mode ───────────────────────────────────────────────────────

    async def _handle_compare_modes(
        self,
        mode_a: str,
        mode_b: str,
        language: str | None = None,
        extra_instruction: str = "",
    ) -> None:
        try:
            lang = language or self.config.get("language", "auto")
            context  = self.backends["context"].get_active_context()
            persona  = self.config.get("persona", {})

            system_a, prompt_a = build_prompt(
                self._last_text, mode_a, self.modes, context, lang, persona, extra_instruction
            )
            system_b, prompt_b = build_prompt(
                self._last_text, mode_b, self.modes, context, lang, persona, extra_instruction
            )
            provider = self._load_provider()
            result_a = ""
            async for chunk in provider.stream(prompt_a, system_a):
                result_a += chunk
            result_b = ""
            async for chunk in provider.stream(prompt_b, system_b):
                result_b += chunk

            emit_comparison_done(mode_a, result_a, mode_b, result_b)
        except Exception as e:
            log.exception("Error running comparison")
            emit_error(str(e))

    # ── Pronunciation guide ───────────────────────────────────────────────────

    async def _handle_get_pronunciation(self, text: str, language: str) -> None:
        try:
            system = (
                "You are a pronunciation guide. Given text in a specific language, "
                "provide: 1) IPA transcription, 2) easy phonetic romanization for "
                "English speakers, 3) one-sentence tip on the trickiest sounds. "
                "Be concise and practical. No preamble."
            )
            user_prompt = (
                f"Provide pronunciation guide for this {language} text:\n\n{text}"
            )
            provider = self._load_provider()
            result = ""
            async for chunk in provider.stream(user_prompt, system):
                result += chunk
            emit_pronunciation(result)
        except Exception as e:
            log.exception("Error getting pronunciation")
            emit_error(str(e))

    # ── Quick templates ───────────────────────────────────────────────────────

    async def _handle_save_template(self, name: str, mode: str, instruction: str) -> None:
        try:
            templates = self.config.get("templates", [])
            # Replace if name already exists, otherwise append
            templates = [t for t in templates if t.get("name") != name]
            templates.append({"name": name, "mode": mode, "instruction": instruction})
            save_user_config({"templates": templates})
            self.config = load_config()
            emit_templates(templates)
        except Exception as e:
            log.exception("Error saving template")
            emit_error(str(e))

    async def _handle_delete_template(self, name: str) -> None:
        try:
            templates = [t for t in self.config.get("templates", []) if t.get("name") != name]
            save_user_config({"templates": templates})
            self.config = load_config()
            emit_templates(templates)
        except Exception as e:
            log.exception("Error deleting template")
            emit_error(str(e))

    # ── Per-mode hotkeys ──────────────────────────────────────────────────────

    def _register_mode_hotkeys(self) -> None:
        """Register optional per-mode hotkeys from config."""
        for mode_id, mode_cfg in self.modes.items():
            hk = mode_cfg.get("hotkey")
            if not hk:
                continue
            try:
                def make_handler(m):
                    def handler():
                        asyncio.run_coroutine_threadsafe(
                            self._handle_mode_direct(m), self._loop
                        )
                    return handler
                self.backends["hotkey"].register(hk, make_handler(mode_id))
                log.info("Registered per-mode hotkey %s → %s", hk, mode_id)
            except Exception as e:
                log.warning("Could not register hotkey %s for mode %s: %s", hk, mode_id, e)

    async def _handle_mode_direct(self, mode: str) -> None:
        """Run a mode directly (triggered by per-mode hotkey) on selected text."""
        try:
            text = self.backends["capture"].get_selected_text()
            if not text or not text.strip():
                return
            self._last_text = text.strip()
            context = self.backends["context"].get_active_context()
            emit_smart_suggestion(mode, f"Running {mode} via hotkey")
            emit_show_overlay(
                text=self._last_text,
                context=context,
                modes=_modes_list(self.modes),
                chains=_chains_list(self.chains),
            )
            await self._handle_mode_selected(mode)
        except Exception as e:
            log.exception("Error in direct mode hotkey")
            emit_error(str(e))

    # ── Command loop ──────────────────────────────────────────────────────────

    async def _command_loop(self) -> None:
        while self._running:
            cmd = await read_command()
            if cmd is None:
                self._running = False
                break

            t = cmd.get("type")
            log.debug("Command: %s", t)

            if t == "mode_selected":
                await self._handle_mode_selected(
                    cmd.get("mode", "rewrite"),
                    language=cmd.get("language"),
                    extra_instruction=cmd.get("extra_instruction", ""),
                )
            elif t == "chain_selected":
                await self._handle_chain_selected(
                    cmd.get("chain_id", ""),
                    language=cmd.get("language"),
                    extra_instruction=cmd.get("extra_instruction", ""),
                )
            elif t == "retry":
                await self._handle_retry(
                    extra_instruction=cmd.get("extra_instruction", "")
                )
            elif t == "replace_confirmed":
                await self._handle_replace_confirmed()
            elif t == "set_result":
                # Lets the UI override _last_result (e.g. after picking a comparison side)
                self._last_result = cmd.get("text", self._last_result)
            elif t == "dismissed":
                self._last_result = ""
            elif t == "tutor_explain":
                await self._handle_tutor_explain(cmd.get("entry_id"))
            elif t == "generate_lesson":
                await self._handle_generate_lesson(cmd.get("period", "daily"))
            elif t == "get_history":
                await self._handle_get_history(
                    limit=cmd.get("limit", 30),
                    language=cmd.get("language"),
                )
            elif t == "toggle_favorite":
                await self._handle_toggle_favorite(cmd.get("entry_id", 0))
            elif t == "export_history":
                await self._handle_export_history(cmd.get("format", "json"))
            elif t == "compare_modes":
                await self._handle_compare_modes(
                    cmd.get("mode_a", "rewrite"),
                    cmd.get("mode_b", "formal"),
                    language=cmd.get("language"),
                    extra_instruction=cmd.get("extra_instruction", ""),
                )
            elif t == "get_pronunciation":
                await self._handle_get_pronunciation(
                    cmd.get("text", self._last_result),
                    cmd.get("language", self._last_language),
                )
            elif t == "save_template":
                await self._handle_save_template(
                    cmd.get("name", ""), cmd.get("mode", ""), cmd.get("instruction", "")
                )
            elif t == "delete_template":
                await self._handle_delete_template(cmd.get("name", ""))
            elif t == "ping":
                emit_ready()
                # Also emit current templates on ping so UI stays in sync
                emit_templates(self.config.get("templates", []))
            elif t == "save_config":
                prev_clipboard_enabled = self.config.get("clipboard_monitor", {}).get("enabled", False)
                save_user_config(cmd.get("config", {}))
                self.config = load_config()
                self.modes, self.chains = load_modes()
                if self._history_enabled():
                    from .history import init_db
                    init_db()
                # Start clipboard monitor if it was just enabled and not already running
                new_clipboard_enabled = self.config.get("clipboard_monitor", {}).get("enabled", False)
                if new_clipboard_enabled and not prev_clipboard_enabled and not self._clipboard_monitor_running:
                    from .clipboard_monitor import run_clipboard_monitor
                    self._clipboard_monitor_running = True
                    asyncio.ensure_future(
                        run_clipboard_monitor(
                            get_enabled=lambda: self.config.get("clipboard_monitor", {}).get("enabled", False),
                            emit_fn=emit_clipboard_change,
                        )
                    )
                    log.info("Clipboard monitor started (enabled via settings).")
            else:
                log.warning("Unknown command: %s", t)

    # ── Entry point ───────────────────────────────────────────────────────────

    async def run(self) -> None:
        self._loop = asyncio.get_event_loop()

        # Initialise history DB if enabled
        if self._history_enabled():
            from .history import init_db
            init_db()

        # Load platform backends
        try:
            self.backends = get_backends()
        except PermissionError as e:
            if str(e) == "accessibility":
                emit_permission_required("accessibility")
                await asyncio.sleep(5)
                self.backends = get_backends()
            else:
                raise

        # Register main hotkey
        hotkey = self.config.get("hotkey", "ctrl+shift+space")
        self.backends["hotkey"].register(hotkey, self._on_hotkey)
        log.info("Quill ready. Hotkey: %s", hotkey)

        # Register per-mode hotkeys (optional)
        self._register_mode_hotkeys()

        # Start clipboard monitor as a background task (opt-in)
        self._clipboard_monitor_running = False
        clipboard_cfg = self.config.get("clipboard_monitor", {})
        if clipboard_cfg.get("enabled"):
            from .clipboard_monitor import run_clipboard_monitor
            self._clipboard_monitor_running = True
            asyncio.ensure_future(
                run_clipboard_monitor(
                    get_enabled=lambda: self.config.get("clipboard_monitor", {}).get("enabled", False),
                    emit_fn=emit_clipboard_change,
                )
            )
            log.info("Clipboard monitor started.")

        emit_ready()
        # Emit persisted templates on startup so the UI can render them immediately
        emit_templates(self.config.get("templates", []))

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
