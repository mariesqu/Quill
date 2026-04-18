use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

// Note on `.lock().unwrap()` throughout this module: release builds set
// `panic = "abort"` in `Cargo.toml`, so a panic inside a handler aborts
// the process before the lock can be poisoned. Debug builds would see
// poisoning, but a panic there is already a "fix the bug" signal — not
// something to recover from. `parking_lot::Mutex` would also work here
// but adds a dependency for a guarantee we already have from the abort
// policy.

use tokio::sync::oneshot;

use crate::core::config::Config;
use crate::core::modes::{ChainConfig, ModeConfig};

pub mod compare;
pub mod hotkey_flow;
pub mod streaming;
pub mod tutor_flow;

// ── Engine state ──────────────────────────────────────────────────────────────

use tokio::sync::mpsc;

use crate::platform::traits::{ContextProbe, TextCapture, TextReplace};
use crate::providers::Provider;
use crate::state::{AppState, UiEvent};

/// Cheap-to-clone handle to the running engine. Internally an `Arc<EngineInner>`.
#[derive(Clone)]
pub struct Engine {
    inner: Arc<EngineInner>,
}

pub(crate) struct EngineInner {
    pub config: Config,
    pub modes: HashMap<String, ModeConfig>,
    pub chains: HashMap<String, ChainConfig>,

    pub(crate) state: Arc<Mutex<AppState>>,
    pub(crate) events: mpsc::UnboundedSender<UiEvent>,

    pub(crate) capture: Arc<dyn TextCapture>,
    pub(crate) replace: Arc<dyn TextReplace>,
    pub(crate) context: Arc<dyn ContextProbe>,
    pub(crate) provider: Arc<dyn Provider>,

    pub(crate) cancel_tx: Mutex<Option<oneshot::Sender<()>>>,
    /// Dedicated cancel for the in-flight `compare_modes` operation.
    /// Compare has its OWN cancellation lifecycle distinct from the
    /// single-stream `cancel_tx` above: a compare runs two silent
    /// streams concurrently via `tokio::join!`, and a user-initiated
    /// cancel must terminate both arms simultaneously.
    pub(crate) compare_cancel_tx: Mutex<Option<oneshot::Sender<()>>>,
    /// Dedicated cancel for the in-flight tutor flow (explain / generate
    /// lesson). Same lifecycle shape as `compare_cancel_tx`: one sender
    /// per in-flight operation, taken by `cancel_stream` to terminate the
    /// tutor's silent streams without affecting an unrelated single or
    /// compare stream that might also be running.
    pub(crate) tutor_cancel_tx: Mutex<Option<oneshot::Sender<()>>>,
    /// Set by `cancel_stream_user` (CANCEL button in the overlay) and cleared
    /// by the cancel arm of whichever stream observes it. Esc uses the silent
    /// Dismiss path (`UiCommand::Dismiss` → engine-chained `cancel_stream()`),
    /// which does NOT touch this flag. Differentiates a
    /// user-initiated cancel — which must surface a "Cancelled" toast and
    /// clear UI flags via `StreamCancelled` — from an engine-chained
    /// cancel fired at the start of `execute_mode` / `execute_chain` /
    /// `compare_modes` / `Dismiss` to abort the previous in-flight stream
    /// before launching a new one (those must NOT emit StreamCancelled:
    /// doing so would wipe the newly-starting stream's buffer and surface
    /// a spurious toast).
    pub(crate) user_cancel_flag: AtomicBool,

    /// Serializes disk writes to `user.yaml`. `save_user_config` does a
    /// load → merge → write cycle with no file-level lock, so two concurrent
    /// writers racing on the channel (e.g. SaveConfig + SetLanguage fired
    /// back-to-back) can lose updates. Holding this async lock around the
    /// full load-modify-save sequence eliminates the race.
    pub(crate) config_write_lock: tokio::sync::Mutex<()>,
}

