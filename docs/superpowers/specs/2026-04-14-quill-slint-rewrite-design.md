# Quill Slint Rewrite — Design Specification

**Date:** 2026-04-14
**Status:** Approved (pending final user review)
**Branch:** `claude/slint-rewrite`
**Supersedes:** Tauri + React architecture from `claude/rust-migration`

---

## Executive Summary

Quill migrates from Tauri + React to **pure Rust + Slint UI**, dropping cross-platform support to focus on a high-quality Windows 11 experience. The rewrite is destructive in-place: Tauri and the React frontend are deleted, most of the existing pure-Rust backend (`core/`, `providers/`, most of `platform/`) is preserved, and a new Slint UI layer replaces the webview. Target: ~22.5 working days (~4.5 calendar weeks) to full feature parity plus a new floating plume indicator (Copilot-style).

---

## Decisions Log

| # | Decision | Choice |
|---|---|---|
| 1 | Visual direction | **Mica Fluent** (Windows 11 native frosted-glass backdrop) |
| 2 | Window topology | **Unified Adaptive** — one window that morphs compact ↔ expanded |
| 3 | Feature scope for day 1 | **Full parity** — every current feature ported before ship |
| 4 | Floating plume indicator | **In day-1 scope** (new feature, not just parity) |
| 5 | Brand glyph | **Classic Feather plume**, used consistently everywhere (no more serif Q) |
| 6 | Indicator color | Brand violet `#c084fc` (distinctive); Mica blue `#60cdff` for window chrome |
| 7 | Window position on summon | Near UIA selection rect → caret rect → cursor fallback |
| 8 | Migration strategy | **Destructive in-place** on `claude/slint-rewrite` branch, `tauri-final` tag as safety net |
| 9 | Cross-platform support | **Dropped** — Windows only, hardcoded `windows-rs` |
| 10 | Project shape | **Single-crate layered** — modules in one `quill` crate, no workspace |

---

## Table of Contents

1. Architecture & Module Layout
2. Window Topology & Slint Component Tree
3. State Management & Data Flow
4. Windows Integration (Mica, UIA, Caret, Tray, Hotkey)
5. Error Handling & Failure Modes
6. Testing Strategy
7. Migration Execution Order
8. Dependencies
9. Risks

---

## 1. Architecture & Module Layout

### Project Shape

Single binary, single `Cargo.toml`, Windows-hardcoded. No workspace, no cross-platform gates.

### Directory Tree

```
quill/
├── Cargo.toml                # single crate
├── build.rs                  # slint-build compiles .slint files; resvg generates quill.ico from plume.svg
├── resources/
│   └── icons/
│       ├── plume.svg         # single source of truth for brand glyph (Classic Feather)
│       └── quill.ico         # generated at build from plume.svg, multi-size
└── src/
    ├── main.rs               # entry: init logging, load config, wire subsystems, run Slint event loop
    │
    ├── core/                 # PURE LOGIC — no OS, no UI, no Slint, no tokio runtime
    │   ├── config.rs         # KEEP — YAML loader, Template, Persona, save_user_config
    │   ├── history.rs        # KEEP — SQLite CRUD, favorites, export
    │   ├── modes.rs          # KEEP — mode/chain definitions
    │   ├── prompt.rs         # KEEP — build_prompt, language handling
    │   ├── think_filter.rs   # KEEP — <think> stripper
    │   ├── tutor.rs          # KEEP — explain/lesson prompts
    │   ├── clipboard.rs      # KEEP — clipboard monitor
    │   └── errors.rs         # NEW — UserFacingError + translate_* functions
    │
    ├── providers/            # PURE LOGIC — HTTP streaming
    │   ├── generic.rs        # KEEP
    │   ├── openai.rs         # KEEP
    │   ├── openrouter.rs     # KEEP
    │   └── ollama.rs         # KEEP
    │
    ├── platform/             # WINDOWS APIs — no UI
    │   ├── capture.rs        # REFACTOR — UIA-first, clipboard fallback
    │   ├── uia.rs            # NEW — IUIAutomation wrapper
    │   ├── replace.rs        # KEEP — simulated Ctrl+V paste (via enigo)
    │   ├── context.rs        # KEEP — active app hint
    │   ├── caret.rs          # NEW — WinEvent hooks → channel
    │   ├── mica.rs           # NEW — DWM Mica backdrop
    │   ├── dwm_shadow.rs     # NEW — DWM native drop shadow
    │   ├── hotkey.rs         # REFACTOR — global-hotkey crate
    │   ├── tray.rs           # NEW — tray-icon crate wrapper
    │   └── traits.rs         # NEW — TextCapture, TextReplace, ContextProbe traits
    │
    ├── engine/               # ORCHESTRATION — uses core + providers + platform
    │   ├── engine.rs         # REFACTOR — no Tauri AppHandle, use channels
    │   ├── streaming.rs      # REFACTOR — stream via tokio::mpsc
    │   ├── hotkey_flow.rs    # REFACTOR — handle_hotkey → capture → show
    │   └── compare.rs        # KEEP logic, update signatures
    │
    ├── state/                # SOURCE OF TRUTH — Arc<Mutex<AppState>>
    │   ├── app_state.rs      # NEW — AppState struct
    │   └── events.rs         # NEW — UiEvent, UiCommand enums
    │
    └── ui/                   # SLINT GLUE
        ├── bridge.rs         # NEW — AppState ↔ Slint property sync
        ├── main_window.rs    # NEW — MainWindow construction + Mica
        ├── pencil_window.rs  # NEW — Floating plume indicator window
        └── slint/            # .slint source files
            ├── main_window.slint
            ├── compact.slint
            ├── expanded.slint
            ├── tabs/
            │   ├── write.slint
            │   ├── history.slint
            │   ├── tutor.slint
            │   ├── settings.slint
            │   └── compare.slint
            ├── components/
            │   ├── lang_row.slint
            │   ├── mode_row.slint
            │   ├── chain_row.slint
            │   ├── diff_view.slint
            │   ├── plume.slint
            │   └── toast.slint
            └── pencil.slint
```

