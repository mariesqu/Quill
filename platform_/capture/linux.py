"""Linux text capture: X11 PRIMARY selection (xclip) with Ctrl+C clipboard fallback."""
from __future__ import annotations

import logging
import subprocess
import time
from typing import Optional

import pyperclip

from .base import CaptureBackend

log = logging.getLogger(__name__)


class LinuxCapture(CaptureBackend):
    def get_selected_text(self) -> Optional[str]:
        text = self._try_primary_selection()
        if text:
            return text.strip()
        return self._try_clipboard()

    def _try_primary_selection(self) -> Optional[str]:
        # X11 PRIMARY selection = whatever is currently highlighted (no Ctrl+C needed)
        try:
            result = subprocess.run(
                ["xclip", "-o", "-selection", "primary"],
                capture_output=True, text=True, timeout=1
            )
            if result.returncode == 0:
                return result.stdout.strip() or None
        except FileNotFoundError:
            log.debug("xclip not found, skipping PRIMARY selection capture")
        except Exception as e:
            log.debug("PRIMARY selection capture failed: %s", e)
        return None

    def _try_clipboard(self) -> Optional[str]:
        original = ""
        try:
            import keyboard
            original = pyperclip.paste()
            pyperclip.copy("")
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
