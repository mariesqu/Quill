# Quill — Project Brief
> Privacy-first, model-agnostic AI writing assistant for Windows, macOS, and Linux

---

> ## 📜 Historical document
>
> This brief was written during **Phase 1** when Quill was a Python sidecar talking to a Tauri frontend over stdio JSON. That architecture was **fully replaced in v0.2.0** by a native Rust backend inside the Tauri binary — no sidecar, no Python runtime, no IPC channel.
>
> This document now contains only the parts that are still accurate: the product vision, the gap Quill fills, the user flow, the roadmap, and the key design decisions (updated).
>
> For current architecture and development setup, see:
> - [README.md](README.md) — user-facing architecture + development setup
> - [CONTRIBUTING.md](CONTRIBUTING.md) — current project structure and quality gates
> - [CHANGELOG.md](CHANGELOG.md) — `[Unreleased]` section documents the full Rust migration

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

## Phased Roadmap

### Phase 1 — Windows MVP ✅ (shipped in 0.1.0)
- Platform abstraction layer scaffolding
- Windows backends: capture, context, hotkey, replace
- OpenRouter + Ollama providers (streaming)
- Built-in modes: Rewrite, Translate, Coach, Shorter, Formal, Fix Grammar, Expand
- Tauri overlay with streaming output + copy/replace + Escape to dismiss
- Config loading (`default.yaml` + `user.yaml` merge, env var override)
- First-run setup wizard

### Phase 2 — Differentiators ✅ (shipped in 0.1.0)
- Custom modes via `config/modes.yaml`
- Per-app context-aware tone defaults
- Settings UI
- All providers: OpenAI, generic OpenAI-compatible
- System tray icon
- Mode chaining (Fix → Formal, etc.)
- Language picker, persona/voice, AI tutor, history, diff view

### Phase 3 — Full Rust Migration ✅ (shipped in 0.2.0)
- Eliminated Python sidecar; all logic native in the Tauri binary
- Two-window layout (compact mini overlay + full studio panel)
- "Obsidian Glass" design system
- Real SSE streaming via `reqwest` + `futures-util`
- Bundled SQLite via `rusqlite` (no system libsqlite3)
- Cross-platform CI matrix (Windows / macOS / Linux)

### Phase 4 — Polish & Distribution (planned)
- Signed release binaries + auto-update via Tauri updater
- `.dmg` (notarized) + `.msi` + `.AppImage` + `.deb`
- Wayland support (depends on upstream `tauri-plugin-global-shortcut`)
- Mode sharing (export/import via GitHub Gist URL)
- Package repositories: `winget`, `homebrew`, `apt`

---

## Key Design Decisions & Rationale

| Decision | Rationale |
|---|---|
| Tauri v2, not Electron | 10× smaller binary, native webview per OS, Rust safety, same React UI everywhere |
| Native Rust backend (as of 0.2.0) | Originally Python for desktop-automation library ecosystem (pywinauto, pynput). Migrated to Rust after the ecosystem matured: `arboard` (clipboard), `enigo` (input sim), `tauri-plugin-global-shortcut` cover the Windows/macOS/Linux surfaces we need. Net: single binary, no interpreter, no IPC, no Python runtime. |
| Platform abstraction from day one | Prevents OS-specific code leaking into core; porting becomes mechanical, not architectural |
| OpenRouter as default | Zero local setup; free tier is usable; OpenAI-compatible so swapping providers is trivial |
| YAML config | Human-readable, Git-friendly, diff-able; easier than JSON for non-developers |
| Clipboard as universal fallback | UIA, Accessibility API, and xdotool all have edge cases; clipboard simulation works on every OS |
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
