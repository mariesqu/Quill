//! Tier 2 — Command Palette Window.
//!
//! Constructs the Slint `PaletteWindow`, populates the item catalog
//! (modes, chains, system commands), installs fuzzy-search filtering,
//! and wires item activation to the engine command channel.

use anyhow::{Context, Result};
use slint::ComponentHandle;

use crate::core::modes::{ChainConfig, ModeConfig};
use crate::state::UiCommand;
use crate::ui::{
    AppBridge, OverlayWindow, PaletteBridge, PaletteItem, PaletteWindow, WorkspaceWindow,
};

/// Build the palette window and wire its callbacks. Hidden at boot;
/// shown when summoned from the tray, the overlay's PALETTE button, or
/// the Ctrl+Shift+P global hotkey.
///
/// `workspace_weak` is used by the `cmd:workspace` / `cmd:settings`
/// palette actions to summon the Tier 3 workspace directly — no round
/// trip through the engine. `overlay_weak` lets the mode/chain actions
/// re-summon the overlay so its stream result is visible — the palette
/// is sometimes launched from the tray or Ctrl+Shift+P without the
/// overlay already being up.
pub fn build(
    modes: &std::collections::HashMap<String, ModeConfig>,
    chains: &std::collections::HashMap<String, ChainConfig>,
    cmd_tx: tokio::sync::mpsc::UnboundedSender<UiCommand>,
    workspace_weak: slint::Weak<WorkspaceWindow>,
    overlay_weak: slint::Weak<OverlayWindow>,
) -> Result<PaletteWindow> {
    let window = PaletteWindow::new().context("PaletteWindow::new failed")?;

    let catalog = build_catalog(modes, chains);
    let bridge = window.global::<PaletteBridge>();

    let slint_items: Vec<PaletteItem> = catalog.iter().map(slint_item).collect();
    bridge.set_items(slint::ModelRc::new(slint::VecModel::from(
        slint_items.clone(),
    )));
    bridge.set_filtered_items(slint::ModelRc::new(slint::VecModel::from(slint_items)));

    // Helper closure to reset the palette to a fresh state — called from
    // both `on_dismiss` and `on_item_activated`. Every summon must start
    // with the full catalog shown, search cleared, and the first item
    // highlighted, regardless of what the user did last time.
    let reset_palette = {
        let catalog = catalog.clone();
        let window_weak = window.as_weak();
        move || {
            if let Some(w) = window_weak.upgrade() {
                let pb = w.global::<PaletteBridge>();
                let items: Vec<PaletteItem> = catalog.iter().map(slint_item).collect();
                pb.set_filtered_items(slint::ModelRc::new(slint::VecModel::from(items)));
                pb.set_search_text("".into());
                pb.set_selected_index(0);
            }
        }
    };

    let catalog_clone = catalog.clone();
    let window_weak = window.as_weak();
    bridge.on_search_changed(move |query| {
        let query = query.to_string().to_lowercase();
        let filtered: Vec<PaletteItem> = if query.is_empty() {
            catalog_clone.iter().map(slint_item).collect()
        } else {
            catalog_clone
                .iter()
                .filter(|item| {
                    // Match against the pre-lowercased label / section
                    // stored on the catalog item. Doing the lowercase here
                    // (per keystroke × per item) allocated thousands of
                    // short strings during live typing — moved to catalog
                    // build time.
                    fuzzy_match_lower(&item.label_lc, &query)
                        || fuzzy_match_lower(&item.section_lc, &query)
                })
                .map(slint_item)
                .collect()
        };
        if let Some(w) = window_weak.upgrade() {
            w.global::<PaletteBridge>()
                .set_filtered_items(slint::ModelRc::new(slint::VecModel::from(filtered)));
        }
    });

    let tx = cmd_tx;
    let window_weak = window.as_weak();
    let workspace_for_cb = workspace_weak.clone();
    let overlay_for_cb = overlay_weak.clone();
    let reset_on_activate = reset_palette.clone();
    bridge.on_item_activated(move |id| {
        let id = id.to_string();
        tracing::debug!(%id, "palette item activated");

        // Read the active language from the palette's own AppBridge
        // before hiding — the palette AppBridge is kept in sync with the
        // overlay/workspace via `apply_to_all` so this mirrors the user's
        // current selection. Hardcoding "auto" ignored the picker.
        let language = window_weak
            .upgrade()
            .map(|w| w.global::<AppBridge>().get_active_language().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "auto".to_string());

        // Hide the palette immediately and reset it for next summon.
        if let Some(w) = window_weak.upgrade() {
            let _ = w.hide();
        }
        reset_on_activate();

        match id.as_str() {
            "cmd:workspace" => {
                summon_workspace(&workspace_for_cb, None);
            }
            "cmd:settings" => {
                summon_workspace(&workspace_for_cb, Some("settings"));
            }
            _ if id.starts_with("mode:") => {
                let mode = id.strip_prefix("mode:").unwrap_or(&id);
                // Make sure the overlay is visible — the stream result
                // renders there. Palette can be summoned from the tray
                // or Ctrl+Shift+P with no overlay up, and without this
                // call the selected mode ran but the user saw nothing.
                summon_overlay(&overlay_for_cb);
                let _ = tx.send(UiCommand::ExecuteMode {
                    mode: mode.to_string(),
                    language: language.clone(),
                    extra: None,
                });
            }
            _ if id.starts_with("chain:") => {
                let chain = id.strip_prefix("chain:").unwrap_or(&id);
                summon_overlay(&overlay_for_cb);
                let _ = tx.send(UiCommand::ExecuteChain {
                    chain_id: chain.to_string(),
                    language: language.clone(),
                    extra: None,
                });
            }
            _ => {
                tracing::warn!(%id, "unknown palette item activated");
            }
        }
    });

    let window_weak = window.as_weak();
    let reset_on_dismiss = reset_palette;
    bridge.on_dismiss(move || {
        if let Some(w) = window_weak.upgrade() {
            let _ = w.hide();
        }
        reset_on_dismiss();
    });

    Ok(window)
}

