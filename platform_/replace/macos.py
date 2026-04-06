"""macOS paste-back backend using pynput + pyperclip."""

from __future__ import annotations

import logging
import time

import pyperclip

from .base import ReplaceBackend

log = logging.getLogger(__name__)


class MacOSReplace(ReplaceBackend):
    def paste_text(self, text: str) -> bool:
        original = ""
        try:
            from pynput import keyboard as kb

            original = pyperclip.paste()
            pyperclip.copy(text)
            time.sleep(0.05)
            ctrl = kb.Controller()
            with ctrl.pressed(kb.Key.cmd):
                ctrl.press("v")
                ctrl.release("v")
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
