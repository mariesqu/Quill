#![allow(unused_imports)] // pub use re-exports used by tests and cross-module consumers
pub mod capture;
pub mod caret;
pub mod context;
pub mod dwm_shadow;
pub mod hotkey;
pub mod replace;
pub mod traits;
pub mod tray;
pub mod uia;

pub use traits::{
    CaptureResult, CaptureSource, ContextProbe, ScreenRect, TextCapture, TextReplace,
};

use std::sync::atomic::AtomicU64;
use std::sync::{LazyLock, Mutex};

/// Process-wide cached `Enigo` instance shared by `capture::simulate_copy`
/// and `replace::simulate_paste`. Creating a new `Enigo` opens an
/// X11/Wayland/AppKit/Win32 connection; on Linux in particular this is
/// non-trivial and wasteful to do on every hotkey press.
///
/// Access is serialised through a `Mutex` so concurrent hotkey + paste-back
/// sequences don't race. `Option` because construction can fail (e.g. a
/// headless build environment); callers simply no-op on `None`.
pub(crate) static SHARED_ENIGO: LazyLock<Mutex<Option<enigo::Enigo>>> =
    LazyLock::new(|| Mutex::new(enigo::Enigo::new(&enigo::Settings::default()).ok()));

/// Paste-back generation counter. Incremented on every `paste_text` call
/// (BEFORE spawning the delayed-restore thread); each restore thread
/// captures the current generation and only restores if it still matches
/// when the 500 ms delay elapses. Two pastes within the delay window
/// cause the first's snapshot to be stale — it skips the restore and
/// leaves the second paste's state on the clipboard.
pub(crate) static PASTE_GENERATION: AtomicU64 = AtomicU64::new(0);
