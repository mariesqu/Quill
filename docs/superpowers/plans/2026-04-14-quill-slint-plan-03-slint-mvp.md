# Quill Slint Rewrite — Plan 3: Slint MainWindow MVP

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring up a native Slint window wired end-to-end to the Plan 2 Engine. Proves the pipeline: real hotkey → engine captures text → UiEvent::ShowOverlay → Slint window appears with Mica backdrop → user clicks a mode → UiCommand::ExecuteMode reaches the engine → stream chunks flow back to the UI.

**Scope:** Compact view only. Expanded view is a placeholder. Full tab UIs (History, Tutor, Compare, Settings) land in Plan 4.

**Architecture:**
- `build.rs` compiles `.slint` files via `slint-build`
- `src/ui/slint/` holds `.slint` source (root `main_window.slint` + global `app_bridge.slint` + `compact.slint` + `components/{plume,lang_row,mode_row,chain_row,toast}.slint`)
- `src/ui/bridge.rs` runs on the Slint event-loop thread: pumps `UiEvent` into `AppBridge` properties via `slint::invoke_from_event_loop`; registers Slint callbacks that forward `UiCommand`s into the engine channel
- `src/ui/main_window.rs` constructs the Slint `MainWindow`, grabs the HWND via `slint::winit::WinitWindowAccessor`, applies Mica + DWM shadow + dark mode + round corners
- `src/main.rs` becomes the full boot: tokio runtime, config, state, UiEvent + UiCommand channels, Engine, global hotkey thread, tray, window, Slint event loop

**Tech Stack:** Slint 1.8 (winit + skia), `slint-build`, `windows-rs 0.58` DWM APIs, `global-hotkey` 0.5, `tray-icon` 0.14, tokio multi-thread runtime.

**Preconditions:**
- Branch `claude/slint-rewrite` at tag `plan-02-engine-refactor-complete`
- 63 lib tests + 7 integration tests green
- `slint`, `slint-build`, `global-hotkey`, `tray-icon` already in `Cargo.toml`
- `resources/icons/plume.svg` + `quill.ico` + `quill-tray-32.png` exist
- `src/platform/{mica,dwm_shadow,hotkey,tray,caret}.rs` built in Plan 1 but still inert
- `src/engine/` + `src/state/` built in Plan 2
- `src/main.rs` has `#![allow(dead_code, unused_imports)]` shield

**End state:**
- `cargo run` launches a compact Slint window with Mica backdrop (or solid fallback on pre-22H2)
- Global hotkey (`Ctrl+Shift+Space` default) triggers the full capture flow and shows the window near the selection
- Clicking a mode button in the window streams tokens back into the `stream-buffer` property
- ESC dismisses; tray icon works; Quit exits cleanly
- `cargo clippy --all-targets -- -D warnings` clean
- Tagged `plan-03-slint-mvp-complete`

**Out of scope (explicit):**
- Expanded view content (tabs, history list, compare, tutor lessons, settings) — Plan 4
- Floating pencil window — Plan 5
- Morph animation between compact and expanded — Plan 4
- Visual polish, motion design, full color palette — Plan 6
- Automated UI tests beyond cargo test --lib passing — Plan 4 when the testable surface grows

---

## Design Reference

### Slint Global: `AppBridge` (subset for Plan 3)

```slint
// ui/slint/app_bridge.slint
export enum ViewMode { compact, expanded }

export struct ModeInfo {
    id: string,
    label: string,
    icon: string,
}

export struct ChainInfo {
    id: string,
    label: string,
    icon: string,
    description: string,
}

export struct LanguageOpt {
    code: string,
    label: string,
}

export global AppBridge {
    // Session
    in-out property <string> selected-text;
    in-out property <string> stream-buffer;
    in-out property <bool>   is-streaming;
    in-out property <bool>   is-done;
    in-out property <string> last-result;
    in-out property <string> active-mode;
    in-out property <string> active-language: "auto";

    // View
    in-out property <ViewMode> view-mode: ViewMode.compact;

    // Collections
    in-out property <[ModeInfo]>    modes;
    in-out property <[ChainInfo]>   chains;
    in-out property <[LanguageOpt]> languages: [
        { code: "auto", label: "Auto" },
        { code: "en",   label: "EN"   },
        { code: "fr",   label: "FR"   },
        { code: "es",   label: "ES"   },
        { code: "de",   label: "DE"   },
        { code: "ja",   label: "JA"   },
        { code: "pt",   label: "PT"   },
        { code: "zh",   label: "ZH"   },
    ];

    // UX
    in-out property <string> error-message;

    // Callbacks (Plan 3 subset)
    callback execute-mode(string /*mode*/, string /*language*/);
    callback execute-chain(string /*chain_id*/, string /*language*/);
    callback set-language(string /*code*/);
    callback confirm-replace();
    callback cancel-stream();
    callback toggle-view();
    callback dismiss();
}
```

Plan 4 extends this with `current-tab`, history/template collections, `compare-modes`, `save-config`, etc.

### Threading map

| Thread | Runs | Owns |
|---|---|---|
| **Main** | Slint event loop | `MainWindow` `ComponentHandle`, tray icon, bridge event pump |
| **Tokio worker pool** | `Engine::handle_command`, streams, history writes | Engine inner |
| **Hotkey listener** | `global_hotkey::GlobalHotKeyEvent::receiver().recv()` loop | hotkey manager |
| **Tray event listener** | `tray_icon::menu::MenuEvent::receiver().recv()` loop | menu channel |

Cross-thread comms:
- `UiEvent`:   engine tasks → `mpsc::UnboundedSender<UiEvent>` → bridge event pump → `AppBridge` properties via `slint::invoke_from_event_loop`
- `UiCommand`: `AppBridge` callbacks → `mpsc::UnboundedSender<UiCommand>` → engine task pool
- `Hotkey → Engine`: hotkey listener thread posts a `UiCommand::Internal` stand-in? NO — it spawns a tokio task calling `engine::hotkey_flow::handle_hotkey(engine.clone())` directly. No channel needed because we have an `Engine` handle on the hotkey thread (Engine is `Clone`).
- `Tray → Main thread`: tray event listener thread forwards events via `slint::invoke_from_event_loop` so the event handler runs on the Slint thread.

### Window construction order (critical)

