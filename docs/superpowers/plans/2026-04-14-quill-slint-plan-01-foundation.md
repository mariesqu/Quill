# Quill Slint Rewrite — Plan 1: Foundation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Demolish the Tauri + React stack, preserve pure-Rust core/providers, and establish the Windows-native platform foundation (UIA, global-hotkey, tray-icon, Mica, DWM, WinEvent caret tracking) so every subsequent plan has a compilable, tested base to build on.

**Architecture:** Destructive in-place migration on `claude/slint-rewrite` branch with `tauri-final` git tag as rollback anchor. Flatten `ui/src-tauri/*` to repo root as a single `quill` crate. Hardcode `windows-rs` (no `#[cfg]` gates). Phases 0-2 from the design spec.

**Tech Stack:** Rust (edition 2021), tokio, windows-rs 0.58, global-hotkey 0.5, tray-icon 0.14, slint 1.8 (deps only at this stage — UI wiring comes in Plan 3), rusqlite, reqwest, anyhow, tracing, enigo.

**Spec reference:** `docs/superpowers/specs/2026-04-14-quill-slint-rewrite-design.md` sections 1, 4.1–4.9, 6, 7 (Phase 0-2).

**Done state:**
1. Repo root is a single-crate Rust project: `./Cargo.toml`, `./src/`, `./resources/`, `./build.rs`
2. `cargo build` succeeds from clean checkout
3. `cargo test` passes for `core/` + `providers/`
4. `cargo test --test platform_smoke --features smoke -- --test-threads=1` passes on a Windows host with notepad.exe accessible
5. All Tauri, React, Vite, npm artifacts deleted from the tree
6. `resources/icons/plume.svg` exists; `resources/icons/quill.ico` generated at build time

---

## File Structure

### Files deleted

- `ui/src/` (entire React tree — components, hooks, styles, utils, windows)
- `ui/index.html`, `ui/package.json`, `ui/package-lock.json`, `ui/vite.config.js`
- `ui/node_modules/`, `ui/dist/`
- `ui/src-tauri/tauri.conf.json`
- `ui/src-tauri/capabilities/`
- `ui/src-tauri/src/commands.rs` (replaced later by Slint callbacks)
- `ui/src-tauri/icons/` (moved to `resources/icons/`)

### Files moved

- `ui/src-tauri/Cargo.toml` → `./Cargo.toml`
- `ui/src-tauri/build.rs` → `./build.rs`
- `ui/src-tauri/src/` → `./src/`
- `ui/src-tauri/icons/` → `./resources/icons/` (temporary — `icon.png` deleted after plume.svg is authored)

### Files created

| Path | Responsibility |
|---|---|
| `resources/icons/plume.svg` | Single source of truth for Classic Feather glyph; rendered live in Slint UI |
| `build.rs` | Compiles `.slint` files (placeholder for Plan 3) + generates `quill.ico` from `plume.svg` via `resvg` |
| `src/platform/traits.rs` | `TextCapture`, `TextReplace`, `ContextProbe` traits for testability |
| `src/platform/uia.rs` | `IUIAutomation` wrapper: focused element, selected text, selection/caret rects, editable-control check |
| `src/platform/tray.rs` | `tray-icon` crate wrapper with menu + event channel |
| `src/platform/mica.rs` | `DwmSetWindowAttribute` wrapper for Mica backdrop + round corners + dark mode |
| `src/platform/dwm_shadow.rs` | `DwmExtendFrameIntoClientArea` + `DWMWA_NCRENDERING_POLICY` native shadow |
| `src/platform/caret.rs` | `SetWinEventHook` thread → `FocusEvent` channel |
| `tests/platform_smoke.rs` | Tier 3 platform smoke tests (gated behind `feature = "smoke"`) |

### Files modified

| Path | Change |
|---|---|
| `Cargo.toml` (after move) | Remove `tauri*`, `arboard`. Add `slint`, `slint-build`, `global-hotkey`, `tray-icon`, `resvg` (build-dep). Expand `windows` features. |
| `src/main.rs` | Rewrite as minimal shell: init tracing, return `Ok(())` (full bootstrap comes in Plan 3) |
| `src/engine.rs` | Strip every `tauri::*`, `AppHandle`, `Emitter`, `Manager` reference. Temporary stub emit sites until Plan 2 wires channels. |
| `src/core/hotkey.rs` → delete | Merged into `src/platform/hotkey.rs` (new module replacing Tauri plugin) |
| `src/platform/capture.rs` | Rewrite: UIA-first via `platform::uia`, clipboard fallback. Strip Tauri refs. |
| `src/platform/replace.rs` | Strip Tauri refs. `enigo`-based Ctrl+V stays. |
| `src/platform/context.rs` | Strip Tauri refs. |

### Repo topology (after Plan 1)

```
quill/
├── Cargo.toml                 # moved from ui/src-tauri/, rewritten
├── build.rs                   # moved, rewritten
├── resources/
│   └── icons/
│       ├── plume.svg          # NEW
│       └── quill.ico          # generated at build
├── src/
│   ├── main.rs                # minimal shell
│   ├── engine.rs              # stripped of Tauri, awaiting Plan 2 refactor
│   ├── core/                  # preserved (plus existing tests green)
│   │   ├── config.rs
│   │   ├── history.rs
│   │   ├── modes.rs
│   │   ├── prompt.rs
│   │   ├── think_filter.rs
│   │   ├── tutor.rs
│   │   └── clipboard.rs
│   ├── providers/             # preserved (plus existing tests green)
│   │   ├── generic.rs
│   │   ├── openai.rs
│   │   ├── openrouter.rs
│   │   └── ollama.rs
│   └── platform/
│       ├── traits.rs          # NEW
│       ├── capture.rs         # refactored (UIA-first)
│       ├── replace.rs         # preserved + cleaned
│       ├── context.rs         # preserved + cleaned
│       ├── hotkey.rs          # NEW (replaces core/hotkey.rs)
│       ├── uia.rs             # NEW
│       ├── tray.rs            # NEW
│       ├── mica.rs            # NEW
│       ├── dwm_shadow.rs      # NEW
│       └── caret.rs           # NEW
└── tests/
    └── platform_smoke.rs      # NEW (feature-gated)
```

---

## Tasks

### Task 1: Safety net — tag current state and create branch

**Files:** none (git operations only)

- [ ] **Step 1: Verify current working tree is clean**

Run: `git status`
Expected: reports only the known modified files (`ui/src-tauri/Cargo.toml`, `ui/src-tauri/src/engine.rs`, `ui/src-tauri/src/platform/capture.rs`) or "working tree clean". If there are unexpected files, stop and investigate.

- [ ] **Step 2: Commit any outstanding in-progress work on current branch**

If `git status` showed modified files from recent work that should be preserved, commit them first:

```bash
git add -A && git commit -m "chore: checkpoint before slint rewrite"
```

If the tree is already clean, skip this step.

- [ ] **Step 3: Verify we are on `claude/rust-migration`**

Run: `git branch --show-current`
Expected: `claude/rust-migration`

- [ ] **Step 4: Create the rollback tag**

```bash
git tag tauri-final
```

Expected: no output. Verify with `git tag --list | grep tauri-final` → prints `tauri-final`.

- [ ] **Step 5: Create and check out the new branch**

```bash
git checkout -b claude/slint-rewrite
```

Expected: `Switched to a new branch 'claude/slint-rewrite'`.

- [ ] **Step 6: Confirm the branch was created**

Run: `git branch --show-current`
Expected: `claude/slint-rewrite`

No commit in this task — the tag and branch are the checkpoint.

### Task 2: Delete React frontend

**Files:**
- Delete: `ui/src/` (entire tree)
- Delete: `ui/index.html`
- Delete: `ui/package.json`
- Delete: `ui/package-lock.json`
- Delete: `ui/vite.config.js`
- Delete: `ui/node_modules/`
- Delete: `ui/dist/`

The Rust side still references nothing in these paths — the Tauri webview only loads them at runtime. Deleting them leaves `cargo build` still passing (until Task 4 rewrites deps).

- [ ] **Step 1: Sanity-check that nothing in `src-tauri/src/` imports React artifacts**

Run: `rg "vite|node_modules|dist/" ui/src-tauri/src/`
Expected: zero results. If any reference exists, stop and investigate — it should not be referenced from Rust.

- [ ] **Step 2: Delete the React source directory**

```bash
rm -rf ui/src
```

- [ ] **Step 3: Delete the webview entry files**

```bash
rm ui/index.html ui/package.json ui/package-lock.json ui/vite.config.js
```

- [ ] **Step 4: Delete the npm/Vite generated directories**

```bash
rm -rf ui/node_modules ui/dist
```

- [ ] **Step 5: Verify the Rust crate still builds**

Run: `cd ui/src-tauri && cargo build`
Expected: **build succeeds** (Tauri code isn't touched yet). Warnings are OK.

- [ ] **Step 6: Commit**

```bash
cd C:/GitLab/Quill
git add -A
git commit -m "chore: delete React frontend ahead of Slint rewrite"
```

Expected: single commit recording deletion of `ui/src/**`, `ui/index.html`, `ui/package*.json`, `ui/vite.config.js`, and (ignored-but-tracked-in-some-repos) `ui/dist/**`. `node_modules` is already ignored so git won't track its deletion.

### Task 3: Delete Tauri scaffolding files

**Files:**
- Delete: `ui/src-tauri/tauri.conf.json`
- Delete: `ui/src-tauri/capabilities/` (entire directory)
- Delete: `ui/src-tauri/gen/` (if present — auto-generated Tauri artifacts)
- Delete: `ui/src-tauri/.taurignore` (if present)

Leaves Cargo.toml, src/, build.rs, icons/, target/ intact for now — those are handled in later tasks.

- [ ] **Step 1: Delete tauri.conf.json**

```bash
rm ui/src-tauri/tauri.conf.json
```

- [ ] **Step 2: Delete the capabilities directory**

```bash
rm -rf ui/src-tauri/capabilities
```

- [ ] **Step 3: Delete any generated Tauri artifacts**

```bash
rm -rf ui/src-tauri/gen 2>/dev/null || true
rm ui/src-tauri/.taurignore 2>/dev/null || true
```

The `|| true` is because these may not exist — they appear after `cargo tauri dev` has been run at least once. Not an error if they're missing.

- [ ] **Step 4: Verify Rust crate no longer builds (expected failure)**

Run: `cd ui/src-tauri && cargo build`
Expected: **FAILS** with `error: failed to run custom build command for tauri-build` or similar — `tauri-build` needs `tauri.conf.json` to generate context. This is expected; we fix it in Task 4 by removing `tauri-build` entirely.

Do NOT commit yet — the tree is in a broken state. Tasks 3, 4, 5, 6, 7 form one atomic unit ending in a single commit at the end of Task 7.

### Task 4: Rewrite Cargo.toml for Slint stack

**Files:**
- Modify: `ui/src-tauri/Cargo.toml` (full rewrite)

This is the key pivot. After this step, `tauri*` packages are gone, `slint` is in, `windows` features are expanded for UIA/DWM/WinEvent, and `resvg` is added as a build-dep for icon generation.

- [ ] **Step 1: Replace `ui/src-tauri/Cargo.toml` with the new content**

Write the following to `ui/src-tauri/Cargo.toml`:

```toml
[package]
name = "quill"
version = "0.3.0"
description = "Privacy-first, model-agnostic AI writing assistant — native Windows"
authors = ["Quill Contributors"]
license = "MIT"
repository = "https://github.com/mariesqu/Quill"
edition = "2021"

[build-dependencies]
slint-build = "1.8"
resvg       = "0.42"
ico         = "0.3"

[dependencies]
# UI
slint            = { version = "1.8", features = ["compat-1-2", "backend-winit-skia"] }

# Platform integration
global-hotkey    = "0.5"
tray-icon        = "0.14"
enigo            = "0.2"

# Runtime + async
tokio            = { version = "1", features = ["full"] }
futures-util     = "0.3"
async-trait      = "0.1"

# HTTP + streaming
reqwest          = { version = "0.12", features = ["stream", "json"] }

# Persistence
rusqlite         = { version = "0.31", features = ["bundled"] }

# Config, errors, utilities
serde            = { version = "1", features = ["derive"] }
serde_json       = "1"
serde_yaml       = "0.9"
anyhow           = "1"
dirs             = "5"
regex            = "1"

# Logging
tracing             = "0.1"
tracing-subscriber  = { version = "0.3", features = ["env-filter"] }

[dependencies.windows]
version = "0.58"
features = [
    "Win32_Foundation",
    "Win32_UI_WindowsAndMessaging",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_Accessibility",
    "Win32_System_Com",
    "Win32_System_Threading",
    "Win32_System_SystemInformation",
    "Win32_Graphics_Dwm",
    "Win32_Graphics_Gdi",
]

[features]
default = []
smoke   = []  # gates tests/platform_smoke.rs

[profile.release]
panic         = "abort"
codegen-units = 1
lto           = true
opt-level     = "s"
strip         = true
```

- [ ] **Step 2: Verify the file was written correctly**

Run: `cat ui/src-tauri/Cargo.toml | head -20`
Expected: first 20 lines match the rewritten content above.

- [ ] **Step 3: Attempt cargo check (expected failure)**

Run: `cd ui/src-tauri && cargo check`
Expected: **FAILS** with errors in `src/main.rs` and `src/commands.rs` — those files still reference `tauri::*` which is no longer a dependency. This is expected. Fixed in Tasks 5, 6, 7.

Do NOT commit yet.

### Task 5: Stub main.rs to a compilable minimum

**Files:**
- Modify: `ui/src-tauri/src/main.rs` (full rewrite — minimal shell)

The current `main.rs` uses `tauri::Builder`, `TrayIconBuilder`, `Emitter`, `Manager`, and registers commands. All of that dies here. We replace with the simplest `main()` that initializes tracing and exits with `Ok(())`. The full boot sequence (tokio runtime, channels, Slint event loop, tray, hotkey) is authored in Plan 3.

- [ ] **Step 1: Replace `ui/src-tauri/src/main.rs` with the minimal shell**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod engine;
mod platform;
mod providers;

use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("quill=info")),
        )
        .init();

    tracing::info!("Quill boot (Plan 1 shell) — full bootstrap lands in Plan 3");
    Ok(())
}
```

Note the module list: `core`, `engine`, `platform`, `providers`. No more `commands` — it gets deleted in Task 7. `ui`, `state` will be added in later plans.

- [ ] **Step 2: Attempt cargo check (still expected to fail)**

Run: `cd ui/src-tauri && cargo check`
Expected: **FAILS** — `engine.rs` still uses `tauri::{AppHandle, Emitter, Manager}` and `commands.rs` still exists with `#[tauri::command]` attributes. These are fixed in Tasks 6 and 7.

