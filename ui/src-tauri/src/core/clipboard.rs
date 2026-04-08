use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

const POLL_INTERVAL_MS: u64 = 500;
const MIN_WORDS: usize = 3;

/// Spawns a background clipboard monitor task.
/// Sends newly-copied text to the returned receiver.
/// `enabled` is checked each cycle — setting it to false stops emission without dropping the task.
pub fn start_clipboard_monitor(
    enabled: Arc<AtomicBool>,
) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel::<String>(32);

    std::thread::spawn(move || {
        let Ok(mut clipboard) = arboard::Clipboard::new() else { return };
        let mut last_text = clipboard.get_text().unwrap_or_default();

        loop {
            std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            if !enabled.load(Ordering::Relaxed) { continue; }

            let Ok(current) = clipboard.get_text() else { continue };
            if current == last_text { continue; }
            last_text = current.clone();

            let word_count = current.split_whitespace().count();
            if word_count >= MIN_WORDS {
                let _ = tx.blocking_send(current);
            }
        }
    });

    rx
}
