use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::core::{config, history};
use crate::core::modes::load_modes;
use crate::engine::{self, SharedEngine};
use crate::platform::replace;

// ── Mode execution ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn execute_mode(
    mode:               String,
    language:           String,
    extra_instruction:  Option<String>,
    engine:             State<'_, SharedEngine>,
    app:                AppHandle,
) -> Result<(), String> {
    let eng = engine.inner().clone();
    tokio::spawn(async move {
        crate::engine::execute_mode(eng, app, mode, language, extra_instruction).await;
    });
    Ok(())
}

#[tauri::command]
pub async fn execute_chain(
    chain_id:           String,
    language:           String,
    extra_instruction:  Option<String>,
    engine:             State<'_, SharedEngine>,
    app:                AppHandle,
) -> Result<(), String> {
    let eng = engine.inner().clone();
    tokio::spawn(async move {
        crate::engine::execute_chain(eng, app, chain_id, language, extra_instruction).await;
    });
    Ok(())
}

#[tauri::command]
pub async fn retry(
    extra_instruction: Option<String>,
    engine:            State<'_, SharedEngine>,
    app:               AppHandle,
) -> Result<(), String> {
    let (mode, language) = {
        let e = engine.lock().unwrap();
        (e.last_mode.clone(), e.last_language.clone())
    };
    if mode.is_empty() { return Ok(()); }

    if mode.starts_with("chain:") {
        let chain_id = mode.trim_start_matches("chain:").to_string();
        let eng = engine.inner().clone();
        tokio::spawn(async move {
            crate::engine::execute_chain(eng, app, chain_id, language, extra_instruction).await;
        });
    } else {
        let eng = engine.inner().clone();
        tokio::spawn(async move {
            crate::engine::execute_mode(eng, app, mode, language, extra_instruction).await;
        });
    }
    Ok(())
}

// ── Output control ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn confirm_replace(engine: State<'_, SharedEngine>) -> Result<(), String> {
    let text = engine.lock().unwrap().last_result.clone();
    if text.is_empty() { return Ok(()); }
    tokio::task::spawn_blocking(move || replace::paste_text(&text))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn set_result(text: String, engine: State<'_, SharedEngine>) -> Result<(), String> {
    engine.lock().unwrap().last_result = text;
    Ok(())
}

#[tauri::command]
pub fn dismiss(engine: State<'_, SharedEngine>, app: AppHandle) -> Result<(), String> {
    engine.lock().unwrap().cancel_stream();
    if let Some(w) = app.get_webview_window("mini") { let _ = w.hide(); }
    Ok(())
}

// ── Window management ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn open_full_panel(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("full") {
        let _ = w.show();
        let _ = w.set_focus();
    }
    Ok(())
}

#[tauri::command]
pub fn close_full_panel(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("full") { let _ = w.hide(); }
    Ok(())
}

// ── Tutor & AI features ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn request_tutor_explain(
    entry_id: Option<i64>,
    engine:   State<'_, SharedEngine>,
    app:      AppHandle,
) -> Result<(), String> {
    let (eid, orig, out, mode, lang) = {
        let e = engine.lock().unwrap();
        let eid = entry_id.or(e.last_entry_id);
        (eid, e.last_text.clone(), e.last_result.clone(), e.last_mode.clone(), e.last_language.clone())
    };

    if let Some(id) = eid {
        // Try to load entry from DB
        if let Ok(entries) = history::get_recent(1, None) {
            if let Some(entry) = entries.iter().find(|e| e.id == id) {
                let eng = engine.inner().clone();
                let orig2 = entry.original_text.clone();
                let out2  = entry.output_text.clone();
                let mode2 = entry.mode.clone().unwrap_or_default();
                let lang2 = entry.language.clone().unwrap_or_default();
                tokio::spawn(async move {
                    engine::explain_entry(eng, app, id, orig2, out2, mode2, lang2).await;
                });
                return Ok(());
            }
        }
    }

    // Fallback: use last in-memory values
    let eng = engine.inner().clone();
    let eid = engine.lock().unwrap().last_entry_id.unwrap_or(0);
    tokio::spawn(async move {
        engine::explain_entry(eng, app, eid, orig, out, mode, lang).await;
    });
    Ok(())
}

#[tauri::command]
pub async fn generate_lesson(
    period: String,
    engine: State<'_, SharedEngine>,
    app:    AppHandle,
) -> Result<(), String> {
    let eng = engine.inner().clone();
    tokio::spawn(async move {
        crate::engine::generate_lesson(eng, app, period).await;
    });
    Ok(())
}

