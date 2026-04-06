"""Linux paste-back backend using keyboard + pyperclip."""
from __future__ import annotations

import logging
import time

import pyperclip

from .base import ReplaceBackend

log = logging.getLogger(__name__)


class LinuxReplace(ReplaceBackend):
    def paste_text(self, text: str) -> bool:
        original = ""
        try:
            original = pyperclip.paste()
            pyperclip.copy(text)
            time.sleep(0.05)
            import keyboard
            keyboard.send("ctrl+v")
            time.sleep(0.1)
            return True
        except Exception as e:
            log.error("Failed to paste text: %s", e)
            return False
        finally:
            try:
                time.sleep(0.05)
                pyperclip.copy(original)
            except Exception:
                pass
