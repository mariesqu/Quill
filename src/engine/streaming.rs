//! Streaming helpers: pump a provider stream, filter <think> blocks,
//! emit UiEvent::StreamChunk/StreamDone/StreamError, mirror the buffer
//! into AppState, and handle cancellation.
//!
//! Two flavours of streaming live here, and they cancel differently:
//!
//! * `run_single_stream` — select-based cancellation. Installs a fresh
//!   `cancel_tx` on the engine's single-stream slot via `set_cancel_tx`
//!   and runs the pump inside `tokio::select!` with a cancel arm that
//!   wakes when the slot's receiver fires. Used by `execute_mode` and
//!   each step of `execute_chain`.
//!
//! * `run_silent_stream` — NO in-function cancel arm, NO cancel slot
//!   touch. Cancellation is driven entirely by the CALLER wrapping this
//!   future in its own `tokio::select!` and dropping it when a
//!   caller-owned cancel channel fires (see `compare.rs` and
//!   `tutor_flow.rs`'s `compare_cancel_tx` / `tutor_cancel_tx` slots).

use futures_util::StreamExt;
use tokio::sync::oneshot;

use crate::core::history;
use crate::core::think_filter::ThinkFilter;
use crate::state::UiEvent;

use super::Engine;

/// Run one provider stream to completion. Returns the full (filtered) text
/// or `None` if the stream was cancelled.
pub async fn run_single_stream(engine: Engine, system: String, user: String) -> Option<String> {
    let stream = match engine.provider().stream_completion(&system, &user).await {
        Ok(s) => s,
        Err(err) => {
            engine.state().lock().unwrap().fail_stream(&err);
            engine.emit(UiEvent::StreamError { message: err });
            return None;
        }
    };

    let (cancel_tx, mut cancel_rx) = oneshot::channel::<()>();
    engine.set_cancel_tx(cancel_tx);

    // Close the race window between `execute_mode`'s `cancel_stream` (which
    // drains the user-cancel flag) and the install above. A user pressing
    // CANCEL in that gap sets the flag but finds no live channel to fire;
    // since `cancel_stream_user` no longer clears the flag on no-op, it
    // would sit latched here and silently consume the next cancel. Draining
    // it now surfaces the cancel as StreamCancelled and exits immediately.
    if engine.take_user_cancel() {
        {
            let mut s = engine.state().lock().unwrap();
            s.is_streaming = false;
            s.is_done = false;
        }
        engine.emit(UiEvent::StreamCancelled);
        return None;
    }

    let mut full_text = String::new();
    let mut filter = ThinkFilter::new();
    tokio::pin!(stream);

    loop {
        tokio::select! {
            chunk = stream.next() => {
                match chunk {
                    Some(token) => {
                        let visible = filter.push(&token);
                        if !visible.is_empty() {
                            full_text.push_str(&visible);
                            engine.state().lock().unwrap().push_chunk(&visible);
                            engine.emit(UiEvent::StreamChunk { text: visible });
                        }
                    }
                    None => break,
                }
            }
            _ = &mut cancel_rx => {
                // Gate BOTH the state mutation and the event emission on
                // the user-cancel flag. Engine-chained cancel
                // (execute_mode / execute_chain / compare_modes /
                // Dismiss firing cancel before launching a new stream)
                // must stay fully silent here — the successor flow's
                // `begin_stream` has already flipped is_streaming=true,
                // so writing is_streaming=false here would clobber the
                // successor's fresh state. On the user-initiated path,
                // we own both the state reset and the StreamCancelled
                // toast.
                if engine.take_user_cancel() {
                    {
                        let mut s = engine.state().lock().unwrap();
                        s.is_streaming = false;
                        s.is_done = false;
                    }
                    engine.emit(UiEvent::StreamCancelled);
                }
                return None;
            }
        }
    }

    let tail = filter.flush();
    if !tail.is_empty() {
        full_text.push_str(&tail);
        engine.state().lock().unwrap().push_chunk(&tail);
        engine.emit(UiEvent::StreamChunk { text: tail });
    }

    // Do NOT take the cancel slot here. Between the tail-flush above and
    // this point, a successor stream B can install its own Sender via
    // `set_cancel_tx`; a manual take would steal B's Sender and leave B
    // uncancellable. Natural completion leaves the slot populated with a
    // stale sender whose receiver has just been dropped — the next
    // `set_cancel_tx` (now fire-before-replace) drains it safely, and
    // `fire_all_cancels` (R11-3) no longer counts stale send-failures.

    Some(full_text)
}

/// Persist a completed stream to history (if enabled), latch it onto
/// AppState, and emit `UiEvent::StreamDone`. If tutor auto-explain is on,
/// spawn the explanation in the background.
#[allow(clippy::too_many_arguments)]
pub async fn finalize_result(
    engine: Engine,
    original: &str,
    output: &str,
    mode: &str,
    language: &str,
    history_en: bool,
    tutor_en: bool,
) {
    // Models often prepend blank lines after a stripped <think> block — trim
    // once here so Replace, history, and StreamDone all see the cleaned text.
    let output = output.trim();

    let (app_hint, persona_tone, max_entries, auto_explain) = {
        let cfg = engine.config();
        (
            engine.state().lock().unwrap().last_app_hint.clone(),
            cfg.persona.tone.clone(),
            cfg.history.max_entries,
            cfg.tutor.auto_explain,
        )
    };

    let entry_id = if history_en {
        // history::save_entry is blocking SQLite. Running it directly on
        // the tokio worker would stall the scheduler (tens of ms per
        // commit). Every other history call uses spawn_blocking; this
        // finalize path was the last holdout.
        let original_owned = original.to_string();
        let output_owned = output.to_string();
        let mode_owned = mode.to_string();
        let language_owned = language.to_string();
        tokio::task::spawn_blocking(move || {
            history::save_entry(
                &original_owned,
                &output_owned,
                &mode_owned,
                &language_owned,
                &app_hint,
                &persona_tone,
                max_entries,
            )
        })
        .await
        .ok()
        .and_then(|res| match res {
            Ok(id) => Some(id),
            Err(e) => {
                tracing::warn!("history save failed: {e}");
                None
            }
        })
    } else {
        None
    };

    engine
        .state()
        .lock()
        .unwrap()
        .finish_stream(output, entry_id);

    engine.emit(UiEvent::StreamDone {
        full_text: output.to_string(),
        entry_id,
    });

    if tutor_en && auto_explain {
        if let Some(eid) = entry_id {
            let engine2 = engine.clone();
            let orig = original.to_string();
            let out = output.to_string();
            let mode_s = mode.to_string();
            let lang_s = language.to_string();
            tokio::spawn(async move {
                super::tutor_flow::explain_entry(engine2, eid, orig, out, mode_s, lang_s).await;
            });
        }
    }
}

/// Silent stream used by `compare_modes` — no StreamChunk emission, no
/// cancel_tx touch. Returns the final text or the provider error so the
/// caller can surface it to the user instead of silently rendering a
/// blank side-by-side panel.
pub async fn run_silent_stream(
    engine: Engine,
    system: String,
    user: String,
) -> Result<String, String> {
    let stream = engine.provider().stream_completion(&system, &user).await?;
    let mut full = String::new();
    let mut filter = ThinkFilter::new();
    tokio::pin!(stream);
    while let Some(token) = stream.next().await {
        full.push_str(&filter.push(&token));
    }
    full.push_str(&filter.flush());
    Ok(full)
}