1. Create tokio runtime (`tokio::runtime::Builder::new_multi_thread().enable_all().build()`) and keep a `Runtime` handle
2. Call `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` — NON-NEGOTIABLE, must come before any window creation
3. Load config, build state + channels, build real Engine
4. Build `MainWindow` via generated `MainWindow::new()?`
5. Grab `HWND` via `window_handle_for_main(&main_window)` (see Task 10)
6. Apply Mica + DWM shadow + round corners + dark mode + extend frame
7. Register global hotkey; spawn hotkey listener thread; each hit calls `rt.spawn(engine::hotkey_flow::handle_hotkey(engine.clone()))`
8. Build tray; spawn tray listener
9. Register Slint callbacks (Command forwarder)
10. Spawn Bridge event pump on tokio — it calls `slint::invoke_from_event_loop` per `UiEvent`
11. `main_window.run()?` — blocks until window closes

---

## Phase 1 — Slint Build Integration (Day 1 AM)

### Task 1: Wire `slint-build` into `build.rs` + create stub `main_window.slint`

**Files:**
- Modify: `build.rs`
- Create: `src/ui/slint/main_window.slint`

- [ ] **Step 1: Create the directory and stub slint file**

```bash
mkdir -p src/ui/slint
```

Create `src/ui/slint/main_window.slint`:

```slint
// Plan 3 stub — replaced with full AppBridge wiring in Task 5.
export component MainWindow inherits Window {
    title: "Quill";
    width: 380px;
    height: 260px;
    background: #1e1e28;
    Text {
        text: "Quill — Plan 3 stub";
        color: white;
        horizontal-alignment: center;
        vertical-alignment: center;
    }
}
```

- [ ] **Step 2: Extend `build.rs`**

At the top of `fn main()`, BEFORE the existing plume.svg/ICO generation block, add:

```rust
// Compile Slint sources. MainWindow is the root component exported from
// main_window.slint; slint-build generates src/ui/slint/main_window.rs
// which slint::include_modules!() picks up at build time.
let slint_config = slint_build::CompilerConfiguration::new()
    .with_style("fluent-dark".into());
slint_build::compile_with_config("src/ui/slint/main_window.slint", slint_config)
    .expect("slint compilation failed");
println!("cargo:rerun-if-changed=src/ui/slint");
```

Add the `slint_build` use at the top: the `slint-build` crate is already in `[build-dependencies]` as verified in `Cargo.toml`. No `use` needed — use the fully qualified path above.

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: clean build. The slint compiler should run silently and emit a generated Rust source into `OUT_DIR`. If you see an error like "component MainWindow not marked for export", add `export component` (not just `component`) — already correct above.

- [ ] **Step 4: Commit**

```bash
git add build.rs src/ui/slint/main_window.slint
git commit -m "build: compile slint sources from src/ui/slint via slint-build"
```

### Task 2: Create `src/ui/mod.rs` with `slint::include_modules!()` and verify

**Files:**
- Create: `src/ui/mod.rs`
- Modify: `src/lib.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create `src/ui/mod.rs`**

```rust
//! Slint UI layer: bridge between `AppState`/`UiEvent`/`UiCommand` and the
//! generated Slint `MainWindow` component.
//!
//! The `slint::include_modules!()` macro pulls in the Rust code that
//! `slint-build` emits from `src/ui/slint/*.slint`. Do NOT hand-edit the
//! generated output — change the `.slint` sources instead.

slint::include_modules!();
```

- [ ] **Step 2: Add `pub mod ui;` to `src/lib.rs`**

```rust
pub mod core;
pub mod engine;
pub mod platform;
pub mod providers;
pub mod state;
pub mod ui;
```

- [ ] **Step 3: Add `mod ui;` to `src/main.rs`** (keep it under the other `mod` declarations)

```rust
mod core;
mod engine;
mod platform;
mod providers;
mod state;
mod ui;
```

- [ ] **Step 4: Smoke the generated code is accessible**

Add a one-line `tracing::debug!("Slint types available: MainWindow");` at the top of `main()` after the tracing init, and temporarily add:

```rust
let _ = ui::MainWindow::new();  // proves MainWindow is in scope
```

Run: `cargo build`
Expected: clean. If the `MainWindow` type isn't visible, double-check `export component` in the slint file.

Remove the smoke line before committing.

- [ ] **Step 5: Commit**

```bash
git add src/ui/mod.rs src/lib.rs src/main.rs
git commit -m "feat(ui): scaffold ui module with slint::include_modules!"
```

---

## Phase 2 — Slint Components (Day 1 PM)

This phase creates the `.slint` source files. All file paths below are under `src/ui/slint/`. None of these tasks touch Rust code — Slint compiles to Rust via `build.rs` from Phase 1.

**Convention**: identifiers in Slint are `kebab-case` for properties/callbacks, `PascalCase` for components/structs/enums.

### Task 3: `app_bridge.slint` — the Global Singleton

**Files:**
- Create: `src/ui/slint/app_bridge.slint`

- [ ] **Step 1: Write the file**

```slint
// Shared types and the AppBridge global. This file is IMPORTED by
// main_window.slint and by every component that needs to read properties
// or fire callbacks. AppBridge is the ONLY interface between Rust and Slint.

export enum ViewMode { compact, expanded }

export struct ModeInfo {
    id: string,
    label: string,
    icon: string,
}

export struct ChainInfo {
    id: string,
    label: string,
    icon: string,
    description: string,
}

export struct LanguageOpt {
    code: string,
    label: string,
}

