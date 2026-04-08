use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::oneshot;

use crate::core::config::Config;
use crate::core::history;
use crate::core::modes::{chains_list, modes_list, ChainConfig, ModeConfig};
use crate::core::prompt::{build_prompt, suggest_mode};
use crate::core::tutor;
use crate::platform::context::get_active_context;
use crate::providers::build_provider;

// ── Engine state ──────────────────────────────────────────────────────────────

pub struct Engine {
    pub config: Config,
    pub modes: HashMap<String, ModeConfig>,
    pub chains: HashMap<String, ChainConfig>,
    pub last_text: String,
    pub last_result: String,
    pub last_entry_id: Option<i64>,
    pub last_mode: String,
    pub last_language: String,
    pub last_app_hint: String,
    pub clipboard_monitor_running: Arc<AtomicBool>,
    cancel_tx: Option<oneshot::Sender<()>>,
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
            last_text: String::new(),
            last_result: String::new(),
            last_entry_id: None,
            last_mode: String::new(),
            last_language: "auto".into(),
            last_app_hint: String::new(),
            clipboard_monitor_running: Arc::new(AtomicBool::new(clipboard_enabled)),
            cancel_tx: None,
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
    // Capture selected text AND active-app context concurrently, both on
    // blocking threads. Both do OS FFI (clipboard simulation + osascript /
    // xdotool / Win32) and neither belongs on the tokio worker pool.
    let text_fut = tokio::task::spawn_blocking(crate::platform::capture::get_selected_text);
    let ctx_fut = tokio::task::spawn_blocking(get_active_context);
    let (text_res, ctx_res) = tokio::join!(text_fut, ctx_fut);
    let text = text_res.ok().flatten();
    let context = ctx_res.unwrap_or_default();

    let text = match text {
        Some(t) if !t.trim().is_empty() => t,
        _ => {
            // No text selected — show mini overlay with empty state, passing
            // the already-captured context so `show_mini_overlay` doesn't
            // re-run the OS FFI synchronously on the tokio runtime.
            show_mini_overlay(&engine, &app, String::new(), &context, None);
            return;
        }
    };

    {
        let mut e = engine.lock().unwrap();
        e.last_text = text.clone();
        e.last_language = "auto".into();
        e.last_app_hint = context.hint.clone();
    }

    // Compute the heuristic mode suggestion up front — it travels INSIDE the
    // show_overlay payload below so the frontend can set both at once. Prior
    // versions emitted it as a separate event before show_overlay, and the
    // show_overlay listener's `setSuggestion(null)` reset then wiped it
    // before the user ever saw it.
    let (suggested_mode, suggestion_reason) = suggest_mode(&text, &context);

    show_mini_overlay(
        &engine,
        &app,
        text,
        &context,
        Some((suggested_mode, suggestion_reason)),
    );
}

/// Render the mini overlay.
///
/// Takes the already-captured `context` by reference — we never re-run the
/// OS FFI here, because `handle_hotkey` already did it via `spawn_blocking`.
/// This avoids (a) blocking the tokio worker on macOS `osascript`, and
/// (b) double-invoking osascript per hotkey press.
///
/// `suggestion` is an optional `(mode_id, reason)` pair computed by
/// `suggest_mode`, embedded in the payload so the frontend applies it
/// atomically with the rest of the overlay state.
fn show_mini_overlay(
    engine: &SharedEngine,
    app: &AppHandle,
    text: String,
    context: &crate::platform::context::AppContext,
    suggestion: Option<(String, String)>,
) {
    let (modes_info, chains_info) = {
        let e = engine.lock().unwrap();
        (modes_list(&e.modes), chains_list(&e.chains))
    };

    let suggestion_payload =
        suggestion.map(|(mode_id, reason)| json!({ "mode_id": mode_id, "reason": reason }));

    let _ = app.emit(
        "quill://show_overlay",
        json!({
            "text":       text,
            "context":    context,
            "modes":      modes_info,
            "chains":     chains_info,
            "suggestion": suggestion_payload,
        }),
    );
    // Window show/focus is handled by the MiniOverlay frontend after React
    // renders the content. Showing from Rust before the JS event is processed
    // causes a transparent-window flash (empty window appears, then content
    // animates in from opacity:0). Letting the frontend call window.show()
    // means the window only becomes visible once content is already painted.
}

