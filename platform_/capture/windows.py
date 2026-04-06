"""Windows text capture: UIA (pywinauto) with clipboard fallback."""

from __future__ import annotations

import logging
import time
from typing import Optional

import pyperclip

from .base import CaptureBackend

log = logging.getLogger(__name__)


class WindowsCapture(CaptureBackend):
    def get_selected_text(self) -> Optional[str]:
        text = self._try_uia()
        if text:
            return text.strip()
        return self._try_clipboard()

    def _try_uia(self) -> Optional[str]:
        try:
            from pywinauto import Desktop

            desktop = Desktop(backend="uia")
            focused = desktop.get_focus()
            if focused:
                # Try getting selected text via UIA text range provider
                try:
                    sel = focused.iface_text_range_provider
                    if sel:
                        return sel.GetText(-1)
                except Exception:
                    pass
                # Fallback: read selection via clipboard within UIA context
        except Exception as e:
            log.debug("UIA capture failed: %s", e)
        return None

    def _try_clipboard(self) -> Optional[str]:
        original = ""
        try:
            original = pyperclip.paste()
            pyperclip.copy("")
            import keyboard

            keyboard.send("ctrl+c")
            # Retry with change detection instead of fixed sleep
            for _ in range(6):  # up to 300ms total
                time.sleep(0.05)
                result = pyperclip.paste()
                if result:  # clipboard changed from empty
                    return result.strip() or None
            return None
        except Exception as e:
            log.debug("Clipboard capture failed: %s", e)
            return None
        finally:
            try:
                pyperclip.copy(original)
            except Exception:
                pass
