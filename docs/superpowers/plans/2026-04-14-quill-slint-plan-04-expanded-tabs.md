# Quill Slint Rewrite — Plan 4: Expanded View + Tabs

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the expanded view online with five working tabs — Write, History, Tutor, Compare, Settings — and a functional toggle between compact and expanded states. Each tab reaches "MVP usable" — not polished. Visual polish lands in Plan 6.

**Scope:** All five tabs wired end-to-end to `AppState`, `UiEvent`, `UiCommand`, and the engine. History list renders from SQLite; favorite toggle persists; tutor explains last edit + generates lessons; compare streams two modes and picks a winner; settings saves provider/hotkey/api-key changes to YAML.

**Out of scope (deferred to Plan 6 polish):**
- Morph animation compact ↔ expanded (Plan 4 uses snap resize)
- Diff view in Write tab (Plan 6)
- Template editor in Settings (Plan 4 shows existing templates read-only; Plan 6 edits)
- History export UI with file picker (Plan 4 has a button that triggers `ExportHistory` and shows a toast "Exported to …"; Plan 6 adds the picker)
- Tutor lesson history graphs (Plan 4 shows current lesson text only)

**Architecture:**
- Extend `state/events.rs`: add `UiEvent::HistoryLoaded`, `HistoryEntryUpdated`, `TemplatesUpdated`, `Toast`
- Extend `state/events.rs`: add `UiCommand::LoadHistory`, `ToggleFavorite`, `ExportHistory`, `SaveConfig`, `SaveTemplate`, `DeleteTemplate`, `SwitchTab`, `ToggleView`
- Extend `state/app_state.rs`: add `history_entries`, `templates`, `comparison`, `tutor_explanation`, `current_tab`
- Extend `engine/mod.rs::handle_command`: route new variants to new methods on `Engine`
- Create `src/engine/history_flow.rs` (loads + toggles + exports) and `src/engine/settings_flow.rs` (save_config + save/delete template)
- Extend AppBridge in Slint: add `current-tab`, `history`, `templates`, `last-tutor-explanation`, `last-tutor-lesson`, new callbacks
- Create `src/ui/slint/expanded.slint` + `src/ui/slint/tabs/{write,history,tutor,compare,settings}.slint`
- Update `src/ui/slint/main_window.slint` to render `ExpandedView` when `AppBridge.view-mode == ViewMode.Expanded`
- Update `src/ui/bridge.rs` to handle new events + register new callbacks
- Update `src/main.rs` to resize the window when `view-mode` toggles

**Tech stack:** Same as Plan 3 — Slint 1.15, windows-rs 0.58, tokio, anyhow.

**Preconditions:**
- Branch `claude/slint-rewrite` at tag `plan-03-slint-mvp-complete`
- 63 lib tests + 7 integration tests green
- Compact view fully functional: hotkey → capture → stream → replace
- History SQLite DB at `~/.quill/history.db` with existing entries (or empty — both OK)

**End state:**
- Clicking ⤢ in the compact header toggles to expanded view (840×600)
- Expanded view shows a tab bar; each tab renders its own content
- History tab lists recent entries; click-to-view detail; star toggle persists
- Compare tab runs two modes and shows both results side-by-side
- Settings tab saves provider/api-key/hotkey edits to `~/.quill/user.yaml`
- Clicking ⤢ again (or the — chrome button) returns to compact
- `cargo clippy --all-targets -- -D warnings` clean
- `cargo test --lib` + `cargo test --test engine_integration` all green
- Tagged `plan-04-expanded-tabs-complete`

---

## Design Reference

### New `UiEvent` variants

```rust
pub enum UiEvent {
    // ... existing variants from Plan 2 ...
    HistoryLoaded(Vec<crate::core::history::HistoryEntry>),
    HistoryEntryUpdated { id: i64, favorited: bool },
    TemplatesUpdated(Vec<crate::core::config::Template>),
    Toast { kind: ToastKind, message: String },
}
```

`ToastKind` is already in `state/app_state.rs` — re-export at the `state::` root.

### New `UiCommand` variants

```rust
pub enum UiCommand {
    // ... existing variants from Plan 2 ...
    LoadHistory  { limit: usize, language_filter: Option<String> },
    ToggleFavorite { entry_id: i64 },
    ExportHistory  { format: String }, // "json" | "csv" | "md"
    SaveConfig     { updates: serde_json::Value },
    SaveTemplate   (crate::core::config::Template),
    DeleteTemplate { name: String },
    SwitchTab      { tab: String }, // "write"|"history"|"tutor"|"compare"|"settings"
    ToggleView,
}
```

### Extended `AppState`

```rust
pub struct AppState {
    // ... existing fields from Plan 2 ...
    pub history_entries: Vec<crate::core::history::HistoryEntry>,
    pub templates: Vec<crate::core::config::Template>,
    pub comparison: Option<ComparisonSnapshot>,
    pub tutor_explanation: Option<String>,
    pub tutor_lesson: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ComparisonSnapshot {
    pub mode_a: String,
    pub result_a: String,
    pub mode_b: String,
    pub result_b: String,
}
```

### Extended Slint `AppBridge`

```slint
// New properties
in-out property <string> current-tab: "write";
in-out property <[HistoryEntry]> history;
in-out property <[Template]> templates;
in-out property <string> tutor-explanation;
in-out property <string> tutor-lesson;
in-out property <string> compare-mode-a;
in-out property <string> compare-result-a;
in-out property <string> compare-mode-b;
in-out property <string> compare-result-b;

// New callbacks
callback load-history();
callback toggle-favorite(int);
callback export-history(string);
callback save-config(string /*json*/);
callback save-template(string /*name*/, string /*prompt*/);
callback delete-template(string);
callback switch-tab(string);
callback run-compare(string /*mode_a*/, string /*mode_b*/);
callback request-tutor-explain(int);
callback generate-lesson(string /*daily|weekly*/);
```

### Layout (expanded)

```
ExpandedView (840x600, min 680x480, resizable)
├─ Header (plume, "Quill", tab bar, — □ ✕ controls)
├─ TabBar (Write | History | Tutor | Compare | Settings)
└─ TabContent (stack — only current tab visible)
    ├─ WriteTab    — selected text, stream output, replace/copy/retry/undo
    ├─ HistoryTab  — left: list with search + star; right: detail pane
    ├─ TutorTab    — explanation area + "Daily lesson" / "Weekly lesson" buttons
    ├─ CompareTab  — two mode dropdowns, Compare button, side-by-side result
    └─ SettingsTab — provider select, api-key, hotkey, save
```

---

## Phase 1 — Extend State + Events + Commands (Day 1 AM)

### Task 1: Extend `state/events.rs` with new `UiEvent` + `UiCommand` variants

**Files:**
- Modify: `src/state/events.rs`

- [ ] **Step 1: Add new variants**

Open `src/state/events.rs`. Locate the `UiEvent` enum and append the following variants before the closing `}`:

```rust
    HistoryLoaded(Vec<crate::core::history::HistoryEntry>),
    HistoryEntryUpdated { id: i64, favorited: bool },
    TemplatesUpdated(Vec<crate::core::config::Template>),
    Toast { kind: crate::state::app_state::ToastKind, message: String },
```

