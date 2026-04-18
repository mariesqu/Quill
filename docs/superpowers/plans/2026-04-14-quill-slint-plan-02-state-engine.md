# Quill Slint Rewrite — Plan 2: State + Engine Refactor

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Tauri-era `noop_emit` shims in `src/engine.rs` with a channel-based architecture. Centralize session/view state in `Arc<Mutex<AppState>>`, split the 724-line engine into focused modules, inject platform traits into the engine, and cover the result with Tier 2 integration tests using in-memory fakes.

**Architecture:**
- New `src/state/` module: `AppState` (source of truth), `UiEvent` (engine → UI), `UiCommand` (UI → engine)
- New `src/engine/` directory replacing `src/engine.rs`: `mod.rs` (Engine struct + command dispatch), `hotkey_flow.rs`, `streaming.rs`, `compare.rs`, `tutor_flow.rs`
- Engine holds `Arc<dyn TextCapture>`, `Arc<dyn TextReplace>`, `Arc<dyn ContextProbe>`, `Arc<dyn Provider>`, `Arc<Mutex<AppState>>`, `mpsc::UnboundedSender<UiEvent>`
- Integration tests in `tests/engine_integration.rs` wire fakes from `tests/common/fakes/`

**Tech Stack:** Rust 2021, tokio (mpsc + oneshot), async-trait, anyhow, serde, rusqlite (existing), existing provider trait.

**Preconditions:**
- On branch `claude/slint-rewrite` at tag `plan-01-foundation-complete`
- `cargo test` passes (54 unit tests)
- `cargo build` passes
- `src/platform/traits.rs` already defines `TextCapture`, `TextReplace`, `ContextProbe`
- `src/engine.rs` still emits via `noop_emit` stubs

**End state:**
- `src/engine.rs` no longer exists; replaced by `src/engine/` directory
- Every `noop_emit` call site is gone; all engine → UI signals flow through `mpsc::UnboundedSender<UiEvent>`
- `AppState` is the source of truth; engine mutates it under its lock
- 10+ new Tier 2 integration tests, all green
- Zero regressions in existing unit tests
- `cargo build`, `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check` all clean
- Tagged `plan-02-engine-refactor-complete`

---

## Design Reference

### UiEvent variants (engine → UI)

```rust
pub enum UiEvent {
    ShowOverlay { text: String, context: AppContext, suggestion: Option<Suggestion> },
    Dismiss,
    StreamStart { mode: String, language: String },
    StreamChunk { text: String },
    StreamDone  { full_text: String, entry_id: Option<i64> },
    StreamError { message: String },
    ChainProgress { step: usize, total: usize, mode: String },
    ComparisonResult { mode_a: String, result_a: String, mode_b: String, result_b: String },
    TutorExplanation { entry_id: i64, text: String },
    TutorLesson { period: String, text: String },
    Pronunciation { text: String },
    Error { message: String },
}
```

### UiCommand variants (UI → engine)

```rust
pub enum UiCommand {
    ExecuteMode  { mode: String, language: String, extra: Option<String> },
    ExecuteChain { chain_id: String, language: String, extra: Option<String> },
    CompareModes { mode_a: String, mode_b: String, language: String, extra: Option<String> },
    GetPronunciation { text: String, language: String },
    RequestTutorExplain { entry_id: i64 },
    GenerateLesson { period: String },
    ConfirmReplace,
    SetResult { text: String },
    Undo,
    CancelStream,
    Dismiss,
}
```

