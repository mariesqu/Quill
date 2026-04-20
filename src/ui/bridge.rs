//! Rust ↔ Slint bridge — event pump and command forwarder.
//!
//! ## Per-window globals
//! Slint globals (`AppBridge`) are allocated PER WINDOW, not shared across
//! the app. `WorkspaceWindow::new()`, `OverlayWindow::new()`, and every
//! other window construct their own independent `SharedGlobals` instance —
//! see `i-slint-compiler/generator/rust.rs:1645`. That means every state
//! write must hit EVERY `AppBridge` instance (the event pump does this via
//! `apply_to_all`), and every callback must be registered on the windows
//! that can fire it (the command forwarder does this via `install_on_*`).
//!
//! ## Three-tier window model
//! - **Overlay** (Tier 1): ephemeral, hotkey-summoned, near-caret.
//! - **Palette** (Tier 2): transient command palette. Has its own
//!   `PaletteBridge` for search + item list (wired in `palette_window.rs`)
//!   but ALSO has its own `AppBridge` instance — the footer reads
//!   `AppBridge.selected-text` so we must mirror state into it too.
//! - **Workspace** (Tier 3): persistent tabbed window — Write, History,
//!   Tutor, Compare, Settings.
//!
//! All three windows share the same `AppBridge` contract; `apply_to_all`
//! mirrors state writes into every window so the palette's footer is
//! current whenever it's summoned.

use std::sync::{Arc, Mutex};

use slint::{ComponentHandle, Model};
use tokio::sync::mpsc;

use crate::state::{AppState, UiCommand, UiEvent};
use crate::ui::{AppBridge, OverlayWindow, PaletteWindow, WorkspaceWindow};

// ── Seed ──────────────────────────────────────────────────────────────────────

/// Seed every window's `AppBridge` with the engine's modes/chains/settings.
/// Called once at startup before any window is shown.
pub fn seed_bridge(
    workspace: &WorkspaceWindow,
    overlay: &OverlayWindow,
    palette: &PaletteWindow,
    config: &crate::core::config::Config,
    modes: &std::collections::HashMap<String, crate::core::modes::ModeConfig>,
    chains: &std::collections::HashMap<String, crate::core::modes::ChainConfig>,
) {
    use crate::core::modes::{chains_list, modes_list};

    // Build the model vectors FRESH for each window. Slint's VecModel::from
    // consumes its input, so we cannot share one Vec between two AppBridges.
    let modes_list_cache = modes_list(modes);
    let chains_list_cache = chains_list(chains);

    let seed_one = |bridge: &AppBridge<'_>| {
        let mode_models: Vec<crate::ui::ModeInfo> = modes_list_cache
            .iter()
            .map(|m| crate::ui::ModeInfo {
                id: m.id.clone().into(),
                label: m.label.clone().into(),
                icon: m.icon.clone().into(),
            })
            .collect();
        bridge.set_modes(slint::ModelRc::new(slint::VecModel::from(mode_models)));

        let chain_models: Vec<crate::ui::ChainInfo> = chains_list_cache
            .iter()
            .map(|c| crate::ui::ChainInfo {
                id: c.id.clone().into(),
                label: c.label.clone().into(),
                icon: c.icon.clone().into(),
                description: c.description.clone().into(),
            })
            .collect();
        bridge.set_chains(slint::ModelRc::new(slint::VecModel::from(chain_models)));

        bridge.set_settings_provider(config.provider.clone().into());
        bridge.set_settings_api_key(config.api_key.clone().unwrap_or_default().into());
        bridge.set_settings_model(config.model.clone().into());
        bridge.set_settings_hotkey(config.hotkey.clone().unwrap_or_default().into());
        bridge
            .set_settings_palette_hotkey(config.hotkey_palette.clone().unwrap_or_default().into());

        bridge.set_persona_enabled(config.persona.enabled);
        bridge.set_persona_tone(config.persona.tone.clone().into());
        bridge.set_persona_style(config.persona.style.clone().into());
        bridge.set_persona_avoid(config.persona.avoid.clone().into());

        // Seed active-language from the saved config so the picker and
        // every bridge mirror in sync from boot.
        bridge.set_active_language(config.language.clone().into());

        let a_code = config.ui.overlay.pinned_translate.a.clone();
        let b_code = config.ui.overlay.pinned_translate.b.clone();
        let a_label = lang_label(&a_code);
        let b_label = lang_label(&b_code);
        bridge.set_pinned_translate_a_code(a_code.into());
        bridge.set_pinned_translate_a_label(a_label.into());
        bridge.set_pinned_translate_b_code(b_code.into());
        bridge.set_pinned_translate_b_label(b_label.into());
    };

    seed_one(&workspace.global::<AppBridge>());
    seed_one(&overlay.global::<AppBridge>());
    seed_one(&palette.global::<AppBridge>());
}

