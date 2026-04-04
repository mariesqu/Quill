# Quill — Project Brief
> Privacy-first, model-agnostic AI writing assistant for Windows, macOS, and Linux

---

## Vision

Quill is a lightweight, always-available desktop tool that detects selected text anywhere on screen and offers instant AI-powered actions: rewrite, translate, coach, or any custom prompt the user defines.

It is **not** another cloud-locked AI assistant. Its core promise is:

> **"Works with any AI. Keeps your words on your machine."**

Unlike Copilot, Grammarly, or browser extensions, Quill is:
- Free and open-source (MIT)
- **Cross-platform** — Windows, macOS, Linux from a single codebase
- Provider-agnostic — swap models without touching code
- Context-aware — adapts behavior based on which app is focused
- Fully offline-capable when paired with a local model via Ollama

---

## The Gap It Fills

| Tool | Problem |
|---|---|
| GitHub Copilot / MS Copilot | Paywalled, cloud-only, Windows/browser only |
| Grammarly | Subscription, reads all your text, no custom prompts |
| Browser extensions | Only works in browser |
| Samsung AI Keyboard (Android) | No desktop equivalent exists on any OS |
| PowerToys | Windows-only, no AI integration |

Quill's differentiator: **hotkey-triggered overlay + context-awareness + bring-your-own-model + runs everywhere**.

---

## How It Works (User Flow)

1. User selects any text in any app on any OS
2. Presses global hotkey (`Ctrl+Shift+Space` on Windows/Linux, `Cmd+Shift+Space` on macOS)
3. A small floating overlay appears near the cursor
4. User picks a mode: **Rewrite / Translate / Coach** (or any custom mode)
5. Response streams into the overlay
6. User clicks **Replace** (pastes back into original app) or **Copy**

---

## Architecture Overview

```
┌──────────────────────────────────────────────────────┐
│                  OS System Layer                     │
│  Global Hotkey │ Text Selection │ Active App Info    │
│                                                      │
│  [Windows]          [macOS]           [Linux]        │
│  UIA + keyboard     Accessibility     xdotool/pynput │
│                     API + AppKit      X11/Wayland    │
└──────────────────────────┬───────────────────────────┘
                           │
┌──────────────────────────▼───────────────────────────┐
│           Platform Abstraction Layer (Python)         │
│                                                      │
│   capture/          context/          replace/       │
│   ├─ base.py        ├─ base.py        ├─ base.py     │
│   ├─ windows.py     ├─ windows.py     ├─ windows.py  │
│   ├─ macos.py       ├─ macos.py       ├─ macos.py    │
│   └─ linux.py       └─ linux.py       └─ linux.py    │
│                                                      │
│   hotkey/                                            │
│   ├─ base.py                                         │
│   ├─ windows.py  (keyboard lib)                      │
│   ├─ macos.py    (pynput)                            │
│   └─ linux.py    (pynput)                            │
└──────────────────────────┬───────────────────────────┘
                           │ text + context (platform-agnostic)
┌──────────────────────────▼───────────────────────────┐
│                  Core Engine (Python)                 │
│   Prompt Builder │ Mode Manager │ Response Streamer  │
└──────────────────────────┬───────────────────────────┘
                           │ prompt
┌──────────────────────────▼───────────────────────────┐
│              Provider Abstraction Layer               │
│   OpenRouter │ Ollama │ OpenAI │ Custom API           │
│   (OpenAI-compatible interface for all)               │
└──────────────────────────┬───────────────────────────┘
                           │ streamed response
┌──────────────────────────▼───────────────────────────┐
│          Overlay UI (Tauri v2 + React)                │
│   Floating popup │ Streaming text │ Mode buttons      │
│   Replace / Copy / Dismiss  [identical across OS]     │
└──────────────────────────────────────────────────────┘
```

---

## Platform Compatibility Matrix

