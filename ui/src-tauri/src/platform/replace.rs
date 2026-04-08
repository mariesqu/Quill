use std::time::Duration;

use crate::core::clipboard::mute_default;
use crate::platform::SHARED_ENIGO;

/// Paste `text` into the currently focused application.
/// Sets the clipboard to `text`, simulates Ctrl/Cmd+V, then restores the clipboard.
///
/// While this runs, the in-process clipboard monitor is silenced via
/// `mute_default()` so that it doesn't pick up our paste-back as a "new
/// clipboard event" and loop it back to the user. The default window is
/// derived from the monitor's poll interval and covers the full
/// write → paste → 500 ms delayed restore sequence.
pub fn paste_text(text: &str) -> Result<(), String> {
    mute_default();

    let mut clipboard = arboard::Clipboard::new().map_err(|e| format!("Clipboard error: {e}"))?;

    let saved = clipboard.get_text().ok();

    clipboard
        .set_text(text)
        .map_err(|e| format!("Failed to set clipboard: {e}"))?;

    // Small delay to let the clipboard update propagate
    std::thread::sleep(Duration::from_millis(30));

    simulate_paste().map_err(|e| format!("Paste simulation failed: {e}"))?;

    // Restore the previous clipboard after a short delay, but ONLY if it
    // hasn't been replaced by something else in the meantime (e.g. the user
    // copied something new during the 500 ms window). We snapshot what we
    // just wrote so we can detect "unchanged since our paste" reliably.
    let pasted = text.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(500));
        if let Ok(mut cb) = arboard::Clipboard::new() {
            match (cb.get_text().ok(), saved) {
                (Some(current), Some(prev)) if current == pasted => {
                    let _ = cb.set_text(prev);
                }
                // If the current clipboard differs from what we pasted, the
                // user copied something else — leave it alone. If `saved` was
                // None (non-text clipboard before paste), also leave it alone.
                _ => {}
            }
        }
    });

    Ok(())
}

#[cfg(target_os = "macos")]
fn simulate_paste() -> Result<(), String> {
    use enigo::{Direction, Key, Keyboard};
    let mut guard = SHARED_ENIGO
        .lock()
        .map_err(|e| format!("Enigo mutex poisoned: {e}"))?;
    let e = guard
        .as_mut()
        .ok_or_else(|| "Enigo not available on this platform".to_string())?;
    e.key(Key::Meta, Direction::Press)
        .map_err(|e| e.to_string())?;
    e.key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| e.to_string())?;
    e.key(Key::Meta, Direction::Release)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn simulate_paste() -> Result<(), String> {
    use enigo::{Direction, Key, Keyboard};
    let mut guard = SHARED_ENIGO
        .lock()
        .map_err(|e| format!("Enigo mutex poisoned: {e}"))?;
    let e = guard
        .as_mut()
        .ok_or_else(|| "Enigo not available on this platform".to_string())?;
    e.key(Key::Control, Direction::Press)
        .map_err(|e| e.to_string())?;
    e.key(Key::Unicode('v'), Direction::Click)
        .map_err(|e| e.to_string())?;
    e.key(Key::Control, Direction::Release)
        .map_err(|e| e.to_string())?;
    Ok(())
}
