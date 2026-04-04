#!/usr/bin/env bash
# Quill — macOS one-liner installer
# Usage: bash scripts/install-macos.sh

set -euo pipefail

QUILL_DIR="$HOME/.quill"
REPO="https://github.com/mariesqu/Quill"

echo ""
echo "🪶 Quill — macOS Installer"
echo "=================================="

# ── Homebrew check ────────────────────────────────────────────────────────────
if ! command -v brew &>/dev/null; then
    echo "→ Installing Homebrew..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
fi

# ── Python ────────────────────────────────────────────────────────────────────
echo "→ Installing Python 3.11+..."
brew install python@3.11 2>/dev/null || true

PYTHON="$(brew --prefix python@3.11)/bin/python3.11"

# ── Python venv ───────────────────────────────────────────────────────────────
echo "→ Setting up Python environment..."
mkdir -p "$QUILL_DIR"
"$PYTHON" -m venv "$QUILL_DIR/venv"
source "$QUILL_DIR/venv/bin/activate"

# ── Python deps ───────────────────────────────────────────────────────────────
echo "→ Installing Python dependencies..."
pip install --upgrade pip -q
pip install httpx pyyaml pyperclip pynput pyobjc-framework-AppKit psutil -q

# ── Quill source ──────────────────────────────────────────────────────────────
if [ -d "$QUILL_DIR/src" ]; then
    echo "→ Updating Quill source..."
    git -C "$QUILL_DIR/src" pull
else
    echo "→ Cloning Quill..."
    git clone "$REPO" "$QUILL_DIR/src"
fi

# ── Launcher ──────────────────────────────────────────────────────────────────
echo "→ Creating launcher..."
mkdir -p /usr/local/bin
cat > /usr/local/bin/quill << EOF
#!/usr/bin/env bash
source "$QUILL_DIR/venv/bin/activate"
cd "$QUILL_DIR/src"
exec python -m core.main "\$@"
EOF
chmod +x /usr/local/bin/quill

echo ""
echo "✅ Quill installed! Run: quill"
echo ""
echo "⚠️  macOS Accessibility permission required on first run."
echo "   Go to: System Settings → Privacy & Security → Accessibility"
echo "   Add Quill to the allowed apps."
echo ""
echo "   Hotkey: Cmd+Shift+Space"
