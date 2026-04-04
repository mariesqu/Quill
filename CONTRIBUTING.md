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
| Python | 3.11+ |
| Node.js | 18+ |
| Rust + Cargo | stable (via [rustup.rs](https://rustup.rs)) |

### Steps

```bash
# 1. Fork and clone
git clone https://github.com/YOUR_USERNAME/Quill
cd Quill

# 2. Install Python dependencies (dev extras)
pip install -e ".[dev]"

# 3. Copy config and add your API key
cp config/default.yaml config/user.yaml
# Edit config/user.yaml — set api_key

# 4. Install Node dependencies
cd ui && npm install && cd ..

# 5. Run the Python core (Terminal 1)
python -m core.main

# 6. Run the Tauri dev server (Terminal 2)
cd ui && npm run tauri dev
```

### Platform extras

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

### Running tests

```bash
pytest                                      # all tests
pytest tests/test_prompt_builder.py -v     # specific file
pytest tests/ -v --tb=short                # verbose with short tracebacks
```

---

## Project Structure

```
Quill/
├── core/               # Python async engine
│   ├── main.py         # entry point + command loop
│   ├── config_loader.py
│   ├── prompt_builder.py
│   ├── streamer.py     # IPC emit helpers
│   ├── history.py      # SQLite history store
│   └── tutor.py        # AI Tutor prompt builders
├── platform_/          # OS abstraction layer (underscore avoids stdlib clash)
│   ├── capture/        # text selection capture
│   ├── context/        # active app detection
│   ├── hotkey/         # global hotkey registration
│   └── replace/        # paste-back logic
├── providers/          # AI provider backends (OpenAI-compatible)
├── config/
│   ├── default.yaml    # shipped defaults (no secrets)
│   ├── modes.yaml      # built-in modes + chains
│   └── user.yaml       # gitignored — user overrides
├── ui/
│   ├── src/            # React components + hooks
│   └── src-tauri/      # Tauri Rust shell
└── tests/
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

3. Add or update tests for any logic changes in `core/` or `providers/`.

4. Run the full test suite: `pytest`

5. Run the linter: `ruff check . && ruff format --check .`

6. Push and open a PR against `main`.

---

## Pull Request Guidelines

- **One concern per PR** — don't mix a bug fix with a refactor
- **Describe the change** — fill in the PR template (what, why, how to test)
- **Link the issue** — use `Closes #123` in the PR body
- **Keep it small** — large PRs are hard to review; split if needed
- **Don't break platform tests** — platform-specific tests auto-skip if the OS doesn't match; don't remove skip guards

---

## Coding Standards

### Python

- Style enforced by **ruff** (line length 100, target Python 3.11)
- Type hints on all public functions
- `async/await` throughout `core/` — no blocking calls on the event loop
- No secrets or API keys in code or tests

### JavaScript / React

- Functional components only
- No external UI libraries — the glassmorphism design is custom CSS
- Keep components focused; extract logic to hooks in `ui/src/hooks/`
- No `console.log` left in production paths

### Rust

- Keep `main.rs` thin — it's a bridge, not business logic
- All new Python message types need a matching `relay_message()` arm

### Commit messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add Wayland hotkey support
fix: relay chain_step messages through Tauri
docs: update provider setup guide
test: add prompt builder persona tests
chore: bump httpx to 0.28
```

---

Thank you for contributing!