| Feature | Windows | macOS | Linux (X11) | Linux (Wayland) |
|---|---|---|---|---|
| Global hotkey | `keyboard` lib | `pynput` | `pynput` | `pynput` ⚠️ limited |
| Text capture (primary) | Windows UIA | Accessibility API | `xdotool` + PRIMARY selection | limited |
| Text capture (fallback) | `Ctrl+C` clipboard | `Cmd+C` clipboard | `Ctrl+C` clipboard | `Ctrl+C` clipboard |
| Active app detection | `pygetwindow` + `psutil` | `AppKit` NSWorkspace | `xdotool` + `psutil` | `psutil` only |
| Paste back | `Ctrl+V` | `Cmd+V` | `Ctrl+V` | `Ctrl+V` |
| Permissions required | None | Accessibility access ⚠️ | None | None |
| Overlay UI | Tauri (WebView2) | Tauri (WebKit) | Tauri (WebKit) | Tauri (WebKit) |
| System tray | Yes | Menu bar | Yes | Yes |

**⚠️ macOS note:** Accessibility API access requires explicit user permission via System Settings → Privacy & Security → Accessibility. Quill must guide the user through this on first run. This is a hard OS requirement and cannot be bypassed.

**⚠️ Wayland note:** Global hotkeys and window focus detection are fundamentally restricted by design on Wayland. Phase 1–3 target X11. Wayland is a Phase 4 goal.

---

## Tech Stack

| Layer | Technology | Reason |
|---|---|---|
| Core engine | Python 3.11+ | Best cross-platform ecosystem for desktop automation |
| Platform backends | Per-OS modules behind base classes | Clean separation; each OS uses its best-fit libraries |
| Hotkey (Windows/Linux) | `keyboard` lib | Simple, reliable global hotkeys |
| Hotkey (macOS) | `pynput` | Better macOS compatibility than `keyboard` |
| Text capture (Windows) | `pywinauto` (UIA) + `pyperclip` | Native UIA + clipboard fallback |
| Text capture (macOS) | `osascript` Accessibility API + `pynput` | Best available without private APIs |
| Text capture (Linux) | `xdotool` + `xclip` + `pyperclip` | X11 PRIMARY selection + clipboard fallback |
| Context detection (Windows) | `pygetwindow` + `psutil` | Active window + process info |
| Context detection (macOS) | `AppKit.NSWorkspace` via `pyobjc` | Native active app API |
| Context detection (Linux) | `xdotool` + `psutil` | X11 window name + process info |
| Provider clients | `httpx` (async) | Streaming HTTP, works on all platforms |
| Overlay UI | Tauri v2 + React | Single UI codebase, native webview per OS, <10MB binary |
| IPC | Tauri sidecar + JSON over stdio | Python core ↔ Tauri frontend, cross-platform |
| Config | YAML (`pyyaml`) | Human-editable, version-controllable |
| Packaging | PyInstaller + Tauri bundler | Native installer per platform |

---

## Project Structure

