use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};

use std::sync::LazyLock;

use crate::core::hotkey::register_hotkey;
use crate::core::modes::load_modes;
use crate::core::{config, history};
use crate::engine::{self, SharedEngine};
use crate::platform::replace;

/// Process-wide serialization mutex for `save_config`. Tauri dispatches
/// commands concurrently — if two Settings saves race, the
/// write-then-reload-then-swap sequence in each can interleave and clobber
/// each other's view of the config. Taking this async mutex at the top of
/// `save_config` makes the sequence atomic without blocking any other
/// command (only other `save_config` calls wait).
static SAVE_CONFIG_LOCK: LazyLock<tokio::sync::Mutex<()>> =
    LazyLock::new(|| tokio::sync::Mutex::new(()));

// ── Mode execution ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn execute_mode(
    mode: String,
    language: String,
    extra_instruction: Option<String>,
    engine: State<'_, SharedEngine>,
    app: AppHandle,
) -> Result<(), String> {
    let eng = engine.inner().clone();
    tokio::spawn(async move {
        crate::engine::execute_mode(eng, app, mode, language, extra_instruction).await;
    });
    Ok(())
}

#[tauri::command]
pub async fn execute_chain(
    chain_id: String,
    language: String,
    extra_instruction: Option<String>,
    engine: State<'_, SharedEngine>,
    app: AppHandle,
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
    engine: State<'_, SharedEngine>,
    app: AppHandle,
) -> Result<(), String> {
    let (mode, language) = {
        let e = engine.lock().unwrap();
        (e.last_mode.clone(), e.last_language.clone())
    };
    if mode.is_empty() {
        return Ok(());
    }

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
    if text.is_empty() {
        return Ok(());
    }
    tokio::task::spawn_blocking(move || replace::paste_text(&text))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn set_result(text: String, engine: State<'_, SharedEngine>) -> Result<(), String> {
    engine.lock().unwrap().last_result = text;
    Ok(())
}

/// Replace the engine's "currently selected text" with the given value and
/// open a fresh mini-overlay session around it. Used by the clipboard-monitor
/// toast "Use" action to promote a freshly-observed clipboard entry into the
/// active selection.
///
/// This command does EVERYTHING a hotkey trigger would do (minus the actual
/// text capture): cancel any in-flight stream, resolve the active-app
/// context via blocking FFI on a worker thread, reset ALL per-selection
/// engine state, emit a full `show_overlay` payload with modes/chains, and
/// show + focus the mini window.
#[tauri::command]
pub async fn set_selected_text(
    text: String,
    engine: State<'_, SharedEngine>,
    app: AppHandle,
) -> Result<(), String> {
    // Resolve the active-app context OUTSIDE any engine lock. `get_active_context`
    // does OS FFI (osascript / xdotool / Win32) and can block for tens to
    // hundreds of ms. Running it on the tokio blocking pool keeps the engine
    // mutex available to every other command during that window.
    let ctx = tokio::task::spawn_blocking(crate::platform::context::get_active_context)
        .await
        .unwrap_or_default();

    // Single locked scope — mutate ALL per-selection state and read the
    // modes/chains that will be emitted in one atomic critical section.
    // This closes the previous "two locks in succession" race where a
    // concurrent `save_config` could swap modes between the two reads.
    let (modes_info, chains_info, ctx_value) = {
        let mut e = engine.lock().unwrap();

        // Cancel any in-flight stream so its trailing chunks don't bleed
        // into the fresh overlay session the user is about to see.
        e.cancel_stream();

        // Reset ALL per-selection state. Retaining any of these would make
        // `retry()` or the undo stack reference the previous (unrelated)
        // selection's output.
        e.last_text = text.clone();
        e.last_result = String::new();
        e.last_entry_id = None;
        e.last_mode = String::new();
        e.last_language = "auto".into();
        e.last_app_hint = ctx.hint.clone();

        (
            crate::core::modes::modes_list(&e.modes),
            crate::core::modes::chains_list(&e.chains),
            serde_json::to_value(&ctx).unwrap_or_default(),
        )
    };

    let _ = app.emit(
        "quill://show_overlay",
        json!({
            "text":       text,
            "context":    ctx_value,
            "modes":      modes_info,
            "chains":     chains_info,
            "suggestion": serde_json::Value::Null,
        }),
    );

    // Explicitly ensure the mini window is visible — the clipboard-toast
    // flow may be invoked from a state where the window is hidden.
    if let Some(w) = app.get_webview_window("mini") {
        let _ = w.show();
        let _ = w.set_focus();
    }

    Ok(())
}

#[tauri::command]
pub fn dismiss(engine: State<'_, SharedEngine>, app: AppHandle) -> Result<(), String> {
    engine.lock().unwrap().cancel_stream();
    if let Some(w) = app.get_webview_window("mini") {
        let _ = w.hide();
    }
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
    if let Some(w) = app.get_webview_window("full") {
        let _ = w.hide();
    }
    Ok(())
}

// ── Tutor & AI features ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn request_tutor_explain(
    entry_id: Option<i64>,
    engine: State<'_, SharedEngine>,
    app: AppHandle,
) -> Result<(), String> {
    // Determine which entry the user is asking about: explicit id wins,
    // otherwise fall back to the last entry produced in this session.
    let target_id = entry_id.or_else(|| engine.lock().unwrap().last_entry_id);

    // If we have a concrete history id, look it up directly (O(1) query by PK).
    if let Some(id) = target_id {
        match history::get_by_id(id) {
            Ok(Some(entry)) => {
                let eng = engine.inner().clone();
                let orig = entry.original_text.clone();
                let out = entry.output_text.clone();
                let mode = entry.mode.clone().unwrap_or_default();
                let lang = entry.language.clone().unwrap_or_default();
                tokio::spawn(async move {
                    engine::explain_entry(eng, app, id, orig, out, mode, lang).await;
                });
                return Ok(());
            }
            Ok(None) => { /* fall through to in-memory fallback */ }
            Err(e) => {
                let _ = app.emit(
                    "quill://error",
                    json!({"message": format!("History lookup failed: {e}")}),
                );
                return Ok(());
            }
        }
    }

    // In-memory fallback: history disabled, or nothing streamed yet with an id.
    // Use whatever the engine remembers from the current session.
    let (orig, out, mode, lang) = {
        let e = engine.lock().unwrap();
        (
            e.last_text.clone(),
            e.last_result.clone(),
            e.last_mode.clone(),
            e.last_language.clone(),
        )
    };
    if out.is_empty() {
        // Nothing to explain yet.
        return Ok(());
    }
    let eng = engine.inner().clone();
    tokio::spawn(async move {
        // id = 0 signals "no persisted entry" to explain_entry; save path will be no-op.
        engine::explain_entry(eng, app, 0, orig, out, mode, lang).await;
    });
    Ok(())
}

