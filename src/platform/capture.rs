use std::time::Duration;

use async_trait::async_trait;

use super::traits::{CaptureResult, CaptureSource, ScreenRect, TextCapture};
use crate::core::clipboard::mute_default;
use crate::platform::SHARED_ENIGO;

/// Maximum time to wait for the user's physical hotkey modifiers to be
/// released before we go ahead and inject the capture `Ctrl+C` anyway.
///
/// Typical human release latency from press-to-release on a single tap is
/// 40–100 ms, so 300 ms is a comfortable ceiling that still lets us move on
/// in the rare case where someone is deliberately holding the hotkey down.
#[cfg(target_os = "windows")]
const MODIFIER_WAIT_TIMEOUT: Duration = Duration::from_millis(300);

/// Poll interval while waiting for modifier release.
#[cfg(target_os = "windows")]
const MODIFIER_POLL_INTERVAL: Duration = Duration::from_millis(5);

/// Primary capture entry point.
///
/// Strategy:
///   1. Wait for the user's hotkey modifiers to physically release.
///   2. Try UIA first — zero side-effects, also yields the selection rectangle.
///   3. Fall back to the clipboard hack (save → Ctrl+C → read → restore) when
///      UIA returns nothing (element doesn't support Text pattern, or focus is
///      in a non-UIA control).
pub fn capture_selection_blocking() -> CaptureResult {
    // Step 1: wait for Ctrl/Shift/Alt modifiers to release before any
    // injection. See `wait_for_hotkey_modifiers_released` for details.
    wait_for_hotkey_modifiers_released();

    // Step 2: try UIA — no clipboard side effects.
    if let Ok((Some(text), anchor_opt)) = uia_capture() {
        return CaptureResult {
            text,
            anchor: anchor_opt,
            source: CaptureSource::Uia,
        };
    }

    // Step 3: clipboard fallback.
    if let Some(text) = clipboard_capture_fallback() {
        if !text.is_empty() {
            return CaptureResult {
                text,
                anchor: None,
                source: CaptureSource::Clipboard,
            };
        }
    }

    CaptureResult::default()
}

/// Attempt to read the selected text (and its bounding rect) via UI Automation.
///
/// Returns `Ok((None, None))` when the focused element doesn't support the
/// Text pattern or nothing is selected — NOT an error.  Returns `Err` only on
/// COM initialisation or fatal UIA failures.
fn uia_capture() -> anyhow::Result<(Option<String>, Option<ScreenRect>)> {
    use super::uia::Uia;
    Uia::with(|uia| {
        let text = uia.selected_text().ok().flatten();
        let anchor = uia.selection_bounds().ok().flatten();
        (text, anchor)
    })
}

/// Clipboard-hack fallback.
///
/// Strategy:
///   1. **Mute the clipboard monitor** for the duration of the capture — we're
///      about to write twice to the system clipboard (simulate-copy then
///      restore), and we don't want the monitor polling thread to observe
///      either transition and fire a spurious `clipboard_change` toast.
///   2. Save the current *text* clipboard (may be `None` for images/files/empty)
///   3. Simulate Ctrl/Cmd+C to ask the focused app to copy its selection
///   4. Wait briefly for the copy to propagate
///   5. Read the post-copy clipboard
///   6. Restore the previous text clipboard (only if we had one — never write
///      an empty string back, which would destroy non-text clipboard content)
///   7. Return the captured text only if it actually differs from what was in
///      the clipboard beforehand — this prevents reporting a stale clipboard as
///      if it were a fresh selection when nothing was actually selected.
///
/// NOTE: `wait_for_hotkey_modifiers_released` is NOT called here; it is called
/// once at the top of `capture_selection_blocking` before either path runs.
fn clipboard_capture_fallback() -> Option<String> {
    tracing::debug!("clipboard_capture_fallback: starting");
    // Silence the clipboard monitor for the whole capture window.
    // `DEFAULT_MUTE_MS` = 2 × poll interval + 200 ms headroom, so the
    // monitor can never observe the capture's own transient clipboard state.
    mute_default();

    let mut clipboard = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("clipboard_capture_fallback: arboard init failed: {e}");
            return None;
        }
    };

    // Capture the existing text clipboard state.
    let saved = clipboard.get_text().ok();
    tracing::debug!(
        saved_len = saved.as_ref().map(|s| s.len()).unwrap_or(0),
        "clipboard_capture_fallback: saved previous clipboard"
    );

    // Trigger a copy and give the target app a moment to respond.
    simulate_copy();
    std::thread::sleep(Duration::from_millis(80));

    // Read the (possibly updated) clipboard.
    let selected_raw = clipboard.get_text().ok();
    tracing::debug!(
        selected_len = selected_raw.as_ref().map(|s| s.len()).unwrap_or(0),
        "clipboard_capture_fallback: post-copy clipboard read"
    );

    // Restore the previous text clipboard. CRITICAL paths:
    //
    //   1. If `saved` was `None` (clipboard was empty or held non-text
    //      content like an image), we must NOT write an empty string —
    //      that would silently destroy the user's clipboard image/file.
    //
    //   2. Only restore if the clipboard STILL matches the text we just
    //      wrote via simulate_copy. Between the copy and this check, an
    //      external clipboard watcher (or a very fast user re-copy) could
    //      have replaced the clipboard content. If we blindly overwrote
    //      with `prev`, we'd destroy that newer content. We re-read here
    //      and only write our rollback when the current content is STILL
    //      our own post-Ctrl+C snapshot.
    if let Some(ref prev) = saved {
        let recent = clipboard.get_text().ok();
        match (&recent, &selected_raw) {
            (Some(now), Some(ours)) if now == ours => {
                let _ = clipboard.set_text(prev.clone());
            }
            _ => {
                tracing::debug!(
                    "clipboard_capture_fallback: skipping restore — \
                     clipboard was modified externally after our copy"
                );
            }
        }
    }

    // Reject the capture if it's empty OR identical to the pre-existing
    // text clipboard (meaning Ctrl+C had no effect — nothing was selected).
    let selected = match selected_raw {
        Some(s) => s,
        None => {
            tracing::debug!(
                "clipboard_capture_fallback: post-copy clipboard was None (non-text or empty)"
            );
            return None;
        }
    };
    if let Some(prev) = saved.as_deref() {
        if selected == prev {
            tracing::debug!(
                "clipboard_capture_fallback: rejected — copy didn't change clipboard (Ctrl+C had no effect)"
            );
            return None;
        }
    }
    let text = selected.trim().to_string();
    if text.is_empty() {
        tracing::debug!("clipboard_capture_fallback: trimmed to empty");
        None
    } else {
        tracing::debug!(len = text.len(), "clipboard_capture_fallback: success");
        Some(text)
    }
}

