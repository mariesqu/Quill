# 🪶 Quill

> **Privacy-first, model-agnostic AI writing assistant — native Windows**

Select text anywhere → press a hotkey → instantly rewrite, translate, coach, and more.
Works with any AI model — cloud or fully local. Single native `.exe`, warm-editorial amber-on-black aesthetic, vendored typography, analog grain + scanline overlays. Free and open-source (MIT).

---

## Table of Contents

- [Why Quill?](#why-quill)
- [Quick Start](#quick-start)
- [How It Works](#how-it-works)
- [Modes](#modes)
- [Language Picker](#language-picker)
- [Mode Chaining](#mode-chaining)
- [Keyboard Shortcuts](#keyboard-shortcuts)
- [Workspace (Tier 3)](#workspace-tier-3)
- [More Features](#more-features)
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

**Quill's differentiator:** two global hotkeys + three purpose-built windows (ephemeral overlay / command palette / persistent workspace) + floating pencil indicator + context-awareness + bring-your-own-model. Zero lock-in — swap models without touching code. Zero sidecar — single native `quill.exe`.

---

## Quick Start

### Requirements

- **Windows 10 or 11** — UIA, WinEvent hooks, and DWM drop shadow all work on both
- **Rust stable** (via [rustup](https://rustup.rs)) if building from source

### Install via MSI (end users)

Download the `quill-<version>-x86_64.msi` from the [latest Release](https://github.com/mariesqu/Quill/releases/latest) and double-click it. The installer drops `quill.exe` into `Program Files\Quill`, creates a Start Menu shortcut, and registers an Add/Remove Programs entry. Launch from the Start Menu. No additional dependencies to install for the default providers (OpenRouter / Ollama / OpenAI / generic).

### Build from source (developers)

```bash
git clone https://github.com/mariesqu/Quill
cd Quill
cargo build --release
# Binary: target/release/quill.exe
./target/release/quill.exe
```

The first build compiles every crate (Slint, tokio, reqwest, windows-rs, rusqlite) — allow 3–5 minutes. Subsequent builds are incremental and take seconds.

To build the MSI installer locally (requires [WiX Toolset 3.x](https://github.com/wixtoolset/wix3/releases) + `cargo install cargo-wix`):

```bash
cargo build --release
cargo wix --no-build
# MSI lands at target/wix/quill-<version>-x86_64.msi
```

Releases are cut by pushing a `v*` tag; the `release.yml` workflow builds the MSI on `windows-latest` and attaches it to a GitHub Release automatically.

### Configure

Copy `config/default.yaml` to `config/user.yaml` and set your provider and API key (see [AI Providers](#ai-providers) and [Configuration Reference](#configuration-reference)). The config file is gitignored, so your keys stay local.

```bash
cp config/default.yaml config/user.yaml
# edit config/user.yaml → set provider + api_key
```

On launch Quill reads `config/user.yaml`, registers both global hotkeys (overlay + command palette), and installs a tray icon. Every window stays hidden until you interact — hotkey, pencil click, or tray menu item.

---

## How It Works

```
1. Select text in any app (Notepad, Word, VS Code, Outlook, Chrome, …)
2. Press  Ctrl+Shift+Space  (customizable in Settings)
3. The overlay appears NEAR your caret — 460 × 200 px, frameless, always-on-top
4. Pick a language (optional) → pick a mode → result streams in, overlay grows to 380 px
5. Click  REPLACE  to paste back into the app you were typing in
```

Text capture uses **UIA (UI Automation)** as the primary path — zero side effects, works in any app that exposes its text control. When UIA doesn't expose the selection (Chrome, Electron, elevated apps), Quill falls back to simulated `Ctrl+C` → read clipboard → restore, silencing the clipboard monitor during the capture so nothing leaks.

The overlay is **non-destructive** — your original text is never modified until you click REPLACE. When you replace, Quill restores focus to the HWND you were typing in (snapshotted at hotkey time) before simulating `Ctrl+V`, so the paste lands in Outlook / Slack / VS Code, not inside the overlay itself.

### The three-tier window model

Quill uses three purpose-built windows, each with its own role:

| Tier | Window | Purpose | Summon |
|---|---|---|---|
| **1** | Overlay | Ephemeral, frameless, always-on-top, near-caret | `Ctrl+Shift+Space`, pencil click, tray → Show Overlay |
| **2** | Command Palette | Transient fuzzy search across modes, chains, system commands | `Ctrl+Shift+P`, PALETTE button in overlay, tray → Command Palette |
| **3** | Workspace | Persistent tabbed studio (Write / History / Tutor / Compare / Settings) | tray → Open Workspace; close-to-tray (app only quits from tray → Quit Quill) |

Each window has its own Slint `AppBridge` instance — state writes mirror across all three so the workspace's History tab stays in sync while the overlay runs, and the palette footer shows the current selection context.

### Floating pencil

When your cursor lands inside an editable text control, a 32×32 plume icon fades in next to the caret. Clicking it triggers the same capture flow as the hotkey. The pencil is:

- **Always on top** (Slint `always-on-top: true`)
- **Click-through when idle** (`WS_EX_TRANSPARENT`) — the cursor passes through to the underlying app
- **Click-capturing when hovered** — once you move the cursor within 40 px, the transparent flag drops so the click registers
- **Never steals focus** (`WS_EX_NOACTIVATE`)
- **Skipped in the taskbar + Alt-Tab** (`WS_EX_TOOLWINDOW`)

The pencil only appears when UIA reports the focused element is an editable text control. It hides when you Alt-Tab to File Explorer, the taskbar, or any non-editable window.

---

## Modes

Seven built-in modes:

| Icon | Mode | What it does |
|---|---|---|
| ✏️ | **Rewrite** | Improve clarity and flow, same meaning |
| 🌐 | **Translate** | Translate to the selected language |
| 💡 | **Coach** | 2–3 actionable suggestions to improve the text |
| ✂️ | **Shorter** | Cut length by 40–50%, keep the key message |
| 👔 | **Formal** | Business-appropriate professional tone |
| 📝 | **Fix Grammar** | Correct spelling, grammar, and punctuation silently |
| 📖 | **Expand** | Double the length with more detail and depth |

> Modes are activated by clicking; keyboard shortcuts are a follow-up.

Quill also detects the **active app** (VS Code, Slack, Outlook, etc.) and adjusts the default tone automatically — a rewrite in Slack sounds casual; the same rewrite in Outlook sounds professional.

---

## Language Picker

A language chip bar sits above the mode buttons. Pick any language and **every mode outputs in that language** — not just Translate.

**Pinned translate targets on the overlay:** configurable (default EN / FR) — the overlay shows exactly two Translate quick-action buttons for those codes. The workspace's Write tab uses the full 8-language picker (`LangRow`); all surfaces stay in sync via the shared `AppBridge.active-language` property.

**Any other language:** set it via `language:` in `user.yaml`, or extend the `languages` list in `src/ui/slint/app_bridge.slint` (custom language-picker UI is a post-ship follow-up).

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

Three built-in chains (shown in the workspace's Write tab with a hairline solid border and amber-tinted icon — ChainRow hover promotes the border to the cream text colour):

| Chain | Steps | Use case |
|---|---|---|
| **Fix → Formal** | Fix Grammar → Formal | Clean up errors, then polish for business |
| **Fix → Translate** | Fix Grammar → Translate | Correct the source, then translate cleanly |
| **Polish → Short** | Rewrite → Shorter | Improve quality, then distil to essentials |

Progress dots animate as each step completes. You can define your own chains — see [Custom Modes & Chains](#custom-modes--chains).

---

## Keyboard Shortcuts

### Global (work anywhere in Windows)

| Key | Action |
|---|---|
| `Ctrl+Shift+Space` | Capture selection → summon the overlay near the caret |
| `Ctrl+Shift+P` | Summon the command palette (centered on primary monitor) |

Both hotkeys are customisable — see `hotkey` and `hotkey_palette` in the config reference.

### When the overlay is focused

| Key | Action |
|---|---|
| Typing | Edits the captured text in place (one-way bind; edits sync to `AppState.selected_text`) |
| Click **REWRITE** / **TRANSLATE → EN** / **TRANSLATE → FR** | Run the mode on the current text |
| `PALETTE` (button) | Hide overlay, open the command palette |
| `Esc` or **✕** | Dismiss the overlay |
| Drag the header | Reposition the overlay (Win32 `WM_SYSCOMMAND SC_MOVE`) |

The two pinned translate pairs are configurable — see `ui.overlay.pinned_translate` in `user.yaml`.

### When the workspace is focused

| Key | Action |
|---|---|
| Click a tab | Switch to Write / History / Tutor / Compare / Settings |
| `Esc` or OS close (✕) | Hide the workspace to tray (app stays alive; Quit via tray menu) |

### When the palette is focused

| Key | Action |
|---|---|
| Type | Fuzzy-filter the catalog |
| `↑` / `↓` | Move the highlight |
| `Enter` | Activate the highlighted item |
| Click a row | Activate that item |
| `Esc` | Dismiss the palette |

### Tray menu

| Item | Action |
|---|---|
| **Show Overlay** | Summon the overlay (blank scratch prompt) |
| **Command Palette** | Summon the palette |
| **Open Workspace** | Summon the workspace on the **Write** tab |
| **Settings…** | Summon the workspace on the **Settings** tab |
| **Quit Quill** | Shut the app down (the ONLY way to exit) |

---

## Workspace (Tier 3)

The workspace is the "deep work" surface — a persistent, resizable, taskbar-visible window that hosts five tabs. Summon it from the tray (**Open Workspace**, **Settings…**) or the command palette.

| Tab | What it does |
|---|---|
| **Write** | Full mode grid, chain row, and 8-language picker — the overlay only exposes a tight Rewrite button plus the two configured Translate quick-actions. Stream output + REPLACE button |
| **History** | List of past transformations with ★ favorites and native JSON / CSV / Markdown export |
| **Tutor** | "Explain last edit" + daily/weekly lesson generation bound to your actual usage |
| **Compare** | Run two modes on the same input side-by-side and pick the winner |
| **Settings** | Provider, API key (masked), model, hotkey — save writes back to `~/.quill/config/user.yaml` |

The workspace's **X / Alt+F4 / Esc** all hide to tray rather than quitting — matches user expectations for an always-available productivity tool. The app only truly exits from **tray → Quit Quill**.

All session state (selected text, stream buffer, active mode, history) is mirrored into every window's `AppBridge` via `apply_to_all` in the bridge, so you can run a mode in the overlay, open the workspace mid-stream, and watch the same buffer fill in the Write tab.

### History export

From the History tab click **JSON** / **CSV** / **MARKDOWN**. A native Windows save dialog opens (`IFileSaveDialog` via `rfd`) and writes the full history to the file you choose. Favoriting an entry persists to SQLite immediately via `history::toggle_favorite`.

---

## More Features

A handful of smaller tools layered on top of the core rewrite/translate/coach loop.

### Smart mode suggestion

When the overlay opens, Quill runs a heuristic over the selected text + active app and pre-highlights the mode it thinks you want (e.g. a grammar-heavy message in Outlook → **Fix Grammar**, a long Slack draft → **Shorter**). Click the suggestion chip to accept, or ignore it and pick your own mode.

### Clipboard monitor *(opt-in)*

Enable under `clipboard_monitor.enabled` in `user.yaml`. Quill watches the system clipboard and, when new text lands there, surfaces a discreet toast inside the overlay offering to transform it — handy for paste-then-polish workflows. Opt-in because it's a privacy-sensitive sensor.

### Side-by-side comparison

Open the Workspace (tray → Open Workspace or Ctrl+Shift+P → Open Workspace) and switch to the **Compare** tab. Type two mode IDs, click **Compare** — Quill streams both in parallel and shows the results next to each other so you can pick the winner before replacing. Useful when you're torn between, say, **Shorter** and **Rewrite**.

### Reasoning-model support

Quill works transparently with chain-of-thought models (**MiniMax**, **DeepSeek-R1**, **Qwen3**, etc.). Their `<think>…</think>` internal monologue is stripped from the stream **and** from history as tokens flow in, so the overlay shows only the final answer and your tutor lessons stay anchored to the actual output, not the reasoning scratchpad.

---

## My Voice (Persona)

Configure a personal writing style that applies to **all modes**. When enabled, Quill sounds like you — not like a generic AI.

**Enable via `persona.enabled: true` in `~/.quill/config/user.yaml`. The persona is read fresh on each mode invocation.**

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

---

## AI Tutor

The AI Tutor turns Quill into a language learning companion. It learns from your usage and explains changes in plain language.

**Enable via `tutor.enabled: true` in `~/.quill/config/user.yaml` (requires `history.enabled: true`).**

### Explain changes

Open the Workspace (tray → Open Workspace or Ctrl+Shift+P → Open Workspace) and switch to the **Tutor** tab. Click **Explain last edit**. The tutor will:

1. List the 2–3 most significant changes made
2. Name the specific rule or principle behind each one
3. Give a tip you can apply yourself next time
4. If a translation was involved, highlight an interesting linguistic difference

If `tutor.auto_explain: true` in `user.yaml`, every successful transformation triggers the explanation automatically in the background — the Tutor tab is pre-populated the next time you open it.

### Daily & Weekly Lessons

In the **Tutor tab**, click **Daily** or **Weekly** to generate personalised lessons:

- **Daily insight** — What you worked on today, one key rule, one tip
- **Weekly review** — Patterns across the week, language corner for your target language

Lessons are **anchored to your actual text** — not generic grammar rules. If you translated to French 8 times this week, the lesson will reference specific French constructs from your own sentences. Lessons persist to `history.db` so you can reference them later.

### History

The **History tab** in the workspace shows every past transformation with ★ favorites, click-to-toggle-favorite, and native Export JSON / CSV / Markdown buttons. All history is stored **locally** in `~/.quill/history.db`. Nothing is sent anywhere.

> **Coming post-ship:** per-entry detail pane with before/after diff and per-entry explain button. The current tab is list-only.

### Privacy

| What | Where | Leaves device? |
|---|---|---|
| History database | `~/.quill/history.db` | Never |
| API key | `config/user.yaml` | Never |
| Text you transform | Sent to your chosen AI provider | Only when you press a mode |
| Lesson content | Generated by your AI provider | Prompt sent, response returned |

---

## AI Providers

Quill works with any OpenAI-compatible API, a local Ollama daemon, or the Claude Code CLI (ride your Claude subscription, no API key needed). Configure in `config/user.yaml` or via the workspace's **Settings tab** (tray → Settings…). The tab exposes five fields: `PROVIDER` (openrouter | openai | ollama | claude-cli | generic), `API KEY` (password-masked; hidden when the provider doesn't need a key), `MODEL`, `HOTKEY`, `PALETTE HOTKEY`. Persona, tutor, custom modes, and advanced options live in `~/.quill/config/user.yaml` — the tab prompts you to edit the YAML directly for anything beyond those five fields.

| Provider | Best for | Default model |
|---|---|---|
| **OpenRouter** *(default)* | Free tier, zero setup | `google/gemma-3-27b-it` |
| **Ollama** | 100% offline, full privacy | `gemma3:4b` |
| **OpenAI** | Best quality (API key) | `gpt-4o-mini` |
| **Claude CLI** | Your Claude Max/Pro subscription, no API key | `sonnet` |
| **Custom endpoint** | LM Studio, Groq, Jan.ai, etc. | Any OpenAI-compatible |

### OpenRouter (recommended for getting started)

1. Create a free account at [openrouter.ai](https://openrouter.ai)
2. Generate an API key
3. Paste it into the workspace's Settings tab (`API KEY` field, password-masked). If Quill boots without a usable config (empty key / unknown provider), the workspace auto-opens on the Settings tab so you land on the form.

Free tier includes `google/gemma-3-27b-it` (excellent quality) and `meta-llama/llama-3.3-70b-instruct:free`.

### Ollama (fully private)

1. Install Ollama from [ollama.com](https://ollama.com)
2. Pull a model: `ollama pull gemma3:4b`
3. In the Settings tab, type `ollama` into the `PROVIDER` field and `gemma3:4b` (or your pulled model) into `MODEL`

No internet required after the model is downloaded. Your text never leaves your machine.

### Claude CLI (Claude Max / Pro subscription — no API key)

Ride your existing Claude subscription directly — no separate API key, no OpenRouter hop.

1. Install the Claude Code CLI: `npm install -g @anthropic-ai/claude-code`
2. Sign in once: `claude login` (opens a browser for OAuth)
3. In the Settings tab:
   - `PROVIDER` → `claude-cli`
   - `MODEL` → `haiku` (fast), `sonnet` (balanced), or `opus` (max quality). Full model IDs like `claude-sonnet-4-6` also work.
   - `API KEY` is hidden — the CLI uses your logged-in Anthropic session.

Quill invokes the CLI in pure-completion mode: `--system-prompt` replaces Claude Code's agentic system prompt, `--tools ""` disables tool use, `--permission-mode bypassPermissions` skips approval prompts, and the child runs from your temp directory so no project-level `CLAUDE.md` gets picked up. Stream is read byte-by-byte from stdout with UTF-8 boundary safety; the child process dies (`kill_on_drop`) the moment you hit CANCEL.

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
provider: openrouter          # openrouter | ollama | openai | claude-cli | generic
model: google/gemma-3-27b-it  # for claude-cli: haiku | sonnet | opus (or full ID)
api_key: sk-or-xxxx           # or set env var QUILL_API_KEY; ignored for ollama/claude-cli
base_url: https://openrouter.ai/api/v1

# ── Behaviour ─────────────────────────────────────────────────────────────────
hotkey: null                  # null = Ctrl+Shift+Space; override with e.g. "Ctrl+Alt+Q"
hotkey_palette: "Ctrl+Shift+P" # command palette hotkey; null to disable
language: auto                # default output language (overridden by Language row picker)
stream: true

# ── UI ────────────────────────────────────────────────────────────────────────
ui:
  overlay:
    # Two pinned TRANSLATE targets shown as quick-action buttons on the overlay.
    # Labels are derived from the codes (uppercased) automatically.
    pinned_translate:
      a: "en"
      b: "fr"

# ── My Voice ──────────────────────────────────────────────────────────────────
persona:
  enabled: false
  tone: natural               # natural | casual | professional | witty | direct | warm
  style: ""                   # "Short punchy sentences. I use em-dashes."
  avoid: ""                   # "passive voice, corporate buzzwords"

# ── History & AI Tutor ────────────────────────────────────────────────────────
history:
  enabled: true               # default ON; stores transformations in ~/.quill/history.db
  max_entries: 10000          # rolling cap — oldest rows pruned on insert past this

tutor:
  enabled: false              # opt-in; requires history.enabled: true
  auto_explain: false         # automatically explain every transformation
  lesson_language: auto       # focus lessons on this language (auto = most used)

# ── Clipboard monitor ────────────────────────────────────────────────────────
clipboard_monitor:
  enabled: false              # opt-in: watch clipboard and offer to transform new text

# ── Templates ────────────────────────────────────────────────────────────────
# Reserved for a future "quick presets" UI. The field is parsed out of
# user.yaml but not yet surfaced in the workspace — declaring templates
# here is forward-compatible but has no runtime effect today.
templates: []
```

> Active-app awareness (Slack vs Outlook vs VS Code → tone hint) is built into the prompt builder and requires no configuration — Quill adapts automatically based on the foreground application.

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

- Windows 10 or 11
- [Rust stable](https://rustup.rs) (edition 2021)
- Microsoft C++ Build Tools (for `windows-rs` + native compilation)

No Node.js, no npm, no webview, no sidecar. The whole app is one Cargo crate.

### Run in development

```bash
git clone https://github.com/mariesqu/Quill
cd Quill

# Copy default config and add your API key
cp config/default.yaml config/user.yaml
# Edit config/user.yaml and set api_key

cargo run                      # debug build + launch
```

The first build compiles ~300 crates (Slint, tokio, reqwest, windows-rs, rusqlite) — allow 3–5 minutes. Incremental builds are seconds. The `build.rs` step compiles the Slint component graph and rasterises a 32×32 `quill-tray-32.png` into `OUT_DIR` from `resources/icons/plume.svg` via `resvg` — the tray icon is the only generated bitmap and it stays out of the source tree. The exe has no embedded icon resource (no `winres`/`embed_resource` dependency); distribution tooling that wants a file-icon can ship a hand-authored `.ico` alongside.

### Quality gates

```bash
cargo fmt --check
cargo clippy --all-targets --no-deps -- -D warnings
cargo test --all-targets                  # all tests pass
cargo build --release
```

CI runs the same sweep on `windows-latest` — see `.github/workflows/ci.yml`.

### Build for distribution

```bash
cargo build --release
# Output: target/release/quill.exe — LTO + strip + opt-level=s
```

Both dev and release profiles set `panic = "abort"` (see Cargo.toml). Panics die fast — every hot-path `std::sync::Mutex::lock().unwrap()` is audited against the abort invariant, not unwind safety. Binary is statically linked against `rusqlite` (bundled SQLite) and dynamically linked against only the standard Win32 DLLs that ship with Windows 10+. Drop-in distribution — no installer, no external runtime.

### Testing architecture

Platform IO is behind traits (`TextCapture`, `TextReplace`, `ContextProbe`, `Provider`) so the engine can be driven by in-memory fakes. `tests/common/fakes.rs` provides `FakeCapture`, `FakeReplace`, `FakeContext`, `FakeProvider`. `tests/engine_integration.rs` uses them to assert end-to-end event flow without ever touching the OS:

```rust
let fake_capture  = Arc::new(FakeCapture::with_text("Hello"));
let fake_provider = Arc::new(FakeProvider::with_chunks(vec!["Hola", " ", "mundo"]));
// ... build engine with fakes ...
engine.handle_command(UiCommand::ExecuteMode { mode: "rewrite".into(), ... }).await;
// assert UiEvent::StreamStart → StreamChunk* → StreamDone
```

See `tests/engine_integration.rs` for the full harness.

---

## Platform Notes

Quill is Windows-only — the Slint rewrite hardcoded Win32 APIs (UIA for capture, `SetWinEventHook` for caret tracking, DWM for drop shadow on frameless windows, `IFileSaveDialog` for export). Earlier Tauri builds supported macOS and Linux; those are preserved at the `tauri-final` tag in git history if you want to fork.

| Feature | Implementation |
|---|---|
| Window system | `slint` 1.8 with `backend-winit` + `renderer-winit-skia` |
| Warm editorial look | Solid warm-black (`#0d0d0d`) background, amber (`#e8a845`) accent, cream (`#e8e0d0`) text, hairline borders, vendored JetBrains Mono + DM Serif Display + Lora, grain + scanline SVG overlays. Mica is intentionally NOT used — see `src/ui/slint/theme.slint` for the full token set |
| Frameless drop shadow | `DwmSetWindowAttribute(DWMWA_NCRENDERING_POLICY, DWMNCRP_ENABLED)` applied once after first show for every frameless window (overlay / palette / pencil) |
| Global hotkeys | `global-hotkey` crate — TWO `GlobalHotKeyManager` instances (one per binding), polled on the Slint main thread at 60Hz |
| Text capture | `IUIAutomation` primary path — two calls: `selected_text()` for the selection string, `selection_bounds()` for the selection rectangle; `SendInput` Ctrl+C clipboard fallback with clipboard-monitor muting |
| Active app detection | `GetForegroundWindow` → `QueryFullProcessImageNameW` (never the window title — "How to use Gmail — Arc" would false-match) |
| Caret + focus tracking | `SetWinEventHook` on a dedicated thread, forwarded through a `tokio::mpsc` channel, enriched by a UIA worker thread with its own COM apartment |
| Near-caret overlay positioning | `UiEvent::ShowOverlay.anchor_rect` carries the UIA selection bounds (or element bounds fallback); bridge clamps to the primary monitor, flips above the caret if below would overflow |
| Floating pencil | Slint `PencilWindow` with `WS_EX_LAYERED` + `WS_EX_TOOLWINDOW` + `WS_EX_NOACTIVATE` + `WS_EX_TRANSPARENT` — 30 Hz proximity timer toggles `WS_EX_TRANSPARENT` when the cursor is within 40 px |
| Paste back | Snapshot foreground HWND → emit Dismiss → 60 ms OS settle → `SetForegroundWindow` to the original HWND → `enigo` Ctrl+V |
| Tray icon | `tray-icon` crate with Slint-thread-driven menu polling at 75 ms |
| File dialogs | `rfd::FileDialog` → native `IFileSaveDialog` |
| History DB | Bundled SQLite (`rusqlite` with `bundled` feature) at `~/.quill/history.db`, WAL journal mode |
| DPI awareness | `SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)` — called before any window creation |

### Chrome / Electron notes

Chromium-based apps (Chrome, Edge, Electron apps like VS Code or Slack) expose incomplete UIA trees. UIA typically returns empty on Chromium/Electron; Quill then falls back to the clipboard path with the monitor muted — still zero-side-effect because the previous clipboard text is restored after the capture window closes.

---

## Architecture

```
┌───────────────────────────────────────────────────┐
│                   Win32 / DWM                     │
│  GlobalHotKey · UIA · SetWinEventHook · DWM NC    │
│  WS_EX_LAYERED · IFileSaveDialog · Shell Tray     │
└─────────────────────────┬─────────────────────────┘
                          │
┌─────────────────────────▼─────────────────────────┐
│     Platform layer (src/platform/*)               │
│  capture · uia · replace · context · hotkey       │
│  caret · dwm_shadow · tray · traits               │
└─────────────────────────┬─────────────────────────┘
                          │  TextCapture · TextReplace · ContextProbe
┌─────────────────────────▼─────────────────────────┐
│          Engine (src/engine/*)                    │
│  mod.rs (handle_command dispatcher) ·             │
│  hotkey_flow · streaming · compare · tutor_flow   │
│          ▲                              │         │
│          │  mpsc<UiCommand>             │         │
│          │                              ▼         │
│                                   mpsc<UiEvent>   │
└─────────────────────────┬─────────────────────────┘
                          │  Arc<Mutex<AppState>>
┌─────────────────────────▼─────────────────────────┐
│          State (src/state/*)                      │
│  AppState · UiEvent · UiCommand · FocusSnapshot   │
└─────────────────────────┬─────────────────────────┘
                          │
┌─────────────────────────▼─────────────────────────┐
│          UI (src/ui/*)                            │
│  bridge.rs — seed / event-pump / command-forward  │
│                                                   │
│  Three-tier windows:                              │
│   • overlay_window   (Tier 1 — ephemeral)         │
│   • palette_window   (Tier 2 — transient)         │
│   • workspace_window (Tier 3 — persistent)        │
│  Plus pencil_window + pencil_controller           │
│                                                   │
│  slint/ — theme, app_bridge, three Window files,  │
│           components/, tabs/                      │
└───────────────────────────────────────────────────┘
```

### Data flow

- **Hotkey / pencil click** → `engine::hotkey_flow::handle_hotkey` (tokio task) → platform capture via injected `TextCapture` trait → emits `UiEvent::ShowOverlay` on the event channel
- **UI bridge** drains `mpsc<UiEvent>`, hops onto the Slint thread via `slint::invoke_from_event_loop`, and projects each event onto `AppBridge` properties (the Slint global singleton)
- **Slint callbacks** (mode click, confirm replace, tab switch, …) pack arguments into `UiCommand` and send over `mpsc<UiCommand>`
- **Command drain** on tokio picks up each `UiCommand`, calls `Engine::handle_command(cmd).await`, which routes to the right flow module
- **`AppState` is the source of truth** — wrapped in `Arc<Mutex<AppState>>` and shared between the engine (writer) and the bridge (reader). Slint properties are a *projection* of `AppState`; if they drift, state wins on the next event.

### Layer dependency rules

```
main.rs  ──wires──▶  ui, engine, platform, state, core, providers
ui       ──────▶    state, engine, platform (for DWM/HWND only), core (for modes/config)
engine   ──────▶    core, providers, platform, state
state    ──────▶    core
platform ──────▶    (nothing project-internal — only windows-rs + std)
core     ──────▶    (nothing project-internal — only std + 3rd-party pure libs)
providers──────▶    (only reqwest + futures + core for config)
```

One-way dependencies, no cycles. `core/` and `providers/` are trivially unit-testable (no OS, no UI). `platform/` is mockable via the traits in `platform/traits.rs`. The engine is covered by 7 Tier-2 integration tests driven entirely by in-memory fakes.

### Threading map

| Thread | Owns | Work |
|---|---|---|
| Slint main | All four `ComponentHandle`s (overlay / palette / workspace / pencil), tray `TrayService`, bridge event pump callback, proximity timer, two hotkey polling timers | Rendering, UI callbacks, `invoke_from_event_loop` closures, `GlobalHotKeyEvent::receiver` drain |
| Tokio worker pool | Engine inner state | `handle_command`, HTTP streaming, history writes, prompt building, config save (serialized via `config_write_lock`) |
| Caret WinEvent hook | `SetWinEventHook` handle, Win32 message pump | Forwards raw `FocusEvent`s to the UIA worker channel |
| UIA worker (pencil) | Thread-local `Uia` + COM apartment | Enriches raw `FocusEvent`s (editability + element bounds), posts `PencilCmd`s via `invoke_from_event_loop` |

Cross-thread communication is always `Arc<Mutex<_>>` (read-mostly state) or `mpsc::UnboundedChannel` (events + commands). No shared mutable state without synchronization; no `.await` points while holding `std::sync::MutexGuard<AppState>` (the engine is careful about scope). Config writes acquire a dedicated `tokio::sync::Mutex` (`EngineInner::config_write_lock`) so concurrent `SaveConfig` + `SetLanguage` can't lose updates.

### Slint ↔ Rust boundary

`AppBridge` is a Slint `global` — but **per-window, not shared**. Each `ComponentHandle` (`OverlayWindow`, `PaletteWindow`, `WorkspaceWindow`) instantiates its own `AppBridge` with its own backing store. The bridge module in `src/ui/bridge.rs` makes this manageable:

- **`seed_bridge`** — writes modes / chains / settings / pinned translate into every window's AppBridge at startup
- **`spawn_event_pump`** — drains `mpsc<UiEvent>` on tokio, projects each event onto all three AppBridges via `invoke_from_event_loop`. For `StreamChunk` events it reads the canonical buffer from `AppState` once and broadcasts the same `SharedString` to every window — O(N) instead of the naive O(N²) that would result from per-window get-modify-set round-trips.
- **`install_command_forwarder`** — registers every `AppBridge.on_*` callback on the overlay and workspace (palette has its own `PaletteBridge`); each callback packs arguments into a `UiCommand` and sends via `mpsc<UiCommand>`.

Zero IPC, zero serialization — Slint and Rust share the same address space.

### Key crates

- **UI**: `slint` 1.8 (`backend-winit` + `renderer-winit-skia` + `raw-window-handle-06`), `slint-build`
- **Platform**: `windows` 0.58 (Win32 + DWM + UIA + Threading), `raw-window-handle` 0.6, `global-hotkey` 0.5, `tray-icon` 0.14, `enigo` 0.2, `arboard` 3, `rfd` 0.14
- **Runtime**: `tokio` (full), `futures-util`, `async-trait`
- **HTTP**: `reqwest` 0.12 with `stream` + `json` — shared `HTTP_CLIENT` has a 10 s TCP/TLS connect timeout
- **Persistence**: `rusqlite` 0.31 with `bundled` (no system SQLite); WAL journal
- **Config**: `serde` + `serde_json` + `serde_yaml`
- **Build**: `resvg` 0.42 (build.rs rasterises `resources/icons/plume.svg` into a 32×32 `quill-tray-32.png` inside `OUT_DIR` — consumed by `src/platform/tray.rs` via `include_bytes!`)
- **Logging**: `tracing` + `tracing-subscriber` + `tracing-appender` (synchronous file writes to `~/.quill/quill.log.YYYY-MM-DD` via the `daily` rolling appender — no async buffering, every line flushed immediately)
- **Reasoning-model support**: streaming `ThinkFilter` strips `<think>…</think>` blocks from chain-of-thought models (MiniMax, DeepSeek-R1, Qwen3, …) before tokens reach the UI or history

---

## License

MIT — see [LICENSE](LICENSE)