impl Engine {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: Config,
        modes: HashMap<String, ModeConfig>,
        chains: HashMap<String, ChainConfig>,
        state: Arc<Mutex<AppState>>,
        events: mpsc::UnboundedSender<UiEvent>,
        capture: Arc<dyn TextCapture>,
        replace: Arc<dyn TextReplace>,
        context: Arc<dyn ContextProbe>,
        provider: Arc<dyn Provider>,
    ) -> Self {
        Self {
            inner: Arc::new(EngineInner {
                config,
                modes,
                chains,
                state,
                events,
                capture,
                replace,
                context,
                provider,
                cancel_tx: Mutex::new(None),
                compare_cancel_tx: Mutex::new(None),
                tutor_cancel_tx: Mutex::new(None),
                user_cancel_flag: AtomicBool::new(false),
                config_write_lock: tokio::sync::Mutex::new(()),
            }),
        }
    }

    pub fn config(&self) -> &Config {
        &self.inner.config
    }
    pub fn modes(&self) -> &HashMap<String, ModeConfig> {
        &self.inner.modes
    }
    pub fn chains(&self) -> &HashMap<String, ChainConfig> {
        &self.inner.chains
    }

    pub fn history_enabled(&self) -> bool {
        self.inner.config.history.enabled
    }

    pub fn tutor_enabled(&self) -> bool {
        self.inner.config.tutor.enabled && self.history_enabled()
    }

    /// Fire every in-flight cancel channel. Returns `true` only if at
    /// least one sender's receiver was still alive to observe the
    /// signal. Does NOT touch `user_cancel_flag` — flag management is
    /// the caller's job (`cancel_stream` clears it, `cancel_stream_user`
    /// sets it).
    ///
    /// Counting `fired` only on `send().is_ok()` matters because slots
    /// can legitimately hold stale senders after natural completion
    /// (compare + single + tutor all leave the slot populated; the
    /// install paths are fire-before-replace so the stale sender is
    /// drained safely on the NEXT install). If we counted stale takes,
    /// `cancel_stream_user` would report fired=true on idle cancels and
    /// the leaked `user_cancel_flag` would stick, bleeding into the
    /// next unrelated stream's cancel arm.
    fn fire_all_cancels(&self) -> bool {
        // No current caller holds two of these slot mutexes simultaneously —
        // each is locked, taken, and dropped before the next. This order is
        // declared prophylactically for future contributors: if you ever need
        // to hold more than one, acquire in the order cancel_tx →
        // compare_cancel_tx → tutor_cancel_tx to avoid potential deadlock.
        let mut fired = false;
        if let Some(tx) = self.inner.cancel_tx.lock().unwrap().take() {
            if tx.send(()).is_ok() {
                fired = true;
            }
        }
        // Also cancel any in-flight compare operation. Compare's two
        // silent streams don't install the single-stream cancel_tx, so
        // without this the user's cancel would only affect single
        // streams and leave compare running in the background.
        if let Some(tx) = self.inner.compare_cancel_tx.lock().unwrap().take() {
            if tx.send(()).is_ok() {
                fired = true;
            }
        }
        // Same reasoning for the tutor flow — explain/generate-lesson run
        // silent streams that the base cancel_tx never sees.
        if let Some(tx) = self.inner.tutor_cancel_tx.lock().unwrap().take() {
            if tx.send(()).is_ok() {
                fired = true;
            }
        }
        fired
    }

    /// Engine-initiated cancel. Fires the three cancel channels WITHOUT
    /// setting the user-cancel flag.
    ///
    /// Callers: `execute_mode`, `execute_chain`, `compare_modes`, and the
    /// `Dismiss` handler. The chained-start paths need to abort any previous
    /// in-flight stream before launching a new one; `Dismiss` drops an open
    /// overlay without leaving a zombie stream behind. A StreamCancelled
    /// emission here would wipe the newly-starting stream's UI state
    /// (buffer, flags) and surface a spurious "Cancelled" toast on
    /// workspace/palette, so this path stays silent.
    ///
    /// CRITICAL: clears `user_cancel_flag` BEFORE firing. Two leak paths
    /// this guards against:
    ///   1. Rapid CANCEL → Mode B: user hits CANCEL (flag=true, channel
    ///      fired), then immediately selects Mode B. Mode A's cancel arm
    ///      wakes on another worker AFTER begin_stream/StreamStart(B) —
    ///      if the flag were still true it would emit StreamCancelled
    ///      and wipe Mode B's fresh buffer.
    ///   2. Natural completion race: a stream ends the same microsecond
    ///      the user hits CANCEL. The stream-end arm wins the `select!`
    ///      and exits, but cancel_stream_user's `store(true)` already
    ///      landed. The flag leaks — the next engine-chained cancel
    ///      would inherit it. Clearing here breaks that inheritance.
    ///
    /// Returns `true` if any cancel channel actually fired a sender —
    /// i.e. a stream was in flight and its cancel arm WILL observe the
    /// (now-false) flag.
    #[must_use = "cancel_stream returns whether a cancel was delivered — ignore with `let _ = ...` if intentional"]
    pub fn cancel_stream(&self) -> bool {
        self.inner.user_cancel_flag.store(false, Ordering::Release);
        self.fire_all_cancels()
    }

    /// User-initiated cancel (CANCEL button in the overlay; dispatched via
    /// `UiCommand::CancelStream`). The flag must be set BEFORE firing the
    /// cancel channels — the receiving cancel arm runs on another task
    /// and can wake up as soon as `tx.send` completes, so setting the
    /// flag after the send is a real race (the arm might read the old
    /// `false`).
    ///
    /// Never drains the flag on a no-op. An idle cancel (nothing in flight)
    /// LEAKS `user_cancel_flag = true` until consumed by either:
    ///   (a) the next engine-chained `cancel_stream()` — invoked by
    ///       `execute_mode`, `execute_chain`, `compare_modes`, and the
    ///       `Dismiss` handler before they launch a new flow; or
    ///   (b) an explicit `take_user_cancel()` drain at a stream entry
    ///       point — `run_single_stream` closes the execute_mode →
    ///       set_cancel_tx gap; the tutor entry points (`explain_entry` /
    ///       `generate_lesson`) drain pre-install AND post-install to
    ///       catch cancels landing in the predecessor-fire window.
    /// The leak is acceptable because every subsequent user-triggered
    /// operation drains it before proceeding.
    ///
    /// The previous "first_setter && !fired → drain" shortcut destroyed
    /// cancel signals that arrived during the stream-setup gap between
    /// `cancel_stream()` draining all slots and `set_cancel_tx()`
    /// installing a fresh sender: `fire_all_cancels` returned false
    /// (nothing live), the drain branch fired, and the subsequent
    /// stream's post-install `take_user_cancel` read false — user's
    /// cancel lost. That gap can span TTFT (tens of ms to seconds), so
    /// the shortcut traded idle-leak hygiene for correctness on the hot
    /// path.
    #[must_use = "cancel_stream_user returns whether a cancel was delivered — ignore with `let _ = ...` if intentional"]
    pub fn cancel_stream_user(&self) -> bool {
        self.inner.user_cancel_flag.store(true, Ordering::Release);
        self.fire_all_cancels()
    }

    /// Consume the user-cancel flag. Returns `true` only when the cancel
    /// was user-initiated — the stream cancel arms use this to decide
    /// whether to emit `UiEvent::StreamCancelled` (clear UI + toast) or
    /// exit silently so the chained next-stream's `StreamStart` owns the
    /// UI reset.
    pub(crate) fn take_user_cancel(&self) -> bool {
        self.inner.user_cancel_flag.swap(false, Ordering::AcqRel)
    }

    /// Send a `UiEvent`. Drops silently if the receiver has been closed
    /// (only happens during shutdown).
    pub(crate) fn emit(&self, event: UiEvent) {
        let _ = self.inner.events.send(event);
    }

    pub(crate) fn state(&self) -> &Arc<Mutex<AppState>> {
        &self.inner.state
    }
    pub(crate) fn capture(&self) -> &Arc<dyn TextCapture> {
        &self.inner.capture
    }
    pub(crate) fn replace(&self) -> &Arc<dyn TextReplace> {
        &self.inner.replace
    }
    pub(crate) fn context(&self) -> &Arc<dyn ContextProbe> {
        &self.inner.context
    }
    pub(crate) fn provider(&self) -> &Arc<dyn Provider> {
        &self.inner.provider
    }

    /// Fire-before-replace install of the single-stream cancel channel.
    /// A stale sender left in the slot (natural-completion path no
    /// longer drains it — see streaming::run_single_stream) is fired on
    /// the next install; its receiver has been dropped so the send
    /// fails harmlessly. This avoids the clobber class where a
    /// successor stream B's install could have its Sender stolen by
    /// predecessor A's manual cleanup.
    pub(crate) fn set_cancel_tx(&self, tx: oneshot::Sender<()>) {
        let mut slot = self.inner.cancel_tx.lock().unwrap();
        if let Some(old) = slot.take() {
            let _ = old.send(());
        }
        *slot = Some(tx);
    }

    pub async fn execute_mode(
        &self,
        mode: String,
        language: String,
        extra_instruction: Option<String>,
    ) {
        let _ = self.cancel_stream();

        {
            let mut s = self.state().lock().unwrap();
            s.begin_stream(&mode, &language);
        }

        self.emit(UiEvent::StreamStart {
            mode: mode.clone(),
            language: language.clone(),
        });

        // Resolve active context OUTSIDE any lock — FFI can block for tens of ms.
        let ctx_probe = self.context().clone();
        let ctx = tokio::task::spawn_blocking(move || ctx_probe.active_context())
            .await
            .unwrap_or_default();

        let (original, system, user, history_en, tutor_en) = {
            let s = self.state().lock().unwrap();
            let built = crate::core::prompt::build_prompt(
                &s.selected_text,
                &mode,
                self.modes(),
                &ctx,
                &language,
                &self.config().persona,
                extra_instruction.as_deref(),
            );
            match built {
                Ok((sys, usr)) => (
                    s.selected_text.clone(),
                    sys,
                    usr,
                    self.history_enabled(),
                    self.tutor_enabled(),
                ),
                Err(err) => {
                    drop(s);
                    self.state().lock().unwrap().fail_stream(&err);
                    self.emit(UiEvent::StreamError { message: err });
                    return;
                }
            }
        };

        let engine = self.clone();
        if let Some(full_text) = streaming::run_single_stream(engine.clone(), system, user).await {
            streaming::finalize_result(
                engine, &original, &full_text, &mode, &language, history_en, tutor_en,
            )
            .await;
        }
    }

    pub async fn execute_chain(
        &self,
        chain_id: String,
        language: String,
        extra_instruction: Option<String>,
    ) {
        let _ = self.cancel_stream();

        let steps = match self.chains().get(&chain_id).map(|c| c.steps.clone()) {
            Some(s) if !s.is_empty() => s,
            _ => {
                let msg = format!("Unknown or empty chain: {chain_id}");
                self.state().lock().unwrap().fail_stream(&msg);
                self.emit(UiEvent::StreamError { message: msg });
                return;
            }
        };

        let total = steps.len();
        let chain_original = self.state().lock().unwrap().selected_text.clone();
        let mut current_text = chain_original.clone();

        // Resolve context once — the active app doesn't change mid-chain.
        let ctx_probe = self.context().clone();
        let ctx = tokio::task::spawn_blocking(move || ctx_probe.active_context())
            .await
            .unwrap_or_default();

        {
            let mut s = self.state().lock().unwrap();
            s.begin_stream(&format!("chain:{chain_id}"), &language);
        }

        self.emit(UiEvent::StreamStart {
            mode: format!("chain:{chain_id}"),
            language: language.clone(),
        });

        // Cleanup helper — must run on EVERY exit path (success, cancel,
        // prompt-build error) so the user's original selection and a clean
        // chain_progress are always restored. A scope-local closure keeps
        // the logic in one place without requiring a full guard type.
        let restore_session = |engine: &Engine, original: &str| {
            let mut s = engine.state().lock().unwrap();
            s.selected_text = original.to_string();
            s.chain_progress = None;
        };

        for (idx, step_mode) in steps.iter().enumerate() {
            self.emit(UiEvent::ChainProgress {
                step: idx + 1,
                total,
                mode: step_mode.clone(),
            });
            {
                let mut s = self.state().lock().unwrap();
                s.chain_progress = Some(crate::state::ChainProgress {
                    step: idx + 1,
                    total,
                    mode: step_mode.clone(),
                });
                // The next step's input is the previous step's output.
                s.selected_text = current_text.clone();
            }

            let (system, user) = {
                let s = self.state().lock().unwrap();
                let built = crate::core::prompt::build_prompt(
                    &current_text,
                    step_mode,
                    self.modes(),
                    &ctx,
                    &language,
                    &self.config().persona,
                    if idx == 0 {
                        extra_instruction.as_deref()
                    } else {
                        None
                    },
                );
                match built {
                    Ok(p) => p,
                    Err(err) => {
                        drop(s);
                        self.state().lock().unwrap().fail_stream(&err);
                        self.emit(UiEvent::StreamError { message: err });
                        // Restore before bailing so a retry sees the right
                        // selection — previously the chain's intermediate
                        // step text was left in selected_text.
                        restore_session(self, &chain_original);
                        return;
                    }
                }
            };

            match streaming::run_single_stream(self.clone(), system, user).await {
                Some(text) => current_text = text,
                None => {
                    // Cancelled or errored mid-chain. Same restoration — we
                    // must not leave the user's selection pointing at an
                    // intermediate step's output.
                    restore_session(self, &chain_original);
                    return;
                }
            }
        }

        // Restore the original selection so a follow-up retry operates on
        // what the user selected, not the last intermediate step's input.
        let (history_en, tutor_en) = {
            let mut s = self.state().lock().unwrap();
            s.selected_text = chain_original.clone();
            s.chain_progress = None;
            (self.history_enabled(), self.tutor_enabled())
        };

        streaming::finalize_result(
            self.clone(),
            &chain_original,
            &current_text,
            &format!("chain:{chain_id}"),
            &language,
            history_en,
            tutor_en,
        )
        .await;
    }

    pub async fn handle_command(&self, cmd: crate::state::UiCommand) {
        use crate::state::UiCommand::*;
        match cmd {
            ExecuteMode {
                mode,
                language,
                extra,
            } => {
                self.execute_mode(mode, language, extra).await;
            }
            ExecuteChain {
                chain_id,
                language,
                extra,
            } => {
                self.execute_chain(chain_id, language, extra).await;
            }
            CompareModes {
                mode_a,
                mode_b,
                language,
                extra,
            } => {
                compare::compare_modes(self.clone(), mode_a, mode_b, language, extra).await;
            }
            RequestTutorExplain { entry_id } => {
                // id == 0 is a sentinel meaning "the UI didn't have an id,
                // use whatever was last finalised". This matches the
                // default-zero property on AppBridge.last-entry-id so a
                // click fired before StreamDone has propagated still hits
                // the right entry.
                let resolved_id = if entry_id == 0 {
                    self.state().lock().unwrap().last_entry_id.unwrap_or(0)
                } else {
                    entry_id
                };
                if resolved_id == 0 {
                    // Still nothing — no-op silently (no toast spam; the
                    // pane already shows "No explanation yet" copy).
                    return;
                }
                // History calls are blocking SQLite; run on the blocking
                // pool so we don't stall a tokio worker.
                let entry_res = tokio::task::spawn_blocking(move || {
                    crate::core::history::get_entry(resolved_id)
                })
                .await;
                if let Ok(Ok(entry)) = entry_res {
                    tutor_flow::explain_entry(
                        self.clone(),
                        entry.id,
                        entry.original_text,
                        entry.output_text,
                        entry.mode.unwrap_or_default(),
                        entry.language.unwrap_or_default(),
                    )
                    .await;
                }
            }
            GenerateLesson { period } => {
                tutor_flow::generate_lesson(self.clone(), period).await;
            }
            ConfirmReplace => {
                let (text, focus_target) = {
                    let s = self.state().lock().unwrap();
                    (s.last_result.clone(), s.focus_target)
                };
                if !text.is_empty() {
                    // ORDER MATTERS here. SetForegroundWindow is gated by
                    // UIPI — the calling process must already be foreground
                    // (or have been granted permission via
                    // AllowSetForegroundWindow) for it to succeed. If we
                    // restore the foreground target FIRST while the overlay
                    // is still visible and owning foreground, the OS sees
                    // contention and SetForegroundWindow frequently fails
                    // (symptom: Ctrl+V paste lands back inside the overlay).
                    //
                    // The correct sequence is:
                    //   1. Hide the overlay (DismissOverlay). The workspace
                    //      stays open — it's a separate tier.
                    //   2. Let the OS process the visibility change (small
                    //      sleep; winit's hide isn't instantly synchronous
                    //      with DefWindowProc's foreground reassignment).
                    //   3. NOW restore the user's original HWND as
                    //      foreground — our process no longer has a visible
                    //      window contending, so SetForegroundWindow
                    //      succeeds cleanly.
                    //   4. Another short sleep so the OS has time to deliver
                    //      WM_ACTIVATE to the target before we simulate the
                    //      Ctrl+V keystroke. 60 ms matches Windows' default
                    //      input-queue attachment latency.
                    //   5. Paste.
                    self.emit(crate::state::UiEvent::DismissOverlay);
                    tokio::time::sleep(std::time::Duration::from_millis(60)).await;
                    if let Some(snapshot) = focus_target {
                        restore_foreground_window(snapshot);
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(60)).await;
                    if let Err(e) = self.replace().paste(&text).await {
                        self.emit(crate::state::UiEvent::Error {
                            message: format!("Paste failed: {e}"),
                        });
                    }
                }
            }
            CancelStream => {
                let _ = self.cancel_stream_user();
            }
            Dismiss => {
                // Overlay-only dismiss. The workspace has its own close
                // path (window X / Esc call window.hide() directly on the
                // Slint side — no engine round-trip needed).
                let _ = self.cancel_stream();
                {
                    let mut s = self.state().lock().unwrap();
                    s.is_streaming = false;
                    // Also clear is_done — leaving it latched means the
                    // next summon boots with a stale "done" flag until
                    // begin_stream clears it, and the bridge's DismissOverlay
                    // handler already clears both UI flags via apply_to_all.
                    s.is_done = false;
                }
                self.emit(crate::state::UiEvent::DismissOverlay);
            }
            DismissWorkspace => {
                // Workspace-only dismiss. No stream cancellation — the
                // workspace can be reopened and the last result is still
                // there. The overlay remains untouched.
                self.emit(crate::state::UiEvent::DismissWorkspace);
            }
            LoadHistory { limit } => {
                // SQLite calls are blocking. Running them directly on a
                // tokio worker stalls the scheduler; spawn_blocking moves
                // them to the dedicated blocking pool.
                let result =
                    tokio::task::spawn_blocking(move || crate::core::history::get_recent(limit))
                        .await;
                match result {
                    Ok(Ok(entries)) => {
                        self.state().lock().unwrap().history_entries = entries.clone();
                        self.emit(crate::state::UiEvent::HistoryLoaded(entries));
                    }
                    Ok(Err(e)) => {
                        self.emit(crate::state::UiEvent::Error {
                            message: format!("History load failed: {e}"),
                        });
                    }
                    Err(e) => {
                        self.emit(crate::state::UiEvent::Error {
                            message: format!("History load task failed: {e}"),
                        });
                    }
                }
            }
            ToggleFavorite { entry_id } => {
                let result = tokio::task::spawn_blocking(move || {
                    crate::core::history::toggle_favorite(entry_id)
                })
                .await;
                match result {
                    Ok(Ok(new_state)) => {
                        // Mirror into AppState so the Slint list can re-render.
                        let mut s = self.state().lock().unwrap();
                        if let Some(entry) = s.history_entries.iter_mut().find(|e| e.id == entry_id)
                        {
                            entry.favorited = new_state;
                        }
                        drop(s);
                        self.emit(crate::state::UiEvent::HistoryEntryUpdated {
                            id: entry_id,
                            favorited: new_state,
                        });
                    }
                    Ok(Err(e)) => {
                        self.emit(crate::state::UiEvent::Error {
                            message: format!("Toggle favorite failed: {e}"),
                        });
                    }
                    Err(e) => {
                        self.emit(crate::state::UiEvent::Error {
                            message: format!("Toggle favorite task failed: {e}"),
                        });
                    }
                }
            }
            ExportHistory { format: fmt, path } => {
                // Export walks the full history table + writes the file —
                // both blocking. Run on the blocking pool.
                let path_for_msg = path.clone();
                let result = tokio::task::spawn_blocking(move || match fmt.as_str() {
                    "json" => export_history_json(&path),
                    "csv" => export_history_csv(&path),
                    "md" => export_history_md(&path),
                    other => Err(anyhow::anyhow!("unknown export format: {other}")),
                })
                .await;
                let msg = match result {
                    Ok(Ok(())) => format!("Exported to {}", path_for_msg.display()),
                    Ok(Err(e)) => format!("Export failed: {e}"),
                    Err(e) => format!("Export task failed: {e}"),
                };
                self.emit(crate::state::UiEvent::Toast {
                    kind: crate::state::app_state::ToastKind::Info,
                    message: msg,
                });
            }
            SaveConfig { updates } => {
                let _w = self.inner.config_write_lock.lock().await;
                // save_user_config does sync file I/O — run on blocking pool.
                let result = tokio::task::spawn_blocking(move || {
                    crate::core::config::save_user_config(updates)
                })
                .await;
                match result {
                    Ok(Ok(())) => {
                        self.emit(crate::state::UiEvent::Toast {
                            kind: crate::state::app_state::ToastKind::Success,
                            message: "Settings saved".into(),
                        });
                    }
                    Ok(Err(e)) => {
                        self.emit(crate::state::UiEvent::Error {
                            message: format!("Save config failed: {e}"),
                        });
                    }
                    Err(e) => {
                        self.emit(crate::state::UiEvent::Error {
                            message: format!("Save config task failed: {e}"),
                        });
                    }
                }
            }
            SetLanguage { code } => {
                // Broadcast to every window's AppBridge — the picker on
                // one window must mirror to the other two. No AppState
                // write needed: ExecuteMode carries the language as an arg
                // (sourced from AppBridge.active-language); AppState no
                // longer tracks an authoritative language of its own.
                self.emit(crate::state::UiEvent::LanguageChanged { code: code.clone() });
                // Persist to user.yaml under the config write lock.
                let _w = self.inner.config_write_lock.lock().await;
                let code_for_task = code;
                let result = tokio::task::spawn_blocking(move || {
                    let updates = serde_json::json!({ "language": code_for_task });
                    crate::core::config::save_user_config(updates)
                })
                .await;
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => self.emit(crate::state::UiEvent::Error {
                        message: format!("Save language failed: {e}"),
                    }),
                    Err(e) => self.emit(crate::state::UiEvent::Error {
                        message: format!("Save language task failed: {e}"),
                    }),
                }
            }
            EmitError { message } => {
                self.emit(crate::state::UiEvent::Error { message });
            }
            EmitInfo { message } => {
                self.emit(crate::state::UiEvent::Toast {
                    kind: crate::state::app_state::ToastKind::Info,
                    message,
                });
            }
            SwitchTab { tab } => {
                // The bridge's local handler has already updated the Slint
                // side (AppBridge.current-tab). Routing through the engine
                // lets tab-switch side effects (e.g. auto-load history on
                // entering the History tab) hook here.
                if tab == "history" {
                    // Inlined rather than re-dispatched as another UiCommand
                    // so the tab switch and the history load are observed
                    // atomically by the UI. SQLite is blocking — run the
                    // get_recent call on the blocking pool so we don't stall
                    // a tokio worker.
                    let result =
                        tokio::task::spawn_blocking(move || crate::core::history::get_recent(100))
                            .await;
                    match result {
                        Ok(Ok(entries)) => {
                            self.state().lock().unwrap().history_entries = entries.clone();
                            self.emit(crate::state::UiEvent::HistoryLoaded(entries));
                        }
                        Ok(Err(e)) => {
                            self.emit(crate::state::UiEvent::Error {
                                message: format!("History load failed: {e}"),
                            });
                        }
                        Err(e) => {
                            self.emit(crate::state::UiEvent::Error {
                                message: format!("History load task failed: {e}"),
                            });
                        }
                    }
                }
            }
        }
    }
}