export global AppBridge {
    // Session
    in-out property <string> selected-text;
    in-out property <string> stream-buffer;
    in-out property <bool>   is-streaming;
    in-out property <bool>   is-done;
    in-out property <string> last-result;
    in-out property <string> active-mode;
    in-out property <string> active-language: "auto";

    // View
    in-out property <ViewMode> view-mode: ViewMode.compact;

    // Collections — seeded from Rust on startup
    in-out property <[ModeInfo]>    modes;
    in-out property <[ChainInfo]>   chains;
    in-out property <[LanguageOpt]> languages: [
        { code: "auto", label: "Auto" },
        { code: "en",   label: "EN"   },
        { code: "fr",   label: "FR"   },
        { code: "es",   label: "ES"   },
        { code: "de",   label: "DE"   },
        { code: "ja",   label: "JA"   },
        { code: "pt",   label: "PT"   },
        { code: "zh",   label: "ZH"   },
    ];

    // UX
    in-out property <string> error-message;

    // Callbacks — Rust registers handlers in ui/bridge.rs.
    callback execute-mode(string /*mode*/, string /*language*/);
    callback execute-chain(string /*chain_id*/, string /*language*/);
    callback set-language(string /*code*/);
    callback confirm-replace();
    callback cancel-stream();
    callback toggle-view();
    callback dismiss();
}
```

- [ ] **Step 2: Commit** (no build yet — not imported anywhere)

```bash
git add src/ui/slint/app_bridge.slint
git commit -m "feat(ui/slint): AppBridge global singleton with compact-view surface"
```

### Task 4: Component slint files (plume, lang_row, mode_row, chain_row, toast)

**Files:**
- Create: `src/ui/slint/components/plume.slint`
- Create: `src/ui/slint/components/lang_row.slint`
- Create: `src/ui/slint/components/mode_row.slint`
- Create: `src/ui/slint/components/chain_row.slint`
- Create: `src/ui/slint/components/toast.slint`

- [ ] **Step 1: `components/plume.slint`**

```slint
// Brand glyph. Embeds the SVG so the exe is self-contained.
export component Plume inherits Image {
    source: @image-url("../../../../resources/icons/plume.svg");
    width: 20px;
    height: 20px;
    colorize: #c084fc;
}
```

NOTE: the `@image-url` path is relative to the `.slint` FILE, not the manifest. From `src/ui/slint/components/plume.slint` the path `../../../../resources/icons/plume.svg` walks up to `resources/`. Verify with a build after Task 5 wires it in.

- [ ] **Step 2: `components/lang_row.slint`**

```slint
import { AppBridge, LanguageOpt } from "../app_bridge.slint";

export component LangRow inherits HorizontalLayout {
    spacing: 4px;
    alignment: start;
    for lang in AppBridge.languages: Rectangle {
        width: 34px;
        height: 22px;
        border-radius: 11px;
        background: AppBridge.active-language == lang.code ? #c084fc : #3a3450;
        Text {
            text: lang.label;
            color: AppBridge.active-language == lang.code ? #1a1530 : #e0dbe8;
            horizontal-alignment: center;
            vertical-alignment: center;
            font-size: 11px;
            font-weight: AppBridge.active-language == lang.code ? 600 : 400;
        }
        TouchArea {
            clicked => { AppBridge.set-language(lang.code); }
        }
    }
}
```

- [ ] **Step 3: `components/mode_row.slint`**

```slint
import { AppBridge, ModeInfo } from "../app_bridge.slint";

