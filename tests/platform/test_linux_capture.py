"""Linux capture tests — skipped on non-Linux."""

import platform
import pytest

pytestmark = pytest.mark.skipif(platform.system() != "Linux", reason="Linux only")


def test_linux_capture_imports():
    from platform_.capture.linux import LinuxCapture

    capture = LinuxCapture()
    assert hasattr(capture, "get_selected_text")


def test_linux_capture_primary_selection_graceful_failure(monkeypatch):
    """If xclip is not installed, should return None gracefully."""
    from platform_.capture.linux import LinuxCapture
    import subprocess

    def mock_run(*args, **kwargs):
        raise FileNotFoundError("xclip not found")

    monkeypatch.setattr(subprocess, "run", mock_run)
    capture = LinuxCapture()
    # Should not raise
    result = capture._try_primary_selection()
    assert result is None


def test_linux_permissions_check():
    from platform_.permissions.linux import check_xdotool, check_xclip

    # These return bool — just verify no crash
    assert isinstance(check_xdotool(), bool)
    assert isinstance(check_xclip(), bool)
