# Quill Slint Rewrite — Plan 6: Polish + Ship

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship-ready polish pass. Add the features every previous plan deferred with "— Plan 6": morph animation compact↔expanded, pencil fade animation, proper toast component with kind-based styling, file-picker for history export, and CI configuration. Also fix the stale `"Quill boot — Plan 4 expanded view"` log line.

**Architecture:**
- `rfd` crate for native file save dialogs (history export)
- Slint `animate` blocks on `width`/`height` of `MainWindow` for morph
- Slint `animate opacity` on `PencilWindow`'s inner Rectangle for fade
- `AppBridge.toast` struct with `kind` + `message` + `visible` fields; `components/toast.slint` styled based on kind
- `.github/workflows/ci.yml` — `cargo test`, `cargo clippy`, `cargo fmt --check`, `cargo build --release` on `windows-latest`

**Preconditions:**
- Branch `claude/slint-rewrite` at tag `plan-05-floating-pencil-complete`
- 64 lib tests + 7 integration tests green
- All five views (compact + 5 tabs) functional
- Floating pencil wired end-to-end

**End state:**
- Toggle ⤢ → window smoothly morphs from 380×360 to 840×600 (and back) over ~200 ms
- Pencil appears with 80 ms fade-in, disappears with 120 ms fade-out
- Toast appears with kind-specific styling: Info (purple), Success (green), Warning (amber), Error (red)
- Click "Export JSON" in History tab → native save dialog opens; file saved to chosen location
- CI config at `.github/workflows/ci.yml` runs the four standard checks on windows-latest
- Boot log says "Quill boot — Plan 6 polish"
- `cargo clippy --all-targets -- -D warnings` clean
- `cargo test --lib` + `cargo test --test engine_integration` green
- Tagged `plan-06-polish-ship-complete`

**Out of scope (genuine follow-ups post-ship):**
- Diff view in Write tab (requires `similar` crate + diff chunk layout)
- Template editor UI (save/delete callbacks exist from Plan 4; UI lands later)
- Hover tooltip on pencil
- Hide-pencil-when-main-window-focused
- Per-monitor DPI for pencil position
- `core/errors.rs` translation layer (the existing `providers/mod.rs::friendly_error` covers the HTTP-status cases; reqwest `is_timeout` / `is_connect` surfaces through the existing error message strings. Can ship without.)
- Installer / MSI — cargo build --release produces a runnable exe; distribution strategy is a business decision

---

## Design Reference

### Toast struct

```slint
// app_bridge.slint
export enum ToastKind { info, success, warning, error }

export struct Toast {
    kind: ToastKind,
    message: string,
    visible: bool,
}
```

Replace the existing `error-message: string` property with `toast: Toast` — the new component reads from the struct.

### Morph animation

Slint `animate` blocks:

```slint
// main_window.slint
width: AppBridge.view-mode == ViewMode.compact ? 380px : 840px;
height: AppBridge.view-mode == ViewMode.compact ? 360px : 600px;

animate width  { duration: 200ms; easing: ease-in-out; }
animate height { duration: 200ms; easing: ease-in-out; }
```

Slint interpolates the size property over 200 ms with ease-in-out curve. The Rust side doesn't drive the animation — Slint's property animator does. The fade-between-views part can be added later; this MVP just smooths the resize.

### Pencil fade

```slint
// pencil.slint
property <bool> is-visible: false;
opacity: is-visible ? 1.0 : 0.0;
animate opacity {
    duration: is-visible ? 80ms : 120ms;
    easing: ease-out;
}
```

Rust sets `is-visible` via a new setter. The pencil window itself is always shown by winit; the Slint root fades its contents. This avoids show/hide flicker.

### File picker

`rfd::FileDialog::new().set_file_name("history-export.json").add_filter("JSON", &["json"]).save_file()` → `Option<PathBuf>`.

Call this on the Slint main thread (rfd is synchronous and on Windows uses the native `IFileSaveDialog` which requires a valid parent HWND — passing `None` shows a parentless dialog, which is fine for Plan 6).

Route: `UiCommand::ExportHistory { format, path }` — Plan 4's variant gains a `path: PathBuf` field. The bridge's `on_export_history` callback runs on the Slint thread, opens the picker, then sends the command with the chosen path. The engine writes to that path instead of the Plan 4 fixed location.

---

## Phase 1 — Fixups + Foundation (Day 1 AM)

### Task 1: Cosmetic cleanup — boot log + dependency refresh

**Files:**
- Modify: `src/main.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Fix the boot log**

In `src/main.rs`, change the `tracing::info!("Quill boot — Plan 4 expanded view");` line to:

```rust
    tracing::info!("Quill boot — Plan 6 polish");
