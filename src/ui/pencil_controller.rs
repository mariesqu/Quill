//! UIA worker thread that enriches raw `caret::FocusEvent`s with editability
//! + caret bounds, and posts `PencilCmd`s onto the Slint event loop.
//!
//! Running UIA calls in a dedicated worker avoids:
//! - COM apartment pitfalls inside the WinEvent hook callback
//! - Re-entrancy when UIA internally posts its own events
//! - Blocking the tokio worker pool on synchronous COM calls
//!
//! The worker owns a single `Uia` instance constructed on its thread via
//! `Uia::with` (a thread-local singleton — `Uia::new()` is called lazily on
//! first access, which calls `CoInitializeEx` on that OS thread).
//!
//! The worker doubles as a selection-state poller. Because Windows does
//! NOT emit focus events when the selection inside an already-focused
//! editable control collapses (e.g. user presses Esc or clicks to clear
//! a selection), we wake up every `POLL_INTERVAL` to re-query UIA and
//! hide the pencil when the selection vanishes (or surface it when the
//! user makes a new selection inside the same control).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use slint::ComponentHandle;

use crate::platform::caret::FocusEvent;
use crate::platform::uia::Uia;
use crate::ui::{pencil_window, PencilWindow};

/// How often the worker wakes up to re-query UIA selection state when no
/// focus event has arrived. 120ms is fast enough that making or clearing
/// a selection feels instantaneous, cheap enough that the poll cost is
/// negligible (one cross-process COM call per tick).
const POLL_INTERVAL: Duration = Duration::from_millis(120);

/// Thread-local flag: have we applied Win32 styles to the pencil window yet?
/// Styles must be applied AFTER the event loop creates the native HWND (i.e.,
/// after the first `show()`). We do it once on the Slint thread.
use std::cell::Cell;
thread_local! {
    static STYLES_APPLIED: Cell<bool> = const { Cell::new(false) };
}

/// Command posted to the Slint thread to update the pencil window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PencilCmd {
    ShowAt { x: i32, y: i32 },
    Hide,
}

/// Horizontal offset from the selection's right edge to the pencil's left
/// edge, in physical pixels. Kept small so the pencil tracks close to the
/// caret and doesn't visually disconnect from the selection.
const CARET_EDGE_OFFSET_X: i32 = 8;

/// Nominal pencil window height in physical pixels (matches `pencil.slint`
/// at 1.0 DPI — not DPI-aware, but good enough for rough vertical centering
/// against the caret rect).
const PENCIL_HEIGHT_PX: i32 = 32;

