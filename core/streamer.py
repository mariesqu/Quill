"""
IPC streamer: emits newline-delimited JSON messages to stdout (read by Tauri sidecar)
and reads JSON commands from stdin.
"""

from __future__ import annotations

import asyncio
import json
import sys
from typing import Any, AsyncIterator


def _emit(msg: dict[str, Any]) -> None:
    """Write a single JSON message to stdout (Tauri sidecar reads this)."""
    print(json.dumps(msg, ensure_ascii=False), flush=True)


def emit_show_overlay(
    text: str, context: dict[str, Any], modes: list[dict], chains: list[dict]
) -> None:
    _emit(
        {
            "type": "show_overlay",
            "text": text,
            "context": context,
            "modes": modes,
            "chains": chains,
        }
    )


def emit_chunk(chunk: str) -> None:
    _emit({"type": "stream_chunk", "chunk": chunk})


def emit_done(full_text: str, entry_id: int | None = None) -> None:
    _emit({"type": "stream_done", "full_text": full_text, "entry_id": entry_id})


def emit_chain_step(step: int, total: int, mode: str) -> None:
    _emit({"type": "chain_step", "step": step, "total": total, "mode": mode})


def emit_tutor_explanation(explanation: str, entry_id: int | None = None) -> None:
    _emit({"type": "tutor_explanation", "explanation": explanation, "entry_id": entry_id})


def emit_tutor_lesson(lesson_md: str, period: str) -> None:
    _emit({"type": "tutor_lesson", "lesson_md": lesson_md, "period": period})


def emit_history(entries: list[dict]) -> None:
    _emit({"type": "history", "entries": entries})


def emit_smart_suggestion(mode_id: str, reason: str) -> None:
    _emit({"type": "smart_suggestion", "mode_id": mode_id, "reason": reason})


def emit_favorite_toggled(entry_id: int, favorited: bool) -> None:
    _emit({"type": "favorite_toggled", "entry_id": entry_id, "favorited": favorited})


def emit_export_data(entries: list[dict], fmt: str) -> None:
    _emit({"type": "export_data", "entries": entries, "format": fmt})


def emit_comparison_done(mode_a: str, result_a: str, mode_b: str, result_b: str) -> None:
    _emit(
        {
            "type": "comparison_done",
            "mode_a": mode_a,
            "result_a": result_a,
            "mode_b": mode_b,
            "result_b": result_b,
        }
    )


def emit_pronunciation(text: str) -> None:
    _emit({"type": "pronunciation", "text": text})


def emit_clipboard_change(text: str) -> None:
    _emit({"type": "clipboard_change", "text": text})


def emit_templates(templates: list[dict]) -> None:
    _emit({"type": "templates_updated", "templates": templates})


def emit_error(message: str) -> None:
    _emit({"type": "error", "message": message})


def emit_permission_required(permission: str) -> None:
    _emit({"type": "permission_required", "permission": permission})


def emit_ready() -> None:
    _emit({"type": "ready"})


async def read_command() -> dict[str, Any] | None:
    """Read one newline-delimited JSON command from stdin (async)."""
    loop = asyncio.get_event_loop()
    line = await loop.run_in_executor(None, sys.stdin.readline)
    if not line:
        return None
    try:
        return json.loads(line.strip())
    except json.JSONDecodeError as e:
        import logging

        logging.getLogger("quill.streamer").warning(
            "Invalid JSON command: %s (error: %s)", line.strip()[:100], e
        )
        return None


async def stream_to_overlay(
    chunks: AsyncIterator[str],
) -> str:
    """Stream AI response chunks to the overlay; return full assembled text."""
    full = []
    async for chunk in chunks:
        emit_chunk(chunk)
        full.append(chunk)
    text = "".join(full)
    return text
