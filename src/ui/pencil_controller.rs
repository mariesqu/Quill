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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use slint::ComponentHandle;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::platform::caret::FocusEvent;
use crate::platform::uia::Uia;
use crate::ui::{pencil_window, PencilWindow};

/// Thread-local flag: have we applied Win32 styles to the pencil window yet?
/// Styles must be applied AFTER the event loop creates the native HWND (i.e.,
/// after the first `show()`). We do it once on the Slint thread.
use std::cell::Cell;
thread_local! {
    static STYLES_APPLIED: Cell<bool> = const { Cell::new(false) };
}

/// Command posted to the Slint thread to update the pencil window.
#[derive(Debug, Clone, Copy)]
enum PencilCmd {
    ShowAt { x: i32, y: i32 },
    Hide,
}

/// Horizontal offset from the focused element's right edge to the pencil's
/// left edge, in physical pixels. This is the element's bounding rect
/// (from UIA's `CurrentBoundingRectangle`) — NOT the caret position. A
/// true caret-relative offset would require `TextRange::GetBoundingRectangles`
/// on the selection / insertion point, which the current flow doesn't do.
const ELEMENT_EDGE_OFFSET_X: i32 = 24;

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
                while !stop_flag_clone.load(Ordering::Acquire) {
                    // Use `blocking_recv` since this is a dedicated std::thread
                    // (not a tokio task). `None` means the sender was dropped —
                    // caret service is shutting down; exit.
                    let Some(event) = rx.blocking_recv() else {
                        break;
                    };

                    // `Uia::with` lazily initialises COM + the IUIAutomation
                    // singleton on this thread's first call.
                    let cmd = Uia::with(|uia| handle_focus_event(uia, event))
                        .ok()
                        .flatten();

                    if let Some(cmd) = cmd {
                        let weak = weak.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            let Some(window) = weak.upgrade() else {
                                return;
                            };
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
    tracing::debug!(?event, "pencil_controller: focus event");
    match event {
        FocusEvent::FocusChanged { .. } => {
            // Combined lookup — one `focused_element` COM call, not three.
            match uia.editable_element_bounds() {
                Ok(Some((rect, true))) => {
                    tracing::debug!(
                        left = rect.left,
                        top = rect.top,
                        right = rect.right,
                        bottom = rect.bottom,
                        "editable element_bounds hit"
                    );
                    Some(PencilCmd::ShowAt {
                        x: rect.right + ELEMENT_EDGE_OFFSET_X,
                        y: rect.top,
                    })
                }
                Ok(Some((_, false))) => Some(PencilCmd::Hide),
                Ok(None) => {
                    tracing::debug!("editable_element_bounds: no focused element");
                    None
                }
                Err(e) => {
                    tracing::debug!("editable_element_bounds failed: {e:#}");
                    Some(PencilCmd::Hide)
                }
            }
        }
        FocusEvent::FocusLost => Some(PencilCmd::Hide),
    }
}

fn apply_cmd(window: &PencilWindow, cmd: PencilCmd) {
    match cmd {
        PencilCmd::ShowAt { x, y } => {
            pencil_window::set_position(window, x, y);
            // Try to apply Win32 extended styles BEFORE the first show() so
            // the pencil never paints a frame with default styles (which
            // briefly include foreground-stealing behaviour). hwnd_of fails
            // before winit has materialised the HWND — on that first call
            // we fall through to the after-show path below.
            let mut applied_pre_show = false;
            STYLES_APPLIED.with(|applied| {
                if !applied.get() {
                    if let Ok(hwnd) = pencil_window::hwnd_of(window) {
                        pencil_window::apply_pencil_styles(hwnd, true);
                        applied.set(true);
                        applied_pre_show = true;
                    }
                }
            });
            // Ensure the native window is visible so the opacity fade is seen.
            let _ = window.show();
            // Fallback: first ever show — the HWND wasn't ready above, so
            // apply styles now that show() has materialised it. Subsequent
            // shows skip this branch because STYLES_APPLIED is latched.
            if !applied_pre_show {
                STYLES_APPLIED.with(|applied| {
                    if !applied.get() {
                        if let Ok(hwnd) = pencil_window::hwnd_of(window) {
                            pencil_window::apply_pencil_styles(hwnd, true);
                            applied.set(true);
                        }
                    }
                });
            }
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
