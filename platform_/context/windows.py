"""Windows active app context detection."""
from __future__ import annotations

import logging
from .base import ContextBackend
from ._app_map import lookup_context

log = logging.getLogger(__name__)


class WindowsContext(ContextBackend):
    def get_active_context(self) -> dict:
        try:
            import pygetwindow as gw
            win = gw.getActiveWindow()
            if win and win.title:
                return lookup_context(win.title)
        except Exception as e:
            log.debug("pygetwindow failed: %s", e)

        try:
            import psutil
            import ctypes
            hwnd = ctypes.windll.user32.GetForegroundWindow()
            pid = ctypes.c_ulong()
            ctypes.windll.user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
            proc = psutil.Process(pid.value)
            return lookup_context(proc.name().replace(".exe", ""))
        except Exception as e:
            log.debug("psutil context failed: %s", e)

        return self._fallback()