(Subset of the spec's full enum — the rest lands in Plans 3–5 as features come online.)

### AppState core fields

```rust
pub struct AppState {
    pub selected_text: String,
    pub last_result: String,
    pub last_mode: String,
    pub last_language: String,
    pub last_app_hint: String,
    pub last_entry_id: Option<i64>,
    pub undo_stack: Vec<String>,

    pub view_mode: ViewMode,       // Compact | Expanded
    pub current_tab: TabId,        // Write | History | Tutor | Compare | Settings
    pub is_visible: bool,
    pub is_streaming: bool,
    pub is_done: bool,
    pub stream_buffer: String,
    pub chain_progress: Option<ChainProgress>,

    pub error: Option<String>,
}
```

Only fields needed for Plan 2 live in state now. History/comparison/tutor-lesson collections land in Plan 4 (Expanded view).

### Engine shape

```rust
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

    pub(crate) capture:  Arc<dyn TextCapture>,
    pub(crate) replace:  Arc<dyn TextReplace>,
    pub(crate) context:  Arc<dyn ContextProbe>,
    pub(crate) provider: Arc<dyn Provider>,

    pub(crate) cancel_tx: Mutex<Option<oneshot::Sender<()>>>,
}
```

`Engine: Clone` via `Arc<EngineInner>` so tokio-spawned flows can move a clone into their task.

---

## Phase 1 — State Module (Day 1)

### Task 1: Create `src/state/events.rs` with UiEvent + UiCommand

**Files:**
- Create: `src/state/events.rs`

- [ ] **Step 1: Write the file**

```rust
//! Cross-thread signal types between the engine and the UI layer.
//!
//! `UiEvent` flows engine → UI via `mpsc::UnboundedSender<UiEvent>`.
//! `UiCommand` flows UI → engine via `mpsc::UnboundedSender<UiCommand>`.
//! These two enums are the ONLY communication surface between the two sides.

use crate::platform::context::AppContext;

#[derive(Debug, Clone)]
pub struct Suggestion {
    pub mode_id: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub enum UiEvent {
    ShowOverlay {
        text: String,
        context: AppContext,
        suggestion: Option<Suggestion>,
    },
    Dismiss,

    StreamStart { mode: String, language: String },
    StreamChunk { text: String },
    StreamDone  { full_text: String, entry_id: Option<i64> },
    StreamError { message: String },

    ChainProgress { step: usize, total: usize, mode: String },

    ComparisonResult {
        mode_a: String,
        result_a: String,
        mode_b: String,
        result_b: String,
    },

    TutorExplanation { entry_id: i64, text: String },
    TutorLesson      { period: String, text: String },
    Pronunciation    { text: String },

    Error { message: String },
}

#[derive(Debug, Clone)]
pub enum UiCommand {
    ExecuteMode      { mode: String, language: String, extra: Option<String> },
    ExecuteChain     { chain_id: String, language: String, extra: Option<String> },
    CompareModes     { mode_a: String, mode_b: String, language: String, extra: Option<String> },
    GetPronunciation { text: String, language: String },
    RequestTutorExplain { entry_id: i64 },
    GenerateLesson   { period: String },
    ConfirmReplace,
    SetResult { text: String },
    Undo,
    CancelStream,
    Dismiss,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_event_is_clone_debug() {
        let e = UiEvent::StreamChunk { text: "hi".into() };
        let _ = format!("{e:?}");
        let _ = e.clone();
    }

    #[test]
    fn ui_command_is_clone_debug() {
        let c = UiCommand::ExecuteMode {
            mode: "rewrite".into(),
            language: "auto".into(),
            extra: None,
        };
        let _ = format!("{c:?}");
        let _ = c.clone();
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add src/state/events.rs
git commit -m "feat(state): add UiEvent and UiCommand enums"
```

(File is not yet wired into the crate — that happens in Task 3. Build will not see it yet.)

### Task 2: Create `src/state/app_state.rs` with AppState + enums

**Files:**
- Create: `src/state/app_state.rs`

- [ ] **Step 1: Write the failing test**

Tests are inline (`#[cfg(test)] mod tests`) — added in Step 3 below.

- [ ] **Step 2: Write the file**

```rust
//! Single source of truth for runtime app state.
//!
//! All state lives here under `Arc<Mutex<AppState>>`. The engine mutates it
//! under the lock; the UI reads it and projects it onto Slint properties.
//! If the two ever drift, `AppState` wins on the next event.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Compact,
    Expanded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabId {
    #[default]
    Write,
    History,
    Tutor,
    Compare,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub kind: ToastKind,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ChainProgress {
    pub step: usize,
    pub total: usize,
    pub mode: String,
}

#[derive(Debug, Clone, Default)]
pub struct AppState {
    // Session (reset on every hotkey trigger)
    pub selected_text: String,
    pub last_result: String,
    pub last_mode: String,
    pub last_language: String,
    pub last_app_hint: String,
    pub last_entry_id: Option<i64>,
    pub undo_stack: Vec<String>,

    // View
    pub view_mode: ViewMode,
    pub current_tab: TabId,
    pub is_visible: bool,
    pub is_streaming: bool,
    pub is_done: bool,
    pub stream_buffer: String,
    pub chain_progress: Option<ChainProgress>,

    // UX
    pub error: Option<String>,
    pub toast: Option<Toast>,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset the per-session fields at the start of a new hotkey capture.
    /// Preserves view_mode, current_tab, visibility, and any pending toast.
    pub fn reset_session(&mut self) {
        self.selected_text.clear();
        self.last_result.clear();
        self.last_mode.clear();
        self.last_language.clear();
        self.last_app_hint.clear();
        self.last_entry_id = None;
        self.undo_stack.clear();
        self.is_streaming = false;
        self.is_done = false;
        self.stream_buffer.clear();
        self.chain_progress = None;
        self.error = None;
    }

    pub fn begin_stream(&mut self, mode: &str, language: &str) {
        self.last_mode = mode.to_string();
        self.last_language = language.to_string();
        self.is_streaming = true;
        self.is_done = false;
        self.stream_buffer.clear();
        self.error = None;
    }

    pub fn push_chunk(&mut self, chunk: &str) {
        self.stream_buffer.push_str(chunk);
    }

    pub fn finish_stream(&mut self, full_text: &str, entry_id: Option<i64>) {
        self.last_result = full_text.to_string();
        self.last_entry_id = entry_id;
        self.is_streaming = false;
        self.is_done = true;
    }

    pub fn fail_stream(&mut self, message: &str) {
        self.is_streaming = false;
        self.is_done = false;
        self.error = Some(message.to_string());
    }

    pub fn toggle_view(&mut self) {
        self.view_mode = match self.view_mode {
            ViewMode::Compact => ViewMode::Expanded,
            ViewMode::Expanded => ViewMode::Compact,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_sensible() {
        let s = AppState::new();
        assert_eq!(s.view_mode, ViewMode::Compact);
        assert_eq!(s.current_tab, TabId::Write);
        assert!(!s.is_visible);
        assert!(!s.is_streaming);
        assert!(!s.is_done);
        assert!(s.stream_buffer.is_empty());
        assert!(s.error.is_none());
    }

    #[test]
    fn reset_session_clears_per_capture_fields_only() {
        let mut s = AppState::new();
        s.view_mode = ViewMode::Expanded;
        s.is_visible = true;
        s.selected_text = "hello".into();
        s.stream_buffer = "partial".into();
        s.is_streaming = true;
        s.last_entry_id = Some(42);

        s.reset_session();

        // Per-session fields cleared
        assert!(s.selected_text.is_empty());
        assert!(s.stream_buffer.is_empty());
        assert!(!s.is_streaming);
        assert_eq!(s.last_entry_id, None);
        // View state preserved
        assert_eq!(s.view_mode, ViewMode::Expanded);
        assert!(s.is_visible);
    }

    #[test]
    fn begin_stream_sets_flags_and_mode() {
        let mut s = AppState::new();
        s.stream_buffer = "leftover".into();
        s.error = Some("old".into());

        s.begin_stream("rewrite", "en");

        assert_eq!(s.last_mode, "rewrite");
        assert_eq!(s.last_language, "en");
        assert!(s.is_streaming);
        assert!(!s.is_done);
        assert!(s.stream_buffer.is_empty());
        assert!(s.error.is_none());
    }

    #[test]
    fn push_chunk_appends() {
        let mut s = AppState::new();
        s.push_chunk("Hel");
        s.push_chunk("lo");
        assert_eq!(s.stream_buffer, "Hello");
    }

    #[test]
    fn finish_stream_latches_result() {
        let mut s = AppState::new();
        s.begin_stream("rewrite", "en");
        s.push_chunk("Hello world");
        s.finish_stream("Hello world", Some(7));
        assert!(!s.is_streaming);
        assert!(s.is_done);
        assert_eq!(s.last_result, "Hello world");
        assert_eq!(s.last_entry_id, Some(7));
    }

    #[test]
    fn fail_stream_sets_error() {
        let mut s = AppState::new();
        s.begin_stream("rewrite", "en");
        s.fail_stream("network down");
        assert!(!s.is_streaming);
        assert!(!s.is_done);
        assert_eq!(s.error.as_deref(), Some("network down"));
    }

    #[test]
    fn toggle_view_switches() {
        let mut s = AppState::new();
        assert_eq!(s.view_mode, ViewMode::Compact);
        s.toggle_view();
        assert_eq!(s.view_mode, ViewMode::Expanded);
        s.toggle_view();
        assert_eq!(s.view_mode, ViewMode::Compact);
    }
}
```

- [ ] **Step 3: Commit**

```bash
git add src/state/app_state.rs
git commit -m "feat(state): add AppState with session/view helpers"
```

### Task 3: Wire `state` module into the crate

**Files:**
- Create: `src/state/mod.rs`
- Modify: `src/lib.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create `src/state/mod.rs`**

```rust
pub mod app_state;
pub mod events;

pub use app_state::{AppState, ChainProgress, TabId, Toast, ToastKind, ViewMode};
pub use events::{Suggestion, UiCommand, UiEvent};
```

- [ ] **Step 2: Add `pub mod state;` to `src/lib.rs`**

Final file:
```rust
pub mod core;
pub mod engine;
pub mod platform;
pub mod providers;
pub mod state;
```

- [ ] **Step 3: Add `mod state;` to `src/main.rs`**

Insert after the existing `mod providers;` line:
```rust
mod core;
mod engine;
mod platform;
mod providers;
mod state;
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib state`
Expected: all 8 tests in `state::app_state::tests` and `state::events::tests` pass.

- [ ] **Step 5: Build the whole crate**

Run: `cargo build`
Expected: clean build (no new warnings attributable to this task).

- [ ] **Step 6: Commit**

```bash
git add src/state/mod.rs src/lib.rs src/main.rs
git commit -m "feat(state): wire state module into crate"
```

---

## Phase 2 — Engine Skeleton & Injection (Day 1-2)

### Task 4: Switch `src/engine.rs` → `src/engine/mod.rs` (no behavior change)

Rust resolves a module ambiguously named both as `engine.rs` file and `engine/` directory, so this step is sequenced carefully.

**Files:**
- Delete: `src/engine.rs`
- Create: `src/engine/mod.rs` (byte-for-byte identical contents)

- [ ] **Step 1: Move the file**

```bash
mkdir -p src/engine
git mv src/engine.rs src/engine/mod.rs
```

- [ ] **Step 2: Build to confirm zero behavior change**

Run: `cargo build`
Expected: clean build (no warnings changed).

- [ ] **Step 3: Run tests**

Run: `cargo test --lib`
Expected: same pass count as before the move.

- [ ] **Step 4: Commit**

```bash
git commit -m "refactor(engine): move engine.rs to engine/mod.rs"
```

### Task 5: Change `build_provider` to return `Arc<dyn Provider>`

**Files:**
- Modify: `src/providers/mod.rs`

- [ ] **Step 1: Change the function signature**

In `src/providers/mod.rs`, replace:
```rust
pub fn build_provider(cfg: &Config) -> Box<dyn Provider> {
    match cfg.provider.as_str() {
        "openrouter" => Box::new(openrouter::OpenRouterProvider::new(cfg)),
        "ollama" => Box::new(ollama::OllamaProvider::new(cfg)),
        "openai" => Box::new(openai::OpenAIProvider::new(cfg)),
        _ => Box::new(generic::GenericProvider::new(cfg)),
    }
}
```
with:
```rust
use std::sync::Arc;

pub fn build_provider(cfg: &Config) -> Arc<dyn Provider> {
    match cfg.provider.as_str() {
        "openrouter" => Arc::new(openrouter::OpenRouterProvider::new(cfg)),
        "ollama" => Arc::new(ollama::OllamaProvider::new(cfg)),
        "openai" => Arc::new(openai::OpenAIProvider::new(cfg)),
        _ => Arc::new(generic::GenericProvider::new(cfg)),
    }
}
```

- [ ] **Step 2: Fix the one current caller in `engine/mod.rs`**

`engine/mod.rs` currently has two sites that do `let provider = { ... build_provider(&e.config) };` then `provider.stream_completion(...)`. Both still compile because `Arc<dyn Provider>` auto-derefs for method calls. No code changes needed at the call sites for this step — just confirm they still compile.

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add src/providers/mod.rs
git commit -m "refactor(providers): return Arc<dyn Provider> from build_provider"
```

### Task 6: Rewrite `Engine` struct with trait injection and state/events

This task REPLACES the public shape of `Engine` but leaves every function body alone — they still reference `engine.lock().unwrap()` as today. We migrate each function body in Phase 3.

**Files:**
- Modify: `src/engine/mod.rs`

- [ ] **Step 1: Replace the top of `src/engine/mod.rs`**

Locate the block starting at `// ── Engine state ──` and ending at `pub type SharedEngine = Arc<Mutex<Engine>>;`. Replace it with:

```rust
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

    pub(crate) capture:  Arc<dyn TextCapture>,
    pub(crate) replace:  Arc<dyn TextReplace>,
    pub(crate) context:  Arc<dyn ContextProbe>,
    pub(crate) provider: Arc<dyn Provider>,

    pub(crate) cancel_tx: Mutex<Option<oneshot::Sender<()>>>,
    pub(crate) clipboard_monitor_running: Arc<AtomicBool>,
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
        let clipboard_enabled = config.clipboard_monitor.enabled;
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
                clipboard_monitor_running: Arc::new(AtomicBool::new(clipboard_enabled)),
            }),
        }
    }

    pub fn config(&self) -> &Config { &self.inner.config }
    pub fn modes(&self) -> &HashMap<String, ModeConfig> { &self.inner.modes }
    pub fn chains(&self) -> &HashMap<String, ChainConfig> { &self.inner.chains }

    pub fn history_enabled(&self) -> bool {
        self.inner.config.history.enabled
    }

    pub fn tutor_enabled(&self) -> bool {
        self.inner.config.tutor.enabled && self.history_enabled()
    }

    pub fn cancel_stream(&self) {
        if let Some(tx) = self.inner.cancel_tx.lock().unwrap().take() {
            let _ = tx.send(());
        }
    }

    /// Send a `UiEvent`. Drops silently if the receiver has been closed
    /// (only happens during shutdown).
    pub(crate) fn emit(&self, event: UiEvent) {
        let _ = self.inner.events.send(event);
    }

    pub(crate) fn state(&self) -> &Arc<Mutex<AppState>> { &self.inner.state }
    pub(crate) fn capture(&self) -> &Arc<dyn TextCapture> { &self.inner.capture }
    pub(crate) fn replace(&self) -> &Arc<dyn TextReplace> { &self.inner.replace }
    pub(crate) fn context(&self) -> &Arc<dyn ContextProbe> { &self.inner.context }
    pub(crate) fn provider(&self) -> &Arc<dyn Provider> { &self.inner.provider }

    pub(crate) fn set_cancel_tx(&self, tx: oneshot::Sender<()>) {
        *self.inner.cancel_tx.lock().unwrap() = Some(tx);
    }
}
```

- [ ] **Step 2: Delete the old `Engine` struct, `impl Engine`, and `SharedEngine` type alias**

Delete lines that were replaced (old `pub struct Engine { ... }`, old `impl Engine { pub fn new(...) ... cancel_stream / history_enabled / tutor_enabled }`, and `pub type SharedEngine = Arc<Mutex<Engine>>;`).

- [ ] **Step 3: Temporarily gate the rest of engine/mod.rs behind `#[cfg(any())]`**

Immediately before the `// ── Hotkey handler ──` comment, insert:

```rust
// Phase 2 checkpoint: the handlers below still reference the OLD engine API
// and `noop_emit`. They are gated off so the crate compiles while Phase 3
// rewrites each handler one at a time. The gate comes off at the end of Task 12.
#[cfg(any())]
mod legacy_handlers {
    use super::*;
```

And at the very end of the file, add `}` to close `mod legacy_handlers`.

- [ ] **Step 4: Build**

Run: `cargo build`
Expected: warnings are acceptable (unused fields on `EngineInner` — capture/replace/context/provider/state/events — those fire in Phase 3). No errors.

- [ ] **Step 5: Run tests**

Run: `cargo test --lib`
Expected: existing state tests pass; existing core/providers tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/engine/mod.rs
git commit -m "refactor(engine): inject traits/state/events, gate legacy handlers"
```

---

## Phase 3 — Feature Flows (Day 2)

Each task in this phase creates one new submodule under `src/engine/`, wires it from `engine/mod.rs`, and replaces the gated-off legacy handler with the new implementation. Every new flow emits `UiEvent`s via `engine.emit(...)` and mutates `engine.state()` under the lock (never across an `.await`).

### Task 7: Extract `hotkey_flow.rs`

**Files:**
- Create: `src/engine/hotkey_flow.rs`
- Modify: `src/engine/mod.rs`

- [ ] **Step 1: Create `src/engine/hotkey_flow.rs`**

```rust
//! Hotkey entry point: capture selected text + active app context,
//! then emit `UiEvent::ShowOverlay` for the UI to render.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::core::prompt::suggest_mode;
use crate::state::{Suggestion, UiEvent};

use super::Engine;

/// Reentrancy guard for `handle_hotkey`. See the Tauri-era commit history
/// for the data-loss scenario this prevents: repeated hotkey presses each
/// injected a `Ctrl` release via enigo, clearing the real modifier state
/// mid-capture and letting the next physical keystroke land without Ctrl.
static HOTKEY_BUSY: AtomicBool = AtomicBool::new(false);

struct BusyGuard;
impl Drop for BusyGuard {
    fn drop(&mut self) {
        HOTKEY_BUSY.store(false, Ordering::Release);
    }
}

pub async fn handle_hotkey(engine: Engine) {
    // Reject reentrant invocations silently.
    if HOTKEY_BUSY
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    let _busy_guard = BusyGuard;

    // Run capture (already async) and active-app context probe concurrently.
    // ContextProbe::active_context is synchronous, so wrap it in spawn_blocking
    // to keep the tokio worker pool free during blocking FFI.
    let capture_fut = engine.capture().capture();
    let ctx_probe = engine.context().clone();
    let context_fut = tokio::task::spawn_blocking(move || ctx_probe.active_context());
    let (capture, ctx_res) = tokio::join!(capture_fut, context_fut);
    let context = ctx_res.unwrap_or_default();
    let text = capture.text;

    // Reset per-session state BEFORE mutating with new data.
    {
        let mut s = engine.state().lock().unwrap();
        s.reset_session();
        s.selected_text = text.clone();
        s.last_language = "auto".into();
        s.last_app_hint = context.hint.clone();
        s.is_visible = true;
    }

    let suggestion = if text.trim().is_empty() {
        None
    } else {
        let (mode_id, reason) = suggest_mode(&text, &context);
        Some(Suggestion { mode_id, reason })
    };

    engine.emit(UiEvent::ShowOverlay {
        text,
        context,
        suggestion,
    });
}
```

- [ ] **Step 2: Wire the module from `src/engine/mod.rs`**

At the top of `engine/mod.rs`, below the `use` block but above the `// ── Engine state ──` header, add:

```rust
pub mod hotkey_flow;
```

- [ ] **Step 3: Build + run the lib tests**

Run: `cargo build && cargo test --lib`
Expected: clean build, existing tests still green.

- [ ] **Step 4: Commit**

```bash
git add src/engine/hotkey_flow.rs src/engine/mod.rs
git commit -m "feat(engine): hotkey_flow emits UiEvent::ShowOverlay"
```

### Task 8: Extract `streaming.rs` — single-stream helper + finalize

**Files:**
- Create: `src/engine/streaming.rs`
- Modify: `src/engine/mod.rs`

- [ ] **Step 1: Create `src/engine/streaming.rs`**

```rust
//! Streaming helpers: pump a provider stream, filter <think> blocks,
//! emit UiEvent::StreamChunk/StreamDone/StreamError, mirror the buffer
//! into AppState, and handle cancellation.

use futures_util::StreamExt;
use tokio::sync::oneshot;

use crate::core::history;
use crate::core::think_filter::ThinkFilter;
use crate::state::UiEvent;

use super::Engine;

/// Run one provider stream to completion. Returns the full (filtered) text
/// or `None` if the stream was cancelled.
pub async fn run_single_stream(
    engine: Engine,
    system: String,
    user: String,
) -> Option<String> {
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
        match history::save_entry(
            original, output, mode, language, &app_hint, &persona_tone, max_entries,
        ) {
            Ok(id) => Some(id),
            Err(e) => {
                tracing::warn!("history save failed: {e}");
                None
            }
        }
    } else {
        None
    };

    engine.state().lock().unwrap().finish_stream(output, entry_id);

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
/// cancel_tx touch. The caller gets the final text once both arms complete.
pub async fn run_silent_stream(
    engine: Engine,
    system: String,
    user: String,
) -> Option<String> {
    let stream = engine.provider().stream_completion(&system, &user).await.ok()?;
    let mut full = String::new();
    let mut filter = ThinkFilter::new();
    tokio::pin!(stream);
    while let Some(token) = stream.next().await {
        full.push_str(&filter.push(&token));
    }
    full.push_str(&filter.flush());
    Some(full)
}
```

- [ ] **Step 2: Wire `pub mod streaming;` in `engine/mod.rs`**

Add alongside `pub mod hotkey_flow;`:
```rust
pub mod streaming;
```

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: error — `super::tutor_flow::explain_entry` not found. That is expected; tutor_flow lands in Task 12. Temporarily replace the `tokio::spawn` block with:

```rust
if tutor_en && auto_explain {
    if let Some(_eid) = entry_id {
        // TODO(task-12): re-enable auto-explain once tutor_flow lands
        let _ = (original, output);
    }
}
```

Re-run `cargo build`. Expected: clean build (warnings about unused helpers are acceptable).

- [ ] **Step 4: Commit**

```bash
git add src/engine/streaming.rs src/engine/mod.rs
git commit -m "feat(engine): streaming helpers emit Stream* events"
```

### Task 9: Add `execute_mode` in `engine/mod.rs`

**Files:**
- Modify: `src/engine/mod.rs`

- [ ] **Step 1: Add `execute_mode` as a method on `Engine`**

Inside `impl Engine { ... }`, append:

```rust
pub async fn execute_mode(
    &self,
    mode: String,
    language: String,
    extra_instruction: Option<String>,
) {
    self.cancel_stream();

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
    if let Some(full_text) =
        streaming::run_single_stream(engine.clone(), system, user).await
    {
        streaming::finalize_result(
            engine, &original, &full_text, &mode, &language, history_en, tutor_en,
        )
        .await;
    }
}
```

- [ ] **Step 2: Build**

Run: `cargo build`
Expected: clean (still gated legacy handlers module). Fix any import errors by adding missing `use` statements at the top of `engine/mod.rs` — most likely `use crate::state::UiEvent;` is already there from Task 6 but double-check.

- [ ] **Step 3: Commit**

```bash
git add src/engine/mod.rs
git commit -m "feat(engine): execute_mode pumps StreamStart + run_single_stream"
```

### Task 10: Add `execute_chain` in `engine/mod.rs`

**Files:**
- Modify: `src/engine/mod.rs`

- [ ] **Step 1: Append `execute_chain` to `impl Engine`**

```rust
pub async fn execute_chain(
    &self,
    chain_id: String,
    language: String,
    extra_instruction: Option<String>,
) {
    self.cancel_stream();

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
                if idx == 0 { extra_instruction.as_deref() } else { None },
            );
            match built {
                Ok(p) => p,
                Err(err) => {
                    drop(s);
                    self.state().lock().unwrap().fail_stream(&err);
                    self.emit(UiEvent::StreamError { message: err });
                    return;
                }
            }
        };

        match streaming::run_single_stream(self.clone(), system, user).await {
            Some(text) => current_text = text,
            None => return, // cancelled or errored
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
```

- [ ] **Step 2: Build**

Run: `cargo build`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add src/engine/mod.rs
git commit -m "feat(engine): execute_chain emits ChainProgress per step"
```

### Task 11: Extract `compare.rs`

**Files:**
- Create: `src/engine/compare.rs`
- Modify: `src/engine/mod.rs`

- [ ] **Step 1: Create `src/engine/compare.rs`**

```rust
//! Side-by-side comparison of two modes. Emits a single
//! `UiEvent::ComparisonResult` once BOTH arms complete.

use crate::state::UiEvent;

use super::{streaming, Engine};

pub async fn compare_modes(
    engine: Engine,
    mode_a: String,
    mode_b: String,
    language: String,
    extra_instruction: Option<String>,
) {
    // Cancel any in-flight single stream so chunks don't bleed into the UI.
    engine.cancel_stream();

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
        let msg = pa.err().unwrap();
        engine.emit(UiEvent::Error { message: msg });
        return;
    }

    let result_a = match pa {
        Ok((sys, usr)) => streaming::run_silent_stream(engine.clone(), sys, usr).await,
        Err(err) => {
            engine.emit(UiEvent::Error {
                message: format!("Mode '{mode_a}' failed: {err}"),
            });
            None
        }
    };
    let result_b = match pb {
        Ok((sys, usr)) => streaming::run_silent_stream(engine.clone(), sys, usr).await,
        Err(err) => {
            engine.emit(UiEvent::Error {
                message: format!("Mode '{mode_b}' failed: {err}"),
            });
            None
        }
    };

    let result_a_text = result_a.unwrap_or_default();
    let result_b_text = result_b.unwrap_or_default();

    // Pre-seed last_result with mode_a's output so Replace works even if the
    // user never explicitly picks one via SetResult.
    {
        let mut s = engine.state().lock().unwrap();
        if !result_a_text.is_empty() {
            s.last_result = result_a_text.clone();
        }
    }

    engine.emit(UiEvent::ComparisonResult {
        mode_a,
        result_a: result_a_text,
        mode_b,
        result_b: result_b_text,
    });
}
```

- [ ] **Step 2: Wire in `engine/mod.rs`**

Add near the other `pub mod` lines:
```rust
pub mod compare;
```

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add src/engine/compare.rs src/engine/mod.rs
git commit -m "feat(engine): compare_modes emits ComparisonResult"
```

