#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod core;
mod engine;
mod platform;
mod providers;

use std::sync::{Arc, Mutex};
use std::sync::atomic::Ordering;

use tauri::{AppHandle, Emitter, Manager};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use core::config::load_config;
use core::history::init_db;
use core::modes::load_modes;
use core::clipboard::start_clipboard_monitor;
use engine::{Engine, SharedEngine, handle_hotkey};

fn build_tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let show     = MenuItem::with_id(app, "show",     "Show Quill",      true, None::<&str>)?;
    let panel    = MenuItem::with_id(app, "panel",    "Open Full Panel…", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…",       true, None::<&str>)?;
    let sep      = tauri::menu::PredefinedMenuItem::separator(app)?;
    let quit     = MenuItem::with_id(app, "quit",     "Quit Quill",      true, None::<&str>)?;
    Menu::with_items(app, &[&show, &panel, &settings, &sep, &quit])
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("quill=info".parse().unwrap()))
        .init();

    let cfg = load_config();
    let (modes, chains) = load_modes(&cfg);

    // Init history DB if enabled
    if cfg.history.enabled {
        if let Err(e) = init_db() { eprintln!("[history] init error: {e}"); }
    }

    let engine: SharedEngine = Arc::new(Mutex::new(Engine::new(cfg, modes, chains)));

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(engine.clone())
        .invoke_handler(tauri::generate_handler![
            commands::execute_mode,
            commands::execute_chain,
            commands::retry,
            commands::confirm_replace,
            commands::set_result,
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
            commands::open_accessibility_settings,
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
                        button_state: MouseButtonState::Up, ..
                    } = event {
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
            let hotkey_str = {
                engine.lock().unwrap().config.hotkey.clone()
            };

            let shortcut = parse_hotkey(hotkey_str.as_deref());
            let engine_hk = engine.clone();
            let handle_hk = handle.clone();

            app.handle().global_shortcut().on_shortcut(shortcut, move |_app, _sc, event| {
                if event.state() == ShortcutState::Pressed {
                    let eng = engine_hk.clone();
                    let app = handle_hk.clone();
                    tauri::async_runtime::spawn(async move {
                        handle_hotkey(eng, app).await;
                    });
                }
            })?;

            // ── Emit templates on startup ─────────────────────────────────────
            {
                let templates = engine.lock().unwrap().config.templates.clone();
                let _ = handle.emit("quill://templates_updated", serde_json::json!({"templates": templates}));
            }

            // ── Clipboard monitor ─────────────────────────────────────────────
            let enabled_flag = engine.lock().unwrap().clipboard_monitor_running.clone();
            let mut rx = start_clipboard_monitor(enabled_flag);
            let handle_cm = handle.clone();

            tauri::async_runtime::spawn(async move {
                while let Some(text) = rx.recv().await {
                    let _ = handle_cm.emit("quill://clipboard_change", serde_json::json!({"text": text}));
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Quill failed to start");
}

/// Parse a hotkey string like "ctrl+shift+space" into a Tauri Shortcut.
fn parse_hotkey(s: Option<&str>) -> Shortcut {
    let s = s.unwrap_or("");

    // Detect OS default
    #[cfg(target_os = "macos")]
    let default_modifiers = Modifiers::META | Modifiers::SHIFT;
    #[cfg(not(target_os = "macos"))]
    let default_modifiers = Modifiers::CONTROL | Modifiers::SHIFT;

    if s.is_empty() {
        return Shortcut::new(Some(default_modifiers), Code::Space);
    }

    let lower = s.to_lowercase();
    let parts: Vec<&str> = lower.split('+').map(str::trim).collect();

    let mut modifiers = Modifiers::empty();
    let mut code = Code::Space;

    for part in &parts {
        match *part {
            "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
            "cmd" | "meta"     => modifiers |= Modifiers::META,
            "shift"            => modifiers |= Modifiers::SHIFT,
            "alt" | "option"   => modifiers |= Modifiers::ALT,
            key => {
                code = match key {
                    "space" => Code::Space,
                    "a"..="z" => {
                        let c = key.chars().next().unwrap();
                        match c {
                            'a' => Code::KeyA, 'b' => Code::KeyB, 'c' => Code::KeyC,
                            'd' => Code::KeyD, 'e' => Code::KeyE, 'f' => Code::KeyF,
                            'g' => Code::KeyG, 'h' => Code::KeyH, 'i' => Code::KeyI,
                            'j' => Code::KeyJ, 'k' => Code::KeyK, 'l' => Code::KeyL,
                            'm' => Code::KeyM, 'n' => Code::KeyN, 'o' => Code::KeyO,
                            'p' => Code::KeyP, 'q' => Code::KeyQ, 'r' => Code::KeyR,
                            's' => Code::KeyS, 't' => Code::KeyT, 'u' => Code::KeyU,
                            'v' => Code::KeyV, 'w' => Code::KeyW, 'x' => Code::KeyX,
                            'y' => Code::KeyY, 'z' => Code::KeyZ,
                            _ => Code::Space,
                        }
                    }
                    _ => Code::Space,
                };
            }
        }
    }

    Shortcut::new(if modifiers.is_empty() { None } else { Some(modifiers) }, code)
}
