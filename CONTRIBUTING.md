# Contributing to Quill

Thank you for your interest in contributing! This document covers everything you need to get started.

---

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Ways to Contribute](#ways-to-contribute)
- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Making Changes](#making-changes)
- [Pull Request Guidelines](#pull-request-guidelines)
- [Coding Standards](#coding-standards)

---

## Code of Conduct

This project follows the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md). By participating, you agree to uphold it. Report unacceptable behaviour to the maintainer via GitHub.

---

## Ways to Contribute

- **Bug reports** — open an issue using the bug report template
- **Feature requests** — open an issue using the feature request template
- **Code** — fix a bug, implement a feature request, improve tests
- **Documentation** — fix typos, improve explanations, add examples
- **Platform support** — improve Windows/macOS/Linux/Wayland backends

If you plan a large change, open an issue first to discuss the approach before investing time in a PR.

---

## Development Setup

### Prerequisites

| Tool | Version |
|---|---|
| Rust + Cargo | stable (via [rustup.rs](https://rustup.rs)) |
| Node.js | 18+ |
| Tauri system deps | see [v2.tauri.app/start/prerequisites](https://v2.tauri.app/start/prerequisites/) |

### Steps

```bash
# 1. Fork and clone
git clone https://github.com/YOUR_USERNAME/Quill
cd Quill

# 2. Copy config and add your API key
cp config/default.yaml config/user.yaml
# Edit config/user.yaml — set api_key

# 3. Install Node dependencies and run
cd ui
npm install
npm run tauri dev        # launches with hot-reload
```

Tauri compiles the Rust backend (`ui/src-tauri/`) and serves the Vite frontend (`ui/src/`). First build downloads and compiles every crate — allow several minutes.

### Platform prerequisites

**Windows** — Microsoft C++ Build Tools + WebView2 (usually already installed via Edge)

**macOS** — Xcode Command Line Tools (`xcode-select --install`)

**Linux (Debian/Ubuntu)**
```bash
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  librsvg2-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  libxdo-dev \
  libssl-dev \
  patchelf xdotool xclip
```

### Quality gates

```bash
cd ui/src-tauri
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-features
cargo check                    # fast type check

cd ../
npm run build                  # Vite frontend build
```

---

## Project Structure

```
Quill/
├── config/
│   ├── default.yaml    # shipped defaults (no secrets)
│   ├── modes.yaml      # built-in modes + chains
│   └── user.yaml       # gitignored — user overrides
└── ui/
    ├── src/                        # React frontend
    │   ├── windows/
    │   │   ├── MiniOverlay.jsx     # compact quick-access window
    │   │   └── FullPanel.jsx       # 4-tab studio
    │   ├── components/             # DiffView, ComparisonView, FirstRun
    │   └── hooks/useQuillBridge.js # invoke() + listen() adapter
    └── src-tauri/                  # Rust backend (Tauri v2)
        ├── Cargo.toml
        ├── tauri.conf.json
        ├── capabilities/           # permission manifests
        ├── icons/                  # app icons (png, ico)
        └── src/
            ├── main.rs             # window setup + tray + hotkey wiring
            ├── engine.rs           # orchestrator
            ├── commands.rs         # #[tauri::command] handlers
            ├── core/               # config/modes/prompt/history/tutor/clipboard
            ├── platform/           # capture/context/replace (Win32 · osascript · xdotool)
            └── providers/          # OpenRouter · OpenAI · Ollama · Generic (SSE streaming)
```

---

## Making Changes

1. Create a branch from `main`:
   ```bash
   git checkout -b fix/your-bug-description
   # or
   git checkout -b feat/your-feature-name
   ```

2. Make your changes, keeping commits focused and atomic.

3. Add or update tests for any logic changes in `ui/src-tauri/src/`.

4. Run the quality gates:
   ```bash
   cd ui/src-tauri
   cargo fmt --all -- --check
   cargo clippy --all-targets -- -D warnings
   cargo test --all-features
   ```

5. Push and open a PR against `main`.

---

## Pull Request Guidelines

- **One concern per PR** — don't mix a bug fix with a refactor
- **Describe the change** — fill in the PR template (what, why, how to test)
- **Link the issue** — use `Closes #123` in the PR body
- **Keep it small** — large PRs are hard to review; split if needed
- **Don't break platform tests** — platform-specific tests auto-skip if the OS doesn't match; don't remove skip guards

---

## Coding Standards

### Rust

- Style enforced by **`cargo fmt`** — run it before committing
- **`cargo clippy --all-targets -- -D warnings`** must be clean
- Use `anyhow::Result` for fallible functions that bubble up to commands
- All `#[tauri::command]` handlers live in `commands.rs` — keep them thin; delegate to `engine.rs`
- No blocking calls inside async command handlers — use `tokio::task::spawn_blocking` for filesystem / OS FFI work
- No secrets or API keys in code or tests

### JavaScript / React

- Functional components + hooks only
- No external UI libraries — the "Obsidian Glass" design is custom CSS in `globals.css`
- Keep components focused; extract logic to hooks in `ui/src/hooks/`
- No `console.log` left in production paths
- All backend calls go through `useQuillBridge.js` (centralised `invoke()` + `listen()`)

### Commit messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add Wayland hotkey support
fix: correct chain_step event payload
docs: update provider setup guide
test: add prompt builder persona tests
chore: bump reqwest to 0.12
```

---

Thank you for contributing!
