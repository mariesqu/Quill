//! Tier 3 — Workspace Window.
//!
//! A standard app window with title bar + taskbar presence + resizable
//! frame. Unlike the ephemeral overlay (Tier 1) or the transient palette
//! (Tier 2), this window lives on the taskbar and is meant to stay open
//! when the user wants to browse history, tune settings, or review
//! tutor output.
//!
//! Closing the window (OS close button, Alt+F4, Escape) HIDES it — the
//! app only truly quits from the tray's "Quit Quill" menu item. This
//! matches the overlay's model where the tray is the single source of
//! truth for process lifetime.
//!
//! Lives on the Slint main thread like every other window wrapper
//! (`overlay_window`, `palette_window`, `pencil_window`).
//! Slint globals are allocated PER WINDOW, so `bridge.rs` seeds and
//! installs callbacks on the WorkspaceWindow's own `AppBridge` instance.

use std::sync::Once;

use anyhow::{Context, Result};
use slint::ComponentHandle;
use windows::Win32::Foundation::HWND;

use crate::platform::dwm_shadow;
use crate::ui::WorkspaceWindow;

/// Guard so the DWM drop shadow attribute is written exactly once per
/// process lifetime — same pattern as `overlay_window::VISUAL_ONCE`.
static VISUAL_ONCE: Once = Once::new();

/// Build the workspace window. The OS HWND is not created until the event
/// loop first spins and `show()` is called, so DWM shadow application is
/// deferred to `reapply_visual_treatment` after the first show.
pub fn build() -> Result<WorkspaceWindow> {
    let window = WorkspaceWindow::new().context("WorkspaceWindow::new failed")?;
    install_close_to_tray(&window);
    Ok(window)
}

/// Intercept the OS close event so the X button hides the window instead
/// of quitting the app. The tray's "Quit Quill" item is the only path
/// that terminates the process — this matches user expectation for an
/// always-available productivity tool.
fn install_close_to_tray(window: &WorkspaceWindow) {
    let weak = window.as_weak();
    window.window().on_close_requested(move || {
        // Explicit hide() in case the response variant below isn't
        // honored by older Slint backends; harmless duplication when
        // it is.
        if let Some(w) = weak.upgrade() {
            let _ = w.hide();
        }
        slint::CloseRequestResponse::HideWindow
    });
}

/// Re-apply the DWM drop shadow after the workspace has been shown for
/// the first time. No-op on subsequent calls (guarded by `Once`).
pub fn reapply_visual_treatment(window: &WorkspaceWindow) {
    VISUAL_ONCE.call_once(|| match hwnd_of(window) {
        Ok(hwnd) => {
            if let Err(e) = dwm_shadow::enable(hwnd) {
                tracing::warn!("workspace DWM shadow enable failed (non-fatal): {e}");
            }
            tracing::debug!("workspace DWM shadow applied after first show");
        }
        Err(e) => {
            tracing::warn!("workspace HWND unavailable after show — shadow skipped: {e}");
        }
    });
}

/// Raise the workspace to the top of the Z-order and give it foreground
/// focus. Called from the tray summon path after `show()` so clicking
/// "Open Workspace" while another app is focused actually surfaces it.
pub fn bring_to_front(window: &WorkspaceWindow) {
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };

    let Ok(hwnd) = hwnd_of(window) else {
        tracing::debug!("workspace bring_to_front: HWND unavailable");
        return;
    };
    unsafe {
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
    }
}

/// Extract the native Win32 HWND. Returns an error before the event loop
/// has spun and `show()` has been called (winit creates the HWND lazily).
pub fn hwnd_of(window: &WorkspaceWindow) -> Result<HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let slint_handle = window.window().window_handle();
    let rw_handle = slint_handle
        .window_handle()
        .context("workspace window has no raw window handle (not yet shown)")?;
    match rw_handle.as_raw() {
        RawWindowHandle::Win32(h) => Ok(HWND(h.hwnd.get() as *mut _)),
        other => Err(anyhow::anyhow!("expected Win32 handle, got {:?}", other)),
    }
}