```

- [ ] **Step 2: Add the `rfd` crate**

In `Cargo.toml` under `[dependencies]`, add:

```toml
rfd = { version = "0.14", default-features = false, features = ["xdg-portal", "tokio"] }
```

NOTE: the feature list depends on the `rfd` version. For Windows-only builds, `default-features = false` with NO features enabled uses the native Win32 dialog backend. Verify with `cargo tree -p rfd` after the first build. If the native path isn't the default, use:

```toml
rfd = "0.14"
```

and let it pull its default Windows backend.

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: clean. `rfd` pulls a few transitive deps (windows-rs already pinned; rfd uses its own subset).

- [ ] **Step 4: Commit**

```bash
git add src/main.rs Cargo.toml Cargo.lock
git commit -m "chore: update boot log, add rfd for native file picker"
```

### Task 2: Extend `UiCommand::ExportHistory` with a `path` field

**Files:**
- Modify: `src/state/events.rs`
- Modify: `src/engine/mod.rs`

- [ ] **Step 1: Extend the variant**

In `src/state/events.rs`, change:

```rust
    ExportHistory  { format: String },
```

to:

```rust
    ExportHistory  { format: String, path: std::path::PathBuf },
```

- [ ] **Step 2: Update the engine handler**

In `src/engine/mod.rs::handle_command`, find the `ExportHistory { format: fmt } => { ... }` arm. Replace the fixed-path construction with the passed-in path:

```rust
        ExportHistory { format: fmt, path } => {
            let result = match fmt.as_str() {
                "json" => export_history_json(&path),
                "csv"  => export_history_csv(&path),
                "md"   => export_history_md(&path),
                other  => Err(anyhow::anyhow!("unknown export format: {other}")),
            };
            let msg = match result {
                Ok(()) => format!("Exported to {}", path.display()),
                Err(e) => format!("Export failed: {e}"),
            };
            self.emit(crate::state::UiEvent::Toast {
                kind: crate::state::app_state::ToastKind::Info,
                message: msg,
            });
        }
```

Remove the `dirs::home_dir()` lookup and the `~/.quill/history-export.*` fallback — callers must always pass a path.

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: clean — the bridge doesn't call `ExportHistory` yet (Phase 3 wires it), so no call sites need updating.

- [ ] **Step 4: Commit**

```bash
git add src/state/events.rs src/engine/mod.rs
git commit -m "feat(state): ExportHistory takes a PathBuf destination"
```

---

## Phase 2 — Slint Polish: Toast, Morph, Pencil Fade (Day 1 PM / Day 2 AM)

### Task 3: Toast struct + kind-styled Toast component

**Files:**
- Modify: `src/ui/slint/app_bridge.slint`
- Modify: `src/ui/slint/components/toast.slint`
- Modify: `src/ui/bridge.rs`

- [ ] **Step 1: Extend `app_bridge.slint`**

Add these to `app_bridge.slint` BEFORE the `export global AppBridge` block:

```slint
export enum ToastKind { info, success, warning, error }

export struct Toast {
    kind: ToastKind,
    message: string,
    visible: bool,
}
```

Replace the existing `in-out property <string> error-message;` inside `AppBridge` with:

```slint
    in-out property <Toast> toast: { kind: ToastKind.info, message: "", visible: false };
```

KEEP `error-message` for now if removing it is disruptive — the bridge can set both `error-message` AND `toast` during the transition. If you prefer a cleaner break, remove `error-message` entirely and update every reader (`toast.slint`, `compact.slint`, `expanded.slint` — all use `AppBridge.error-message`).

Recommended: remove `error-message` cleanly. Phase 2 updates all three readers in this task.

- [ ] **Step 2: Rewrite `components/toast.slint`**

Replace the file's contents with:

```slint
import { AppBridge, ToastKind } from "../app_bridge.slint";