### Layer Dependency Rules

```
main.rs  ──wires──▶  ui, engine, platform, state, core, providers
ui       ──────▶    state, engine
engine   ──────▶    core, providers, platform, state
state    ──────▶    core
platform ──────▶    (nothing project-internal — only `windows` crate + std)
core     ──────▶    (nothing project-internal — only std + 3rd-party pure libs)
providers──────▶    (only reqwest + futures, no project deps)
```

One-way dependencies only. No cycles. `core/` and `providers/` are trivially unit-testable because they depend on nothing project-internal. `platform/` is mockable via the traits in `platform/traits.rs`.

## 2. Window Topology & Slint Component Tree

### Two Slint Windows

- **MainWindow** — the morphing compact ↔ expanded window, summoned by hotkey, tray, or pencil click
- **PencilWindow** — tiny always-on-top plume indicator that follows the focused text control's caret

Both live in a single Slint event loop.

### MainWindow Specs

| Attribute | Compact state | Expanded state |
|---|---|---|
| Size | 380 × 260 px | 840 × 600 px (resizable, min 680 × 480) |
| Decorations | none (custom drag handle) | none (custom chrome) |
| Backdrop | Mica (`DWMSBT_MAINWINDOW`) | Mica (`DWMSBT_TABBEDWINDOW`) |
| Shadow | DWM native | DWM native |
| Corner radius | 12 px (Fluent) | 12 px |
| Always-on-top | yes | no |
| Resizable | no | yes |
| Position on summon | near selection rect (UIA) → caret → cursor | restored last position |

### MainWindow Component Tree

```
MainWindow.slint (root)
│
├─ states: compact | expanded (drives animated resize + cross-fade)
│
├─ CompactView (visible when state == compact)
│   ├─ Header                  — plume glyph, "Quill", ↗ expand, ✕ close
│   ├─ InputPreview            — selected text (italic, muted)
│   ├─ LangRow                 — Auto + EN/FR/ES/DE/JA/PT/ZH pills
│   ├─ ModeRow                 — 7 mode icon buttons
│   ├─ ChainRow                — multi-step chain buttons
│   ├─ InstructionField        — collapsible "+ Add instruction…"
│   └─ StreamOutput            — live token stream + actions when streaming/done
│
└─ ExpandedView (visible when state == expanded)
    ├─ Chrome                  — plume, title, tab row, — □ ✕
    ├─ TabBar                  — Write | History | Tutor | Compare | Settings
    └─ TabContent (stack)
        ├─ WriteTab            — selected text, live stream, diff view, replace/copy/retry/undo
        ├─ HistoryTab          — list, search, favorites filter, detail pane, export
        ├─ TutorTab            — explain-last-edit + daily/weekly lessons
        ├─ CompareTab          — two modes side-by-side, winner selector
        └─ SettingsTab         — provider, API key, hotkey, modes, chains, templates, persona
```

### PencilWindow Specs

| Attribute | Value |
|---|---|
| Size | 32 × 32 px |
| Decorations | none, transparent |
| Always-on-top | yes |
| Click-through | yes when idle (`WS_EX_LAYERED \| WS_EX_TRANSPARENT`), off when pointer within 40 px |
| Position | driven by caret rect + 24 px offset right |
| Visible | only when focused control is editable text (UIA-verified) |

```
PencilWindow.slint (root)
└─ PlumeIndicator
    ├─ Glyph (brand violet #c084fc, drop-shadow glow)
    ├─ HoverTooltip ("Rewrite with Quill")
    └─ FadeInOut (60ms in, 120ms out)
```

### Morph Animation (compact ↔ expanded)

Three-beat sequence, ~500 ms total:

1. **Fade out current content** — Slint opacity animation on `CompactView` or `ExpandedView`, 140 ms, ease-out
2. **Window resize** — Rust timer ticks `window.set_size()` with eased interpolation, 180 ms, cubic ease-in-out
3. **Fade in new content** — Slint opacity animation on target view, 160 ms, ease-in

Beats 1 and 3 are declarative Slint animations. Beat 2 is driven by `slint::Timer` at 60 Hz from Rust.

### Focus Handling

- **MainWindow compact**: grabs focus on summon. ESC dismisses. Click outside → hides.
- **MainWindow expanded**: normal app window. Lose focus → stays open.
- **PencilWindow**: NEVER steals focus (`WS_EX_NOACTIVATE`). Clicking fires the same command as the hotkey.

## 3. State Management & Data Flow

### Single Source of Truth

All state lives in `Arc<Mutex<AppState>>`, mutated by the engine and the UI bridge. Slint properties are a **projection** of AppState, not the source. If they drift, AppState wins on the next event.

```rust
// state/app_state.rs
pub struct AppState {
    // Session (reset on every hotkey trigger)
    pub selected_text: String,
    pub last_result: String,
    pub last_mode: String,
    pub last_language: Language,
    pub last_app_hint: String,
    pub last_entry_id: Option<i64>,
    pub undo_stack: Vec<String>,

    // View
    pub view_mode: ViewMode,           // Compact | Expanded
    pub current_tab: TabId,            // Write | History | Tutor | Compare | Settings
    pub is_visible: bool,
    pub is_streaming: bool,
    pub is_done: bool,
    pub stream_buffer: String,
    pub chain_progress: Option<ChainProgress>,

    // Config mirror
    pub config: Config,
    pub modes: Vec<ModeInfo>,
    pub chains: Vec<ChainInfo>,
    pub templates: Vec<Template>,

    // Expanded-only feature state
    pub history_entries: Vec<HistoryEntry>,
    pub history_filter: HistoryFilter,
    pub comparison: Option<ComparisonResult>,
    pub tutor_explanation: Option<String>,
    pub tutor_lessons: HashMap<LessonPeriod, String>,

    // UX
    pub error: Option<String>,
    pub toast: Option<Toast>,
}
```