export component ModeRow inherits HorizontalLayout {
    spacing: 6px;
    alignment: start;
    for mode in AppBridge.modes: Rectangle {
        width: 40px;
        height: 40px;
        border-radius: 10px;
        background: touch.pressed
            ? #c084fc
            : (touch.has-hover ? #4a4060 : #2a2540);
        animate background { duration: 120ms; }
        VerticalLayout {
            padding: 4px;
            spacing: 2px;
            alignment: center;
            Text {
                text: mode.icon;
                horizontal-alignment: center;
                font-size: 16px;
            }
            Text {
                text: mode.label;
                horizontal-alignment: center;
                color: #e0dbe8;
                font-size: 8px;
                overflow: elide;
            }
        }
        touch := TouchArea {
            clicked => {
                AppBridge.active-mode = mode.id;
                AppBridge.execute-mode(mode.id, AppBridge.active-language);
            }
        }
    }
}
```

- [ ] **Step 4: `components/chain_row.slint`**

```slint
import { AppBridge, ChainInfo } from "../app_bridge.slint";

export component ChainRow inherits HorizontalLayout {
    spacing: 6px;
    alignment: start;
    for chain in AppBridge.chains: Rectangle {
        height: 26px;
        min-width: 60px;
        border-radius: 13px;
        background: touch.has-hover ? #4a4060 : #2a2540;
        animate background { duration: 120ms; }
        HorizontalLayout {
            padding-left: 8px;
            padding-right: 10px;
            spacing: 4px;
            alignment: center;
            Text { text: chain.icon; color: #c084fc; font-size: 12px; }
            Text { text: chain.label; color: #e0dbe8; font-size: 11px; }
        }
        touch := TouchArea {
            clicked => { AppBridge.execute-chain(chain.id, AppBridge.active-language); }
        }
    }
}
```

- [ ] **Step 5: `components/toast.slint`**

```slint
import { AppBridge } from "../app_bridge.slint";

export component Toast inherits Rectangle {
    visible: AppBridge.error-message != "";
    background: #803030;
    border-radius: 6px;
    height: 28px;
    HorizontalLayout {
        padding-left: 10px;
        padding-right: 10px;
        alignment: start;
        Text {
            text: AppBridge.error-message;
            color: white;
            font-size: 11px;
            vertical-alignment: center;
        }
    }
}
```

- [ ] **Step 6: Commit** (still no compile — nothing imports these yet; the `build.rs` compiler only touches files reachable from `main_window.slint`)

```bash
git add src/ui/slint/components/
git commit -m "feat(ui/slint): components — plume, lang_row, mode_row, chain_row, toast"
```

---

## Phase 3 — Compact View + MainWindow Root (Day 2 AM)

### Task 5: `compact.slint` — compact layout

**Files:**
- Create: `src/ui/slint/compact.slint`

- [ ] **Step 1: Write the file**

```slint
import { AppBridge } from "./app_bridge.slint";
import { Plume }    from "./components/plume.slint";
import { LangRow }  from "./components/lang_row.slint";
import { ModeRow }  from "./components/mode_row.slint";
import { ChainRow } from "./components/chain_row.slint";
import { Toast }    from "./components/toast.slint";

export component CompactView inherits Rectangle {
    background: rgba(38, 36, 48, 0.65);  // Mica shows through
    border-radius: 12px;

    VerticalLayout {
        padding: 14px;
        spacing: 10px;

        // Header row
        HorizontalLayout {
            height: 24px;
            spacing: 8px;
            alignment: space-between;
            HorizontalLayout {
                spacing: 6px;
                Plume {}
                Text {
                    text: "Quill";
                    color: #e0dbe8;
                    font-size: 13px;
                    font-weight: 600;
                    vertical-alignment: center;
                }
            }
            HorizontalLayout {
                spacing: 4px;
                Rectangle {
                    width: 22px;
                    height: 22px;
                    border-radius: 4px;
                    background: expand-touch.has-hover ? #4a4060 : transparent;
                    Text {
                        text: "⤢";
                        color: #e0dbe8;
                        horizontal-alignment: center;
                        vertical-alignment: center;
                    }
                    expand-touch := TouchArea {
                        clicked => { AppBridge.toggle-view(); }
                    }
                }
                Rectangle {
                    width: 22px;
                    height: 22px;
                    border-radius: 4px;
                    background: close-touch.has-hover ? #803030 : transparent;
                    Text {
                        text: "✕";
                        color: #e0dbe8;
                        horizontal-alignment: center;
                        vertical-alignment: center;
                    }
                    close-touch := TouchArea {
                        clicked => { AppBridge.dismiss(); }
                    }
                }
            }
        }

        // Selected text preview
        Rectangle {
            height: 40px;
            background: rgba(20, 18, 30, 0.5);
            border-radius: 6px;
            Text {
                text: AppBridge.selected-text == "" ? "No text selected" : AppBridge.selected-text;
                color: #9b93a8;
                font-size: 11px;
                font-italic: true;
                wrap: word-wrap;
                overflow: elide;
                horizontal-alignment: left;
                vertical-alignment: center;
                x: 8px;
                width: parent.width - 16px;
            }
        }

        // Language row
        LangRow {}

        // Mode row
        ModeRow {}

        // Chain row
        ChainRow {}

        // Stream output (only while streaming or done)
        if AppBridge.is-streaming || AppBridge.is-done: Rectangle {
            background: rgba(20, 18, 30, 0.6);
            border-radius: 6px;
            min-height: 60px;
            VerticalLayout {
                padding: 8px;
                spacing: 6px;
                Text {
                    text: AppBridge.stream-buffer;
                    color: #ffffff;
                    font-size: 12px;
                    wrap: word-wrap;
                }
                if AppBridge.is-done: HorizontalLayout {
                    spacing: 6px;
                    alignment: end;
                    Rectangle {
                        width: 72px;
                        height: 22px;
                        border-radius: 11px;
                        background: replace-touch.has-hover ? #c084fc : #9a6be0;
                        Text {
                            text: "Replace";
                            color: #1a1530;
                            font-size: 11px;
                            font-weight: 600;
                            horizontal-alignment: center;
                            vertical-alignment: center;
                        }
                        replace-touch := TouchArea {
                            clicked => { AppBridge.confirm-replace(); }
                        }
                    }
                }
                if AppBridge.is-streaming: HorizontalLayout {
                    spacing: 6px;
                    alignment: end;
                    Rectangle {
                        width: 60px;
                        height: 22px;
                        border-radius: 11px;
                        background: cancel-touch.has-hover ? #6a4a90 : #3a3450;
                        Text {
                            text: "Cancel";
                            color: #e0dbe8;
                            font-size: 11px;
                            horizontal-alignment: center;
                            vertical-alignment: center;
                        }
                        cancel-touch := TouchArea {
                            clicked => { AppBridge.cancel-stream(); }
                        }
                    }
                }
            }
        }

        // Error toast (sticks to the bottom)
        Toast {}
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add src/ui/slint/compact.slint
git commit -m "feat(ui/slint): compact view layout"
```

### Task 6: Rewrite `main_window.slint` — real root bound to AppBridge

**Files:**
- Modify: `src/ui/slint/main_window.slint`

- [ ] **Step 1: Replace the stub file wholesale**

```slint
import { AppBridge, ViewMode, ModeInfo, ChainInfo, LanguageOpt } from "./app_bridge.slint";
import { CompactView } from "./compact.slint";

// Re-export types so `slint::include_modules!()` exposes them to Rust.
export { AppBridge, ViewMode, ModeInfo, ChainInfo, LanguageOpt }

export component MainWindow inherits Window {
    title: "Quill";
    background: transparent;
    no-frame: true;
    always-on-top: AppBridge.view-mode == ViewMode.compact;

    // Compact default. Plan 4 grows this into a stateful morph.
    width: 380px;
    height: 360px;

    forward-focus: compact-focus;
    compact-focus := FocusScope {
        key-pressed(event) => {
            if (event.text == Key.Escape) {
                AppBridge.dismiss();
                return accept;
            }
            return reject;
        }
    }

    if AppBridge.view-mode == ViewMode.compact: CompactView {
        width: parent.width;
        height: parent.height;
    }

    if AppBridge.view-mode == ViewMode.expanded: Rectangle {
        background: rgba(38, 36, 48, 0.7);
        border-radius: 12px;
        width: parent.width;
        height: parent.height;
        Text {
            text: "Expanded view — Plan 4";
            color: #e0dbe8;
            horizontal-alignment: center;
            vertical-alignment: center;
        }
    }
}
```

- [ ] **Step 2: Build**

Run: `cargo build`
Expected: slint compiles the whole graph successfully. If you see errors about `@image-url` path resolution, adjust the relative path in `components/plume.slint` until it resolves against the actual file location.

Common errors and fixes:
- `cannot import "./app_bridge.slint"` — check the component file's directory; the import is always relative to the CURRENT `.slint` file.
- `unknown image format` — the SVG path is wrong or unreadable.
- `unused component/struct` — fine, just a warning.

- [ ] **Step 3: Commit**

```bash
git add src/ui/slint/main_window.slint
git commit -m "feat(ui/slint): main_window root imports CompactView + AppBridge"
```

---

## Phase 4 — Rust ↔ Slint Bridge (Day 2 PM)

### Task 7: `ui/bridge.rs` — event pump

**Files:**
- Create: `src/ui/bridge.rs`
- Modify: `src/ui/mod.rs` (add `pub mod bridge;`)

The event pump is a tokio task that `recv()`s `UiEvent`s and hops onto the Slint event loop via `slint::invoke_from_event_loop` to mutate `AppBridge` properties on the correct thread.

Slint-generated types live in `crate::ui::*` thanks to `slint::include_modules!()`. The generated `MainWindow` implements `ComponentHandle` — we can get a `Weak<MainWindow>` from it.

- [ ] **Step 1: Create `src/ui/bridge.rs`**

```rust
//! Rust ↔ Slint bridge — event pump and command forwarder.
//!
//! Architecture:
//! - `spawn_event_pump` runs on a tokio task. For each `UiEvent` it pops
//!   off the channel, it calls `slint::invoke_from_event_loop` with a closure
//!   that upgrades the `Weak<MainWindow>` and mutates the `AppBridge`
//!   properties on the Slint thread.
//! - `install_command_forwarder` runs on the Slint thread at startup.
//!   It registers Slint callbacks that pack arguments into `UiCommand`
//!   values and send them over a `mpsc::UnboundedSender<UiCommand>`.
//!
//! The Slint side only ever reads/writes its own properties. The Rust side
//! only ever reads/writes `AppState`. The channels are the full bridge.

use slint::ComponentHandle;
use tokio::sync::mpsc;

use crate::state::{UiCommand, UiEvent};
use crate::ui::{AppBridge, MainWindow};

/// Seed the AppBridge collections (modes, chains) from the engine's HashMaps.
/// Called once at startup BEFORE the window is shown.
pub fn seed_bridge(
    window: &MainWindow,
    modes: &std::collections::HashMap<String, crate::core::modes::ModeConfig>,
    chains: &std::collections::HashMap<String, crate::core::modes::ChainConfig>,
) {
    use crate::core::modes::{chains_list, modes_list};
    let bridge = window.global::<AppBridge>();

    let mode_models: Vec<crate::ui::ModeInfo> = modes_list(modes)
        .into_iter()
        .map(|m| crate::ui::ModeInfo {
            id: m.id.into(),
            label: m.label.into(),
            icon: m.icon.into(),
        })
        .collect();
    bridge.set_modes(slint::ModelRc::new(slint::VecModel::from(mode_models)));

    let chain_models: Vec<crate::ui::ChainInfo> = chains_list(chains)
        .into_iter()
        .map(|c| crate::ui::ChainInfo {
            id: c.id.into(),
            label: c.label.into(),
            icon: c.icon.into(),
            description: c.description.into(),
        })
        .collect();
    bridge.set_chains(slint::ModelRc::new(slint::VecModel::from(chain_models)));
}

/// Spawn a tokio task that drains `rx` and projects each `UiEvent` onto the
/// Slint `AppBridge` properties via `slint::invoke_from_event_loop`.
///
/// The `Weak<MainWindow>` is used instead of a strong handle so that when the
/// window is closed and dropped, the pump's closures become no-ops and the
/// pump terminates naturally when the event channel closes.
pub fn spawn_event_pump(window: &MainWindow, mut rx: mpsc::UnboundedReceiver<UiEvent>) {
    let weak = window.as_weak();
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let weak = weak.clone();
            // invoke_from_event_loop is the ONLY safe way to touch Slint
            // properties from a non-Slint thread. It posts the closure to
            // the Slint event queue; it runs synchronously on the next tick.
            let _ = slint::invoke_from_event_loop(move || {
                let Some(window) = weak.upgrade() else { return; };
                apply_event_on_ui_thread(&window, event);
            });
        }
        tracing::debug!("UiEvent pump shutting down — channel closed");
    });
}

fn apply_event_on_ui_thread(window: &MainWindow, event: UiEvent) {
    let bridge = window.global::<AppBridge>();
    match event {
        UiEvent::ShowOverlay { text, context, suggestion } => {
            bridge.set_selected_text(text.into());
            bridge.set_stream_buffer("".into());
            bridge.set_last_result("".into());
            bridge.set_is_streaming(false);
            bridge.set_is_done(false);
            bridge.set_error_message("".into());
            if let Some(s) = suggestion {
                bridge.set_active_mode(s.mode_id.into());
            } else {
                bridge.set_active_mode("".into());
            }
            // Show the window. Task 10 extends this with Mica reapply + positioning.
            let _ = window.show();
            tracing::debug!(app = %context.app, "ShowOverlay applied");
        }
        UiEvent::Dismiss => {
            let _ = window.hide();
        }
        UiEvent::StreamStart { mode, language } => {
            bridge.set_active_mode(mode.into());
            bridge.set_active_language(language.into());
            bridge.set_is_streaming(true);
            bridge.set_is_done(false);
            bridge.set_stream_buffer("".into());
        }
        UiEvent::StreamChunk { text } => {
            let mut buf = bridge.get_stream_buffer().to_string();
            buf.push_str(&text);
            bridge.set_stream_buffer(buf.into());
        }
        UiEvent::StreamDone { full_text, entry_id: _ } => {
            bridge.set_stream_buffer(full_text.clone().into());
            bridge.set_last_result(full_text.into());
            bridge.set_is_streaming(false);
            bridge.set_is_done(true);
        }
        UiEvent::StreamError { message } => {
            bridge.set_is_streaming(false);
            bridge.set_is_done(false);
            bridge.set_error_message(message.into());
        }
        UiEvent::ChainProgress { step, total, mode } => {
            bridge.set_active_mode(format!("{mode} ({step}/{total})").into());
        }
        UiEvent::ComparisonResult { .. } => {
            // Plan 4: surface in Compare tab. Ignored in Plan 3 MVP.
        }
        UiEvent::TutorExplanation { .. }
        | UiEvent::TutorLesson { .. }
        | UiEvent::Pronunciation { .. } => {
            // Plan 4 wires these up with the Tutor tab.
        }
        UiEvent::Error { message } => {
            bridge.set_error_message(message.into());
        }
    }
}

/// Register Slint callbacks that forward user intent into the engine via
/// the `UiCommand` channel. Must be called on the Slint thread (during
/// `MainWindow::new()` bootstrap).
pub fn install_command_forwarder(
    window: &MainWindow,
    tx: mpsc::UnboundedSender<UiCommand>,
) {
    let bridge = window.global::<AppBridge>();

    let tx_em = tx.clone();
    bridge.on_execute_mode(move |mode, lang| {
        let _ = tx_em.send(UiCommand::ExecuteMode {
            mode: mode.to_string(),
            language: lang.to_string(),
            extra: None,
        });
    });

    let tx_ec = tx.clone();
    bridge.on_execute_chain(move |chain_id, lang| {
        let _ = tx_ec.send(UiCommand::ExecuteChain {
            chain_id: chain_id.to_string(),
            language: lang.to_string(),
            extra: None,
        });
    });

    let window_weak = window.as_weak();
    bridge.on_set_language(move |code| {
        if let Some(w) = window_weak.upgrade() {
            w.global::<AppBridge>().set_active_language(code);
        }
    });

    let tx_cr = tx.clone();
    bridge.on_confirm_replace(move || {
        let _ = tx_cr.send(UiCommand::ConfirmReplace);
    });

    let tx_cs = tx.clone();
    bridge.on_cancel_stream(move || {
        let _ = tx_cs.send(UiCommand::CancelStream);
    });

    let tx_tv = tx.clone();
    let window_weak = window.as_weak();
    bridge.on_toggle_view(move || {
        // Local UI state — no engine involvement. But we still record it in
        // AppState via a small trip through the command channel in Plan 4.
        let _ = tx_tv; // reserved for Plan 4's UiCommand::ToggleView
        if let Some(w) = window_weak.upgrade() {
            let b = w.global::<AppBridge>();
            let next = match b.get_view_mode() {
                crate::ui::ViewMode::Compact => crate::ui::ViewMode::Expanded,
                crate::ui::ViewMode::Expanded => crate::ui::ViewMode::Compact,
            };
            b.set_view_mode(next);
        }
    });

    bridge.on_dismiss(move || {
        let _ = tx.send(UiCommand::Dismiss);
    });
}
```

- [ ] **Step 2: Update `src/ui/mod.rs`**

```rust
//! Slint UI layer.

slint::include_modules!();

pub mod bridge;
```

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: clean. Known gotchas:
- `slint::VecModel::from(Vec<T>)` requires `T` to be `Clone`. The generated `ModeInfo`/`ChainInfo` are `Clone`.
- `ComponentHandle::as_weak()` returns `slint::Weak<T>`. `Weak::upgrade()` returns `Option<T>`.
- `bridge.get_stream_buffer()` returns a `SharedString`; `.to_string()` gives an owned `String`.
- If the compiler says `ViewMode` isn't imported from `crate::ui::ViewMode`, verify `main_window.slint` re-exports it via the `export { ... }` statement at the top (Task 6 Step 1).

- [ ] **Step 4: Commit**

```bash
git add src/ui/bridge.rs src/ui/mod.rs
git commit -m "feat(ui): bridge event pump + command forwarder"
```

---

## Phase 5 — Window Construction + Mica (Day 2 late / Day 3 AM)

### Task 8: `ui/main_window.rs` — HWND grab + Mica + DWM shadow

**Files:**
- Create: `src/ui/main_window.rs`
- Modify: `src/ui/mod.rs`

This module lives on the Slint thread and is called at boot. It:
1. Constructs `MainWindow::new()?`
2. Retrieves the native `HWND` from the winit window
3. Applies DPI awareness, Mica backdrop, DWM native shadow, round corners

- [ ] **Step 1: Check how Slint 1.8 exposes the winit window**

Slint 1.8 under the `backend-winit` feature exposes `slint::WindowHandle` / `slint::winit::WinitWindowAccessor`. The accessor pattern differs slightly between 1.7 and 1.8. Before writing code, verify the available API:

```bash
grep -rn "WinitWindowAccessor\|with_winit_window\|raw_window_handle" ~/.cargo/registry/src 2>/dev/null | head -5
```

If `slint::winit::WinitWindowAccessor::with_winit_window` exists, use it. Otherwise fall back to `raw_window_handle::HasWindowHandle` via `window.window_handle()?.as_raw()` — `RawWindowHandle::Win32(h)` gives `HWND` as `h.hwnd.get() as HWND`.

(The subagent implementing this task should pick the path that compiles against the actually-installed Slint version. Both are spec-legitimate.)

- [ ] **Step 2: Create `src/ui/main_window.rs`**

```rust
//! Construct the Slint MainWindow and apply Windows-specific visual attributes
//! (Mica backdrop, DWM shadow, round corners, dark mode). All operations on
//! this module's functions run on the Slint event-loop thread.

use anyhow::{Context, Result};
use slint::ComponentHandle;
use windows::Win32::Foundation::HWND;

use crate::platform::{dwm_shadow, mica};
use crate::ui::MainWindow;

/// Build the MainWindow and apply Windows visual treatment. Returns the
/// strong handle — caller keeps it alive for the event loop's duration.
pub fn build() -> Result<MainWindow> {
    let window = MainWindow::new().context("MainWindow::new failed")?;

    // Grab the HWND. See Task 8 Step 1 for the API selection rationale.
    let hwnd = hwnd_of(&window)?;

    // Apply visual treatment. Failures are non-fatal — on pre-22H2 Windows or
    // in Wine/virtualized envs, Mica silently fails; the window falls back
    // to the solid background set in main_window.slint.
    if let Err(e) = mica::apply(hwnd, mica::MicaVariant::Main) {
        tracing::warn!("Mica apply failed: {e}");
    }
    if let Err(e) = dwm_shadow::enable(hwnd) {
        tracing::warn!("DWM shadow enable failed: {e}");
    }

    Ok(window)
}

/// Extract the native Win32 HWND from a Slint MainWindow.
///
/// Uses the `raw-window-handle` path (`slint::Window::window_handle`) which is
/// stable across Slint 1.7/1.8 under the `backend-winit` feature.
fn hwnd_of(window: &MainWindow) -> Result<HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = window
        .window()
        .window_handle()
        .context("no raw-window-handle available from Slint window")?;
    match handle.as_raw() {
        RawWindowHandle::Win32(h) => Ok(HWND(h.hwnd.get() as *mut _)),
        other => Err(anyhow::anyhow!("expected Win32 window handle, got {:?}", other)),
    }
}
```

NOTE: `raw-window-handle` may not be a direct dependency. `slint 1.8` re-exports it — if the import fails, add `raw-window-handle = "0.6"` to `Cargo.toml` `[dependencies]`. Check the active version:

```bash
cargo tree | grep raw-window-handle
```

If Slint pulls 0.6.x, match that version in `Cargo.toml`.

- [ ] **Step 3: Update `src/ui/mod.rs`**

```rust
//! Slint UI layer.

slint::include_modules!();

pub mod bridge;
pub mod main_window;
```

- [ ] **Step 4: Build**

Run: `cargo build`
Expected: clean. Most likely snags:
- `raw-window-handle` version mismatch — add explicit dep matching Slint's.
- `HWND` cast from `NonZeroIsize`: use `.get() as isize as *mut _`.
- `mica::apply` returns `Result<()>` — the `if let Err` pattern above works; no unwrap.

- [ ] **Step 5: Commit**

```bash
git add src/ui/main_window.rs src/ui/mod.rs Cargo.toml
git commit -m "feat(ui): MainWindow construction with Mica + DWM shadow"
```

---

## Phase 6 — Full Boot & Smoke (Day 3)

### Task 9: Full `main.rs` rewrite — tokio runtime, engine task loop, hotkey, tray, window

**Files:**
- Modify: `src/main.rs`

This is the biggest single task in Plan 3. It wires every subsystem into the Slint event loop.

**Critical ordering:**
1. Tracing init
2. `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` — MUST come before Slint window creation
3. Build tokio `Runtime` explicitly (Slint owns the main thread from `run()` onward, so we can't use `#[tokio::main]`)
4. Load config + modes + chains
5. Build `AppState`, `UiEvent` channel, `UiCommand` channel
6. Build `Engine` with real platform impls
7. Build `MainWindow` via `ui::main_window::build()`
8. Seed AppBridge collections (modes, chains)
9. `ui::bridge::install_command_forwarder(&window, ui_tx.clone())`
10. Spawn tokio task that drains `ui_rx` (`UiCommand`s) and calls `engine.handle_command(cmd).await` for each
11. Spawn `ui::bridge::spawn_event_pump(&window, event_rx)` — event pump must be spawned AFTER the window exists so the `Weak<MainWindow>` is valid
12. Register global hotkey; spawn a background thread that drains `GlobalHotKeyEvent::receiver()` and posts `rt.spawn(engine::hotkey_flow::handle_hotkey(engine.clone()))`
13. Build tray + spawn tray event listener (uses `slint::invoke_from_event_loop` for menu actions that touch the window)
14. `window.run()?` — blocks until window closes
15. Graceful shutdown: drop engine, drop rt, return

- [ ] **Step 1: Replace `src/main.rs`**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod engine;
mod platform;
mod providers;
mod state;
mod ui;

use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use slint::ComponentHandle;
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

use crate::core::config::load_config;
use crate::core::modes::load_modes;
use crate::engine::Engine;
use crate::platform::capture::Capture;
use crate::platform::context::Context as ContextImpl;
use crate::platform::hotkey::HotkeyService;
use crate::platform::replace::Replace;
use crate::platform::traits::{ContextProbe, TextCapture, TextReplace};
use crate::platform::tray::{TrayEvent, TrayService, TRAY_ICON_PNG};
use crate::providers::{build_provider, Provider};
use crate::state::{AppState, UiCommand, UiEvent};
use crate::ui::{bridge, main_window};

fn main() -> Result<()> {
    init_tracing();
    tracing::info!("Quill boot — Plan 3 Slint MVP");

    enable_dpi_awareness();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    let rt_handle = rt.handle().clone();

    let config = load_config();
    let (modes, chains) = load_modes(&config);

    let state = Arc::new(Mutex::new(AppState::new()));
    let (event_tx, event_rx) = mpsc::unbounded_channel::<UiEvent>();
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<UiCommand>();

    let capture_impl: Arc<dyn TextCapture> = Arc::new(Capture);
    let replace_impl: Arc<dyn TextReplace> = Arc::new(Replace);
    let context_impl: Arc<dyn ContextProbe> = Arc::new(ContextImpl);
    let provider: Arc<dyn Provider> = build_provider(&config);

    let engine = Engine::new(
        config.clone(),
        modes.clone(),
        chains.clone(),
        state,
        event_tx,
        capture_impl,
        replace_impl,
        context_impl,
        provider,
    );

    // Build the Slint window + apply Mica. Must run on the main (Slint) thread.
    let window = main_window::build()?;
    bridge::seed_bridge(&window, &modes, &chains);
    bridge::install_command_forwarder(&window, cmd_tx.clone());
    bridge::spawn_event_pump(&window, event_rx);

    // Drain UiCommand → engine.handle_command on tokio worker tasks. Each
    // command runs concurrently; cancellation is handled inside the engine.
    {
        let engine = engine.clone();
        rt_handle.spawn(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                let engine = engine.clone();
                tokio::spawn(async move {
                    engine.handle_command(cmd).await;
                });
            }
            tracing::debug!("UiCommand drain shutting down — channel closed");
        });
    }

    // Global hotkey — listener thread posts handle_hotkey onto tokio
    let hotkey_spec = config.hotkey.clone().unwrap_or_else(|| "Ctrl+Shift+Space".into());
    let _hotkey_service = spawn_hotkey_listener(&hotkey_spec, engine.clone(), rt_handle.clone())?;

    // Tray — listener thread forwards menu events to the Slint thread
    let _tray_service = spawn_tray_listener(window.as_weak(), cmd_tx.clone())?;

    tracing::info!("Quill ready — entering Slint event loop");
    window.run().context("Slint event loop returned an error")?;

    // Graceful shutdown. Dropping `rt` joins all worker threads.
    tracing::info!("Quill exit — shutting down tokio runtime");
    drop(rt);
    Ok(())
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("quill=info")),
        )
        .init();
}