```
quill/
├── README.md
├── QUILL_PROJECT_BRIEF.md          ← this file
├── pyproject.toml                  ← Python deps (uv or pip)
├── config/
│   ├── default.yaml                ← shipped defaults
│   ├── modes.yaml                  ← built-in + user-defined modes
│   └── user.yaml                   ← gitignored, user overrides
├── core/
│   ├── __init__.py
│   ├── main.py                     ← entry point, orchestrator
│   ├── platform.py                 ← OS detection + backend loader
│   ├── prompt_builder.py           ← builds prompt from mode + context
│   ├── streamer.py                 ← handles streaming response chunks
│   └── config_loader.py            ← merges default.yaml + user.yaml
├── platform/
│   ├── __init__.py
│   ├── capture/
│   │   ├── base.py                 ← abstract CaptureBackend
│   │   ├── windows.py              ← UIA + clipboard
│   │   ├── macos.py                ← Accessibility API + clipboard
│   │   └── linux.py                ← xdotool PRIMARY + clipboard
│   ├── context/
│   │   ├── base.py                 ← abstract ContextBackend
│   │   ├── windows.py              ← pygetwindow + psutil
│   │   ├── macos.py                ← AppKit NSWorkspace
│   │   └── linux.py                ← xdotool + psutil
│   ├── hotkey/
│   │   ├── base.py                 ← abstract HotkeyBackend
│   │   ├── windows.py              ← keyboard lib
│   │   ├── macos.py                ← pynput GlobalHotKeys
│   │   └── linux.py                ← pynput GlobalHotKeys
│   ├── replace/
│   │   ├── base.py                 ← abstract ReplaceBackend
│   │   ├── windows.py              ← Ctrl+V via keyboard
│   │   ├── macos.py                ← Cmd+V via pynput
│   │   └── linux.py                ← Ctrl+V via keyboard
│   └── permissions/
│       ├── macos.py                ← Accessibility permission check + prompt
│       └── linux.py                ← xdotool availability check
├── providers/
│   ├── __init__.py
│   ├── base.py                     ← abstract Provider interface
│   ├── openrouter.py               ← default: free tier
│   ├── ollama.py                   ← local model via Ollama
│   ├── openai.py                   ← OpenAI direct
│   └── generic.py                  ← any OpenAI-compatible endpoint
├── ui/                             ← Tauri app (identical across platforms)
│   ├── src-tauri/
│   │   ├── Cargo.toml
│   │   ├── tauri.conf.json
│   │   └── src/
│   │       └── main.rs             ← Tauri entry, sidecar IPC bridge
│   └── src/
│       ├── main.jsx
│       ├── components/
│       │   ├── Overlay.jsx         ← floating popup
│       │   ├── ModeBar.jsx         ← mode buttons
│       │   ├── StreamOutput.jsx    ← streamed text display
│       │   ├── Settings.jsx        ← provider + mode config UI
│       │   └── FirstRun.jsx        ← setup wizard (provider + permissions)
│       └── styles/
│           └── overlay.css
├── tests/
│   ├── test_prompt_builder.py
│   ├── test_providers.py
│   ├── test_config_loader.py
│   └── platform/
│       ├── test_windows_capture.py   ← skipped on non-Windows
│       ├── test_macos_capture.py     ← skipped on non-macOS
│       └── test_linux_capture.py     ← skipped on non-Linux
└── scripts/
    ├── install-windows.ps1         ← Windows one-liner installer
    ├── install-macos.sh            ← macOS one-liner installer
    ├── install-linux.sh            ← Linux one-liner installer
    └── build.sh                    ← cross-platform build script
```

---

## Platform Abstraction Layer

Every platform-specific feature sits behind a base class. The core engine only ever talks to the base interface — it never imports OS-specific code directly.

### `core/platform.py` — Backend Loader

```python
import platform as _platform

def get_backends() -> dict:
    """Load the correct backend implementations for the current OS."""
    os_name = _platform.system()  # 'Windows' | 'Darwin' | 'Linux'

    if os_name == "Windows":
        from platform.capture.windows import WindowsCapture
        from platform.context.windows import WindowsContext
        from platform.hotkey.windows import WindowsHotkey
        from platform.replace.windows import WindowsReplace
        return {
            "capture": WindowsCapture(),
            "context": WindowsContext(),
            "hotkey":  WindowsHotkey(),
            "replace": WindowsReplace(),
        }

    elif os_name == "Darwin":
        from platform.permissions.macos import check_accessibility_permission
        if not check_accessibility_permission():
            raise PermissionError("accessibility")  # caught by main.py → shows UI prompt

        from platform.capture.macos import MacOSCapture
        from platform.context.macos import MacOSContext
        from platform.hotkey.macos import MacOSHotkey
        from platform.replace.macos import MacOSReplace
        return {
            "capture": MacOSCapture(),
            "context": MacOSContext(),
            "hotkey":  MacOSHotkey(),
            "replace": MacOSReplace(),
        }

    elif os_name == "Linux":
        from platform.capture.linux import LinuxCapture
        from platform.context.linux import LinuxContext
        from platform.hotkey.linux import LinuxHotkey
        from platform.replace.linux import LinuxReplace
        return {
            "capture": LinuxCapture(),
            "context": LinuxContext(),
            "hotkey":  LinuxHotkey(),
            "replace": LinuxReplace(),
        }

    else:
        raise RuntimeError(f"Unsupported OS: {os_name}")
```

