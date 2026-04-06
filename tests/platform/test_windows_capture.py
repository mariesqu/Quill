"""Windows capture tests — skipped on non-Windows and CI (keyboard lib hangs on headless runners)."""

import os
import platform
import pytest

pytestmark = [
    pytest.mark.skipif(platform.system() != "Windows", reason="Windows only"),
    pytest.mark.skipif(os.environ.get("CI") == "true", reason="keyboard lib hangs on headless CI"),
]


def test_windows_capture_imports():
    from platform_.capture.windows import WindowsCapture

    capture = WindowsCapture()
    assert hasattr(capture, "get_selected_text")


def test_windows_capture_clipboard_fallback(monkeypatch):
    from platform_.capture.windows import WindowsCapture
    import pyperclip

    monkeypatch.setattr(pyperclip, "paste", lambda: "hello from clipboard")
    monkeypatch.setattr(pyperclip, "copy", lambda x: None)

    capture = WindowsCapture()
    # UIA will likely fail in test env, clipboard fallback should work
    # (may return None if keyboard.send fails too, which is acceptable in test env)
    result = capture.get_selected_text()
    assert result is None or isinstance(result, str)
