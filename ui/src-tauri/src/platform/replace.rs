use std::time::Duration;

/// Paste `text` into the currently focused application.
/// Sets the clipboard to `text`, simulates Ctrl/Cmd+V, then restores the clipboard.
pub fn paste_text(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new()
        .map_err(|e| format!("Clipboard error: {e}"))?;

    let saved = clipboard.get_text().ok();

    clipboard.set_text(text)
        .map_err(|e| format!("Failed to set clipboard: {e}"))?;

    // Small delay to let the clipboard update propagate
    std::thread::sleep(Duration::from_millis(30));

    simulate_paste().map_err(|e| format!("Paste simulation failed: {e}"))?;

    // Restore previous clipboard after a short delay
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(500));
        if let Ok(mut cb) = arboard::Clipboard::new() {
            if let Some(prev) = saved {
                let _ = cb.set_text(prev);
            }
        }
    });

    Ok(())
}

#[cfg(target_os = "macos")]
fn simulate_paste() -> Result<(), String> {
    use enigo::{Enigo, Key, Keyboard, Settings, Direction};
    let mut e = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    e.key(Key::Meta, Direction::Press).map_err(|e| e.to_string())?;
    e.key(Key::Unicode('v'), Direction::Click).map_err(|e| e.to_string())?;
    e.key(Key::Meta, Direction::Release).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn simulate_paste() -> Result<(), String> {
    use enigo::{Enigo, Key, Keyboard, Settings, Direction};
    let mut e = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    e.key(Key::Control, Direction::Press).map_err(|e| e.to_string())?;
    e.key(Key::Unicode('v'), Direction::Click).map_err(|e| e.to_string())?;
    e.key(Key::Control, Direction::Release).map_err(|e| e.to_string())?;
    Ok(())
}