/// Summon (or re-summon) the Tier 1 overlay so the stream result of
/// a palette-dispatched mode / chain has a surface to render on. The
/// overlay stays at whatever position it last occupied — no repositioning
/// here because palette-dispatched flows don't have a fresh caret-capture
/// to anchor against. Idempotent: calling `show()` on a visible window
/// is a no-op.
fn summon_overlay(weak: &slint::Weak<OverlayWindow>) {
    if let Some(w) = weak.upgrade() {
        let _ = w.show();
        crate::ui::overlay_window::reapply_visual_treatment(&w);
        crate::ui::overlay_window::focus_text_input(&w);
    }
}

/// Summon the Tier 3 workspace from the palette, optionally switching
/// to a named tab. Mirrors `main::summon_workspace` but takes the weak
/// handle by reference so the palette can hold its own clone.
fn summon_workspace(weak: &slint::Weak<WorkspaceWindow>, focus_tab: Option<&'static str>) {
    if let Some(w) = weak.upgrade() {
        if let Some(tab) = focus_tab {
            w.global::<AppBridge>().set_current_tab(tab.into());
        }
        let _ = w.show();
        crate::ui::workspace_window::reapply_visual_treatment(&w);
        crate::ui::workspace_window::bring_to_front(&w);
    }
}