### Engine ↔ UI Channels

```
┌─────────────────┐    UiCommand    ┌─────────────────┐
│     Slint UI    │ ───────────────▶│     Engine      │
│  (event loop)   │                 │  (tokio tasks)  │
│                 │◀─── UiEvent ────│                 │
└─────────────────┘                 └─────────────────┘
         │                                    │
         └──────── Arc<Mutex<AppState>> ──────┘
```

- `tokio::mpsc::UnboundedSender<UiCommand>` — UI → Engine (button clicks, tab switches, settings saves)
- `tokio::mpsc::UnboundedSender<UiEvent>` — Engine → UI (stream chunks, errors, history loaded)

These enums live in `state/events.rs` and are the **only** communication surface between the two sides.

### `UiEvent` enum (engine → UI)

```rust
pub enum UiEvent {
    ShowOverlay { text: String, context: AppContext, suggestion: Option<Suggestion> },
    Dismiss,

    StreamStart { mode: String, language: Language },
    StreamChunk { text: String },
    StreamDone  { full_text: String, entry_id: Option<i64> },
    StreamError { message: String },

    ExpandView,
    CollapseView,
    SwitchTab(TabId),

    ChainProgress { step: usize, total: usize, mode: String },

    HistoryLoaded(Vec<HistoryEntry>),
    HistoryEntryUpdated { id: i64, favorited: bool },

    TutorExplanation { entry_id: i64, text: String },
    TutorLesson      { period: LessonPeriod, text: String },

    ComparisonResult(ComparisonResult),

    TemplatesUpdated(Vec<Template>),
    ClipboardChange  { text: String },

    Error { message: String },
    Toast { kind: ToastKind, message: String },
}
```

### `UiCommand` enum (UI → engine)

```rust
pub enum UiCommand {
    ExecuteMode  { mode: String, language: Language, extra: Option<String> },
    ExecuteChain { chain_id: String, language: Language, extra: Option<String> },
    Retry        { extra: Option<String> },
    CancelStream,
    ConfirmReplace,
    SetResult    { text: String },
    Undo,
    ToggleView,
    SwitchTab(TabId),
    RequestTutorExplain { entry_id: Option<i64> },
    GenerateLesson      { period: LessonPeriod },
    CompareModes        { mode_a: String, mode_b: String, language: Language, extra: Option<String> },
    GetPronunciation    { text: String, language: Language },
    LoadHistory         { limit: usize, language_filter: Option<Language> },
    ToggleFavorite      { entry_id: i64 },
    ExportHistory       { format: ExportFormat },
    SaveConfig          (ConfigUpdate),
    SaveTemplate        (Template),
    DeleteTemplate      { name: String },
    OpenFullPanel,
    CloseFullPanel,
    Dismiss,
}
```

### UI Bridge (`ui/bridge.rs`)

Runs on the Slint event loop thread. Two halves:

1. **Event pump** — task draining `mpsc::UnboundedReceiver<UiEvent>`. Each event calls `slint::invoke_from_event_loop(move || { ... })` to update Slint Global properties on the correct thread.
2. **Command forwarder** — Slint callbacks pack arguments into `UiCommand` and send via `mpsc::UnboundedSender<UiCommand>`. Engine spawns a tokio task to handle it.

### Slint Global Singleton

```slint
// ui/slint/app_bridge.slint
export global AppBridge {
    // Session
    in-out property <string> selected-text;
    in-out property <string> stream-buffer;
    in-out property <bool>   is-streaming;
    in-out property <bool>   is-done;
    in-out property <string> last-result;
    in-out property <string> active-mode;

    // View
    in-out property <ViewMode> view-mode;
    in-out property <TabId>    current-tab;

    // Collections
    in-out property <[ModeInfo]>    modes;
    in-out property <[ChainInfo]>   chains;
    in-out property <[LanguageOpt]> languages;
    in-out property <[HistoryEntry]> history;
    in-out property <[Template]>    templates;

    // UX
    in-out property <string>        error-message;
    in-out property <Toast>         toast;

    // Callbacks
    callback execute-mode(string, string);
    callback execute-chain(string, string);
    callback retry(string);
    callback confirm-replace();
    callback cancel-stream();
    callback set-result(string);
    callback undo();
    callback toggle-view();
    callback switch-tab(TabId);
    callback request-tutor-explain(int);
    callback generate-lesson(string);
    callback compare-modes(string, string, string);
    callback get-pronunciation(string);
    callback load-history();
    callback toggle-favorite(int);
    callback export-history(string);
    callback save-config(string);
    callback save-template(Template);
    callback delete-template(string);
    callback open-full-panel();
    callback close-full-panel();
    callback dismiss();
}
```

Slint components bind directly to `AppBridge` properties (e.g. `AppBridge.selected-text`, `AppBridge.is-streaming`). Rust sets the properties. UI re-renders. Zero IPC, zero serialization.

### Threading Summary

| Thread | Runs | Owns |
|---|---|---|
| **Main (Slint event loop)** | Slint rendering, UI callbacks | `ComponentHandle`s |
| **Tokio runtime (multi-thread)** | Engine tasks, HTTP streaming, history writes | business logic |
| **WinEvent hook thread** | `SetWinEventHook` for caret tracking | focus/caret → channel |
| **Clipboard monitor thread** | Clipboard polling | `tokio::mpsc::Sender<String>` |

All cross-thread communication goes through channels or the shared `Arc<Mutex<AppState>>`.

## 4. Windows Integration

### 4.1 Mica Backdrop (`platform/mica.rs`)

