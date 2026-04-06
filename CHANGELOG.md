# Changelog

All notable changes to Quill are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [Unreleased]

## [0.1.0] — 2026-04-06

### Added

- **Core engine** — Python 3.11+ async sidecar, YAML config with deep merge, env var overrides
- **Platform abstraction** — text capture, active app detection, hotkey, paste-back for Windows / macOS / Linux
- **Provider backends** — OpenRouter (default), Ollama (local), OpenAI, any generic OpenAI-compatible endpoint
- **Custom endpoint auth** — custom HTTP headers for API gateways (Cloudflare Zero Trust, Kong, Azure APIM, Basic auth)
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
- **Retry** — re-run the last mode with a new generation (`r` key or Retry button)
- **Keyboard shortcuts** — `1`–`7` trigger modes, `r` retries, `Esc` dismisses; numbers visible on mode buttons
- **Word count badge** — `before→after (±delta)` after every transformation (mono font)
- **First-run wizard** — guided setup for provider + API key on first launch
- **System tray** — Show, AI Tutor, Settings, Quit; left-click shows overlay; custom feather icon
- **Settings UI** — tabbed: AI Provider, Behaviour, My Voice, AI Tutor, Templates, About; compact Save in topbar
- **Dark glassmorphism UI** — Tauri v2 + React, DM Sans + JetBrains Mono typography, refined editorial design
- **Draggable windows** — all screens draggable via header, resizable, hide on close/dismiss
- **Error boundary** — React error boundary catches crashes with user-friendly restart message
- **Light & dark theme** — toggle in Settings, persisted via localStorage

### Fixed

- **Sidecar stability** — readiness sync via AtomicBool, graceful spawn failure, crash recovery with user notification
- **Text capture** — clipboard retry with change detection (replaces fixed 150ms sleep); wait for hotkey modifier release on Windows/Linux before sending Ctrl+C
- **Replace paste-back** — overlay hides and returns focus to previous app before pasting; strips leading/trailing whitespace from AI responses
- **UTF-8 stdout** — forced UTF-8 on Windows (cp1252 crashed on emoji in mode labels)
- **Config path resolution** — PyInstaller sidecar walks up directory tree to find project root config; works in both dev mode (target/debug/) and production
- **Custom headers** — saved as YAML dict (not fragile multi-line string); clearing from UI properly removes from config
- **Custom endpoint URL** — no `/chat/completions` appended; user provides full URL as-is
- **History pruning** — enforces max_entries limit (default 10K), deletes oldest on overflow
- **Provider caching** — reuses provider instance, resets on config change
- **Tutor error handling** — loading state resets on error, errors auto-dismiss after 8 seconds
- **Event listener cleanup** — null-safe unsubscribe in App.jsx, proper async lifecycle
- **Undo race condition** — awaits Python sync before mutating React state
- **Clipboard copy** — unhandled promise rejection caught
- **Language detection** — input capped at 100K chars to prevent ReDoS
- **Hotkey listener leak** — stops old pynput listener before creating new one (macOS/Linux)
- **API key handling** — empty string instead of "none" fallback; no key leakage in logs
- **Streaming validation** — defensive `.get()` on response structure; handles empty choices
- **Bare except** in DB migration narrowed to `sqlite3.OperationalError`
- **CSP enabled** — `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'`
- **CI stability** — skip Windows capture tests on headless runners; handle macOS pyobjc unavailability; correct sidecar dummy path for cargo check

### Security

- Input sanitization on custom language input (alpha/space/hyphen only, 50 char max)
- HTML escaping in tutor markdown renderer
- API keys never logged; `_mask_key()` helper available
- `follow_redirects=False` on API requests (prevents auth token leakage via redirects)
- 20MB sidecar binary purged from git history

[0.1.0]: https://github.com/mariesqu/Quill/releases/tag/v0.1.0
