#!/usr/bin/env bash
# Quill — cross-platform build script
# Builds Python sidecar (PyInstaller) + Tauri app bundle

set -euo pipefail

check_command() {
    if ! command -v "$1" &>/dev/null; then
        echo "❌ Required tool not found: $1"
        exit 1
    fi
}

check_command python3
check_command pip3
check_command npm
check_command cargo

OS="$(uname -s)"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$SCRIPT_DIR/.."

echo ""
echo "🪶 Quill — Build"
echo "=============================="
echo "Platform: $OS"
echo ""

# ── Python sidecar (PyInstaller) ──────────────────────────────────────────────
echo "→ Building Python sidecar (quill-core)..."
cd "$ROOT"

if ! command -v pyinstaller &>/dev/null; then
    pip install "pyinstaller>=6.0,<7.0" -q
fi

# PyInstaller --add-data separator is : on Unix, ; on Windows
SEP=":"
if [[ "$OS" == MINGW* ]] || [[ "$OS" == MSYS* ]] || [[ "$OS" == CYGWIN* ]]; then
    SEP=";"
fi

pyinstaller \
    --onefile \
    --name quill-core \
    --distpath ui \
    --add-data "config/default.yaml${SEP}config" \
    --add-data "config/modes.yaml${SEP}config" \
    --hidden-import core \
    --hidden-import core.main \
    --hidden-import core.config_loader \
    --hidden-import core.streamer \
    --hidden-import core.history \
    --hidden-import core.tutor \
    --hidden-import core.clipboard_monitor \
    --hidden-import core.platform \
    --hidden-import core.prompt_builder \
    --hidden-import providers \
    --hidden-import providers.openrouter \
    --hidden-import providers.ollama \
    --hidden-import providers.openai \
    --hidden-import providers.generic \
    --hidden-import providers.generic_endpoint \
    --hidden-import platform_ \
    --hidden-import pynput.keyboard._xorg \
    --hidden-import pynput.keyboard._darwin \
    --hidden-import pynput.keyboard._win32 \
    quill_entry.py

echo "✓ quill-core built at ui/quill-core"

# ── Tauri app ─────────────────────────────────────────────────────────────────
echo "→ Building Tauri app..."
cd "$ROOT/ui"

if ! command -v npm &>/dev/null; then
    echo "❌ npm not found. Please install Node.js >= 18."
    exit 1
fi

npm install
npm run tauri build

echo ""
echo "✅ Build complete!"
echo "   Installer: ui/src-tauri/target/release/bundle/"
