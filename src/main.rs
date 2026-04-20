#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod engine;
mod platform;
mod providers;
mod state;
mod ui;

use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use slint::ComponentHandle;
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

use crate::core::config::load_config;
use crate::core::modes::load_modes;
use crate::engine::Engine;
use crate::platform::capture::Capture;
use crate::platform::caret::{CaretHookService, FocusEvent};
use crate::platform::context::Context as ContextImpl;
use crate::platform::replace::Replace;
use crate::platform::traits::{ContextProbe, TextCapture, TextReplace};
use crate::platform::tray::{TrayMenu, TrayService, TRAY_ICON_PNG};
use crate::providers::{build_provider, Provider};
use crate::state::{AppState, UiCommand, UiEvent};
use crate::ui::{
    bridge, overlay_window, palette_window, pencil_controller::PencilController, pencil_window,
    workspace_window,
};

fn main() -> Result<()> {
    init_tracing();
    tracing::info!("Quill boot");

    enable_dpi_awareness();

    // Build an explicit multi-thread tokio runtime. We cannot use
    // `#[tokio::main]` because Slint takes ownership of the main thread via
    // `window.run()` and cannot be moved to a tokio worker.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;
    let rt_handle = rt.handle().clone();

    let config = load_config();
    let (modes, chains) = load_modes(&config);

    // Bring the history SQLite schema up before any query can fire. Cheap
    // idempotent `CREATE TABLE IF NOT EXISTS` — safe on every boot. Failing
    // here is non-fatal (history features degrade gracefully); we warn and
    // flip the HISTORY_USABLE flag so every subsequent call short-circuits
    // early instead of re-opening, re-failing, and spamming the log.
    if config.history.enabled {
        if let Err(e) = crate::core::history::init_db() {
            tracing::warn!("history::init_db failed (history features disabled): {e}");
            crate::core::history::HISTORY_USABLE.store(false, std::sync::atomic::Ordering::Release);
        }
    } else {
        // History is opt-out via user.yaml. Disable the usability flag so
        // no stray call re-opens the DB unexpectedly.
        crate::core::history::HISTORY_USABLE.store(false, std::sync::atomic::Ordering::Release);
    }

    let state = Arc::new(Mutex::new(AppState::new()));
    // Clone for the bridge's command forwarder — callbacks need to sync
    // overlay TextInput edits into AppState before dispatching commands.
    let state_for_bridge = Arc::clone(&state);
    let (event_tx, event_rx) = mpsc::unbounded_channel::<UiEvent>();
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<UiCommand>();

    let capture_impl: Arc<dyn TextCapture> = Arc::new(Capture);
    let replace_impl: Arc<dyn TextReplace> = Arc::new(Replace);
    let context_impl: Arc<dyn ContextProbe> = Arc::new(ContextImpl);
    let provider: Arc<dyn Provider> = build_provider(&config);

    let engine = Engine::new(
        config.clone(),
        modes.clone(),
        chains.clone(),
        state,
        event_tx,
        capture_impl,
        replace_impl,
        context_impl,
        provider,
    );

    // Build the three-tier Slint windows. HWND is not valid until winit
    // spins the event loop and `show()` is called; visual treatment (DWM
    // shadow) is applied lazily after the first show for each.
    //
    // Tier 1 — ephemeral overlay (hotkey-summoned, near-caret, frameless,
    // always-on-top). Constructed but hidden at startup.
    let overlay = overlay_window::build()?;
    // Tier 3 — persistent tabbed workspace (Write / History / Tutor /
    // Compare / Settings). Built BEFORE the palette so we can hand its
    // weak handle to the palette's `cmd:workspace`/`cmd:settings` actions.
    // Normal taskbar window, resizable, NOT always-on-top. Close button
    // hides to tray — the app only quits from the tray.
    let workspace = workspace_window::build()?;
    // Tier 2 — transient command palette. Constructed but hidden — shown
    // from the tray, the overlay's PALETTE button, or Ctrl+Shift+P.
    let palette = palette_window::build(
        &modes,
        &chains,
        cmd_tx.clone(),
        workspace.as_weak(),
        overlay.as_weak(),
    )?;
    // Window icons are bound at Slint compile time via each Window's
    // `icon: @image-url(...)` declaration (see overlay.slint / palette.slint
    // / workspace.slint). Slint routes that to winit's set_window_icon,
    // which Windows reads for the taskbar entry of the running process.
    // Slint globals are per-window, so seed + install MUST target all
    // three windows. See `ui/bridge.rs` header for the full explanation.
    bridge::seed_bridge(&workspace, &overlay, &palette, &config, &modes, &chains);
    bridge::install_command_forwarder(
        &workspace,
        &overlay,
        Arc::clone(&state_for_bridge),
        cmd_tx.clone(),
        &config,
    );
    bridge::spawn_event_pump(
        &workspace,
        &overlay,
        &palette,
        Arc::clone(&state_for_bridge),
        event_rx,
        &rt_handle,
    );

    // Wire the overlay's PALETTE button to show the palette. The callback
    // is registered on the OVERLAY's AppBridge (per-window globals — each
    // window instantiates its own AppBridge).
    //
    // The overlay is intentionally NOT hidden when the palette opens: the
    // palette is `always-on-top` + frameless + opaque so it covers the
    // overlay's centre while open. Leaving the overlay up means that when
    // the user picks a mode/chain in the palette, the stream result has
    // a surface to render on. Previously we hid the overlay first, so
    // palette → mode → stream ran but the user saw nothing.
    {
        let palette_weak = palette.as_weak();
        overlay.global::<ui::AppBridge>().on_open_palette(move || {
            if let Some(p) = palette_weak.upgrade() {
                palette_window::center_on_screen(&p);
                let _ = p.show();
                palette_window::bring_to_front(&p);
            }
        });
    }

    // Pre-load history so the History tab is populated on first open.
    {
        let engine = engine.clone();
        rt_handle.spawn(async move {
            engine
                .handle_command(crate::state::UiCommand::LoadHistory { limit: 100 })
                .await;
        });
    }

    // Clipboard monitor — opt-in via `clipboard_monitor.enabled` in
    // user.yaml. Requires app restart to pick up runtime enable/disable
    // changes: we check the flag ONCE here at startup and only spawn the
    // monitor thread + drain task if enabled. Flipping the yaml value
    // later won't start a new monitor until the next boot.
    //
    // When enabled, we spawn a tokio task that drains the monitor's
    // channel and emits an informational toast each time fresh clipboard
    // text appears.
    if config.clipboard_monitor.enabled {
        let mut rx = crate::core::clipboard::start_clipboard_monitor();
        let engine_mon = engine.clone();
        rt_handle.spawn(async move {
            while let Some(text) = rx.recv().await {
                // Short preview so the toast doesn't wrap.
                let preview: String = text.chars().take(80).collect();
                let message = if preview.is_empty() {
                    "New clipboard text — press hotkey to transform".to_string()
                } else {
                    format!("New clipboard text: \"{preview}\" — press hotkey to transform")
                };
                engine_mon.emit(crate::state::UiEvent::Toast {
                    kind: crate::state::app_state::ToastKind::Info,
                    message,
                });
            }
        });
    }

    // Floating pencil indicator — appears next to the caret when the user
    // focuses an editable text control. Clicking it triggers the same flow
    // as the global hotkey.
    let pencil = pencil_window::build()?;
    let (caret_tx, caret_rx) = std::sync::mpsc::channel::<FocusEvent>();
    let _caret_service = CaretHookService::start(caret_tx).context("start caret hook service")?;
    let _pencil_controller = PencilController::start(&pencil, caret_rx);
    let _pencil_proximity_timer = pencil_window::install_proximity_toggle(&pencil);

    // Pencil click → trigger the same flow as the hotkey. We capture the
    // foreground HWND here on the Slint main thread BEFORE hopping to
    // tokio, because by the time the spawned future runs, the overlay may
    // already be in the middle of materializing (a race that caused the
    // snapshot taken inside `handle_hotkey` to point at the wrong HWND).
    {
        let engine = engine.clone();
        let rt_handle = rt_handle.clone();
        pencil.on_clicked(move || {
            use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
            let snapshot = unsafe {
                let hwnd = GetForegroundWindow();
                crate::state::FocusSnapshot {
                    hwnd_raw: hwnd.0 as isize,
                }
            };
            let engine = engine.clone();
            rt_handle.spawn(async move {
                crate::engine::hotkey_flow::handle_hotkey_with_focus(engine, Some(snapshot)).await;
            });
        });
    }

    // Drain UiCommand → engine.handle_command on tokio worker tasks.
    // Each command runs concurrently; cancellation is handled inside the engine.
    {
        let engine = engine.clone();
        rt_handle.spawn(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                let engine = engine.clone();
                tokio::spawn(async move {
                    engine.handle_command(cmd).await;
                });
            }
            tracing::debug!("UiCommand drain shutting down — channel closed");
        });
    }

    // Global hotkey — MUST live on the Slint main thread.
    //
    // Why: on Windows, `global-hotkey` installs a hidden message-only window
    // when `GlobalHotKeyManager::new()` is called. WM_HOTKEY is posted to that
    // window by the OS and only dispatched when the thread that created the
    // manager pumps messages. If we create the manager on a bare
    // `std::thread::spawn` (no message loop), presses are registered with
    // RegisterHotKey but WM_HOTKEY is never pumped → the crate's internal
    // receiver channel stays empty → `handle_hotkey` never runs.
    //
    // The Slint event loop pumps all messages on this thread, so creating
    // the manager here and draining `GlobalHotKeyEvent::receiver()` from a
    // `slint::Timer` (at ~60Hz) guarantees presses are delivered.
    // Treat `Some("")` the same as `None` — an empty string would reach
    // `HotkeyService::register` and fail silently (global-hotkey rejects
    // unparseable specs). Users who clear the field in Settings expect
    // the compiled-in default to take over; without this filter they'd
    // boot with no overlay hotkey at all.
    let hotkey_spec = config
        .hotkey
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Ctrl+Shift+Space")
        .to_string();
    let palette_hotkey_spec: Option<String> = config
        .hotkey_palette
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string());
    let _hotkey = build_hotkey_listener(
        &hotkey_spec,
        palette_hotkey_spec.as_deref(),
        engine.clone(),
        rt_handle.clone(),
        palette.as_weak(),
    )?;

    // Tray — TrayService wraps tray_icon::TrayIcon which is !Send, so we keep
    // it on the event-loop thread and poll it via a slint::Timer every 75 ms.
    // The `_tray` binding keeps the TrayService alive for the event-loop lifetime.
    let _tray = build_tray(
        workspace.as_weak(),
        palette.as_weak(),
        engine.clone(),
        Arc::clone(&state_for_bridge),
    )?;

    // First-run convenience: if the loaded config isn't usable (no API
    // key, unknown provider, etc.), pop the workspace's Settings tab on
    // boot instead of leaving the user staring at a silent hidden app.
    // Uses the normal summon path so the Slint event loop is the only
    // thread that touches the window.
    if !crate::core::config::config_is_usable(&config) {
        tracing::info!("config not usable at startup — opening Settings tab");
        summon_workspace(workspace.as_weak(), Some("settings"));
    }

    // Quill boots invisible (unless the first-run check above forced the
    // Settings tab open): we never call `show()` on any window at startup.
    // A window materializes later when the user interacts — the overlay on
    // hotkey/pencil press, the workspace on a tray click, the palette on
    // Ctrl+Shift+P or the overlay's PALETTE button. The bridge's event
    // pump and the tray summon path handle the actual `show()` call on
    // the Slint thread.
    //
    // CRITICAL: use `run_event_loop_until_quit` (NOT `window.run()` or
    // `run_event_loop`) — those exit as soon as the last visible window
    // closes, which with an invisible-on-startup window means immediately.
    // `run_event_loop_until_quit` only exits on explicit `quit_event_loop()`,
    // which we call from the tray's Quit menu item.
    //
    // The Slint event loop still processes tray_icon's hidden shell-notify
    // message window and the hotkey listener thread's tokio dispatches, so
    // every background subsystem stays alive.
    tracing::info!("Quill ready — entering Slint event loop (hidden, waiting for summon)");
    slint::run_event_loop_until_quit().context("Slint event loop returned an error")?;

    // Shutdown order matters, and this is the SPECIFIC real bug we fix:
    //
    // `_pencil_controller` is a `PencilController` whose worker thread
    // blocks on `caret_rx.blocking_recv()`. That `rx` only sees `None`
    // (and thus the thread only exits) once EVERY sender is dropped. The
    // matching sender is held inside `_caret_service`'s hook thread local.
    // If `_pencil_controller` drops BEFORE `_caret_service`, its `Drop`
    // calls `thread.join()` on a worker still parked on `blocking_recv`
    // with a live sender — join blocks forever and main never returns.
    //
    // Natural drop order (reverse declaration) would run
    // `_pencil_controller` BEFORE `_caret_service`, so we explicitly drop
    // in the correct order here.
    //
    // We also drop the two slint::Timer-bearing holders (`_hotkey`,
    // `_pencil_proximity_timer`) first so no timer callback fires during
    // teardown and touches a half-dead window weak. Then tray, then the
    // three Slint windows (releasing every `slint::Weak` held by tokio
    // tasks), then the tokio runtime.
    //
    // Note: `tokio::runtime::Runtime::drop` calls `shutdown_background()`
    // under the hood, which is non-blocking — it does NOT wait for tasks
    // to finish. So the drop order here is about releasing Slint Weak
    // handles cleanly and unblocking the caret→pencil channel, not about
    // tokio blocking on workers.
    tracing::info!("Quill exit — explicit drop order to avoid caret/pencil join hang");
    drop(_hotkey);
    drop(_pencil_proximity_timer);
    drop(_caret_service);
    drop(_pencil_controller);
    drop(_tray);
    drop(palette);
    drop(overlay);
    drop(workspace);
    drop(pencil);
    drop(rt);
    Ok(())
}