/// Derive a short uppercase label for a language code. "auto" → "Auto"
/// stays cased for the dropdown, everything else (en/fr/es/de/ja/pt/zh
/// and any locale the user sets) uppercases. Keeps the overlay's quick-
/// action buttons tight even for multi-letter codes like "zh-hans".
fn lang_label(code: &str) -> String {
    if code == "auto" {
        return "Auto".into();
    }
    code.to_uppercase()
}

/// Overlay dimensions used for screen-edge clamping. Match the `width` /
/// `height` set in overlay.slint (the overlay grows taller when a result
/// streams in — we clamp against the expanded height). In LOGICAL Slint
/// pixels — multiply by the window's scale factor before clamping against
/// physical-pixel monitor rectangles.
const OVERLAY_WIDTH_LOGICAL: i32 = 460;
const OVERLAY_HEIGHT_EXPANDED_LOGICAL: i32 = 380;
/// Gap between the caret/element rect and the overlay's top edge.
const OVERLAY_ANCHOR_GAP: i32 = 8;

/// Position the overlay near the user's caret (below-right of the focused
/// element's bounding rect) and clamp to the containing monitor's work
/// area. If no anchor is available, center the overlay on the monitor
/// under the foreground window (fallback to primary).
///
/// Multi-monitor + DPI-aware: resolves the monitor via `MonitorFromPoint`
/// /`MonitorFromRect`, reads `rcWork` in physical pixels, and scales the
/// overlay's logical dimensions by `window().scale_factor()` so the clamp
/// holds on non-100% DPI displays.
///
/// Safe to call before `show()` — winit honors `set_position` on unshown
/// windows, and the HWND materializes at the given coordinates.
fn position_overlay(overlay: &OverlayWindow, anchor: Option<crate::platform::traits::ScreenRect>) {
    let scale = overlay.window().scale_factor().max(0.1);
    let overlay_w_phys = (OVERLAY_WIDTH_LOGICAL as f32 * scale).round() as i32;
    let overlay_h_phys = (OVERLAY_HEIGHT_EXPANDED_LOGICAL as f32 * scale).round() as i32;

    let work = match anchor {
        Some(r) => monitor_work_area_for_rect(r.left, r.top, r.right, r.bottom),
        None => monitor_work_area_for_foreground(),
    };

    let (x, y) = match anchor {
        Some(r) => {
            // Prefer below the element. If that overflows the bottom of
            // the work area, flip above. Left-align with the element so
            // longer overlays don't obscure the left edge of the text.
            let below_y = r.bottom + OVERLAY_ANCHOR_GAP;
            let y = if below_y + overlay_h_phys > work.bottom {
                (r.top - overlay_h_phys - OVERLAY_ANCHOR_GAP).max(work.top)
            } else {
                below_y
            };
            let x_min = work.left;
            let x_max = (work.right - overlay_w_phys).max(work.left);
            let x = r.left.clamp(x_min, x_max);
            (x, y)
        }
        None => {
            // Center within the work area.
            let x = work.left + ((work.right - work.left) - overlay_w_phys) / 2;
            let y = work.top + ((work.bottom - work.top) - overlay_h_phys) / 2;
            (x.max(work.left), y.max(work.top))
        }
    };

    crate::ui::overlay_window::set_position(overlay, x, y);
}

/// Physical-pixel rectangle (monitor work area), using Win32 RECT semantics
/// (right/bottom are exclusive).
#[derive(Debug, Clone, Copy)]
struct WorkRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl WorkRect {
    fn primary_fallback() -> Self {
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        let (w, h) = unsafe {
            (
                GetSystemMetrics(SM_CXSCREEN).max(1),
                GetSystemMetrics(SM_CYSCREEN).max(1),
            )
        };
        Self {
            left: 0,
            top: 0,
            right: w,
            bottom: h,
        }
    }
}

