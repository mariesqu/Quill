// Quill — Tauri main entry point
// Bridges the React UI ↔ Python sidecar via stdin/stdout JSON IPC.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use tauri::{
    AppHandle, Emitter, Manager, State,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

/// Holds the handle to the running Python sidecar process.
struct PythonSidecar(Mutex<Option<CommandChild>>);

/// Send a JSON message to the Python sidecar's stdin.
#[tauri::command]
async fn send_to_python(
    message: String,
    sidecar: State<'_, PythonSidecar>,
) -> Result<(), String> {
    let mut guard = sidecar.0.lock().unwrap();
    if let Some(child) = guard.as_mut() {
        let msg = format!("{}\n", message);
        child
            .write(msg.as_bytes())
            .map_err(|e| format!("Failed to write to sidecar: {e}"))?;
    }
    Ok(())
}

/// Open macOS Accessibility settings (no-op on other platforms).
#[tauri::command]
async fn open_accessibility_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Spawn the Python sidecar and wire up its stdout → Tauri events.
fn start_python_sidecar(app: &AppHandle) {
    let sidecar_state = app.state::<PythonSidecar>();
    let app_handle = app.clone();

    let sidecar_cmd = app
        .shell()
        .sidecar("quill-core")
        .expect("quill-core sidecar not found");

    let (mut rx, child) = sidecar_cmd.spawn().expect("Failed to spawn quill-core");

    *sidecar_state.0.lock().unwrap() = Some(child);

    // Spawn an async task to relay sidecar stdout → frontend events
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    if let Ok(text) = String::from_utf8(line) {
                        relay_message(&app_handle, text.trim());
                    }
                }
                CommandEvent::Stderr(line) => {
                    if let Ok(text) = String::from_utf8(line) {
                        eprintln!("[quill-core] {}", text.trim());
                    }
                }
                CommandEvent::Terminated(status) => {
                    eprintln!("[quill-core] Process exited: {:?}", status);
                    break;
                }
                _ => {}
            }
        }
    });
}

/// Parse a JSON line from Python and emit the appropriate Tauri event.
fn relay_message(app: &AppHandle, line: &str) {
    if line.is_empty() {
        return;
    }

    let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) else {
        eprintln!("[relay] Failed to parse JSON: {line}");
        return;
    };

    let msg_type = msg.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match msg_type {
        "show_overlay" => {
            let _ = app.emit("quill://show_overlay", &msg);
        }
        "stream_chunk" => {
            let _ = app.emit("quill://stream_chunk", &msg);
        }
        "stream_done" => {
            let _ = app.emit("quill://stream_done", &msg);
        }
        "error" => {
            let _ = app.emit("quill://error", &msg);
        }
        "permission_required" => {
            let permission = msg
                .get("permission")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let _ = app.emit("quill://permission_required", permission);
        }
        "ready" => {
            eprintln!("[quill] Python sidecar ready");
        }
        _ => {
            eprintln!("[relay] Unknown message type: {msg_type}");
        }
    }
}

fn build_tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let show = MenuItem::with_id(app, "show", "Show Quill", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let separator = tauri::menu::PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Quill", true, None::<&str>)?;
    Menu::with_items(app, &[&show, &settings, &separator, &quit])
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(PythonSidecar(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            send_to_python,
            open_accessibility_settings,
        ])
        .setup(|app| {
            // Build system tray
            let menu = build_tray_menu(app.handle())?;
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
                        if let Some(window) = app.get_webview_window("overlay") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("overlay") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "settings" => {
                        let _ = app.emit("quill://open_settings", ());
                        if let Some(window) = app.get_webview_window("overlay") {
                            let _ = window.show();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            // Start Python sidecar
            start_python_sidecar(app.handle());

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Quill");
}