// ── Foreground window restore ────────────────────────────────────────────────

/// Restore foreground to a previously snapshotted HWND. Called right before
/// `Replace`'s Ctrl+V simulation so the paste lands in the original app.
///
/// Windows' `SetForegroundWindow` is gated by UIPI: it only succeeds when
/// the calling process is already foreground OR was granted permission via
/// `AllowSetForegroundWindow`. In our case the overlay just had foreground
/// (the user clicked Replace), so we satisfy rule 1 and the call works.
/// If it still fails we fall back to `BringWindowToTop` which at least
/// raises the Z-order.
fn restore_foreground_window(snapshot: crate::state::FocusSnapshot) {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            BringWindowToTop, IsWindow, SetForegroundWindow, ShowWindow, SW_RESTORE,
        };

        let hwnd = HWND(snapshot.hwnd_raw as *mut _);
        unsafe {
            if !IsWindow(hwnd).as_bool() {
                tracing::warn!(
                    "restore_foreground_window: stale HWND {:p} — window no longer exists",
                    hwnd.0
                );
                return;
            }
            // Unminimize in case the user minimized the target after the
            // hotkey fired but before clicking Replace.
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = BringWindowToTop(hwnd);
            if !SetForegroundWindow(hwnd).as_bool() {
                tracing::warn!(
                    "SetForegroundWindow({:p}) returned false — paste may land on wrong window",
                    hwnd.0
                );
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = snapshot;
    }
}

