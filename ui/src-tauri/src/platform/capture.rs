use std::time::Duration;

/// Capture currently selected text.
/// Strategy: save clipboard → simulate Copy → read clipboard → restore.
/// Returns None if no text was selected or capture failed.
pub fn get_selected_text() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;

    // Save current clipboard
    let saved = clipboard.get_text().ok();

    // Simulate Ctrl+C / Cmd+C
    simulate_copy();
    std::thread::sleep(Duration::from_millis(80));

    // Read new clipboard content
    let selected = clipboard.get_text().ok();

    // Restore previous clipboard content
    if let Some(prev) = saved {
        let _ = clipboard.set_text(prev);
    } else {
        // Clear clipboard (set to empty string as closest equivalent)
        let _ = clipboard.set_text(String::new());
    }

    // Return only if we got something different (and non-empty)
    let text = selected?.trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

#[cfg(target_os = "macos")]
fn simulate_copy() {
    use enigo::{Enigo, Key, Keyboard, Settings, Direction};
    if let Ok(mut e) = Enigo::new(&Settings::default()) {
        let _ = e.key(Key::Meta, Direction::Press);
        let _ = e.key(Key::Unicode('c'), Direction::Click);
        let _ = e.key(Key::Meta, Direction::Release);
    }
}

#[cfg(not(target_os = "macos"))]
fn simulate_copy() {
    use enigo::{Enigo, Key, Keyboard, Settings, Direction};
    if let Ok(mut e) = Enigo::new(&Settings::default()) {
        let _ = e.key(Key::Control, Direction::Press);
        let _ = e.key(Key::Unicode('c'), Direction::Click);
        let _ = e.key(Key::Control, Direction::Release);
    }
}