1. Create Slint window with `winit::WindowAttributes::transparent(true)` and no decorations — Slint's Skia backend honors alpha.
2. Retrieve HWND via `slint::winit::WinitWindowAccessor`.
3. Enable dark mode: `DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE = 20, &TRUE, 4)`.
4. Set backdrop: `DwmSetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE = 38, &DWMSBT_MAINWINDOW = 2, 4)`. Use `DWMSBT_TABBEDWINDOW = 4` for expanded view.
5. Request round corners: `DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE = 33, &DWMWCP_ROUND = 2, 4)`.
6. Extend frame into client area: `DwmExtendFrameIntoClientArea(hwnd, &MARGINS { cxLeftWidth: -1, cxRightWidth: -1, cyTopHeight: -1, cyBottomHeight: -1 })`.

Slint root window must have `background: transparent;`. Content panels use `rgba(38,36,48,0.4)` tints so Mica shows through.

**Fallback:** on pre-22H2 Windows, `DWMWA_SYSTEMBACKDROP_TYPE` silently fails. Detect via `RtlGetVersion()`, fall back to solid `#1e1e28`.

### 4.2 DWM Native Shadow (`platform/dwm_shadow.rs`)

Native shadow comes free once `DwmExtendFrameIntoClientArea` is called with non-zero margins. Also set `DWMWA_NCRENDERING_POLICY = DWMNCRP_ENABLED` explicitly. **No CSS shadow** — that's what makes the current Tauri version look wrong.

### 4.3 UIA Text Capture (`platform/uia.rs`)

New module wrapping `IUIAutomation`. Initialized with `CoInitializeEx(COINIT_MULTITHREADED)` in a dedicated worker thread.

```rust
pub struct Uia { automation: IUIAutomation }

impl Uia {
    pub fn new() -> Result<Self>;
    pub fn focused_element(&self) -> Result<IUIAutomationElement>;
    pub fn selected_text(&self) -> Result<Option<String>>;
    pub fn selection_bounds(&self) -> Result<Option<RECT>>;  // screen coords
    pub fn caret_bounds(&self) -> Result<Option<RECT>>;
    pub fn is_editable_text(&self, element: &IUIAutomationElement) -> Result<bool>;
}
```

- `selected_text()` — `GetCurrentPattern(UIA_TextPatternId)` → `GetSelection()` → iterate `IUIAutomationTextRangeArray` → `GetText(-1)`
- `selection_bounds()` — `GetBoundingRectangles()` on the text range, unioned
- `caret_bounds()` — `TextPattern.GetCaretRange()` (Windows 10+) → zero-length range bounding rect
- `is_editable_text()` — `CurrentControlType == UIA_EditControlTypeId || UIA_DocumentControlTypeId` AND `IsEnabled` AND `TextPattern` supported

### 4.4 Capture Flow (`platform/capture.rs`, refactored)

```rust
pub fn capture_selection() -> CaptureResult {
    wait_for_hotkey_modifiers_released();

    // Step 1: UIA first (zero side-effects)
    if let Ok(Some(text)) = Uia::thread_local().selected_text() {
        let rect = Uia::thread_local().selection_bounds().ok().flatten();
        return CaptureResult { text, anchor: rect, source: Source::Uia };
    }

    // Step 2: clipboard fallback (side-effect: touches clipboard)
    if let Ok(text) = clipboard_capture_fallback() {
        return CaptureResult { text, anchor: None, source: Source::Clipboard };
    }

    CaptureResult::empty()
}
```

UIA is the primary path. Clipboard hack is the safety net for Chromium, elevated windows, and legacy apps with poor UIA exposure.

### 4.5 Caret & Focus Tracking (`platform/caret.rs`)

Dedicated thread running a message pump + `SetWinEventHook`:

```rust
pub fn install_focus_hooks(sender: UnboundedSender<FocusEvent>) {
    std::thread::spawn(move || {
        SetWinEventHook(EVENT_SYSTEM_FOREGROUND, ..., WINEVENT_OUTOFCONTEXT);
        SetWinEventHook(EVENT_OBJECT_FOCUS,      ..., WINEVENT_OUTOFCONTEXT);
        SetWinEventHook(EVENT_OBJECT_LOCATIONCHANGE, ..., WINEVENT_OUTOFCONTEXT);
        run_message_pump();
    });
}

pub enum FocusEvent {
    FocusChanged { editable: bool, caret_rect: Option<RECT>, app_hint: String },
    CaretMoved   { rect: RECT },
    FocusLost,
}
```

Debounces `LOCATIONCHANGE` at ~30 Hz to avoid flooding the channel during rapid caret movement.

### 4.6 Pencil Window Style — Click-Through + Always-on-Top

```rust
pub fn configure_pencil_window_style(hwnd: HWND) {
    let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
    SetWindowLongPtrW(hwnd, GWL_EXSTYLE,
        style | WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT);
}
```

- `WS_EX_LAYERED` — alpha compositing
- `WS_EX_TOOLWINDOW` — no taskbar entry, no Alt-Tab
- `WS_EX_NOACTIVATE` — clicking doesn't steal focus
- `WS_EX_TRANSPARENT` — clicks pass through when idle; toggled off when cursor is within 40 px

Position updates driven by `FocusEvent::CaretMoved` — Slint gets `set_position()` calls.

### 4.7 Global Hotkey (`platform/hotkey.rs`)

Replace `tauri-plugin-global-shortcut` with `global-hotkey` crate:

```rust
use global_hotkey::{GlobalHotKeyManager, hotkey::{HotKey, Code, Modifiers}};

pub struct HotkeyService {
    manager: GlobalHotKeyManager,
    current: Option<HotKey>,
}
```

Events drain from `global_hotkey::GlobalHotKeyEvent::receiver()` on a dedicated thread; each hit posts a command to `engine::hotkey_flow::handle_hotkey`.

### 4.8 Tray Icon (`platform/tray.rs`)

Uses `tray-icon` crate (same crate Tauri uses internally):