export component Toast inherits Rectangle {
    visible: AppBridge.toast.visible && AppBridge.toast.message != "";
    height: 30px;
    border-radius: 6px;
    background: AppBridge.toast.kind == ToastKind.success ? #2a6e3a
              : AppBridge.toast.kind == ToastKind.warning ? #8a6a20
              : AppBridge.toast.kind == ToastKind.error   ? #803030
              : /* info */                                  #4a3a70;
    animate background { duration: 160ms; }
    HorizontalLayout {
        padding-left: 12px;
        padding-right: 12px;
        alignment: start;
        spacing: 8px;
        Text {
            text: AppBridge.toast.kind == ToastKind.success ? "✓"
                : AppBridge.toast.kind == ToastKind.warning ? "⚠"
                : AppBridge.toast.kind == ToastKind.error   ? "✕"
                : /* info */                                   "ℹ";
            color: white;
            font-size: 12px;
            vertical-alignment: center;
        }
        Text {
            text: AppBridge.toast.message;
            color: white;
            font-size: 11px;
            vertical-alignment: center;
        }
    }
}
```

- [ ] **Step 3: Update `compact.slint` and `expanded.slint`**

Search each file for `AppBridge.error-message` — if present, remove the entire conditional element or replace with a `Toast {}` instance. (The compact view Toast is already a `Toast {}` import; no change likely needed.) Confirm with:

```bash
grep -rn "error-message" src/ui/slint/
```

If any reference remains, replace it with a `Toast {}` component instance, or delete the block entirely.

- [ ] **Step 4: Update `src/ui/bridge.rs`**

In `apply_event_on_ui_thread`, find the two arms that set `error-message`:

```rust
        UiEvent::Error { message } => {
            bridge.set_error_message(message.into());
        }
        UiEvent::Toast { kind: _, message } => {
            bridge.set_error_message(message.into());
        }
```

Replace with:

```rust
        UiEvent::Error { message } => {
            bridge.set_toast(crate::ui::Toast {
                kind: crate::ui::ToastKind::Error,
                message: message.into(),
                visible: true,
            });
        }
        UiEvent::Toast { kind, message } => {
            let kind = match kind {
                crate::state::app_state::ToastKind::Info    => crate::ui::ToastKind::Info,
                crate::state::app_state::ToastKind::Success => crate::ui::ToastKind::Success,
                crate::state::app_state::ToastKind::Warning => crate::ui::ToastKind::Warning,
                crate::state::app_state::ToastKind::Error   => crate::ui::ToastKind::Error,
            };
            bridge.set_toast(crate::ui::Toast {
                kind,
                message: message.into(),
                visible: true,
            });
        }
```

- [ ] **Step 5: Build**

Run: `cargo build`
Expected: clean. Snags:
- The generated `crate::ui::Toast` struct may differ slightly (field order or `visible` type). If fields are `SharedString` vs `String`, convert with `.into()`.
- `crate::ui::ToastKind::Info` etc. — verify PascalCase variants (generated from `ToastKind { info, success, warning, error }` in slint).

- [ ] **Step 6: Commit**

```bash
git add src/ui/slint/app_bridge.slint src/ui/slint/components/toast.slint src/ui/slint/compact.slint src/ui/slint/expanded.slint src/ui/bridge.rs
git commit -m "feat(ui): Toast struct with kind-based styling"
```

### Task 4: Morph animation on `MainWindow` + pencil fade

**Files:**
- Modify: `src/ui/slint/main_window.slint`
- Modify: `src/ui/slint/pencil.slint`

- [ ] **Step 1: Add morph to `main_window.slint`**

Find the `MainWindow` `width:` / `height:` declarations. After them, add `animate` blocks:

```slint
    width: AppBridge.view-mode == ViewMode.compact ? 380px : 840px;
    height: AppBridge.view-mode == ViewMode.compact ? 360px : 600px;
    animate width  { duration: 200ms; easing: ease-in-out; }
    animate height { duration: 200ms; easing: ease-in-out; }
```

- [ ] **Step 2: Add fade to `pencil.slint`**

Replace the current `PencilWindow` contents with:

```slint
import { Plume } from "./components/plume.slint";

export component PencilWindow inherits Window {
    title: "Quill Pencil";
    background: transparent;
    no-frame: true;
    always-on-top: true;
    width: 32px;
    height: 32px;

    in-out property <bool> is-visible: false;

    callback clicked();

    Rectangle {
        width: 100%;
        height: 100%;
        opacity: root.is-visible ? 1.0 : 0.0;
        animate opacity {
            duration: root.is-visible ? 80ms : 120ms;
            easing: ease-out;
        }
        background: touch.has-hover ? rgba(192, 132, 252, 0.2) : transparent;
        border-radius: 8px;
        animate background { duration: 120ms; }
        Plume { width: 24px; height: 24px; }
        touch := TouchArea {
            clicked => { root.clicked(); }
        }
    }
}
```

- [ ] **Step 3: Update the pencil controller to drive `is-visible` instead of show/hide**

In `src/ui/pencil_controller.rs`, inside `apply_cmd`:

```rust
fn apply_cmd(window: &PencilWindow, cmd: PencilCmd) {
    match cmd {
        PencilCmd::ShowAt { x, y } => {
            pencil_window::set_position(window, x, y);
            // Ensure the native window is visible so opacity fade is seen.
            let _ = window.show();
            window.set_is_visible(true);
        }
        PencilCmd::Hide => {
            window.set_is_visible(false);
            // Keep the native window visible so the fade-out animation runs;
            // a slint::Timer queued below fires hide() after the fade completes.
            // For Plan 6 we keep it simple: leave the native window alive and
            // rely on the 0.0 opacity to make it visually absent.
        }
    }
}
```

RATIONALE: calling `window.hide()` immediately after `set_is_visible(false)` kills the animation because the native window disappears before opacity can interpolate. Plan 6 keeps the native window visible and relies on `opacity: 0.0` for hiding. The visible-but-transparent window still participates in the click-through logic from Plan 5 — no regression.

- [ ] **Step 4: Update `pencil_window::build()` to set initial `is-visible = false`**

In `src/ui/pencil_window.rs::build`, after constructing `PencilWindow::new()`, call:

```rust
    window.set_is_visible(false);
