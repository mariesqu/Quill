//! Cross-thread signal types between the engine and the UI layer.
//!
//! `UiEvent` flows engine → UI via `mpsc::UnboundedSender<UiEvent>`.
//! `UiCommand` flows UI → engine via `mpsc::UnboundedSender<UiCommand>`.
//! These two enums are the ONLY communication surface between the two sides.
#![allow(dead_code)] // Some UiEvent / UiCommand variants are fired only by optional flows

use crate::platform::context::AppContext;

#[derive(Debug, Clone)]
pub struct Suggestion {
    pub mode_id: String,
}

#[derive(Debug, Clone)]
pub enum UiEvent {
    ShowOverlay {
        text: String,
        context: AppContext,
        suggestion: Option<Suggestion>,
        /// Screen-physical rect of the element the caret is in, if UIA
        /// could find it. The bridge positions the overlay near this rect
        /// (below-right) and falls back to screen center when `None`.
        anchor_rect: Option<crate::platform::traits::ScreenRect>,
    },
    /// Hide the ephemeral overlay (Tier 1). Does NOT touch the workspace
    /// (Tier 3) — those have separate lifecycles: the overlay is summoned
    /// per-capture, the workspace is a long-lived tabbed window.
    DismissOverlay,
    /// Hide the workspace (Tier 3). Used when the user explicitly closes
    /// the tabbed window; does NOT affect the overlay.
    DismissWorkspace,

    StreamStart {
        mode: String,
        language: String,
    },
    StreamChunk {
        text: String,
    },
    StreamDone {
        full_text: String,
        entry_id: Option<i64>,
    },
    StreamError {
        message: String,
    },

    ChainProgress {
        step: usize,
        total: usize,
        mode: String,
    },

    ComparisonResult {
        mode_a: String,
        result_a: String,
        mode_b: String,
        result_b: String,
    },

    TutorExplanation {
        entry_id: i64,
        text: String,
    },
    TutorLesson {
        period: String,
        text: String,
    },

    Error {
        message: String,
    },

    HistoryLoaded(Vec<crate::core::history::HistoryEntry>),
    HistoryEntryUpdated {
        id: i64,
        favorited: bool,
    },
    /// Active language picker was changed (on any window). The bridge
    /// mirrors this across all three AppBridges so the pickers stay in
    /// sync. Persisted to user.yaml by the engine handler.
    LanguageChanged {
        code: String,
    },
    Toast {
        kind: crate::state::app_state::ToastKind,
        message: String,
    },
    /// User-initiated cancel of a stream (single, compare). The bridge
    /// clears `is_streaming` / `is_done` / `stream_buffer` on every
    /// AppBridge and surfaces a neutral Info toast. Tutor cancels do NOT
    /// use this — a tutor flow runs AFTER `finalize_result` has latched
    /// `is_done = true` on the primary stream, and we don't want a tutor
    /// cancel to wipe the replaceable primary result.
    StreamCancelled,
}

#[derive(Debug, Clone)]
pub enum UiCommand {
    ExecuteMode {
        mode: String,
        language: String,
        extra: Option<String>,
    },
    ExecuteChain {
        chain_id: String,
        language: String,
        extra: Option<String>,
    },
    CompareModes {
        mode_a: String,
        mode_b: String,
        language: String,
        extra: Option<String>,
    },
    RequestTutorExplain {
        entry_id: i64,
    },
    GenerateLesson {
        period: String,
    },
    ConfirmReplace,
    CancelStream,
    /// User asked to close the ephemeral overlay (Tier 1). The engine
    /// cancels any running stream and emits `UiEvent::DismissOverlay`.
    Dismiss,
    /// User asked to close the workspace (Tier 3). The engine emits
    /// `UiEvent::DismissWorkspace`; no stream cancellation (the workspace
    /// can be reopened any time without losing the buffer).
    DismissWorkspace,

    LoadHistory {
        limit: usize,
    },
    ToggleFavorite {
        entry_id: i64,
    },
    ExportHistory {
        format: String,
        path: std::path::PathBuf,
    },
    SaveConfig {
        updates: serde_json::Value,
    },
    /// User changed the active language (via any window's LangRow). Engine
    /// persists to user.yaml and emits `UiEvent::LanguageChanged` so the
    /// bridge mirrors the new code across every window's AppBridge.
    SetLanguage {
        code: String,
    },
    SwitchTab {
        tab: String,
    },
    /// Surface a validation error (e.g. unparseable hotkey spec) to the UI
    /// without going through any I/O. The engine translates this into a
    /// `UiEvent::Error` which the bridge displays as an error toast.
    EmitError {
        message: String,
    },
    /// Surface an informational toast (e.g. "Restart Quill for hotkey
    /// change to take effect"). Translated by the engine into
    /// `UiEvent::Toast { kind: Info, message }`.
    EmitInfo {
        message: String,
    },
}

// Templates feature removed: the engine had SaveTemplate / DeleteTemplate
// commands but nothing on the Slint side ever fired them (no Templates
// tab, `save-template(name, prompt)` in the Workspace bridge hard-coded
// `mode: String::new()`, and `seed_bridge` never populated the templates
// model). Users can still declare templates in `user.yaml` for forward
// compatibility — the Template struct stays in config.rs so YAML parsing
// continues to succeed — but the engine no longer writes them.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_event_is_clone_debug() {
        let e = UiEvent::StreamChunk { text: "hi".into() };
        let _ = format!("{e:?}");
        let _ = e.clone();
    }

    #[test]
    fn ui_command_is_clone_debug() {
        let c = UiCommand::ExecuteMode {
            mode: "rewrite".into(),
            language: "auto".into(),
            extra: None,
        };
        let _ = format!("{c:?}");
        let _ = c.clone();
    }
}
