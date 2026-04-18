//! Tutor flows: explain a single history entry and generate a daily/weekly
//! lesson from history stats.

use tokio::sync::oneshot;

use crate::core::{history, tutor};
use crate::state::app_state::ToastKind;
use crate::state::UiEvent;

use super::{streaming, Engine};

/// Install a fresh `tutor_cancel_tx` on the engine and return the
/// matching receiver. Mirrors `compare.rs` — `run_silent_stream` ignores
/// the engine's base `cancel_tx`, so tutor flows need their own channel
/// that `cancel_stream` drains.
///
/// Fires any previous tutor sender BEFORE replacing it. Without this,
/// a second concurrent tutor flow (e.g. auto-explain is streaming and
/// the user clicks Generate Lesson) would simply drop the old Sender.
/// The drop wakes the old tutor's `cancel_rx`, which then races the new
/// tutor's cleanup and ends up stealing the new tutor's sender out of
/// the slot — leaving the new tutor uncancellable. Firing the old
/// sender here gives the old tutor its own cancel signal; its cleanup
/// path no longer touches the slot (see natural-completion tail below).
fn install_tutor_cancel(engine: &Engine) -> oneshot::Receiver<()> {
    let (tx, rx) = oneshot::channel::<()>();
    let mut slot = engine.inner.tutor_cancel_tx.lock().unwrap();
    if let Some(old) = slot.take() {
        let _ = old.send(());
    }
    *slot = Some(tx);
    rx
}

pub async fn explain_entry(
    engine: Engine,
    entry_id: i64,
    original: String,
    output: String,
    mode: String,
    language: String,
) {
    if !engine.tutor_enabled() {
        engine.emit(UiEvent::Toast {
            kind: ToastKind::Warning,
            message: "Enable tutor.enabled in user.yaml to use this feature".into(),
        });
        return;
    }
    let system = tutor::EXPLAIN_SYSTEM.to_string();
    let user = tutor::build_explain_prompt(&original, &output, &mode, &language);

    // Drain any idle-leaked user-cancel flag BEFORE install_tutor_cancel
    // fires the predecessor tutor's sender. Without this, the predecessor's
    // cancel arm wakes, reads the leaked flag via take_user_cancel, and
    // emits a spurious "Cancelled" toast for a cancel the user never
    // issued against that stream. Legitimate user cancels during the
    // select below still flow through correctly: CANCEL sets the flag,
    // fires the new tx, the arm wakes, take_user_cancel reads true.
    let _ = engine.take_user_cancel();
    let mut cancel_rx = install_tutor_cancel(&engine);

    // Post-install drain-and-check. If CANCEL landed in the microsecond
    // window between the pre-install drain and install_tutor_cancel (or
    // between install and this point), the flag is set but no sender was
    // live to relay it into cancel_rx — the select below would not observe
    // the cancel and the tutor would proceed. Mirrors run_single_stream.
    // Tutor-specific: emit Info toast only (never StreamCancelled) to honor
    // the tutor cancel contract; is_done on the primary stream is not touched.
    if engine.take_user_cancel() {
        engine.emit(UiEvent::Toast {
            kind: ToastKind::Info,
            message: "Cancelled".into(),
        });
        return;
    }

    // run_silent_stream now returns Result<String, String>: convert to
    // Option at the arm boundary so both select! arms unify, and surface
    // provider errors as an Error toast instead of silently blanking the
    // explain pane.
    let (explanation, cancelled) = tokio::select! {
        v = streaming::run_silent_stream(engine.clone(), system, user) => match v {
            Ok(s) => (Some(s), false),
            Err(e) => {
                engine.emit(UiEvent::Error { message: e });
                (None, false)
            }
        },
        _ = &mut cancel_rx => (None, true),
    };

    // Deliberately do NOT take the tutor_cancel_tx slot here. The slot is
    // shared across concurrent tutor flows (auto-explain + user-triggered
    // lesson); only `install_tutor_cancel` is allowed to swap it. If this
    // cleanup took the slot, a racing second tutor that just installed its
    // own sender would have that sender stolen and become uncancellable.
    // A stale sender left in the slot is harmless: the next
    // `install_tutor_cancel` fires and replaces it, and `cancel_stream`
    // drains it unconditionally.

    if cancelled {
        // Tutor runs AFTER streaming::finalize_result has already latched
        // `is_done = true` on the primary stream. A tutor cancel MUST NOT
        // emit `UiEvent::StreamCancelled` (like streaming.rs / compare.rs
        // do) — that broadcast clears `is_done` across every AppBridge and
        // makes the replaceable primary result vanish from the overlay.
        // Plain Info toast only; leave primary stream flags alone.
        //
        // Gate the toast on the user-cancel flag: tutor cancels come in
        // two flavours. A user-initiated cancel (Esc / CANCEL button)
        // set the flag → surface the "Cancelled" toast. An engine-chained
        // cancel (user picked Mode B while tutor was still running) drained
        // the flag in `cancel_stream` → stay silent; the new flow's
        // StreamStart owns the UI, and a stray toast here is jarring UX.
        // `take_user_cancel` consumes the flag so a subsequent cancel arm
        // on another stream doesn't re-observe it.
        if engine.take_user_cancel() {
            engine.emit(UiEvent::Toast {
                kind: ToastKind::Info,
                message: "Cancelled".into(),
            });
        }
        return;
    }

    if let Some(explanation) = explanation {
        if entry_id > 0 {
            // rusqlite calls are blocking. Hop them onto the blocking pool
            // so we don't stall a tokio worker on disk I/O.
            let explanation_for_db = explanation.clone();
            let save_res = tokio::task::spawn_blocking(move || {
                history::save_tutor_explanation(entry_id, &explanation_for_db)
            })
            .await;
            match save_res {
                Ok(Ok(())) => {}
                Ok(Err(e)) => tracing::warn!("failed to save tutor explanation: {e}"),
                Err(e) => tracing::warn!("tutor explanation save task failed: {e}"),
            }
        }
        engine.emit(UiEvent::TutorExplanation {
            entry_id,
            text: explanation,
        });
    }
}

