"""
Opt-in clipboard monitor — polls the clipboard every 500 ms and emits
a `clipboard_change` IPC event when new text is detected.

Only runs when `clipboard_monitor.enabled: true` in config.
Uses pyperclip which is already a core dependency.
"""
from __future__ import annotations

import asyncio
import logging

log = logging.getLogger(__name__)

_POLL_INTERVAL = 0.5   # seconds
_MIN_WORDS     = 3     # ignore clipboard entries shorter than this


async def run_clipboard_monitor(get_enabled: callable, emit_fn: callable) -> None:
    """
    Continuously poll the clipboard. Call this as a background asyncio task.

    Args:
        get_enabled: zero-arg callable that returns True if monitoring is enabled.
        emit_fn:     callable(text) to fire when new clipboard text is detected.
    """
    import pyperclip

    last_text = ""
    try:
        last_text = pyperclip.paste() or ""
    except Exception:
        pass

    while True:
        await asyncio.sleep(_POLL_INTERVAL)
        if not get_enabled():
            continue
        try:
            current = pyperclip.paste() or ""
        except Exception:
            continue

        if current != last_text:
            last_text = current
            if len(current.split()) >= _MIN_WORDS:
                try:
                    emit_fn(current)
                except Exception as e:
                    log.debug("clipboard emit error: %s", e)
