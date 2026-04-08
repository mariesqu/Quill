#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod core;
mod engine;
mod platform;
mod providers;

use std::sync::{Arc, Mutex};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

use core::clipboard::start_clipboard_monitor;
use core::config::{config_is_usable, load_config};
use core::history::init_db;
use core::hotkey::register_hotkey;
use core::modes::load_modes;
use engine::{Engine, SharedEngine};

fn build_tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let show = MenuItem::with_id(app, "show", "Show Quill", true, None::<&str>)?;
    let panel = MenuItem::with_id(app, "panel", "Open Full Panel…", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let sep = tauri::menu::PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Quill", true, None::<&str>)?;
    Menu::with_items(app, &[&show, &panel, &settings, &sep, &quit])
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("quill=info".parse().unwrap()),
        )
        .init();

    let cfg = load_config();
    let (modes, chains) = load_modes(&cfg);

    // Init history DB if enabled
    if cfg.history.enabled {
        if let Err(e) = init_db() {
            eprintln!("[history] init error: {e}");
        }
    }

    let engine: SharedEngine = Arc::new(Mutex::new(Engine::new(cfg, modes, chains)));

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(engine.clone())
        .invoke_handler(tauri::generate_handler![
            commands::execute_mode,
            commands::execute_chain,
            commands::retry,
            commands::confirm_replace,
            commands::set_result,
            commands::set_selected_text,
            commands::dismiss,
            commands::open_full_panel,
            commands::close_full_panel,
            commands::request_tutor_explain,
            commands::generate_lesson,
            commands::compare_modes_cmd,
            commands::get_pronunciation,
            commands::get_history,
            commands::toggle_favorite,
            commands::export_history,
            commands::save_config,
            commands::save_template,
            commands::delete_template,
            commands::get_config,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            // ── System tray ───────────────────────────────────────────────────
            let menu = build_tray_menu(&handle)?;
            TrayIconBuilder::new()
                .menu(&menu)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("mini") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("mini") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "panel" => {
                        if let Some(w) = app.get_webview_window("full") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "settings" => {
                        let _ = app.emit("quill://open_settings", ());
                        if let Some(w) = app.get_webview_window("full") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // ── Global hotkey ─────────────────────────────────────────────────
            let hotkey_str = { engine.lock().unwrap().config.hotkey.clone() };
            if let Err(err) = register_hotkey(&handle, engine.clone(), hotkey_str.as_deref()) {
                eprintln!("[hotkey] registration failed: {err}");
                // The error toast was already emitted via quill://error by
                // register_hotkey, but the mini window is hidden and only
                // shows errors when visible. Open the full panel so the
                // user can see the conflict message and fix it in Settings.
                if let Some(w) = handle.get_webview_window("full") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }

            // ── Emit templates on startup ─────────────────────────────────────
            {
                let templates = engine.lock().unwrap().config.templates.clone();
                let _ = handle.emit(
                    "quill://templates_updated",
                    serde_json::json!({"templates": templates}),
                );
            }

            // ── Clipboard monitor ─────────────────────────────────────────────
            let enabled_flag = engine.lock().unwrap().clipboard_monitor_running.clone();
            let mut rx = start_clipboard_monitor(enabled_flag);
            let handle_cm = handle.clone();

            tauri::async_runtime::spawn(async move {
                while let Some(text) = rx.recv().await {
                    let _ = handle_cm.emit(
                        "quill://clipboard_change",
                        serde_json::json!({"text": text}),
                    );
                }
            });

            // ── First-run bootstrap ───────────────────────────────────────────
            // If the config is not usable (missing provider, or missing API key
            // for a non-local provider), force-open the Full Panel window so
            // the FirstRun wizard is reachable. We intentionally check the
            // EFFECTIVE config — a stale or half-written `user.yaml` that
            // merely EXISTS is not enough; the user still needs the wizard.
            //
            // Without this, a fresh user (or one with an incomplete config)
            // presses Ctrl+Shift+Space → only the mini window shows →
            // `App.jsx` renders null because setup isn't complete → the wizard
            // (which only renders in the full window) is unreachable.
            if !config_is_usable(&engine.lock().unwrap().config) {
                if let Some(w) = handle.get_webview_window("full") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Quill failed to start");
}