// ── Mode execution ────────────────────────────────────────────────────────────

pub async fn execute_mode(
    engine: SharedEngine,
    app: AppHandle,
    mode: String,
    language: String,
    extra_instruction: Option<String>,
) {
    // Cancel any in-progress stream
    {
        let mut e = engine.lock().unwrap();
        e.cancel_stream();
        e.last_mode = mode.clone();
        e.last_language = language.clone();
    }

    // Resolve app context OUTSIDE the engine lock — `get_active_context` does
    // OS FFI (macOS `osascript`, Linux `xdotool`) that can block for tens to
    // hundreds of ms. Holding the engine Mutex across that would stall every
    // other command (dismiss, set_result, save_config, toggle_favorite, …).
    let ctx = tokio::task::spawn_blocking(get_active_context)
        .await
        .unwrap_or_default();

    let (system, user, history_en, tutor_en) = {
        let e = engine.lock().unwrap();
        match build_prompt(
            &e.last_text,
            &mode,
            &e.modes,
            &ctx,
            &language,
            &e.config.persona,
            extra_instruction.as_deref(),
        ) {
            Ok((sys, usr)) => (sys, usr, e.history_enabled(), e.tutor_enabled()),
            Err(err) => {
                let _ = app.emit("quill://error", json!({"message": err}));
                return;
            }
        }
    };

    stream_and_collect(
        engine, app, system, user, mode, language, history_en, tutor_en,
    )
    .await;
}

