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

### Download a release

Grab the latest installer for your OS from the [Releases page](https://github.com/mariesqu/Quill/releases):

- **Windows** — `Quill_x.y.z_x64-setup.exe` (MSI / NSIS installer)
- **macOS** — `Quill_x.y.z_universal.dmg`
- **Linux** — `quill_x.y.z_amd64.AppImage` or `.deb`

> ⚠️ macOS requires **Accessibility permission** on first run — Quill will guide you through it.

### Build from source

```bash
git clone https://github.com/mariesqu/Quill
cd Quill/ui
npm install
npm run tauri dev          # hot-reload dev build
# or
npm run tauri build        # production bundle in ui/src-tauri/target/release/bundle/
```

Prerequisites: [Rust](https://rustup.rs) (stable) and Node.js 18+. Tauri's [prerequisites page](https://v2.tauri.app/start/prerequisites/) covers per-OS system dependencies (webkit2gtk on Linux, Xcode CLT on macOS, WebView2 on Windows — usually already installed).

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

- [Rust](https://rustup.rs) (stable, edition 2021)
- Node.js 18+ and npm
- Tauri system dependencies — follow [v2.tauri.app/start/prerequisites](https://v2.tauri.app/start/prerequisites/):
  - **Windows** — Microsoft C++ Build Tools + WebView2 (Edge ships with it)
  - **macOS** — Xcode Command Line Tools
  - **Linux** — `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `libgtk-3-dev`, `libxdo-dev`, `patchelf`

### Run in development

```bash
git clone https://github.com/mariesqu/Quill
cd Quill

# Copy default config and add your API key
cp config/default.yaml config/user.yaml
# Edit config/user.yaml and set api_key

cd ui
npm install
npm run tauri dev              # launches the app with hot-reload
```

Tauri builds the Rust backend (`ui/src-tauri/`) and serves the React frontend (`ui/src/`) from Vite. The first build downloads and compiles every crate — allow a few minutes.

### Quality gates

```bash
cd ui/src-tauri
cargo fmt --all -- --check     # formatting
cargo clippy --all-targets -- -D warnings
cargo test --all-features
cargo check                    # fast type check without building
```

Frontend:
```bash
cd ui
npm run build                  # Vite production build
```

### Build for distribution

```bash
cd ui
npm run tauri build
# Output: ui/src-tauri/target/release/bundle/
#   - Windows: .msi / .exe
#   - macOS:   .dmg / .app
#   - Linux:   .AppImage / .deb
```

---

## Platform Notes

| Feature | Windows | macOS | Linux (X11) | Linux (Wayland) |
|---|---|---|---|---|
| Global hotkey | `tauri-plugin-global-shortcut` | `tauri-plugin-global-shortcut` | `tauri-plugin-global-shortcut` | ⚠️ limited |
| Text capture | clipboard simulation (`arboard`) | clipboard simulation (`arboard`) | clipboard simulation (`arboard`) | clipboard only |
| Active app detection | Win32 API | `osascript` | `xdotool` | limited |
| Paste back | `enigo` → Ctrl+V | `enigo` → Cmd+V | `enigo` → Ctrl+V | `enigo` → Ctrl+V |
| Permissions needed | None | Accessibility ⚠️ | None | None |
| Installer | `.msi` / `.exe` | `.dmg` / `.app` | `.AppImage` / `.deb` | — |

**macOS — Accessibility permission:**
Required so Quill can simulate paste-back via `enigo`. On first run, Quill shows a step-by-step guide:
1. Click "Open System Settings"
2. Find Quill in the Accessibility list
3. Toggle it ON
4. Relaunch Quill

This is a macOS OS-level requirement — it cannot be bypassed.

**Linux — xdotool:**
Used for active-app detection. If not installed, Quill still works but context-aware tone defaults are disabled. Install for best experience: `sudo apt install xdotool xclip`

**Linux — Wayland:**
Global hotkeys and window focus detection are fundamentally restricted on Wayland by design. X11 is recommended. Wayland support depends on upstream `tauri-plugin-global-shortcut` improvements.

---

## Architecture

```
┌─────────────────────────────────────────────────┐
│              OS System Layer                    │
│  Global Hotkey │ Text Selection │ Active App    │
└────────────────────────┬────────────────────────┘
                         │
┌────────────────────────▼────────────────────────┐
│   Platform Abstraction (src-tauri/src/platform) │
│  capture.rs  │  context.rs  │  replace.rs       │
│  Win32 · osascript · xdotool backends           │
└────────────────────────┬────────────────────────┘
                         │
┌────────────────────────▼────────────────────────┐
│         Core Engine (Rust — src-tauri/src)      │
│  engine.rs · core/{config,modes,prompt,         │
│    history,tutor,clipboard}.rs · providers/     │
│  Single native binary — no Python, no sidecar   │
└────────────────────────┬────────────────────────┘
                         │  tauri::command + events
┌────────────────────────▼────────────────────────┐
│         Overlay UI (Tauri v2 + React 18)        │
│  windows/MiniOverlay · windows/FullPanel        │
│  DiffView · ComparisonView · FirstRun           │
└─────────────────────────────────────────────────┘
```

- **Backend:** Rust (edition 2021) compiled into the Tauri binary — no sidecar, no external process. Async streaming via `reqwest` + `futures-util`.
- **UI:** Tauri v2 + React 18 two-window design (`mini` 380×560 quick-access overlay, `full` 600×740 studio panel).
- **IPC:** Direct `tauri::command` invocations + typed events (`quill://stream_chunk`, `quill://stream_done`, etc.). No JSON-over-stdio, no Python.
- **Providers:** `Provider` trait with OpenRouter, OpenAI, Ollama, and generic OpenAI-compatible implementations — all with real SSE streaming.
- **History:** Bundled SQLite via `rusqlite` (no system dependency) at `~/.quill/history.db`, opt-in.
- **Config:** YAML (`config/default.yaml`, `config/user.yaml`) — human-editable, diff-able, env-overridable.
- **Key crates:** `tauri`, `tauri-plugin-global-shortcut`, `tauri-plugin-clipboard-manager`, `reqwest`, `tokio`, `rusqlite`, `serde_yaml`, `arboard`, `enigo`, `anyhow`, `tracing`.

---

## License

MIT — see [LICENSE](LICENSE)