#[cfg(target_os = "windows")]
fn enable_dpi_awareness() {
    use windows::Win32::UI::HiDpi::{
        SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };
    unsafe {
        if let Err(e) = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) {
            tracing::warn!("SetProcessDpiAwarenessContext failed: {e}");
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn enable_dpi_awareness() {}

fn spawn_hotkey_listener(
    spec: &str,
    engine: Engine,
    rt: tokio::runtime::Handle,
) -> Result<HotkeyService> {
    let mut service = HotkeyService::new().context("HotkeyService::new")?;
    service
        .register(spec)
        .with_context(|| format!("register hotkey {spec}"))?;

    std::thread::spawn(move || {
        use global_hotkey::GlobalHotKeyEvent;
        let receiver = GlobalHotKeyEvent::receiver();
        loop {
            match receiver.recv() {
                Ok(event) => {
                    if crate::platform::hotkey::is_pressed(&event) {
                        let engine = engine.clone();
                        rt.spawn(async move {
                            crate::engine::hotkey_flow::handle_hotkey(engine).await;
                        });
                    }
                }
                Err(e) => {
                    tracing::warn!("hotkey receiver error: {e}");
                    break;
                }
            }
        }
    });

    Ok(service)
}

fn spawn_tray_listener(
    window_weak: slint::Weak<ui::MainWindow>,
    cmd_tx: mpsc::UnboundedSender<UiCommand>,
) -> Result<TrayService> {
    let service = TrayService::new(TRAY_ICON_PNG).context("TrayService::new")?;

    std::thread::spawn(move || loop {
        let events = service_poll_workaround();
        for event in events {
            match event {
                TrayEvent::IconClicked => {
                    let weak = window_weak.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = weak.upgrade() {
                            let _ = w.show();
                        }
                    });
                }
                TrayEvent::MenuItem(id) => match id.as_str() {
                    "quit" => {
                        let _ = slint::invoke_from_event_loop(|| slint::quit_event_loop());
                        break;
                    }
                    "show" | "panel" | "settings" => {
                        let weak = window_weak.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = weak.upgrade() {
                                let _ = w.show();
                            }
                        });
                    }
                    _ => {}
                },
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    });

    Ok(service)
}

