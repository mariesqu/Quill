//! Tier 2 integration tests: engine orchestration with in-memory fakes.

mod common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use tokio::sync::{mpsc, Mutex as TokioMutex};

use common::fakes::{FakeCapture, FakeContext, FakeProvider, FakeReplace};

use quill::core::config::Config;
use quill::core::modes::{ChainConfig, ModeConfig};
use quill::engine::Engine;
use quill::platform::traits::{ContextProbe, TextCapture, TextReplace};
use quill::providers::Provider;
use quill::state::{AppState, UiCommand, UiEvent};

/// Process-wide async mutex that serialises all `handle_hotkey` calls across
/// parallel integration tests. `HOTKEY_BUSY` is a process-level static, so
/// concurrent tests racing to reset + call it corrupt each other's state.
/// Uses `tokio::sync::Mutex` so the guard is safely held across `.await`.
static HOTKEY_SERIAL: OnceLock<TokioMutex<()>> = OnceLock::new();

/// Reset HOTKEY_BUSY and call handle_hotkey under the serial lock so parallel
/// tests don't race on the process-wide reentrancy guard.
async fn call_handle_hotkey(engine: quill::engine::Engine) {
    let lock = HOTKEY_SERIAL.get_or_init(|| TokioMutex::new(()));
    let _guard = lock.lock().await;
    quill::engine::hotkey_flow::reset_busy_for_test();
    quill::engine::hotkey_flow::handle_hotkey(engine).await;
}

fn test_modes() -> HashMap<String, ModeConfig> {
    let mut m = HashMap::new();
    m.insert(
        "rewrite".into(),
        ModeConfig {
            label: "Rewrite".into(),
            icon: "✍".into(),
            prompt: "Rewrite the following text: {text}".into(),
        },
    );
    m.insert(
        "translate".into(),
        ModeConfig {
            label: "Translate".into(),
            icon: "🌐".into(),
            prompt: "Translate to {language}: {text}".into(),
        },
    );
    m
}

fn test_chains() -> HashMap<String, ChainConfig> {
    let mut c = HashMap::new();
    c.insert(
        "polish".into(),
        ChainConfig {
            label: "Polish".into(),
            icon: "✨".into(),
            steps: vec!["rewrite".into(), "translate".into()],
            description: "Rewrite then translate".into(),
        },
    );
    c
}

fn test_config() -> Config {
    let mut cfg = Config::default();
    cfg.history.enabled = false;
    cfg.tutor.enabled = false;
    cfg.tutor.auto_explain = false;
    cfg
}

struct Harness {
    engine: Engine,
    state: Arc<Mutex<AppState>>,
    rx: mpsc::UnboundedReceiver<UiEvent>,
    replace: Arc<FakeReplace>,
}

fn build_harness(
    capture: Arc<dyn TextCapture>,
    context: Arc<dyn ContextProbe>,
    provider: Arc<dyn Provider>,
) -> Harness {
    let (tx, rx) = mpsc::unbounded_channel::<UiEvent>();
    let state = Arc::new(Mutex::new(AppState::new()));
    let replace = Arc::new(FakeReplace::default());
    let replace_dyn: Arc<dyn TextReplace> = replace.clone();
    let engine = Engine::new(
        test_config(),
        test_modes(),
        test_chains(),
        state.clone(),
        tx,
        capture,
        replace_dyn,
        context,
        provider,
    );
    Harness {
        engine,
        state,
        rx,
        replace,
    }
}

async fn drain_events(rx: &mut mpsc::UnboundedReceiver<UiEvent>) -> Vec<UiEvent> {
    let mut out = Vec::new();
    // Small sleep to let spawn_blocking tasks complete, then yield.
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    tokio::task::yield_now().await;
    while let Ok(event) = rx.try_recv() {
        out.push(event);
    }
    out
}

