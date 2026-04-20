//! WinEvent-based focus & caret tracking.
//!
//! Installs `SetWinEventHook` on a dedicated thread that runs a Windows
//! message pump. Events are classified and forwarded as `FocusEvent`s along
//! a `std::sync::mpsc::Sender`. The caller decides what to do with them —
//! currently the floating pencil indicator controller
//! (`ui::pencil_controller`).
//!
//! Uses `std::sync::mpsc` (not tokio) because the consumer needs
//! `recv_timeout` for periodic selection-state polling, and neither the
//! producer nor the consumer is running inside a tokio runtime.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MsgWaitForMultipleObjectsEx, PeekMessageW, TranslateMessage,
    EVENT_OBJECT_FOCUS, EVENT_SYSTEM_FOREGROUND, MSG, MWMO_INPUTAVAILABLE, PM_REMOVE, QS_ALLINPUT,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_QUIT,
};

use super::traits::ScreenRect;

/// Events produced by the caret hook service and forwarded to consumers.
///
/// The hook emits raw structural focus events (control gained/lost focus).
/// The consumer (`ui::pencil_controller`) enriches these via UIA to decide
/// whether the focused control is editable and where the caret rect is
/// on-screen.
///
/// `editable`, `anchor`, and `app_hint` on `FocusChanged` are populated
/// by the hook but not all are destructured by the current controller —
/// we keep them so future consumers (e.g. a caret-tracker) can enrich
/// without reshaping this enum.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum FocusEvent {
    /// A new control gained focus.
    ///
    /// - `editable`: best-effort hint from the hook; the controller also
    ///   re-verifies via `Uia::is_editable_text` before showing the pencil.
    /// - `anchor`: bounding rect of the focused control, when available.
    /// - `app_hint`: opaque string for diagnostics — currently `"hwnd=0x…"`.
    FocusChanged {
        editable: bool,
        anchor: Option<ScreenRect>,
        app_hint: String,
    },
    /// Focus moved to a control that cannot be interrogated (e.g. an elevated
    /// window that blocks UIA cross-process queries).
    FocusLost,
}

/// Handle to the background WinEvent hook thread.
///
/// Dropping this value calls [`CaretHookService::stop`] automatically,
/// ensuring the hook thread exits cleanly before control returns to the
/// caller.
pub struct CaretHookService {
    stop_flag: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl CaretHookService {
    /// Spawn the hook thread. Events flow on `sender` until `stop()` is
    /// called or the service is dropped.
    ///
    /// The thread installs ONE WinEvent hook:
    /// - `EVENT_SYSTEM_FOREGROUND..EVENT_OBJECT_FOCUS` for focus changes.
    ///
    /// The hook uses `WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS` so
    /// the callback runs on this thread (no DLL injection) and self-events
    /// from Quill are suppressed.
    pub fn start(sender: Sender<FocusEvent>) -> Result<Self> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = Arc::clone(&stop_flag);

        let thread = std::thread::Builder::new()
            .name("quill-caret-hook".into())
            .spawn(move || {
                if let Err(e) = run_hook_thread(sender, stop_flag_clone) {
                    tracing::error!("caret hook thread exited with error: {e:#}");
                }
            })
            .map_err(|e| anyhow!("spawn caret hook thread: {e}"))?;

        Ok(Self {
            stop_flag,
            thread: Some(thread),
        })
    }

    /// Signal the hook thread to stop and wait for it to exit.
    ///
    /// Safe to call multiple times; subsequent calls are no-ops.
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for CaretHookService {
    fn drop(&mut self) {
        self.stop();
    }
}

// ---------------------------------------------------------------------------
// Thread-local sender — allows the `extern "system"` hook proc to reach the
// channel without global mutable state visible across threads.
//
// SAFETY: Only the caret-hook thread writes to and reads from this cell.
// The hook proc is always invoked on the thread that owns the message pump
// (because we use WINEVENT_OUTOFCONTEXT), so there is no concurrent access.
// ---------------------------------------------------------------------------
thread_local! {
    static SENDER: std::cell::RefCell<Option<Sender<FocusEvent>>> =
        const { std::cell::RefCell::new(None) };
}

/// Body of the hook thread.
fn run_hook_thread(sender: Sender<FocusEvent>, stop: Arc<AtomicBool>) -> Result<()> {
    // Install the sender into the thread-local so the hook proc can reach it.
    SENDER.with(|cell| {
        *cell.borrow_mut() = Some(sender);
    });

    unsafe {
        // Foreground + focus events only. We used to also install an
        // `EVENT_OBJECT_LOCATIONCHANGE` hook, but that event fires
        // thousands of times per second during normal use (every window
        // move, cursor blink, tooltip hover, mouse drag). The callback
        // already dropped the event without doing any work, so the net
        // effect was burning CPU thread-context-switching through
        // no-ops. Removing the registration entirely lets the OS stop
        // posting them to us at all. If per-pixel caret tracking is
        // needed later we'll reintroduce a debounced registration.
        let focus_hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_OBJECT_FOCUS,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        if focus_hook.is_invalid() {
            return Err(anyhow!("SetWinEventHook(EVENT_OBJECT_FOCUS) failed"));
        }

        // Message pump — drives WinEvent dispatch (required for OUTOFCONTEXT
        // hooks) and exits when stop_flag is set by the owning thread.
        //
        // MsgWaitForMultipleObjectsEx blocks until either a message arrives
        // on the thread's input queue or the 50 ms timeout expires. This
        // replaces the previous `PeekMessage + 10ms sleep` spin, which
        // burned a visible fraction of a core while idle. The 50 ms
        // ceiling is how often we check `stop_flag`.
        let mut msg = MSG::default();
        loop {
            if stop.load(Ordering::SeqCst) {
                break;
            }
            // Wait up to 50ms for input availability. Returning early is
            // fine — PeekMessageW below drains whatever arrived.
            let _ = MsgWaitForMultipleObjectsEx(None, 50, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
            // Drain any pending messages (may be zero on timeout).
            while PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    let _ = UnhookWinEvent(focus_hook);
                    // Clear TLS sender here since we're bailing early.
                    SENDER.with(|cell| {
                        *cell.borrow_mut() = None;
                    });
                    return Ok(());
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        let _ = UnhookWinEvent(focus_hook);
    }

    // Clear the thread-local so no sender clone leaks after the thread exits.
    SENDER.with(|cell| {
        *cell.borrow_mut() = None;
    });

    Ok(())
}

/// WinEvent hook callback — invoked on the caret-hook thread by the message
/// pump for every event matching the installed hooks.
///
/// # Safety
///
/// `WINEVENT_OUTOFCONTEXT` guarantees this function runs on the thread that
/// called `SetWinEventHook` (the caret-hook thread), so `SENDER` access is
/// safe without any additional locking.
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
                // The downstream pencil controller re-queries UIA for the
                // current focused element + caret bounds, so we don't need
                // to carry rect/editability here — caret.rs is intentionally
                // a thin "something changed" signal.
                let _ = tx.send(FocusEvent::FocusChanged {
                    editable: false,
                    anchor: None,
                    app_hint: format!("hwnd={hwnd:?}"),
                });
            }
            // Any other event is noise — the LOCATIONCHANGE registration
            // was removed (see `run_hook_thread` above) so this arm only
            // fires if future hooks are added and should ignore events
            // they don't know about.
            _ => {}
        }
    });
}