```rust
use tray_icon::{TrayIconBuilder, menu::{Menu, MenuItem, PredefinedMenuItem}};

pub fn build_tray(menu_events: UnboundedSender<TrayMenu>) -> Result<TrayIcon> {
    let menu = Menu::new();
    menu.append_items(&[
        &MenuItem::with_id("show",     "Show Quill",          true, None),
        &MenuItem::with_id("panel",    "Open Full Panel…",    true, None),
        &MenuItem::with_id("settings", "Settings…",           true, None),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id("quit",     "Quit Quill",          true, None),
    ])?;
    TrayIconBuilder::new()
        .with_icon(load_embedded_icon())
        .with_tooltip("Quill")
        .with_menu(Box::new(menu))
        .build()
}
```

### 4.9 DPI Awareness

`SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` called in `main()` **before any window creation**. Slint's winit backend honors this. Non-negotiable — without it the app looks blurry on 4K monitors.

### 4.10 Icon Resources

- **Source of truth**: `resources/icons/plume.svg` — Classic Feather glyph, brand violet `#c084fc`
- **App icon**: `resources/icons/quill.ico` — plume on a rounded-square violet radial gradient `#2a1f3d → #1a1530`, packed at 16/20/24/32/40/48/64/96/128/256 px. Generated at build time via `resvg` in `build.rs`. No hand-edited ICOs.
- **In-app header glyph**: raw `plume.svg` rendered live by Slint's SVG support (brand violet, no background)
- **Tray icon**: embedded via `include_bytes!` from the generated `quill.ico`

## 5. Error Handling & Failure Modes

### Principles

1. **Never block the UI thread.** All fallible work runs on tokio; errors travel back as `UiEvent::Error` or `UiEvent::Toast`.
2. **Never show raw Rust error types.** Every `anyhow::Error` is translated to a short, plain-English message at the engine/platform boundary. Technical detail stays in the log.
3. **Degrade gracefully, crash loudly.** Non-critical subsystems (history DB, floating pencil, Mica, tray) can fail individually without killing the app. Critical subsystems (Slint rendering, tokio runtime) abort the process.
4. **Errors during streaming preserve partial output.** If a stream dies at token 400, the user sees the first 400 tokens plus an error toast.

### Error Taxonomy

| Kind | Example | Surfacing |
|---|---|---|
| **Capture** | UIA fails, clipboard fails | Compact overlay "No text selected" state (non-blocking) |
| **Network** | reqwest timeout, DNS fail | Toast: `Couldn't reach {provider}: {short_reason}` |
| **HTTP 401 / 403** | Bad API key | Toast + auto-navigate to Settings tab |
| **HTTP 429** | Rate limited | Toast: `Rate limited — retry in Ns` (parse Retry-After) |
| **HTTP 5xx** | Provider down | Toast with inline Retry button |
| **Stream interrupted** | Connection dropped mid-stream | Preserve partial text, toast with Retry action |
| **Config invalid** | YAML parse fail | Load defaults, toast: `Config invalid, using defaults` |
| **Hotkey collision** | Another app owns the combo | Toast + open Settings hotkey field with error inline |
| **Mica unavailable** | Windows 10 / pre-22H2 | Silent fallback to solid `#1e1e28`, log DEBUG |
| **UIA init fail** | COM or IUIAutomation unavailable | Log WARN, capture uses clipboard-only path |
| **Caret hook fail** | `SetWinEventHook` denied | Log WARN, disable floating pencil this session |
| **Tray icon fail** | `Shell_NotifyIcon` error | Log WARN, continue — hotkey still works |
| **History DB fail** | SQLite corrupt | Log ERROR, disable history this session, toast |
| **Slint render panic** | unrecoverable | Abort. Crash dump written to log file. |

### Error Translation Layer

`core/errors.rs` — new module with one translation function per error family:

```rust
pub fn translate_provider_error(err: &reqwest::Error) -> UserFacingError {
    if err.is_timeout() { return UserFacingError::soft("Request timed out — check your connection.", err); }
    if err.is_connect() { return UserFacingError::soft("Couldn't reach the provider. Check the URL in Settings.", err); }
    if let Some(status) = err.status() {
        match status.as_u16() {
            401 | 403 => UserFacingError::auth("Authentication failed. Check your API key.", err),
            429       => UserFacingError::rate_limit("Rate limited. Try again in a moment.", err),
            500..=599 => UserFacingError::retryable("Provider error — try again in a moment.", err),
            _         => UserFacingError::soft(format!("Unexpected provider response: {status}"), err),
        }
    } else {
        UserFacingError::soft("Network error while talking to the provider.", err)
    }
}
```

`UserFacingError` carries `{ message, severity, auto_action }`. `auto_action` is `None | OpenSettings | Retry | OpenHotkeyField`. The UI bridge reads the action and dispatches it automatically.

### Toast System

- `AppBridge.toast: Toast` property — `{ kind, message, visible, action_label, action_id }`
- `ToastKind`: `Info | Success | Warning | Error`
- Position: bottom-center, 14 px inset, slides up from below
- Auto-dismiss after 5 s unless severity is `Error` with an action
- Click-outside dismisses
- Max 1 visible at a time; subsequent toasts queue

### Logging

- `tracing_subscriber` with env-filter
- Default filter: `quill=info,slint=warn` (suppresses Slint noise)
- File sink: `%LOCALAPPDATA%\Quill\logs\quill.log` — rolling daily, keep 7 days
- `RUST_LOG=quill=debug` for verbose tracing
- Error translation writes original error at DEBUG with full `{:?}` formatting; user-facing message stays at WARN/ERROR

### Crash Behavior

- `panic = abort` in release — no unwinding
- `std::panic::set_hook` writes panic info to `quill.log` before aborting
- No in-app crash report UI
- On next startup, `main.rs` checks for an unclean-shutdown marker and opens Settings → Diagnostics tab with the last panic printed in-place if present

