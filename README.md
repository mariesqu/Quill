# 🪶 Quill

> **Privacy-first, model-agnostic AI writing assistant for Windows, macOS, and Linux**

Select text anywhere → press a hotkey → instantly rewrite, translate, coach, shorten, or apply any custom AI action. Works with any AI — cloud or fully local.

---

## Why Quill?

| Tool | Problem |
|---|---|
| GitHub Copilot / MS Copilot | Paywalled, cloud-only, Windows/browser only |
| Grammarly | Subscription, reads all your text, no custom prompts |
| Browser extensions | Only works in browser |

**Quill's differentiator:** hotkey-triggered overlay + context-awareness + bring-your-own-model + runs everywhere. Free and open-source (MIT).

---

## Quick Start

### Linux
```bash
bash scripts/install-linux.sh
quill
```

### macOS
```bash
bash scripts/install-macos.sh
quill
```

### Windows (PowerShell)
```powershell
powershell -ExecutionPolicy Bypass -File scripts\install-windows.ps1
quill
```

---

## How It Works

1. Select text in **any app**
2. Press **`Ctrl+Shift+Space`** (Windows/Linux) or **`Cmd+Shift+Space`** (macOS)
3. A floating overlay appears — pick a mode:
   - ✏️ **Rewrite** — Improve clarity and flow
   - 🌐 **Translate** — Translate to any language
   - 💡 **Coach** — Get actionable improvement tips
   - ✂️ **Shorter** — Cut length by 40-50%
   - 👔 **Formal** — Business-appropriate tone
   - 📝 **Fix Grammar** — Correct errors silently
   - 📖 **Expand** — Add depth and detail
4. Response streams in real time
5. **Replace** (paste back) or **Copy**

---

## AI Providers

Quill works with any OpenAI-compatible API:

| Provider | Notes |
|---|---|
| **OpenRouter** *(default)* | Free tier, `google/gemma-3-27b-it` |
| **Ollama** | 100% local, no internet required |
| **OpenAI** | `gpt-4o-mini` recommended |
| **Any endpoint** | LM Studio, Groq, Jan.ai, etc. |

Configure in `config/user.yaml` or via the Settings UI.

---

## Configuration

Copy `config/default.yaml` to `config/user.yaml` (gitignored) and customize:

```yaml
provider: openrouter
model: google/gemma-3-27b-it
api_key: sk-or-xxxx          # or set QUILL_API_KEY env var

# For local Ollama:
# provider: ollama
# model: gemma3:4b
# base_url: http://localhost:11434
```

### Custom Modes

Add to `config/user.yaml`:
```yaml
custom_modes:
  legal:
    label: "Legal"
    icon: "⚖️"
    prompt: "Rewrite in formal legal language. Return only the rewritten text."
```

---

## Architecture

```
Global Hotkey → Capture Selected Text → Detect App Context
    → Build Prompt → Stream AI Response → Overlay UI
    → Replace in Original App
```

- **Core:** Python 3.11+, async streaming via `httpx`
- **UI:** Tauri v2 + React (native webview, ~8MB binary)
- **IPC:** JSON over stdio (Python sidecar ↔ Tauri)
- **Config:** YAML, human-editable
- **Platform layer:** Clean OS abstraction, each platform uses its best-fit library

---

## Development Setup

### Prerequisites
- Python 3.11+
- Node.js 18+
- Rust (for Tauri)

### Run

```bash
# Terminal 1: Python core
python -m core.main

# Terminal 2: Tauri UI
cd ui && npm install && npm run tauri dev
```

### Tests

```bash
pip install -e ".[dev]"
pytest
```

---

## Platform Notes

| Feature | Windows | macOS | Linux (X11) |
|---|---|---|---|
| Global hotkey | `keyboard` lib | `pynput` | `pynput` |
| Text capture | UIA + clipboard | Accessibility API + clipboard | xclip PRIMARY + clipboard |
| Paste back | Ctrl+V | Cmd+V | Ctrl+V |
| Permissions | None | Accessibility ⚠️ | None |

**macOS:** Accessibility permission required. Quill will guide you through it on first run.

**Linux Wayland:** Limited support. X11 recommended for Phase 1.

---

## License

MIT — see [LICENSE](LICENSE)
