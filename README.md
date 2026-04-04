# 🪶 Quill

> **Privacy-first, model-agnostic AI writing assistant for Windows, macOS, and Linux**

Select text anywhere → press a hotkey → instantly rewrite, translate, coach, and more.
Works with any AI model — cloud or fully local. Free and open-source (MIT).

---

## Table of Contents

- [Why Quill?](#why-quill)
- [Quick Start](#quick-start)
- [How It Works](#how-it-works)
- [Modes](#modes)
- [Language Picker](#language-picker)
- [Mode Chaining](#mode-chaining)
- [Keyboard Shortcuts](#keyboard-shortcuts)
- [My Voice (Persona)](#my-voice-persona)
- [AI Tutor](#ai-tutor)
- [AI Providers](#ai-providers)
- [Configuration Reference](#configuration-reference)
- [Custom Modes & Chains](#custom-modes--chains)
- [Development Setup](#development-setup)
- [Platform Notes](#platform-notes)
- [Architecture](#architecture)

---

## Why Quill?

| Tool | Problem |
|---|---|
| GitHub Copilot / MS Copilot | Paywalled, cloud-only, Windows/browser only |
| Grammarly | Subscription, reads all your text, no custom prompts |
| Browser extensions | Only works in the browser |
| Samsung AI Keyboard | No desktop equivalent exists |

**Quill's differentiator:** global hotkey + floating overlay + context-awareness + bring-your-own-model + runs everywhere. Zero lock-in — swap models without touching code.

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
> ⚠️ macOS requires **Accessibility permission** on first run. Quill will guide you through it.

### Windows (PowerShell)
```powershell
powershell -ExecutionPolicy Bypass -File scripts\install-windows.ps1
quill
```

On first launch a **setup wizard** walks you through choosing a provider and entering your API key. You can change everything later in Settings.

---

## How It Works

```
1. Select text in any app
2. Press  Ctrl+Shift+Space  (Windows/Linux)
              or
          Cmd+Shift+Space   (macOS)
3. The Quill overlay appears
4. Pick a language (optional) → pick a mode → result streams in
5. Click  ↩ Replace  to paste back, or  ⎘ Copy
```

The overlay is **non-destructive** — your original text is never modified until you click Replace.

---

## Modes

Seven built-in modes, all accessible with keyboard shortcuts `1`–`7`:

| Key | Icon | Mode | What it does |
|---|---|---|---|
| `1` | ✏️ | **Rewrite** | Improve clarity and flow, same meaning |
| `2` | 🌐 | **Translate** | Translate to the selected language |
| `3` | 💡 | **Coach** | 2–3 actionable suggestions to improve the text |
| `4` | ✂️ | **Shorter** | Cut length by 40–50%, keep the key message |
| `5` | 👔 | **Formal** | Business-appropriate professional tone |
| `6` | 📝 | **Fix Grammar** | Correct spelling, grammar, and punctuation silently |
| `7` | 📖 | **Expand** | Double the length with more detail and depth |

Quill also detects the **active app** (VS Code, Slack, Outlook, etc.) and adjusts the default tone automatically — a rewrite in Slack sounds casual; the same rewrite in Outlook sounds professional.

---

## Language Picker

A language chip bar sits above the mode buttons. Pick any language and **every mode outputs in that language** — not just Translate.

**Quick languages:** Auto · 🇫🇷 French · 🇪🇸 Spanish · 🇩🇪 German · 🇵🇹 Portuguese · 🇮🇹 Italian · 🇯🇵 Japanese · 🇨🇳 Chinese · 🇸🇦 Arabic · 🇳🇱 Dutch · 🇰🇷 Korean

**Any other language:** click **+ Other** and type it (Polish, Hindi, Swahili, etc.).

Your language choice is **remembered** between sessions.

### Examples

| You write in | Language picker | Mode | Result |
|---|---|---|---|
| English | 🇫🇷 French | Rewrite | Clarity-improved text in French |
| English | 🇩🇪 German | Formal | Business email in German |
| French | Auto | Fix Grammar | Corrected French (language preserved) |
| Any | 🇯🇵 Japanese | Shorter | Concise Japanese summary |

---

## Mode Chaining

Chains run two modes back-to-back automatically. The output of step 1 becomes the input of step 2.

Three built-in chains (shown in the mode bar with a dashed border):

| Chain | Steps | Use case |
|---|---|---|
| **Fix → Formal** | Fix Grammar → Formal | Clean up errors, then polish for business |
| **Fix → Translate** | Fix Grammar → Translate | Correct the source, then translate cleanly |
| **Polish → Short** | Rewrite → Shorter | Improve quality, then distil to essentials |

Progress dots animate as each step completes. You can define your own chains — see [Custom Modes & Chains](#custom-modes--chains).

---

## Keyboard Shortcuts

All shortcuts work when the overlay is focused.

| Key | Action |
|---|---|
| `1` – `7` | Trigger mode by index (shown in button tooltips) |
| `r` | Retry — re-run the last mode with a new generation |
| `Esc` | Dismiss the overlay |
| `⊞` (button) | Toggle diff view on/off |
| `↻` (button) | Same as `r` — retry |

### One-off Instruction

Click **✍️ Add instruction…** above the mode bar to type a one-off prompt that gets prepended to the mode:

```
"make it more urgent"
"keep under 50 words"
"use bullet points"
"address the reader as 'you'"
```

The instruction applies to that invocation only (including retries within the same session).

---

## Diff View

After any transformation, click **⊞** in the action bar to toggle a word-level diff:

- 🟢 **Green highlight** — words added
- 🔴 **Red strikethrough** — words removed
- **Stats row** — `42→28 words (−14)`

Diff view makes it easy to see exactly what the AI changed before deciding to replace.

---

## My Voice (Persona)

Configure a personal writing style that applies to **all modes**. When enabled, Quill sounds like you — not like a generic AI.

**Enable in Settings → My Voice tab.**

### Tone presets

| Tone | Description |
|---|---|
| Natural | Let the mode guide the tone (default) |
| Casual | Friendly, conversational |
| Professional | Polished, business-appropriate |
| Witty | Clever, light humour — never forced |
| Direct | Extremely concise, zero fluff |
| Warm | Empathetic, human |

### Style notes

Free text describing how you write:

```
Short punchy sentences. I use em-dashes for emphasis.
I avoid passive voice and always get to the point quickly.
```

### Always avoid

Comma-separated words or patterns the AI should never use:

```
corporate buzzwords, passive voice, exclamation marks, "utilize"
```

The **Live preview** in Settings shows exactly what gets injected into the system prompt so there are no surprises.

---

## AI Tutor

The AI Tutor turns Quill into a language learning companion. It learns from your usage and explains changes in plain language.

**Enable in Settings → AI Tutor tab** (requires History to be enabled first).

### Explain changes

After any transformation, click **💡 Explain what changed & why** in the overlay. The tutor will:

1. List the 2–3 most significant changes made
2. Name the specific rule or principle behind each one
3. Give a tip you can apply yourself next time
4. If a translation was involved, highlight an interesting linguistic difference

### Daily & Weekly Lessons

Open the **🎓 Tutor panel** (overlay header icon or tray → AI Tutor…) and generate personalised lessons:

- **Daily insight** — What you worked on today, one key rule, one tip
- **Weekly review** — Patterns across the week, language corner for your target language

Lessons are **anchored to your actual text** — not generic grammar rules. If you translated to French 8 times this week, the lesson will reference specific French constructs from your own sentences.

### History

The History tab shows every past transformation:

- Expand any entry to see the before/after
- Click **Show diff** for a word-level diff
- Click **💡 Explain changes** to get a tutor explanation for that specific entry

All history is stored **locally** in `~/.quill/history.db`. Nothing is sent anywhere.

### Privacy

| What | Where | Leaves device? |
|---|---|---|
| History database | `~/.quill/history.db` | Never |
| API key | `config/user.yaml` | Never |
| Text you transform | Sent to your chosen AI provider | Only when you press a mode |
| Lesson content | Generated by your AI provider | Prompt sent, response returned |

---

## AI Providers

Quill works with any OpenAI-compatible API. Configure in `config/user.yaml` or via **Settings → AI Provider**.

| Provider | Best for | Default model |
|---|---|---|
| **OpenRouter** *(default)* | Free tier, zero setup | `google/gemma-3-27b-it` |
| **Ollama** | 100% offline, full privacy | `gemma3:4b` |
| **OpenAI** | Best quality | `gpt-4o-mini` |
| **Custom endpoint** | LM Studio, Groq, Jan.ai, etc. | Any OpenAI-compatible |

### OpenRouter (recommended for getting started)

1. Create a free account at [openrouter.ai](https://openrouter.ai)
2. Generate an API key
3. Enter it in the first-run wizard or Settings → AI Provider

Free tier includes `google/gemma-3-27b-it` (excellent quality) and `meta-llama/llama-3.3-70b-instruct:free`.

### Ollama (fully private)

1. Install Ollama from [ollama.com](https://ollama.com)
2. Pull a model: `ollama pull gemma3:4b`
3. In Settings → AI Provider, select **Ollama (local)**

No internet required after the model is downloaded. Your text never leaves your machine.

### Groq (very fast, generous free tier)

Use the **Custom endpoint** provider:
```yaml
provider: generic
model: llama-3.3-70b-versatile
base_url: https://api.groq.com/openai/v1
api_key: gsk_xxxx
```

---

## Configuration Reference

All configuration lives in `config/user.yaml` (gitignored — safe for API keys).
Copy from `config/default.yaml` and override only what you need.

```yaml
# ── AI Provider ───────────────────────────────────────────────────────────────
provider: openrouter          # openrouter | ollama | openai | generic
model: google/gemma-3-27b-it
api_key: sk-or-xxxx           # or set env var QUILL_API_KEY
base_url: https://openrouter.ai/api/v1

# ── Behaviour ─────────────────────────────────────────────────────────────────
hotkey: null                  # null = OS default (Ctrl/Cmd+Shift+Space)
language: auto                # default output language (overridden by overlay picker)
overlay_position: near_cursor # near_cursor | top_right | top_left
stream: true

# ── My Voice ──────────────────────────────────────────────────────────────────
persona:
  enabled: false
  tone: natural               # natural | casual | professional | witty | direct | warm
  style: ""                   # "Short punchy sentences. I use em-dashes."
  avoid: ""                   # "passive voice, corporate buzzwords"

# ── History & AI Tutor ────────────────────────────────────────────────────────
history:
  enabled: false              # stores transformations in ~/.quill/history.db
  max_entries: 10000

tutor:
  enabled: false              # requires history.enabled: true
  auto_explain: false         # automatically explain every transformation

# ── Context-aware overrides (optional) ───────────────────────────────────────
# context:
#   outlook:
#     default_mode: formal
#   code:
#     default_mode: technical
```

### Environment variables

All sensitive values can be set via environment instead of the config file:

| Variable | Overrides |
|---|---|
| `QUILL_API_KEY` | `api_key` |
| `QUILL_PROVIDER` | `provider` |
| `QUILL_MODEL` | `model` |
| `QUILL_BASE_URL` | `base_url` |

---

## Custom Modes & Chains

### Custom modes

Add to `config/user.yaml`:

```yaml
custom_modes:
  legal:
    label: "Legal"
    icon: "⚖️"
    prompt: |
      Rewrite in formal legal language. Use precise, unambiguous terms.
      Return only the rewritten text.

  simplify:
    label: "Simplify"
    icon: "🧒"
    prompt: |
      Rewrite so a 12-year-old can understand it easily.
      Use short sentences and everyday words.
      Return only the simplified text.

  tweet:
    label: "Tweet"
    icon: "🐦"
    prompt: |
      Rewrite as an engaging tweet under 280 characters.
      Keep it punchy. No hashtags unless natural.
      Return only the tweet text.
```

### Custom chains

```yaml
custom_chains:
  simplify_translate:
    label: "Simplify → Translate"
    icon: "🧒🌐"
    steps: [simplify, translate]
    description: "Simplify first, then translate to target language"

  full_polish:
    label: "Full Polish"
    icon: "✨"
    steps: [fix_grammar, rewrite, formal]
    description: "Grammar → clarity → business tone"
```

Custom modes and chains appear immediately in the overlay after saving — no restart needed.

---

## Development Setup

### Prerequisites

- Python 3.11+
- Node.js 18+
- Rust + Cargo ([rustup.rs](https://rustup.rs))

### Install and run

```bash
# Clone
git clone https://github.com/mariesqu/Quill
cd Quill

# Python deps
pip install -e ".[dev]"        # installs core + dev extras

# Add your API key
cp config/default.yaml config/user.yaml
# Edit config/user.yaml and set api_key

# Terminal 1 — Python core (sidecar)
python -m core.main

# Terminal 2 — Tauri dev server
cd ui
npm install
npm run tauri dev
```

### Platform-specific extras

**macOS:**
```bash
pip install pyobjc-framework-AppKit
```

**Linux:**
```bash
sudo apt install xdotool xclip
pip install keyboard
```

**Windows:**
```bash
pip install pywinauto pygetwindow keyboard
```

### Tests

```bash
pytest                        # all tests (platform tests auto-skipped)
pytest tests/test_prompt_builder.py -v
pytest tests/test_providers.py -v
```

### Build for distribution

```bash
bash scripts/build.sh
# Output: ui/src-tauri/target/release/bundle/
```

---

## Platform Notes

| Feature | Windows | macOS | Linux (X11) | Linux (Wayland) |
|---|---|---|---|---|
| Global hotkey | `keyboard` lib | `pynput` | `pynput` | ⚠️ limited |
| Text capture | UIA + clipboard | Accessibility API + clipboard | xclip PRIMARY + clipboard | clipboard only |
| Active app detection | pygetwindow + psutil | AppKit NSWorkspace | xdotool + psutil | psutil only |
| Paste back | Ctrl+V | Cmd+V | Ctrl+V | Ctrl+V |
| Permissions needed | None | Accessibility ⚠️ | None | None |
| Installer | `.exe` | `.dmg` | `.AppImage` / `.deb` | — |

**macOS — Accessibility permission:**
Required so Quill can read selected text from other apps. On first run, Quill shows a step-by-step guide:
1. Click "Open System Settings"
2. Find Quill in the Accessibility list
3. Toggle it ON
4. Relaunch Quill

This is a macOS OS-level requirement — it cannot be bypassed.

**Linux — xdotool:**
If `xdotool` is not installed, Quill falls back to clipboard-only capture (works everywhere, slightly slower). Install for best experience: `sudo apt install xdotool xclip`

**Linux — Wayland:**
Global hotkeys and window focus detection are fundamentally restricted on Wayland by design. X11 is recommended for Phase 1. Wayland support is on the Phase 4 roadmap.

---

## Architecture

```
┌─────────────────────────────────────────────────┐
│              OS System Layer                    │
│  Global Hotkey │ Text Selection │ Active App    │
└────────────────────────┬────────────────────────┘
                         │
┌────────────────────────▼────────────────────────┐
│        Platform Abstraction (platform_/)        │
│  capture/ │ context/ │ hotkey/ │ replace/       │
│  Windows · macOS · Linux backends               │
└────────────────────────┬────────────────────────┘
                         │
┌────────────────────────▼────────────────────────┐
│           Core Engine (Python 3.11+)            │
│  main.py · config_loader · prompt_builder       │
│  history · tutor · streamer                     │
└────────────────────────┬────────────────────────┘
                         │ JSON over stdio (sidecar IPC)
┌────────────────────────▼────────────────────────┐
│         Overlay UI (Tauri v2 + React)           │
│  Overlay · DiffView · TutorPanel · Settings     │
│  FirstRun wizard · PermissionPrompt             │
└─────────────────────────────────────────────────┘
```

- **Core:** Python 3.11+, async streaming via `httpx`
- **UI:** Tauri v2 + React, dark glassmorphism design, ~8MB binary
- **IPC:** Newline-delimited JSON over stdio (Python sidecar ↔ Tauri)
- **History:** SQLite at `~/.quill/history.db`, opt-in
- **Config:** YAML — human-editable, diff-able, version-controllable

---

## License

MIT — see [LICENSE](LICENSE)