#[tauri::command]
pub async fn compare_modes_cmd(
    mode_a:            String,
    mode_b:            String,
    language:          String,
    extra_instruction: Option<String>,
    engine:            State<'_, SharedEngine>,
    app:               AppHandle,
) -> Result<(), String> {
    let eng = engine.inner().clone();
    tokio::spawn(async move {
        crate::engine::compare_modes(eng, app, mode_a, mode_b, language, extra_instruction).await;
    });
    Ok(())
}

#[tauri::command]
pub async fn get_pronunciation(
    text:     String,
    language: String,
    engine:   State<'_, SharedEngine>,
    app:      AppHandle,
) -> Result<(), String> {
    let eng = engine.inner().clone();
    tokio::spawn(async move {
        crate::engine::get_pronunciation(eng, app, text, language).await;
    });
    Ok(())
}

// ── History ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_history(
    limit:    Option<usize>,
    language: Option<String>,
    app:      AppHandle,
) -> Result<(), String> {
    let entries = history::get_recent(limit.unwrap_or(50), language.as_deref())
        .map_err(|e| e.to_string())?;
    let _ = app.emit("quill://history", json!({"entries": entries, "update": null}));
    Ok(())
}

#[tauri::command]
pub fn toggle_favorite(entry_id: i64, app: AppHandle) -> Result<(), String> {
    let new_state = history::toggle_favorite(entry_id).map_err(|e| e.to_string())?;
    let _ = app.emit("quill://favorite_toggled", json!({"entry_id": entry_id, "favorited": new_state}));
    Ok(())
}

#[tauri::command]
pub fn export_history(format: String, app: AppHandle) -> Result<(), String> {
    let entries = history::get_all_entries().map_err(|e| e.to_string())?;
    let _ = app.emit("quill://export_data", json!({"entries": entries, "format": format}));
    Ok(())
}

// ── Config & templates ────────────────────────────────────────────────────────

#[tauri::command]
pub fn save_config(
    config_update: Value,
    engine:        State<'_, SharedEngine>,
    app:           AppHandle,
) -> Result<(), String> {
    config::save_user_config(config_update).map_err(|e| e.to_string())?;

    let new_cfg = config::load_config();
    let (new_modes, new_chains) = load_modes(&new_cfg);

    {
        let mut e = engine.lock().unwrap();
        let was_enabled = e.clipboard_monitor_running.load(std::sync::atomic::Ordering::Relaxed);
        let now_enabled = new_cfg.clipboard_monitor.enabled;
        e.clipboard_monitor_running.store(now_enabled, std::sync::atomic::Ordering::Relaxed);

        if new_cfg.history.enabled {
            if let Err(err) = history::init_db() {
                eprintln!("[history] init error: {err}");
            }
        }
        e.config = new_cfg.clone();
        e.modes  = new_modes;
        e.chains = new_chains;
    }

    // Emit updated templates to frontend
    let _ = app.emit("quill://templates_updated", json!({"templates": new_cfg.templates}));
    Ok(())
}

#[tauri::command]
pub fn save_template(
    name:        String,
    mode:        String,
    instruction: String,
    engine:      State<'_, SharedEngine>,
    app:         AppHandle,
) -> Result<(), String> {
    let mut e = engine.lock().unwrap();
    e.config.templates.retain(|t| t.name != name);
    e.config.templates.push(crate::core::config::Template { name, mode, instruction });
    let templates = e.config.templates.clone();
    let update = json!({"templates": templates});
    drop(e);
    config::save_user_config(update).map_err(|e| e.to_string())?;
    let templates = engine.lock().unwrap().config.templates.clone();
    let _ = app.emit("quill://templates_updated", json!({"templates": templates}));
    Ok(())
}

#[tauri::command]
pub fn delete_template(
    name:   String,
    engine: State<'_, SharedEngine>,
    app:    AppHandle,
) -> Result<(), String> {
    let mut e = engine.lock().unwrap();
    e.config.templates.retain(|t| t.name != name);
    let templates = e.config.templates.clone();
    let update = json!({"templates": templates});
    drop(e);
    config::save_user_config(update).map_err(|e| e.to_string())?;
    let templates = engine.lock().unwrap().config.templates.clone();
    let _ = app.emit("quill://templates_updated", json!({"templates": templates}));
    Ok(())
}

// ── Platform ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn open_accessibility_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_config(engine: State<'_, SharedEngine>) -> Value {
    let e = engine.lock().unwrap();
    serde_json::to_value(&e.config).unwrap_or_default()
}
