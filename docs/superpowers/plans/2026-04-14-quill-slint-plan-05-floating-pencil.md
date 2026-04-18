# Quill Slint Rewrite — Plan 5: Floating Pencil Window

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the always-on-top plume indicator online. When the user's focus lands on an editable text control, a 32×32 pencil glyph appears next to the caret. Clicking it triggers the same capture flow as the hotkey. Plan 1 scaffolded `platform/caret.rs` (WinEvent hook thread) and `platform/uia.rs` (UIA wrapper) — Plan 5 wires them end-to-end and builds the pencil UI.

**Architecture:**
- Enrich `caret::FocusEvent` via a worker thread that calls `uia::Uia::is_editable_text` + `caret_bounds`. Raw WinEvent hooks stay naive (just forward hwnd); UIA lookups happen on a separate tokio-blocking worker to avoid COM re-entrancy inside the hook callback.
- New Slint component `pencil.slint` with an exported `PencilWindow` root.
- New Rust module `src/ui/pencil_window.rs` — constructs the window, extracts HWND, applies `WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT`, exposes `show()`, `hide()`, `set_position(x, y)`, `set_click_through(bool)`.
- New Rust module `src/ui/pencil_controller.rs` — drains the `FocusEvent` channel, enriches via UIA, shows/moves/hides the pencil, polls mouse position at ~30 Hz to toggle click-through when the cursor is within 40 px.
- `src/main.rs` — start `CaretHookService`, build `PencilWindow`, spawn `pencil_controller`, wire the pencil's click callback to `engine::hotkey_flow::handle_hotkey`.

**Preconditions:**
- Branch `claude/slint-rewrite` at tag `plan-04-expanded-tabs-complete`
- 64 lib tests + 7 integration tests green
- `src/platform/caret.rs` — `CaretHookService::start(tx)` exists; hook emits naive `FocusEvent::FocusChanged { editable: false, anchor: None }` and `CaretMoved { rect: (0,0,0,0) }`
- `src/platform/uia.rs` — `Uia::focused_element`, `Uia::is_editable_text`, `Uia::caret_bounds` exist
- Plan 3's `src/ui/main_window.rs` pattern for HWND extraction via `raw-window-handle` works

**End state:**
- Focus in Notepad / Word / VSCode editable region → pencil appears near the caret
- Focus in Explorer, taskbar, a non-editable window → pencil hides
- Moving the caret within an editable region → pencil tracks at ~30 Hz (debounced)
- Hovering within 40 px → pencil becomes clickable (click-through disabled)
- Clicking the pencil → same flow as hotkey (capture → show main window)
- `cargo clippy --all-targets -- -D warnings` clean
- `cargo test --lib` + `cargo test --test engine_integration` green
- Tagged `plan-05-floating-pencil-complete`

**Out of scope (deferred to Plan 6 polish):**
- Hover tooltip "Rewrite with Quill" — Plan 5 has the glyph only
- Fade-in/fade-out animation (60 ms in, 120 ms out) — Plan 5 uses instant show/hide
- Position smoothing when caret jumps — Plan 5 snaps
- Pencil hiding when the main window is focused — Plan 5 hides only on non-editable focus
- Per-monitor DPI awareness for the pencil position — Plan 5 trusts UIA to return physical pixels and uses them directly

---

## Design Reference

### Threading model

```
caret-hook thread (Plan 1)       uia-worker thread (Plan 5)         Slint main thread
──────────────────────           ──────────────────────             ─────────────────
SetWinEventHook callback    ──▶  mpsc::Receiver<RawFocusEvt>   ──▶  slint::invoke_from_event_loop
   emits RawFocusEvt                                                    → PencilWindow::show/move/hide
                                  (drains events, calls UIA on
                                   blocking task, enriches with
                                   editable + caret bounds,
                                   forwards PencilCmd)
```

Why a separate UIA worker thread: calling `Uia::focused_element` inside the WinEvent hook callback is unsafe — the hook runs synchronously on the message pump, COM apartment is uninitialized by default, and UIA itself may re-enter the hook. The UIA worker runs in its own `CoInitializeEx(COINIT_MULTITHREADED)` apartment, owned by one dedicated std::thread.

