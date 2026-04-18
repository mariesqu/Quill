#![allow(dead_code)] // CaptureResult.anchor, ScreenRect helpers used by UIA callers
//! Platform abstraction traits.
//!
//! These traits define the minimal boundary between the engine and the
//! Windows-specific implementations in this module. Production wires the
//! real implementations (`platform::capture::Capture`, `platform::replace::Replace`,
//! `platform::context::Context`). Tests wire fakes in `tests/common/fakes.rs`.
//!
//! Traits intentionally cover ONLY what the engine integration tests need to
//! swap — capture, replace, and context probing. Fire-and-forget side effects
//! (DWM shadow, tray, hotkey registration, caret hooks) are free functions
//! with nothing meaningful to assert against in isolation.

use async_trait::async_trait;

use crate::platform::context::AppContext;

/// Result of capturing selected text from the currently focused control.
#[derive(Debug, Clone, Default)]
pub struct CaptureResult {
    /// The captured text. Empty if nothing was captured.
    pub text: String,
    /// Screen-space rectangle of the selection, if UIA could provide it.
    /// Used to anchor the overlay to the user's focus.
    pub anchor: Option<ScreenRect>,
    /// Which code path produced this result — useful for diagnostics.
    pub source: CaptureSource,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum CaptureSource {
    #[default]
    None,
    Uia,
    Clipboard,
}

#[derive(Debug, Clone, Copy)]
pub struct ScreenRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl ScreenRect {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
}

/// Read selected text from the currently focused control.
#[async_trait]
pub trait TextCapture: Send + Sync {
    async fn capture(&self) -> CaptureResult;
}

/// Paste text into the currently focused control (simulated Ctrl+V).
#[async_trait]
pub trait TextReplace: Send + Sync {
    async fn paste(&self, text: &str) -> anyhow::Result<()>;
}

/// Read metadata about the currently foregrounded application.
/// Used to bias prompt building with app context.
pub trait ContextProbe: Send + Sync {
    fn active_context(&self) -> AppContext;
}
