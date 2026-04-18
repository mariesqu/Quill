//! Tier 1 — Ephemeral Overlay Window.
//!
//! Constructs the Slint `OverlayWindow`, applies the DWM drop shadow, and
//! exposes helpers to position / show / focus the window. Lives on the
//! Slint main thread like `pencil_window` and `workspace_window`.
//!
//! Near-caret positioning is handled by `bridge::position_overlay` using
//! the `anchor_rect` attached to `UiEvent::ShowOverlay` (captured via UIA
//! in `engine::hotkey_flow::handle_hotkey` before focus is stolen).

use std::sync::Once;

use anyhow::{Context, Result};
use slint::ComponentHandle;
use windows::Win32::Foundation::HWND;

use crate::platform::dwm_shadow;
use crate::ui::OverlayWindow;

/// Guard so the DWM drop shadow attribute is written exactly once per
/// process lifetime — same pattern as `workspace_window::VISUAL_ONCE`.
static VISUAL_ONCE: Once = Once::new();

/// Build the overlay window. The OS HWND is not created until the event
/// loop first spins and `show()` is called, so DWM shadow application is
/// deferred to `reapply_visual_treatment` after the first show.
pub fn build() -> Result<OverlayWindow> {
    OverlayWindow::new().context("OverlayWindow::new failed")
}

/// Re-apply the DWM drop shadow after the overlay has been shown for the
/// first time. No-op on subsequent calls (guarded by `Once`).
pub fn reapply_visual_treatment(window: &OverlayWindow) {
    VISUAL_ONCE.call_once(|| match hwnd_of(window) {
        Ok(hwnd) => {
            if let Err(e) = dwm_shadow::enable(hwnd) {
                tracing::warn!("overlay DWM shadow enable failed (non-fatal): {e}");
            }
            tracing::debug!("overlay DWM shadow applied after first show");
        }
        Err(e) => {
            tracing::warn!("overlay HWND unavailable after show — shadow skipped: {e}");
        }
    });
}

/// Place the overlay at a given screen-physical position. Coordinates are
/// in the same space as UIA `GetBoundingRectangles` and Win32 `POINT`.
pub fn set_position(window: &OverlayWindow, x: i32, y: i32) {
    window
        .window()
        .set_position(slint::PhysicalPosition::new(x, y));
}

/// Move input focus to the overlay's TextInput so the user can type
/// immediately without a prior mouse click. Invokes the Slint-side
/// `focus-text-input` callback which internally calls `text-input.focus()`.
///
/// MUST be called AFTER `show()` — the TextInput element does not exist
/// as a focusable target until the window is materialized.
pub fn focus_text_input(window: &OverlayWindow) {
    window.invoke_focus_text_input();
}

/// Extract the native Win32 HWND from the overlay window. Returns an error
/// before the event loop has spun and `show()` has been called, because
/// winit creates the OS window lazily on first show.
pub fn hwnd_of(window: &OverlayWindow) -> Result<HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let slint_handle = window.window().window_handle();
    let rw_handle = slint_handle
        .window_handle()
        .context("overlay window has no raw window handle (not yet shown)")?;
    match rw_handle.as_raw() {
        RawWindowHandle::Win32(h) => Ok(HWND(h.hwnd.get() as *mut _)),
        other => Err(anyhow::anyhow!("expected Win32 handle, got {:?}", other)),
    }
}