// ── Tracing ───────────────────────────────────────────────────────────────────

fn init_tracing() {
    // Always write logs to ~/.quill/quill.log.YYYY-MM-DD so release builds
    // (which have no console because of `windows_subsystem = "windows"`)
    // still leave a diagnostic trail. Uses the `daily` rolling appender:
    // every local-day boundary the appender closes the current file and
    // opens a fresh one with today's date suffix. This prevents the
    // single-file `never` appender from growing unbounded over long-running
    // sessions.
    //
    // Writes remain synchronous — `RollingFileAppender` implements
    // `MakeWriter` directly, so each log line flushes before returning and
    // the file looks live to an external tailer.
    let log_dir = dirs::home_dir().unwrap_or_default().join(".quill");
    let _ = std::fs::create_dir_all(&log_dir);

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("quill=debug"));

    let file_appender = tracing_appender::rolling::daily(&log_dir, "quill.log");

    // Log retention — cheap one-shot on every boot. List any
    // `quill.log.*` files in ~/.quill, sort lexicographically (the
    // `YYYY-MM-DD` suffix sorts correctly as text), and delete anything
    // older than the most recent 30. Failures are non-fatal.
    if let Ok(entries) = std::fs::read_dir(&log_dir) {
        let mut logs: Vec<std::path::PathBuf> = entries
            .filter_map(|e| e.ok().map(|d| d.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("quill.log."))
                    .unwrap_or(false)
            })
            .collect();
        logs.sort();
        let keep = 30;
        if logs.len() > keep {
            for old in logs.iter().take(logs.len() - keep) {
                let _ = std::fs::remove_file(old);
            }
        }
    }

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(file_appender)
        .with_ansi(false)
        .init();

    eprintln!(
        "quill: logging to {}/quill.log.<YYYY-MM-DD>",
        log_dir.display()
    );
}