### Base Interfaces

**`platform/capture/base.py`**
```python
from abc import ABC, abstractmethod
from typing import Optional

class CaptureBackend(ABC):
    @abstractmethod
    def get_selected_text(self) -> Optional[str]:
        """Return currently selected text, or None if nothing selected."""
        ...
```

**`platform/context/base.py`**
```python
from abc import ABC, abstractmethod

class ContextBackend(ABC):
    @abstractmethod
    def get_active_context(self) -> dict:
        """
        Return dict with keys:
          app  (str): process name without extension
          tone (str): neutral | professional | technical | casual | formal
          hint (str): human-readable e.g. 'email', 'code editor', 'browser'
        """
        ...
```

**`platform/hotkey/base.py`**
```python
from abc import ABC, abstractmethod
from typing import Callable

class HotkeyBackend(ABC):
    @abstractmethod
    def register(self, hotkey: str, callback: Callable) -> None:
        """Register a global hotkey. hotkey format: 'ctrl+shift+space'"""
        ...

    @abstractmethod
    def unregister_all(self) -> None:
        ...
```

**`platform/replace/base.py`**
```python
from abc import ABC, abstractmethod

class ReplaceBackend(ABC):
    @abstractmethod
    def paste_text(self, text: str) -> None:
        """
        Write text to clipboard, trigger OS paste shortcut in the
        previously focused window, then restore original clipboard.
        Must restore clipboard even if paste fails.
        """
        ...
```

---

### Key Platform Implementations

**`platform/capture/macos.py`** — Accessibility API + Cmd+C fallback:
```python
import subprocess
import time
import pyperclip
from pynput import keyboard as kb
from .base import CaptureBackend
from typing import Optional

class MacOSCapture(CaptureBackend):
    def get_selected_text(self) -> Optional[str]:
        text = self._try_accessibility()
        if text:
            return text.strip()
        return self._try_clipboard()

    def _try_accessibility(self) -> Optional[str]:
        try:
            script = (
                'tell application "System Events" to return '
                'value of attribute "AXSelectedText" of focused UI element'
            )
            result = subprocess.run(
                ["osascript", "-e", script],
                capture_output=True, text=True, timeout=1
            )
            return result.stdout.strip() or None
        except Exception:
            return None

    def _try_clipboard(self) -> Optional[str]:
        try:
            original = pyperclip.paste()
            pyperclip.copy("")
            ctrl = kb.Controller()
            with ctrl.pressed(kb.Key.cmd):
                ctrl.press("c"); ctrl.release("c")
            time.sleep(0.15)
            result = pyperclip.paste()
            pyperclip.copy(original)
            return result or None
        except Exception:
            return None
```

**`platform/capture/linux.py`** — xdotool PRIMARY selection + Ctrl+C fallback:
```python
import subprocess
import time
import pyperclip
from .base import CaptureBackend
from typing import Optional

class LinuxCapture(CaptureBackend):
    def get_selected_text(self) -> Optional[str]:
        text = self._try_primary_selection()
        if text:
            return text.strip()
        return self._try_clipboard()

    def _try_primary_selection(self) -> Optional[str]:
        # X11 PRIMARY selection = whatever is highlighted, no Ctrl+C needed
        try:
            result = subprocess.run(
                ["xclip", "-o", "-selection", "primary"],
                capture_output=True, text=True, timeout=1
            )
            return result.stdout.strip() or None
        except Exception:
            return None

    def _try_clipboard(self) -> Optional[str]:
        try:
            import keyboard
            original = pyperclip.paste()
            pyperclip.copy("")
            keyboard.send("ctrl+c")
            time.sleep(0.15)
            result = pyperclip.paste()
            pyperclip.copy(original)
            return result or None
        except Exception:
            return None
```