fn monitor_work_area_for_rect(left: i32, top: i32, right: i32, bottom: i32) -> WorkRect {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromRect, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    unsafe {
        let rect = RECT {
            left,
            top,
            right,
            bottom,
        };
        let hmon = MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(hmon, &mut info).as_bool() {
            WorkRect {
                left: info.rcWork.left,
                top: info.rcWork.top,
                right: info.rcWork.right,
                bottom: info.rcWork.bottom,
            }
        } else {
            WorkRect::primary_fallback()
        }
    }
}

fn monitor_work_area_for_foreground() -> WorkRect {
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowRect};
    unsafe {
        let hwnd = GetForegroundWindow();
        let hmon = if hwnd.0.is_null() {
            let pt = POINT { x: 0, y: 0 };
            MonitorFromPoint(pt, MONITOR_DEFAULTTOPRIMARY)
        } else {
            // Prefer the window's containing monitor; if the window rect
            // is invalid, fall back to MonitorFromWindow's own logic.
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
            WorkRect {
                left: info.rcWork.left,
                top: info.rcWork.top,
                right: info.rcWork.right,
                bottom: info.rcWork.bottom,
            }
        } else {
            WorkRect::primary_fallback()
        }
    }
}

// ── Event pump ────────────────────────────────────────────────────────────────

/// Spawn a tokio task that drains `rx` and mirrors each `UiEvent` into
/// every window's `AppBridge` via `slint::invoke_from_event_loop`.
///
/// `state` is shared with the engine. For events like `StreamChunk` the
/// bridge reads the current authoritative buffer from `AppState` instead
/// of round-tripping through each window's own `AppBridge.stream-buffer`
/// getter — that earlier pattern was O(N²) in total response length
/// across 3 windows per chunk. Reading once from AppState and pushing
/// the same `SharedString` clone to each window is O(N) per chunk.
pub fn spawn_event_pump(
    workspace: &WorkspaceWindow,
    overlay: &OverlayWindow,
    palette: &PaletteWindow,
    state: Arc<Mutex<AppState>>,
    mut rx: mpsc::UnboundedReceiver<UiEvent>,
    rt: &tokio::runtime::Handle,
) {
    let weak_workspace = workspace.as_weak();
    let weak_overlay = overlay.as_weak();
    let weak_palette = palette.as_weak();
    rt.spawn(async move {
        while let Some(event) = rx.recv().await {
            let weak_workspace = weak_workspace.clone();
            let weak_overlay = weak_overlay.clone();
            let weak_palette = weak_palette.clone();
            let state = Arc::clone(&state);
            let _ = slint::invoke_from_event_loop(move || {
                let Some(workspace) = weak_workspace.upgrade() else {
                    return;
                };
                let Some(overlay) = weak_overlay.upgrade() else {
                    return;
                };
                let Some(palette) = weak_palette.upgrade() else {
                    return;
                };
                apply_event_on_ui_thread(&workspace, &overlay, &palette, &state, event);
            });
        }
        tracing::debug!("UiEvent pump shutting down — channel closed");
    });
}

