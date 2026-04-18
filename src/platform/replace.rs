use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::core::clipboard::mute_default;
use crate::platform::{PASTE_GENERATION, SHARED_ENIGO};

/// Paste `text` into the currently focused application.
/// Sets the clipboard to `text`, simulates Ctrl/Cmd+V, then restores the clipboard.
///
/// While this runs, the in-process clipboard monitor is silenced via
/// `mute_default()` so that it doesn't pick up our paste-back as a "new
/// clipboard event" and loop it back to the user. The default window is
/// derived from the monitor's poll interval and covers the full
/// write → paste → 500 ms delayed restore sequence.
///
/// Two pastes in quick succession (< 500 ms) are now race-safe: the
/// first's delayed-restore thread sees PASTE_GENERATION has advanced
/// past its snapshot and skips the restore, leaving the second paste's
/// state intact. Before this, both threads would race to set the
/// clipboard and one might win with stale data.
pub fn paste_text(text: &str) -> Result<(), String> {
    mute_default();

    // Bump the generation BEFORE spawning the restore thread so any
    // still-pending older threads see a newer value and skip their restore.
    let my_gen = PASTE_GENERATION.fetch_add(1, Ordering::AcqRel) + 1;

    let mut clipboard = arboard::Clipboard::new().map_err(|e| format!("Clipboard error: {e}"))?;

    let saved = clipboard.get_text().ok();

    clipboard
        .set_text(text)
        .map_err(|e| format!("Failed to set clipboard: {e}"))?;

    // Small delay to let the clipboard update propagate
    std::thread::sleep(Duration::from_millis(30));

    simulate_paste().map_err(|e| format!("Paste simulation failed: {e}"))?;

    // Restore the previous clipboard after a short delay, but ONLY if:
    //   - our generation is still the latest (no newer paste has fired)
    //   - the clipboard still matches what we pasted (user didn't copy
    //     something new in the meantime)
    //   - we had a non-None `saved` (we never write empty back over a
    //     non-text clipboard like an image).
    let pasted = text.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(500));
        if PASTE_GENERATION.load(Ordering::Acquire) != my_gen {
            // A newer paste has run — leave its state on the clipboard.
            return;
        }
        if let Ok(mut cb) = arboard::Clipboard::new() {
            match (cb.get_text().ok(), saved) {
                (Some(current), Some(prev)) if current == pasted => {
                    let _ = cb.set_text(prev);
                }
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

// ---------------------------------------------------------------------------
// Trait implementation
// ---------------------------------------------------------------------------

use async_trait::async_trait;

use super::traits::TextReplace;

/// Production implementation of `TextReplace`. Wraps the existing `paste_text`
/// function (enigo-based Ctrl+V simulation).
#[derive(Default)]
pub struct Replace;

#[async_trait]
impl TextReplace for Replace {
    async fn paste(&self, text: &str) -> anyhow::Result<()> {
        let text = text.to_owned();
        tokio::task::spawn_blocking(move || paste_text(&text))
            .await
            .map_err(|e| anyhow::anyhow!("paste task join error: {e}"))?
            .map_err(|e| anyhow::anyhow!("{e}"))
    }
}