### Task 12: Extract `tutor_flow.rs`

**Files:**
- Create: `src/engine/tutor_flow.rs`
- Modify: `src/engine/streaming.rs` (re-enable auto-explain spawn)
- Modify: `src/engine/mod.rs`

- [ ] **Step 1: Create `src/engine/tutor_flow.rs`**

```rust
//! Tutor flows: explain a single history entry, generate a daily/weekly
//! lesson from history stats, and one-shot pronunciation guides.

use crate::core::{history, tutor};
use crate::state::UiEvent;

use super::{streaming, Engine};

pub async fn explain_entry(
    engine: Engine,
    entry_id: i64,
    original: String,
    output: String,
    mode: String,
    language: String,
) {
    let system = tutor::EXPLAIN_SYSTEM.to_string();
    let user = tutor::build_explain_prompt(&original, &output, &mode, &language);

    if let Some(explanation) =
        streaming::run_silent_stream(engine.clone(), system, user).await
    {
        if entry_id > 0 {
            let _ = history::save_tutor_explanation(entry_id, &explanation);
        }
        engine.emit(UiEvent::TutorExplanation {
            entry_id,
            text: explanation,
        });
    }
}

pub async fn generate_lesson(engine: Engine, period: String) {
    let days = if period == "daily" { 1 } else { 7 };
    let stats = match history::get_stats(days) {
        Ok(s) => s,
        Err(e) => {
            engine.emit(UiEvent::Error {
                message: format!("History error: {e}"),
            });
            return;
        }
    };

    let system = tutor::LESSON_SYSTEM.to_string();
    let user = tutor::build_lesson_prompt(&stats, &period);

    if let Some(lesson) =
        streaming::run_silent_stream(engine.clone(), system, user).await
    {
        let lang = engine.config().tutor.lesson_language.clone();
        let _ = history::save_lesson(&period, &lesson, &lang);
        engine.emit(UiEvent::TutorLesson {
            period,
            text: lesson,
        });
    }
}

pub async fn get_pronunciation(engine: Engine, text: String, language: String) {
    let system = "You are a linguistics expert. Provide a concise pronunciation guide.";
    let user = format!(
        "Provide a brief pronunciation guide for this {language} text. \
         Use IPA notation and simple phonetic spelling. Keep it under 100 words.\n\n{text}"
    );
    if let Some(result) =
        streaming::run_silent_stream(engine.clone(), system.to_string(), user).await
    {
        engine.emit(UiEvent::Pronunciation { text: result });
    }
}
```