**`platform/context/macos.py`** — AppKit NSWorkspace:
```python
from AppKit import NSWorkspace
from .base import ContextBackend

APP_CONTEXT_MAP = {
    "mail":       {"tone": "professional", "hint": "email"},
    "outlook":    {"tone": "professional", "hint": "email"},
    "xcode":      {"tone": "technical",    "hint": "code editor"},
    "vscode":     {"tone": "technical",    "hint": "code editor"},
    "safari":     {"tone": "neutral",      "hint": "browser"},
    "chrome":     {"tone": "neutral",      "hint": "browser"},
    "slack":      {"tone": "casual",       "hint": "chat"},
    "messages":   {"tone": "casual",       "hint": "chat"},
    "word":       {"tone": "formal",       "hint": "document"},
    "pages":      {"tone": "formal",       "hint": "document"},
    "notion":     {"tone": "neutral",      "hint": "notes"},
}

class MacOSContext(ContextBackend):
    def get_active_context(self) -> dict:
        try:
            app = NSWorkspace.sharedWorkspace().frontmostApplication()
            name = app.localizedName().lower()
            for key, ctx in APP_CONTEXT_MAP.items():
                if key in name:
                    return {"app": name, **ctx}
            return {"app": name, "tone": "neutral", "hint": "general app"}
        except Exception:
            return {"app": "unknown", "tone": "neutral", "hint": "general"}
```

**`platform/hotkey/macos.py`** — pynput GlobalHotKeys:
```python
from pynput import keyboard
from typing import Callable
from .base import HotkeyBackend

class MacOSHotkey(HotkeyBackend):
    def __init__(self):
        self._listener = None

    def register(self, hotkey: str, callback: Callable) -> None:
        combo = self._to_pynput(hotkey)
        self._listener = keyboard.GlobalHotKeys({combo: callback})
        self._listener.start()

    def _to_pynput(self, hotkey: str) -> str:
        # "cmd+shift+space" → "<cmd>+<shift>+<space>"
        special = {"cmd", "ctrl", "alt", "shift", "space", "tab", "esc"}
        parts = hotkey.lower().split("+")
        return "+".join(f"<{p}>" if p in special else p for p in parts)

    def unregister_all(self) -> None:
        if self._listener:
            self._listener.stop()
            self._listener = None
```

**`platform/permissions/macos.py`** — Accessibility check + prompt:
```python
import subprocess

def check_accessibility_permission() -> bool:
    """True if Quill has Accessibility API access."""
    try:
        result = subprocess.run(
            ["osascript", "-e",
             'tell application "System Events" to return name of first process'],
            capture_output=True, timeout=2
        )
        return result.returncode == 0
    except Exception:
        return False

def open_accessibility_settings() -> None:
    """Open the Accessibility pane in System Settings."""
    subprocess.run([
        "open",
        "x-apple.systempreferences:"
        "com.apple.preference.security?Privacy_Accessibility"
    ])
```

---

## macOS First-Run Permission Flow

Shown automatically when `check_accessibility_permission()` returns False:

```
┌─────────────────────────────────────────────┐
│  One permission needed                      │
│                                             │
│  To read selected text from other apps,    │
│  macOS requires Accessibility access.       │
│                                             │
│  1. Click "Open System Settings" below     │
│  2. Find "Quill" in the list               │
│  3. Toggle it ON                           │
│  4. Relaunch Quill                         │
│                                             │
│  Your text never leaves your Mac unless    │
│  you configure a cloud provider.            │
│                                             │
│  [ Open System Settings ]  [ Learn More ]  │
└─────────────────────────────────────────────┘
```

