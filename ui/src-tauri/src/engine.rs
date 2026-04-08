use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use futures_util::StreamExt;
use serde_json::json;
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;

use crate::core::config::{Config, Template};
use crate::core::modes::{ChainConfig, ModeConfig, chains_list, modes_list};
use crate::core::prompt::{build_prompt, suggest_mode};
use crate::core::history;
use crate::core::tutor;
use crate::platform::context::get_active_context;
use crate::providers::build_provider;

// ── Engine state ──────────────────────────────────────────────────────────────

pub struct Engine {
    pub config:                    Config,
    pub modes:                     HashMap<String, ModeConfig>,
    pub chains:                    HashMap<String, ChainConfig>,
    pub last_text:                 String,
    pub last_result:               String,
    pub last_entry_id:             Option<i64>,
    pub last_mode:                 String,
    pub last_language:             String,
    pub last_app_hint:             String,
    pub clipboard_monitor_running: Arc<AtomicBool>,
    cancel_tx:                     Option<oneshot::Sender<()>>,
}

impl Engine {
    pub fn new(
        config: Config,
        modes: HashMap<String, ModeConfig>,
        chains: HashMap<String, ChainConfig>,
    ) -> Self {
        let clipboard_enabled = config.clipboard_monitor.enabled;
        Self {
            config,
            modes,
            chains,
            last_text:                 String::new(),
            last_result:               String::new(),
            last_entry_id:             None,
            last_mode:                 String::new(),
            last_language:             "auto".into(),
            last_app_hint:             String::new(),
            clipboard_monitor_running: Arc::new(AtomicBool::new(clipboard_enabled)),
            cancel_tx:                 None,
        }
    }

    pub fn cancel_stream(&mut self) {
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
    }

    pub fn history_enabled(&self) -> bool {
        self.config.history.enabled
    }

    pub fn tutor_enabled(&self) -> bool {
        self.config.tutor.enabled && self.history_enabled()
    }
}

pub type SharedEngine = Arc<Mutex<Engine>>;

// ── Hotkey handler ────────────────────────────────────────────────────────────

pub async fn handle_hotkey(engine: SharedEngine, app: AppHandle) {
    // Capture selected text on a blocking thread (clipboard I/O)
    let text = tokio::task::spawn_blocking(crate::platform::capture::get_selected_text)
        .await
        .unwrap_or(None);

    let text = match text {
        Some(t) if !t.trim().is_empty() => t,
        _ => {
            // No text selected — show mini overlay with empty state
            show_mini_overlay(&engine, &app, String::new());
            return;
        }
    };

    let context = tokio::task::spawn_blocking(get_active_context).await.unwrap_or_default();

    {
        let mut e = engine.lock().unwrap();
        e.last_text     = text.clone();
        e.last_language = "auto".into();
        e.last_app_hint = context.hint.clone();
    }

    let (suggested_mode, suggestion_reason) = suggest_mode(&text, &context);
    let _ = app.emit("quill://smart_suggestion", json!({
        "mode_id": suggested_mode,
        "reason":  suggestion_reason,
    }));

    show_mini_overlay(&engine, &app, text);
}