pub async fn execute_chain(
    engine: SharedEngine,
    app: AppHandle,
    chain_id: String,
    language: String,
    extra_instruction: Option<String>,
) {
    let chain_steps = {
        let mut e = engine.lock().unwrap();
        e.cancel_stream();
        e.chains.get(&chain_id).map(|c| c.steps.clone())
    };

    let steps = match chain_steps {
        Some(s) => s,
        None => {
            let _ = app.emit(
                "quill://error",
                json!({"message": format!("Unknown chain: {chain_id}")}),
            );
            return;
        }
    };

    let total = steps.len();
    let mut current_text = engine.lock().unwrap().last_text.clone();

    // Snapshot the user's ORIGINAL selection before the loop starts. Each
    // step overwrites `engine.last_text` with that step's input (the previous
    // step's output), so reading `engine.last_text` after the loop gives us
    // the penultimate output, not the original. History must record what
    // the user actually selected so tutor explanations can diff correctly.
    let chain_original = current_text.clone();

    // Resolve context ONCE before the loop (not per-step): the active app
    // doesn't change during a chain, and `get_active_context` is blocking FFI.
    // Doing it in the loop would stall the engine Mutex for every step.
    let ctx = tokio::task::spawn_blocking(get_active_context)
        .await
        .unwrap_or_default();

    for (idx, step_mode) in steps.iter().enumerate() {
        let _ = app.emit(
            "quill://chain_step",
            json!({
                "step":  idx + 1,
                "total": total,
                "mode":  step_mode,
            }),
        );

        // Update last_text for this step
        engine.lock().unwrap().last_text = current_text.clone();

        let (system, user) = {
            let e = engine.lock().unwrap();
            match build_prompt(
                &current_text,
                step_mode,
                &e.modes,
                &ctx,
                &language,
                &e.config.persona,
                if idx == 0 {
                    extra_instruction.as_deref()
                } else {
                    None
                },
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

    // Save chain result to history using the TRUE original selection
    // (snapshotted before the loop began), not the per-step mutated value.
    //
    // Also RESTORE `engine.last_text` to the original selection so that a
    // subsequent `retry()` or single-mode invocation operates on what the
    // user originally selected, not on whatever intermediate step input
    // was left behind at the tail of the loop.
    let (history_en, tutor_en) = {
        let mut e = engine.lock().unwrap();
        e.last_text = chain_original.clone();
        (e.history_enabled(), e.tutor_enabled())
    };
    finalize_result(
        engine,
        &app,
        &chain_original,
        &current_text,
        &format!("chain:{chain_id}"),
        &language,
        history_en,
        tutor_en,
    )
    .await;
}

// ── Streaming helpers ─────────────────────────────────────────────────────────

async fn run_single_stream(
    engine: SharedEngine,
    app: AppHandle,
    system: String,
    user: String,
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

#[allow(clippy::too_many_arguments)]
async fn stream_and_collect(
    engine: SharedEngine,
    app: AppHandle,
    system: String,
    user: String,
    mode: String,
    language: String,
    history_en: bool,
    tutor_en: bool,
) {
    let original = engine.lock().unwrap().last_text.clone();
    let result = run_single_stream(engine.clone(), app.clone(), system, user).await;

    if let Some(full_text) = result {
        finalize_result(
            engine, &app, &original, &full_text, &mode, &language, history_en, tutor_en,
        )
        .await;
    }
}

#[allow(clippy::too_many_arguments)]
async fn finalize_result(
    engine: SharedEngine,
    app: &AppHandle,
    original: &str,
    output: &str,
    mode: &str,
    language: &str,
    history_en: bool,
    tutor_en: bool,
) {
    let (app_hint, persona_tone, max_entries) = {
        let e = engine.lock().unwrap();
        (
            e.last_app_hint.clone(),
            e.config.persona.tone.clone(),
            e.config.history.max_entries,
        )
    };

    let entry_id = if history_en {
        match history::save_entry(
            original,
            output,
            mode,
            language,
            &app_hint,
            &persona_tone,
            max_entries,
        ) {
            Ok(id) => Some(id),
            Err(e) => {
                eprintln!("[history] save error: {e}");
                None
            }
        }
    } else {
        None
    };

    {
        let mut e = engine.lock().unwrap();
        e.last_result = output.to_string();
        e.last_entry_id = entry_id;
    }

    let _ = app.emit(
        "quill://stream_done",
        json!({
            "full_text": output,
            "entry_id":  entry_id,
        }),
    );

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
    engine: SharedEngine,
    app: AppHandle,
    entry_id: i64,
    original: String,
    output: String,
    mode: String,
    language: String,
) {
    let system = tutor::EXPLAIN_SYSTEM.to_string();
    let user = tutor::build_explain_prompt(&original, &output, &mode, &language);

    if let Some(explanation) = run_single_stream(engine, app.clone(), system, user).await {
        // Only persist when there's a real history row to attach to — callers
        // may pass id = 0 when tutor is invoked on an in-memory result that was
        // never saved (history disabled, or explain-before-stream-done).
        if entry_id > 0 {
            let _ = history::save_tutor_explanation(entry_id, &explanation);
        }
        let _ = app.emit(
            "quill://tutor_explanation",
            json!({
                "explanation": explanation,
                "entry_id":    entry_id,
            }),
        );
    }
}

pub async fn generate_lesson(engine: SharedEngine, app: AppHandle, period: String) {
    let days = if period == "daily" { 1 } else { 7 };
    let stats = match history::get_stats(days) {
        Ok(s) => s,
        Err(e) => {
            let _ = app.emit(
                "quill://error",
                json!({"message": format!("History error: {e}")}),
            );
            return;
        }
    };

    let system = tutor::LESSON_SYSTEM.to_string();
    let user = tutor::build_lesson_prompt(&stats, &period);

    if let Some(lesson) = run_single_stream(engine.clone(), app.clone(), system, user).await {
        let lang = engine.lock().unwrap().config.tutor.lesson_language.clone();
        let _ = history::save_lesson(&period, &lesson, &lang);
        let _ = app.emit(
            "quill://tutor_lesson",
            json!({"lesson_md": lesson, "period": period}),
        );
    }
}

// ── Compare modes ─────────────────────────────────────────────────────────────

/// Silent streaming helper used exclusively by `compare_modes`.
///
/// Unlike `run_single_stream`, this variant:
/// - does NOT emit `quill://stream_chunk` events (so two concurrent compare
///   streams cannot interleave chunks into the same frontend channel),
/// - does NOT touch `engine.cancel_tx` (so the two concurrent streams cannot
///   race to overwrite each other's cancel slot).
///
/// The comparison UX shows only the FINAL outputs once both arms complete,
/// so per-chunk streaming is unnecessary here.
async fn run_silent_stream(config: Config, system: String, user: String) -> Option<String> {
    let provider = build_provider(&config);
    let stream = provider.stream_completion(&system, &user).await.ok()?;
    let mut full = String::new();
    tokio::pin!(stream);
    while let Some(token) = stream.next().await {
        full.push_str(&token);
    }
    Some(full)
}

pub async fn compare_modes(
    engine: SharedEngine,
    app: AppHandle,
    mode_a: String,
    mode_b: String,
    language: String,
    extra_instruction: Option<String>,
) {
    // Cancel any in-flight normal stream so its chunks don't bleed into the UI
    // while the comparison is running.
    engine.lock().unwrap().cancel_stream();

    // Resolve context OUTSIDE the engine lock — FFI (see execute_mode comment).
    let ctx = tokio::task::spawn_blocking(get_active_context)
        .await
        .unwrap_or_default();

    let (pa, pb, config) = {
        let e = engine.lock().unwrap();
        let extra = extra_instruction.as_deref();
        let pa = build_prompt(
            &e.last_text,
            &mode_a,
            &e.modes,
            &ctx,
            &language,
            &e.config.persona,
            extra,
        );
        let pb = build_prompt(
            &e.last_text,
            &mode_b,
            &e.modes,
            &ctx,
            &language,
            &e.config.persona,
            extra,
        );
        (pa, pb, e.config.clone())
    };

    // If BOTH arms failed to build, surface the error and bail. If only one
    // failed, we still attempt the surviving arm so the user at least gets
    // a result from the valid mode. The comparison UI will show an empty
    // result for the failed side.
    if pa.is_err() && pb.is_err() {
        let err = pa.err().unwrap();
        let _ = app.emit("quill://error", json!({"message": err}));
        return;
    }

    let mode_a2 = mode_a.clone();
    let mode_b2 = mode_b.clone();

    let result_a = match pa {
        Ok((sys_a, usr_a)) => run_silent_stream(config.clone(), sys_a, usr_a).await,
        Err(err) => {
            let _ = app.emit(
                "quill://error",
                json!({"message": format!("Mode '{mode_a}' failed: {err}")}),
            );
            None
        }
    };
    let result_b = match pb {
        Ok((sys_b, usr_b)) => run_silent_stream(config, sys_b, usr_b).await,
        Err(err) => {
            let _ = app.emit(
                "quill://error",
                json!({"message": format!("Mode '{mode_b}' failed: {err}")}),
            );
            None
        }
    };

    let result_a_text = result_a.unwrap_or_default();
    let result_b_text = result_b.unwrap_or_default();

    // Pre-seed `last_result` with mode_a's output so that Replace works even
    // if the user skips clicking "Use" in the comparison UI. The frontend will
    // override via `set_result` when the user explicitly picks one.
    {
        let mut e = engine.lock().unwrap();
        if !result_a_text.is_empty() {
            e.last_result = result_a_text.clone();
        }
    }

    let _ = app.emit(
        "quill://comparison_done",
        json!({
            "mode_a":   mode_a2,
            "result_a": result_a_text,
            "mode_b":   mode_b2,
            "result_b": result_b_text,
        }),
    );
}

// ── Pronunciation ─────────────────────────────────────────────────────────────

pub async fn get_pronunciation(
    engine: SharedEngine,
    app: AppHandle,
    text: String,
    language: String,
) {
    let system = "You are a linguistics expert. Provide a concise pronunciation guide.";
    let user = format!(
        "Provide a brief pronunciation guide for this {language} text. \
         Use IPA notation and simple phonetic spelling. Keep it under 100 words.\n\n{text}"
    );
    if let Some(result) = run_single_stream(engine, app.clone(), system.to_string(), user).await {
        let _ = app.emit("quill://pronunciation", json!({"text": result}));
    }
}