// ── DPI awareness ─────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn enable_dpi_awareness() {
    use windows::Win32::UI::HiDpi::{
        SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };
    unsafe {
        if let Err(e) = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) {
            tracing::warn!("SetProcessDpiAwarenessContext failed: {e}");
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn enable_dpi_awareness() {}

// ── Hotkey listener ───────────────────────────────────────────────────────────

/// Build the `HotkeyService`(s) on the Slint main thread and install a
/// `slint::Timer` that drains `GlobalHotKeyEvent::receiver()` every 16 ms
/// (~60 Hz) on the same thread. Each press dispatches `handle_hotkey`
/// onto the tokio pool.
///
/// See the comment at the call site for why this MUST run on the main
/// thread and not a side `std::thread::spawn`.
///
/// The return shape is `(Timer, overlay_service, Option<palette_service>)`:
///
///   * `Timer` — the 60 Hz polling timer; dropping it halts event drain.
///   * `overlay_service` — always present; owns the overlay hotkey
///     registration (Ctrl+Shift+Space by default).
///   * `Option<HotkeyService>` — present iff the caller passed a
///     `palette_spec`. `HotkeyService` tracks only one binding at a time,
///     so the palette gets its own service/manager pair, and its own ID
///     to match against in the timer.
///
/// All three returned values MUST be kept alive for the event-loop
/// duration — dropping any one tears down either the hotkey
/// registration(s) or the polling timer. The order of the tuple
/// corresponds to the declaration order the caller should bind into
/// (Rust drops tuple fields in reverse declaration order, so the palette
/// service is dropped first, then the overlay service, then the timer —
/// matching registration/teardown expectations).
fn build_hotkey_listener(
    spec: &str,
    palette_spec: Option<&str>,
    engine: Engine,
    rt: tokio::runtime::Handle,
    palette_weak: slint::Weak<ui::PaletteWindow>,
) -> Result<(
    slint::Timer,
    crate::platform::hotkey::HotkeyService,
    Option<crate::platform::hotkey::HotkeyService>,
)> {
    use crate::platform::hotkey::{is_pressed, HotkeyService};
    use global_hotkey::GlobalHotKeyEvent;

    // Overlay hotkey (Ctrl+Shift+Space by default).
    let mut overlay_service = HotkeyService::new().context("HotkeyService::new (overlay)")?;
    if let Err(e) = overlay_service.register(spec) {
        tracing::warn!("register overlay hotkey `{spec}` failed (non-fatal): {e}");
    } else {
        tracing::info!("overlay hotkey registered: {spec}");
    }
    let overlay_id = overlay_service.current_id();

    // Palette hotkey (Ctrl+Shift+P by default). Separate service + manager
    // because HotkeyService only tracks one binding at a time; two
    // services give us two independent IDs we can match against.
    let (palette_service, palette_id) = match palette_spec {
        Some(s) => {
            let mut svc = HotkeyService::new().context("HotkeyService::new (palette)")?;
            match svc.register(s) {
                Ok(()) => {
                    tracing::info!("palette hotkey registered: {s}");
                    let id = svc.current_id();
                    (Some(svc), id)
                }
                Err(e) => {
                    tracing::warn!("register palette hotkey `{s}` failed (non-fatal): {e}");
                    (Some(svc), None)
                }
            }
        }
        None => (None, None),
    };

    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(16),
        move || {
            let receiver = GlobalHotKeyEvent::receiver();
            while let Ok(event) = receiver.try_recv() {
                let pressed = is_pressed(&event);
                tracing::debug!(?event, pressed, "hotkey event received");
                if !pressed {
                    continue;
                }
                // Dispatch by ID. The palette ID takes precedence if
                // somehow both matched (shouldn't happen — distinct
                // specs → distinct IDs).
                if palette_id == Some(event.id) {
                    summon_palette(palette_weak.clone());
                } else if overlay_id == Some(event.id) {
                    // Only dispatch to the overlay when the id DEFINITIVELY
                    // matches. The previous `|| overlay_id.is_none()` arm
                    // also caught unknown ids that weren't either registered
                    // binding — e.g. a press left in the receiver from a
                    // stale registration on restart would fire the overlay
                    // flow unconditionally. Dropping unknown ids is safer.
                    let engine = engine.clone();
                    rt.spawn(async move {
                        tracing::debug!("dispatching handle_hotkey");
                        crate::engine::hotkey_flow::handle_hotkey(engine).await;
                    });
                }
            }
        },
    );
    tracing::info!("hotkey listener installed on main thread");
    // Return the timer FIRST so `drop(_hotkey)` in main tears the timer
    // down before the HotkeyServices — no stale timer tick can fire
    // against a half-dropped receiver during shutdown.
    Ok((timer, overlay_service, palette_service))
}