fn show_mini_overlay(engine: &SharedEngine, app: &AppHandle, text: String) {
    let (modes_info, chains_info, context) = {
        let e = engine.lock().unwrap();
        let ctx = get_active_context();
        (modes_list(&e.modes), chains_list(&e.chains), ctx)
    };

    let _ = app.emit("quill://show_overlay", json!({
        "text":    text,
        "context": context,
        "modes":   modes_info,
        "chains":  chains_info,
    }));

    // Show the mini window
    if let Some(w) = app.get_webview_window("mini") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

// ── Mode execution ────────────────────────────────────────────────────────────

pub async fn execute_mode(
    engine:            SharedEngine,
    app:               AppHandle,
    mode:              String,
    language:          String,
    extra_instruction: Option<String>,
) {
    // Cancel any in-progress stream
    {
        let mut e = engine.lock().unwrap();
        e.cancel_stream();
        e.last_mode     = mode.clone();
        e.last_language = language.clone();
    }

    let (system, user, text, persona, history_en, tutor_en) = {
        let e = engine.lock().unwrap();
        let ctx = get_active_context();
        match build_prompt(
            &e.last_text, &mode, &e.modes,
            &ctx, &language, &e.config.persona,
            extra_instruction.as_deref(),
        ) {
            Ok((sys, usr)) => (sys, usr, e.last_text.clone(), e.config.persona.clone(), e.history_enabled(), e.tutor_enabled()),
            Err(err) => {
                let _ = app.emit("quill://error", json!({"message": err}));
                return;
            }
        }
    };

    stream_and_collect(engine, app, system, user, mode, language, history_en, tutor_en).await;
}

pub async fn execute_chain(
    engine:            SharedEngine,
    app:               AppHandle,
    chain_id:          String,
    language:          String,
    extra_instruction: Option<String>,
) {
    let chain_steps = {
        let e = engine.lock().unwrap();
        e.cancel_stream();
        e.chains.get(&chain_id).map(|c| c.steps.clone())
    };

    let steps = match chain_steps {
        Some(s) => s,
        None => {
            let _ = app.emit("quill://error", json!({"message": format!("Unknown chain: {chain_id}")}));
            return;
        }
    };

    let total = steps.len();
    let mut current_text = engine.lock().unwrap().last_text.clone();

    for (idx, step_mode) in steps.iter().enumerate() {
        let _ = app.emit("quill://chain_step", json!({
            "step":  idx + 1,
            "total": total,
            "mode":  step_mode,
        }));

        // Update last_text for this step
        engine.lock().unwrap().last_text = current_text.clone();

        let (system, user) = {
            let e = engine.lock().unwrap();
            let ctx = get_active_context();
            match build_prompt(
                &current_text, step_mode, &e.modes,
                &ctx, &language, &e.config.persona,
                if idx == 0 { extra_instruction.as_deref() } else { None },
            ) {
                Ok(p) => p,
                Err(err) => {
                    let _ = app.emit("quill://error", json!({"message": err}));
                    return;
                }
            }
        };

        let result = run_single_stream(engine.clone(), app.clone(), system, user).await;
        match result {
            Some(text) => current_text = text,
            None => return, // cancelled or error
        }
    }

    // Save chain result to history
    let original = engine.lock().unwrap().last_text.clone();
    let (history_en, tutor_en) = {
        let e = engine.lock().unwrap();
        (e.history_enabled(), e.tutor_enabled())
    };
    finalize_result(engine, &app, &original, &current_text, &format!("chain:{chain_id}"), &language, history_en, tutor_en).await;
}

// ── Streaming helpers ─────────────────────────────────────────────────────────

async fn run_single_stream(
    engine: SharedEngine,
    app:    AppHandle,
    system: String,
    user:   String,
) -> Option<String> {
    let provider = {
        let e = engine.lock().unwrap();
        build_provider(&e.config)
    };

    let stream = match provider.stream_completion(&system, &user).await {
        Ok(s) => s,
        Err(err) => {
            let _ = app.emit("quill://error", json!({"message": err}));
            return None;
        }
    };

    let (cancel_tx, mut cancel_rx) = oneshot::channel::<()>();
    engine.lock().unwrap().cancel_tx = Some(cancel_tx);

    let mut full_text = String::new();
    tokio::pin!(stream);

    loop {
        tokio::select! {
            chunk = stream.next() => {
                match chunk {
                    Some(token) => {
                        full_text.push_str(&token);
                        let _ = app.emit("quill://stream_chunk", json!({"chunk": token}));
                    }
                    None => break,
                }
            }
            _ = &mut cancel_rx => {
                return None; // cancelled
            }
        }
    }

    Some(full_text)
}

async fn stream_and_collect(
    engine:     SharedEngine,
    app:        AppHandle,
    system:     String,
    user:       String,
    mode:       String,
    language:   String,
    history_en: bool,
    tutor_en:   bool,
) {
    let original = engine.lock().unwrap().last_text.clone();
    let result = run_single_stream(engine.clone(), app.clone(), system, user).await;

    if let Some(full_text) = result {
        finalize_result(engine, &app, &original, &full_text, &mode, &language, history_en, tutor_en).await;
    }
}

async fn finalize_result(
    engine:     SharedEngine,
    app:        &AppHandle,
    original:   &str,
    output:     &str,
    mode:       &str,
    language:   &str,
    history_en: bool,
    tutor_en:   bool,
) {
    let app_hint = engine.lock().unwrap().last_app_hint.clone();
    let persona_tone = engine.lock().unwrap().config.persona.tone.clone();

    let entry_id = if history_en {
        match history::save_entry(original, output, mode, language, &app_hint, &persona_tone) {
            Ok(id) => Some(id),
            Err(e) => { eprintln!("[history] save error: {e}"); None }
        }
    } else { None };

    {
        let mut e = engine.lock().unwrap();
        e.last_result   = output.to_string();
        e.last_entry_id = entry_id;
    }

    let _ = app.emit("quill://stream_done", json!({
        "full_text": output,
        "entry_id":  entry_id,
    }));

    // Auto-explain if tutor enabled
    if tutor_en && engine.lock().unwrap().config.tutor.auto_explain {
        if let Some(eid) = entry_id {
            let engine2 = engine.clone();
            let app2 = app.clone();
            let orig = original.to_string();
            let out = output.to_string();
            let mode_str = mode.to_string();
            let lang_str = language.to_string();
            tokio::spawn(async move {
                explain_entry(engine2, app2, eid, orig, out, mode_str, lang_str).await;
            });
        }
    }
}

// ── Tutor ─────────────────────────────────────────────────────────────────────

pub async fn explain_entry(
    engine:   SharedEngine,
    app:      AppHandle,
    entry_id: i64,
    original: String,
    output:   String,
    mode:     String,
    language: String,
) {
    let system = tutor::EXPLAIN_SYSTEM.to_string();
    let user = tutor::build_explain_prompt(&original, &output, &mode, &language);

    if let Some(explanation) = run_single_stream(engine, app.clone(), system, user).await {
        let _ = history::save_tutor_explanation(entry_id, &explanation);
        let _ = app.emit("quill://tutor_explanation", json!({
            "explanation": explanation,
            "entry_id":    entry_id,
        }));
    }
}

pub async fn generate_lesson(engine: SharedEngine, app: AppHandle, period: String) {
    let days = if period == "daily" { 1 } else { 7 };
    let stats = match history::get_stats(days) {
        Ok(s) => s,
        Err(e) => {
            let _ = app.emit("quill://error", json!({"message": format!("History error: {e}")}));
            return;
        }
    };

    let system = tutor::LESSON_SYSTEM.to_string();
    let user = tutor::build_lesson_prompt(&stats, &period);

    if let Some(lesson) = run_single_stream(engine.clone(), app.clone(), system, user).await {
        let lang = engine.lock().unwrap().config.tutor.lesson_language.clone();
        let _ = history::save_lesson(&period, &lesson, &lang);
        let _ = app.emit("quill://tutor_lesson", json!({"lesson_md": lesson, "period": period}));
    }
}

// ── Compare modes ─────────────────────────────────────────────────────────────

pub async fn compare_modes(
    engine:            SharedEngine,
    app:               AppHandle,
    mode_a:            String,
    mode_b:            String,
    language:          String,
    extra_instruction: Option<String>,
) {
    let (sys_a, usr_a, sys_b, usr_b) = {
        let e = engine.lock().unwrap();
        let ctx = get_active_context();
        let extra = extra_instruction.as_deref();
        let pa = build_prompt(&e.last_text, &mode_a, &e.modes, &ctx, &language, &e.config.persona, extra);
        let pb = build_prompt(&e.last_text, &mode_b, &e.modes, &ctx, &language, &e.config.persona, extra);
        match (pa, pb) {
            (Ok(a), Ok(b)) => (a.0, a.1, b.0, b.1),
            (Err(e), _) | (_, Err(e)) => {
                let _ = app.emit("quill://error", json!({"message": e}));
                return;
            }
        }
    };

    let engine_a = engine.clone();
    let engine_b = engine.clone();
    let app_a = app.clone();
    let app_b = app.clone();
    let mode_a2 = mode_a.clone();
    let mode_b2 = mode_b.clone();

    let (result_a, result_b) = tokio::join!(
        run_single_stream(engine_a, app_a, sys_a, usr_a),
        run_single_stream(engine_b, app_b, sys_b, usr_b),
    );

    let _ = app.emit("quill://comparison_done", json!({
        "mode_a":   mode_a2,
        "result_a": result_a.unwrap_or_default(),
        "mode_b":   mode_b2,
        "result_b": result_b.unwrap_or_default(),
    }));
}

// ── Pronunciation ─────────────────────────────────────────────────────────────

pub async fn get_pronunciation(engine: SharedEngine, app: AppHandle, text: String, language: String) {
    let system = "You are a linguistics expert. Provide a concise pronunciation guide.";
    let user = format!(
        "Provide a brief pronunciation guide for this {language} text. \
         Use IPA notation and simple phonetic spelling. Keep it under 100 words.\n\n{text}"
    );
    if let Some(result) = run_single_stream(engine, app.clone(), system.to_string(), user).await {
        let _ = app.emit("quill://pronunciation", json!({"text": result}));
    }
}
