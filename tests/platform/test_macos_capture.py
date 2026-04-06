"""macOS capture tests — skipped on non-macOS."""

import platform
import pytest

pytestmark = pytest.mark.skipif(platform.system() != "Darwin", reason="macOS only")


def test_macos_capture_imports():
    from platform_.capture.macos import MacOSCapture

    capture = MacOSCapture()
    assert hasattr(capture, "get_selected_text")


def test_macos_capture_returns_none_or_str():
    from platform_.capture.macos import MacOSCapture

    capture = MacOSCapture()
    result = capture.get_selected_text()
    assert result is None or isinstance(result, str)