#[tokio::test]
async fn hotkey_happy_path_emits_show_overlay_with_selected_text() {
    let capture: Arc<dyn TextCapture> = Arc::new(FakeCapture::with_text("Hello, world!"));
    let context: Arc<dyn ContextProbe> =
        Arc::new(FakeContext::with_app("notepad", "neutral", "general"));
    let provider: Arc<dyn Provider> = Arc::new(FakeProvider::with_chunks(Vec::<String>::new()));

    let mut h = build_harness(capture, context, provider);
    call_handle_hotkey(h.engine.clone()).await;

    let events = drain_events(&mut h.rx).await;
    assert_eq!(
        events.len(),
        1,
        "expected exactly one ShowOverlay, got {events:?}"
    );
    match &events[0] {
        UiEvent::ShowOverlay {
            text,
            context,
            suggestion,
            anchor_rect: _,
        } => {
            assert_eq!(text, "Hello, world!");
            assert_eq!(context.app, "notepad");
            assert!(
                suggestion.is_some(),
                "non-empty selection should carry a suggestion"
            );
        }
        other => panic!("expected ShowOverlay, got {other:?}"),
    }

    let s = h.state.lock().unwrap();
    assert_eq!(s.selected_text, "Hello, world!");
    assert_eq!(s.last_app_hint, "general");
}

#[tokio::test]
async fn hotkey_empty_selection_emits_show_overlay_without_suggestion() {
    let capture: Arc<dyn TextCapture> = Arc::new(FakeCapture::empty());
    let context: Arc<dyn ContextProbe> =
        Arc::new(FakeContext::with_app("explorer", "neutral", "general"));
    let provider: Arc<dyn Provider> = Arc::new(FakeProvider::with_chunks(Vec::<String>::new()));

    let mut h = build_harness(capture, context, provider);
    call_handle_hotkey(h.engine.clone()).await;

    let events = drain_events(&mut h.rx).await;
    assert_eq!(events.len(), 1);
    match &events[0] {
        UiEvent::ShowOverlay {
            text, suggestion, ..
        } => {
            assert!(text.is_empty());
            assert!(suggestion.is_none());
        }
        other => panic!("expected ShowOverlay, got {other:?}"),
    }
}

