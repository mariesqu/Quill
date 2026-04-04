"""Linux active app context detection via xdotool + psutil."""
from __future__ import annotations

import logging
import subprocess
from .base import ContextBackend
from ._app_map import lookup_context

log = logging.getLogger(__name__)


class LinuxContext(ContextBackend):
    def get_active_context(self) -> dict:
        # Try xdotool for window name
        try:
            result = subprocess.run(
                ["xdotool", "getactivewindow", "getwindowname"],
                capture_output=True, text=True, timeout=1
            )
            if result.returncode == 0 and result.stdout.strip():
                return lookup_context(result.stdout.strip())
        except FileNotFoundError:
            log.debug("xdotool not found")
        except Exception as e:
            log.debug("xdotool context failed: %s", e)

        # Fallback: xdotool get PID → psutil
        try:
            pid_result = subprocess.run(
                ["xdotool", "getactivewindow", "getwindowpid"],
                capture_output=True, text=True, timeout=1
            )
            if pid_result.returncode == 0:
                import psutil
                pid = int(pid_result.stdout.strip())
                proc = psutil.Process(pid)
                return lookup_context(proc.name())
        except Exception as e:
            log.debug("psutil context failed: %s", e)

        return self._fallback()