/// Handle to the background worker thread.
pub struct PencilController {
    stop_flag: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl PencilController {
    /// Spawn the worker thread. It drains `rx` (raw `FocusEvent`s from the
    /// caret hook) AND polls UIA every `POLL_INTERVAL` so selection changes
    /// inside the already-focused control (which don't emit any Windows
    /// event) are detected. Commands are projected onto `window` via
    /// `slint::invoke_from_event_loop`.
    pub fn start(window: &PencilWindow, rx: Receiver<FocusEvent>) -> Self {
        let weak = window.as_weak();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = Arc::clone(&stop_flag);

        let thread = thread::Builder::new()
            .name("quill-pencil-uia".into())
            .spawn(move || {
                // Tracks the last command we sent to the Slint thread so we
                // can deduplicate the polling stream (otherwise we'd spam
                // identical ShowAt at 120ms cadence).
                let mut last_cmd: Option<PencilCmd> = None;

                loop {
                    if stop_flag_clone.load(Ordering::Acquire) {
                        break;
                    }

                    // Either a focus event arrived, or the timer elapsed —
                    // both are reasons to re-query UIA state.
                    let got_event = match rx.recv_timeout(POLL_INTERVAL) {
                        Ok(_event) => true,
                        Err(RecvTimeoutError::Timeout) => false,
                        Err(RecvTimeoutError::Disconnected) => break,
                    };

                    // Uia::with lazily initialises COM + IUIAutomation on
                    // this thread's first call.
                    let cmd = Uia::with(|uia| query_pencil_state(uia, got_event))
                        .ok()
                        .flatten();

                    // Dedup: only cross the thread boundary when the command
                    // actually changes. Slint windows are cheap to update,
                    // but `invoke_from_event_loop` allocates + queues a
                    // closure on every call.
                    if let Some(cmd) = cmd {
                        if last_cmd != Some(cmd) {
                            last_cmd = Some(cmd);
                            let weak = weak.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                let Some(window) = weak.upgrade() else {
                                    return;
                                };
                                apply_cmd(&window, cmd);
                            });
                        }
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

/// Query the current pencil state from UIA and translate it into a command.
///
/// Option B semantics: the pencil is visible ONLY when the focused control
/// is an editable text control AND the user has a non-empty selection
/// (`is_caret` is `true`, meaning `editable_caret_or_element_bounds`
/// returned a real caret/selection rect — not the fallback element rect).
///
/// `triggered_by_event` is purely for logging — the logic is identical
/// regardless of whether this call was woken by a focus event or by the
/// periodic poll.
fn query_pencil_state(uia: &Uia, triggered_by_event: bool) -> Option<PencilCmd> {
    match uia.editable_caret_or_element_bounds() {
        Ok(Some((rect, true, true))) => {
            // Editable + non-empty selection → anchor next to the caret.
            let caret_height = (rect.bottom - rect.top).max(1);
            let vertical_offset = (PENCIL_HEIGHT_PX - caret_height) / 2;
            let x = rect.right + CARET_EDGE_OFFSET_X;
            let y = rect.top - vertical_offset.max(0);
            tracing::debug!(
                triggered_by_event,
                left = rect.left,
                top = rect.top,
                right = rect.right,
                bottom = rect.bottom,
                x,
                y,
                "pencil: show at caret"
            );
            Some(PencilCmd::ShowAt { x, y })
        }
        Ok(Some((_, true, false))) => {
            // Editable but no selection (caret rect unavailable) — hide.
            // This is the "user deselected text" case.
            Some(PencilCmd::Hide)
        }
        Ok(Some((_, false, _))) => {
            // Focus moved to a non-editable control.
            Some(PencilCmd::Hide)
        }
        Ok(None) => {
            // No focused element at all.
            Some(PencilCmd::Hide)
        }
        Err(e) => {
            tracing::debug!(
                triggered_by_event,
                "editable_caret_or_element_bounds failed: {e:#}"
            );
            Some(PencilCmd::Hide)
        }
    }
}

fn apply_cmd(window: &PencilWindow, cmd: PencilCmd) {
    match cmd {
        PencilCmd::ShowAt { x, y } => {
            // Materialise the native HWND first. Slint's winit backend creates
            // the window lazily on the first `show()` call. Before that,
            // `Window::set_position()` is a silent no-op — which is why
            // positioning then showing (the old order) left the pencil at
            // Windows' default cascade position on the first ever focus event.
            let _ = window.show();

            // Now that the HWND exists, apply Win32 extended styles (layered,
            // tool window, no-activate, click-through). Latched so we only
            // do it once per process.
            STYLES_APPLIED.with(|applied| {
                if !applied.get() {
                    if let Ok(hwnd) = pencil_window::hwnd_of(window) {
                        pencil_window::apply_pencil_styles(hwnd, true);
                        applied.set(true);
                    }
                }
            });

            // Position AFTER show() — only works once the winit window exists.
            pencil_window::set_position(window, x, y);

            tracing::debug!(x, y, "pencil ShowAt: positioned after show()");

            window.set_is_visible(true);
        }
        PencilCmd::Hide => {
            window.set_is_visible(false);
            // Keep the native window visible so the fade-out animation runs;
            // opacity: 0.0 makes it visually absent while the native window
            // stays alive and continues to participate in the click-through
            // logic installed by `pencil_window::install_proximity_toggle`.
        }
    }
}