// Placeholder — TrayService in Plan 1 uses poll semantics. If the actual API
// is channel-based in your copy of tray.rs, replace `service_poll_workaround`
// with the channel's `try_iter()` or `recv_timeout()` pattern. The goal is
// the same: a thread loop that drains events.
fn service_poll_workaround() -> Vec<TrayEvent> {
    Vec::new()
}
```

**Snags to watch while writing this:**
- `HotkeyService::new` and `TrayService::new` signatures may differ from the plan. Read `src/platform/hotkey.rs` and `src/platform/tray.rs` first and match exactly.
- `TrayService` in Plan 1 uses a `poll()` method returning `Vec<TrayEvent>`. The `spawn_tray_listener` helper above is written around that. If the actual signature is different, replace `service_poll_workaround()` with the real `service.poll()` call, and move `service` into the thread closure properly (it's `!Send`? check).
- `Config::hotkey` field name may differ — grep for `hotkey` in `src/core/config.rs`.
- `slint::quit_event_loop()` returns `Result<(), EventLoopError>` in some versions — ignore with `let _ = ...`.
- Don't forget to REMOVE the `#![allow(dead_code, unused_imports)]` shield now — main.rs is fully wired. If removing it exposes warnings in OTHER files (`platform/caret.rs`, etc.), push the allow DOWN to those specific files rather than keeping it crate-wide.