fn apply_event_on_ui_thread(
    workspace: &WorkspaceWindow,
    overlay: &OverlayWindow,
    palette: &PaletteWindow,
    state: &Arc<Mutex<AppState>>,
    event: UiEvent,
) {
    match event {
        UiEvent::ShowOverlay {
            text,
            context,
            suggestion,
            anchor_rect,
        } => {
            // Length-only — see rationale in hotkey_flow.rs (logs may be
            // shared for support; captured text can be sensitive).
            tracing::debug!(
                text_len = text.len(),
                ?anchor_rect,
                "apply_event: ShowOverlay — positioning, showing, and mirroring state"
            );
            // Position BEFORE show() so the window materializes at the
            // right spot on the first paint (no visible jump from origin).
            position_overlay(overlay, anchor_rect);
            // Show the window so winit creates the HWND and Slint
            // finalizes component construction (including default-value
            // propagation through any two-way bindings). THEN write the
            // selected-text and other state. Order matters here: if we set
            // `selected-text` BEFORE the first `show()`, the TextInput's
            // default-empty `text` property is written back through the
            // `<=>` binding during the first render pass and clobbers our
            // value.
            match overlay.show() {
                Ok(()) => {
                    crate::ui::overlay_window::reapply_visual_treatment(overlay);
                    crate::ui::overlay_window::focus_text_input(overlay);
                    apply_to_all(workspace, overlay, palette, |b| {
                        b.set_selected_text(text.clone().into());
                        b.set_stream_buffer("".into());
                        b.set_last_result("".into());
                        b.set_is_streaming(false);
                        b.set_is_done(false);
                        b.set_toast(crate::ui::Toast {
                            kind: crate::ui::ToastKind::Info,
                            message: "".into(),
                            visible: false,
                        });
                        if let Some(s) = &suggestion {
                            b.set_active_mode(s.mode_id.clone().into());
                        } else {
                            b.set_active_mode("".into());
                        }
                    });
                    tracing::debug!(app = %context.app, "ShowOverlay applied across all windows");
                }
                Err(err) => {
                    // Don't log "applied" — the overlay didn't even become
                    // visible. Swallowing this silently made the "why did
                    // the hotkey do nothing?" class of bugs invisible.
                    tracing::warn!("overlay.show() failed: {err}");
                }
            }
        }
        UiEvent::DismissOverlay => {
            // Tier 1 only — the workspace (Tier 3) is a long-lived window
            // with its own close logic; users expect Replace/Esc on the
            // overlay to leave the workspace alone.
            let _ = overlay.hide();
            // Clear any in-flight streaming state so the workspace (and
            // palette footer) don't linger with a "streaming…" indicator
            // or a half-filled buffer after the overlay disappears.
            apply_to_all(workspace, overlay, palette, |b| {
                b.set_is_streaming(false);
                b.set_is_done(false);
                b.set_stream_buffer("".into());
            });
        }
        UiEvent::DismissWorkspace => {
            let _ = workspace.hide();
        }
        UiEvent::StreamStart { mode, language: _ } => {
            // Do NOT write active-language back here — the picker's value
            // IS the authoritative choice that fed this stream. Writing the
            // resolved language back (especially after "auto" → concrete
            // code resolution) clobbers the user's "Auto" selection and
            // makes the picker jump unexpectedly.
            apply_to_all(workspace, overlay, palette, |b| {
                b.set_active_mode(mode.clone().into());
                b.set_is_streaming(true);
                b.set_is_done(false);
                b.set_stream_buffer("".into());
            });
        }
        UiEvent::StreamChunk { text: _ } => {
            // AppState.stream_buffer is the canonical buffer — streaming.rs
            // pushes every chunk into it BEFORE emitting this event. Read
            // once and broadcast, avoiding per-window get-modify-set (which
            // was O(buffer_length * 3 windows) per chunk — quadratic over
            // a full stream).
            let full: slint::SharedString = state.lock().unwrap().stream_buffer.clone().into();
            apply_to_all(workspace, overlay, palette, |b| {
                b.set_stream_buffer(full.clone());
            });
        }
        UiEvent::StreamDone {
            full_text,
            entry_id,
        } => {
            // Clamp to i32 (Slint int) — history ids are i64 in Rust but
            // never exceed i32::MAX in practice. Zero means "no entry"
            // (history disabled or save failed); the Tutor EXPLAIN handler
            // falls back to AppState.last_entry_id in that case.
            let eid_i32 = entry_id.unwrap_or(0).min(i32::MAX as i64) as i32;
            apply_to_all(workspace, overlay, palette, |b| {
                b.set_stream_buffer(full_text.clone().into());
                b.set_last_result(full_text.clone().into());
                b.set_is_streaming(false);
                b.set_is_done(true);
                b.set_last_entry_id(eid_i32);
            });
        }
        UiEvent::StreamError { message } => {
            apply_to_all(workspace, overlay, palette, |b| {
                b.set_is_streaming(false);
                b.set_is_done(false);
                b.set_toast(crate::ui::Toast {
                    kind: crate::ui::ToastKind::Error,
                    message: message.clone().into(),
                    visible: true,
                });
            });
        }
        UiEvent::ChainProgress { step, total, mode } => {
            apply_to_all(workspace, overlay, palette, |b| {
                b.set_active_mode(format!("{mode} ({step}/{total})").into());
            });
        }
        UiEvent::ComparisonResult {
            mode_a,
            result_a,
            mode_b,
            result_b,
        } => {
            apply_to_all(workspace, overlay, palette, |b| {
                b.set_compare_mode_a(mode_a.clone().into());
                b.set_compare_result_a(result_a.clone().into());
                b.set_compare_mode_b(mode_b.clone().into());
                b.set_compare_result_b(result_b.clone().into());
            });
        }
        UiEvent::TutorExplanation { entry_id: _, text } => {
            apply_to_all(workspace, overlay, palette, |b| {
                b.set_last_tutor_explanation(text.clone().into());
            });
        }
        UiEvent::TutorLesson { period: _, text } => {
            apply_to_all(workspace, overlay, palette, |b| {
                b.set_last_tutor_lesson(text.clone().into());
            });
        }
        UiEvent::Error { message } => {
            apply_to_all(workspace, overlay, palette, |b| {
                b.set_toast(crate::ui::Toast {
                    kind: crate::ui::ToastKind::Error,
                    message: message.clone().into(),
                    visible: true,
                });
            });
        }
        UiEvent::HistoryLoaded(entries) => {
            // HistoryLoaded only affects the WorkspaceWindow's History tab.
            // The overlay has no history UI, so we skip the overlay write.
            let b = workspace.global::<AppBridge>();
            let models: Vec<crate::ui::HistoryEntry> = entries
                .into_iter()
                .map(|e| crate::ui::HistoryEntry {
                    id: e.id as i32,
                    timestamp: e.timestamp.into(),
                    mode: e.mode.unwrap_or_default().into(),
                    language: e.language.unwrap_or_default().into(),
                    original: e.original_text.into(),
                    output: e.output_text.into(),
                    favorited: e.favorited,
                })
                .collect();
            b.set_history(slint::ModelRc::new(slint::VecModel::from(models)));
        }
        UiEvent::HistoryEntryUpdated { id, favorited } => {
            let b = workspace.global::<AppBridge>();
            let model = b.get_history();
            if let Some(vec_model) = model
                .as_any()
                .downcast_ref::<slint::VecModel<crate::ui::HistoryEntry>>()
            {
                for i in 0..vec_model.row_count() {
                    if let Some(entry) = vec_model.row_data(i) {
                        if entry.id == id as i32 {
                            let mut updated = entry.clone();
                            updated.favorited = favorited;
                            vec_model.set_row_data(i, updated);
                            break;
                        }
                    }
                }
            }
        }
        UiEvent::LanguageChanged { code } => {
            // Broadcast so every window's language picker stays in sync.
            apply_to_all(workspace, overlay, palette, |b| {
                b.set_active_language(code.clone().into());
            });
        }
        UiEvent::Toast { kind, message } => {
            apply_to_all(workspace, overlay, palette, |b| {
                let k = match kind {
                    crate::state::app_state::ToastKind::Info => crate::ui::ToastKind::Info,
                    crate::state::app_state::ToastKind::Success => crate::ui::ToastKind::Success,
                    crate::state::app_state::ToastKind::Warning => crate::ui::ToastKind::Warning,
                    crate::state::app_state::ToastKind::Error => crate::ui::ToastKind::Error,
                };
                b.set_toast(crate::ui::Toast {
                    kind: k,
                    message: message.clone().into(),
                    visible: true,
                });
            });
        }
        UiEvent::StreamCancelled => {
            // Overlay CANCEL is gated on `is-streaming`, REPLACE on
            // `is-done`. If we only emit a Toast on cancel the overlay
            // stays stuck with CANCEL visible and REPLACE never appears.
            // Mirror StreamDone's broadcast shape to clear the stream UI
            // across every window, THEN surface the Info toast.
            apply_to_all(workspace, overlay, palette, |b| {
                b.set_is_streaming(false);
                b.set_is_done(false);
                b.set_stream_buffer("".into());
                b.set_toast(crate::ui::Toast {
                    kind: crate::ui::ToastKind::Info,
                    message: "Cancelled".into(),
                    visible: true,
                });
            });
        }
    }
}