```

Keep the existing Plan 5 `pencil_window::build()` lazy HWND initialization logic intact.

- [ ] **Step 5: Build**

Run: `cargo build`
Expected: clean. The Slint compiler may complain about accessing `root.is-visible` — if so, change to `is-visible` (unqualified) since we're inside the component itself.

- [ ] **Step 6: Commit**

```bash
git add src/ui/slint/main_window.slint src/ui/slint/pencil.slint src/ui/pencil_controller.rs src/ui/pencil_window.rs
git commit -m "feat(ui): morph animation + pencil fade"
```

---

## Phase 3 — File Picker + CI + Ship (Day 2 PM / Day 3)

### Task 5: Wire file picker into `on_export_history`

**Files:**
- Modify: `src/ui/bridge.rs`

- [ ] **Step 1: Replace the `on_export_history` handler**

Find the Plan 4 `on_export_history` registration in `install_command_forwarder`:

```rust
    let tx_eh = tx.clone();
    bridge.on_export_history(move |format| {
        let _ = tx_eh.send(UiCommand::ExportHistory {
            format: format.to_string(),
        });
    });
```

Replace with:

```rust
    let tx_eh = tx.clone();
    bridge.on_export_history(move |format| {
        let format = format.to_string();
        let (default_ext, filter_name, filter_exts): (&str, &str, &[&str]) = match format.as_str() {
            "json" => ("json", "JSON",     &["json"]),
            "csv"  => ("csv",  "CSV",      &["csv"]),
            "md"   => ("md",   "Markdown", &["md"]),
            _      => ("txt",  "All",      &["*"]),
        };
        let default_name = format!("quill-history.{default_ext}");
        let picked = rfd::FileDialog::new()
            .set_file_name(&default_name)
            .add_filter(filter_name, filter_exts)
            .save_file();
        if let Some(path) = picked {
            let _ = tx_eh.send(UiCommand::ExportHistory { format, path });
        }
        // Cancelled → no command sent; no toast. Matches native file-picker UX.
    });
```

- [ ] **Step 2: Build**

Run: `cargo build`
Expected: clean. Snags:
- `rfd::FileDialog` may be behind a `default` feature — if Task 1 used `default-features = false`, re-enable the default feature for Windows: `rfd = { version = "0.14" }`. Adjust and rebuild.
- Calling `save_file()` blocks the Slint main thread until the user picks or cancels — that's acceptable for Plan 6; the alternative (`save_file_async`) requires a tokio Handle and adds complexity.

- [ ] **Step 3: Commit**

```bash
git add src/ui/bridge.rs Cargo.toml Cargo.lock
git commit -m "feat(ui): native file picker for history export"
```

### Task 6: CI configuration — `.github/workflows/ci.yml`

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Create the directory**

```bash
mkdir -p .github/workflows
```

- [ ] **Step 2: Write the CI file**

```yaml
name: CI

on:
  push:
    branches: [main, claude/slint-rewrite]
  pull_request:
    branches: [main]

jobs:
  build-and-test:
    runs-on: windows-latest

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust stable
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Cache cargo registry + target
        uses: Swatinem/rust-cache@v2

      - name: cargo fmt --check
        run: cargo fmt --check

      - name: cargo clippy
        run: cargo clippy --all-targets -- -D warnings

      - name: cargo test --lib
        run: cargo test --lib --verbose

      - name: cargo test engine_integration
        run: cargo test --test engine_integration --verbose

      - name: cargo build --release
        run: cargo build --release
```

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: run fmt/clippy/tests/release-build on windows-latest"
```

### Task 7: Final sweep + tag `plan-06-polish-ship-complete`

