pub mod capture;
pub mod context;
pub mod replace;

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