### Mouse-proximity click-through toggle

The pencil is rendered with `WS_EX_TRANSPARENT` (clicks pass through) so it never eats stray clicks in the user's real application. When the cursor approaches within 40 px of the pencil center, Plan 5 removes `WS_EX_TRANSPARENT` via `SetWindowLongPtrW` — clicks now register. When the cursor leaves the 40 px radius, `WS_EX_TRANSPARENT` is restored.

Polling frequency: 30 Hz (33 ms tick) via `slint::Timer`. Runs on the Slint thread — no extra thread.

### New channel shape

```rust
// src/ui/pencil_controller.rs
pub enum PencilCmd {
    ShowAt { x: i32, y: i32 },
    Hide,
}
```

Emitted by the UIA worker, consumed by the Slint-thread receiver via `slint::invoke_from_event_loop`.

### New Slint component

```slint
// src/ui/slint/pencil.slint
import { Plume } from "./components/plume.slint";

export component PencilWindow inherits Window {
    title: "Quill Pencil";
    background: transparent;
    no-frame: true;
    always-on-top: true;
    width: 32px;
    height: 32px;

    callback clicked();

    Rectangle {
        width: 100%;
        height: 100%;
        background: touch.has-hover ? rgba(192, 132, 252, 0.2) : transparent;
        border-radius: 8px;
        Plume { width: 24px; height: 24px; }
        touch := TouchArea {
            clicked => { root.clicked(); }
        }
    }
}
```

---

## Phase 1 — Pencil Slint + Rust Window Module (Day 1)

### Task 1: Create `src/ui/slint/pencil.slint`

**Files:**
- Create: `src/ui/slint/pencil.slint`

- [ ] **Step 1: Write the file**

```slint
import { Plume } from "./components/plume.slint";

export component PencilWindow inherits Window {
    title: "Quill Pencil";
    background: transparent;
    no-frame: true;
    always-on-top: true;
    width: 32px;
    height: 32px;

    callback clicked();

    Rectangle {
        width: 100%;
        height: 100%;
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

- [ ] **Step 2: Register the file with slint-build**

Open `build.rs`. Below the existing `slint_build::compile_with_config(...)` line for `main_window.slint`, add a second compile call for `pencil.slint`:

```rust
slint_build::compile_with_config(
    "src/ui/slint/pencil.slint",
    slint_build::CompilerConfiguration::new().with_style("fluent-dark".into()),
)
.expect("slint pencil compilation failed");
```

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: slint compiles the pencil component. The generated Rust type `PencilWindow` becomes available via `slint::include_modules!()` — already in `src/ui/mod.rs`.

- [ ] **Step 4: Commit**

```bash
git add src/ui/slint/pencil.slint build.rs
git commit -m "feat(ui/slint): pencil window component"
```

### Task 2: Create `src/ui/pencil_window.rs` — Rust wrapper + WinAPI styles

**Files:**
- Create: `src/ui/pencil_window.rs`
- Modify: `src/ui/mod.rs`

- [ ] **Step 1: Create `src/ui/pencil_window.rs`**

```rust
//! Floating pencil window. Constructs the Slint `PencilWindow`, grabs the
//! HWND, applies always-on-top + click-through + no-taskbar + no-activate
//! Win32 extended styles, and exposes helpers to position / show / hide
//! and to toggle click-through at runtime.
//!
//! The pencil lives on the Slint main thread. All methods must be called
//! from that thread (they ultimately touch `ComponentHandle`).

use anyhow::{Context, Result};
use slint::ComponentHandle;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
};

use crate::ui::PencilWindow;

