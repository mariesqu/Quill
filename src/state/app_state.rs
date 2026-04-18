//! Single source of truth for runtime app state.
//!
//! All state lives here under `Arc<Mutex<AppState>>`. The engine mutates it
//! under the lock; the UI reads it and projects it onto Slint properties.
//! If the two ever drift, `AppState` wins on the next event.
#![allow(dead_code)] // ToastKind / ChainProgress / FocusSnapshot variants + helpers consumed across UI paths that don't all fire in tests

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct ChainProgress {
    pub step: usize,
    pub total: usize,
    pub mode: String,
}

/// Snapshot of the OS focus state captured at the instant the hotkey fires.
///
/// `hwnd_raw` is the foreground window's HWND stored as `isize` so the
/// snapshot can live in `AppState` (which is `Send + Sync`). HWND itself is
/// `*mut c_void` and therefore not thread-safe; we cast back to `HWND` only
/// at the moment we need to call Win32 APIs on the Slint main thread.
///
/// On Replace, the engine restores focus to this HWND before simulating
/// Ctrl+V so that the paste lands in the app the user was originally
/// typing in (e.g. Teams, Outlook), not inside the overlay itself.
#[derive(Debug, Clone, Copy, Default)]
pub struct FocusSnapshot {
    pub hwnd_raw: isize,
}

#[derive(Debug, Clone, Default)]
pub struct AppState {
    // Session (reset on every hotkey trigger)
    pub selected_text: String,
    pub last_result: String,
    pub last_app_hint: String,
    pub last_entry_id: Option<i64>,

    // View
    pub is_streaming: bool,
    pub is_done: bool,
    pub stream_buffer: String,
    pub chain_progress: Option<ChainProgress>,

    // Workspace tab state (history)
    pub history_entries: Vec<crate::core::history::HistoryEntry>,

    // Focus snapshot — captured by `handle_hotkey` right before the overlay
    // steals foreground. Used by `ConfirmReplace` to restore the user's
    // original app before simulating Ctrl+V.
    pub focus_target: Option<FocusSnapshot>,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset the per-session fields at the start of a new hotkey capture.
    /// Preserves focus target.
    pub fn reset_session(&mut self) {
        self.selected_text.clear();
        self.last_result.clear();
        self.last_app_hint.clear();
        self.last_entry_id = None;
        self.is_streaming = false;
        self.is_done = false;
        self.stream_buffer.clear();
        self.chain_progress = None;
    }

    pub fn begin_stream(&mut self, _mode: &str, _language: &str) {
        self.is_streaming = true;
        self.is_done = false;
        self.stream_buffer.clear();
    }

    pub fn push_chunk(&mut self, chunk: &str) {
        self.stream_buffer.push_str(chunk);
    }

    pub fn finish_stream(&mut self, full_text: &str, entry_id: Option<i64>) {
        self.last_result = full_text.to_string();
        self.last_entry_id = entry_id;
        self.is_streaming = false;
        self.is_done = true;
    }

    pub fn fail_stream(&mut self, _message: &str) {
        self.is_streaming = false;
        self.is_done = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_sensible() {
        let s = AppState::new();
        assert!(!s.is_streaming);
        assert!(!s.is_done);
        assert!(s.stream_buffer.is_empty());
    }

    #[test]
    fn reset_session_clears_per_capture_fields_only() {
        let mut s = AppState::new();
        s.selected_text = "hello".into();
        s.stream_buffer = "partial".into();
        s.is_streaming = true;
        s.last_entry_id = Some(42);
        s.focus_target = Some(FocusSnapshot { hwnd_raw: 123 });

        s.reset_session();

        // Per-session fields cleared
        assert!(s.selected_text.is_empty());
        assert!(s.stream_buffer.is_empty());
        assert!(!s.is_streaming);
        assert_eq!(s.last_entry_id, None);
        // Focus target preserved
        assert!(s.focus_target.is_some());
    }

    #[test]
    fn begin_stream_sets_flags() {
        let mut s = AppState::new();
        s.stream_buffer = "leftover".into();

        s.begin_stream("rewrite", "en");

        assert!(s.is_streaming);
        assert!(!s.is_done);
        assert!(s.stream_buffer.is_empty());
    }

    #[test]
    fn push_chunk_appends() {
        let mut s = AppState::new();
        s.push_chunk("Hel");
        s.push_chunk("lo");
        assert_eq!(s.stream_buffer, "Hello");
    }

    #[test]
    fn finish_stream_latches_result() {
        let mut s = AppState::new();
        s.begin_stream("rewrite", "en");
        s.push_chunk("Hello world");
        s.finish_stream("Hello world", Some(7));
        assert!(!s.is_streaming);
        assert!(s.is_done);
        assert_eq!(s.last_result, "Hello world");
        assert_eq!(s.last_entry_id, Some(7));
    }

    #[test]
    fn fail_stream_clears_stream_flags() {
        let mut s = AppState::new();
        s.begin_stream("rewrite", "en");
        s.fail_stream("network down");
        assert!(!s.is_streaming);
        assert!(!s.is_done);
    }

    #[test]
    fn new_starts_with_empty_collections() {
        let s = AppState::new();
        assert!(s.history_entries.is_empty());
    }
}