### The Streaming + Dismiss Race

Already handled in current Tauri code via `cancel_tx` in engine.rs — ported verbatim. `UiCommand::Dismiss` sends a cancel signal to the running stream task; the task's `tokio::select!` resolves on the cancel branch and returns without emitting `StreamDone`. AppState resets cleanly.

## 6. Testing Strategy

### Tiers at a Glance

| Tier | What | Where it runs | Speed |
|---|---|---|---|
| **1. Unit** | pure logic (`core/`, `providers/`, `state/`) | `cargo test` | ms |
| **2. Integration** | engine orchestration with fakes | `cargo test` | ms |
| **3. Smoke (platform)** | UIA/capture/hotkey/tray against scripted target | `cargo test --test platform_smoke --features smoke` | seconds |
| **4. UI property** | AppBridge + Slint callbacks, no rendering | `cargo test` | ms |
| **5. Manual E2E** | full flows | run binary | minutes |

Tiers 1-4 run in CI. Tier 5 is pre-release manual.

### Tier 1 — Unit Tests on Pure Modules

- **`core::config`** — YAML round-trip, API key masking, persona/template merge, invalid → defaults
- **`core::history`** — CRUD with in-memory SQLite, favorites, filters, pagination
- **`core::modes`** — mode resolution, chain expansion, template variable substitution
- **`core::prompt`** — language injection, persona tone, extra instruction, edge cases
- **`core::think_filter`** — `<think>` tags split across chunk boundaries
- **`providers::*`** — each provider's request building + response parsing via `wiremock` stub HTTP server; happy path + 4xx/5xx/timeout/malformed chunks
- **`state::app_state`** — pure transition functions (reducer-like)

### Tier 2 — Engine Integration Tests

Selective trait-ification at the platform boundary for testability:

```rust
// platform/traits.rs
#[async_trait::async_trait]
pub trait TextCapture: Send + Sync {
    async fn capture(&self) -> CaptureResult;
}

#[async_trait::async_trait]
pub trait TextReplace: Send + Sync {
    async fn paste(&self, text: &str) -> Result<()>;
}

pub trait ContextProbe: Send + Sync {
    fn active_context(&self) -> AppContext;
}
```

**Trait-ified**: `TextCapture`, `TextReplace`, `ContextProbe`, `Provider` (already a trait).
**Not trait-ified**: `mica`, `dwm_shadow`, `tray`, `hotkey`, `caret` — fire-and-forget side effects with nothing meaningful to assert.

Example integration test:

```rust
#[tokio::test]
async fn hotkey_flow_end_to_end() {
    let fake_capture  = Arc::new(FakeCapture::with("Hello world"));
    let fake_replace  = Arc::new(FakeReplace::default());
    let fake_provider = Arc::new(FakeProvider::with_stream(vec!["Hola", " ", "mundo"]));
    let fake_context  = Arc::new(FakeContext::default());

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    let engine = Engine::new(config_for_test(), fake_capture, fake_replace,
                             fake_provider, fake_context, event_tx);

    engine.handle_hotkey().await;
    assert_matches!(event_rx.recv().await, Some(UiEvent::ShowOverlay { text, .. }) if text == "Hello world");

    engine.execute_mode("translate".into(), Language::Spanish, None).await;
    let chunks = drain_chunks(&mut event_rx).await;
    assert_eq!(chunks.join(""), "Hola mundo");

    engine.confirm_replace().await;
    assert_eq!(fake_replace.last_pasted(), "Hola mundo");
}
```

### Tier 3 — Platform Smoke Tests

Live in `tests/platform_smoke.rs`, gated behind `feature = "smoke"`:

```bash
cargo test --test platform_smoke --features smoke -- --test-threads=1
```

Harness:
1. Spawns `notepad.exe`
2. Types pre-seeded text via `SendInput`
3. Selects with Ctrl+A
4. Invokes `platform::capture::capture_selection()`
5. Asserts the captured string
6. Kills notepad

Tests cover:
- UIA path (notepad exposes full UIA)
- Clipboard fallback (force UIA to fail, verify clipboard works)
- Hotkey register/unregister + synthetic `WM_HOTKEY`
- Tray build/destroy
- Mica attribute apply + read-back (or correct "not supported" on pre-22H2)

`--test-threads=1` because these tests poke real OS global state.

### Tier 4 — UI Property Tests

- AppBridge property round-trip (set from Rust, read back)
- Callback registration + invocation (verify engine receives correct `UiCommand`)
- State machine transitions (verify `view-mode` change activates `expanded` state)

No pixel-level assertions. Visual correctness is Tier 5.

### Tier 5 — Manual E2E Checklist (`docs/TESTING.md`)

- Cold start → tray icon visible → hotkey registers
- Hotkey with text selected in Notepad → overlay opens near selection
- Hotkey with text in Edge / Word / VSCode → overlay opens correctly (UIA path)
- Hotkey with NO text selected → empty overlay, friendly message
- Pick a mode → stream renders → Replace pastes correctly
- Retry with instruction → different result
- Expand to full panel → all 5 tabs load
- Settings → change hotkey → new hotkey works, old one freed
- Settings → change provider / API key → stream works
- Compare → two modes stream → pick one → replace works
- Floating pencil appears in Notepad/Word → click → flow works
- Floating pencil does NOT appear in Explorer, taskbar, non-editable windows
- Ctrl+Z after replace → original text restored
- Alt-Tab away from expanded panel → stays open
- ESC in compact → dismisses cleanly

### CI Setup

GitHub Actions on `windows-latest`:

```yaml
- run: cargo test                                             # Tiers 1, 2, 4
- run: cargo test --features smoke -- --test-threads=1        # Tier 3
- run: cargo clippy -- -D warnings
- run: cargo fmt --check
- run: cargo build --release
```

## 7. Migration Execution Order

### Safety Net

```bash
git tag tauri-final              # permanent rollback anchor
git checkout -b claude/slint-rewrite
```