#[tokio::test]
async fn execute_mode_streams_and_emits_stream_done() {
    let capture: Arc<dyn TextCapture> = Arc::new(FakeCapture::with_text("Hello"));
    let context: Arc<dyn ContextProbe> = Arc::new(FakeContext::default());
    let provider: Arc<dyn Provider> =
        Arc::new(FakeProvider::with_chunks(vec!["Hola", " ", "mundo"]));

    let mut h = build_harness(capture, context, provider);

    // Simulate the hotkey path first so selected_text is populated.
    call_handle_hotkey(h.engine.clone()).await;
    // Drain the ShowOverlay.
    let _ = drain_events(&mut h.rx).await;

    h.engine
        .execute_mode("rewrite".into(), "auto".into(), None)
        .await;

    let events = drain_events(&mut h.rx).await;
    let kinds: Vec<_> = events
        .iter()
        .map(|e| match e {
            UiEvent::StreamStart { .. } => "start",
            UiEvent::StreamChunk { .. } => "chunk",
            UiEvent::StreamDone { .. } => "done",
            UiEvent::StreamError { .. } => "error",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds.first(), Some(&"start"));
    assert!(kinds.contains(&"chunk"));
    assert_eq!(kinds.last(), Some(&"done"));

    let s = h.state.lock().unwrap();
    assert!(s.is_done);
    assert!(!s.is_streaming);
    assert_eq!(s.last_result, "Hola mundo");
}

#[tokio::test]
async fn execute_mode_reports_provider_error() {
    let capture: Arc<dyn TextCapture> = Arc::new(FakeCapture::with_text("Hello"));
    let context: Arc<dyn ContextProbe> = Arc::new(FakeContext::default());
    let provider: Arc<dyn Provider> = Arc::new(FakeProvider::failing("network down"));

    let mut h = build_harness(capture, context, provider);
    call_handle_hotkey(h.engine.clone()).await;
    let _ = drain_events(&mut h.rx).await;

    h.engine
        .execute_mode("rewrite".into(), "auto".into(), None)
        .await;

    let events = drain_events(&mut h.rx).await;
    assert!(
        events.iter().any(
            |e| matches!(e, UiEvent::StreamError { message } if message.contains("network down"))
        ),
        "expected StreamError, got {events:?}"
    );
    assert!(!h.state.lock().unwrap().is_streaming);
}

#[tokio::test]
async fn execute_chain_emits_progress_per_step() {
    let capture: Arc<dyn TextCapture> = Arc::new(FakeCapture::with_text("Hello"));
    let context: Arc<dyn ContextProbe> = Arc::new(FakeContext::default());
    let provider: Arc<dyn Provider> = Arc::new(FakeProvider::with_chunks(vec!["ok"]));

    let mut h = build_harness(capture, context, provider);
    call_handle_hotkey(h.engine.clone()).await;
    let _ = drain_events(&mut h.rx).await;

    h.engine
        .execute_chain("polish".into(), "auto".into(), None)
        .await;

    let events = drain_events(&mut h.rx).await;
    let progress_events: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            UiEvent::ChainProgress { step, total, mode } => Some((*step, *total, mode.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        progress_events.len(),
        2,
        "two-step chain should emit 2 progress events"
    );
    assert_eq!(progress_events[0], (1, 2, "rewrite".to_string()));
    assert_eq!(progress_events[1], (2, 2, "translate".to_string()));

    assert!(events
        .iter()
        .any(|e| matches!(e, UiEvent::StreamDone { .. })));
}

#[tokio::test]
async fn compare_modes_emits_comparison_result() {
    let capture: Arc<dyn TextCapture> = Arc::new(FakeCapture::with_text("Hello"));
    let context: Arc<dyn ContextProbe> = Arc::new(FakeContext::default());
    let provider: Arc<dyn Provider> = Arc::new(FakeProvider::with_chunks(vec!["A-out"]));

    let mut h = build_harness(capture, context, provider);
    call_handle_hotkey(h.engine.clone()).await;
    let _ = drain_events(&mut h.rx).await;

    h.engine
        .handle_command(UiCommand::CompareModes {
            mode_a: "rewrite".into(),
            mode_b: "translate".into(),
            language: "auto".into(),
            extra: None,
        })
        .await;

    let events = drain_events(&mut h.rx).await;
    let cmp = events.iter().find_map(|e| match e {
        UiEvent::ComparisonResult {
            mode_a,
            result_a,
            mode_b,
            result_b,
        } => Some((
            mode_a.clone(),
            result_a.clone(),
            mode_b.clone(),
            result_b.clone(),
        )),
        _ => None,
    });
    let (mode_a, result_a, mode_b, result_b) = cmp.expect("expected ComparisonResult");
    assert_eq!(mode_a, "rewrite");
    assert_eq!(mode_b, "translate");
    assert_eq!(result_a, "A-out");
    assert_eq!(result_b, "A-out"); // same fake provider for both arms

    let s = h.state.lock().unwrap();
    assert_eq!(s.last_result, "A-out");
}

#[tokio::test]
async fn confirm_replace_pastes_last_result() {
    let capture: Arc<dyn TextCapture> = Arc::new(FakeCapture::with_text("Hello"));
    let context: Arc<dyn ContextProbe> = Arc::new(FakeContext::default());
    let provider: Arc<dyn Provider> = Arc::new(FakeProvider::with_chunks(vec!["Hola mundo"]));

    let mut h = build_harness(capture, context, provider);
    call_handle_hotkey(h.engine.clone()).await;
    let _ = drain_events(&mut h.rx).await;

    h.engine
        .execute_mode("rewrite".into(), "auto".into(), None)
        .await;
    let _ = drain_events(&mut h.rx).await;

    h.engine.handle_command(UiCommand::ConfirmReplace).await;

    assert_eq!(h.replace.last().as_deref(), Some("Hola mundo"));
}