- [ ] **Step 2: Wire `pub mod tutor_flow;` in `engine/mod.rs`**

Add alongside the other `pub mod` lines.

- [ ] **Step 3: Re-enable auto-explain in `streaming::finalize_result`**

Undo the Task 8 workaround. Replace the stubbed `if tutor_en && auto_explain { ... }` block with:

```rust
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
```

- [ ] **Step 4: Build**

Run: `cargo build`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src/engine/tutor_flow.rs src/engine/streaming.rs src/engine/mod.rs
git commit -m "feat(engine): tutor_flow + auto-explain restored"
```

---

## Phase 4 — Command Dispatcher & Legacy Cleanup (Day 2)

### Task 13: Add `Engine::handle_command` dispatcher

**Files:**
- Modify: `src/engine/mod.rs`

- [ ] **Step 1: Append the dispatcher method to `impl Engine`**

```rust
pub async fn handle_command(&self, cmd: crate::state::UiCommand) {
    use crate::state::UiCommand::*;
    match cmd {
        ExecuteMode { mode, language, extra } => {
            self.execute_mode(mode, language, extra).await;
        }
        ExecuteChain { chain_id, language, extra } => {
            self.execute_chain(chain_id, language, extra).await;
        }
        CompareModes { mode_a, mode_b, language, extra } => {
            compare::compare_modes(self.clone(), mode_a, mode_b, language, extra).await;
        }
        GetPronunciation { text, language } => {
            tutor_flow::get_pronunciation(self.clone(), text, language).await;
        }
        RequestTutorExplain { entry_id } => {
            // Look up the entry from history; if found, kick off explain.
            if let Ok(entry) = crate::core::history::get_entry(entry_id) {
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
            let text = self.state().lock().unwrap().last_result.clone();
            if !text.is_empty() {
                if let Err(e) = self.replace().paste(&text).await {
                    self.emit(crate::state::UiEvent::Error {
                        message: format!("Paste failed: {e}"),
                    });
                } else {
                    // Push to undo stack so Ctrl+Z can restore the original.
                    let mut s = self.state().lock().unwrap();
                    let original = s.selected_text.clone();
                    s.undo_stack.push(original);
                }
            }
        }
        SetResult { text } => {
            self.state().lock().unwrap().last_result = text;
        }
        Undo => {
            let prev = self.state().lock().unwrap().undo_stack.pop();
            if let Some(text) = prev {
                let _ = self.replace().paste(&text).await;
            }
        }
        CancelStream => self.cancel_stream(),
        Dismiss => {
            self.cancel_stream();
            {
                let mut s = self.state().lock().unwrap();
                s.is_visible = false;
                s.is_streaming = false;
            }
            self.emit(crate::state::UiEvent::Dismiss);
        }
    }
}
```

- [ ] **Step 2: Verify `history::get_entry` exists**

Run: `grep -n "pub fn get_entry" src/core/history.rs`
If it does NOT exist, add this helper to `src/core/history.rs` (after `save_entry`):

```rust
pub fn get_entry(id: i64) -> Result<HistoryEntry> {
    let conn = open()?;
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, app_hint, mode, language, persona_tone,
                original_text, output_text, word_count_before, word_count_after,
                tutor_explanation, favorited
         FROM history WHERE id = ?1",
    )?;
    let entry = stmt.query_row(params![id], |row| {
        Ok(HistoryEntry {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            app_hint: row.get(2)?,
            mode: row.get(3)?,
            language: row.get(4)?,
            persona_tone: row.get(5)?,
            original_text: row.get(6)?,
            output_text: row.get(7)?,
            word_count_before: row.get(8)?,
            word_count_after: row.get(9)?,
            tutor_explanation: row.get(10)?,
            favorited: row.get::<_, i64>(11)? != 0,
        })
    })?;
    Ok(entry)
}
```

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add src/engine/mod.rs src/core/history.rs
git commit -m "feat(engine): handle_command dispatcher"
```

