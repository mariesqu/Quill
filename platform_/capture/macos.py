"""macOS text capture: Accessibility API with Cmd+C clipboard fallback."""
from __future__ import annotations

import logging
import subprocess
import time
from typing import Optional

import pyperclip

from .base import CaptureBackend

log = logging.getLogger(__name__)


class MacOSCapture(CaptureBackend):
    def get_selected_text(self) -> Optional[str]:
        text = self._try_accessibility()
        if text:
            return text.strip()
        return self._try_clipboard()

    def _try_accessibility(self) -> Optional[str]:
        try:
            script = (
                'tell application "System Events" to return '
                'value of attribute "AXSelectedText" of focused UI element'
            )
            result = subprocess.run(
                ["osascript", "-e", script],
                capture_output=True, text=True, timeout=1
            )
            return result.stdout.strip() or None
        except Exception as e:
            log.debug("Accessibility capture failed: %s", e)
            return None

    def _try_clipboard(self) -> Optional[str]:
        original = ""
        try:
            from pynput import keyboard as kb
            original = pyperclip.paste()
            pyperclip.copy("")
            ctrl = kb.Controller()
            with ctrl.pressed(kb.Key.cmd):
                ctrl.press("c")
                ctrl.release("c")
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
