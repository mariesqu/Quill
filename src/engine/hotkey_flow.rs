//! Hotkey entry point: capture selected text + active app context,
//! then emit `UiEvent::ShowOverlay` for the UI to render.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::core::prompt::suggest_mode;
use crate::state::{FocusSnapshot, Suggestion, UiEvent};

use super::Engine;

/// Reentrancy guard for `handle_hotkey`. Rationale: repeated hotkey
/// presses each inject a `Ctrl` release via enigo during capture. Without
/// this guard, pressing the hotkey twice in quick succession clears the
/// real modifier state mid-capture and lets the next physical keystroke
/// land without Ctrl — observed as lost input on fast retriggers.
static HOTKEY_BUSY: AtomicBool = AtomicBool::new(false);

struct BusyGuard;
impl Drop for BusyGuard {
    fn drop(&mut self) {
        HOTKEY_BUSY.store(false, Ordering::Release);
    }
}

/// Reset the reentrancy guard.
///
/// **Test-only**: allows integration tests running in the same process to
/// call `handle_hotkey` multiple times without the static `HOTKEY_BUSY`
/// preventing subsequent calls. Do not call this from production code.
///
/// This stays `pub fn` (not gated on `#[cfg(test)]`) because Quill's
/// integration tests live under `tests/` and Cargo compiles each test
/// file as a separate crate — `#[cfg(test)]` in the library crate does
/// not propagate to them. The function is named `_for_test` so grep /
/// code review catches any production call.
///
/// `#[allow(dead_code)]` is required because the binary crate never calls
/// this — only `tests/engine_integration.rs` does, and that's compiled as
/// a separate crate so the lib's `cargo check` sees it as unused.
#[allow(dead_code)]
pub fn reset_busy_for_test() {
    HOTKEY_BUSY.store(false, Ordering::Release);
}

pub async fn handle_hotkey(engine: Engine) {
    handle_hotkey_with_focus(engine, None).await
}

/// Variant of `handle_hotkey` that accepts a pre-captured foreground
/// HWND snapshot. Used by the pencil click path — the click-handler runs
/// on the Slint main thread where the foreground is the target app (the
/// pencil window itself never takes foreground because `WS_EX_NOACTIVATE`
/// is applied). Capturing there and passing the snapshot in avoids a race
/// where `GetForegroundWindow` inside `handle_hotkey` might observe the
/// overlay mid-show.
pub async fn handle_hotkey_with_focus(engine: Engine, pre_captured_focus: Option<FocusSnapshot>) {
    // Reject reentrant invocations silently.
    if HOTKEY_BUSY
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    let _busy_guard = BusyGuard;

    // Snapshot the foreground HWND FIRST (if the caller didn't already).
    // This must happen before capture simulates Ctrl+C (which briefly
    // changes focus) and definitely before the overlay is shown (which
    // steals foreground). The HWND is what `ConfirmReplace` will restore
    // before simulating Ctrl+V so the paste lands in the user's original
    // app (Teams, Outlook, …).
    let focus_snapshot = match pre_captured_focus {
        Some(snap) => Some(snap),
        None => tokio::task::spawn_blocking(|| {
            use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
            let hwnd = unsafe { GetForegroundWindow() };
            FocusSnapshot {
                hwnd_raw: hwnd.0 as isize,
            }
        })
        .await
        .ok(),
    };

    // Run capture (already async) and active-app context probe concurrently.
    // ContextProbe::active_context is synchronous, so wrap it in spawn_blocking
    // to keep the tokio worker pool free during blocking FFI.
    let capture_fut = engine.capture().capture();
    let ctx_probe = engine.context().clone();
    let context_fut = tokio::task::spawn_blocking(move || ctx_probe.active_context());
    let (capture, ctx_res) = tokio::join!(capture_fut, context_fut);
    let context = ctx_res.unwrap_or_default();
    let text = capture.text;
    // NOTE: we log `captured_len` but NOT the text content itself — the log
    // file at ~/.quill/quill.log is often shared for support, and the
    // captured text can be sensitive (email drafts, private messages).
    tracing::debug!(
        captured_len = text.len(),
        source = ?capture.source,
        app = %context.app,
        "handle_hotkey: capture complete"
    );

    // Resolve an anchor rectangle for near-caret overlay positioning.
    // Priority:
    //   1. `capture.anchor` — the UIA selection rect, computed during
    //      capture when text was actually selected. This is the TIGHTEST
    //      anchor (the selection itself), not the whole text element.
    //   2. `Uia::element_bounds()` — the focused element's rect, used when
    //      the user triggered the hotkey without a selection (free-text
    //      flow). Still places the overlay over the user's input area.
    //   3. `None` — the bridge centers the overlay on the primary screen.
    //
    // Second-UIA query runs on a blocking thread; UIA calls are sync COM.
    let anchor_rect = if let Some(a) = capture.anchor {
        Some(a)
    } else {
        tokio::task::spawn_blocking(|| {
            crate::platform::uia::Uia::with(|uia| uia.element_bounds().ok().flatten())
                .ok()
                .flatten()
        })
        .await
        .ok()
        .flatten()
    };

    // Reset per-session state BEFORE mutating with new data.
    //
    // `reset_session()` wipes per-capture scratch (selected_text, last_result,
    // stream_buffer, streaming/done flags). The authoritative language for
    // the next run lives in the UI's AppBridge.active-language picker and
    // is threaded through every ExecuteMode — AppState no longer carries it.
    {
        let mut s = engine.state().lock().unwrap();
        s.reset_session();
        s.selected_text = text.clone();
        s.last_app_hint = context.hint.clone();
        s.focus_target = focus_snapshot;
    }

    let suggestion = if text.trim().is_empty() {
        None
    } else {
        let mode_id = suggest_mode(&text, &context);
        Some(Suggestion { mode_id })
    };

    engine.emit(UiEvent::ShowOverlay {
        text,
        context,
        suggestion,
        anchor_rect,
    });
}