### Task 14: Delete gated legacy handlers + `noop_emit` helper

**Files:**
- Modify: `src/engine/mod.rs`

- [ ] **Step 1: Delete the gated legacy module**

Remove the `// Phase 2 checkpoint: ... #[cfg(any())] mod legacy_handlers { use super::*;` opening and the matching `}` at EOF, PLUS every function body between them (the old `handle_hotkey`, `show_mini_overlay`, `execute_mode`, `execute_chain`, `run_single_stream`, `stream_and_collect`, `finalize_result`, `explain_entry`, `generate_lesson`, `run_silent_stream`, `compare_modes`, `get_pronunciation`). These have all been replaced by the new modules/methods.

- [ ] **Step 2: Delete `fn noop_emit` and its `// ── UI event stub ──` comment block**

- [ ] **Step 3: Verify no remaining references**

Run: `grep -rn "noop_emit\|SharedEngine\|legacy_handlers" src/`
Expected: no matches.

- [ ] **Step 4: Build + run tests + clippy**

Run: `cargo build && cargo test --lib && cargo clippy -- -D warnings`
Expected: all three commands succeed. Fix any new clippy warnings inline.

- [ ] **Step 5: Commit**

```bash
git add src/engine/mod.rs
git commit -m "refactor(engine): remove noop_emit shims and legacy gate"
```

