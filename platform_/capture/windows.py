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
            import pywinauto
            from pywinauto import Desktop
            desktop = Desktop(backend="uia")
            focused = desktop.get_focus()
            if focused:
                patterns = focused.element_info.control_type
                # Try getting selected text via UIA value pattern
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
            time.sleep(0.15)
            result = pyperclip.paste()
            return result.strip() or None
        except Exception as e:
            log.debug("Clipboard capture failed: %s", e)
            return None
        finally:
            try:
                pyperclip.copy(original)
            except Exception:
                pass