- [ ] **Step 1: Full sweep**

```bash
cargo fmt
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --lib
cargo test --test engine_integration
cargo build --release
```

Fix any new clippy lints inline. Commit fmt as `style: cargo fmt after plan 6` if needed.

- [ ] **Step 2: Sanity grep**

```bash
grep -rn "todo!\|unimplemented!" src/ .github/
grep -rn "noop_emit\|SharedEngine\|tauri::" src/ tests/
grep -rn "error-message" src/ui/slint/
```

The third grep catches stale references to the old `error-message` property. Should be empty after Task 3's cleanup.

- [ ] **Step 3: Release artifact check**

```bash
ls -la target/release/quill.exe
```

Confirm the binary exists and is reasonable in size (Slint + tokio + reqwest → expect 15-30 MB).

- [ ] **Step 4: Tag**

```bash
git tag plan-06-polish-ship-complete
git tag -l plan-06*
```

- [ ] **Step 5: Engram save**

Call `mcp__plugin_engram_engram__mem_save`:
- project: `Quill`
- scope: `project`
- topic_key: `quill/slint-rewrite/plan-06`
- type: `architecture`
- title: `Plan 6 (Polish + ship) complete — Quill Slint rewrite SHIPPED`
- content: Summarize:
  - Toast struct with kind-based styling (Info/Success/Warning/Error); old `error-message` property removed
  - Morph animation (200 ms ease-in-out) on `MainWindow` width/height; pencil fade (80/120 ms) via opacity animation
  - Native `rfd` file picker for history export; `UiCommand::ExportHistory { format, path }`
  - CI config at `.github/workflows/ci.yml` — fmt/clippy/test/release on windows-latest
  - Boot log corrected
  - All six plans of the Quill Slint rewrite complete. Total commits since `tauri-final`: [count via git]. Total plans: 6. Total tests: 64 lib + 7 integration = 71
  - Genuine follow-ups (not-yet-planned): diff view in Write tab, template editor UI, hover tooltip on pencil, hide-pencil-when-main-window-focused, per-monitor DPI, `core/errors.rs` translation layer (existing `providers/friendly_error` sufficient for ship)

- [ ] **Step 6: Print the manual verification checklist** (to the user, not committed)

```
1. cargo run — compact window opens
2. ⤢ → window smoothly animates to 840×600 over ~200 ms
3. — / ⤢ toggle → smooth morph back to compact
4. History tab → Export JSON → native save dialog → pick location → success toast
5. History tab → Export CSV / MD → same
6. Successful export → toast is green (Success kind)
7. Failing export (readonly path) → toast is red (Error kind)
8. Focus Notepad → pencil fades in (80ms) next to caret
9. Alt-Tab away → pencil fades out (120ms)
10. Clicking pencil → capture flow → main window opens
11. Cancel file picker → no toast, no error
12. Manually verify target/release/quill.exe runs standalone
```

---

## Self-Review Notes

**Deliberately skipped for ship:**
- Diff view in Write tab — would need the `similar` crate + diff chunk layout; visual polish rather than functional
- Template editor UI — Plan 4's `save-template`/`delete-template` callbacks are wired in the bridge; the UI is additive
- Hover tooltip on pencil — cosmetic, not blocking
- Hide-pencil-when-main-window-focused — cosmetic
- Per-monitor DPI handling for pencil — works on single-monitor and multi-monitor-same-DPI; mixed DPI is an edge case
- `core/errors.rs` translation layer — `providers/mod.rs::friendly_error` already covers HTTP status translation for the 90% case; reqwest connect/timeout errors flow through as string messages which the toast now renders with error-kind styling

**Likely snags for the implementer:**
1. `rfd` feature gates vary by version — if the plan's `features = ["xdg-portal", "tokio"]` breaks the build, drop to `rfd = "0.14"` without custom features (Windows backend is the default)
2. Slint's `animate width/height` blocks must be declared INSIDE the component that owns the property — verify the placement inside `MainWindow`
3. `window.set_is_visible(bool)` on the pencil — the setter name follows Slint's `set_<property-kebab-to-snake>` convention; verify with the generated code if compile fails
4. Removing `error-message` may break `compact.slint`'s `Toast {}` which reads from `AppBridge.toast` not `error-message` — confirm the import is `Toast` (not `toast.slint`'s internal) and the references are consistent
5. The `save_file()` blocking call on the Slint thread is fine for Plan 6; if you see UI freezing complaints, that's a Plan 7 concern (offload to tokio with a channel callback)

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-04-14-quill-slint-plan-06-polish-ship.md`.**

Execute subagent-driven, one implementer per phase.
