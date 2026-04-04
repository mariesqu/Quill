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


def emit_show_overlay(text: str, context: dict[str, Any], modes: list[dict]) -> None:
    _emit({
        "type": "show_overlay",
        "text": text,
        "context": context,
        "modes": modes,
    })


def emit_chunk(chunk: str) -> None:
    _emit({"type": "stream_chunk", "chunk": chunk})


def emit_done(full_text: str) -> None:
    _emit({"type": "stream_done", "full_text": full_text})


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
    except json.JSONDecodeError:
        return None


async def stream_to_overlay(
    chunks: AsyncIterator[str],
) -> str:
    """Stream AI response chunks to the overlay, return full assembled text."""
    full = []
    async for chunk in chunks:
        emit_chunk(chunk)
        full.append(chunk)
    text = "".join(full)
    emit_done(text)
    return text