/// Build the pencil window and apply Win32 extended styles.
///
/// Note: per Phase 5 findings in Plan 3, the HWND is NOT available until the
/// window is shown for the first time. This function:
///   1. Constructs the window
///   2. Calls `show()` once to force winit to create the native HWND
///   3. Grabs the HWND + applies styles
///   4. Calls `hide()` so the window is not visible on startup
///
/// After this function returns, the caller can call `show()`/`hide()` freely
/// — the styles persist across hide/show cycles.
pub fn build() -> Result<PencilWindow> {
    let window = PencilWindow::new().context("PencilWindow::new failed")?;

    // Force native HWND creation by showing once.
    window.show().context("initial show() to materialize HWND")?;

    let hwnd = hwnd_of(&window)?;
    apply_pencil_styles(hwnd, true);

    window.hide().context("hide() after style apply")?;
    Ok(window)
}

/// Apply the base pencil Win32 extended styles. `click_through=true` adds
/// `WS_EX_TRANSPARENT`; `false` removes it so the pencil can receive clicks.
pub fn apply_pencil_styles(hwnd: HWND, click_through: bool) {
    unsafe {
        let current = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let base = current
            | WS_EX_LAYERED.0
            | WS_EX_TOOLWINDOW.0
            | WS_EX_NOACTIVATE.0;
        let new = if click_through {
            base | WS_EX_TRANSPARENT.0
        } else {
            base & !WS_EX_TRANSPARENT.0
        };
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new as isize);
    }
}

/// Set the pencil's screen position. `x` / `y` are in physical pixels
/// (the same coordinate space as UIA `GetBoundingRectangles`).
pub fn set_position(window: &PencilWindow, x: i32, y: i32) {
    window
        .window()
        .set_position(slint::PhysicalPosition::new(x, y));
}

/// Retrieve the HWND from the pencil window's native handle.
pub fn hwnd_of(window: &PencilWindow) -> Result<HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = window
        .window()
        .window_handle()
        .context("pencil window has no raw window handle")?;
    match handle.as_raw() {
        RawWindowHandle::Win32(h) => Ok(HWND(h.hwnd.get() as *mut _)),
        other => Err(anyhow::anyhow!("expected Win32 handle, got {:?}", other)),
    }
}
```

- [ ] **Step 2: Register the module in `src/ui/mod.rs`**

Add `pub mod pencil_window;` alongside `pub mod main_window;` and `pub mod bridge;`.

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: clean. Possible snags:
- `slint::PhysicalPosition::new(x, y)` — verify the constructor on Slint 1.15. Alternative: `slint::LogicalPosition::new(x as f32, y as f32)` wrapped in `PhysicalPosition::from_logical` if needed.
- `WS_EX_TRANSPARENT.0 & !WS_EX_TRANSPARENT.0` — `WS_EX_TRANSPARENT` is a `WINDOW_EX_STYLE(u32)` tuple struct in `windows-rs` 0.58; `.0` yields the `u32`.
- `GetWindowLongPtrW` / `SetWindowLongPtrW` return `isize` on 64-bit — the cast `as u32` truncates, then re-casting with `as isize` packs the 32-bit style back. On 32-bit Windows this would be lossy, but Plan 5 targets x86_64 only.

- [ ] **Step 4: Commit**

```bash
git add src/ui/pencil_window.rs src/ui/mod.rs
git commit -m "feat(ui): pencil_window Rust wrapper with click-through styles"
```

---

## Phase 2 — Pencil Controller + UIA Worker (Day 2)

### Task 3: Create `src/ui/pencil_controller.rs`

This module owns the UIA worker thread, enriches raw `caret::FocusEvent`s, and pumps `PencilCmd`s onto the Slint event loop.

**Files:**
- Create: `src/ui/pencil_controller.rs`
- Modify: `src/ui/mod.rs`

- [ ] **Step 1: Create `src/ui/pencil_controller.rs`**

```rust
//! UIA worker thread that enriches raw `caret::FocusEvent`s with editability
//! + caret bounds, and posts `PencilCmd`s onto the Slint event loop.
//!
//! Running UIA calls in a dedicated worker avoids:
//! - COM apartment pitfalls inside the WinEvent hook callback
//! - Re-entrancy when UIA internally posts its own events
//! - Blocking the tokio worker pool on synchronous COM calls
//!
//! The worker owns a single `Uia` instance constructed on its thread
//! (Uia::new() calls CoInitializeEx internally — safe here).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use slint::ComponentHandle;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::platform::caret::FocusEvent;
use crate::platform::uia::Uia;
use crate::ui::{pencil_window, PencilWindow};

