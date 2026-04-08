use std::time::Duration;

use crate::core::clipboard::mute_default;
use crate::platform::SHARED_ENIGO;

/// Capture currently selected text.
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
pub fn get_selected_text() -> Option<String> {
    // Step 1: silence the clipboard monitor for the whole capture window.
    // `DEFAULT_MUTE_MS` = 2 × poll interval + 200 ms headroom, so the
    // monitor can never observe the capture's own transient clipboard state.
    mute_default();

    let mut clipboard = arboard::Clipboard::new().ok()?;

    // Step 2: capture the existing text clipboard state.
    let saved = clipboard.get_text().ok();

    // Step 3-4: trigger a copy and give the target app a moment to respond.
    simulate_copy();
    std::thread::sleep(Duration::from_millis(80));

    // Step 5: read the (possibly updated) clipboard.
    let selected_raw = clipboard.get_text().ok();

    // Step 6: restore the previous text clipboard. CRITICAL: if `saved` was
    // `None` (clipboard was empty or held non-text content like an image),
    // we must NOT write an empty string — that would silently destroy the
    // user's clipboard image/file.
    if let Some(ref prev) = saved {
        let _ = clipboard.set_text(prev.clone());
    }

    // Step 7: reject the capture if it's empty OR identical to the pre-existing
    // text clipboard (meaning Ctrl+C had no effect — nothing was selected).
    let selected = selected_raw?;
    if let Some(prev) = saved.as_deref() {
        if selected == prev {
            return None;
        }
    }
    let text = selected.trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
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