/// Apply the same mutation to every window's `AppBridge` instance.
/// The closure is invoked once per window — so every `set_*` call inside
/// hits all three (workspace + overlay + palette). Keeps the palette's
/// footer / context preview current with the latest `selected-text`.
fn apply_to_all(
    workspace: &WorkspaceWindow,
    overlay: &OverlayWindow,
    palette: &PaletteWindow,
    mut f: impl FnMut(&AppBridge<'_>),
) {
    f(&workspace.global::<AppBridge>());
    f(&overlay.global::<AppBridge>());
    f(&palette.global::<AppBridge>());
}

// ── Command forwarder ─────────────────────────────────────────────────────────

/// Register Slint callbacks on the overlay and workspace `AppBridge`
/// instances so that user clicks dispatch `UiCommand`s to the engine.
/// `state` is used to sync overlay TextInput edits into
/// `AppState.selected_text` before dispatching text-dependent commands.
///
/// Palette callbacks are wired separately in `palette_window::build` —
/// that window has its own `PaletteBridge` for navigation and uses the
/// `cmd_tx` channel directly.
pub fn install_command_forwarder(
    workspace: &WorkspaceWindow,
    overlay: &OverlayWindow,
    state: Arc<Mutex<AppState>>,
    tx: mpsc::UnboundedSender<UiCommand>,
    config: &crate::core::config::Config,
) {
    // Snapshot the hotkey values at startup so the Save Settings handler
    // can detect whether the user actually changed them. Previously the
    // restart toast fired on EVERY save (including no-op saves from
    // unrelated fields), training users to ignore it.
    let original_hotkey = config.hotkey.clone().unwrap_or_default();
    let original_palette_hotkey = config.hotkey_palette.clone().unwrap_or_default();
    install_on_workspace(
        workspace,
        tx.clone(),
        original_hotkey,
        original_palette_hotkey,
    );
    install_on_overlay(overlay, state, tx);
}

fn install_on_workspace(
    window: &WorkspaceWindow,
    tx: mpsc::UnboundedSender<UiCommand>,
    original_hotkey: String,
    original_palette_hotkey: String,
) {
    // Workspace doesn't have an editable free-text input in the Write tab
    // (only read-only preview of the captured selection) — the overlay is
    // the single place the user can type a free-text prompt. So no
    // AppState text sync is needed before dispatching commands from here.
    let bridge = window.global::<AppBridge>();

    let tx_em = tx.clone();
    bridge.on_execute_mode(move |mode, lang| {
        let _ = tx_em.send(UiCommand::ExecuteMode {
            mode: mode.to_string(),
            language: lang.to_string(),
            extra: None,
        });
    });

    let tx_ec = tx.clone();
    bridge.on_execute_chain(move |chain_id, lang| {
        let _ = tx_ec.send(UiCommand::ExecuteChain {
            chain_id: chain_id.to_string(),
            language: lang.to_string(),
            extra: None,
        });
    });

    let tx_sl = tx.clone();
    bridge.on_set_language(move |code| {
        // Route through the engine so the change is persisted to
        // user.yaml AND broadcast to all three AppBridges (overlay,
        // palette, workspace) via UiEvent::LanguageChanged. Writing to
        // the local bridge only was a bug — the other two windows kept
        // showing the previous language.
        let _ = tx_sl.send(UiCommand::SetLanguage {
            code: code.to_string(),
        });
    });

    let tx_cr = tx.clone();
    bridge.on_confirm_replace(move || {
        let _ = tx_cr.send(UiCommand::ConfirmReplace);
    });

    // No `on_cancel_stream` registration here: only overlay.slint invokes
    // AppBridge.cancel-stream(). The workspace has no CANCEL affordance
    // and keeping the dead handler just invited confusion about where
    // cancels originate.

    let tx_dm = tx.clone();
    bridge.on_dismiss(move || {
        // Workspace dismiss targets ONLY the workspace — `UiCommand::Dismiss`
        // would hide the overlay (and cancel any running stream), which is
        // not what the user asked for when they hit Esc / X on the workspace.
        // The engine handler for `DismissWorkspace` emits
        // `UiEvent::DismissWorkspace`, which the event pump translates into
        // `workspace.hide()` only.
        let _ = tx_dm.send(UiCommand::DismissWorkspace);
    });

    // Workspace has native window chrome — drag is handled by the OS.
    // The drag-window callback exists only on the overlay.

    let tx_lh = tx.clone();
    bridge.on_load_history(move || {
        let _ = tx_lh.send(UiCommand::LoadHistory { limit: 100 });
    });

    let tx_tf = tx.clone();
    bridge.on_toggle_favorite(move |id| {
        let _ = tx_tf.send(UiCommand::ToggleFavorite {
            entry_id: id as i64,
        });
    });

    let tx_eh = tx.clone();
    bridge.on_export_history(move |format| {
        let format = format.to_string();
        let (default_ext, filter_name, filter_exts): (&str, &str, &[&str]) = match format.as_str() {
            "json" => ("json", "JSON", &["json"]),
            "csv" => ("csv", "CSV", &["csv"]),
            "md" => ("md", "Markdown", &["md"]),
            _ => ("txt", "All", &["*"]),
        };
        let default_name = format!("quill-history.{default_ext}");
        let picked = rfd::FileDialog::new()
            .set_file_name(&default_name)
            .add_filter(filter_name, filter_exts)
            .save_file();
        if let Some(path) = picked {
            let _ = tx_eh.send(UiCommand::ExportHistory { format, path });
        }
    });

    let tx_st = tx.clone();
    let window_weak = window.as_weak();
    bridge.on_switch_tab(move |tab| {
        if let Some(w) = window_weak.upgrade() {
            w.global::<AppBridge>().set_current_tab(tab.clone());
        }
        let _ = tx_st.send(UiCommand::SwitchTab {
            tab: tab.to_string(),
        });
    });

    let tx_rc = tx.clone();
    let window_weak_rc = window.as_weak();
    bridge.on_run_compare(move |a, b| {
        // Read the user's currently-selected language from the workspace's
        // AppBridge (the Language picker mirrors into AppBridge.active-
        // language). Hardcoding "auto" was a bug — it ignored the picker
        // and forced the model to detect input language every time, even
        // when the user had explicitly selected a target.
        let language = window_weak_rc
            .upgrade()
            .map(|w| w.global::<AppBridge>().get_active_language().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "auto".to_string());
        let _ = tx_rc.send(UiCommand::CompareModes {
            mode_a: a.to_string(),
            mode_b: b.to_string(),
            language,
            extra: None,
        });
    });

    let tx_rte = tx.clone();
    bridge.on_request_tutor_explain(move |eid| {
        let _ = tx_rte.send(UiCommand::RequestTutorExplain {
            entry_id: eid as i64,
        });
    });

    let tx_gl = tx.clone();
    bridge.on_generate_lesson(move |period| {
        let _ = tx_gl.send(UiCommand::GenerateLesson {
            period: period.to_string(),
        });
    });

    let window_weak_ss = window.as_weak();
    let tx_ss = tx.clone();
    let tx_err = tx.clone();
    bridge.on_save_settings(move || {
        let Some(w) = window_weak_ss.upgrade() else {
            return;
        };
        let b = w.global::<AppBridge>();
        let hotkey = b.get_settings_hotkey().to_string();
        let palette_hotkey = b.get_settings_palette_hotkey().to_string();

        // Validate hotkey specs BEFORE persisting — an unparseable spec
        // would be saved, then on next boot `HotkeyService::register`
        // would fail and the hotkey would silently not work. Empty is
        // allowed — it means "disable this binding".
        if let Err(e) = validate_hotkey_opt(&hotkey) {
            let msg = format!("Invalid hotkey: {e}");
            tracing::warn!("{msg}");
            // Surface to the UI via the dedicated `EmitError` command
            // so the user sees an error toast (not just a log line).
            let _ = tx_err.send(UiCommand::EmitError { message: msg });
            return;
        }
        if let Err(e) = validate_hotkey_opt(&palette_hotkey) {
            let msg = format!("Invalid palette hotkey: {e}");
            tracing::warn!("{msg}");
            let _ = tx_err.send(UiCommand::EmitError { message: msg });
            return;
        }

        let updates = serde_json::json!({
            "provider":       b.get_settings_provider().as_str(),
            "api_key":        b.get_settings_api_key().as_str(),
            "model":          b.get_settings_model().as_str(),
            "hotkey":         hotkey.clone(),
            "hotkey_palette": palette_hotkey.clone(),
        });
        let _ = tx_ss.send(UiCommand::SaveConfig { updates });

        // R2-32 / R3-6: hotkey changes don't take effect at runtime — the
        // HotkeyService was built at startup and there's no live re-
        // registration path. Nudge the user ONLY when the saved hotkey
        // actually differs from what the current process booted with.
        // Firing on every save (even no-op saves from unrelated fields)
        // trained users to ignore the toast.
        let hotkey_changed = hotkey != original_hotkey;
        let palette_changed = palette_hotkey != original_palette_hotkey;
        if hotkey_changed || palette_changed {
            let _ = tx_ss.send(UiCommand::EmitInfo {
                message: "Restart Quill for hotkey change to take effect".into(),
            });
        }
    });

    // ── Voice / Persona save handler ────────────────────────────────────
    // Mirrors the settings save flow: read AppBridge.persona-* properties,
    // pack them into a JSON merge patch, route through UiCommand::SaveConfig
    // so save_user_config can merge into user.yaml under the config write
    // lock. Engine::inner.config is cached at boot, so changes take effect
    // on next launch — same as provider/model/hotkey.
    let window_weak_vs = window.as_weak();
    let tx_vs = tx.clone();
    bridge.on_save_persona(move || {
        let Some(w) = window_weak_vs.upgrade() else {
            return;
        };
        let b = w.global::<AppBridge>();
        let updates = serde_json::json!({
            "persona": {
                "enabled": b.get_persona_enabled(),
                "tone":    b.get_persona_tone().as_str(),
                "style":   b.get_persona_style().as_str(),
                "avoid":   b.get_persona_avoid().as_str(),
            }
        });
        let _ = tx_vs.send(UiCommand::SaveConfig { updates });
        let _ = tx_vs.send(UiCommand::EmitInfo {
            message: "Voice saved — restart Quill to apply".into(),
        });
    });
}

/// Validate an optional hotkey spec. An empty string is "unset" (disables
/// the binding) — accepted. Anything else must round-trip through
/// `parse_hotkey_spec` so the user isn't left with a silently-broken
/// hotkey on next boot.
fn validate_hotkey_opt(spec: &str) -> Result<(), String> {
    if spec.trim().is_empty() {
        return Ok(());
    }
    crate::platform::hotkey::parse_hotkey_spec(spec)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn install_on_overlay(
    window: &OverlayWindow,
    state: Arc<Mutex<AppState>>,
    tx: mpsc::UnboundedSender<UiCommand>,
) {
    let bridge = window.global::<AppBridge>();

    // ── Text-sync helpers ─────────────────────────────────────────────────
    // Before dispatching any command that operates on text, sync the
    // overlay's TextInput value (which lives in overlay's AppBridge.selected-
    // text) into `AppState.selected_text` so the engine sees the user's
    // edits. The overlay is the ONLY place in Quill where the user can type
    // a free-text prompt, so this sync is critical to the free-text feature.
    let sync_text = {
        let window_weak = window.as_weak();
        let state = state.clone();
        move || {
            if let Some(w) = window_weak.upgrade() {
                let current = w.global::<AppBridge>().get_selected_text().to_string();
                if let Ok(mut s) = state.lock() {
                    s.selected_text = current;
                }
            }
        }
    };

    let tx_em = tx.clone();
    let sync = sync_text.clone();
    bridge.on_execute_mode(move |mode, lang| {
        sync();
        let _ = tx_em.send(UiCommand::ExecuteMode {
            mode: mode.to_string(),
            language: lang.to_string(),
            extra: None,
        });
    });

    let tx_ec = tx.clone();
    let sync = sync_text.clone();
    bridge.on_execute_chain(move |chain_id, lang| {
        sync();
        let _ = tx_ec.send(UiCommand::ExecuteChain {
            chain_id: chain_id.to_string(),
            language: lang.to_string(),
            extra: None,
        });
    });

    // NOTE: `on_set_language` is intentionally NOT registered here. The
    // overlay.slint has no LangRow — the language picker lives on the
    // workspace's Write tab. Registering a no-op handler on the overlay
    // was confusing and suggested the overlay emitted language-change
    // callbacks that it doesn't.

    let tx_cr = tx.clone();
    bridge.on_confirm_replace(move || {
        let _ = tx_cr.send(UiCommand::ConfirmReplace);
    });

    let tx_cs = tx.clone();
    bridge.on_cancel_stream(move || {
        let _ = tx_cs.send(UiCommand::CancelStream);
    });

    let tx_dm = tx.clone();
    bridge.on_dismiss(move || {
        let _ = tx_dm.send(UiCommand::Dismiss);
    });

    let window_weak = window.as_weak();
    bridge.on_drag_window(move || {
        if let Some(w) = window_weak.upgrade() {
            // Reuse the same Win32 SC_MOVE trick the workspace window
            // does not need (it has native chrome), but the overlay is
            // frameless — manual drag.
            if let Ok(hwnd) = crate::ui::overlay_window::hwnd_of(&w) {
                use windows::Win32::Foundation::{LPARAM, WPARAM};
                use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
                use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_SYSCOMMAND};
                unsafe {
                    let _ = ReleaseCapture();
                    let _ = PostMessageW(hwnd, WM_SYSCOMMAND, WPARAM(0xF012), LPARAM(0));
                }
            }
        }
    });
}