---

## Phase 5 — Tier 2 Integration Tests with Fakes (Day 3)

### Task 15: Create `tests/common/fakes.rs` — platform fakes

**Files:**
- Create: `tests/common/mod.rs`
- Create: `tests/common/fakes.rs`

- [ ] **Step 1: Create `tests/common/mod.rs`**

```rust
pub mod fakes;
```

- [ ] **Step 2: Create `tests/common/fakes.rs`**

```rust
//! In-memory platform + provider fakes used by Tier 2 engine integration tests.

use std::sync::Mutex;

use async_trait::async_trait;
use futures_util::stream;

use quill::platform::context::AppContext;
use quill::platform::traits::{
    CaptureResult, CaptureSource, ContextProbe, ScreenRect, TextCapture, TextReplace,
};
use quill::providers::{ChunkStream, Provider};

// ── FakeCapture ───────────────────────────────────────────────────────────

#[derive(Default)]
pub struct FakeCapture {
    queue: Mutex<Vec<CaptureResult>>,
}

impl FakeCapture {
    pub fn with_text(text: &str) -> Self {
        Self {
            queue: Mutex::new(vec![CaptureResult {
                text: text.to_string(),
                anchor: Some(ScreenRect {
                    left: 100,
                    top: 200,
                    right: 400,
                    bottom: 240,
                }),
                source: CaptureSource::Uia,
            }]),
        }
    }

    pub fn empty() -> Self {
        Self {
            queue: Mutex::new(vec![CaptureResult::default()]),
        }
    }
}

#[async_trait]
impl TextCapture for FakeCapture {
    async fn capture(&self) -> CaptureResult {
        self.queue.lock().unwrap().pop().unwrap_or_default()
    }
}

// ── FakeReplace ───────────────────────────────────────────────────────────

#[derive(Default)]
pub struct FakeReplace {
    pub pasted: Mutex<Vec<String>>,
}

impl FakeReplace {
    pub fn last(&self) -> Option<String> {
        self.pasted.lock().unwrap().last().cloned()
    }
}

#[async_trait]
impl TextReplace for FakeReplace {
    async fn paste(&self, text: &str) -> anyhow::Result<()> {
        self.pasted.lock().unwrap().push(text.to_string());
        Ok(())
    }
}

// ── FakeContext ───────────────────────────────────────────────────────────

#[derive(Default)]
pub struct FakeContext {
    pub ctx: AppContext,
}

impl FakeContext {
    pub fn with_app(app: &str, tone: &str, hint: &str) -> Self {
        Self {
            ctx: AppContext {
                app: app.into(),
                tone: tone.into(),
                hint: hint.into(),
            },
        }
    }
}

impl ContextProbe for FakeContext {
    fn active_context(&self) -> AppContext {
        self.ctx.clone()
    }
}

// ── FakeProvider ──────────────────────────────────────────────────────────

pub struct FakeProvider {
    chunks: Vec<String>,
    err: Option<String>,
}

impl FakeProvider {
    pub fn with_chunks(chunks: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            chunks: chunks.into_iter().map(Into::into).collect(),
            err: None,
        }
    }

    pub fn failing(message: impl Into<String>) -> Self {
        Self {
            chunks: vec![],
            err: Some(message.into()),
        }
    }
}

#[async_trait]
impl Provider for FakeProvider {
    async fn stream_completion(
        &self,
        _system: &str,
        _user: &str,
    ) -> Result<ChunkStream, String> {
        if let Some(err) = &self.err {
            return Err(err.clone());
        }
        let chunks = self.chunks.clone();
        Ok(Box::pin(stream::iter(chunks)))
    }
}
```

- [ ] **Step 3: Run `cargo check --tests`**

Run: `cargo check --tests`
Expected: clean — common module compiles even with no tests yet referencing it. (Rust warns about unused modules in integration tests unless at least one `.rs` test file in `tests/` uses them. Add `#[allow(dead_code)]` above the `pub mod fakes;` declaration in `tests/common/mod.rs` if needed.)

- [ ] **Step 4: Commit**

```bash
git add tests/common/mod.rs tests/common/fakes.rs
git commit -m "test(fakes): in-memory capture/replace/context/provider fakes"
```

### Task 16: Integration test — hotkey flow happy + empty paths