---

## Configuration System

### `config/default.yaml`

```yaml
# Quill default configuration
# Copy to config/user.yaml to override (user.yaml is gitignored)

provider: openrouter
model: google/gemma-3-27b-it       # Free tier on OpenRouter
api_key: null                       # Set via env QUILL_API_KEY or settings UI
base_url: https://openrouter.ai/api/v1

# null = OS-adaptive: Ctrl+Shift+Space on Windows/Linux, Cmd+Shift+Space on macOS
hotkey: null

language: auto                      # Target language for translation (auto = detect)
overlay_position: near_cursor       # near_cursor | top_right | top_left
stream: true

# Context-aware mode overrides (optional)
# context:
#   outlook:
#     default_mode: formal
#   code:
#     default_mode: technical
```

### `config/modes.yaml`

```yaml
modes:
  rewrite:
    label: "Rewrite"
    icon: "✏️"
    prompt: |
      Rewrite the following text to improve clarity and flow.
      Keep the same meaning and approximate length.
      Return only the rewritten text, no explanation.

  translate:
    label: "Translate"
    icon: "🌐"
    prompt: |
      Translate the following text to {language}.
      If already in {language}, improve the phrasing.
      Return only the translated text, no explanation.

  coach:
    label: "Coach"
    icon: "💡"
    prompt: |
      Give 2-3 specific, actionable suggestions to improve this text's
      clarity, tone, or impact. Be concise and direct.

  shorter:
    label: "Shorter"
    icon: "✂️"
    prompt: |
      Make this 40-50% shorter while keeping the key message.
      Return only the shortened text.

  formal:
    label: "Formal"
    icon: "👔"
    prompt: |
      Rewrite in a professional, formal tone suitable for business.
      Return only the rewritten text.

# Add custom modes here or in config/user.yaml
# custom_modes:
#   legal:
#     label: "Legal"
#     icon: "⚖️"
#     prompt: "Rewrite in formal legal language..."
```

---

## Provider Abstraction

### `providers/base.py`

```python
from abc import ABC, abstractmethod
from typing import AsyncIterator

class BaseProvider(ABC):
    @abstractmethod
    async def stream(self, prompt: str, system: str = "") -> AsyncIterator[str]:
        """Yield streamed response text chunks."""
        ...

    @abstractmethod
    def is_available(self) -> bool:
        """True if provider is configured and reachable."""
        ...
```

### Provider Config Examples

```yaml
# OpenRouter — default (free tier, cloud)
provider: openrouter
model: google/gemma-3-27b-it
api_key: sk-or-xxxx

# Ollama — fully private, local
provider: ollama
model: gemma3:4b
base_url: http://localhost:11434

# OpenAI
provider: openai
model: gpt-4o-mini
api_key: sk-xxxx

# Any OpenAI-compatible endpoint (LM Studio, Jan.ai, Groq, etc.)
provider: generic
model: your-model-name
base_url: http://localhost:1234/v1
api_key: optional
```

---

## Overlay UI (Tauri v2 + React)

The UI is **100% identical across all platforms** — this is Tauri's main advantage.

### Tauri window config

```json
{
  "windows": [{
    "label": "overlay",
    "title": "Quill",
    "width": 420,
    "height": 280,
    "decorations": false,
    "alwaysOnTop": true,
    "transparent": true,
    "resizable": false,
    "visible": false,
    "skipTaskbar": true
  }]
}
```

### IPC Flow (Python ↔ Tauri)