If the rewrite hits a wall, `git checkout tauri-final` gets the working Tauri Quill back.

### Phase 0 — Demolition & Restructure (0.5 day)

| Action | Files |
|---|---|
| Delete React + build chain | `ui/src/`, `ui/index.html`, `ui/package.json`, `ui/vite.config.js`, `ui/node_modules/`, `ui/dist/` |
| Delete Tauri scaffolding | `ui/src-tauri/tauri.conf.json`, `ui/src-tauri/capabilities/`, `ui/src-tauri/commands.rs` |
| Flatten repo layout | `mv ui/src-tauri/{Cargo.toml,src,build.rs} ./`, `mv ui/src-tauri/icons ./resources/icons`, `rm -rf ui/` |
| Rewrite `Cargo.toml` | Remove `tauri`, `tauri-build`, `tauri-plugin-global-shortcut`, `arboard`. Add `slint`, `slint-build`, `global-hotkey`, `tray-icon`. Expand `windows` features. |
| Create plume source | Hand-draft `resources/icons/plume.svg`; add `build.rs` step that calls `resvg` to generate `quill.ico` |
| Minimal `src/main.rs` | Init tracing, return `Ok(())` |

**End state:** `cargo build` succeeds. Empty binary runs and exits.

### Phase 1 — Preserve Pure Logic (0.5 day)

Re-verify `src/core/` and `src/providers/` compile after dependency rewrite. Fix any stray Tauri imports. Run existing tests.

**End state:** `cargo test` passes for core + providers.

### Phase 2 — Platform Foundation (2.5 days)

1. **Day 1 AM:** `capture.rs`, `replace.rs`, `context.rs` — port, strip Tauri refs
2. **Day 1 PM:** `hotkey.rs` — rewrite using `global-hotkey` crate
3. **Day 2 AM:** `uia.rs` — `IUIAutomation` wrapper
4. **Day 2 PM:** rewrite `capture.rs` for UIA-first + clipboard fallback
5. **Day 3 AM:** `tray.rs` — `tray-icon` crate wrapper
6. **Day 3 PM:** `mica.rs`, `dwm_shadow.rs` — DWM attributes
7. **Day 3 late:** `caret.rs` — WinEvent hook thread + message pump

**End state:** Tier 3 smoke tests pass against notepad.exe.

### Phase 3 — State + Engine Refactor (3 days)

1. **Day 1:** Create `state/app_state.rs` + `state/events.rs` — AppState, UiEvent, UiCommand
2. **Day 1-2:** Add `platform/traits.rs` — `TextCapture`, `TextReplace`, `ContextProbe`
3. **Day 2:** Refactor `engine/engine.rs` — strip all Tauri, replace emit sites with channel sends, accept trait objects
4. **Day 2-3:** Split into `engine.rs`, `streaming.rs`, `hotkey_flow.rs`, `compare.rs`
5. **Day 3:** Write Tier 2 integration tests with fakes

**End state:** Engine compiles, passes integration tests, zero Tauri dependencies.

### Phase 4 — Slint Main Window Skeleton (3 days)

1. **Day 1:** `main_window.slint` with `states [compact, expanded]` and AppBridge bindings
2. **Day 1-2:** Shared components — `plume.slint`, `lang_row.slint`, `mode_row.slint`, `chain_row.slint`, `toast.slint`
3. **Day 2:** `compact.slint` — full compact layout
4. **Day 2-3:** `ui/bridge.rs` — AppBridge property sync, callback registration, event pump
5. **Day 3:** `ui/main_window.rs` — window construction, Mica apply, DWM shadow, icon embedding
6. **Day 3:** Rewrite `main.rs` — full boot (config, state, channels, spawn engine, build window, register tray + hotkey, run Slint event loop)

**End state:** `cargo run` launches Quill. MainWindow visible in Mica Fluent.

### Phase 5 — Wire Core Flow (2 days)

- Hotkey → `handle_hotkey` → capture → emit `ShowOverlay` → compact view renders
- Mode click → `ExecuteMode` → stream chunks update the property → user sees live tokens
- Replace button → `ConfirmReplace` → `paste_text` → success toast → dismiss
- ESC → `Dismiss` → cancel in-flight stream → hide
- Retry, Cancel, Undo wired

**End state:** **FIRST USABLE MILESTONE.** Minimal Quill works end-to-end.

### Phase 6 — Expanded View + Tabs (5 days)

1. **Day 1:** Morph animation, `ExpandedView`, tab bar, `WriteTab`
2. **Day 2:** `HistoryTab` — list, search, favorites, detail pane, export
3. **Day 3:** `TutorTab` — explanation + lessons
4. **Day 4:** `CompareTab` — two-column streaming comparison
5. **Day 5:** `SettingsTab` — provider, API key, hotkey, modes, chains, templates, persona, first-run wizard

**End state:** Full feature parity except floating pencil.

### Phase 7 — Floating Pencil (3 days)

1. **Day 1:** `pencil.slint` + `pencil_window.rs` — tiny 32×32 window
2. **Day 2:** Wire `caret::FocusEvent` stream to pencil controller (show/hide/reposition)
3. **Day 2:** Click handler → same code path as hotkey
4. **Day 3:** Multi-monitor DPI + edge cases (elevated windows, UWP, Chromium)

**End state:** **FULL SCOPE DONE.**

### Phase 8 — Polish, Tests, Ship (3 days)

