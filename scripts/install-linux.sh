#!/usr/bin/env bash
# Quill — Linux one-liner installer
# Usage: bash scripts/install-linux.sh

set -euo pipefail

QUILL_DIR="$HOME/.quill"
REPO="https://github.com/mariesqu/Quill"

echo ""
echo "🪶 Quill — Linux Installer"
echo "=================================="

# ── System deps ───────────────────────────────────────────────────────────────
echo "→ Installing system dependencies..."
if command -v apt-get &>/dev/null; then
    sudo apt-get install -y xdotool xclip python3 python3-pip python3-venv
elif command -v dnf &>/dev/null; then
    sudo dnf install -y xdotool xclip python3 python3-pip
elif command -v pacman &>/dev/null; then
    sudo pacman -S --noconfirm xdotool xclip python python-pip
else
    echo "⚠  Unknown package manager. Install xdotool and xclip manually."
fi

if ! command -v python3 &>/dev/null; then
    echo "❌ Python 3 installation failed. Please install manually."
    exit 1
fi

# ── Python venv ───────────────────────────────────────────────────────────────
echo "→ Setting up Python environment..."
mkdir -p "$QUILL_DIR"
python3 -m venv "$QUILL_DIR/venv"
source "$QUILL_DIR/venv/bin/activate"

# ── Python deps ───────────────────────────────────────────────────────────────
echo "→ Installing Python dependencies..."
pip install --upgrade pip -q
pip install httpx pyyaml pyperclip pynput keyboard psutil -q

# ── Quill source ──────────────────────────────────────────────────────────────
if [ -d "$QUILL_DIR/src" ]; then
    echo "→ Updating Quill source..."
    git -C "$QUILL_DIR/src" pull
else
    echo "→ Cloning Quill..."
    git clone "$REPO" "$QUILL_DIR/src"
fi

# ── Launcher script ───────────────────────────────────────────────────────────
echo "→ Creating launcher..."
cat > "$HOME/.local/bin/quill" << EOF
#!/usr/bin/env bash
source "$QUILL_DIR/venv/bin/activate"
cd "$QUILL_DIR/src"
exec python -m core.main "\$@"
EOF
chmod +x "$HOME/.local/bin/quill"

echo ""
echo "✅ Quill installed! Run: quill"
echo "   Hotkey: Ctrl+Shift+Space"
echo ""
echo "💡 For best experience, install xdotool if not already:"
echo "   sudo apt install xdotool xclip"