/// Position the palette centered on the monitor under the current
/// foreground window (fallback to primary). Multi-monitor + DPI-aware:
/// the palette's logical dimensions (560×460) are scaled by
/// `window().scale_factor()` before centering within the target monitor's
/// work area.
pub fn center_on_screen(window: &PaletteWindow) {
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetSystemMetrics, GetWindowRect, SM_CXSCREEN, SM_CYSCREEN,
    };

    // Logical palette size, matching the Slint component.
    let scale = window.window().scale_factor().max(0.1);
    let win_w_phys = (560.0 * scale).round() as i32;
    let win_h_phys = (460.0 * scale).round() as i32;

    let (left, top, right, bottom) = unsafe {
        let hwnd = GetForegroundWindow();
        let hmon = if hwnd.0.is_null() {
            let pt = POINT { x: 0, y: 0 };
            MonitorFromPoint(pt, MONITOR_DEFAULTTOPRIMARY)
        } else {
            let mut wr = RECT::default();
            if GetWindowRect(hwnd, &mut wr).is_ok() {
                let center = POINT {
                    x: (wr.left + wr.right) / 2,
                    y: (wr.top + wr.bottom) / 2,
                };
                MonitorFromPoint(center, MONITOR_DEFAULTTOPRIMARY)
            } else {
                MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY)
            }
        };
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(hmon, &mut info).as_bool() {
            (
                info.rcWork.left,
                info.rcWork.top,
                info.rcWork.right,
                info.rcWork.bottom,
            )
        } else {
            let w = GetSystemMetrics(SM_CXSCREEN).max(1);
            let h = GetSystemMetrics(SM_CYSCREEN).max(1);
            (0, 0, w, h)
        }
    };

    let x = left + ((right - left) - win_w_phys) / 2;
    let y = top + ((bottom - top) - win_h_phys) / 2;
    window
        .window()
        .set_position(slint::PhysicalPosition::new(x.max(left), y.max(top)));
}

/// Raise the palette to the top of the Z-order and give it foreground
/// focus. Needed on summon paths (tray click, Ctrl+Shift+P hotkey) so
/// the user can start typing into the search input immediately without
/// an extra mouse click.
pub fn bring_to_front(window: &PaletteWindow) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, SetForegroundWindow, ShowWindow, SW_SHOW,
    };

    let slint_handle = window.window().window_handle();
    let Ok(rw_handle) = slint_handle.window_handle() else {
        tracing::debug!("palette bring_to_front: HWND unavailable");
        return;
    };
    let RawWindowHandle::Win32(h) = rw_handle.as_raw() else {
        return;
    };
    let hwnd = HWND(h.hwnd.get() as *mut _);
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
    }
}

// ── Catalog ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct CatalogItem {
    id: String,
    label: String,
    section: String,
    icon: String,
    /// Pre-lowercased label for fuzzy matching. Computed once at catalog
    /// build time so every keystroke doesn't pay the `to_lowercase()`
    /// allocation cost per item.
    label_lc: String,
    /// Pre-lowercased section for fuzzy matching. Same rationale.
    section_lc: String,
}

fn build_catalog(
    modes: &std::collections::HashMap<String, ModeConfig>,
    chains: &std::collections::HashMap<String, ChainConfig>,
) -> Vec<CatalogItem> {
    use crate::core::modes::{chains_list, modes_list};

    let mut items = Vec::new();

    for m in modes_list(modes) {
        items.push(make_item(
            format!("mode:{}", m.id),
            m.label.clone(),
            "MODES".into(),
            m.icon.clone(),
        ));
    }

    for c in chains_list(chains) {
        items.push(make_item(
            format!("chain:{}", c.id),
            c.label.clone(),
            "CHAINS".into(),
            c.icon.clone(),
        ));
    }

    items.push(make_item(
        "cmd:workspace".into(),
        "Open Workspace".into(),
        "COMMANDS".into(),
        "⊞".into(),
    ));
    items.push(make_item(
        "cmd:settings".into(),
        "Open Settings".into(),
        "COMMANDS".into(),
        "⚙".into(),
    ));

    items
}

fn make_item(id: String, label: String, section: String, icon: String) -> CatalogItem {
    let label_lc = label.to_lowercase();
    let section_lc = section.to_lowercase();
    CatalogItem {
        id,
        label,
        section,
        icon,
        label_lc,
        section_lc,
    }
}

fn slint_item(item: &CatalogItem) -> PaletteItem {
    PaletteItem {
        id: item.id.clone().into(),
        label: item.label.clone().into(),
        section: item.section.clone().into(),
        icon: item.icon.clone().into(),
    }
}

// ── Fuzzy match ─────────────────────────────────────────────────────────────

/// Subsequence match over pre-lowercased strings. Both `haystack` and
/// `needle` MUST already be lowercased — this function does not allocate
/// a temporary `String` for normalization.
fn fuzzy_match_lower(haystack: &str, needle: &str) -> bool {
    let mut hay_chars = haystack.chars();
    for nc in needle.chars() {
        loop {
            match hay_chars.next() {
                Some(hc) if hc == nc => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}
