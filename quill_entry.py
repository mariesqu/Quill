"""PyInstaller entry point for Quill sidecar — uses absolute imports."""
import sys
import os

# Ensure the project root is on the path so 'core', 'providers', 'platform_' are importable
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from core.main import cli_entry

if __name__ == "__main__":
    cli_entry()