/// Command posted to the Slint thread to update the pencil window.
#[derive(Debug, Clone, Copy)]
enum PencilCmd {
    ShowAt { x: i32, y: i32 },
    Hide,
}

/// Caret-to-pencil horizontal offset in physical pixels.
const CARET_OFFSET_X: i32 = 24;

/// Handle to the background worker thread.
pub struct PencilController {
    stop_flag: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl PencilController {
    /// Spawn the worker thread. It drains `rx` (raw `FocusEvent`s from the
    /// caret hook) and projects enriched pencil commands onto `window` via
    /// `slint::invoke_from_event_loop`.
    pub fn start(window: &PencilWindow, mut rx: UnboundedReceiver<FocusEvent>) -> Self {
        let weak = window.as_weak();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = Arc::clone(&stop_flag);

        let thread = thread::Builder::new()
            .name("quill-pencil-uia".into())
            .spawn(move || {
                // Initialize UIA on this thread's COM apartment.
                let uia = match Uia::new() {
                    Ok(u) => u,
                    Err(e) => {
                        tracing::warn!("pencil UIA worker: Uia::new failed: {e:#}");
                        return;
                    }
                };

                while !stop_flag_clone.load(Ordering::Acquire) {
                    // Use `blocking_recv` since this is a dedicated std::thread
                    // (not a tokio task). `None` means the sender was dropped —
                    // caret service is shutting down; exit.
                    let Some(event) = rx.blocking_recv() else {
                        break;
                    };

                    let cmd = handle_focus_event(&uia, event);
                    if let Some(cmd) = cmd {
                        let weak = weak.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            let Some(window) = weak.upgrade() else { return; };
                            apply_cmd(&window, cmd);
                        });
                    }
                }
            })
            .expect("spawn pencil UIA worker thread");