**Files:**
- Create: `tests/engine_integration.rs`

- [ ] **Step 1: Create `tests/engine_integration.rs` with a shared harness**

```rust
//! Tier 2 integration tests: engine orchestration with in-memory fakes.

mod common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use common::fakes::{FakeCapture, FakeContext, FakeProvider, FakeReplace};

use quill::core::config::Config;
use quill::core::modes::{ChainConfig, ModeConfig};
use quill::engine::Engine;
use quill::platform::traits::{ContextProbe, TextCapture, TextReplace};
use quill::providers::Provider;
use quill::state::{AppState, UiCommand, UiEvent};

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
    Harness { engine, state, rx, replace }
}

async fn drain_events(rx: &mut mpsc::UnboundedReceiver<UiEvent>) -> Vec<UiEvent> {
    let mut out = Vec::new();
    // Yield once so all queued sends are visible.
    tokio::task::yield_now().await;
    while let Ok(event) = rx.try_recv() {
        out.push(event);
    }
    out
}

#[tokio::test]
async fn hotkey_happy_path_emits_show_overlay_with_selected_text() {
    let capture: Arc<dyn TextCapture> = Arc::new(FakeCapture::with_text("Hello, world!"));
    let context: Arc<dyn ContextProbe> = Arc::new(FakeContext::with_app("notepad", "neutral", "general"));
    let provider: Arc<dyn Provider> = Arc::new(FakeProvider::with_chunks(Vec::<String>::new()));

    let mut h = build_harness(capture, context, provider);
    quill::engine::hotkey_flow::handle_hotkey(h.engine.clone()).await;

    let events = drain_events(&mut h.rx).await;
    assert_eq!(events.len(), 1, "expected exactly one ShowOverlay, got {events:?}");
    match &events[0] {
        UiEvent::ShowOverlay { text, context, suggestion } => {
            assert_eq!(text, "Hello, world!");
            assert_eq!(context.app, "notepad");
            assert!(suggestion.is_some(), "non-empty selection should carry a suggestion");
        }
        other => panic!("expected ShowOverlay, got {other:?}"),
    }

    let s = h.state.lock().unwrap();
    assert_eq!(s.selected_text, "Hello, world!");
    assert_eq!(s.last_app_hint, "general");
    assert!(s.is_visible);
}

#[tokio::test]
async fn hotkey_empty_selection_emits_show_overlay_without_suggestion() {
    let capture: Arc<dyn TextCapture> = Arc::new(FakeCapture::empty());
    let context: Arc<dyn ContextProbe> = Arc::new(FakeContext::with_app("explorer", "neutral", "general"));
    let provider: Arc<dyn Provider> = Arc::new(FakeProvider::with_chunks(Vec::<String>::new()));

    let mut h = build_harness(capture, context, provider);
    quill::engine::hotkey_flow::handle_hotkey(h.engine.clone()).await;

    let events = drain_events(&mut h.rx).await;
    assert_eq!(events.len(), 1);
    match &events[0] {
        UiEvent::ShowOverlay { text, suggestion, .. } => {
            assert!(text.is_empty());
            assert!(suggestion.is_none());
        }
        other => panic!("expected ShowOverlay, got {other:?}"),
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test engine_integration`
Expected: 2 passed.

- [ ] **Step 3: Commit**

```bash
git add tests/engine_integration.rs
git commit -m "test(engine): hotkey flow integration coverage"
```

### Task 17: Integration test — execute_mode streaming

**Files:**
- Modify: `tests/engine_integration.rs`

- [ ] **Step 1: Append streaming test cases**

```rust
#[tokio::test]
async fn execute_mode_streams_and_emits_stream_done() {
    let capture: Arc<dyn TextCapture> = Arc::new(FakeCapture::with_text("Hello"));
    let context: Arc<dyn ContextProbe> = Arc::new(FakeContext::default());
    let provider: Arc<dyn Provider> =
        Arc::new(FakeProvider::with_chunks(vec!["Hola", " ", "mundo"]));

    let mut h = build_harness(capture, context, provider);

    // Simulate the hotkey path first so selected_text is populated.
    quill::engine::hotkey_flow::handle_hotkey(h.engine.clone()).await;
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
    quill::engine::hotkey_flow::handle_hotkey(h.engine.clone()).await;
    let _ = drain_events(&mut h.rx).await;

    h.engine
        .execute_mode("rewrite".into(), "auto".into(), None)
        .await;

    let events = drain_events(&mut h.rx).await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e, UiEvent::StreamError { message } if message.contains("network down"))),
        "expected StreamError, got {events:?}"
    );
    assert!(!h.state.lock().unwrap().is_streaming);
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test engine_integration`
Expected: 4 passed.

- [ ] **Step 3: Commit**

```bash
git add tests/engine_integration.rs
git commit -m "test(engine): execute_mode streaming + error coverage"
```

### Task 18: Integration test — execute_chain emits ChainProgress

**Files:**
- Modify: `tests/engine_integration.rs`

- [ ] **Step 1: Append**

```rust
#[tokio::test]
async fn execute_chain_emits_progress_per_step() {
    let capture: Arc<dyn TextCapture> = Arc::new(FakeCapture::with_text("Hello"));
    let context: Arc<dyn ContextProbe> = Arc::new(FakeContext::default());
    let provider: Arc<dyn Provider> =
        Arc::new(FakeProvider::with_chunks(vec!["ok"]));

    let mut h = build_harness(capture, context, provider);
    quill::engine::hotkey_flow::handle_hotkey(h.engine.clone()).await;
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
    assert_eq!(progress_events.len(), 2, "two-step chain should emit 2 progress events");
    assert_eq!(progress_events[0], (1, 2, "rewrite".to_string()));
    assert_eq!(progress_events[1], (2, 2, "translate".to_string()));

    assert!(events.iter().any(|e| matches!(e, UiEvent::StreamDone { .. })));
}
```

Note: `FakeProvider::with_chunks` as implemented consumes its scripted chunks once. Because each chain step builds a fresh stream via `stream_completion`, and `FakeProvider::chunks` is cloned inside each call, both steps receive the same `["ok"]` script. That's fine for this test.

- [ ] **Step 2: Run**

Run: `cargo test --test engine_integration`
Expected: 5 passed.

- [ ] **Step 3: Commit**

```bash
git add tests/engine_integration.rs
git commit -m "test(engine): execute_chain ChainProgress coverage"
```

### Task 19: Integration test — compare_modes emits ComparisonResult

**Files:**
- Modify: `tests/engine_integration.rs`

- [ ] **Step 1: Append**

```rust
#[tokio::test]
async fn compare_modes_emits_comparison_result() {
    let capture: Arc<dyn TextCapture> = Arc::new(FakeCapture::with_text("Hello"));
    let context: Arc<dyn ContextProbe> = Arc::new(FakeContext::default());
    let provider: Arc<dyn Provider> =
        Arc::new(FakeProvider::with_chunks(vec!["A-out"]));

    let mut h = build_harness(capture, context, provider);
    quill::engine::hotkey_flow::handle_hotkey(h.engine.clone()).await;
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
        UiEvent::ComparisonResult { mode_a, result_a, mode_b, result_b } => {
            Some((mode_a.clone(), result_a.clone(), mode_b.clone(), result_b.clone()))
        }
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
```