pub async fn generate_lesson(engine: Engine, period: String) {
    if !engine.tutor_enabled() {
        engine.emit(UiEvent::Toast {
            kind: ToastKind::Warning,
            message: "Enable tutor.enabled in user.yaml to use this feature".into(),
        });
        return;
    }
    let days = if period == "daily" { 1 } else { 7 };
    // history::get_stats runs SQL aggregates — move to the blocking pool.
    let stats = match tokio::task::spawn_blocking(move || history::get_stats(days)).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            engine.emit(UiEvent::Error {
                message: format!("History error: {e}"),
            });
            return;
        }
        Err(e) => {
            engine.emit(UiEvent::Error {
                message: format!("History task failed: {e}"),
            });
            return;
        }
    };

    let system = tutor::LESSON_SYSTEM.to_string();
    let user = tutor::build_lesson_prompt(&stats, &period);

    // See explain_entry: drain any idle-leaked user-cancel flag BEFORE
    // install_tutor_cancel fires the predecessor's sender, so the
    // predecessor's cancel arm doesn't inherit stale user intent and
    // emit a spurious "Cancelled" toast.
    let _ = engine.take_user_cancel();
    let mut cancel_rx = install_tutor_cancel(&engine);

    // Post-install drain-and-check — see explain_entry for full rationale.
    // Catches a CANCEL that landed in the pre-install → install gap or
    // install → select gap, where the flag is set but no live sender relayed
    // it into cancel_rx.
    if engine.take_user_cancel() {
        engine.emit(UiEvent::Toast {
            kind: ToastKind::Info,
            message: "Cancelled".into(),
        });
        return;
    }

    // Same unify-types + surface-Err treatment as explain_entry.
    let (lesson, cancelled) = tokio::select! {
        v = streaming::run_silent_stream(engine.clone(), system, user) => match v {
            Ok(s) => (Some(s), false),
            Err(e) => {
                engine.emit(UiEvent::Error { message: e });
                (None, false)
            }
        },
        _ = &mut cancel_rx => (None, true),
    };

    // See explain_entry: do NOT take the slot here; it's shared.

    if cancelled {
        // Same tutor-specific contract as explain_entry: do NOT emit
        // StreamCancelled here — that would wipe is_done on the primary
        // stream's overlay. Plain Info toast only, and gated on the
        // user-cancel flag so engine-chained cancels (Mode B selected
        // mid-lesson) stay silent — the new flow's StreamStart owns the
        // UI.
        if engine.take_user_cancel() {
            engine.emit(UiEvent::Toast {
                kind: ToastKind::Info,
                message: "Cancelled".into(),
            });
        }
        return;
    }

    if let Some(lesson) = lesson {
        let lang = engine.config().tutor.lesson_language.clone();
        let period_for_db = period.clone();
        let lesson_for_db = lesson.clone();
        let save_res = tokio::task::spawn_blocking(move || {
            history::save_lesson(&period_for_db, &lesson_for_db, &lang)
        })
        .await;
        match save_res {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::warn!("failed to save tutor lesson: {e}"),
            Err(e) => tracing::warn!("tutor lesson save task failed: {e}"),
        }
        engine.emit(UiEvent::TutorLesson {
            period,
            text: lesson,
        });
    }
}