        Self {
            stop_flag,
            thread: Some(thread),
        }
    }

    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for PencilController {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Translate one raw `FocusEvent` into a pencil command (or `None` if the
/// event is noise — e.g. focus in a non-editable control).
fn handle_focus_event(uia: &Uia, event: FocusEvent) -> Option<PencilCmd> {
    match event {
        FocusEvent::FocusChanged { .. } => {
            // Query the currently focused element. UIA can report None on
            // elevated windows or transient state.
            let element = uia.focused_element().ok()?;
            let editable = uia.is_editable_text(&element).ok().unwrap_or(false);
            if !editable {
                return Some(PencilCmd::Hide);
            }
            // Position next to the caret if we can get it; otherwise hide.
            let rect = uia.caret_bounds().ok().flatten()?;
            Some(PencilCmd::ShowAt {
                x: rect.right + CARET_OFFSET_X,
                y: rect.top,
            })
        }
        FocusEvent::CaretMoved { .. } => {
            // Re-query the focused element — a caret move within a newly
            // focused control may not yet have generated a FocusChanged.
            let element = uia.focused_element().ok()?;
            if !uia.is_editable_text(&element).ok().unwrap_or(false) {
                return Some(PencilCmd::Hide);
            }
            let rect = uia.caret_bounds().ok().flatten()?;
            Some(PencilCmd::ShowAt {
                x: rect.right + CARET_OFFSET_X,
                y: rect.top,
            })
        }
        FocusEvent::FocusLost => Some(PencilCmd::Hide),
    }
}

fn apply_cmd(window: &PencilWindow, cmd: PencilCmd) {
    match cmd {
        PencilCmd::ShowAt { x, y } => {
            pencil_window::set_position(window, x, y);
            let _ = window.show();
        }
        PencilCmd::Hide => {
            let _ = window.hide();
        }
    }
}
```

- [ ] **Step 2: Register the module in `src/ui/mod.rs`**

Add `pub mod pencil_controller;` alongside the other `pub mod` lines.

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: clean. Possible snags:
- `UnboundedReceiver::blocking_recv()` requires feature `rt` on tokio — already enabled via `tokio = { features = ["full"] }`.
- `Uia::caret_bounds` returns `Result<Option<ScreenRect>>` per Plan 1's `uia.rs`. The `.ok().flatten()?` chain handles the Result + Option layers.
- If `ScreenRect` has named fields (`left`, `top`, `right`, `bottom`), the `.right` / `.top` accesses work. Verify with `grep -n "struct ScreenRect" src/platform/`.

- [ ] **Step 4: Commit**

```bash
git add src/ui/pencil_controller.rs src/ui/mod.rs
git commit -m "feat(ui): pencil_controller enriches focus events via UIA worker"
```

### Task 4: Add click-through proximity toggle to `pencil_window.rs`

**Files:**
- Modify: `src/ui/pencil_window.rs`

The toggle polls the mouse position at 30 Hz on the Slint thread via `slint::Timer`, and flips `WS_EX_TRANSPARENT` when the cursor is within 40 px of the pencil center.

- [ ] **Step 1: Add the proximity toggle helper**

Append to `src/ui/pencil_window.rs`:

```rust
/// Install a 30 Hz `slint::Timer` that polls the mouse cursor position and
/// toggles click-through off when within `PROXIMITY_PX` of the pencil center.
/// Returns the timer so the caller can keep it alive (dropping it stops the
/// polling).
pub fn install_proximity_toggle(window: &PencilWindow) -> slint::Timer {
    use slint::{Timer, TimerMode};
    use std::cell::Cell;
    use std::rc::Rc;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    use windows::Win32::Foundation::POINT;

    const PROXIMITY_PX: i32 = 40;
    const PENCIL_HALF_WIDTH: i32 = 16;
    const PENCIL_HALF_HEIGHT: i32 = 16;

    let weak = window.as_weak();
    let was_click_through: Rc<Cell<bool>> = Rc::new(Cell::new(true));

    let timer = Timer::default();
    timer.start(
        TimerMode::Repeated,
        std::time::Duration::from_millis(33),
        move || {
            let Some(window) = weak.upgrade() else { return; };

            // Read the pencil position.
            let pos = window.window().position();
            let pencil_center = (
                pos.x + PENCIL_HALF_WIDTH,
                pos.y + PENCIL_HALF_HEIGHT,
            );

            // Read the cursor position.
            let mut cursor = POINT { x: 0, y: 0 };
            if unsafe { GetCursorPos(&mut cursor) }.is_err() {
                return;
            }

            let dx = cursor.x - pencil_center.0;
            let dy = cursor.y - pencil_center.1;
            let within = (dx * dx + dy * dy) <= (PROXIMITY_PX * PROXIMITY_PX);

            let should_click_through = !within;
            if should_click_through != was_click_through.get() {
                // Toggled — re-apply styles.
                if let Ok(hwnd) = hwnd_of(&window) {
                    apply_pencil_styles(hwnd, should_click_through);
                    was_click_through.set(should_click_through);
                }
            }
        },
    );
    timer
}
```

- [ ] **Step 2: Build**

Run: `cargo build`
Expected: clean. Snags:
- `window.window().position()` returns `PhysicalPosition` in 1.15 — has `.x` and `.y` fields as `i32`. If the type is `LogicalPosition`, convert with `.to_physical(scale_factor)` — verify via compile.
- `GetCursorPos(&mut cursor)` — signature in `windows-rs` 0.58 is `GetCursorPos(lppoint: *mut POINT) -> Result<()>`. The `&mut cursor` pattern works (the macro auto-raw-pointers).

- [ ] **Step 3: Commit**

```bash
git add src/ui/pencil_window.rs
git commit -m "feat(ui): pencil proximity click-through toggle at 30Hz"
```

---

## Phase 3 — main.rs Wiring + Click → Hotkey Flow (Day 3)

### Task 5: Wire pencil into `src/main.rs`

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add imports**

Near the other `use crate::...` lines, add:

```rust
use crate::platform::caret::{CaretHookService, FocusEvent};
use crate::ui::pencil_controller::PencilController;
use crate::ui::pencil_window;
```

- [ ] **Step 2: Build the pencil window, start the controller, install proximity timer**

After `bridge::spawn_event_pump(&window, event_rx);` and the history pre-load block, add:

```rust
    // Floating pencil window — Plan 5
    let pencil = pencil_window::build()?;
    let (caret_tx, caret_rx) = mpsc::unbounded_channel::<FocusEvent>();
    let _caret_service = CaretHookService::start(caret_tx)
        .context("start caret hook service")?;
    let _pencil_controller = PencilController::start(&pencil, caret_rx);
    let _pencil_proximity_timer = pencil_window::install_proximity_toggle(&pencil);

    // Pencil click → trigger the same flow as the hotkey.
    {
        let engine = engine.clone();
        let rt_handle = rt_handle.clone();
        pencil.on_clicked(move || {
            let engine = engine.clone();
            rt_handle.spawn(async move {
                crate::engine::hotkey_flow::handle_hotkey(engine).await;
            });
        });
    }
```

**Why store everything in `_name` bindings?** Dropping any of them cancels that subsystem:
- `_caret_service` drop → hook thread stops, channel closes, `PencilController` exits
- `_pencil_controller` drop → UIA worker stops
- `_pencil_proximity_timer` drop → timer stops firing
- `pencil` itself is the Slint window handle — dropping it destroys the Slint window

All four must stay in `main()`'s stack frame for the lifetime of the event loop.

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: clean. Known snags:
- `CaretHookService::start(tx)` — verify exact signature. Plan 1 returns `Result<CaretHookService>` — the `?` propagates the error.
- The pencil window is materialized (then hidden) inside `pencil_window::build()`. When the event loop spins, it becomes visible ONLY when `PencilController` calls `show()`. That's the intended behavior.
- `rt_handle.clone()` — `tokio::runtime::Handle` is `Clone`, that works.
- `pencil.on_clicked(closure)` — the closure must be `'static + Send + Sync`? In Slint 1.15, callbacks are `FnMut` and don't require Send. The closure captures `engine` (Clone) and `rt_handle` — both Send+Sync+Clone.

- [ ] **Step 4: Smoke run**

Run: `cargo run 2>&1 | head -30` (6-8 second timeout, then kill)
Expected:
- Tracing log shows "start caret hook service" and "pencil UIA worker: ..." is not printed (means UIA init succeeded)
- "Quill ready — entering Slint event loop"
- Process keeps running until killed

The pencil window should NOT flash on startup (it's hidden after build). It should only appear once the user focuses an editable text control — which the subagent can't test interactively.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat(main): wire pencil window, caret service, UIA controller"
```

### Task 6: Final sweep + tag `plan-05-floating-pencil-complete`

- [ ] **Step 1: Format + clippy + tests**

```bash
cargo fmt
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --lib
cargo test --test engine_integration
cargo build --release
```

Fix any new clippy lints. The `#[allow(dead_code)]` shield on `src/platform/caret.rs` (added in Plan 1 with comment "wired in Plan 5") and `src/platform/uia.rs` (with comment "caret_bounds and is_editable_text wired in Plan 5") can now come off. Try removing them. If >2 new warnings appear, put them back with a tighter comment.

- [ ] **Step 2: Sanity grep**

```bash
grep -rn "todo!\|unimplemented!" src/ui/ src/engine/ src/state/ src/platform/
grep -rn "noop_emit\|SharedEngine\|tauri::" src/ tests/
```

Both should be empty.

- [ ] **Step 3: Tag**

```bash
git tag plan-05-floating-pencil-complete
git tag -l plan-05*
```

- [ ] **Step 4: Engram save**

Call `mcp__plugin_engram_engram__mem_save`:
- project: `Quill`
- scope: `project`
- topic_key: `quill/slint-rewrite/plan-05`
- type: `architecture`
- title: `Plan 5 (Floating pencil) complete`
- content: Summarize the shape — `src/ui/slint/pencil.slint` + `src/ui/pencil_window.rs` + `src/ui/pencil_controller.rs`; CaretHookService drives a dedicated UIA worker thread (CoInitializeEx on its own apartment); worker consumes raw `FocusEvent`s, enriches via `Uia::is_editable_text` + `Uia::caret_bounds`, posts `PencilCmd`s via `slint::invoke_from_event_loop`; pencil window uses `WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT`; 30 Hz proximity timer on Slint thread toggles `WS_EX_TRANSPARENT` when the cursor is within 40 px; pencil click routes to `engine::hotkey_flow::handle_hotkey` via a closure that captures `engine.clone()` and `rt_handle.clone()`. Remaining Plan 6 polish: fade-in/out animation, hover tooltip "Rewrite with Quill", position smoothing when caret jumps across lines, per-monitor DPI handling, hide-when-main-window-focused.

- [ ] **Step 5: Manual verification checklist** (print to the user)

```
1. `cargo run` — main window appears; pencil does NOT appear yet.
2. Focus Notepad and type a few chars → pencil appears next to the caret.
3. Move the caret with arrow keys → pencil follows (may lag 1-2 frames, that's fine).
4. Alt-Tab to File Explorer → pencil disappears.
5. Alt-Tab back to Notepad → pencil reappears.
6. Move the mouse within 40 px of the pencil → hover tint appears (clicks will register).
7. Click the pencil → capture flow runs, main window appears with the last-typed selection.
8. Move the mouse >40 px away → pencil becomes click-through (cursor passes through to Notepad).
9. Close the main window → process exits cleanly; pencil disappears with it.
```

---

## Self-Review Notes

**Known limitations in Plan 5:**
- No hover tooltip yet (Plan 6)
- No fade-in/out animation — pencil snaps visible/hidden (Plan 6)
- `Uia::caret_bounds` returning `None` (no caret tracked) → pencil hides. Some apps (Chrome, Electron) don't expose a UIA text range for the caret even on editable fields; the pencil will be invisible there, which is acceptable Plan 5 behavior.
- The UIA worker calls `focused_element()` on every event, which means every `CaretMoved` triggers a UIA round-trip. This can be 50-100 µs per call — fine at human typing speed, not at programmatic text insertion speed. Debouncing lands in Plan 6.
- Pencil position does NOT account for multi-monitor DPI scaling differences. UIA bounds are in physical pixels; Slint's `set_position(PhysicalPosition)` takes physical pixels. Should be correct on single-monitor setups. Multi-monitor with mixed DPI scales may place the pencil slightly off — Plan 6 adds DPI-aware translation.

**Likely snags for the implementer:**
1. `slint::PhysicalPosition::new(x: i32, y: i32)` — may be `::new(x as f32, y as f32)` in some Slint versions. If the compiler complains, cast to f32.
2. The `Uia` struct stores an `IUIAutomation` handle — verify it's `!Send` (COM apartment-bound). The UIA worker owns it on a single thread, so Send doesn't matter. But if the type is accidentally marked Send, tests could try to share it across threads.
3. `window.window().position()` — on Slint 1.15 this may require `window.window().as_winit_window().unwrap().inner_position()` as a fallback. Adapt if the simple path fails.
4. `mpsc::UnboundedReceiver::blocking_recv()` — requires that no other async runtime is polling the receiver. Since Plan 5 gives ownership to the std::thread, that's fine.
5. The `_caret_service`, `_pencil_controller`, `_pencil_proximity_timer` bindings MUST stay in `main()`'s frame — clippy may flag them as "unused" without the leading underscore. Use the underscore to silence.
6. `engine::hotkey_flow::handle_hotkey` is `async` — the pencil click closure spawns it via `rt_handle.spawn(async move { ... })`. The closure itself is synchronous (a Slint callback), which is correct — do NOT make it async.

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-04-14-quill-slint-plan-05-floating-pencil.md`.**

Execute subagent-driven, one implementer per phase (same pattern as Plans 2/3/4).