// ── History export helpers ───────────────────────────────────────────────────

fn export_history_json(dest: &std::path::Path) -> anyhow::Result<()> {
    let entries = crate::core::history::get_all_entries()?;
    let json = serde_json::to_string_pretty(&entries)?;
    std::fs::create_dir_all(dest.parent().unwrap_or(std::path::Path::new(".")))?;
    std::fs::write(dest, json)?;
    Ok(())
}

fn export_history_csv(dest: &std::path::Path) -> anyhow::Result<()> {
    // RFC 4180: CRLF line terminators between records, fields wrapped in
    // double quotes, inner quotes doubled. Literal newlines inside quoted
    // fields are LEGAL — we normalise any `\r\n` inside a field to `\n` so
    // Excel doesn't break the row boundary, but we keep the `\n`s because
    // stripping them would mangle multi-line rewrites.
    let entries = crate::core::history::get_all_entries()?;
    let mut out = String::from("id,timestamp,mode,language,original,output,favorited\r\n");
    for e in entries {
        let original = e.original_text.replace("\r\n", "\n");
        let output = e.output_text.replace("\r\n", "\n");
        let fields = [
            e.id.to_string(),
            e.timestamp,
            e.mode.unwrap_or_default(),
            e.language.unwrap_or_default(),
            original,
            output,
            if e.favorited { "1".into() } else { "0".into() },
        ];
        for (i, f) in fields.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push('"');
            out.push_str(&f.replace('"', "\"\""));
            out.push('"');
        }
        out.push_str("\r\n");
    }
    std::fs::create_dir_all(dest.parent().unwrap_or(std::path::Path::new(".")))?;
    std::fs::write(dest, out)?;
    Ok(())
}

fn export_history_md(dest: &std::path::Path) -> anyhow::Result<()> {
    let entries = crate::core::history::get_all_entries()?;
    let mut out = String::from("# Quill History Export\n\n");
    for e in entries {
        out.push_str(&format!(
            "## {} — {}\n\n**Mode:** {}  \n**Language:** {}  \n\n### Original\n\n{}\n\n### Output\n\n{}\n\n---\n\n",
            e.timestamp,
            if e.favorited { "★" } else { "" },
            e.mode.as_deref().unwrap_or("—"),
            e.language.as_deref().unwrap_or("—"),
            e.original_text,
            e.output_text,
        ));
    }
    std::fs::create_dir_all(dest.parent().unwrap_or(std::path::Path::new(".")))?;
    std::fs::write(dest, out)?;
    Ok(())
}
