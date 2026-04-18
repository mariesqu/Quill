# Changelog

All notable changes to Quill are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [Unreleased]

## [1.0.0] — 2026-04-17

First stable release of the Slint rewrite. The Tauri + Python-sidecar
stack is gone; Quill is now a single native `quill.exe` built on Rust +
Slint with a warm-editorial three-tier window model (Overlay / Palette /
Workspace). Engine and cancel protocol verified across 15 rounds of
adversarial judgment-day review on Opus 4.7.

### Added — Claude Code CLI provider

- New `claude-cli` provider (`src/providers/claude_cli.rs`) — shells out to the local `claude` binary so users can ride their Claude Max / Pro subscription directly without an API key or OpenRouter hop.
- Spawned in pure-completion shape: `--system-prompt <quill>` replaces Claude Code's agentic system prompt, `--tools ""` disables tool use, `--permission-mode bypassPermissions` skips approval prompts, `current_dir(temp_dir)` isolates from any cwd-level `CLAUDE.md`.
- Streams stdout in 256-byte bursts with UTF-8 boundary safety; `kill_on_drop(true)` so user cancel tears down the child immediately.
- Model aliases (`haiku` / `sonnet` / `opus`) and full IDs both supported via the MODEL field. Windows `.cmd` shim from npm installs is detected automatically.
- Settings UI: API KEY field hidden for `claude-cli` (same pattern as `ollama`); MODEL label hints at the alias set.

### Fixed — Palette → mode dispatch left overlay hidden

- The overlay's PALETTE button previously hid the overlay before showing the palette. When the user picked a mode/chain, the palette hid itself and dispatched `ExecuteMode` — but the overlay was already gone, so the stream result had no surface to render on.
- Fixed by NOT hiding the overlay on palette-open (palette is `always-on-top` + opaque, it covers visually) AND having the palette's `item_activated` re-summon the overlay on mode/chain so it also works when the palette was summoned from the tray or Ctrl+Shift+P without the overlay already being up.

### Changed — Full Rust migration

- **Architecture** — Eliminated the Python sidecar entirely. All core logic (config loading, modes, prompt building, history, tutor, providers, platform capture/replace/context) now lives inside the Tauri binary as native Rust modules. No more stdio JSON IPC, no Python runtime, no PyInstaller bundles.
- **Backend** — New Rust modules under `ui/src-tauri/src/`:
  - `core/{config,modes,prompt,history,tutor,clipboard}.rs`
  - `providers/{openrouter,openai,ollama,generic}.rs` — all with real SSE streaming via `reqwest` + `futures-util`
  - `platform/{capture,replace,context}.rs` — Win32 / osascript / xdotool backends
  - `engine.rs` — orchestrator (capture → stream → history)
  - `commands.rs` — `#[tauri::command]` handlers exposed to React
- **UI** — New two-window layout:
  - `windows/MiniOverlay.jsx` — compact 380×560 quick-access pill
  - `windows/FullPanel.jsx` — 4-tab studio (Write / History / Tutor / Settings)
  - New "Obsidian Glass" design system with violet/coral/mint accents, spring animations, streaming-pulse glow
- **IPC** — `useQuillBridge.js` rewritten for direct `invoke()` + `listen()`. All `send_to_python` calls replaced with typed commands.
- **Storage** — SQLite history now via bundled `rusqlite` (no system `libsqlite3` dependency).
- **Binary size** — Single Tauri binary; no Python interpreter, no `httpx`/`pynput`/`keyboard` dependencies.

### Removed

- `core/`, `platform_/`, `providers/` (Python packages)
- `pyproject.toml`, `quill.egg-info/`, `scripts/install-*.{sh,ps1}`, `scripts/build.sh`
- `tests/` (pytest)
- Dead React components: `Overlay.jsx`, `Settings.jsx`, `TutorPanel.jsx`, `PermissionPrompt.jsx`
- Stale CI workflow (was still building Python matrix)

### Fixed

- `capabilities/default.json` — was targeting a nonexistent `"overlay"` window, and requesting `shell:allow-execute` without the plugin installed. Now targets `["mini", "full"]` with correct `global-shortcut:default` + `clipboard-manager:default` permissions.
- `platform/context.rs` — added missing `windows` crate for Win32 FFI.
- `engine.rs` — missing `tauri::Manager` trait import; immutable binding was calling mutable method.
- `main.rs` — illegal `"a"..="z"` range pattern on `&str` in hotkey parser.
- `core/history.rs` — rusqlite `Statement` lifetime issue in `get_recent()`.
- Missing `ui/src-tauri/icons/` — icons referenced in `tauri.conf.json` were never created.

## [0.1.0] — 2026-04-04

### Added

- **Core engine** — Python 3.11+ async sidecar, YAML config with deep merge, env var overrides *(replaced by native Rust in 0.2.0)*
- **Platform abstraction** — text capture, active app detection, hotkey, paste-back for Windows / macOS / Linux
- **Provider backends** — OpenRouter (default), Ollama (local), OpenAI, any generic OpenAI-compatible endpoint
- **Seven built-in modes** — Rewrite, Translate, Coach, Shorter, Formal, Fix Grammar, Expand
- **Language picker** — output any mode in any of 11 languages (or a custom one); persisted across sessions
- **My Voice / Persona** — tone presets, style notes, avoid list injected into every prompt
- **Diff view** — pure-JS LCS word-level diff with added/removed highlights and word-count stats
- **Mode chaining** — sequential mode execution (Fix → Formal, Fix → Translate, Polish → Short)
- **Custom modes & chains** — define your own in `config/user.yaml`, hot-reloaded without restart
- **AI Tutor** — per-transformation explain, daily/weekly personalised lessons anchored to actual usage
- **Local history** — opt-in SQLite store at `~/.quill/history.db`; expandable entries with diff and tutor explain
- **Smart mode suggestion** — heuristic pre-analysis of selected text suggests the best mode
- **One-off instruction field** — prepend a custom prompt to any mode invocation
- **Retry** — re-run the last mode with a new generation (`r` key or ↻ button)
- **Keyboard shortcuts** — `1`–`7` trigger modes, `r` retries, `Esc` dismisses
- **Word count badge** — `before→after (±delta)` after every transformation
- **First-run wizard** — guided setup for provider + API key on first launch
- **System tray** — Show, AI Tutor, Settings, Quit; left-click shows overlay
- **Settings UI** — tabbed: AI Provider, Behaviour, My Voice, AI Tutor, About
- **Dark glassmorphism UI** — Tauri v2 + React, custom CSS, ~8MB binary

### Fixed

- API errors now surface a human-readable message (bad key, model not found, rate limit, etc.) instead of a raw HTTP status code

[0.1.0]: https://github.com/mariesqu/Quill/releases/tag/v0.1.0