#[tauri::command]
pub async fn generate_lesson(
    period: String,
    engine: State<'_, SharedEngine>,
    app: AppHandle,
) -> Result<(), String> {
    let eng = engine.inner().clone();
    tokio::spawn(async move {
        crate::engine::generate_lesson(eng, app, period).await;
    });
    Ok(())
}

#[tauri::command]
pub async fn compare_modes_cmd(
    mode_a: String,
    mode_b: String,
    language: String,
    extra_instruction: Option<String>,
    engine: State<'_, SharedEngine>,
    app: AppHandle,
) -> Result<(), String> {
    let eng = engine.inner().clone();
    tokio::spawn(async move {
        crate::engine::compare_modes(eng, app, mode_a, mode_b, language, extra_instruction).await;
    });
    Ok(())
}

#[tauri::command]
pub async fn get_pronunciation(
    text: String,
    language: String,
    engine: State<'_, SharedEngine>,
    app: AppHandle,
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
    limit: Option<usize>,
    language: Option<String>,
    app: AppHandle,
) -> Result<(), String> {
    let entries =
        history::get_recent(limit.unwrap_or(50), language.as_deref()).map_err(|e| e.to_string())?;
    let _ = app.emit(
        "quill://history",
        json!({"entries": entries, "update": null}),
    );
    Ok(())
}

#[tauri::command]
pub fn toggle_favorite(entry_id: i64, app: AppHandle) -> Result<(), String> {
    let new_state = history::toggle_favorite(entry_id).map_err(|e| e.to_string())?;
    let _ = app.emit(
        "quill://favorite_toggled",
        json!({"entry_id": entry_id, "favorited": new_state}),
    );
    Ok(())
}

