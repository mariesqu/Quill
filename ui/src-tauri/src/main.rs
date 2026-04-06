// Quill — Tauri main entry point
// Bridges the React UI ↔ Python sidecar via stdin/stdout JSON IPC.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use tauri::{
    AppHandle, Emitter, Manager, State,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

/// Holds the handle to the running Python sidecar process.
struct PythonSidecar {
    child: Mutex<Option<CommandChild>>,
    ready: Arc<AtomicBool>,
}

/// Send a JSON message to the Python sidecar's stdin.
#[tauri::command]
async fn send_to_python(
    message: String,
    sidecar: State<'_, PythonSidecar>,
) -> Result<(), String> {
    if !sidecar.ready.load(Ordering::Acquire) {
        return Err("Sidecar not ready yet".to_string());
    }
    let mut guard = sidecar.child.lock().unwrap();
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

    let sidecar_cmd = match app.shell().sidecar("quill-core") {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("[quill] quill-core sidecar not found: {}", e);
            let _ = app_handle.emit("quill://error", serde_json::json!({"type": "error", "message": "Python sidecar not found. Please reinstall Quill."}));
            return;
        }
    };

    let (mut rx, child) = match sidecar_cmd.spawn() {
        Ok(result) => result,
        Err(e) => {
            eprintln!("[quill] Failed to spawn quill-core: {}", e);
            let _ = app_handle.emit("quill://error", serde_json::json!({"type": "error", "message": "Failed to start Python core. Check logs."}));
            return;
        }
    };

    *sidecar_state.child.lock().unwrap() = Some(child);
    let ready_flag = Arc::clone(&sidecar_state.ready);

    // Spawn an async task to relay sidecar stdout → frontend events
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    if let Ok(text) = String::from_utf8(line) {
                        relay_message(&app_handle, &ready_flag, text.trim());
                    }
                }
                CommandEvent::Stderr(line) => {
                    if let Ok(text) = String::from_utf8(line) {
                        eprintln!("[quill-core] {}", text.trim());
                    }
                }
                CommandEvent::Terminated(status) => {
                    eprintln!("[quill-core] Process exited: {:?}", status);
                    ready_flag.store(false, Ordering::Release);
                    let _ = app_handle.emit("quill://error",
                        serde_json::json!({"type": "error", "message": "Python backend stopped unexpectedly. Please restart Quill."}));
                    break;
                }
                _ => {}
            }
        }
    });
}

/// Parse a JSON line from Python and emit the appropriate Tauri event.
fn relay_message(app: &AppHandle, ready_flag: &Arc<AtomicBool>, line: &str) {
    if line.is_empty() {
        return;
    }

    let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) else {
        eprintln!("[relay] Failed to parse JSON: {line}");
        return;
    };

    let msg_type = msg.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match msg_type {
        // ── Overlay core ─────────────────────────────────────────────────────
        "show_overlay" => {
            let _ = app.emit("quill://show_overlay", &msg);
        }
        "stream_chunk" => {
            let _ = app.emit("quill://stream_chunk", &msg);
        }
        "stream_done" => {
            let _ = app.emit("quill://stream_done", &msg);
        }
        // ── Mode chaining ─────────────────────────────────────────────────────
        "chain_step" => {
            let _ = app.emit("quill://chain_step", &msg);
        }
        // ── Smart suggestion ──────────────────────────────────────────────────
        "smart_suggestion" => {
            let _ = app.emit("quill://smart_suggestion", &msg);
        }
        // ── AI Tutor ──────────────────────────────────────────────────────────
        "tutor_explanation" => {
            let _ = app.emit("quill://tutor_explanation", &msg);
        }
        "tutor_lesson" => {
            let _ = app.emit("quill://tutor_lesson", &msg);
        }
        "history" => {
            let _ = app.emit("quill://history", &msg);
        }
        // ── New features ──────────────────────────────────────────────────────
        "favorite_toggled" => {
            let _ = app.emit("quill://favorite_toggled", &msg);
        }
        "export_data" => {
            let _ = app.emit("quill://export_data", &msg);
        }
        "comparison_done" => {
            let _ = app.emit("quill://comparison_done", &msg);
        }
        "pronunciation" => {
            let _ = app.emit("quill://pronunciation", &msg);
        }
        "clipboard_change" => {
            let _ = app.emit("quill://clipboard_change", &msg);
        }
        "templates_updated" => {
            let _ = app.emit("quill://templates_updated", &msg);
        }
        // ── System ────────────────────────────────────────────────────────────
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
            ready_flag.store(true, Ordering::Release);
            eprintln!("[quill] Python sidecar ready");
        }
        _ => {
            eprintln!("[relay] Unknown message type: {msg_type} — {}", &line[..line.len().min(200)]);
        }
    }
}

const MENU_SHOW: &str = "show";
const MENU_TUTOR: &str = "tutor";
const MENU_SETTINGS: &str = "settings";
const MENU_QUIT: &str = "quit";

fn build_tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let show     = MenuItem::with_id(app, MENU_SHOW,     "Show Quill",   true, None::<&str>)?;
    let tutor    = MenuItem::with_id(app, MENU_TUTOR,    "AI Tutor…",    true, None::<&str>)?;
    let settings = MenuItem::with_id(app, MENU_SETTINGS, "Settings…",    true, None::<&str>)?;
    let sep      = tauri::menu::PredefinedMenuItem::separator(app)?;
    let quit     = MenuItem::with_id(app, MENU_QUIT,     "Quit Quill",   true, None::<&str>)?;
    Menu::with_items(app, &[&show, &tutor, &settings, &sep, &quit])
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(PythonSidecar {
            child: Mutex::new(None),
            ready: Arc::new(AtomicBool::new(false)),
        })
        .invoke_handler(tauri::generate_handler![
            send_to_python,
            open_accessibility_settings,
        ])
        .setup(|app| {
            // Build system tray
            let menu = build_tray_menu(app.handle())?;
            let icon = app.default_window_icon().cloned().unwrap();
            TrayIconBuilder::new()
                .icon(icon)
                .tooltip("Quill")
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
                    MENU_SHOW => {
                        if let Some(window) = app.get_webview_window("overlay") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    MENU_TUTOR => {
                        let _ = app.emit("quill://open_tutor", ());
                        if let Some(window) = app.get_webview_window("overlay") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    MENU_SETTINGS => {
                        let _ = app.emit("quill://open_settings", ());
                        if let Some(window) = app.get_webview_window("overlay") {
                            let _ = window.show();
                        }
                    }
                    MENU_QUIT => {
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
