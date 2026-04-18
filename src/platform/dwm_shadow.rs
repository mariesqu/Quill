//! DWM native drop shadow for borderless windows.
//!
//! Quill uses frameless / `no-frame: true` windows for the overlay, palette,
//! and pencil indicator. Without DWM's non-client rendering turned ON, they
//! render flat against the desktop. Calling `enable(hwnd)` forces the NC
//! rendering policy on, which is what produces the subtle drop shadow.

use anyhow::{Context, Result};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMNCRENDERINGPOLICY, DWMNCRP_ENABLED, DWMWA_NCRENDERING_POLICY,
};

pub fn enable(hwnd: HWND) -> Result<()> {
    unsafe {
        let policy: DWMNCRENDERINGPOLICY = DWMNCRP_ENABLED;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_NCRENDERING_POLICY,
            &policy as *const _ as *const _,
            std::mem::size_of::<DWMNCRENDERINGPOLICY>() as u32,
        )
        .context("DwmSetWindowAttribute(DWMWA_NCRENDERING_POLICY)")?;
    }
    Ok(())
}