- [ ] **Step 2: Fix compiler errors iteratively**

`cargo build` is likely to show several errors the first time. Fix each one:
- Missing imports
- Signature mismatches with `HotkeyService` / `TrayService`
- `ContextProbe`-as-trait import path
- Slint `ModelRc`/`VecModel` generics
- `raw-window-handle` version drift (Task 8)

Work through them. Do NOT suppress anything with `#[allow(...)]` unless it's genuinely pre-existing Plan 2 scaffolding.

- [ ] **Step 3: Smoke run**

Run: `cargo run 2>&1 | head -40`
Expected: Quill window appears (dark Mica backdrop, plume glyph, "No text selected" preview, mode row, language row, chain row). Tracing logs `Quill ready — entering Slint event loop`.

Close the window → process exits cleanly with "Quill exit — shutting down tokio runtime".

If the window is invisible or crashes, check:
- The `.slint` compiler picked up your changes (`touch src/ui/slint/main_window.slint && cargo build`)
- Mica fallback: black window is fine, transparent black is also fine — means Mica isn't applying but the app still runs
- Hotkey conflict: the default `Ctrl+Shift+Space` may already be taken. Check logs for "register hotkey ... failed" — if so, it's non-fatal for Plan 3 (the UI still works via mouse).

- [ ] **Step 4: Commit**