- Run all tests in CI; fix red
- Walk Tier 5 manual checklist
- Performance pass: startup < 150 ms cold, first chunk < 50 ms from click, morph < 500 ms
- Finalize `plume.svg` + `quill.ico`
- Update `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `QUILL_PROJECT_BRIEF.md`
- Delete every leftover from Tauri era
- Final clippy + fmt + doc pass

**End state:** **Ready to merge.**

### Summary Timeline

| Phase | Days | Cumulative | Milestone |
|---|---|---|---|
| 0 — Demolition | 0.5 | 0.5 | empty binary |
| 1 — Preserve core | 0.5 | 1.0 | tests green |
| 2 — Platform | 2.5 | 3.5 | smoke tests green |
| 3 — State + engine | 3.0 | 6.5 | integration tests green |
| 4 — Slint skeleton | 3.0 | 9.5 | window visible |
| 5 — Core flow wired | 2.0 | 11.5 | **daily-usable minimal Quill** |
| 6 — Expanded + tabs | 5.0 | 16.5 | **full parity minus pencil** |
| 7 — Pencil | 3.0 | 19.5 | **full scope** |
| 8 — Polish + ship | 3.0 | **22.5 days** | **ready to merge** |

**22.5 working days ≈ 4.5 calendar weeks.**

## 8. Dependencies

### Added

| Crate | Version | Purpose |
|---|---|---|
| `slint` | `1.8` | UI toolkit, features: `compat-1-2`, `backend-winit-skia` |
| `slint-build` | `1.8` | build-time `.slint` compiler (build.rs only) |
| `global-hotkey` | `0.5` | global shortcut registration |
| `tray-icon` | `0.14` | system tray icon + menu |
| `resvg` | `0.42` | build-time SVG → PNG rendering for icon generation (build-dep only) |

### Removed

- `tauri`, `tauri-build`, `tauri-plugin-global-shortcut` — replaced by Slint + global-hotkey + tray-icon
- `arboard` — replaced by UIA-based capture; clipboard fallback uses raw Win32 clipboard APIs (`OpenClipboard`, `GetClipboardData`, `CloseClipboard`)

### Kept

- `tokio` (full features) — runtime for engine tasks, streaming, channels
- `reqwest` (stream, json) — HTTP client for providers
- `futures-util` — stream utilities
- `rusqlite` (bundled) — history DB
- `async-trait` — trait boundaries
- `anyhow` — engine-internal error propagation
- `dirs` — config/log path resolution
- `regex` — prompt/template matching
- `serde`, `serde_json`, `serde_yaml` — config serialization
- `tracing-subscriber` (env-filter) — logging
- `enigo` — simulated Ctrl+V paste (only universally reliable method)

### Expanded Windows Features

```toml
[dependencies.windows]
version = "0.58"
features = [
    "Win32_Foundation",
    "Win32_UI_WindowsAndMessaging",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_Accessibility",        # NEW — IUIAutomation, UIA patterns, TextPattern
    "Win32_System_Com",              # NEW — CoInitializeEx, CoCreateInstance
    "Win32_Graphics_Dwm",            # NEW — DwmSetWindowAttribute, DwmExtendFrameIntoClientArea
    "Win32_Graphics_Gdi",            # NEW — RECT utilities, monitor enumeration
    "Win32_System_Threading",
    "Win32_System_SystemInformation",# NEW — RtlGetVersion for Windows version detection
]
```

No `#[cfg(target_os = "windows")]` gates anywhere — the whole codebase is Windows-only.

## 9. Risks & Open Questions

### Technical Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| **Slint + Mica transparency incompatibility** | Medium | High | Verify early in Phase 4 with a minimal test window. If Slint's Skia backend can't render over a Mica-enabled HWND, fall back to `DWMSBT_TRANSIENTWINDOW` or a solid tinted background. Decision gate at Phase 4 Day 1 end. |
| **UIA coverage gaps in Chromium-based apps** | High | Medium | Accept gracefully — clipboard fallback handles most cases. Pencil window hides when UIA reports no editable element. |
| **Caret-following performance on 4K multi-monitor** | Medium | Medium | Debounce `EVENT_OBJECT_LOCATIONCHANGE` at 30 Hz. Limit UIA calls on focus change, not on every caret move. Profile in Phase 7. |
| **`SetWinEventHook` denied in elevated contexts** | Low | Low | Pencil stays hidden in elevated target apps (can't hook). Hotkey and main window still work. |
| **Destructive migration leaves user without working app during rewrite** | High | Medium | `tauri-final` git tag provides instant rollback. Phase 5 (day ~11.5) restores a working minimal Quill. |
| **Stream idle timeout writing very long design specs** | Observed | Low | Already mitigated — this spec was written in phased Edit calls after skeleton creation. |
| **Slint 1.8 API churn between now and merge** | Low | Low | Pin exact version; upgrade only intentionally after merge. |
| **Plume icon illegible at 16 px tray size** | Low | Low | Badged background gives weight; iterate during Phase 8 polish. |

### Open Questions

None — all decisions resolved during brainstorming.

### Non-Goals (Explicitly Out of Scope)

- **Cross-platform support** — no macOS, no Linux, no ARM64. x86_64 Windows 10/11 only.
- **Light theme** — dark-only. Mica backdrop adapts to user's system color anyway.
- **Custom theming** — brand violet and Mica blue are hardcoded; no theme switcher.
- **Plugin system** — modes and chains are still YAML-configurable, but no dynamic loading.
- **Telemetry / crash reporting service** — local log file only.
- **Auto-update mechanism** — manual download for this rewrite; add later if needed.

### Success Criteria

1. `cargo run` launches the app with a visible Mica-backdropped window in under 150 ms cold start
2. Hotkey → capture → stream first chunk visible in under 50 ms from click
3. All current Tauri-era features functional (verified via Tier 5 checklist)
4. Floating plume indicator appears near the caret in Notepad, Word, Outlook, VSCode; hides in non-editable contexts
5. All Tier 1-4 tests pass in CI
6. Binary size < 25 MB stripped release
7. RAM footprint < 60 MB at idle (vs ~100 MB for the Tauri version)

### Approvals

- **Brainstorming complete:** 2026-04-14
- **Design approved in principle:** 2026-04-14 (pending final written-spec review)
- **Implementation starts:** upon user approval of this document