```
Python detects hotkey → captures text → gets app context
    → emits JSON to Tauri via sidecar stdout
    → Tauri positions overlay near cursor, makes it visible
    → user picks mode → Tauri sends choice to Python via stdin
    → Python builds prompt, calls provider, streams chunks
    → each chunk → Tauri renders it in real time
    → user clicks Replace → Tauri signals Python
    → Python calls platform ReplaceBackend → text pasted in original app
    → overlay hides, clipboard restored
```

---

## First Run Experience

```
┌─────────────────────────────────────────────┐
│  Welcome to Quill                           │
│                                             │
│  How would you like to connect?             │
│                                             │
│  ◉ Free cloud (OpenRouter)                  │
│    Quick setup. Needs a free account.       │
│                                             │
│  ○ Local model (Ollama)                     │
│    Fully private. Needs GPU + Ollama.       │
│                                             │
│  ○ My own API key                           │
│    OpenAI, Anthropic, Groq, or any URL.    │
│                                             │
│  [Continue →]                               │
└─────────────────────────────────────────────┘
```

macOS shows an additional screen if Accessibility is not yet granted (see above).

---

## Default Model Recommendations

| Provider | Recommended Model | Notes |
|---|---|---|
| OpenRouter (free) | `google/gemma-3-27b-it` | Best quality on free tier, multilingual |
| OpenRouter (free alt) | `meta-llama/llama-3.3-70b-instruct:free` | Excellent rewrites |
| Ollama (local) | `gemma3:4b` | Fast, ~3GB, good quality/speed |
| Ollama (local, better) | `gemma3:12b` | Better quality, needs 8GB+ VRAM |
| OpenAI | `gpt-4o-mini` | Cheap, very fast |
| Groq (via generic) | `llama-3.3-70b-versatile` | Extremely fast, generous free tier |

---

## Phased Roadmap

### Phase 1 — Windows MVP (2-3 weeks)
- [ ] Platform abstraction layer scaffolding (all base classes)
- [ ] Windows backends: capture (UIA + clipboard), context, hotkey, replace
- [ ] OpenRouter provider (streaming)
- [ ] Ollama provider (streaming)
- [ ] 3 built-in modes: Rewrite, Translate, Coach
- [ ] Tauri overlay: mode buttons + streaming output + copy/replace + Escape to dismiss
- [ ] Config loading (`default.yaml` + `user.yaml` merge, env var override)
- [ ] First run setup wizard

### Phase 2 — Differentiators (+ 2 weeks)
- [ ] Custom modes via `modes.yaml`
- [ ] Per-app mode profile overrides (context-aware defaults)
- [ ] Settings UI (no manual YAML editing)
- [ ] All providers: OpenAI, generic OpenAI-compatible
- [ ] System tray icon + quit/settings menu (Windows)
- [ ] Additional built-in modes: Shorter, Formal, Fix Grammar, Expand

### Phase 3 — macOS Port (+ 2 weeks)
- [ ] macOS backends: capture, context, hotkey, replace
- [ ] First-run Accessibility permission flow
- [ ] Cmd+Shift+Space as OS-adaptive default
- [ ] macOS menu bar icon
- [ ] `.dmg` installer + notarization
- [ ] CI: GitHub Actions matrix build (Windows + macOS)

### Phase 4 — Linux + Polish (+ 2 weeks)
- [ ] Linux backends: capture (xdotool + xclip), context, hotkey, replace
- [ ] Graceful fallback if xdotool not installed
- [ ] `.AppImage` + `.deb` packages
- [ ] Wayland research + limited support
- [ ] Mode sharing (export/import via GitHub Gist URL)
- [ ] Optional local history log (SQLite, opt-in, local only)
- [ ] Auto-update via Tauri updater
- [ ] `winget` + `homebrew` + `apt` packages

---

## Key Design Decisions & Rationale