```bash
git add src/main.rs Cargo.toml
git commit -m "feat(main): full Slint event loop with hotkey and tray wiring"
```

### Task 10: Final verification + tag `plan-03-slint-mvp-complete`

- [ ] **Step 1: Full sweep**

```bash
cargo fmt
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --lib
cargo test --test engine_integration
cargo build --release
```

All must pass. Any clippy lint that fires on new code gets fixed inline; clippy lints on pre-existing Plan 1 platform modules can be allow-listed if (and only if) they're genuinely WIP scaffolding not yet wired (examples: `caret.rs` focus event consumers land in Plan 5).

- [ ] **Step 2: Sanity greps**

```bash
grep -rn "todo!\|unimplemented!\|unreachable!" src/ui/
grep -rn "noop_emit\|SharedEngine\|tauri::" src/ tests/
```

Expected: empty. (`unreachable!()` inside a match on a closed-set enum is allowed; `todo!()` is not.)

- [ ] **Step 3: Tag**

```bash
git tag plan-03-slint-mvp-complete
```

- [ ] **Step 4: Save session to engram**

Call `mcp__plugin_engram_engram__mem_save` with:
- project: `Quill`
- scope: `project`
- topic_key: `quill/slint-rewrite/plan-03`
- type: `architecture`
- title: `Plan 3 (Slint MVP) complete`
- content: Summarize the final shape — `src/ui/slint/*.slint` files, `src/ui/{bridge,main_window,mod}.rs`, main.rs full boot, first green Slint window launch with Mica, hotkey + tray listeners wired. Note remaining follow-ups: Expanded view tabs (Plan 4), floating pencil (Plan 5), visual polish (Plan 6). Flag any deviations the implementer had to make from this plan (likely around raw-window-handle version or Slint 1.8 winit API details that drifted from the spec).

- [ ] **Step 5: Report manual verification checklist**

Print (to the user, not committed) a short list of things to manually eyeball:
1. Window appears with Mica-like translucency (or solid dark fallback) — OK
2. Plume glyph visible in header — OK
3. Language row clickable, active pill highlights — OK
4. Hotkey captures selection from Notepad and shows the window with text preview — OK
5. Clicking a mode with a reachable provider streams tokens into the output area — OK if provider config is valid, OK-to-error-toast if not
6. ESC and ✕ both dismiss the window — OK
7. Tray right-click shows the menu; Quit exits — OK

---

## Self-Review Notes

**Known drift from the spec (and why):**
- No floating pencil (`PencilWindow`) — Plan 5
- No expanded view content (tabs/history/settings/compare/tutor) — Plan 4
- No morph animation between compact and expanded — placeholder swap only — Plan 4
- Tray listener uses a `poll()` loop (matching Plan 1 `TrayService`) rather than the spec's receiver-based design — can flip in Plan 4 cleanup if desired
- `UiCommand::ToggleView` variant not added — Plan 3 toggles view-mode locally in the Slint thread since there's nothing meaningful for the engine to do yet. Plan 4 promotes it to a real command when view-mode affects active requests (e.g. auto-load-history on expand)

**Likely snags for the implementer:**
1. `slint::winit::WinitWindowAccessor` API shape varies between 1.7 and 1.8. Task 8 Step 1 explicitly tells you to verify and pick a path.
2. `raw-window-handle` version drift — may need an explicit `Cargo.toml` entry matching Slint 1.8's pinned version.
3. `TrayService` poll loop may need to hold `TrayService` inside the thread (not return it) if the underlying `tray_icon::TrayIcon` isn't `Send`. Adapt accordingly — the plan's structure is a guideline.
4. The `#![allow(dead_code, unused_imports)]` shield from Plan 2 can probably come off in Plan 3 since most platform modules are now reachable. Try removing it first; if >5 new warnings appear, push the allow down to the specific files that are still WIP.

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-04-14-quill-slint-plan-03-slint-mvp.md`.**

Execution: subagent-driven, one implementer per phase (same pattern as Plan 2).
