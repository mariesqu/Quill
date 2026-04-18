//! Tier 3 platform smoke tests. Gated behind `feature = "smoke"`.
//!
//! These tests exercise the platform/ modules against real Windows APIs.
//! They verify that each module can initialise, perform its operation, and
//! tear down without crashing. They do NOT assert UI correctness or pixel
//! accuracy.
//!
//! Run with:
//!
//! ```bash
//! cargo test --test platform_smoke --features smoke -- --test-threads=1
//! ```
//!
//! `--test-threads=1` is required: these tests touch global OS state
//! (clipboard, tray, foreground window) and must not run concurrently.

#![cfg(feature = "smoke")]

use std::time::Duration;

use quill::platform::caret::{CaretHookService, FocusEvent};
use quill::platform::hotkey::{parse_hotkey_spec, HotkeyService};
use quill::platform::tray::{TrayService, TRAY_ICON_PNG};
use quill::platform::uia::Uia;

#[test]
fn hotkey_parser_accepts_canonical_spec() {
    use global_hotkey::hotkey::{Code, Modifiers};

    let h = parse_hotkey_spec("Ctrl+Shift+Space").unwrap();

    // HotKey has public fields `mods` and `key` — inspect them directly.
    assert_eq!(h.mods, Modifiers::CONTROL | Modifiers::SHIFT);
    assert_eq!(h.key, Code::Space);

    // Modifiers-only spec must fail (no key token).
    assert!(parse_hotkey_spec("Ctrl+Shift").is_err());

    // Single key (no modifiers) must succeed.
    assert!(parse_hotkey_spec("Space").is_ok());
}

#[test]
fn hotkey_service_round_trip() {
    // Register → unregister → re-register a different combo.
    // Verifies that the GlobalHotKeyManager holds a registration through
    // a full lifecycle without leaking the combo.
    let mut service = HotkeyService::new().expect("create HotkeyService");
    service
        .register("Ctrl+Alt+F12")
        .expect("register 1st hotkey");
    service.unregister_current().expect("unregister");
    service
        .register("Ctrl+Alt+F11")
        .expect("register 2nd hotkey");
    // Drop unregisters implicitly.
}

#[test]
fn tray_service_builds_and_drops() {
    // Verifies that TrayService::new succeeds with the embedded PNG.
    // The icon actually appears in the tray during this test — eyeball it.
    let tray = TrayService::new(TRAY_ICON_PNG).expect("build tray");
    std::thread::sleep(Duration::from_millis(500));
    // Drain any spurious events during the window. We don't assert the
    // event count — the user might have clicked during the 500ms window
    // — just verify poll() doesn't panic and the tray drops cleanly.
    let _events = tray.poll();
    drop(tray);
}

#[test]
fn caret_hook_service_starts_and_stops() {
    // Verify that the WinEvent hook thread installs, pumps messages, and
    // tears down cleanly. We don't assert specific events — we verify the
    // thread doesn't panic and some events flow (or none flow — both OK).
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<FocusEvent>();
    let mut service = CaretHookService::start(tx).expect("start caret hook");

    std::thread::sleep(Duration::from_millis(200));

    let mut received = 0;
    while rx.try_recv().is_ok() {
        received += 1;
    }
    println!("smoke: caret hook received {received} event(s) in 200ms");

    service.stop();
    std::thread::sleep(Duration::from_millis(50));
}

#[test]
fn uia_can_initialize_on_worker_thread() {
    // Verify the UIA thread-local wrapper can initialise COM, create an
    // IUIAutomation instance, and call GetFocusedElement without panicking.
    // Returning an error from focused_element is acceptable (nothing focused
    // in a headless test environment), but the init path must succeed.
    let handle = std::thread::spawn(|| {
        let result = Uia::with(|uia| {
            // Attempt to get focused element — may succeed or Err depending
            // on headless test env. Either outcome is fine as long as we
            // didn't panic during COM init.
            let _ = uia.focused_element();
        });
        result.expect("Uia::with failed to initialise COM or UIA instance")
    });
    handle.join().expect("worker thread panicked");
}

#[test]
#[ignore = "requires notepad.exe as scripted target; run manually"]
fn capture_flow_against_notepad() {
    // End-to-end: spawn notepad, type "Hello smoke", select all, invoke
    // capture_selection_blocking, assert the text matches.
    //
    // Ignored by default because it interacts with the GUI. Run with:
    //     cargo test --test platform_smoke --features smoke \
    //         capture_flow_against_notepad -- --ignored --test-threads=1
    //
    // The test opens notepad in the foreground — don't run it while typing
    // into anything else.
    use quill::platform::capture::capture_selection_blocking;

    let mut child = std::process::Command::new("notepad.exe")
        .spawn()
        .expect("spawn notepad");

    std::thread::sleep(Duration::from_millis(800));

    // TODO (Plan 2+): synthesize keyboard input via enigo/SendInput to type
    // "Hello smoke" and Ctrl+A. For Plan 1 this is a manual placeholder.

    let result = capture_selection_blocking();
    println!(
        "captured: text={:?} source={:?}",
        result.text, result.source
    );

    child.kill().ok();
    child.wait().ok();
}
