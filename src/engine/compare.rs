//! Side-by-side comparison of two modes. Emits a single
//! `UiEvent::ComparisonResult` once BOTH arms complete.

use tokio::sync::oneshot;

use crate::state::UiEvent;

use super::{streaming, Engine};

/// Install a fresh `compare_cancel_tx` on the engine and return the
/// matching receiver. Mirrors `tutor_flow::install_tutor_cancel` — fires
/// any stale sender already in the slot BEFORE replacing it so a
/// successor compare can't have its freshly-installed sender stolen by
/// a predecessor's cleanup path. Cleanup paths in this module no longer
/// touch the slot; a stale sender left behind is drained by the next
/// install or `fire_all_cancels`.
fn install_compare_cancel(engine: &Engine) -> oneshot::Receiver<()> {
    let (tx, rx) = oneshot::channel::<()>();
    let mut slot = engine.inner.compare_cancel_tx.lock().unwrap();
    if let Some(old) = slot.take() {
        let _ = old.send(());
    }
    *slot = Some(tx);
    rx
}

pub async fn compare_modes(
    engine: Engine,
    mode_a: String,
    mode_b: String,
    language: String,
    extra_instruction: Option<String>,
) {
    // Cancel any in-flight single stream AND any older compare so chunks
    // don't bleed into the UI.
    let _ = engine.cancel_stream();

    // Install a dedicated cancel channel for this compare. A user cancel
    // (which `Engine::cancel_stream` forwards to both sides) takes this
    // sender and fires — terminating both arms via the shared receiver.
    let mut compare_rx = install_compare_cancel(&engine);

    // Close the race window between the `cancel_stream` above (which drains
    // the user-cancel flag) and the install just done. A user hitting
    // CANCEL in that gap sets the flag but finds no live channel; since
    // `cancel_stream_user` no longer clears the flag on no-op, it would
    // sit latched here. Drain it now and exit — mirror the cancel-path
    // contract below. Gate state-write + StreamCancelled on the flag so
    // engine-chained cancels (the drain path) stay silent.
    if engine.take_user_cancel() {
        {
            let mut s = engine.state().lock().unwrap();
            s.is_streaming = false;
            s.is_done = false;
        }
        engine.emit(UiEvent::StreamCancelled);
        return;
    }

    // Resolve context outside any lock — FFI can block.
    let ctx_probe = engine.context().clone();
    let ctx = tokio::task::spawn_blocking(move || ctx_probe.active_context())
        .await
        .unwrap_or_default();

    let (pa, pb) = {
        let s = engine.state().lock().unwrap();
        let extra = extra_instruction.as_deref();
        let pa = crate::core::prompt::build_prompt(
            &s.selected_text,
            &mode_a,
            engine.modes(),
            &ctx,
            &language,
            &engine.config().persona,
            extra,
        );
        let pb = crate::core::prompt::build_prompt(
            &s.selected_text,
            &mode_b,
            engine.modes(),
            &ctx,
            &language,
            &engine.config().persona,
            extra,
        );
        (pa, pb)
    };

    if pa.is_err() && pb.is_err() {
        // Surface BOTH failure reasons — they're usually different
        // (different mode IDs, different prompt issues) and dropping B's
        // error hid information from the user.
        let pa_err = pa.err().unwrap();
        let pb_err = pb.err().unwrap();
        engine.emit(UiEvent::Error {
            message: format!("Compare failed: A: {pa_err}; B: {pb_err}"),
        });
        // Leave the cancel slot alone — see install_compare_cancel's doc.
        // A stale sender here is drained by the next install or by
        // fire_all_cancels (the latter's send-failure is now a no-op that
        // doesn't inflate the "fired" count).
        return;
    }

    // Single-arm prompt-build failure: emit the error and skip the
    // ComparisonResult event entirely. Previously we still pushed a
    // half-filled ComparisonResult with one empty side, which rendered as
    // a blank panel and obscured the error toast — users couldn't tell
    // whether their side had completed or silently failed.
    if pa.is_err() {
        let err = pa.as_ref().err().unwrap();
        engine.emit(UiEvent::Error {
            message: format!("Mode '{mode_a}' failed: {err}"),
        });
        return;
    }
    if pb.is_err() {
        let err = pb.as_ref().err().unwrap();
        engine.emit(UiEvent::Error {
            message: format!("Mode '{mode_b}' failed: {err}"),
        });
        return;
    }

    // Both prompts built successfully — unwrap is safe here.
    let (sys_a, usr_a) = pa.unwrap();
    let (sys_b, usr_b) = pb.unwrap();

    // Mark the compare as streaming. run_silent_stream doesn't touch
    // is_streaming (it's the "silent" variant), so without this the flag
    // would stay at whatever the previous operation left it at — UI
    // elements bound to is_streaming (busy indicators, cancel buttons)
    // wouldn't update. Mirror run_single_stream's begin/end pairing.
    {
        let mut s = engine.state().lock().unwrap();
        s.begin_stream("compare", &language);
    }

    // Run both arms CONCURRENTLY with tokio::join! — the README promise is
    // parallel execution, and waiting for A then B serially roughly
    // doubles the wall-clock time on any provider that's not instantly
    // cached. The two futures each hold their own ThinkFilter and don't
    // touch shared mutable state other than the provider handle (which
    // tolerates concurrent callers).
    let fut_a = streaming::run_silent_stream(engine.clone(), sys_a, usr_a);
    let fut_b = streaming::run_silent_stream(engine.clone(), sys_b, usr_b);

    // Race the join against the cancel receiver. If cancel fires first,
    // both arms are abandoned (their futures drop on this function's
    // return — tokio handles the teardown).
    let joined = tokio::select! {
        r = async { tokio::join!(fut_a, fut_b) } => Some(r),
        _ = &mut compare_rx => None,
    };

    // Slot-management note: do NOT `.take()` the compare_cancel_tx slot
    // here. A racing successor compare that just called
    // `install_compare_cancel` owns the slot now — taking it would steal
    // that successor's sender, leaving the successor uncancellable. The
    // slot is drained on the next `install_compare_cancel` (fire-before-
    // replace) or by `fire_all_cancels` (stale send fails harmlessly and
    // no longer inflates the fired count).

    let (result_a, result_b) = match joined {
        Some(r) => r,
        None => {
            tracing::debug!("compare_modes: cancelled before both arms completed");
            // Only surface StreamCancelled AND mutate state on a USER-
            // initiated cancel. An engine-chained cancel (a follow-up
            // execute_mode / compare fired the cancel to clear the path)
            // must stay fully silent: any state mutation here would
            // clobber the successor flow's begin_stream that already
            // flipped is_streaming=true.
            if engine.take_user_cancel() {
                {
                    let mut s = engine.state().lock().unwrap();
                    s.is_streaming = false;
                    s.is_done = false;
                }
                engine.emit(UiEvent::StreamCancelled);
            }
            return;
        }
    };

    // Clear is_streaming on ALL exit paths from here on — we started it
    // above with begin_stream("compare"). Doing this as a tiny helper
    // inside the function keeps the invariant in one place.
    let clear_streaming = |engine: &Engine| {
        let mut s = engine.state().lock().unwrap();
        s.is_streaming = false;
        s.is_done = false;
    };

    // Surface provider failures instead of silently rendering blank panels.
    // Any arm failing means we skip the ComparisonResult entirely — a
    // half-filled result (one arm blank) used to render as an empty panel
    // that obscured the error toast and made it look like success. Match
    // the prompt-build failure path above: one arm failed → error + bail.
    if result_a.is_err() && result_b.is_err() {
        let ea = result_a.as_ref().err().cloned().unwrap_or_default();
        let eb = result_b.as_ref().err().cloned().unwrap_or_default();
        engine.emit(UiEvent::Error {
            message: format!("Compare failed: A: {ea}; B: {eb}"),
        });
        clear_streaming(&engine);
        return;
    }
    if let Err(e) = &result_a {
        engine.emit(UiEvent::Error {
            message: format!("Mode '{mode_a}' failed: {e}"),
        });
        clear_streaming(&engine);
        return;
    }
    if let Err(e) = &result_b {
        engine.emit(UiEvent::Error {
            message: format!("Mode '{mode_b}' failed: {e}"),
        });
        clear_streaming(&engine);
        return;
    }

    let result_a_text = result_a.unwrap_or_default();
    let result_b_text = result_b.unwrap_or_default();

    // Pre-seed last_result with mode_a's output so Replace works even if
    // the user never explicitly picks one. Also clear is_streaming now
    // that the compare produced a result — the ComparisonResult event
    // below is the compare equivalent of StreamDone.
    {
        let mut s = engine.state().lock().unwrap();
        if !result_a_text.is_empty() {
            s.last_result = result_a_text.clone();
        }
        s.is_streaming = false;
        s.is_done = true;
    }

    engine.emit(UiEvent::ComparisonResult {
        mode_a,
        result_a: result_a_text,
        mode_b,
        result_b: result_b_text,
    });
}