| Decision | Rationale |
|---|---|
| Tauri v2, not Electron | 10x smaller binary, native webview per OS, Rust safety, same React UI everywhere |
| Python core, not pure Rust | Desktop automation ecosystem (pywinauto, pynput, AppKit) is Python-first; porting later is viable |
| Platform abstraction from day one | Prevents OS-specific code leaking into core; porting becomes mechanical, not architectural |
| OpenRouter as default | Zero local setup; free tier is usable; OpenAI-compatible so swapping providers is trivial |
| YAML config | Human-readable, Git-friendly, diff-able; easier than JSON for non-developers |
| Clipboard as universal fallback | UIA, Accessibility API, and xdotool all have edge cases; clipboard works on every OS |
| Streaming required | Even 1–2s delay feels laggy for an inline overlay; streaming makes latency invisible |
| Hotkey is OS-adaptive | Ctrl+Shift+Space on Windows/Linux, Cmd+Shift+Space on macOS — respects OS conventions |
| X11 before Wayland | Global hotkeys and focus detection are fundamentally restricted on Wayland; X11 ships first |
| macOS permission as hard gate | Accessibility permission is a real OS requirement; surfacing it clearly prevents user confusion |

---

## What Makes This Different (Pitch)

> Most AI writing tools are cloud-locked, subscription-only, or tied to one OS.
>
> Quill is the only free, open-source, **cross-platform** writing assistant that works with **any** AI — cloud or fully local — and adapts to whatever app you're working in. Select text, press a key, done. On Windows, macOS, or Linux.

**Target users:**
- Developers who want AI writing help outside their IDE, on any OS
- Writers who refuse Grammarly's data practices
- Corporate users where cloud AI is restricted by policy
- macOS and Linux users who have zero good options today
- Power users who want full control over their AI stack

---

## Environment Setup for Development

### Windows
```powershell
pip install keyboard pyperclip pywinauto psutil pygetwindow httpx pyyaml
cargo install tauri-cli
python core/main.py          # Python core
cd ui && cargo tauri dev     # Tauri overlay
```

### macOS
```bash
pip install pynput pyperclip pyobjc-framework-AppKit httpx pyyaml
cargo install tauri-cli
python core/main.py
cd ui && cargo tauri dev
```

### Linux
```bash
sudo apt install xdotool xclip
pip install pynput pyperclip keyboard httpx pyyaml
cargo install tauri-cli
python core/main.py
cd ui && cargo tauri dev
```

---

## Notes for Claude Code

- **Platform abstraction is the most important rule** — `core/` must never import from `platform/windows`, `platform/macos`, or `platform/linux` directly. Always go through `core/platform.py` → `get_backends()`
- **Hotkey is OS-adaptive** — if `hotkey: null` in config, resolve to `ctrl+shift+space` on Windows/Linux and `cmd+shift+space` on macOS before registering
- **macOS permission gates startup** — if `platform.system() == "Darwin"` and `check_accessibility_permission()` is False, raise `PermissionError("accessibility")` which `main.py` catches and signals the UI to show the permission screen; do not proceed until granted
- **Linux xdotool is optional** — if `xdotool` is not installed, log a clear warning and fall back to clipboard-only mode; never crash, never hide the warning
- **Clipboard restore is mandatory** — every capture and replace operation that touches the clipboard must save and restore original content, even if an exception occurs (use try/finally)
- **All AI calls must be async + streaming** — no blocking calls on the main thread
- **Escape always dismisses the overlay** — implement this on the Tauri/React side, not Python
- **Config priority:** env vars → `config/user.yaml` → `config/default.yaml`
- **API keys** go only in `config/user.yaml` (gitignored) or environment variables — never in `default.yaml`, never logged or printed
- **Platform tests** live in `tests/platform/` and are skipped on non-matching OS via `@pytest.mark.skipif(platform.system() != "Windows", reason="Windows only")`
- **IPC contract** — Python emits newline-delimited JSON to stdout; Tauri reads via sidecar. Each message has a `type` field: `show_overlay`, `stream_chunk`, `stream_done`, `error`. Tauri sends back `mode_selected`, `replace_confirmed`, `dismissed`
