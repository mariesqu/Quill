# Changelog

All notable changes to Quill are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [Unreleased]

## [0.1.0] — 2026-04-04

### Added

- **Core engine** — Python 3.11+ async sidecar, YAML config with deep merge, env var overrides
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