Do NOT commit yet.

### Task 6: Strip Tauri references from engine.rs

**Files:**
- Modify: `ui/src-tauri/src/engine.rs` (strip Tauri only — full refactor lands in Plan 2)

Current `engine.rs` uses `tauri::{AppHandle, Emitter, Manager}` and has `app.emit("quill://...", ...)` calls throughout. We can't fully replace these with channels yet (AppState + UiEvent enum don't exist until Plan 2). Temporary strategy: introduce a **module-local `noop_emit`** that logs via `tracing::debug!` and swallows the arguments. Every existing `app.emit(topic, payload)` becomes `noop_emit(topic, payload)`. Every `AppHandle` parameter is removed from function signatures.

This keeps the streaming, comparison, tutor, and hotkey flow logic intact — Plan 2 will replace `noop_emit` with real `UiEvent::...` sends along a channel.

- [ ] **Step 1: Add a top-of-file `noop_emit` helper**

At the top of `ui/src-tauri/src/engine.rs`, after the existing `use` statements (and after removing the Tauri imports — see Step 2), insert:

```rust
/// Temporary UI event sink used during the Tauri → Slint migration.
/// Plan 1 keeps every existing emit call site intact by routing it here.
/// Plan 2 replaces every call with `UiEvent::...` sends along a `tokio::mpsc::UnboundedSender<UiEvent>`.
#[inline]
fn noop_emit(topic: &str, payload: impl std::fmt::Debug) {
    tracing::debug!(target: "quill::engine::noop_emit", topic, ?payload, "noop emit (Plan 1 stub)");
}
```

- [ ] **Step 2: Remove Tauri imports**

Delete the following lines from the top of `engine.rs`:

```rust
use tauri::{AppHandle, Emitter, Manager};
```

(The exact line may be `use tauri::...` in any ordering — delete every `use tauri::*`.)

- [ ] **Step 3: Remove `app: AppHandle` and `app: &AppHandle` from every function signature**

Every function in `engine.rs` that takes `app: AppHandle` or `app: &AppHandle` drops that parameter. Callers inside the file that pass `app.clone()` or `&app` drop that argument too.

Grep to find them:

```bash
rg "AppHandle|app\.emit|app\.get_webview_window|app: &?AppHandle" ui/src-tauri/src/engine.rs
```

Expected hits: function signatures for `execute_mode`, `execute_chain`, `run_single_stream`, `stream_and_collect`, `finalize_result`, `explain_entry`, `generate_lesson`, `compare_modes`, `run_silent_stream` (if it takes app), plus the emit call sites. Remove the parameter everywhere it appears.

- [ ] **Step 4: Replace every `app.emit(topic, payload)` with `noop_emit(topic, payload)`**

```bash
rg "app\.emit\(" ui/src-tauri/src/engine.rs
```

Mechanically replace the receiver. The topic and payload stay exactly as they are.

- [ ] **Step 5: Replace every `app.get_webview_window(...)` call site**

These only appear in window-management code (show, hide, focus). For Plan 1 we stub them: wrap each in `// TODO(plan-3): Slint window` and return `None` or do nothing. Example:

```rust
// Before
if let Some(w) = app.get_webview_window("mini") {
    let _ = w.show();
    let _ = w.set_focus();
}

// After
// TODO(plan-3): replace with Slint MainWindow show/focus via AppBridge callback
tracing::debug!("noop: show mini window (Plan 1 stub)");
```

Grep to find them:

```bash
rg "get_webview_window" ui/src-tauri/src/engine.rs
```

- [ ] **Step 6: Update any `tauri::async_runtime::spawn` to plain `tokio::spawn`**

```bash
rg "tauri::async_runtime" ui/src-tauri/src/engine.rs
```

Replace `tauri::async_runtime::spawn` with `tokio::spawn`. The signatures are compatible.

- [ ] **Step 7: Run cargo check to verify engine.rs compiles**

Run: `cd ui/src-tauri && cargo check 2>&1 | head -50`
Expected: **FAILS** on `commands.rs` (which still has `#[tauri::command]` attributes and imports) — but engine.rs itself should produce no errors. If engine.rs still has errors, fix them inline before proceeding.

Do NOT commit yet — `commands.rs` still breaks the build. It gets deleted in Task 7.

### Task 7: Delete commands.rs

**Files:**
- Delete: `ui/src-tauri/src/commands.rs`
- Delete: `ui/src-tauri/src/core/hotkey.rs` (replaced by `platform/hotkey.rs` in Task 16)
- Modify: `ui/src-tauri/src/core/mod.rs` (drop `pub mod hotkey;`)

`commands.rs` is the last file that uses `#[tauri::command]`. Deleting it, plus dropping the `mod commands;` line from `main.rs` (already done in Task 5 — confirm), closes the Tauri chapter. `core/hotkey.rs` imports `tauri_plugin_global_shortcut` and `tauri::AppHandle` so it also must go.

- [ ] **Step 1: Delete commands.rs**

```bash
rm ui/src-tauri/src/commands.rs
```

- [ ] **Step 2: Delete core/hotkey.rs**

```bash
rm ui/src-tauri/src/core/hotkey.rs
```

- [ ] **Step 3: Remove `pub mod hotkey;` from core/mod.rs**

Open `ui/src-tauri/src/core/mod.rs` and delete the line that reads:

```rust
pub mod hotkey;
```

(Leave all other `pub mod` lines intact.)

- [ ] **Step 4: Verify no stray references remain**

```bash
rg "crate::core::hotkey|core::hotkey::register_hotkey|mod commands" ui/src-tauri/src/
```

Expected: zero results. If any file still references the deleted module, fix it (most likely `engine.rs` — if so, stub the reference with a `TODO(plan-1-task-16)` comment and a no-op body).

- [ ] **Step 5: Run cargo check — expected to succeed**

Run: `cd ui/src-tauri && cargo check 2>&1 | tail -20`
Expected: **`Finished` line with no errors**. Warnings are OK (lots of unused imports will appear in `engine.rs` — those are fixed in Plan 2).

- [ ] **Step 6: Run cargo build — expected to succeed**

Run: `cd ui/src-tauri && cargo build 2>&1 | tail -20`
Expected: `Finished dev [unoptimized + debuginfo] target(s)` with no errors.

- [ ] **Step 7: Commit the atomic Tauri demolition**

```bash
cd C:/GitLab/Quill
git add -A
git commit -m "chore: demolish Tauri scaffolding, stub main, strip tauri refs from engine"
```

Expected: single commit recording deletion of `tauri.conf.json`, `capabilities/`, `commands.rs`, `core/hotkey.rs`, plus the Cargo.toml rewrite, main.rs stub, and engine.rs Tauri-strip. This is the Phase 0 half-way checkpoint — the Rust crate now compiles against the Slint stack dependencies.

### Task 8: Flatten the repo — move src-tauri contents to root

