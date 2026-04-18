#![allow(dead_code)] // clipboard monitor helpers — opt-in feature, not on the default hot path
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

pub const POLL_INTERVAL_MS: u64 = 500;
const MIN_WORDS: usize = 3;

/// Default mute window used by Quill's own clipboard writes. Computed from
/// the poll interval so it automatically stays correct if the interval
/// changes — two full polls plus 200 ms of headroom covers the
/// write → simulate-paste → restore sequence and the next one or two poll
/// cycles where the monitor could otherwise observe either transition.
pub const DEFAULT_MUTE_MS: u64 = POLL_INTERVAL_MS * 2 + 200;

/// Monotonic clock anchor. All mute-window deadlines are stored as
/// "milliseconds since this anchor" so that wall-clock changes
/// (NTP correction, DST, user setting the clock) cannot make a pending
/// mute window expire too early or hang around forever.
static CLOCK_ANCHOR: LazyLock<Instant> = LazyLock::new(Instant::now);

/// Monotonic millis since `CLOCK_ANCHOR`. Stored in an AtomicU64 as a
/// "mute-until" deadline; 0 means "not muted".
pub static MUTE_UNTIL_MONO_MS: AtomicU64 = AtomicU64::new(0);

fn mono_now_ms() -> u64 {
    // `as u64` of a u128 — the LazyLock is initialised at first call, so
    // the value is always non-negative and well under 2^64 ms even over
    // decades of uptime.
    CLOCK_ANCHOR.elapsed().as_millis() as u64
}

/// Temporarily silence the clipboard monitor for `duration_ms` milliseconds.
/// Uses a monotonic clock so wall-clock adjustments cannot affect the window.
///
/// The monitor still tracks `last_text` during the mute window so it doesn't
/// re-fire on the muted change once the window expires.
pub fn mute_for(duration_ms: u64) {
    MUTE_UNTIL_MONO_MS.store(mono_now_ms() + duration_ms, Ordering::Relaxed);
}

/// Mute the monitor for the default window (see [`DEFAULT_MUTE_MS`]).
/// Use this at call sites that don't want to hardcode a duration.
pub fn mute_default() {
    mute_for(DEFAULT_MUTE_MS);
}

/// Spawns a background clipboard monitor task.
///
/// Sends newly-copied text to the returned receiver. The monitor is only
/// started when the feature is enabled in config (see `main.rs`); there's
/// no runtime toggle — enabling/disabling requires an app restart — so we
/// don't plumb a shared `AtomicBool` through.
///
/// * receiver dropped  → monitor thread exits cleanly.
/// * mute window active → monitor updates `last_text` but skips emission.
pub fn start_clipboard_monitor() -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel::<String>(32);

    std::thread::spawn(move || {
        let Ok(mut clipboard) = arboard::Clipboard::new() else {
            tracing::warn!(
                "clipboard monitor: arboard::Clipboard::new() failed — feature disabled for this session"
            );
            return;
        };
        let mut last_text = clipboard.get_text().unwrap_or_default();

        loop {
            std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));

            // Exit cleanly if every receiver has been dropped.
            if tx.is_closed() {
                break;
            }

            let Ok(current) = clipboard.get_text() else {
                continue;
            };
            if current == last_text {
                continue;
            }
            let previous_text = std::mem::replace(&mut last_text, current.clone());

            // Respect the mute window — we've already updated `last_text`
            // above, so we won't re-fire on this value after the mute lifts.
            if mono_now_ms() < MUTE_UNTIL_MONO_MS.load(Ordering::Relaxed) {
                continue;
            }

            // Ignore the very first transition from empty → something (e.g.
            // the monitor starting up catches whatever was already copied).
            if previous_text.is_empty() {
                continue;
            }

            let word_count = current.split_whitespace().count();
            if word_count >= MIN_WORDS {
                // If the receiver vanished between the check above and now,
                // `blocking_send` returns Err — exit the loop rather than
                // leak the thread.
                if tx.blocking_send(current).is_err() {
                    break;
                }
            }
        }
    });

    rx
}