Locate the `UiCommand` enum and append:

```rust
    LoadHistory  { limit: usize, language_filter: Option<String> },
    ToggleFavorite { entry_id: i64 },
    ExportHistory  { format: String },
    SaveConfig     { updates: serde_json::Value },
    SaveTemplate   (crate::core::config::Template),
    DeleteTemplate { name: String },
    SwitchTab      { tab: String },
    ToggleView,
```

- [ ] **Step 2: Verify the new variants are Clone**

`HistoryEntry` is `#[derive(Clone)]` already (verified in `src/core/history.rs:65`). `Template` is `#[derive(Clone)]` (verified in `src/core/config.rs:67`). `ToastKind` is `Copy + Clone` from Plan 2.

`serde_json::Value` is `Clone`, so `SaveConfig` works.

If `#[derive(Clone)]` on `UiEvent` / `UiCommand` breaks because of any added field, fix by deriving `Clone` on the offending leaf type.

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: clean. Warnings about unreachable match arms in existing `handle_command` are expected (Phase 2 wires the new arms).

- [ ] **Step 4: Commit**

```bash
git add src/state/events.rs
git commit -m "feat(state): add HistoryLoaded/Toast/LoadHistory/SaveConfig/ToggleView events"
```

### Task 2: Extend `state/app_state.rs` with new fields

**Files:**
- Modify: `src/state/app_state.rs`

- [ ] **Step 1: Add the `ComparisonSnapshot` struct**

At the top of the module (after the existing `ChainProgress` struct), add:

```rust
#[derive(Debug, Clone, Default)]
pub struct ComparisonSnapshot {
    pub mode_a: String,
    pub result_a: String,
    pub mode_b: String,
    pub result_b: String,
}
```

- [ ] **Step 2: Extend `AppState` with new fields**

Add the following fields to the `AppState` struct (grouped with the other nested state):

```rust
    // Plan 4 — expanded view state
    pub history_entries: Vec<crate::core::history::HistoryEntry>,
    pub templates: Vec<crate::core::config::Template>,
    pub comparison: Option<ComparisonSnapshot>,
    pub tutor_explanation: Option<String>,
    pub tutor_lesson: Option<String>,
    pub current_tab: String,  // "write"|"history"|"tutor"|"compare"|"settings"
```

`current_tab` is a `String` (not an enum) to keep the bridge simple — the UI switches based on a string match. If preferred, use a `TabId` enum later as a polish step.

- [ ] **Step 3: Update `impl Default for AppState`**

The struct already derives `Default` — nothing to do, since `Vec`, `Option`, and `String` all default cleanly. `current_tab` defaults to an empty string; update the `new()` constructor (or add a trivial `Default` impl override) so it initializes to `"write"`:

```rust
impl AppState {
    pub fn new() -> Self {
        Self {
            current_tab: "write".into(),
            ..Default::default()
        }
    }
}
```

- [ ] **Step 4: Extend `reset_session` to NOT clear the persistent fields**