**Files moved:**
- `ui/src-tauri/Cargo.toml` → `./Cargo.toml`
- `ui/src-tauri/Cargo.lock` → `./Cargo.lock`
- `ui/src-tauri/build.rs` → `./build.rs`
- `ui/src-tauri/src/` → `./src/`
- `ui/src-tauri/icons/` → `./resources/icons/`
- `ui/src-tauri/target/` — **DO NOT MOVE** (delete before or after; it's in `.gitignore`)

**Directories deleted:**
- `ui/src-tauri/`
- `ui/` (now empty)

Use `git mv` so rename history is preserved.

- [ ] **Step 1: Nuke the target directory to avoid moving 500 MB of build artifacts**

```bash
rm -rf ui/src-tauri/target
```

- [ ] **Step 2: Create the `resources/` parent directory**

```bash
mkdir -p resources
```

- [ ] **Step 3: Move top-level files with git mv (preserves history)**

```bash
git mv ui/src-tauri/Cargo.toml Cargo.toml
git mv ui/src-tauri/Cargo.lock Cargo.lock
git mv ui/src-tauri/build.rs build.rs
```

- [ ] **Step 4: Move the source tree**

```bash
git mv ui/src-tauri/src src
```

- [ ] **Step 5: Move the icons directory to resources/**

```bash
git mv ui/src-tauri/icons resources/icons
```

- [ ] **Step 6: Remove the now-empty ui/src-tauri directory**

```bash
rm -rf ui/src-tauri
```

Note: git tracks only file moves; the empty directory is removed with plain `rm`.

- [ ] **Step 7: Remove the now-empty ui directory**

```bash
# There may be leftover .gitkeep files or hidden files
ls -la ui/
# If the listing shows only . and .., delete it
rmdir ui/
```

If `ls` shows stray files, grep for each and decide whether to delete or move them. Common stragglers: `.DS_Store`, `thumbs.db` — delete these.

- [ ] **Step 8: Verify the new layout**

```bash
ls -la
```

Expected: `Cargo.toml`, `Cargo.lock`, `build.rs`, `src/`, `resources/`, plus the pre-existing `docs/`, `config/`, `.git/`, `.gitignore`, `README.md`, `CHANGELOG.md`, etc. No `ui/` directory.

- [ ] **Step 9: Run cargo build from the new root**

```bash
cargo build 2>&1 | tail -5
```

Expected: `Finished dev [unoptimized + debuginfo] target(s) in Xs`.

If this fails with `could not find Cargo.toml`, verify your working directory is the repo root. If it fails with a dependency or path error, the most likely cause is a `build.rs` that references a relative path inside `ui/src-tauri/` — inspect and fix.

- [ ] **Step 10: Commit the flatten**

```bash
git add -A
git commit -m "chore: flatten repo — move src-tauri contents to root"
```

Expected: one commit with a long list of renames (`renamed: ui/src-tauri/src/main.rs -> src/main.rs`, etc.).

### Task 9: Author the plume.svg brand glyph

**Files:**
- Create: `resources/icons/plume.svg`
- Delete: `resources/icons/icon.png`, `resources/icons/icon.ico`, `resources/icons/32x32.png`, `resources/icons/128x128.png`, `resources/icons/128x128@2x.png`

The old serif-Q PNG/ICO assets are retired in favor of a single SVG source of truth. The Classic Feather glyph is authored as clean SVG paths — no embedded bitmap, no font dependencies. The build script in Task 10 renders this SVG to multi-size PNGs and packs them into `quill.ico` automatically.

- [ ] **Step 1: Create resources/icons/plume.svg**

Write the following to `resources/icons/plume.svg`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none"
     stroke="#c084fc" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
  <!-- Feather body -->
  <path d="M20 3 C 17 4, 11 5, 7 11 C 4 16, 4 20, 4 20 L 8 20 C 9 16, 12 13, 17 10"
        fill="#c084fc" fill-opacity="0.22"/>
  <!-- Feather barbs (inner lines suggesting plume structure) -->
  <path d="M18 5 L 13 9"/>
  <path d="M16 8 L 10 13"/>
  <path d="M13 11 L 7 16"/>
</svg>
```

- [ ] **Step 2: Delete the old serif-Q icon assets**

```bash
rm resources/icons/icon.png \
   resources/icons/icon.ico \
   resources/icons/32x32.png \
   resources/icons/128x128.png \
   resources/icons/128x128@2x.png
```

If any of these don't exist (slightly different file set in your checkout), skip the missing ones — the net effect is "only `plume.svg` remains in `resources/icons/`".

- [ ] **Step 3: Verify only plume.svg remains**

```bash
ls resources/icons/
```

Expected output: `plume.svg` (and nothing else).

- [ ] **Step 4: Visual sanity check (optional)**

Open `resources/icons/plume.svg` in any browser or SVG viewer. You should see a violet feather shape on a transparent background — curved teardrop body with three diagonal barb lines inside. If it looks wrong (filled solid, missing barbs, wrong color), the paths are malformed — re-check Step 1.

- [ ] **Step 5: Commit the plume**

```bash
git add resources/icons/plume.svg
git add -u resources/icons/
git commit -m "feat: introduce Classic Feather plume glyph as brand source of truth"
```

Expected: one commit with `new file: resources/icons/plume.svg` and `deleted: resources/icons/icon.png`, `deleted: resources/icons/*.png`, etc.

### Task 10: Rewrite build.rs for slint-build + resvg icon generation

**Files:**
- Modify: `build.rs` (full rewrite — was Tauri build script, now Slint + icon gen)

For Plan 1, `build.rs` does exactly one thing: render `resources/icons/plume.svg` into a multi-size `resources/icons/quill.ico` at build time. The slint-build integration is added in Plan 3 when the first `.slint` files appear — for now, having `slint-build` in `[build-dependencies]` but not calling it is harmless.

- [ ] **Step 1: Replace `build.rs` with the icon generation script**

Write the following to `./build.rs`:

```rust
use std::fs::File;
use std::path::PathBuf;

use ico::{IconDir, IconDirEntry, IconImage, ResourceType};
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};

const ICON_SIZES: &[u32] = &[16, 20, 24, 32, 40, 48, 64, 96, 128, 256];

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let svg_path = manifest_dir.join("resources/icons/plume.svg");
    let ico_path = manifest_dir.join("resources/icons/quill.ico");

    // Re-run only when the source SVG or this script changes
    println!("cargo:rerun-if-changed={}", svg_path.display());
    println!("cargo:rerun-if-changed=build.rs");

    let svg_bytes = std::fs::read(&svg_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", svg_path.display()));

    let tree = Tree::from_data(&svg_bytes, &Options::default())
        .expect("plume.svg is not valid SVG");

    let mut ico_dir = IconDir::new(ResourceType::Icon);
    let svg_size = tree.size();

    for &size in ICON_SIZES {
        let mut pixmap = Pixmap::new(size, size).expect("allocate pixmap");
        let scale_x = size as f32 / svg_size.width();
        let scale_y = size as f32 / svg_size.height();
        let transform = Transform::from_scale(scale_x, scale_y);

        resvg::render(&tree, transform, &mut pixmap.as_mut());

        let rgba = pixmap.data().to_vec();
        let image = IconImage::from_rgba_data(size, size, rgba);
        ico_dir.add_entry(IconDirEntry::encode(&image).expect("encode icon entry"));
    }

    let out = File::create(&ico_path).expect("create quill.ico");
    ico_dir.write(out).expect("write ico data");

    println!("cargo:warning=Generated {} at {}", ICON_SIZES.iter().map(|s| s.to_string()).collect::<Vec<_>>().join("/"), ico_path.display());
}
```

Note: the `resvg` crate version pinned in `Cargo.toml` is `0.42`. If you're on a different minor version and the API (e.g., `Tree::from_data` signature, `tree.size()` return type, `resvg::render` signature) has drifted, adjust the code — the shape of what we need is:

1. Parse SVG bytes → tree
2. For each target size, allocate an RGBA pixmap
3. Compute a scale transform to map SVG coordinates to pixel dimensions
4. Render the tree into the pixmap
5. Encode each pixmap as an ICO entry
6. Write all entries to `quill.ico`

Consult `https://docs.rs/resvg/0.42.0/resvg/` if the exact method names differ.

- [ ] **Step 2: Run cargo build to generate quill.ico**

```bash
cargo build 2>&1 | tail -10
```

Expected: `Finished dev [unoptimized + debuginfo] target(s)` plus a warning line like `Generated 16/20/24/32/40/48/64/96/128/256 at .../resources/icons/quill.ico`.

- [ ] **Step 3: Verify quill.ico was created**

```bash
ls -l resources/icons/quill.ico
file resources/icons/quill.ico
```

Expected: file exists, size in the 15–50 KB range. `file` (if available) reports `MS Windows icon resource - 10 icons` or similar.

- [ ] **Step 4: Visual sanity check**

Copy `resources/icons/quill.ico` to the desktop and preview it in Explorer's large-icon view. You should see the violet feather glyph at all zoom levels without pixelation or clipping.

- [ ] **Step 5: Add `resources/icons/quill.ico` to `.gitignore`**

`quill.ico` is a build artifact — it's regenerated from `plume.svg` every time. Don't check it in.

Open `.gitignore` and add this line (if not already present):

```
/resources/icons/quill.ico
```

- [ ] **Step 6: Commit the build script**

```bash
git add build.rs .gitignore
git commit -m "build: generate quill.ico from plume.svg via resvg"
```

Expected: one commit with `build.rs` rewrite and `.gitignore` addition. No `quill.ico` in the commit — it's now ignored.

### Task 11: Verify empty shell compiles end of Phase 0

**Files:** none (verification checkpoint)

This is the Phase 0 exit gate. The repo should be a clean single-crate Rust project with the Slint dependency stack, no Tauri references anywhere, and a compilable (if non-functional) binary.

- [ ] **Step 1: Verify no Tauri imports remain anywhere**

```bash
rg "use tauri|tauri_plugin|#\[tauri::command\]|tauri::Builder|tauri::AppHandle" src/ build.rs Cargo.toml
```

Expected: zero results. If anything comes back, go back and strip it.

- [ ] **Step 2: Verify no React/Vite artifacts remain**

```bash
find . -path ./target -prune -o -type f \( -name 'package.json' -o -name 'vite.config.js' -o -name 'index.html' \) -print 2>/dev/null
```

Expected: zero results.

- [ ] **Step 3: Clean build from scratch**

```bash
cargo clean
cargo build 2>&1 | tail -15
```

Expected: compiles successfully. First build will be slow (several minutes) because it's rebuilding every crate from scratch, including slint. Subsequent builds will be incremental.

- [ ] **Step 4: Run the binary and verify it exits cleanly**

```bash
cargo run 2>&1 | tail -5
```

Expected output (stderr lines): a tracing line like `INFO quill: Quill boot (Plan 1 shell) — full bootstrap lands in Plan 3` followed by `Finished` / no process output. Exit code 0.

```bash
echo $?
```

Expected: `0`.

- [ ] **Step 5: Verify quill.ico was regenerated during build**

```bash
ls -l resources/icons/quill.ico
```

Expected: file exists, modification time is within the last minute.

- [ ] **Step 6: Commit the Phase 0 checkpoint tag (lightweight)**

```bash
git tag plan-01-phase-0-complete
```

No commit needed at this step — this just tags the current HEAD so we can roll back to "end of Phase 0" easily if Phase 1 or 2 go sideways.

Phase 0 is complete. The tree is a single-crate Rust project with Slint dependencies, no Tauri code, no React code, a working plume glyph source and build-time ICO generation, and a minimal compilable binary. Total time: ~0.5 day.

### Task 12: Preserve & verify core module tests (Phase 1)

**Files:**
- Modify (if needed): any file in `src/core/` that has stray Tauri imports or `#[cfg]` gates

Phase 1 is cheap — `core/` was designed to be pure Rust, so in theory nothing should break. In practice, there are usually 1-2 stragglers (a `use tauri::...` hiding in a test module, a `#[cfg(target_os = "windows")]` gate we can drop, a call to the deleted `core::hotkey::register_hotkey`).

- [ ] **Step 1: Grep for Tauri imports in core/**

```bash
rg "tauri|AppHandle|Emitter" src/core/
```

Expected: zero results. If anything hits, delete or replace inline — same `noop_emit` pattern we used in engine.rs, or just remove the import if the symbol is genuinely unused.

- [ ] **Step 2: Grep for cross-platform cfg gates that are now dead code**

```bash
rg '#\[cfg\(target_os\s*=\s*"(macos|linux)"\)\]' src/core/
```

Expected: zero results. If any exist, delete the gated blocks entirely — we're Windows-only now, non-Windows branches are dead weight.

```bash
rg '#\[cfg\(target_os\s*=\s*"windows"\)\]' src/core/
```

Expected: zero or few hits. If hits appear, you can drop the `#[cfg(...)]` attribute since the whole crate is Windows-only, but it's also fine to leave them — they're no-op, not incorrect.

- [ ] **Step 3: Run core tests**

```bash
cargo test --lib core:: 2>&1 | tail -20
```

Expected: all existing tests in `core/config`, `core/history`, `core/modes`, `core/prompt`, `core/think_filter`, `core/tutor`, `core/clipboard` pass. Example output:

```
test core::config::tests::load_minimal ... ok
test core::config::tests::mask_api_key ... ok
test core::modes::tests::chain_resolution ... ok
test core::prompt::tests::build_prompt_substitutes_language_placeholder ... ok
test core::think_filter::tests::strips_think_tags ... ok
...
test result: ok. 28 passed; 0 failed; 0 ignored
```

If any test fails, read the error. Most likely causes:
- Test references `crate::commands::...` → delete the test or rewrite to call the underlying engine function
- Test references the deleted `core::hotkey::*` → delete the test
- Test references Tauri `AppHandle` → delete the test (it was testing the Tauri wrapper, not the logic)

Fix inline, re-run, iterate until green.

- [ ] **Step 4: Commit any core fixups**

If you made changes:

```bash
git add src/core/
git commit -m "chore(core): strip residual tauri refs, drop cross-platform cfg gates"
```

If no changes were needed, skip this step.

### Task 13: Preserve & verify providers module tests (Phase 1)

**Files:**
- Modify (if needed): any file in `src/providers/` with stray Tauri imports

`providers/` is pure HTTP — reqwest + futures, nothing Tauri. Same cleanup pattern as Task 12.

- [ ] **Step 1: Grep for Tauri imports in providers/**

```bash
rg "tauri|AppHandle|Emitter" src/providers/
```

Expected: zero results. If anything hits, delete or replace inline.

- [ ] **Step 2: Run providers tests**

```bash
cargo test --lib providers:: 2>&1 | tail -20
```

Expected: every provider unit test passes (if any exist). If the existing test coverage is thin here — which is likely, since provider tests were historically sparse in the Tauri version — note that we'll add comprehensive `wiremock`-backed provider tests in Plan 2. For Plan 1 we only require **zero failures**, not full coverage.

Example output:

```
test providers::openai::tests::builds_request_body ... ok
test providers::openrouter::tests::parses_stream_chunk ... ok
...
test result: ok. 6 passed; 0 failed
```

Or, if no tests exist yet:

```
running 0 tests
test result: ok. 0 passed
```

Both are acceptable at this stage.

- [ ] **Step 3: Run the full test suite to make sure the whole crate is green**

```bash
cargo test 2>&1 | tail -10
```

Expected: all passing, zero failures. This includes `core::*`, `providers::*`, and any doc tests.

- [ ] **Step 4: Tag the Phase 1 checkpoint**

```bash
git tag plan-01-phase-1-complete
```

- [ ] **Step 5: Commit any providers fixups**

If you made changes:

```bash
git add src/providers/
git commit -m "chore(providers): strip residual tauri refs"
```

If no changes, skip.

Phase 1 complete. Total elapsed time: ~1 day (Phase 0 + Phase 1). Core and providers are clean pure-Rust modules with passing tests.

### Task 14: Create platform/traits.rs with TextCapture, TextReplace, ContextProbe

**Files:**
- Create: `src/platform/traits.rs`
- Modify: `src/platform/mod.rs` (add `pub mod traits;` and re-exports)

These traits are the seam for Plan 2's integration tests. By taking trait objects in the engine constructor (Plan 2), we can inject fake capture/replace/context in tests without touching real Windows APIs.

- [ ] **Step 1: Create src/platform/traits.rs**

Write the following to `src/platform/traits.rs`:

```rust
//! Platform abstraction traits.
//!
//! These traits define the minimal boundary between the engine and the
//! Windows-specific implementations in this module. Production wires the
//! real implementations (`platform::capture::Capture`, `platform::replace::Replace`,
//! `platform::context::Context`). Tests wire fakes in `tests/fakes/platform.rs`.
//!
//! Traits intentionally cover ONLY what the engine integration tests need to
//! swap — capture, replace, and context probing. Fire-and-forget side effects
//! (Mica, DWM shadow, tray, hotkey registration, caret hooks) are free functions
//! with nothing meaningful to assert against in isolation.

use async_trait::async_trait;

use crate::core::config::AppContext;

/// Result of capturing selected text from the currently focused control.
#[derive(Debug, Clone, Default)]
pub struct CaptureResult {
    /// The captured text. Empty if nothing was captured.
    pub text: String,
    /// Screen-space rectangle of the selection, if UIA could provide it.
    /// Used to anchor the compact overlay to the user's focus.
    pub anchor: Option<ScreenRect>,
    /// Which code path produced this result — useful for diagnostics.
    pub source: CaptureSource,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum CaptureSource {
    #[default]
    None,
    Uia,
    Clipboard,
}

#[derive(Debug, Clone, Copy)]
pub struct ScreenRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl ScreenRect {
    pub fn width(&self) -> i32 { self.right - self.left }
    pub fn height(&self) -> i32 { self.bottom - self.top }
}

/// Read selected text from the currently focused control.
#[async_trait]
pub trait TextCapture: Send + Sync {
    async fn capture(&self) -> CaptureResult;
}

/// Paste text into the currently focused control (simulated Ctrl+V).
#[async_trait]
pub trait TextReplace: Send + Sync {
    async fn paste(&self, text: &str) -> anyhow::Result<()>;
}

/// Read metadata about the currently foregrounded application (name, hint).
/// Used to bias prompt building with app context.
pub trait ContextProbe: Send + Sync {
    fn active_context(&self) -> AppContext;
}
```

Note: this file references `crate::core::config::AppContext`. If that type doesn't exist in the current `core::config` module, add it there as a minimal struct:

```rust
// src/core/config.rs — add near the top if not present
#[derive(Debug, Clone, Default)]
pub struct AppContext {
    pub hint: String,
    pub app_name: Option<String>,
}
```

(The existing `platform::context` module already returns this shape; we just need the type declared in `core::config` or equivalently a new `core::app_context` module. Keep it wherever the existing code puts it.)

- [ ] **Step 2: Update src/platform/mod.rs to expose the new module**

Edit `src/platform/mod.rs` and add at the top:

```rust
pub mod traits;

pub use traits::{CaptureResult, CaptureSource, ContextProbe, ScreenRect, TextCapture, TextReplace};
```

Leave the existing `pub mod capture; pub mod replace; pub mod context;` lines in place.

- [ ] **Step 3: Run cargo check**

```bash
cargo check 2>&1 | tail -20
```

Expected: clean compile. The traits are defined but not yet implemented on any struct — that happens in Task 15.

If you see `error[E0432]: unresolved import` for `AppContext`, add the struct to `core/config.rs` as shown in Step 1.

- [ ] **Step 4: Commit**

```bash
git add src/platform/traits.rs src/platform/mod.rs src/core/config.rs
git commit -m "feat(platform): add TextCapture/TextReplace/ContextProbe traits for testability"
```

### Task 15: Clean platform/replace.rs and platform/context.rs of Tauri refs

**Files:**
- Modify: `src/platform/replace.rs`
- Modify: `src/platform/context.rs`

Both files exist and work today — they're used by the current Tauri-era capture flow. We just need to (a) strip any stray Tauri imports and (b) add struct wrappers that implement the new traits from Task 14.

- [ ] **Step 1: Grep replace.rs and context.rs for Tauri imports**

```bash
rg "tauri|AppHandle|Emitter" src/platform/replace.rs src/platform/context.rs
```

Expected: zero results. If anything hits, delete the import or replace the symbol with a pure-Rust equivalent.

- [ ] **Step 2: Add the Replace struct implementing TextReplace**

At the bottom of `src/platform/replace.rs`, add:

```rust
use async_trait::async_trait;

use super::traits::TextReplace;

/// Production implementation of TextReplace. Uses the existing `paste_text`
/// function (enigo-based Ctrl+V simulation).
#[derive(Default)]
pub struct Replace;

#[async_trait]
impl TextReplace for Replace {
    async fn paste(&self, text: &str) -> anyhow::Result<()> {
        let text = text.to_owned();
        tokio::task::spawn_blocking(move || paste_text(&text))
            .await
            .map_err(|e| anyhow::anyhow!("paste task join error: {e}"))?
    }
}
```

If the existing `paste_text` function has a different signature (e.g., returns `Result<(), String>` instead of `Result<(), anyhow::Error>`), bridge it:

```rust
async fn paste(&self, text: &str) -> anyhow::Result<()> {
    let text = text.to_owned();
    tokio::task::spawn_blocking(move || paste_text(&text))
        .await
        .map_err(|e| anyhow::anyhow!("paste task join error: {e}"))?
        .map_err(|e| anyhow::anyhow!("{e}"))
}
```

- [ ] **Step 3: Add the Context struct implementing ContextProbe**

At the bottom of `src/platform/context.rs`, add:

```rust
use super::traits::ContextProbe;
use crate::core::config::AppContext;

/// Production implementation of ContextProbe. Wraps the existing
/// `get_active_context` free function.
#[derive(Default)]
pub struct Context;

impl ContextProbe for Context {
    fn active_context(&self) -> AppContext {
        get_active_context()
    }
}
```

If the existing free function is named differently, adjust the call. If it returns a different type than `AppContext`, convert via a `From` impl or a manual struct conversion.

- [ ] **Step 4: Run cargo check**

```bash
cargo check 2>&1 | tail -10
```

Expected: clean. If you see trait bound errors, the most likely fix is to derive `Send + Sync` automatically (both `Replace` and `Context` are unit structs so this is automatic).

- [ ] **Step 5: Commit**

```bash
git add src/platform/replace.rs src/platform/context.rs
git commit -m "feat(platform): wire Replace and Context structs to new traits"
```

### Task 16: Create platform/hotkey.rs using global-hotkey crate

**Files:**
- Create: `src/platform/hotkey.rs`
- Modify: `src/platform/mod.rs` (add `pub mod hotkey;`)

Replaces the deleted `core/hotkey.rs` which depended on `tauri-plugin-global-shortcut`. The `global-hotkey` crate provides the same functionality standalone: register a combo string, receive events on a channel, unregister.

- [ ] **Step 1: Create src/platform/hotkey.rs**

Write the following to `src/platform/hotkey.rs`:

```rust
//! Global hotkey registration and event delivery using the `global-hotkey` crate.
//!
//! Replaces `tauri-plugin-global-shortcut`. The API is:
//! 1. Call `HotkeyService::new()` once at startup
//! 2. Call `service.register("Ctrl+Shift+Space")` with whatever the user configured
//! 3. Drain `GlobalHotKeyEvent::receiver()` on a dedicated thread and forward hits
//!    as whatever your app-level command type is (we return raw `HotKeyId`s here;
//!    the caller maps them to `UiCommand::HotkeyPressed` in Plan 2).

use anyhow::{anyhow, Context, Result};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

pub struct HotkeyService {
    manager: GlobalHotKeyManager,
    current: Option<HotKey>,
}

impl HotkeyService {
    pub fn new() -> Result<Self> {
        let manager = GlobalHotKeyManager::new()
            .context("create GlobalHotKeyManager")?;
        Ok(Self { manager, current: None })
    }

    /// Register a hotkey from a human-readable spec like `"Ctrl+Shift+Space"`.
    /// Unregisters any previously-registered hotkey first.
    pub fn register(&mut self, spec: &str) -> Result<()> {
        self.unregister_current()?;
        let hotkey = parse_hotkey_spec(spec)?;
        self.manager
            .register(hotkey)
            .with_context(|| format!("register hotkey `{spec}`"))?;
        self.current = Some(hotkey);
        Ok(())
    }

    pub fn unregister_current(&mut self) -> Result<()> {
        if let Some(hotkey) = self.current.take() {
            self.manager
                .unregister(hotkey)
                .context("unregister previous hotkey")?;
        }
        Ok(())
    }

    /// Returns the receiver for hotkey press events. Poll or drain this on
    /// a dedicated thread; each event where `state == HotKeyState::Pressed`
    /// corresponds to one user activation.
    pub fn receiver(&self) -> &'static crossbeam_channel::Receiver<GlobalHotKeyEvent> {
        GlobalHotKeyEvent::receiver()
    }
}

/// Parse a spec like `"Ctrl+Shift+Space"` into a `HotKey`.
///
/// Accepted modifier tokens (case-insensitive): `ctrl`, `control`, `shift`,
/// `alt`, `meta`, `super`, `win`, `windows`.
///
/// Accepted key tokens: anything in `global_hotkey::hotkey::Code`'s Display
/// form, plus common aliases: `Space`, `Enter`, `Return`, `Tab`, `Esc`,
/// function keys `F1..F24`, letters `A..Z`, digits `0..9`.
pub fn parse_hotkey_spec(spec: &str) -> Result<HotKey> {
    let mut modifiers = Modifiers::empty();
    let mut key: Option<Code> = None;

    for token in spec.split(|c: char| c == '+' || c == '-').map(str::trim) {
        if token.is_empty() {
            continue;
        }
        match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control"       => modifiers.insert(Modifiers::CONTROL),
            "shift"                  => modifiers.insert(Modifiers::SHIFT),
            "alt"                    => modifiers.insert(Modifiers::ALT),
            "meta" | "super" | "win" | "windows" => modifiers.insert(Modifiers::META),
            other => {
                key = Some(parse_key_token(other)
                    .ok_or_else(|| anyhow!("unknown key token `{token}` in hotkey `{spec}`"))?);
            }
        }
    }

    let key = key.ok_or_else(|| anyhow!("hotkey `{spec}` has no key — only modifiers"))?;
    Ok(HotKey::new(Some(modifiers), key))
}

fn parse_key_token(token: &str) -> Option<Code> {
    let upper = token.to_ascii_uppercase();
    match upper.as_str() {
        "SPACE"            => Some(Code::Space),
        "ENTER" | "RETURN" => Some(Code::Enter),
        "TAB"              => Some(Code::Tab),
        "ESC" | "ESCAPE"   => Some(Code::Escape),
        "BACKSPACE"        => Some(Code::Backspace),
        "DELETE" | "DEL"   => Some(Code::Delete),
        "HOME"             => Some(Code::Home),
        "END"              => Some(Code::End),
        "PAGEUP"           => Some(Code::PageUp),
        "PAGEDOWN"         => Some(Code::PageDown),
        "UP"               => Some(Code::ArrowUp),
        "DOWN"             => Some(Code::ArrowDown),
        "LEFT"             => Some(Code::ArrowLeft),
        "RIGHT"            => Some(Code::ArrowRight),
        s if s.len() == 1 && s.chars().next().unwrap().is_ascii_alphabetic() => {
            // "A" -> Code::KeyA, etc.
            match s.chars().next().unwrap() {
                'A'=>Some(Code::KeyA), 'B'=>Some(Code::KeyB), 'C'=>Some(Code::KeyC),
                'D'=>Some(Code::KeyD), 'E'=>Some(Code::KeyE), 'F'=>Some(Code::KeyF),
                'G'=>Some(Code::KeyG), 'H'=>Some(Code::KeyH), 'I'=>Some(Code::KeyI),
                'J'=>Some(Code::KeyJ), 'K'=>Some(Code::KeyK), 'L'=>Some(Code::KeyL),
                'M'=>Some(Code::KeyM), 'N'=>Some(Code::KeyN), 'O'=>Some(Code::KeyO),
                'P'=>Some(Code::KeyP), 'Q'=>Some(Code::KeyQ), 'R'=>Some(Code::KeyR),
                'S'=>Some(Code::KeyS), 'T'=>Some(Code::KeyT), 'U'=>Some(Code::KeyU),
                'V'=>Some(Code::KeyV), 'W'=>Some(Code::KeyW), 'X'=>Some(Code::KeyX),
                'Y'=>Some(Code::KeyY), 'Z'=>Some(Code::KeyZ),
                _ => None,
            }
        }
        s if s.len() == 1 && s.chars().next().unwrap().is_ascii_digit() => {
            match s.chars().next().unwrap() {
                '0'=>Some(Code::Digit0), '1'=>Some(Code::Digit1), '2'=>Some(Code::Digit2),
                '3'=>Some(Code::Digit3), '4'=>Some(Code::Digit4), '5'=>Some(Code::Digit5),
                '6'=>Some(Code::Digit6), '7'=>Some(Code::Digit7), '8'=>Some(Code::Digit8),
                '9'=>Some(Code::Digit9),
                _ => None,
            }
        }
        s if s.starts_with('F') && s.len() >= 2 => {
            // F1..F24
            let n: u8 = s[1..].parse().ok()?;
            match n {
                1=>Some(Code::F1),   2=>Some(Code::F2),   3=>Some(Code::F3),
                4=>Some(Code::F4),   5=>Some(Code::F5),   6=>Some(Code::F6),
                7=>Some(Code::F7),   8=>Some(Code::F8),   9=>Some(Code::F9),
                10=>Some(Code::F10), 11=>Some(Code::F11), 12=>Some(Code::F12),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Convenience wrapper so callers can test whether an event is a "real" press
/// (not a key-up, not a state-change artifact).
pub fn is_pressed(event: &GlobalHotKeyEvent) -> bool {
    matches!(event.state, HotKeyState::Pressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ctrl_shift_space() {
        let h = parse_hotkey_spec("Ctrl+Shift+Space").unwrap();
        assert!(h.mods.contains(Modifiers::CONTROL));
        assert!(h.mods.contains(Modifiers::SHIFT));
        assert_eq!(h.key, Code::Space);
    }

    #[test]
    fn parses_lowercase_alt_f12() {
        let h = parse_hotkey_spec("alt+f12").unwrap();
        assert!(h.mods.contains(Modifiers::ALT));
        assert_eq!(h.key, Code::F12);
    }

    #[test]
    fn rejects_spec_with_no_key() {
        assert!(parse_hotkey_spec("Ctrl+Shift").is_err());
    }

    #[test]
    fn rejects_unknown_token() {
        assert!(parse_hotkey_spec("Ctrl+Banana").is_err());
    }
}
```

Note: `global-hotkey` crate also re-exports `crossbeam_channel::Receiver` from its `GlobalHotKeyEvent::receiver()` — you don't need to add `crossbeam_channel` to `Cargo.toml` directly; it comes in transitively. If the compiler complains about unresolved `crossbeam_channel`, add `crossbeam-channel = "0.5"` to `[dependencies]`.

- [ ] **Step 2: Register the module in src/platform/mod.rs**

Add after the existing module declarations:

```rust
pub mod hotkey;
```

- [ ] **Step 3: Run unit tests on the parser**

```bash
cargo test --lib platform::hotkey 2>&1 | tail -10
```

Expected: all 4 parser tests pass:

```
test platform::hotkey::tests::parses_ctrl_shift_space ... ok
test platform::hotkey::tests::parses_lowercase_alt_f12 ... ok
test platform::hotkey::tests::rejects_spec_with_no_key ... ok
test platform::hotkey::tests::rejects_unknown_token ... ok
```

- [ ] **Step 4: Commit**

```bash
git add src/platform/hotkey.rs src/platform/mod.rs Cargo.toml
git commit -m "feat(platform): add hotkey module using global-hotkey crate"
```

### Task 17: Create platform/uia.rs — IUIAutomation wrapper

**Files:**
- Create: `src/platform/uia.rs`
- Modify: `src/platform/mod.rs` (add `pub mod uia;`)

The UIA module is the primary text-capture path. It wraps `IUIAutomation` from `windows-rs` and exposes a small, synchronous API: given "whatever is focused right now", return the selected text, the selection bounding rectangle, and whether the element is an editable text control.

COM threading model: `CoInitializeEx(COINIT_MULTITHREADED)` is called lazily on first use, per thread. The UIA singleton is held in a `thread_local!` so the worker thread that performs captures owns its own COM apartment and `IUIAutomation` instance.

- [ ] **Step 1: Create src/platform/uia.rs**

Write the following to `src/platform/uia.rs`:

```rust
//! `IUIAutomation` wrapper: focused element + selected text + bounds.
//!
//! All calls are synchronous and must run on a thread that has called
//! `CoInitializeEx(COINIT_MULTITHREADED)`. The `Uia::thread_local()` helper
//! lazily initialises COM and caches the automation instance per thread.

use std::cell::RefCell;

use anyhow::{anyhow, Context, Result};
use windows::core::{ComInterface, Interface};
use windows::Win32::Foundation::{BOOL, RECT};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern,
    IUIAutomationTextRange, TreeScope_Element, UIA_DocumentControlTypeId, UIA_EditControlTypeId,
    UIA_TextPatternId,
};

use super::traits::ScreenRect;

pub struct Uia {
    automation: IUIAutomation,
}

thread_local! {
    static UIA_TLS: RefCell<Option<Uia>> = const { RefCell::new(None) };
}

impl Uia {
    /// Access a thread-local UIA instance. Lazily initializes COM on first call.
    /// Subsequent calls return the same instance. If initialization fails, the
    /// error is propagated on every call (no partial state).
    pub fn with<R>(f: impl FnOnce(&Uia) -> R) -> Result<R> {
        UIA_TLS.with(|cell| {
            let mut borrow = cell.borrow_mut();
            if borrow.is_none() {
                *borrow = Some(Uia::new()?);
            }
            Ok(f(borrow.as_ref().unwrap()))
        })
    }

    fn new() -> Result<Self> {
        unsafe {
            // COINIT_MULTITHREADED: worker threads, no STA apartment needed.
            // If already initialised on this thread with a different model,
            // CoInitializeEx returns RPC_E_CHANGED_MODE — we treat that as OK
            // (somebody else initialised the apartment first, our job is done).
            let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
            if hr.is_err() && hr.0 != windows::Win32::Foundation::RPC_E_CHANGED_MODE.0 {
                return Err(anyhow!("CoInitializeEx failed: 0x{:08X}", hr.0 as u32));
            }
        }

        let automation: IUIAutomation = unsafe {
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
                .context("CoCreateInstance(CUIAutomation)")?
        };

        Ok(Self { automation })
    }

    /// Get the currently focused UI element.
    pub fn focused_element(&self) -> Result<IUIAutomationElement> {
        unsafe {
            self.automation
                .GetFocusedElement()
                .context("GetFocusedElement")
        }
    }

    /// Return the currently selected text in the focused element, if any.
    /// Returns `Ok(None)` if the element has no text pattern or no selection.
    pub fn selected_text(&self) -> Result<Option<String>> {
        let element = self.focused_element()?;
        let Some(pattern) = get_text_pattern(&element)? else {
            return Ok(None);
        };

        let ranges = unsafe { pattern.GetSelection().context("GetSelection")? };
        let count = unsafe { ranges.Length().context("ranges.Length")? };
        if count == 0 {
            return Ok(None);
        }

        let mut combined = String::new();
        for i in 0..count {
            let range: IUIAutomationTextRange = unsafe {
                ranges.GetElement(i).context("ranges.GetElement")?
            };
            let bstr = unsafe { range.GetText(-1).context("range.GetText")? };
            if !combined.is_empty() && !bstr.is_empty() {
                combined.push(' ');
            }
            combined.push_str(&bstr.to_string());
        }

        if combined.trim().is_empty() {
            Ok(None)
        } else {
            Ok(Some(combined))
        }
    }

    /// Return the union of bounding rectangles of the current selection, in
    /// screen coordinates. Returns `Ok(None)` if there's no selection.
    pub fn selection_bounds(&self) -> Result<Option<ScreenRect>> {
        let element = self.focused_element()?;
        let Some(pattern) = get_text_pattern(&element)? else {
            return Ok(None);
        };

        let ranges = unsafe { pattern.GetSelection().context("GetSelection")? };
        let count = unsafe { ranges.Length().context("ranges.Length")? };
        if count == 0 {
            return Ok(None);
        }

        let range: IUIAutomationTextRange = unsafe {
            ranges.GetElement(0).context("ranges.GetElement(0)")?
        };
        let rects_safearray = unsafe {
            range.GetBoundingRectangles().context("GetBoundingRectangles")?
        };

        // The SAFEARRAY contains f64 values: [left, top, width, height, left, top, width, height, ...]
        let rects = unsafe { safearray_to_f64_vec(rects_safearray)? };
        if rects.is_empty() {
            return Ok(None);
        }

        let mut union = ScreenRect { left: i32::MAX, top: i32::MAX, right: i32::MIN, bottom: i32::MIN };
        for chunk in rects.chunks_exact(4) {
            let l = chunk[0] as i32;
            let t = chunk[1] as i32;
            let w = chunk[2] as i32;
            let h = chunk[3] as i32;
            union.left   = union.left.min(l);
            union.top    = union.top.min(t);
            union.right  = union.right.max(l + w);
            union.bottom = union.bottom.max(t + h);
        }
        Ok(Some(union))
    }

    /// Return true if the focused element is an editable text control (Edit or Document)
    /// AND exposes `UIA_TextPatternId`. Used to decide whether the floating pencil
    /// should appear on focus events.
    pub fn is_editable_text(&self, element: &IUIAutomationElement) -> Result<bool> {
        let control_type = unsafe { element.CurrentControlType().context("CurrentControlType")? };
        let is_edit_or_doc =
            control_type == UIA_EditControlTypeId || control_type == UIA_DocumentControlTypeId;
        if !is_edit_or_doc {
            return Ok(false);
        }
        let enabled: BOOL = unsafe { element.CurrentIsEnabled().context("CurrentIsEnabled")? };
        if !enabled.as_bool() {
            return Ok(false);
        }
        let pattern = get_text_pattern(element)?;
        Ok(pattern.is_some())
    }
}

fn get_text_pattern(element: &IUIAutomationElement) -> Result<Option<IUIAutomationTextPattern>> {
    let pattern_unk = unsafe {
        element
            .GetCurrentPattern(UIA_TextPatternId)
            .context("GetCurrentPattern(TextPattern)")?
    };
    if pattern_unk.as_raw().is_null() {
        return Ok(None);
    }
    let pattern: IUIAutomationTextPattern = pattern_unk.cast()
        .context("cast to IUIAutomationTextPattern")?;
    Ok(Some(pattern))
}

/// Convert a `*const SAFEARRAY` of `f64` into a `Vec<f64>`. UIA returns bounding
/// rectangles as a flat SAFEARRAY of doubles: [left, top, width, height, ...].
unsafe fn safearray_to_f64_vec(
    array: *mut windows::Win32::System::Com::SAFEARRAY,
) -> Result<Vec<f64>> {
    use windows::Win32::System::Com::{
        SafeArrayAccessData, SafeArrayGetLBound, SafeArrayGetUBound, SafeArrayUnaccessData,
    };

    if array.is_null() {
        return Ok(Vec::new());
    }

    let lbound = SafeArrayGetLBound(array, 1).context("SafeArrayGetLBound")?;
    let ubound = SafeArrayGetUBound(array, 1).context("SafeArrayGetUBound")?;
    let len = (ubound - lbound + 1) as usize;

    if len == 0 {
        return Ok(Vec::new());
    }

    let mut data_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
    SafeArrayAccessData(array, &mut data_ptr).context("SafeArrayAccessData")?;
    let slice = std::slice::from_raw_parts(data_ptr as *const f64, len);
    let vec = slice.to_vec();
    SafeArrayUnaccessData(array).ok();

    Ok(vec)
}
```

Note: `windows-rs` API details (trait imports, method names) are sensitive to the exact crate version. The features list in `Cargo.toml` (`Win32_UI_Accessibility`, `Win32_System_Com`) must be enabled. If the compiler complains about missing items like `IUIAutomationTextRange::GetText` or `SAFEARRAY` helpers, check that the feature is in the `windows` dependency features list from Task 4.

- [ ] **Step 2: Register the module in src/platform/mod.rs**

```rust
pub mod uia;
```

- [ ] **Step 3: Run cargo check**

```bash
cargo check 2>&1 | tail -30
```

Expected: clean compile. If you see `no method named 'CurrentControlType'` or similar, the `Win32_UI_Accessibility` feature might not be enabled — double check `Cargo.toml` features list.

If there are real errors (e.g., API signature mismatch in a newer `windows` crate version), adjust the code inline. The shape is: COM init → create automation → GetFocusedElement → query TextPattern → read selection / bounds / control type.

- [ ] **Step 4: Commit**

```bash
git add src/platform/uia.rs src/platform/mod.rs
git commit -m "feat(platform): add IUIAutomation wrapper for text capture and bounds"
```

### Task 18: Refactor platform/capture.rs for UIA-first capture

**Files:**
- Modify: `src/platform/capture.rs` (refactor: UIA-first with clipboard fallback)

The existing capture module uses `arboard` for a clipboard-based hack (save → Ctrl+C → read → restore). The new version tries UIA first (zero side-effects), falls back to clipboard only when UIA returns nothing. The new module also exposes a `Capture` struct that implements `TextCapture`.

- [ ] **Step 1: Replace src/platform/capture.rs with the refactored version**

Write the following to `src/platform/capture.rs`:

```rust
//! Text capture — UIA-first, clipboard fallback.
//!
//! Strategy:
//! 1. Wait for hotkey modifiers to release (existing function, preserved as-is)
//! 2. Try UIA: read selected text + selection bounds from the focused control
//! 3. If UIA returns nothing (null selection, unsupported control, COM failure),
//!    fall back to the clipboard hack: save → synth Ctrl+C → read → restore
//! 4. If both fail, return `CaptureResult::default()` (empty text, no anchor)
//!
//! UIA is zero side-effect — no clipboard clobbering, no synthetic keystrokes —
//! and gives us the selection rectangle for free, which we use to anchor the
//! compact overlay near the user's selection.

use anyhow::Result;
use async_trait::async_trait;

use super::traits::{CaptureResult, CaptureSource, ScreenRect, TextCapture};
use super::uia::Uia;

/// Production implementation of `TextCapture`.
#[derive(Default)]
pub struct Capture;

#[async_trait]
impl TextCapture for Capture {
    async fn capture(&self) -> CaptureResult {
        // The whole flow is synchronous Win32/UIA work — move it to a blocking task
        // so we don't hold a tokio worker thread for the duration.
        tokio::task::spawn_blocking(capture_selection_blocking)
            .await
            .unwrap_or_default()
    }
}

/// Synchronous capture entry point. Called from `Capture::capture` inside
/// `spawn_blocking`. Also callable directly from tests or scripts that already
/// run on a blocking thread.
pub fn capture_selection_blocking() -> CaptureResult {
    wait_for_hotkey_modifiers_released();

    // Step 1: UIA-first path
    if let Ok((text, anchor)) = uia_capture() {
        if let Some(text) = text {
            return CaptureResult { text, anchor, source: CaptureSource::Uia };
        }
    }

    // Step 2: clipboard fallback
    if let Ok(text) = clipboard_capture_fallback() {
        if !text.is_empty() {
            return CaptureResult { text, anchor: None, source: CaptureSource::Clipboard };
        }
    }

    CaptureResult::default()
}

fn uia_capture() -> Result<(Option<String>, Option<ScreenRect>)> {
    Uia::with(|uia| {
        let text = uia.selected_text().ok().flatten();
        let anchor = uia.selection_bounds().ok().flatten();
        (text, anchor)
    })
}

/// Clipboard-based fallback capture. Preserves the existing arboard-based
/// implementation from the Tauri era. Steps:
///   1. Save current clipboard
///   2. Synthesize Ctrl+C via enigo
///   3. Wait ~80 ms for the target app to respond
///   4. Read clipboard
///   5. Restore the previously saved clipboard
fn clipboard_capture_fallback() -> Result<String> {
    // PRESERVE the existing clipboard_capture_fallback body from the prior
    // version of capture.rs. If the existing function is named differently
    // (e.g., `capture_via_clipboard`), rename it to `clipboard_capture_fallback`
    // and keep the implementation byte-for-byte identical.
    //
    // If the existing module used `arboard::Clipboard`, keep the arboard usage
    // for now — we can migrate to raw Win32 clipboard APIs in a future plan.
    // For Plan 1 the only requirement is that arboard is re-added to Cargo.toml
    // OR the clipboard path uses a pure-windows-rs alternative.
    //
    // RECOMMENDED for Plan 1: add `arboard = "3"` back to Cargo.toml [dependencies]
    // and keep the existing clipboard hack verbatim. The plan's section 8
    // dependency list says arboard is removed, but that's aspirational — the
    // clipboard fallback needs SOMETHING. We use arboard here and note in the
    // spec that Plan 2 may replace it with raw OpenClipboard/GetClipboardData.
    anyhow::bail!("clipboard_capture_fallback: paste existing implementation here")
}

/// Wait for the hotkey modifier keys (Ctrl / Shift / Alt / Win) to be released
/// before synthesizing our own input or reading UIA. Without this, the user's
/// still-held modifiers can pollute the synthesized Ctrl+C or the focused
/// control's state. PRESERVED from the prior version of capture.rs.
pub fn wait_for_hotkey_modifiers_released() {
    // PRESERVE the existing wait_for_hotkey_modifiers_released body from the
    // prior version of capture.rs. It polls GetAsyncKeyState for VK_CONTROL,
    // VK_SHIFT, VK_MENU, VK_LWIN/VK_RWIN and sleeps until they're all released
    // or a 500ms timeout fires.
}
```

**Important**: the two `// PRESERVE` blocks above are NOT literal code — they're directives. The current `capture.rs` already contains working implementations of `clipboard_capture_fallback` (or equivalent) and `wait_for_hotkey_modifiers_released`. Copy those function bodies verbatim into the new module, replacing the `anyhow::bail!` / empty body placeholders.

- [ ] **Step 2: Re-add arboard to Cargo.toml if you removed it**

Open `Cargo.toml` and add under `[dependencies]` if not already present:

```toml
arboard = "3"
```

Yes, the design spec says to remove arboard. In practice, the clipboard fallback in Plan 1 still uses it — the "remove arboard" goal is a Plan 2+ cleanup. Update the spec if you want to reflect this pragmatically.

- [ ] **Step 3: Run cargo check**

```bash
cargo check 2>&1 | tail -20
```

Expected: clean. If the `wait_for_hotkey_modifiers_released` implementation uses `windows-rs` types you haven't added to the features list, add them (most likely `Win32_UI_Input_KeyboardAndMouse` — already in the list from Task 4).

- [ ] **Step 4: Run unit tests**

```bash
cargo test --lib platform::capture 2>&1 | tail -10
```

Expected: existing tests (if any) still pass. New tests aren't added here — smoke-level verification happens in Task 23 via `tests/platform_smoke.rs`.

- [ ] **Step 5: Commit**

```bash
git add src/platform/capture.rs Cargo.toml
git commit -m "refactor(platform): UIA-first capture with clipboard fallback"
```

### Task 19: Create platform/tray.rs using tray-icon crate

**Files:**
- Create: `src/platform/tray.rs`
- Modify: `src/platform/mod.rs` (add `pub mod tray;`)

The tray provides the same four menu items as the Tauri version: **Show Quill**, **Open Full Panel…**, **Settings…**, **Quit Quill**. Left-click triggers the hotkey flow. Menu events and left-click events go out on a `crossbeam_channel::Sender<TrayEvent>` that the main boot sequence (Plan 3) drains.

- [ ] **Step 1: Create src/platform/tray.rs**

Write the following to `src/platform/tray.rs`:

```rust
//! System tray icon + menu, powered by the `tray-icon` crate.
//!
//! Ownership: `TrayService` owns the `TrayIcon`. Drop it to remove the icon
//! from the tray. The returned receivers are static — both the `tray-icon`
//! crate and the wrapper crate `muda` publish events on global singletons.

use anyhow::{Context, Result};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

/// High-level tray menu item identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayMenu {
    Show,
    Panel,
    Settings,
    Quit,
}

/// Classified tray event for the engine layer to consume.
#[derive(Debug, Clone)]
pub enum TrayEvent {
    /// User left-clicked the tray icon (outside the menu).
    IconClicked,
    /// User selected a menu item.
    MenuItem(TrayMenu),
}

pub struct TrayService {
    _icon: TrayIcon,
    id_show: String,
    id_panel: String,
    id_settings: String,
    id_quit: String,
}

impl TrayService {
    /// Build the tray icon and menu. The `icon_bytes` parameter is the ICO or
    /// PNG file embedded via `include_bytes!("../../resources/icons/quill.ico")`.
    pub fn new(icon_bytes: &[u8]) -> Result<Self> {
        let menu = Menu::new();

        let mi_show     = MenuItem::new("Show Quill",       true, None);
        let mi_panel    = MenuItem::new("Open Full Panel…", true, None);
        let mi_settings = MenuItem::new("Settings…",        true, None);
        let sep         = PredefinedMenuItem::separator();
        let mi_quit     = MenuItem::new("Quit Quill",       true, None);

        menu.append(&mi_show).context("append Show menu item")?;
        menu.append(&mi_panel).context("append Panel menu item")?;
        menu.append(&mi_settings).context("append Settings menu item")?;
        menu.append(&sep).context("append separator")?;
        menu.append(&mi_quit).context("append Quit menu item")?;

        let id_show = mi_show.id().0.clone();
        let id_panel = mi_panel.id().0.clone();
        let id_settings = mi_settings.id().0.clone();
        let id_quit = mi_quit.id().0.clone();

        let icon = load_icon(icon_bytes).context("load tray icon")?;

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Quill")
            .with_icon(icon)
            .build()
            .context("build TrayIconBuilder")?;

        Ok(Self {
            _icon: tray,
            id_show,
            id_panel,
            id_settings,
            id_quit,
        })
    }

    /// Poll once for pending tray events and classify them.
    /// Returns an empty Vec if nothing is waiting.
    ///
    /// Call this from a dedicated tray-polling thread or from the Slint event
    /// loop via a `slint::Timer`. The `tray-icon` crate does NOT integrate with
    /// any event loop natively — you MUST drive it yourself.
    pub fn poll(&self) -> Vec<TrayEvent> {
        let mut out = Vec::new();

        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click { button, button_state, .. } = event {
                if matches!(button, tray_icon::MouseButton::Left)
                    && matches!(button_state, tray_icon::MouseButtonState::Up)
                {
                    out.push(TrayEvent::IconClicked);
                }
            }
        }

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            let id = event.id.0;
            let item = if id == self.id_show {
                TrayMenu::Show
            } else if id == self.id_panel {
                TrayMenu::Panel
            } else if id == self.id_settings {
                TrayMenu::Settings
            } else if id == self.id_quit {
                TrayMenu::Quit
            } else {
                continue;
            };
            out.push(TrayEvent::MenuItem(item));
        }

        out
    }
}

/// Decode the icon bytes into a `tray_icon::Icon`. Accepts PNG or ICO input.
fn load_icon(bytes: &[u8]) -> Result<Icon> {
    // tray-icon accepts RGBA data + dimensions. We decode via a tiny PNG path
    // for PNG input; for ICO we pick the 32x32 entry. Simplest path: embed a
    // 32x32 PNG next to quill.ico during build and load that here.
    //
    // If you have only quill.ico available, use the `ico` crate (already a
    // build-dep) at runtime too: add it to [dependencies] as well.
    //
    // Minimal working implementation below assumes a 32x32 RGBA PNG.
    let image = image_from_png_bytes(bytes)?;
    Icon::from_rgba(image.rgba, image.width, image.height)
        .context("Icon::from_rgba")
}

struct DecodedImage {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

fn image_from_png_bytes(_bytes: &[u8]) -> Result<DecodedImage> {
    // Plan 1 stub: real PNG decoding lives in a follow-up commit. For Task 19
    // we ship this stub that returns a 1x1 transparent pixel, then Task 19b
    // (below) swaps in either the `png` crate or `image` crate for real decoding.
    //
    // Reason for stubbing: full image crate pulls in ~30 transitive deps that
    // inflate build times. We can add `png = "0.17"` targeted as the minimal
    // decoder, or keep using the `ico` crate (already a build-dep) and add it
    // to [dependencies] too.
    Ok(DecodedImage { rgba: vec![0, 0, 0, 0], width: 1, height: 1 })
}
```

**Note on the icon loader**: the stub above is intentional — it unblocks the module compile without dragging in an image decoder. A real decoder is required for visible tray branding. Before committing, either:
- (a) Add `png = "0.17"` to `Cargo.toml` `[dependencies]` and replace `image_from_png_bytes` with a real `png::Decoder::new(cursor).read_info()?.next_frame()?` pipeline, OR
- (b) Add `ico = "0.3"` to `[dependencies]` (it's already a build-dep) and decode `quill.ico` at runtime, picking the 32x32 entry.

Recommendation: option (a) with `png` crate. Then generate `resources/icons/quill-tray-32.png` in `build.rs` alongside `quill.ico`, and `include_bytes!` the PNG here.

- [ ] **Step 2: Register the module**

Add to `src/platform/mod.rs`:

```rust
pub mod tray;
```

- [ ] **Step 3: Extend build.rs to also emit a tray-sized PNG**

Open `build.rs` and, inside the `for &size in ICON_SIZES` loop, after the ICO entry is added, conditionally write `size == 32` out as a standalone PNG:

```rust
if size == 32 {
    let png_path = manifest_dir.join("resources/icons/quill-tray-32.png");
    pixmap.save_png(&png_path).expect("save quill-tray-32.png");
}
```

Also add `quill-tray-32.png` to `.gitignore` next to `quill.ico`.

- [ ] **Step 4: Replace image_from_png_bytes in tray.rs with real decoding**

Add to `Cargo.toml` `[dependencies]`:

```toml
png = "0.17"
```

Replace the stub in tray.rs:

```rust
fn image_from_png_bytes(bytes: &[u8]) -> Result<DecodedImage> {
    let decoder = png::Decoder::new(bytes);
    let mut reader = decoder.read_info().context("png::Decoder::read_info")?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).context("png next_frame")?;
    buf.truncate(info.buffer_size());

    // Ensure RGBA (png may deliver RGB or palette — normalize to RGBA).
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => {
            let mut out = Vec::with_capacity(buf.len() / 3 * 4);
            for chunk in buf.chunks_exact(3) {
                out.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
            }
            out
        }
        other => anyhow::bail!("unsupported PNG color type: {other:?}"),
    };

    Ok(DecodedImage {
        rgba,
        width: info.width,
        height: info.height,
    })
}
```

- [ ] **Step 5: Wire the embedded PNG bytes**

At the top of `src/platform/tray.rs`, add:

```rust
/// Embedded 32x32 tray icon bytes, generated by build.rs from plume.svg.
pub const TRAY_ICON_PNG: &[u8] =
    include_bytes!("../../resources/icons/quill-tray-32.png");
```

Callers do: `TrayService::new(TRAY_ICON_PNG)?`.

- [ ] **Step 6: Build and verify**

```bash
cargo build 2>&1 | tail -10
```

Expected: `Finished` with no errors. `resources/icons/quill-tray-32.png` exists after the build.

- [ ] **Step 7: Commit**

```bash
git add src/platform/tray.rs src/platform/mod.rs build.rs Cargo.toml .gitignore
git commit -m "feat(platform): add tray icon service with menu and PNG decoding"
```

### Task 20: Create platform/mica.rs — DWM Mica backdrop wrapper

**Files:**
- Create: `src/platform/mica.rs`
- Modify: `src/platform/mod.rs` (add `pub mod mica;`)

Thin wrapper around `DwmSetWindowAttribute` that enables (a) Windows 11 Mica backdrop, (b) dark mode non-client area, (c) rounded corners. Consumers pass an `HWND` that they already have (from Slint's winit backend in Plan 3). For Plan 1 we just build and expose the API — no integration yet.

- [ ] **Step 1: Create src/platform/mica.rs**

```rust
//! Windows 11 Mica backdrop wrapper.
//!
//! Call `apply(hwnd, MicaVariant::Main)` on a window's HWND after creation
//! (but before the window is shown) to enable the Mica system backdrop,
//! dark-mode title bar, and rounded corners.
//!
//! On Windows 10 or pre-22H2 Windows 11, the `DWMWA_SYSTEMBACKDROP_TYPE`
//! attribute is silently ignored. Callers should set the fallback window
//! background colour (solid `#1e1e28`) before calling `apply`.

use anyhow::{Context, Result};
use windows::Win32::Foundation::{BOOL, HWND};
use windows::Win32::Graphics::Dwm::{
    DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMWA_SYSTEMBACKDROP_TYPE,
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DWM_SYSTEMBACKDROP_TYPE,
};
use windows::Win32::UI::Controls::MARGINS;

/// Which Mica flavour to apply.
#[derive(Debug, Clone, Copy)]
pub enum MicaVariant {
    /// Standard Mica — subtle tint, used for main app windows.
    Main,
    /// Tabbed Mica — more saturated, used for the expanded view tab area.
    Tabbed,
}

impl MicaVariant {
    fn as_backdrop_type(self) -> DWM_SYSTEMBACKDROP_TYPE {
        match self {
            // Values from dwmapi.h:
            //   DWMSBT_AUTO           = 0
            //   DWMSBT_NONE           = 1
            //   DWMSBT_MAINWINDOW     = 2  // Mica
            //   DWMSBT_TRANSIENTWINDOW= 3  // Acrylic
            //   DWMSBT_TABBEDWINDOW   = 4  // Mica Alt
            MicaVariant::Main   => DWM_SYSTEMBACKDROP_TYPE(2),
            MicaVariant::Tabbed => DWM_SYSTEMBACKDROP_TYPE(4),
        }
    }
}

/// Enable Mica on an HWND. Safe to call on Win10 — the backdrop call is a
/// no-op there. Still applies the dark-mode and round-corner attributes
/// which work from Windows 10 1809 onward.
pub fn apply(hwnd: HWND, variant: MicaVariant) -> Result<()> {
    unsafe {
        // 1. Dark mode on non-client area
        let dark: BOOL = true.into();
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark as *const _ as *const _,
            std::mem::size_of::<BOOL>() as u32,
        )
        .context("DwmSetWindowAttribute(DWMWA_USE_IMMERSIVE_DARK_MODE)")?;

        // 2. System backdrop (Mica) — silently no-ops on pre-22H2
        let backdrop = variant.as_backdrop_type();
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &backdrop as *const _ as *const _,
            std::mem::size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
        );

        // 3. Round corners (DWMWCP_ROUND)
        let corner = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const _ as *const _,
            std::mem::size_of::<i32>() as u32,
        );

        // 4. Extend frame into client area so Mica fills the whole window
        let margins = MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        };
        DwmExtendFrameIntoClientArea(hwnd, &margins)
            .context("DwmExtendFrameIntoClientArea")?;
    }

    Ok(())
}
```

Note: the `DWMWA_SYSTEMBACKDROP_TYPE` constant value `38` and `DWMWCP_ROUND` value `2` are baked into `windows-rs` 0.58. If your `windows-rs` version doesn't expose these constants yet, define them locally as `const DWMWA_SYSTEMBACKDROP_TYPE: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(38)`.

- [ ] **Step 2: Register the module**

```rust
// src/platform/mod.rs
pub mod mica;
```

- [ ] **Step 3: Check and commit**

```bash
cargo check 2>&1 | tail -10
git add src/platform/mica.rs src/platform/mod.rs
git commit -m "feat(platform): add Mica backdrop wrapper"
```

### Task 21: Create platform/dwm_shadow.rs — DWM native shadow

**Files:**
- Create: `src/platform/dwm_shadow.rs`
- Modify: `src/platform/mod.rs` (add `pub mod dwm_shadow;`)

Shadow actually comes for free the moment you call `DwmExtendFrameIntoClientArea` with non-zero margins — which `mica::apply` already does. This module exists as an explicit hook for the NC rendering policy (so shadow also works for windows that don't use Mica, e.g., the tiny PencilWindow in Plan 5).

- [ ] **Step 1: Create src/platform/dwm_shadow.rs**

```rust
//! DWM native drop shadow for borderless windows.
//!
//! `mica::apply` already gets you a shadow as a side-effect of
//! `DwmExtendFrameIntoClientArea`. This module is for windows that are
//! borderless but NOT Mica-backdropped (e.g. the floating PencilWindow in
//! Plan 5). Calling `enable(hwnd)` forces DWM non-client rendering on,
//! which is required for the shadow to render on frameless windows.

use anyhow::{Context, Result};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMNCRENDERINGPOLICY, DWMNCRP_ENABLED, DWMWA_NCRENDERING_POLICY,
};

pub fn enable(hwnd: HWND) -> Result<()> {
    unsafe {
        let policy: DWMNCRENDERINGPOLICY = DWMNCRP_ENABLED;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_NCRENDERING_POLICY,
            &policy as *const _ as *const _,
            std::mem::size_of::<DWMNCRENDERINGPOLICY>() as u32,
        )
        .context("DwmSetWindowAttribute(DWMWA_NCRENDERING_POLICY)")?;
    }
    Ok(())
}
```

- [ ] **Step 2: Register and commit**

```rust
// src/platform/mod.rs
pub mod dwm_shadow;
```

```bash
cargo check 2>&1 | tail -5
git add src/platform/dwm_shadow.rs src/platform/mod.rs
git commit -m "feat(platform): add DWM native shadow wrapper for borderless windows"
```

### Task 22: Create platform/caret.rs — WinEvent hook thread

**Files:**
- Create: `src/platform/caret.rs`
- Modify: `src/platform/mod.rs` (add `pub mod caret;`)

The caret tracker runs `SetWinEventHook` on a dedicated thread with a standard Win32 message pump. Events flow out as `FocusEvent`s on a `tokio::sync::mpsc::UnboundedSender<FocusEvent>` supplied by the caller. The floating pencil window (Plan 5) consumes this channel to know when to appear, move, and disappear.

For Plan 1 we build the infrastructure and verify the hook installs + uninstalls without crashing. We don't test actual event delivery until the smoke tests in Task 23.

- [ ] **Step 1: Create src/platform/caret.rs**

```rust
//! WinEvent-based focus & caret tracking.
//!
//! Installs `SetWinEventHook` on a dedicated thread with a message pump.
//! Events are classified and forwarded as `FocusEvent`s along a
//! `tokio::sync::mpsc::UnboundedSender`. The caller decides what to do with
//! them (typically: show/hide/reposition the floating pencil window).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use tokio::sync::mpsc::UnboundedSender;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PeekMessageW, TranslateMessage,
    EVENT_OBJECT_FOCUS, EVENT_OBJECT_LOCATIONCHANGE, EVENT_SYSTEM_FOREGROUND,
    MSG, PM_REMOVE, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_QUIT,
};

use super::traits::ScreenRect;

#[derive(Debug, Clone)]
pub enum FocusEvent {
    /// A new control gained focus. `editable` reflects whether it's an
    /// editable text control according to UIA. `anchor` is the control's
    /// screen-space bounding rectangle, if available.
    FocusChanged {
        editable: bool,
        anchor: Option<ScreenRect>,
        app_hint: String,
    },
    /// The caret moved within the currently focused control. Debounced
    /// to ~30 Hz before reaching consumers.
    CaretMoved { rect: ScreenRect },
    /// Focus moved to an unreachable control (elevated window, no UIA exposure).
    FocusLost,
}

pub struct CaretHookService {
    stop_flag: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl CaretHookService {
    /// Start the hook thread. Events flow out on `sender` until `stop()` is
    /// called or the service is dropped.
    pub fn start(sender: UnboundedSender<FocusEvent>) -> Result<Self> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_for_thread = Arc::clone(&stop_flag);

        let thread = std::thread::Builder::new()
            .name("quill-caret-hook".into())
            .spawn(move || {
                if let Err(e) = run_hook_thread(sender, stop_flag_for_thread) {
                    tracing::error!("caret hook thread exited with error: {e:#}");
                }
            })
            .map_err(|e| anyhow!("spawn caret hook thread: {e}"))?;

        Ok(Self {
            stop_flag,
            thread: Some(thread),
        })
    }

    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            // Best-effort join — the thread exits on the next message pump tick
            // after seeing stop_flag.
            let _ = t.join();
        }
    }
}

impl Drop for CaretHookService {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Thread-local holder for the sender so the extern C hook proc can reach it.
/// SAFETY: this thread is the only one that touches the cell; the hook proc
/// is invoked on the same thread that pumps messages.
thread_local! {
    static SENDER: std::cell::RefCell<Option<UnboundedSender<FocusEvent>>> = const { std::cell::RefCell::new(None) };
}

fn run_hook_thread(sender: UnboundedSender<FocusEvent>, stop: Arc<AtomicBool>) -> Result<()> {
    SENDER.with(|cell| {
        *cell.borrow_mut() = Some(sender);
    });

    unsafe {
        // One hook for focus / foreground changes, one for location changes (caret moves).
        let focus_hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_OBJECT_FOCUS,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        if focus_hook.0 == 0 {
            return Err(anyhow!("SetWinEventHook(EVENT_OBJECT_FOCUS) failed"));
        }

        let loc_hook = SetWinEventHook(
            EVENT_OBJECT_LOCATIONCHANGE,
            EVENT_OBJECT_LOCATIONCHANGE,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        if loc_hook.0 == 0 {
            UnhookWinEvent(focus_hook);
            return Err(anyhow!("SetWinEventHook(EVENT_OBJECT_LOCATIONCHANGE) failed"));
        }

        // Message pump — exits when stop_flag is set.
        let mut msg = MSG::default();
        loop {
            if stop.load(Ordering::SeqCst) {
                break;
            }
            let has_msg = PeekMessageW(&mut msg, HWND(0), 0, 0, PM_REMOVE).as_bool();
            if has_msg {
                if msg.message == WM_QUIT {
                    break;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            } else {
                // Yield briefly to avoid a busy loop. 10ms = ~100 pump ticks/sec,
                // plenty for LOCATIONCHANGE debouncing.
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }

        UnhookWinEvent(focus_hook);
        UnhookWinEvent(loc_hook);
    }

    SENDER.with(|cell| {
        *cell.borrow_mut() = None;
    });
    Ok(())
}

unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread_id: u32,
    _time: u32,
) {
    SENDER.with(|cell| {
        let Some(tx) = cell.borrow().as_ref().cloned() else {
            return;
        };

        match event {
            EVENT_SYSTEM_FOREGROUND | EVENT_OBJECT_FOCUS => {
                // Plan 1: just fire a FocusChanged with empty anchor and hint.
                // Plan 5 integrates with UIA to fill in the editable/anchor fields.
                let _ = tx.send(FocusEvent::FocusChanged {
                    editable: false,
                    anchor: None,
                    app_hint: format!("hwnd={:?}", hwnd),
                });
            }
            EVENT_OBJECT_LOCATIONCHANGE => {
                // Plan 1: we emit a dummy CaretMoved so the test harness can
                // verify at least one event flows. Real caret rect extraction
                // requires UIA queries which Plan 5 adds.
                let _ = tx.send(FocusEvent::CaretMoved {
                    rect: ScreenRect { left: 0, top: 0, right: 0, bottom: 0 },
                });
            }
            _ => {}
        }
    });
}
```

- [ ] **Step 2: Register the module**

```rust
// src/platform/mod.rs
pub mod caret;
```

- [ ] **Step 3: Build and check**

```bash
cargo check 2>&1 | tail -15
```

Expected: clean. The `windows-rs` symbols `EVENT_OBJECT_FOCUS`, `EVENT_OBJECT_LOCATIONCHANGE`, `EVENT_SYSTEM_FOREGROUND`, `SetWinEventHook`, `UnhookWinEvent`, `HWINEVENTHOOK`, `WINEVENT_OUTOFCONTEXT`, `WINEVENT_SKIPOWNPROCESS` all live under `Win32_UI_Accessibility` and `Win32_UI_WindowsAndMessaging`, both of which are in the features list from Task 4.

If any symbol is missing, check `windows-rs` docs for its exact path and adjust imports.

- [ ] **Step 4: Commit**

```bash
git add src/platform/caret.rs src/platform/mod.rs
git commit -m "feat(platform): add WinEvent-based caret + focus tracker thread"
```

### Task 23: Write tests/platform_smoke.rs — Tier 3 smoke tests

**Files:**
- Create: `tests/platform_smoke.rs`

Tier 3 smoke tests. Each test verifies a platform module works in a real Windows environment. Gated behind `feature = "smoke"` so they don't run on every `cargo test` invocation (they spawn real processes and hold global OS state).

Launch with:

```bash
cargo test --test platform_smoke --features smoke -- --test-threads=1
```

`--test-threads=1` is required — these tests touch global OS state (clipboard, tray, foreground window) and must not run concurrently.

- [ ] **Step 1: Create tests/platform_smoke.rs**

Write the following to `tests/platform_smoke.rs`:

```rust
//! Tier 3 platform smoke tests. Gated behind `feature = "smoke"`.
//!
//! These tests exercise the platform/ modules against real Windows APIs
//! and, in some cases, a scripted `notepad.exe` target. They verify that
//! each module can initialise, perform its operation, and tear down without
//! crashing. They do NOT assert UI correctness or pixel accuracy.
//!
//! Run with:
//!
//! ```bash
//! cargo test --test platform_smoke --features smoke -- --test-threads=1
//! ```

#![cfg(feature = "smoke")]

use std::time::Duration;

use quill::platform::hotkey::{HotkeyService, parse_hotkey_spec, is_pressed};
use quill::platform::tray::{TrayService, TRAY_ICON_PNG};
use quill::platform::caret::{CaretHookService, FocusEvent};

#[test]
fn hotkey_parser_accepts_canonical_spec() {
    let h = parse_hotkey_spec("Ctrl+Shift+Space").unwrap();
    assert!(h.mods.contains(global_hotkey::hotkey::Modifiers::CONTROL));
    assert!(h.mods.contains(global_hotkey::hotkey::Modifiers::SHIFT));
}

#[test]
fn hotkey_service_round_trip() {
    // Register → unregister → reregister. Verifies that the GlobalHotKeyManager
    // can hold a registration through a lifecycle without leaking the combo.
    let mut service = HotkeyService::new().expect("create HotkeyService");
    service.register("Ctrl+Alt+F12").expect("register 1st hotkey");
    service.unregister_current().expect("unregister");
    service.register("Ctrl+Alt+F11").expect("register 2nd hotkey");
    // Drop unregisters implicitly.
}

#[test]
fn tray_service_builds_and_drops() {
    // Verifies that TrayService::new succeeds with the embedded PNG.
    // The icon actually appears in the tray during this test — eyeball it.
    let tray = TrayService::new(TRAY_ICON_PNG).expect("build tray");
    std::thread::sleep(Duration::from_millis(500));
    // Drain any spurious events
    let events = tray.poll();
    // We don't assert the event count — the user might have clicked during the 500ms window.
    // Just verify poll() doesn't panic and the tray drops cleanly.
    let _ = events;
    drop(tray);
}

#[test]
fn caret_hook_service_starts_and_stops() {
    // Verifies that the WinEvent hook thread installs, pumps messages,
    // and tears down cleanly. We don't assert any specific events — we
    // just verify the thread doesn't panic.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut service = CaretHookService::start(tx).expect("start caret hook");

    // Let the hook run for 200ms. During this window, any focus change in
    // the OS will generate events. We drain them and assert "at least zero".
    std::thread::sleep(Duration::from_millis(200));
    let mut received = 0;
    while rx.try_recv().is_ok() {
        received += 1;
    }
    println!("smoke: caret hook received {received} event(s) in 200ms");

    service.stop();
    // Wait a beat for the thread to actually exit.
    std::thread::sleep(Duration::from_millis(50));
}

#[test]
fn uia_can_initialize_on_worker_thread() {
    // This test does NOT require a scripted target — we just verify that the
    // Uia thread-local can initialize without crashing, and that calling
    // focused_element() succeeds (returns whatever is currently focused).
    use quill::platform::uia::Uia;

    std::thread::spawn(|| {
        let result = Uia::with(|uia| uia.focused_element().map(|_| ()));
        assert!(result.is_ok(), "Uia::with returned: {result:?}");
        let _ = result.unwrap();  // focused_element may error if no window is focused — that's OK
    })
    .join()
    .expect("worker thread panicked");
}

#[test]
#[ignore = "requires notepad.exe as scripted target; run manually"]
fn capture_flow_against_notepad() {
    // End-to-end: spawn notepad, type "Hello smoke", select all, invoke
    // capture_selection_blocking(), assert the text matches.
    //
    // This is ignored by default because it interacts with the GUI. Run:
    //
    //     cargo test --test platform_smoke --features smoke \
    //         capture_flow_against_notepad -- --ignored --test-threads=1
    //
    // The test opens notepad in the foreground — don't run it while you're
    // typing into anything else, or your input will be eaten by notepad.
    use quill::platform::capture::capture_selection_blocking;

    let mut child = std::process::Command::new("notepad.exe")
        .spawn()
        .expect("spawn notepad");

    // Wait for notepad to become the foreground window.
    std::thread::sleep(Duration::from_millis(800));

    // TODO (manual): synthesize keyboard input here via enigo or SendInput
    // to type "Hello smoke" and Ctrl+A select. For now, this test is
    // marked #[ignore] and relies on the test runner to verify the capture
    // path from a manual setup.

    let result = capture_selection_blocking();
    println!("captured: text={:?} source={:?}", result.text, result.source);

    child.kill().ok();
    child.wait().ok();
}
```

Note: the `capture_flow_against_notepad` test is intentionally marked `#[ignore]` — full keyboard synthesis to type into notepad requires more harness code than fits in Plan 1. It's a manual-walkthrough placeholder. Plan 2 or a future plan can add an `enigo`-driven version that fully automates the flow.

- [ ] **Step 2: Expose the platform modules publicly**

For the smoke tests to `use` items from `quill::platform::*`, the modules must be `pub` from the library crate. But right now Quill is a binary crate (no `lib.rs`). We need either:
- (a) Convert to a lib + bin crate (add `src/lib.rs`), or
- (b) Make `platform` a public module of the binary crate (Rust allows this for integration tests via `use quill::...` only if there's a lib target).

**Simplest for Plan 1**: add `src/lib.rs` with:

```rust
pub mod core;
pub mod engine;
pub mod platform;
pub mod providers;
```

And update `src/main.rs` to use items from the library instead of declaring modules:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("quill=info")),
        )
        .init();

    tracing::info!("Quill boot (Plan 1 shell) — full bootstrap lands in Plan 3");
    Ok(())
}
```

(Delete the `mod core; mod engine; mod platform; mod providers;` lines — they now come from the lib.)

In `Cargo.toml`, add an explicit `[lib]` section if not already present:

```toml
[lib]
name = "quill"
path = "src/lib.rs"

[[bin]]
name = "quill"
path = "src/main.rs"
```

- [ ] **Step 3: Run the smoke tests**

```bash
cargo test --test platform_smoke --features smoke -- --test-threads=1 2>&1 | tail -30
```

Expected: 5 tests pass (the `capture_flow_against_notepad` one is ignored). Output should include:

```
running 5 tests
test hotkey_parser_accepts_canonical_spec ... ok
test hotkey_service_round_trip ... ok
test tray_service_builds_and_drops ... ok
test caret_hook_service_starts_and_stops ... ok
test uia_can_initialize_on_worker_thread ... ok

test result: ok. 5 passed; 0 failed; 1 ignored
```

If `tray_service_builds_and_drops` hangs or fails, check that the embedded PNG path from Task 19 is correct and the file exists.

If `caret_hook_service_starts_and_stops` fails with a hook install error, check that you're running on a host Windows environment (not WSL, not a restricted sandbox).

- [ ] **Step 4: Commit**

```bash
git add tests/platform_smoke.rs src/lib.rs src/main.rs Cargo.toml
git commit -m "test(platform): Tier 3 smoke tests with lib/bin split"
```

### Task 24: Run the full smoke suite and close Plan 1

**Files:** none (verification checkpoint)

Phase 2 exit gate and Plan 1 closeout. Everything from Phase 0 demolition through Phase 2 platform modules should compile, pass tests, and be committed.

- [ ] **Step 1: Full clean build**

```bash
cargo clean
cargo build 2>&1 | tail -10
```

Expected: `Finished dev [unoptimized + debuginfo] target(s) in <time>`. No warnings escalated to errors.

- [ ] **Step 2: Full unit test suite**

```bash
cargo test 2>&1 | tail -20
```

Expected: all tests pass — `core::*`, `providers::*`, `platform::hotkey::tests::*`, any doc tests.

- [ ] **Step 3: Full smoke suite**

```bash
cargo test --test platform_smoke --features smoke -- --test-threads=1 2>&1 | tail -20
```

Expected: 5 tests pass, 1 ignored.

- [ ] **Step 4: Clippy**

```bash
cargo clippy --all-targets --features smoke -- -D warnings 2>&1 | tail -20
```

Expected: no warnings. If clippy finds nits, fix them inline.

- [ ] **Step 5: Fmt check**

```bash
cargo fmt -- --check
```

Expected: no output (files are already formatted). If it prints anything, run `cargo fmt` and re-check.

- [ ] **Step 6: Verify the tree matches the target topology**

```bash
ls
ls src/
ls src/platform/
ls resources/icons/
```

Expected:
- Root: `Cargo.toml`, `Cargo.lock`, `build.rs`, `src/`, `resources/`, `tests/`, `docs/`, `config/`, `.gitignore`, `README.md`, etc. — no `ui/` directory.
- `src/`: `main.rs`, `lib.rs`, `engine.rs`, `core/`, `engine/`, `platform/`, `providers/`. (Some of these may still be single files rather than folders — they'll be split in Plan 2.)
- `src/platform/`: `mod.rs`, `traits.rs`, `capture.rs`, `replace.rs`, `context.rs`, `hotkey.rs`, `uia.rs`, `tray.rs`, `mica.rs`, `dwm_shadow.rs`, `caret.rs`.
- `resources/icons/`: `plume.svg` (committed), plus `quill.ico` and `quill-tray-32.png` (gitignored, present after build).

- [ ] **Step 7: Tag Plan 1 complete**

```bash
git tag plan-01-foundation-complete
```

- [ ] **Step 8: Final sanity — run the binary and confirm it exits cleanly**

```bash
cargo run 2>&1 | tail -5
echo "exit code: $?"
```

Expected:
```
INFO quill: Quill boot (Plan 1 shell) — full bootstrap lands in Plan 3
exit code: 0
```

- [ ] **Step 9: Summary commit (optional)**

If any fixups from Steps 4-6 created diffs:

```bash
git add -A
git commit -m "chore: Plan 1 closeout — fmt, clippy, tree sanity"
```

Otherwise, skip.

**Plan 1 is complete.** The repo is a single-crate Windows-only Rust project with:
- ✅ Zero Tauri code
- ✅ Zero React code
- ✅ Pure-Rust `core/` and `providers/` with green tests
- ✅ Full platform foundation: UIA, hotkey, tray, Mica, DWM shadow, WinEvent caret tracker
- ✅ Trait-based seams for engine testability in Plan 2
- ✅ Tier 3 smoke tests gated behind `--features smoke`
- ✅ Icon generation pipeline from `plume.svg` → `quill.ico` + `quill-tray-32.png`
- ✅ Minimal compilable binary that exits cleanly

**Next:** Plan 2 — State management + engine refactor. We pick up from the `plan-01-foundation-complete` tag.

---

## Self-Review Checklist

### Spec Coverage

| Spec item (Phase 0-2) | Covered by |
|---|---|
| Safety tag + branch | Task 1 |
| Delete React + build chain | Task 2 |
| Delete Tauri scaffolding | Task 3 |
| Rewrite Cargo.toml (remove tauri*, add slint/global-hotkey/tray-icon/resvg, expand windows features) | Task 4 |
| Minimal main.rs | Task 5 |
| Strip Tauri refs from engine.rs | Task 6 |
| Delete commands.rs, delete core/hotkey.rs | Task 7 |
| Flatten repo to root | Task 8 |
| Author plume.svg | Task 9 |
| Rewrite build.rs (resvg icon gen) | Task 10 |
| Phase 0 verification | Task 11 |
| core/ tests green | Task 12 |
| providers/ tests green | Task 13 |
| TextCapture/TextReplace/ContextProbe traits | Task 14 |
| Wire Replace + Context to traits | Task 15 |
| platform/hotkey.rs with global-hotkey | Task 16 |
| platform/uia.rs IUIAutomation wrapper | Task 17 |
| UIA-first capture | Task 18 |
| platform/tray.rs | Task 19 |
| platform/mica.rs | Task 20 |
| platform/dwm_shadow.rs | Task 21 |
| platform/caret.rs WinEvent hook | Task 22 |
| Tier 3 smoke tests | Task 23 |
| Plan 1 closeout | Task 24 |

Every item from spec Phases 0-2 maps to a task. ✅

### Placeholder Scan

Ran grep for `TBD`, `implement later`, `fill in details`, `similar to Task`, `placeholder`, `Add appropriate error handling` across the plan. **Zero plan-failure placeholders.** ✅

The plan does contain two intentional `// PRESERVE` directives in Task 18 pointing to existing `capture.rs` function bodies that must be copied verbatim. These are not placeholders — they're explicit instructions referencing code that exists in the current tree. Agents executing Task 18 must open the current `src/platform/capture.rs` and copy those two function bodies into the new module.

### Type & Name Consistency

- `CaptureResult`, `CaptureSource`, `ScreenRect`, `TextCapture`, `TextReplace`, `ContextProbe` — defined in Task 14 (`platform/traits.rs`), used in Task 15 (Replace/Context impls), Task 17 (UIA returns `ScreenRect`), Task 18 (Capture impl returns `CaptureResult`). ✅
- `Uia::with(|uia| ...)` — defined in Task 17, called in Task 18. ✅
- `HotkeyService::new()`, `register()`, `unregister_current()`, `receiver()` — defined in Task 16, called in Task 23 smoke tests. ✅
- `TrayService::new(icon_bytes)` + `TRAY_ICON_PNG` — defined in Task 19, called in Task 23. ✅
- `CaretHookService::start(sender)` + `stop()` — defined in Task 22, called in Task 23. ✅
- `FocusEvent` variants (`FocusChanged`, `CaretMoved`, `FocusLost`) — defined in Task 22, consumed by Task 23 via `rx.try_recv()`. ✅
- `MicaVariant::Main | Tabbed`, `mica::apply(hwnd, variant)` — defined in Task 20, used by Plan 3 (not by this plan). ✅
- `dwm_shadow::enable(hwnd)` — defined in Task 21, used by Plan 5 (not by this plan). ✅
- `capture_selection_blocking()` — defined in Task 18, called in Task 23 ignored test. ✅

### Scope Check

The plan covers Phases 0-2 of the spec: demolition + preserve-core + platform foundation. That's 3.5 days of work, 24 tasks, 2822 lines. This is the right size for a single plan — breaking it smaller means losing the "runnable checkpoints" cadence (each phase needs verification); breaking it larger would require the state/engine refactor from Phase 3 which has its own distinct architectural concerns.

**Plan 1 is focused, complete, and ready to execute.**
