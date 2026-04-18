//! Floating pencil window. Constructs the Slint `PencilWindow`, grabs the
//! HWND, applies always-on-top + click-through + no-taskbar + no-activate
//! Win32 extended styles, and exposes helpers to position / show / hide
//! and to toggle click-through at runtime.
//!
//! The pencil lives on the Slint main thread. All methods must be called
//! from that thread (they ultimately touch `ComponentHandle`).

use anyhow::{Context, Result};
use slint::ComponentHandle;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
};

use crate::ui::PencilWindow;

/// Build the pencil window.
///
/// Note: the HWND is NOT available until the event loop has spun at least
/// once and `show()` has been called. Therefore this function:
///   1. Constructs the Slint window (no show yet)
///   2. Returns the window handle — styles are applied lazily on first show
///      by `pencil_controller::apply_cmd` via `STYLES_APPLIED` thread-local.
///
/// After this function returns, the caller must NOT call `show()` — the
/// `PencilController` will call it after confirming an editable focus.
pub fn build() -> Result<PencilWindow> {
    let window = PencilWindow::new().context("PencilWindow::new failed")?;
    window.set_is_visible(false);
    Ok(window)
}

/// Apply the base pencil Win32 extended styles. `click_through=true` adds
/// `WS_EX_TRANSPARENT`; `false` removes it so the pencil can receive clicks.
pub fn apply_pencil_styles(hwnd: HWND, click_through: bool) {
    unsafe {
        let current = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let base = current | WS_EX_LAYERED.0 | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0;
        let new = if click_through {
            base | WS_EX_TRANSPARENT.0
        } else {
            base & !WS_EX_TRANSPARENT.0
        };
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new as isize);
    }
}

/// Set the pencil's screen position. `x` / `y` are in physical pixels
/// (the same coordinate space as UIA `GetBoundingRectangles`).
pub fn set_position(window: &PencilWindow, x: i32, y: i32) {
    window
        .window()
        .set_position(slint::PhysicalPosition::new(x, y));
}

/// Retrieve the HWND from the pencil window's native handle.
pub fn hwnd_of(window: &PencilWindow) -> Result<HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    // `slint::Window::window_handle()` returns a `slint::WindowHandle` (not a Result).
    // `slint::WindowHandle` implements `HasWindowHandle` from raw-window-handle 0.6.
    // `HasWindowHandle::window_handle()` returns `Result<WindowHandle<'_>, HandleError>`.
    let slint_handle = window.window().window_handle();
    let rw_handle = slint_handle
        .window_handle()
        .context("pencil window has no raw window handle")?;
    match rw_handle.as_raw() {
        RawWindowHandle::Win32(h) => Ok(HWND(h.hwnd.get() as *mut _)),
        other => Err(anyhow::anyhow!("expected Win32 handle, got {:?}", other)),
    }
}

/// Install a 30 Hz `slint::Timer` that polls the mouse cursor position and
/// toggles click-through off when within `PROXIMITY_PX` of the pencil center.
/// Returns the timer so the caller can keep it alive (dropping it stops the
/// polling).
pub fn install_proximity_toggle(window: &PencilWindow) -> slint::Timer {
    use slint::{Timer, TimerMode};
    use std::cell::Cell;
    use std::rc::Rc;
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    const PROXIMITY_PX: i32 = 40;
    const PENCIL_HALF_WIDTH: i32 = 16;
    const PENCIL_HALF_HEIGHT: i32 = 16;

    let weak = window.as_weak();
    let was_click_through: Rc<Cell<bool>> = Rc::new(Cell::new(true));

    let timer = Timer::default();
    timer.start(
        TimerMode::Repeated,
        std::time::Duration::from_millis(33),
        move || {
            let Some(window) = weak.upgrade() else {
                return;
            };

            // Skip all proximity work when the pencil is hidden — there's
            // no UI to toggle click-through on, and running GetCursorPos /
            // SetWindowLongPtrW every 33ms while the user is away from any
            // editable control was pure overhead.
            if !window.get_is_visible() {
                return;
            }

            // Read the pencil position.
            let pos = window.window().position();
            let pencil_center = (pos.x + PENCIL_HALF_WIDTH, pos.y + PENCIL_HALF_HEIGHT);

            // Read the cursor position.
            let mut cursor = POINT { x: 0, y: 0 };
            if unsafe { GetCursorPos(&mut cursor) }.is_err() {
                return;
            }

            let dx = cursor.x - pencil_center.0;
            let dy = cursor.y - pencil_center.1;
            let within = (dx * dx + dy * dy) <= (PROXIMITY_PX * PROXIMITY_PX);

            let should_click_through = !within;
            if should_click_through != was_click_through.get() {
                // Toggled — re-apply styles.
                if let Ok(hwnd) = hwnd_of(&window) {
                    apply_pencil_styles(hwnd, should_click_through);
                    was_click_through.set(should_click_through);
                }
            }
        },
    );
    timer
}