// ── Summon ────────────────────────────────────────────────────────────────────

/// Bring the workspace (Tier 3) on-screen from anywhere, optionally
/// switching its active tab. Safe to call from any thread — the actual
/// work is posted onto the Slint event loop.
fn summon_workspace(
    workspace_weak: slint::Weak<ui::WorkspaceWindow>,
    focus_tab: Option<&'static str>,
) {
    let _ = slint::invoke_from_event_loop(move || {
        let Some(window) = workspace_weak.upgrade() else {
            return;
        };
        if let Some(tab) = focus_tab {
            window.global::<ui::AppBridge>().set_current_tab(tab.into());
        }
        let _ = window.show();
        crate::ui::workspace_window::reapply_visual_treatment(&window);
        crate::ui::workspace_window::bring_to_front(&window);
    });
}

/// Show the palette (Tier 2), center it on the primary monitor, and
/// give it foreground focus so the user can start typing in the search
/// input without a mouse click. Safe to call from any thread.
fn summon_palette(palette_weak: slint::Weak<ui::PaletteWindow>) {
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(p) = palette_weak.upgrade() {
            crate::ui::palette_window::center_on_screen(&p);
            let _ = p.show();
            crate::ui::palette_window::bring_to_front(&p);
        }
    });
}

/// Show the overlay (Tier 1) without a hotkey capture — used by the tray's
/// "Show Overlay" item when the user just wants a scratch prompt.
///
/// Implementation: clear `state.focus_target` (stale HWND from a prior
/// hotkey fires Replace into the wrong app) and emit a ShowOverlay event
/// with empty text and no anchor. The event pump's existing ShowOverlay
/// handler centers the overlay, shows it, and broadcasts the clean state
/// into ALL THREE windows' AppBridges (not just the overlay's own).
fn summon_overlay(engine: Engine, state: Arc<Mutex<AppState>>) {
    // Align with the hotkey-summon path: wipe the previous capture's
    // per-session state entirely before showing a blank scratch overlay.
    // `reset_session` clears selected_text, stream buffer, last_result,
    // etc. We then also clear the stale focus target and mark the overlay
    // visible.
    if let Ok(mut s) = state.lock() {
        s.reset_session();
        s.focus_target = None;
    }
    engine.emit(UiEvent::ShowOverlay {
        text: String::new(),
        context: crate::platform::context::AppContext::default(),
        suggestion: None,
        anchor_rect: None,
    });
}