In the existing `reset_session` method, ensure the new Plan 4 fields are NOT cleared (they're persistent across hotkey captures). Only clear what's already cleared today — selected_text, last_result, stream_buffer, etc. `history_entries`/`templates`/`current_tab`/`tutor_*` stay intact.

- [ ] **Step 5: Add a unit test for the new defaults**

Inside `#[cfg(test)] mod tests { ... }`, add:

```rust
    #[test]
    fn new_starts_on_write_tab_with_empty_collections() {
        let s = AppState::new();
        assert_eq!(s.current_tab, "write");
        assert!(s.history_entries.is_empty());
        assert!(s.templates.is_empty());
        assert!(s.comparison.is_none());
        assert!(s.tutor_explanation.is_none());
        assert!(s.tutor_lesson.is_none());
    }
```

- [ ] **Step 6: Build + test**

Run: `cargo build && cargo test --lib state`
Expected: all `state::*` tests pass (existing 9 + the new one = 10).

- [ ] **Step 7: Commit**

```bash
git add src/state/app_state.rs
git commit -m "feat(state): AppState fields for history, templates, compare, tutor"
```

---

## Phase 2 — Engine Command Handlers (Day 1 PM)

### Task 3: Add `Engine::handle_command` routing for new variants

**Files:**
- Modify: `src/engine/mod.rs`

- [ ] **Step 1: Extend the `handle_command` match**

Locate the `match cmd { ... }` block in `Engine::handle_command`. Append arms for every new variant:

```rust
        LoadHistory { limit, language_filter } => {
            let language = language_filter.as_deref();
            match crate::core::history::get_recent(limit, language) {
                Ok(entries) => {
                    self.state().lock().unwrap().history_entries = entries.clone();
                    self.emit(crate::state::UiEvent::HistoryLoaded(entries));
                }
                Err(e) => {
                    self.emit(crate::state::UiEvent::Error {
                        message: format!("History load failed: {e}"),
                    });
                }
            }
        }
        ToggleFavorite { entry_id } => {
            match crate::core::history::toggle_favorite(entry_id) {
                Ok(new_state) => {
                    // Mirror into AppState so the Slint list can re-render.
                    let mut s = self.state().lock().unwrap();
                    if let Some(entry) = s.history_entries.iter_mut().find(|e| e.id == entry_id) {
                        entry.favorited = new_state;
                    }
                    drop(s);
                    self.emit(crate::state::UiEvent::HistoryEntryUpdated {
                        id: entry_id,
                        favorited: new_state,
                    });
                }
                Err(e) => {
                    self.emit(crate::state::UiEvent::Error {
                        message: format!("Toggle favorite failed: {e}"),
                    });
                }
            }
        }
        ExportHistory { format } => {
            // Plan 4 MVP: write to a fixed location and emit a Toast. Plan 6
            // adds the file picker.
            let dest = dirs::home_dir()
                .unwrap_or_default()
                .join(".quill")
                .join(format!("history-export.{format}"));
            let result = match format.as_str() {
                "json" => export_history_json(&dest),
                "csv"  => export_history_csv(&dest),
                "md"   => export_history_md(&dest),
                other  => Err(anyhow::anyhow!("unknown export format: {other}")),
            };
            let msg = match result {
                Ok(()) => format!("Exported to {}", dest.display()),
                Err(e) => format!("Export failed: {e}"),
            };
            self.emit(crate::state::UiEvent::Toast {
                kind: crate::state::app_state::ToastKind::Info,
                message: msg,
            });
        }
        SaveConfig { updates } => {
            match crate::core::config::save_user_config(updates) {
                Ok(()) => {
                    self.emit(crate::state::UiEvent::Toast {
                        kind: crate::state::app_state::ToastKind::Success,
                        message: "Settings saved".into(),
                    });
                }
                Err(e) => {
                    self.emit(crate::state::UiEvent::Error {
                        message: format!("Save config failed: {e}"),
                    });
                }
            }
        }
        SaveTemplate(template) => {
            let name = template.name.clone();
            let updates = serde_json::json!({
                "templates": { name.clone(): template },
            });
            match crate::core::config::save_user_config(updates) {
                Ok(()) => self.emit(crate::state::UiEvent::Toast {
                    kind: crate::state::app_state::ToastKind::Success,
                    message: format!("Saved template: {name}"),
                }),
                Err(e) => self.emit(crate::state::UiEvent::Error {
                    message: format!("Save template failed: {e}"),
                }),
            }
        }
        DeleteTemplate { name } => {
            // Delete by writing an explicit null under the template key.
            let updates = serde_json::json!({
                "templates": { name.clone(): serde_json::Value::Null },
            });
            match crate::core::config::save_user_config(updates) {
                Ok(()) => self.emit(crate::state::UiEvent::Toast {
                    kind: crate::state::app_state::ToastKind::Info,
                    message: format!("Deleted template: {name}"),
                }),
                Err(e) => self.emit(crate::state::UiEvent::Error {
                    message: format!("Delete template failed: {e}"),
                }),
            }
        }
        SwitchTab { tab } => {
            self.state().lock().unwrap().current_tab = tab.clone();
            // The bridge picks this up via its own handler so the Slint
            // side re-renders. Plan 4 routes through the engine so future
            // tab-switch side effects (e.g. auto-load history on entering
            // the History tab) can hook here.
            if tab == "history" {
                let engine = self.clone();
                tokio::spawn(async move {
                    engine
                        .handle_command(crate::state::UiCommand::LoadHistory {
                            limit: 100,
                            language_filter: None,
                        })
                        .await;
                });
            }
        }
        ToggleView => {
            let mut s = self.state().lock().unwrap();
            s.view_mode = match s.view_mode {
                crate::state::ViewMode::Compact => crate::state::ViewMode::Expanded,
                crate::state::ViewMode::Expanded => crate::state::ViewMode::Compact,
            };
            // Bridge listens by reading AppState — no UiEvent needed.
        }
```

- [ ] **Step 2: Add the three export helper functions**

At the bottom of `src/engine/mod.rs`, add:

```rust
// ── History export helpers ───────────────────────────────────────────────────

fn export_history_json(dest: &std::path::Path) -> anyhow::Result<()> {
    let entries = crate::core::history::get_all_entries()?;
    let json = serde_json::to_string_pretty(&entries)?;
    std::fs::create_dir_all(dest.parent().unwrap_or(std::path::Path::new(".")))?;
    std::fs::write(dest, json)?;
    Ok(())
}

fn export_history_csv(dest: &std::path::Path) -> anyhow::Result<()> {
    let entries = crate::core::history::get_all_entries()?;
    let mut out = String::from("id,timestamp,mode,language,original,output,favorited\n");
    for e in entries {
        // Very small escape: double any inner quotes, wrap every field in quotes.
        let fields = [
            e.id.to_string(),
            e.timestamp,
            e.mode.unwrap_or_default(),
            e.language.unwrap_or_default(),
            e.original_text,
            e.output_text,
            if e.favorited { "1".into() } else { "0".into() },
        ];
        for (i, f) in fields.iter().enumerate() {
            if i > 0 { out.push(','); }
            out.push('"');
            out.push_str(&f.replace('"', "\"\""));
            out.push('"');
        }
        out.push('\n');
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
```

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: clean. Known snags:
- `Template` field name may be `name` or differ — read `src/core/config.rs:67` first to confirm. Adapt the `SaveTemplate` arm accordingly.
- `save_user_config` takes a `serde_json::Value` — confirm via grep.
- `get_recent(limit, language)` — the second arg type is `Option<&str>` per Plan 2 survey. Match exactly.
- The `HistoryEntry` has a `mode: Option<String>` and `language: Option<String>` per Plan 2 — the CSV exporter uses `.unwrap_or_default()`.
- `serde_json::Value::Null` requires no import since `serde_json` is already used.

- [ ] **Step 4: Commit**

```bash
git add src/engine/mod.rs
git commit -m "feat(engine): route history/settings/tab commands + export helpers"
```

---

## Phase 3 — Slint Extensions: AppBridge + Tab Files (Day 2)

### Task 4: Extend `app_bridge.slint` with Plan 4 surface

**Files:**
- Modify: `src/ui/slint/app_bridge.slint`

- [ ] **Step 1: Add structs**

Inside `src/ui/slint/app_bridge.slint`, BEFORE the `export global AppBridge { ... }` block, add these type declarations:

```slint
export struct HistoryEntry {
    id: int,
    timestamp: string,
    mode: string,
    language: string,
    original: string,
    output: string,
    favorited: bool,
}

export struct Template {
    name: string,
    prompt: string,
}
```

- [ ] **Step 2: Add properties inside `AppBridge`**

Below the existing property block, add:

```slint
    // Plan 4 — expanded view state
    in-out property <string> current-tab: "write";
    in-out property <[HistoryEntry]> history;
    in-out property <[Template]> templates;
    in-out property <string> last-tutor-explanation;
    in-out property <string> last-tutor-lesson;
    in-out property <string> compare-mode-a;
    in-out property <string> compare-result-a;
    in-out property <string> compare-mode-b;
    in-out property <string> compare-result-b;

    // Settings mirror — seeded at startup, edited in-place, flushed via save-config
    in-out property <string> settings-provider;
    in-out property <string> settings-api-key;
    in-out property <string> settings-model;
    in-out property <string> settings-hotkey;
```

- [ ] **Step 3: Add callbacks inside `AppBridge`**

Below the existing callback block, add:

```slint
    // Plan 4 callbacks
    callback switch-tab(string);
    callback load-history();
    callback toggle-favorite(int);
    callback export-history(string);
    callback run-compare(string /*mode_a*/, string /*mode_b*/);
    callback request-tutor-explain(int);
    callback generate-lesson(string /*daily|weekly*/);
    callback save-settings();
    callback save-template(string /*name*/, string /*prompt*/);
    callback delete-template(string);
```

- [ ] **Step 4: Build sanity**

Don't rebuild yet — the new callbacks aren't wired from Rust (Phase 4) and the new properties aren't read by any component (Tasks 5-10). Slint compiler will accept an unused global — it compiles silently.

- [ ] **Step 5: Commit**

```bash
git add src/ui/slint/app_bridge.slint
git commit -m "feat(ui/slint): AppBridge surface for Plan 4 (tabs, history, settings)"
```

### Task 5: Create `expanded.slint` — root with tab bar + stack

**Files:**
- Create: `src/ui/slint/expanded.slint`

- [ ] **Step 1: Write the file**

```slint
import { AppBridge } from "./app_bridge.slint";
import { Plume }    from "./components/plume.slint";
import { Toast }    from "./components/toast.slint";
import { WriteTab }    from "./tabs/write.slint";
import { HistoryTab }  from "./tabs/history.slint";
import { TutorTab }    from "./tabs/tutor.slint";
import { CompareTab }  from "./tabs/compare.slint";
import { SettingsTab } from "./tabs/settings.slint";

component TabButton inherits Rectangle {
    in property <string> label;
    in property <string> tab-id;
    height: 30px;
    min-width: 80px;
    border-radius: 6px;
    background: AppBridge.current-tab == tab-id
        ? #c084fc
        : (touch.has-hover ? #4a4060 : transparent);
    animate background { duration: 120ms; }
    Text {
        text: label;
        color: AppBridge.current-tab == tab-id ? #1a1530 : #e0dbe8;
        horizontal-alignment: center;
        vertical-alignment: center;
        font-size: 12px;
        font-weight: AppBridge.current-tab == tab-id ? 600 : 400;
    }
    touch := TouchArea {
        clicked => { AppBridge.switch-tab(tab-id); }
    }
}

export component ExpandedView inherits Rectangle {
    background: rgba(38, 36, 48, 0.7);
    border-radius: 12px;

    VerticalLayout {
        padding: 16px;
        spacing: 12px;

        // Chrome
        HorizontalLayout {
            height: 28px;
            spacing: 12px;
            alignment: space-between;
            HorizontalLayout {
                spacing: 8px;
                Plume { width: 22px; height: 22px; }
                Text {
                    text: "Quill";
                    color: #e0dbe8;
                    font-size: 14px;
                    font-weight: 600;
                    vertical-alignment: center;
                }
            }
            HorizontalLayout {
                spacing: 6px;
                Rectangle {
                    width: 24px;
                    height: 24px;
                    border-radius: 4px;
                    background: collapse-touch.has-hover ? #4a4060 : transparent;
                    Text {
                        text: "—";
                        color: #e0dbe8;
                        horizontal-alignment: center;
                        vertical-alignment: center;
                    }
                    collapse-touch := TouchArea {
                        clicked => { AppBridge.toggle-view(); }
                    }
                }
                Rectangle {
                    width: 24px;
                    height: 24px;
                    border-radius: 4px;
                    background: close-touch.has-hover ? #803030 : transparent;
                    Text {
                        text: "✕";
                        color: #e0dbe8;
                        horizontal-alignment: center;
                        vertical-alignment: center;
                    }
                    close-touch := TouchArea {
                        clicked => { AppBridge.dismiss(); }
                    }
                }
            }
        }

        // Tab bar
        HorizontalLayout {
            spacing: 6px;
            alignment: start;
            TabButton { label: "Write";    tab-id: "write"; }
            TabButton { label: "History";  tab-id: "history"; }
            TabButton { label: "Tutor";    tab-id: "tutor"; }
            TabButton { label: "Compare";  tab-id: "compare"; }
            TabButton { label: "Settings"; tab-id: "settings"; }
        }

        // Tab content
        if AppBridge.current-tab == "write": WriteTab {}
        if AppBridge.current-tab == "history": HistoryTab {}
        if AppBridge.current-tab == "tutor": TutorTab {}
        if AppBridge.current-tab == "compare": CompareTab {}
        if AppBridge.current-tab == "settings": SettingsTab {}

        // Toast (error or info)
        Toast {}
    }
}
```

- [ ] **Step 2: Commit** (will not compile yet — tab files don't exist; Task 6 creates them as an atomic batch before building)

```bash
git add src/ui/slint/expanded.slint
git commit -m "feat(ui/slint): expanded view chrome + tab bar (no tabs wired yet)"
```

### Task 6: Create all five tab `.slint` files

This task is a single atomic commit — all five files created together so `cargo build` goes from red (Task 5 imports broken) to green in one step.

**Files:**
- Create: `src/ui/slint/tabs/write.slint`
- Create: `src/ui/slint/tabs/history.slint`
- Create: `src/ui/slint/tabs/tutor.slint`
- Create: `src/ui/slint/tabs/compare.slint`
- Create: `src/ui/slint/tabs/settings.slint`

Create the directory first: `mkdir -p src/ui/slint/tabs`

- [ ] **Step 1: `tabs/write.slint`**

```slint
import { AppBridge } from "../app_bridge.slint";
import { LangRow }   from "../components/lang_row.slint";
import { ModeRow }   from "../components/mode_row.slint";
import { ChainRow }  from "../components/chain_row.slint";

export component WriteTab inherits Rectangle {
    VerticalLayout {
        spacing: 10px;

        // Selected text
        Rectangle {
            min-height: 60px;
            background: rgba(20, 18, 30, 0.5);
            border-radius: 6px;
            Text {
                text: AppBridge.selected-text == "" ? "Select text and press the hotkey to begin." : AppBridge.selected-text;
                color: AppBridge.selected-text == "" ? #9b93a8 : #e0dbe8;
                font-size: 12px;
                font-italic: AppBridge.selected-text == "";
                wrap: word-wrap;
                x: 10px;
                width: parent.width - 20px;
            }
        }

        LangRow {}
        ModeRow {}
        ChainRow {}

        // Stream output
        Rectangle {
            background: rgba(20, 18, 30, 0.6);
            border-radius: 6px;
            min-height: 120px;
            VerticalLayout {
                padding: 10px;
                spacing: 8px;
                Text {
                    text: AppBridge.stream-buffer == "" ? "Output will appear here." : AppBridge.stream-buffer;
                    color: AppBridge.stream-buffer == "" ? #9b93a8 : #ffffff;
                    font-size: 12px;
                    wrap: word-wrap;
                }
                if AppBridge.is-done: HorizontalLayout {
                    spacing: 6px;
                    alignment: end;
                    Rectangle {
                        width: 72px;
                        height: 24px;
                        border-radius: 12px;
                        background: replace-touch.has-hover ? #c084fc : #9a6be0;
                        Text {
                            text: "Replace";
                            color: #1a1530;
                            font-size: 11px;
                            font-weight: 600;
                            horizontal-alignment: center;
                            vertical-alignment: center;
                        }
                        replace-touch := TouchArea {
                            clicked => { AppBridge.confirm-replace(); }
                        }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 2: `tabs/history.slint`**

```slint
import { AppBridge, HistoryEntry } from "../app_bridge.slint";

export component HistoryTab inherits Rectangle {
    HorizontalLayout {
        spacing: 10px;

        // Left: list
        Rectangle {
            width: 280px;
            background: rgba(20, 18, 30, 0.5);
            border-radius: 6px;
            VerticalLayout {
                padding: 8px;
                spacing: 6px;
                HorizontalLayout {
                    alignment: space-between;
                    Text {
                        text: "History";
                        color: #e0dbe8;
                        font-size: 12px;
                        font-weight: 600;
                    }
                    Rectangle {
                        width: 60px;
                        height: 20px;
                        border-radius: 10px;
                        background: refresh-touch.has-hover ? #4a4060 : #2a2540;
                        Text {
                            text: "Reload";
                            color: #e0dbe8;
                            font-size: 10px;
                            horizontal-alignment: center;
                            vertical-alignment: center;
                        }
                        refresh-touch := TouchArea {
                            clicked => { AppBridge.load-history(); }
                        }
                    }
                }
                Rectangle {
                    background: transparent;
                    VerticalLayout {
                        spacing: 4px;
                        for entry in AppBridge.history: Rectangle {
                            height: 48px;
                            border-radius: 4px;
                            background: entry-touch.has-hover ? #3a3450 : transparent;
                            VerticalLayout {
                                padding-left: 8px;
                                padding-right: 8px;
                                padding-top: 4px;
                                padding-bottom: 4px;
                                spacing: 2px;
                                HorizontalLayout {
                                    spacing: 4px;
                                    Text {
                                        text: entry.favorited ? "★" : "☆";
                                        color: entry.favorited ? #ffcc66 : #6a6080;
                                        font-size: 11px;
                                    }
                                    Text {
                                        text: entry.mode;
                                        color: #c084fc;
                                        font-size: 10px;
                                    }
                                    Text {
                                        text: entry.timestamp;
                                        color: #9b93a8;
                                        font-size: 9px;
                                    }
                                }
                                Text {
                                    text: entry.output;
                                    color: #e0dbe8;
                                    font-size: 10px;
                                    overflow: elide;
                                }
                            }
                            entry-touch := TouchArea {
                                clicked => { AppBridge.toggle-favorite(entry.id); }
                            }
                        }
                    }
                }
            }
        }

        // Right: detail + export button
        Rectangle {
            background: rgba(20, 18, 30, 0.5);
            border-radius: 6px;
            VerticalLayout {
                padding: 10px;
                spacing: 8px;
                Text {
                    text: "Click ★/☆ to toggle favorite. Plan 6 adds detail view + search.";
                    color: #9b93a8;
                    font-size: 11px;
                    wrap: word-wrap;
                }
                HorizontalLayout {
                    spacing: 6px;
                    Rectangle {
                        width: 72px;
                        height: 24px;
                        border-radius: 12px;
                        background: json-touch.has-hover ? #4a4060 : #2a2540;
                        Text { text: "Export JSON"; color: #e0dbe8; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; }
                        json-touch := TouchArea {
                            clicked => { AppBridge.export-history("json"); }
                        }
                    }
                    Rectangle {
                        width: 60px;
                        height: 24px;
                        border-radius: 12px;
                        background: csv-touch.has-hover ? #4a4060 : #2a2540;
                        Text { text: "CSV"; color: #e0dbe8; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; }
                        csv-touch := TouchArea {
                            clicked => { AppBridge.export-history("csv"); }
                        }
                    }
                    Rectangle {
                        width: 60px;
                        height: 24px;
                        border-radius: 12px;
                        background: md-touch.has-hover ? #4a4060 : #2a2540;
                        Text { text: "Markdown"; color: #e0dbe8; font-size: 10px; horizontal-alignment: center; vertical-alignment: center; }
                        md-touch := TouchArea {
                            clicked => { AppBridge.export-history("md"); }
                        }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 3: `tabs/tutor.slint`**

```slint
import { AppBridge } from "../app_bridge.slint";

export component TutorTab inherits Rectangle {
    VerticalLayout {
        spacing: 10px;

        // Explanation pane
        Rectangle {
            background: rgba(20, 18, 30, 0.5);
            border-radius: 6px;
            min-height: 160px;
            VerticalLayout {
                padding: 10px;
                spacing: 8px;
                Text {
                    text: "Last edit explanation";
                    color: #c084fc;
                    font-size: 11px;
                    font-weight: 600;
                }
                Text {
                    text: AppBridge.last-tutor-explanation == ""
                        ? "No explanation yet. Trigger one with the button below after a recent edit."
                        : AppBridge.last-tutor-explanation;
                    color: #e0dbe8;
                    font-size: 11px;
                    wrap: word-wrap;
                }
                Rectangle {
                    width: 120px;
                    height: 26px;
                    border-radius: 13px;
                    background: explain-touch.has-hover ? #c084fc : #9a6be0;
                    Text {
                        text: "Explain last edit";
                        color: #1a1530;
                        font-size: 10px;
                        font-weight: 600;
                        horizontal-alignment: center;
                        vertical-alignment: center;
                    }
                    explain-touch := TouchArea {
                        clicked => { AppBridge.request-tutor-explain(0); }
                    }
                }
            }
        }

        // Lesson pane
        Rectangle {
            background: rgba(20, 18, 30, 0.5);
            border-radius: 6px;
            min-height: 160px;
            VerticalLayout {
                padding: 10px;
                spacing: 8px;
                Text {
                    text: "Lesson";
                    color: #c084fc;
                    font-size: 11px;
                    font-weight: 600;
                }
                Text {
                    text: AppBridge.last-tutor-lesson == ""
                        ? "Generate a daily or weekly lesson from your recent edits."
                        : AppBridge.last-tutor-lesson;
                    color: #e0dbe8;
                    font-size: 11px;
                    wrap: word-wrap;
                }
                HorizontalLayout {
                    spacing: 6px;
                    Rectangle {
                        width: 80px;
                        height: 26px;
                        border-radius: 13px;
                        background: daily-touch.has-hover ? #c084fc : #9a6be0;
                        Text {
                            text: "Daily";
                            color: #1a1530;
                            font-size: 11px;
                            font-weight: 600;
                            horizontal-alignment: center;
                            vertical-alignment: center;
                        }
                        daily-touch := TouchArea {
                            clicked => { AppBridge.generate-lesson("daily"); }
                        }
                    }
                    Rectangle {
                        width: 80px;
                        height: 26px;
                        border-radius: 13px;
                        background: weekly-touch.has-hover ? #c084fc : #9a6be0;
                        Text {
                            text: "Weekly";
                            color: #1a1530;
                            font-size: 11px;
                            font-weight: 600;
                            horizontal-alignment: center;
                            vertical-alignment: center;
                        }
                        weekly-touch := TouchArea {
                            clicked => { AppBridge.generate-lesson("weekly"); }
                        }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 4: `tabs/compare.slint`**

```slint
import { AppBridge } from "../app_bridge.slint";

export component CompareTab inherits Rectangle {
    VerticalLayout {
        spacing: 10px;

        // Two mode input fields (simple — Plan 6 replaces with dropdowns)
        HorizontalLayout {
            spacing: 10px;
            Rectangle {
                background: rgba(20, 18, 30, 0.5);
                border-radius: 6px;
                height: 36px;
                HorizontalLayout {
                    padding: 8px;
                    spacing: 6px;
                    Text {
                        text: "A:";
                        color: #c084fc;
                        vertical-alignment: center;
                    }
                    mode-a := TextInput {
                        text <=> AppBridge.compare-mode-a;
                        color: #e0dbe8;
                        font-size: 12px;
                        placeholder-text: "rewrite";
                        horizontal-alignment: left;
                        vertical-alignment: center;
                    }
                }
            }
            Rectangle {
                background: rgba(20, 18, 30, 0.5);
                border-radius: 6px;
                height: 36px;
                HorizontalLayout {
                    padding: 8px;
                    spacing: 6px;
                    Text {
                        text: "B:";
                        color: #c084fc;
                        vertical-alignment: center;
                    }
                    mode-b := TextInput {
                        text <=> AppBridge.compare-mode-b;
                        color: #e0dbe8;
                        font-size: 12px;
                        placeholder-text: "translate";
                        horizontal-alignment: left;
                        vertical-alignment: center;
                    }
                }
            }
            Rectangle {
                width: 80px;
                height: 36px;
                border-radius: 18px;
                background: run-touch.has-hover ? #c084fc : #9a6be0;
                Text {
                    text: "Compare";
                    color: #1a1530;
                    font-size: 11px;
                    font-weight: 600;
                    horizontal-alignment: center;
                    vertical-alignment: center;
                }
                run-touch := TouchArea {
                    clicked => { AppBridge.run-compare(AppBridge.compare-mode-a, AppBridge.compare-mode-b); }
                }
            }
        }

        // Side-by-side results
        HorizontalLayout {
            spacing: 10px;
            Rectangle {
                background: rgba(20, 18, 30, 0.6);
                border-radius: 6px;
                min-height: 220px;
                VerticalLayout {
                    padding: 10px;
                    spacing: 6px;
                    Text { text: "Result A"; color: #c084fc; font-size: 10px; font-weight: 600; }
                    Text {
                        text: AppBridge.compare-result-a;
                        color: #ffffff;
                        font-size: 11px;
                        wrap: word-wrap;
                    }
                }
            }
            Rectangle {
                background: rgba(20, 18, 30, 0.6);
                border-radius: 6px;
                min-height: 220px;
                VerticalLayout {
                    padding: 10px;
                    spacing: 6px;
                    Text { text: "Result B"; color: #c084fc; font-size: 10px; font-weight: 600; }
                    Text {
                        text: AppBridge.compare-result-b;
                        color: #ffffff;
                        font-size: 11px;
                        wrap: word-wrap;
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 5: `tabs/settings.slint`**

```slint
import { AppBridge } from "../app_bridge.slint";

component SettingField inherits Rectangle {
    in property <string> label;
    in-out property <string> value;
    in property <bool> is-password;
    height: 44px;
    background: rgba(20, 18, 30, 0.5);
    border-radius: 6px;
    VerticalLayout {
        padding: 6px;
        spacing: 2px;
        Text {
            text: label;
            color: #9b93a8;
            font-size: 9px;
        }
        TextInput {
            text <=> value;
            color: #e0dbe8;
            font-size: 12px;
            horizontal-alignment: left;
        }
    }
}

export component SettingsTab inherits Rectangle {
    VerticalLayout {
        spacing: 8px;

        SettingField {
            label: "Provider (openrouter | openai | ollama | generic)";
            value <=> AppBridge.settings-provider;
        }
        SettingField {
            label: "API key";
            value <=> AppBridge.settings-api-key;
            is-password: true;
        }
        SettingField {
            label: "Model";
            value <=> AppBridge.settings-model;
        }
        SettingField {
            label: "Hotkey (e.g. Ctrl+Shift+Space)";
            value <=> AppBridge.settings-hotkey;
        }

        Rectangle {
            width: 100px;
            height: 28px;
            border-radius: 14px;
            background: save-touch.has-hover ? #c084fc : #9a6be0;
            Text {
                text: "Save";
                color: #1a1530;
                font-size: 12px;
                font-weight: 600;
                horizontal-alignment: center;
                vertical-alignment: center;
            }
            save-touch := TouchArea {
                clicked => { AppBridge.save-settings(); }
            }
        }

        Text {
            text: "Plan 4 MVP — template editor + persona tuning land in Plan 6.";
            color: #9b93a8;
            font-size: 10px;
        }
    }
}
```

- [ ] **Step 6: Build**

Run: `cargo build`
Expected: Slint compiler parses all five tab files AND `expanded.slint`. First real compile of the whole expanded graph. Likely snags:
- `TextInput` `placeholder-text` may not be a valid property in Slint 1.15 — remove it if compiler complains.
- `font-italic` accepts only a boolean literal in some versions — if `font-italic: AppBridge.selected-text == ""` fails, convert to `font-italic: AppBridge.selected-text == "" ? true : false` or drop the condition.
- `text <=> AppBridge.xxx` two-way binding on a global may need a local intermediate property — if Slint complains about writing to a global from inside `TextInput`, hoist the binding into a local `in-out property <string>` and sync manually.

Fix each Slint compiler error until the build is green. Do NOT give up and delete a tab — the plan expects all five tabs to compile.

- [ ] **Step 7: Commit**

```bash
git add src/ui/slint/tabs/
git commit -m "feat(ui/slint): Write, History, Tutor, Compare, Settings tab files"
```

---

## Phase 4 — Wire Expanded into MainWindow + Rust Bridge (Day 3)

### Task 7: Update `main_window.slint` to render `ExpandedView`

**Files:**
- Modify: `src/ui/slint/main_window.slint`

- [ ] **Step 1: Add the import**

At the top, alongside `import { CompactView } from "./compact.slint";`, add:

```slint
import { ExpandedView } from "./expanded.slint";
```

- [ ] **Step 2: Replace the expanded-view placeholder**

Find the block:

```slint
    if AppBridge.view-mode == ViewMode.expanded: Rectangle {
        background: rgba(38, 36, 48, 0.7);
        border-radius: 12px;
        width: parent.width;
        height: parent.height;
        Text {
            text: "Expanded view — Plan 4";
            ...
        }
    }
```

Replace with:

```slint
    if AppBridge.view-mode == ViewMode.expanded: ExpandedView {
        width: parent.width;
        height: parent.height;
    }
```

- [ ] **Step 3: Make the window resize on view-mode change**

At the top of the `MainWindow` component, above the `if` blocks, change the `width` and `height` declarations from the fixed Plan 3 values to:

```slint
    width: AppBridge.view-mode == ViewMode.compact ? 380px : 840px;
    height: AppBridge.view-mode == ViewMode.compact ? 360px : 600px;
    min-width: 380px;
    min-height: 360px;
```

Slint 1.15 honors property-based sizing; the window will resize on the next tick after `view-mode` flips. No Rust timer needed.

- [ ] **Step 4: Build**

Run: `cargo build`
Expected: clean. Known snag: if Slint complains that `min-width`/`min-height` are not supported on `Window`, remove those two lines.

- [ ] **Step 5: Commit**

```bash
git add src/ui/slint/main_window.slint
git commit -m "feat(ui/slint): main_window renders ExpandedView + resizes on view-mode"
```

### Task 8: Extend `src/ui/bridge.rs` — handle new events + callbacks

**Files:**
- Modify: `src/ui/bridge.rs`

- [ ] **Step 1: Extend `apply_event_on_ui_thread` with new `UiEvent` arms**

Inside the `match event { ... }` block, add:

```rust
        UiEvent::HistoryLoaded(entries) => {
            let models: Vec<crate::ui::HistoryEntry> = entries
                .into_iter()
                .map(|e| crate::ui::HistoryEntry {
                    id: e.id as i32,
                    timestamp: e.timestamp.into(),
                    mode: e.mode.unwrap_or_default().into(),
                    language: e.language.unwrap_or_default().into(),
                    original: e.original_text.into(),
                    output: e.output_text.into(),
                    favorited: e.favorited,
                })
                .collect();
            bridge.set_history(slint::ModelRc::new(slint::VecModel::from(models)));
        }
        UiEvent::HistoryEntryUpdated { id, favorited } => {
            // Rebuild the model in place with the updated favorite flag.
            let model = bridge.get_history();
            if let Some(vec_model) = model
                .as_any()
                .downcast_ref::<slint::VecModel<crate::ui::HistoryEntry>>()
            {
                for i in 0..vec_model.row_count() {
                    if let Some(entry) = vec_model.row_data(i) {
                        if entry.id == id as i32 {
                            let mut updated = entry.clone();
                            updated.favorited = favorited;
                            vec_model.set_row_data(i, updated);
                        }
                    }
                }
            }
        }
        UiEvent::TemplatesUpdated(templates) => {
            let models: Vec<crate::ui::Template> = templates
                .into_iter()
                .map(|t| crate::ui::Template {
                    name: t.name.into(),
                    prompt: t.prompt.into(),
                })
                .collect();
            bridge.set_templates(slint::ModelRc::new(slint::VecModel::from(models)));
        }
        UiEvent::Toast { kind: _, message } => {
            // Plan 4 MVP: surface toasts through the same error-message property
            // so the single Toast component displays either info or error. Plan 6
            // adds a proper toast widget with kind-based styling.
            bridge.set_error_message(message.into());
        }
        UiEvent::TutorExplanation { entry_id: _, text } => {
            bridge.set_last_tutor_explanation(text.into());
        }
        UiEvent::TutorLesson { period: _, text } => {
            bridge.set_last_tutor_lesson(text.into());
        }
        UiEvent::ComparisonResult { mode_a, result_a, mode_b, result_b } => {
            bridge.set_compare_mode_a(mode_a.into());
            bridge.set_compare_result_a(result_a.into());
            bridge.set_compare_mode_b(mode_b.into());
            bridge.set_compare_result_b(result_b.into());
        }
```

The existing `ComparisonResult` / `TutorExplanation` / `TutorLesson` arms (if they use `// Plan 4 wires these up`) should be REPLACED with the real implementations above.

- [ ] **Step 2: Extend `install_command_forwarder` with new callbacks**

Inside `install_command_forwarder`, add (alongside the existing `on_*` registrations):

```rust
    let tx_lh = tx.clone();
    bridge.on_load_history(move || {
        let _ = tx_lh.send(UiCommand::LoadHistory {
            limit: 100,
            language_filter: None,
        });
    });

    let tx_tf = tx.clone();
    bridge.on_toggle_favorite(move |id| {
        let _ = tx_tf.send(UiCommand::ToggleFavorite { entry_id: id as i64 });
    });

    let tx_eh = tx.clone();
    bridge.on_export_history(move |format| {
        let _ = tx_eh.send(UiCommand::ExportHistory {
            format: format.to_string(),
        });
    });

    let tx_st = tx.clone();
    bridge.on_switch_tab(move |tab| {
        let _ = tx_st.send(UiCommand::SwitchTab { tab: tab.to_string() });
    });

    let tx_rc = tx.clone();
    bridge.on_run_compare(move |a, b| {
        let _ = tx_rc.send(UiCommand::CompareModes {
            mode_a: a.to_string(),
            mode_b: b.to_string(),
            language: "auto".to_string(),
            extra: None,
        });
    });

    let tx_rte = tx.clone();
    bridge.on_request_tutor_explain(move |eid| {
        let _ = tx_rte.send(UiCommand::RequestTutorExplain { entry_id: eid as i64 });
    });

    let tx_gl = tx.clone();
    bridge.on_generate_lesson(move |period| {
        let _ = tx_gl.send(UiCommand::GenerateLesson { period: period.to_string() });
    });

    let window_weak = window.as_weak();
    let tx_ss = tx.clone();
    bridge.on_save_settings(move || {
        let Some(w) = window_weak.upgrade() else { return; };
        let b = w.global::<AppBridge>();
        let updates = serde_json::json!({
            "provider": b.get_settings_provider().as_str(),
            "api_key":  b.get_settings_api_key().as_str(),
            "model":    b.get_settings_model().as_str(),
            "hotkey":   b.get_settings_hotkey().as_str(),
        });
        let _ = tx_ss.send(UiCommand::SaveConfig { updates });
    });

    // save-template / delete-template — wired for later UI in Plan 6
    let tx_svt = tx.clone();
    bridge.on_save_template(move |name, prompt| {
        let _ = tx_svt.send(UiCommand::SaveTemplate(crate::core::config::Template {
            name: name.to_string(),
            prompt: prompt.to_string(),
        }));
    });

    let tx_dt = tx.clone();
    bridge.on_delete_template(move |name| {
        let _ = tx_dt.send(UiCommand::DeleteTemplate { name: name.to_string() });
    });
```

- [ ] **Step 3: Update `on_toggle_view` to route through the engine**

The Plan 3 `on_toggle_view` mutated the Slint property directly. Plan 4 routes through the engine so `AppState.view_mode` stays authoritative. Replace the existing `on_toggle_view` block with:

```rust
    let tx_tv = tx.clone();
    bridge.on_toggle_view(move || {
        let _ = tx_tv.send(UiCommand::ToggleView);
    });
```

The engine's `ToggleView` handler flips `AppState.view_mode`, but the Slint `AppBridge.view-mode` also needs to be updated for the UI to re-render. Add a new `UiEvent::ViewModeChanged(ViewMode)` emission inside the engine's `ToggleView` arm OR piggyback on an existing event — but the simpler path is to update the Slint property directly from this callback AFTER sending the command. Replace the above with:

```rust
    let tx_tv = tx.clone();
    let window_weak = window.as_weak();
    bridge.on_toggle_view(move || {
        // Route through the engine so AppState stays authoritative…
        let _ = tx_tv.send(UiCommand::ToggleView);
        // …and also update the Slint property so the UI re-renders immediately.
        if let Some(w) = window_weak.upgrade() {
            let b = w.global::<AppBridge>();
            let next = match b.get_view_mode() {
                crate::ui::ViewMode::Compact => crate::ui::ViewMode::Expanded,
                crate::ui::ViewMode::Expanded => crate::ui::ViewMode::Compact,
            };
            b.set_view_mode(next);
        }
    });
```

- [ ] **Step 4: Seed settings fields at startup**

Inside `seed_bridge`, after setting modes/chains, seed the settings-mirror properties from the config:

```rust
    bridge.set_settings_provider(cfg.provider.clone().into());
    bridge.set_settings_api_key(cfg.api_key.clone().unwrap_or_default().into());
    bridge.set_settings_model(cfg.model.clone().unwrap_or_default().into());
    bridge.set_settings_hotkey(cfg.hotkey.clone().unwrap_or_default().into());
```

This requires `seed_bridge` to accept a `&Config` parameter. Update the signature:

```rust
pub fn seed_bridge(
    window: &MainWindow,
    config: &crate::core::config::Config,
    modes: &std::collections::HashMap<String, crate::core::modes::ModeConfig>,
    chains: &std::collections::HashMap<String, crate::core::modes::ChainConfig>,
)
```

And the `main.rs` call site becomes `bridge::seed_bridge(&window, &config, &modes, &chains);` — Task 9 handles this.

NOTE: Config field names in `src/core/config.rs` may differ from the plan's assumptions (`cfg.api_key`, `cfg.model`, `cfg.hotkey`). Read the actual struct and adapt. Use `.unwrap_or_default()` where fields are `Option<String>`.

- [ ] **Step 5: Build**

Run: `cargo build`
Expected: clean. Known snags:
- `slint::VecModel::set_row_data(i, entry)` — double-check method name; may be `row_data_tracked(i)` or `replace_row_data(i, entry)`. Adapt to the actual API.
- `Template` field names — verify in `src/core/config.rs:67`. If the Rust struct has extra fields beyond `name` and `prompt`, set them to defaults in the `SaveTemplate` arm.

- [ ] **Step 6: Commit**

```bash
git add src/ui/bridge.rs
git commit -m "feat(ui): bridge handles Plan 4 events and forwards new callbacks"
```

### Task 9: Wire `main.rs` — seed settings + view-mode toggle resize

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Update the `seed_bridge` call**

Find `bridge::seed_bridge(&window, &modes, &chains);` and change to:

```rust
    bridge::seed_bridge(&window, &config, &modes, &chains);
```

- [ ] **Step 2: Kick off a history load on startup (optional but pleasant)**

After `bridge::spawn_event_pump(&window, event_rx);`, add:

```rust
    // Pre-load history so the History tab is populated on first open.
    {
        let engine = engine.clone();
        rt_handle.spawn(async move {
            engine
                .handle_command(crate::state::UiCommand::LoadHistory {
                    limit: 100,
                    language_filter: None,
                })
                .await;
        });
    }
```

- [ ] **Step 3: Build + smoke test**

Run: `cargo build`
Expected: clean.

Run: `cargo run 2>&1 | head -30` (let it run for a few seconds then kill)
Expected:
- Window appears with compact view
- Tracing logs show history load event
- Clicking the ⤢ button morphs to expanded (snap resize, no animation — that's Plan 6)
- All 5 tab buttons render
- Clicking History shows the list (or "Reload" button if empty)
- Clicking Settings shows provider/api-key/model/hotkey fields populated from `cfg`

You will NOT be able to easily test the click flow in a headless subagent env. If smoke shows the window enters the event loop and clicks propagate to the log, that's enough — full manual verification is left to the user in the final report.

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat(main): seed settings + preload history on startup"
```

---

## Phase 5 — Verification + Tag (Day 3 late)

### Task 10: Full sweep + `plan-04-expanded-tabs-complete` tag

- [ ] **Step 1: Format + clippy + tests**

Run each command and verify pass:

```bash
cargo fmt
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --lib
cargo test --test engine_integration
cargo build --release
```

Fix any new clippy lints or test failures inline. Existing `#![allow(dead_code)]` shields on the 14 files from Plan 3 may now be removed on files that Plan 4 fully wires (e.g. history_flow helpers). Try removing each one; if >2 new warnings appear, put it back.

- [ ] **Step 2: Sanity grep**

```bash
grep -rn "todo!\|unimplemented!" src/ui/ src/engine/ src/state/
grep -rn "noop_emit\|SharedEngine\|tauri::" src/ tests/
```

Expected: empty. `unreachable!()` inside a closed-set match is fine.

- [ ] **Step 3: Tag**

```bash
git tag plan-04-expanded-tabs-complete
```

Verify: `git tag -l plan-04*` should echo the tag.

- [ ] **Step 4: Engram save**

Call `mcp__plugin_engram_engram__mem_save` with:
- project: `Quill`
- scope: `project`
- topic_key: `quill/slint-rewrite/plan-04`
- type: `architecture`
- title: `Plan 4 (Expanded view + tabs) complete`
- content: Summarize the shape — `state/events.rs` extended with history/template/settings variants; `engine/mod.rs::handle_command` routes new commands + history export helpers; `app_bridge.slint` extended with tab/history/settings surface; five tab `.slint` files under `src/ui/slint/tabs/`; `src/ui/bridge.rs` handles new events and registers new callbacks; `main_window.slint` resizes based on `view-mode`; `main.rs` seeds settings and preloads history at boot. Note remaining work for Plan 5 (floating pencil) and Plan 6 (polish — morph animation, template editor, file picker for export, diff view, visual refinement).

- [ ] **Step 5: Manual verification checklist (report to the user, not committed)**

```
1. `cargo run` launches the compact window.
2. ⤢ button → window resizes to 840×600 and shows the tab bar.
3. Write tab shows the selected-text preview + mode/chain/lang rows.
4. History tab → Reload button triggers LoadHistory; list populates (or stays empty if DB has no entries).
5. Click a history entry's ★/☆ → favorite persists across restart (verify by quitting and `cargo run` again).
6. Export JSON → toast "Exported to C:\Users\<you>\.quill\history-export.json" and the file exists on disk.
7. Tutor tab → Daily lesson triggers generate_lesson; lesson text appears after stream completes (requires a working AI provider).
8. Compare tab → fill both mode fields, click Compare, two results stream in.
9. Settings tab → change provider/api-key/hotkey, click Save → toast "Settings saved" → restart and verify they stick.
10. — button returns to compact view.
11. ESC or ✕ dismisses.
```

---

## Self-Review Notes

**Known drift from the spec (and why):**
- No morph animation; snap resize instead — Plan 6
- Tutor tab's "Explain last edit" button sends `entry_id: 0` which Plan 2's `handle_command(RequestTutorExplain)` handler resolves via `history::get_entry`. If `entry_id == 0` returns no row, the arm silently no-ops — that's acceptable Plan 4 behavior. Plan 6 wires up last-entry auto-tracking.
- History tab has no search/filter UI — Plan 6
- No template editor UI — Plan 6 (save-template / delete-template callbacks ARE wired in the bridge so Plan 6 only has to add the UI)
- No file picker for export — Plan 4 uses a fixed path under `~/.quill/`
- `UiEvent::Toast` and `UiEvent::Error` both surface through the same `AppBridge.error-message` property — Plan 6 adds a proper toast component with kind-based styling
- The `settings-*` property mirror approach is pragmatic but can drift if Rust updates the config without re-seeding. Plan 6 can wire a `ConfigChanged` event to re-seed after every `SaveConfig`.

**Likely snags for the implementer:**
1. Config field names. The plan assumes `config.api_key`, `config.model`, `config.hotkey` — may differ. Read `src/core/config.rs` first and adapt.
2. `slint::VecModel::set_row_data` method name — Slint 1.15 may use `replace_row_data`. Check the compile error and adapt.
3. `Template` struct in Plan 2's `config.rs` may have extra fields beyond `name` + `prompt`. The plan's `SaveTemplate` arm passes the whole struct; the `on_save_template` callback constructs it with only those two fields. If the struct requires more, either derive `Default` or add the missing fields to the callback signature.
4. Slint 1.15's `TextInput.placeholder-text` may not exist — remove if unsupported.
5. `AppBridge.view-mode` enum variants were generated by Slint as `ViewMode::Compact`/`ViewMode::Expanded` (PascalCase, confirmed in Plan 3). The Slint source still references `ViewMode.compact`/`.expanded` (lowercase) because slint identifiers are case-insensitive for enum variants. Both forms work.

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-04-14-quill-slint-plan-04-expanded-tabs.md`.**

Execute subagent-driven, one implementer per phase.