#[tauri::command]
pub fn export_history(format: String, app: AppHandle) -> Result<(), String> {
    let entries = history::get_all_entries().map_err(|e| e.to_string())?;
    let _ = app.emit(
        "quill://export_data",
        json!({"entries": entries, "format": format}),
    );
    Ok(())
}

// ── Config & templates ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn save_config(
    config_update: Value,
    engine: State<'_, SharedEngine>,
    app: AppHandle,
) -> Result<(), String> {
    // Serialise concurrent save_config calls. Without this, two rapid saves
    // can have their write → load → swap sequences interleaved, leaving the
    // in-memory engine config out of sync with what's actually on disk.
    let _guard = SAVE_CONFIG_LOCK.lock().await;

    config::save_user_config(config_update).map_err(|e| e.to_string())?;

    let new_cfg = config::load_config();
    let (new_modes, new_chains) = load_modes(&new_cfg);

    // Initialise history DB OUTSIDE the engine lock — this does filesystem I/O
    // (open connection, CREATE TABLE, ALTER TABLE migration) and we don't want
    // to hold the engine Mutex across that. Same anti-pattern we eliminated in
    // execute_mode / execute_chain / compare_modes / show_mini_overlay.
    if new_cfg.history.enabled {
        if let Err(err) = history::init_db() {
            eprintln!("[history] init error: {err}");
        }
    }

    // Capture the OLD hotkey and swap in the new config inside a single
    // locked scope so the two can't diverge under concurrent access.
    let (old_hotkey, new_hotkey) = {
        let mut e = engine.lock().unwrap();
        let old = e.config.hotkey.clone();
        let now_enabled = new_cfg.clipboard_monitor.enabled;
        e.clipboard_monitor_running
            .store(now_enabled, std::sync::atomic::Ordering::Relaxed);
        e.config = new_cfg.clone();
        e.modes = new_modes;
        e.chains = new_chains;
        (old, new_cfg.hotkey.clone())
    };

    // Re-register the global hotkey if the user changed it. Without this,
    // changes made via the Settings UI wouldn't take effect until next restart.
    if old_hotkey != new_hotkey {
        if let Err(err) = register_hotkey(&app, engine.inner().clone(), new_hotkey.as_deref()) {
            eprintln!("[hotkey] re-registration failed: {err}");
        }
    }

    // Emit updated templates to frontend
    let _ = app.emit(
        "quill://templates_updated",
        json!({"templates": new_cfg.templates}),
    );
    Ok(())
}

#[tauri::command]
pub fn save_template(
    name: String,
    mode: String,
    instruction: String,
    engine: State<'_, SharedEngine>,
    app: AppHandle,
) -> Result<(), String> {
    let mut e = engine.lock().unwrap();
    e.config.templates.retain(|t| t.name != name);
    e.config.templates.push(crate::core::config::Template {
        name,
        mode,
        instruction,
    });
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
    name: String,
    engine: State<'_, SharedEngine>,
    app: AppHandle,
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

/// Return the config for the Settings UI.
///
/// **Security**: the `api_key` field is masked — we replace the real value
/// with an empty string and add a sibling `api_key_set` boolean so the UI
/// can show "already configured" state without ever exposing the plaintext
/// key to JavaScript. If any untrusted dependency or future XSS vector gains
/// read access to the frontend, the secret stays behind the IPC boundary.
///
/// The Settings panel is expected to treat an empty `api_key` on save as
/// "keep the existing value" (see `save_user_config`).
#[tauri::command]
pub fn get_config(engine: State<'_, SharedEngine>) -> Value {
    let e = engine.lock().unwrap();
    let mut value = serde_json::to_value(&e.config).unwrap_or_default();
    if let Some(obj) = value.as_object_mut() {
        let has_key = e
            .config
            .api_key
            .as_ref()
            .map(|k| !k.is_empty())
            .unwrap_or(false);
        obj.insert("api_key".into(), Value::String(String::new()));
        obj.insert("api_key_set".into(), Value::Bool(has_key));
    }
    value
}