- [ ] **Step 2: Run + commit**

Run: `cargo test --test engine_integration`
Expected: 6 passed.

```bash
git add tests/engine_integration.rs
git commit -m "test(engine): compare_modes integration coverage"
```

### Task 20: Integration test — ConfirmReplace invokes TextReplace

**Files:**
- Modify: `tests/engine_integration.rs`

- [ ] **Step 1: Append**

```rust
#[tokio::test]
async fn confirm_replace_pastes_last_result_and_pushes_undo() {
    let capture: Arc<dyn TextCapture> = Arc::new(FakeCapture::with_text("Hello"));
    let context: Arc<dyn ContextProbe> = Arc::new(FakeContext::default());
    let provider: Arc<dyn Provider> =
        Arc::new(FakeProvider::with_chunks(vec!["Hola mundo"]));

    let mut h = build_harness(capture, context, provider);
    quill::engine::hotkey_flow::handle_hotkey(h.engine.clone()).await;
    let _ = drain_events(&mut h.rx).await;

    h.engine
        .execute_mode("rewrite".into(), "auto".into(), None)
        .await;
    let _ = drain_events(&mut h.rx).await;

    h.engine.handle_command(UiCommand::ConfirmReplace).await;

    assert_eq!(h.replace.last().as_deref(), Some("Hola mundo"));
    let s = h.state.lock().unwrap();
    assert_eq!(s.undo_stack.len(), 1);
    assert_eq!(s.undo_stack[0], "Hello");
}
```

- [ ] **Step 2: Run + commit**

Run: `cargo test --test engine_integration`
Expected: 7 passed.

```bash
git add tests/engine_integration.rs
git commit -m "test(engine): ConfirmReplace invokes TextReplace + undo stack"
```

---

## Phase 6 — Boot Stub + Close Out (Day 3)

### Task 21: Update `main.rs` to construct the new Engine

This is NOT a full boot (that lands in Plan 3/4 once the Slint UI exists). The goal is just to prove the new Engine links against real platform impls and the crate still produces a runnable binary.

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Replace `src/main.rs`**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod engine;
mod platform;
mod providers;
mod state;

use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

use crate::core::config::load_config;
use crate::core::modes::load_modes;
use crate::engine::Engine;
use crate::platform::capture::Capture;
use crate::platform::context::Context;
use crate::platform::replace::Replace;
use crate::platform::traits::{ContextProbe, TextCapture, TextReplace};
use crate::providers::{build_provider, Provider};
use crate::state::{AppState, UiEvent};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("quill=info")),
        )
        .init();

    tracing::info!("Quill boot (Plan 2 shell) — Slint UI lands in Plan 3");

    let config = load_config();
    let (modes, chains) = load_modes(&config);

    let state = Arc::new(Mutex::new(AppState::new()));
    let (event_tx, _event_rx) = mpsc::unbounded_channel::<UiEvent>();

    let capture: Arc<dyn TextCapture>  = Arc::new(Capture);
    let replace: Arc<dyn TextReplace>  = Arc::new(Replace);
    let context: Arc<dyn ContextProbe> = Arc::new(Context);
    let provider: Arc<dyn Provider>    = build_provider(&config);

    let _engine = Engine::new(
        config, modes, chains, state, event_tx, capture, replace, context, provider,
    );

    tracing::info!("Engine constructed — exiting (no UI yet)");
    Ok(())
}
```

- [ ] **Step 2: Verify `load_config` exists**

Run: `grep -n "pub fn load_config" src/core/config.rs`
If the function name differs, update the import accordingly. If there's no public loader at all, use `Config::default()` in place of `load_config()` for this stub.

- [ ] **Step 3: Build release + debug**

Run: `cargo build && cargo build --release`
Expected: both succeed.

- [ ] **Step 4: Smoke run**

Run: `cargo run`
Expected: exits cleanly with the two tracing INFO lines visible.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat(main): construct new Engine from real platform impls"
```

### Task 22: Final verification + tag `plan-02-engine-refactor-complete`

- [ ] **Step 1: Full check sweep**

Run each command and verify it passes:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --lib
cargo test --test engine_integration
cargo build --release
```

Expected: all clean. Fix any formatting / clippy issues inline with a follow-up commit.

- [ ] **Step 2: Sanity grep — no lingering Tauri/noop refs**

```bash
grep -rn "noop_emit\|SharedEngine\|tauri::\|#\[tauri::" src/ tests/
```

Expected: no matches.

- [ ] **Step 3: Tag**

```bash
git tag plan-02-engine-refactor-complete
```

- [ ] **Step 4: Write summary to engram via `mem_save`**

Save an observation with:
- title: "Plan 2 (State + Engine refactor) complete"
- type: architecture
- topic_key: `quill/slint-rewrite/plan-02`
- content: Summarize the final structure (`state/`, `engine/` submodules, fakes, integration test count) and the final green-test count. Note any remaining follow-ups for Plan 3 (main.rs still no Slint window; UiCommand/UiEvent channels are built but not yet drained by any UI).

---

## Self-Review Notes

**Spec coverage:** Phase 3 of the spec's migration plan (State + Engine refactor, 3 days) is fully addressed across Tasks 1-22. Items intentionally deferred to later plans:
- Full `AppBridge` Slint global singleton (Plan 3)
- `ui/bridge.rs` (Plan 3)
- `TemplatesUpdated`, `HistoryLoaded`, `HistoryEntryUpdated`, `ClipboardChange` UiEvents (Plan 4 — history/templates land with Expanded view)
- `SaveConfig` / `SaveTemplate` / `DeleteTemplate` / `OpenFullPanel` / `CloseFullPanel` / tab switching commands (Plan 3/4 as the UI lands)

**Type consistency check:** `Engine` is `Clone` (Arc-backed); every flow function takes `Engine` by value and `.clone()`s into spawned tasks. Fakes are typed against `quill::platform::traits::*` which must be `pub` — already verified in Plan 1. `FakeProvider` returns `ChunkStream` which is `pub` in `providers::mod.rs` — verified.

**Known sharp edges for the executor:**
1. `Engine::execute_mode` and `execute_chain` borrow `self.modes()` for `build_prompt` while holding the AppState lock. Keep the lock scope tight — drop before `.await`.
2. `handle_command(RequestTutorExplain)` calls a new `history::get_entry` helper added in Task 13. If that function already exists under a different name, update the call site rather than adding a second implementation.
3. The `#[cfg(any())]` gate in Task 6 is a non-obvious Rust idiom — it's always-false so the inner module is skipped by the compiler but still parsed. Alternative: comment out instead. Gate is preferred because it catches syntax rot earlier.
4. `tests/common/mod.rs` must be declared as `mod common;` inside each test file that uses it (standard Rust integration-test pattern).

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-04-14-quill-slint-plan-02-state-engine.md`.**

Execution options:

1. **Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration. REQUIRED SUB-SKILL: `superpowers:subagent-driven-development`.
2. **Inline Execution** — I execute tasks in this session using `superpowers:executing-plans`, batch execution with checkpoints.
