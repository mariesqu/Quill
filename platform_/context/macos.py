"""macOS active app context detection via AppKit NSWorkspace."""

from __future__ import annotations

import logging
from .base import ContextBackend
from ._app_map import lookup_context

log = logging.getLogger(__name__)


class MacOSContext(ContextBackend):
    def get_active_context(self) -> dict:
        try:
            from AppKit import NSWorkspace

            app = NSWorkspace.sharedWorkspace().frontmostApplication()
            name = app.localizedName() or ""
            return lookup_context(name)
        except Exception as e:
            log.debug("NSWorkspace context failed: %s", e)
        return self._fallback()