// ── Tray ──────────────────────────────────────────────────────────────────────

/// Build the `TrayService` and install a `slint::Timer` that polls for tray
/// events every 75 ms on the event-loop thread.
///
/// `tray_icon::TrayIcon` (held inside `TrayService`) is `!Send`. Keeping the
/// poll loop on the Slint event-loop thread avoids any cross-thread transfer.
///
/// The returned `(TrayService, slint::Timer)` tuple MUST be kept alive in the
/// caller for the entire event-loop duration — dropping either removes the
/// tray icon or stops the poll timer.
fn build_tray(
    workspace_weak: slint::Weak<ui::WorkspaceWindow>,
    palette_weak: slint::Weak<ui::PaletteWindow>,
    engine: Engine,
    state: Arc<Mutex<AppState>>,
) -> Result<(std::rc::Rc<TrayService>, slint::Timer)> {
    // `TrayService` is `!Send` (wraps a `tray_icon::TrayIcon`). We keep it
    // on the Slint event-loop thread and share it with the poll timer via
    // an `Rc`. An earlier version used `*const TrayService` into a Boxed
    // stack slot — memory-safe only by heap-allocation coincidence. `Rc`
    // is the idiomatic single-threaded alternative: no raw pointers, no
    // lifetime juggling, and the compiler enforces both holders live long
    // enough.
    let service: std::rc::Rc<TrayService> =
        std::rc::Rc::new(TrayService::new(TRAY_ICON_PNG).context("TrayService::new")?);
    tracing::info!("tray icon created");

    let service_for_timer = std::rc::Rc::clone(&service);
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(75),
        move || {
            // Rc is fine here: Slint timers fire on the event-loop thread,
            // same thread that owns the other `Rc` handle in main(). No
            // cross-thread access.
            let service = &*service_for_timer;
            for event in service.poll() {
                tracing::debug!(?event, "tray event received");
                match event {
                    crate::platform::tray::TrayEvent::IconClicked => {
                        // Left-click the tray icon → open the workspace,
                        // the most productive surface.
                        summon_workspace(workspace_weak.clone(), None);
                    }
                    crate::platform::tray::TrayEvent::MenuItem(menu_item) => match menu_item {
                        TrayMenu::Quit => {
                            tracing::info!("tray menu → Quit");
                            let _ = slint::invoke_from_event_loop(|| {
                                let _ = slint::quit_event_loop();
                            });
                        }
                        TrayMenu::Show => {
                            tracing::debug!("tray menu → Show Overlay");
                            summon_overlay(engine.clone(), Arc::clone(&state));
                        }
                        TrayMenu::Palette => {
                            tracing::debug!("tray menu → Command Palette");
                            summon_palette(palette_weak.clone());
                        }
                        TrayMenu::Workspace => {
                            tracing::debug!("tray menu → Open Workspace");
                            summon_workspace(workspace_weak.clone(), Some("write"));
                        }
                        TrayMenu::Settings => {
                            tracing::debug!("tray menu → Settings");
                            summon_workspace(workspace_weak.clone(), Some("settings"));
                        }
                    },
                }
            }
        },
    );

    Ok((service, timer))
}