/// Block until the user has physically released `Ctrl`, `Shift`, and `Alt`,
/// or until `MODIFIER_WAIT_TIMEOUT` elapses.
///
/// On Windows this uses `GetAsyncKeyState`, which reads the real physical
/// key state — unaffected by synthetic `SendInput` releases from enigo or
/// other injectors. On other platforms we fall back to a fixed short sleep
/// because macOS/Linux don't exhibit the same enigo modifier-desync failure
/// mode (their injection paths handle held real-user modifiers differently).
#[cfg(target_os = "windows")]
fn wait_for_hotkey_modifiers_released() {
    use std::time::Instant;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_CONTROL, VK_MENU, VK_SHIFT,
    };
    let deadline = Instant::now() + MODIFIER_WAIT_TIMEOUT;
    loop {
        // `GetAsyncKeyState` returns a SHORT whose high bit (0x8000) is set
        // while the key is currently physically pressed. We only care about
        // the high bit — the low bit ("pressed since last call") is irrelevant
        // for a polling loop.
        let held = unsafe {
            (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0
                || (GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0
                || (GetAsyncKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0
        };
        if !held {
            return;
        }
        if Instant::now() >= deadline {
            return;
        }
        std::thread::sleep(MODIFIER_POLL_INTERVAL);
    }
}

#[cfg(not(target_os = "windows"))]
fn wait_for_hotkey_modifiers_released() {
    // Give the user a brief window to release the hotkey before we inject.
    // On macOS/Linux the enigo-vs-held-modifier desync hasn't been observed
    // in practice — the platform injection paths handle it — but a small
    // pause still helps avoid edge-case key-state flaps.
    std::thread::sleep(Duration::from_millis(30));
}

#[cfg(target_os = "macos")]
fn simulate_copy() {
    use enigo::{Direction, Key, Keyboard};
    if let Ok(mut guard) = SHARED_ENIGO.lock() {
        if let Some(e) = guard.as_mut() {
            let _ = e.key(Key::Meta, Direction::Press);
            let _ = e.key(Key::Unicode('c'), Direction::Click);
            let _ = e.key(Key::Meta, Direction::Release);
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn simulate_copy() {
    use enigo::{Direction, Key, Keyboard};
    if let Ok(mut guard) = SHARED_ENIGO.lock() {
        if let Some(e) = guard.as_mut() {
            let _ = e.key(Key::Control, Direction::Press);
            let _ = e.key(Key::Unicode('c'), Direction::Click);
            let _ = e.key(Key::Control, Direction::Release);
        }
    }
}

/// Production implementation of `TextCapture`. Dispatches the synchronous
/// `capture_selection_blocking` onto a blocking worker thread so the async
/// engine never stalls the tokio runtime.
#[derive(Default)]
pub struct Capture;

#[async_trait]
impl TextCapture for Capture {
    async fn capture(&self) -> CaptureResult {
        tokio::task::spawn_blocking(capture_selection_blocking)
            .await
            .unwrap_or_default()
    }
}
