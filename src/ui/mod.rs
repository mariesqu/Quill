//! Slint UI layer: bridge between `AppState`/`UiEvent`/`UiCommand` and the
//! generated Slint components.
//!
//! The `slint::include_modules!()` macro pulls in the Rust code that
//! `slint-build` emits from `src/ui/slint/*.slint`. Do NOT hand-edit the
//! generated output — change the `.slint` sources instead.
//!
//! Three-tier window model:
//!
//! - `overlay_window` (Tier 1) — ephemeral, hotkey-summoned, near-caret.
//! - `palette_window` (Tier 2) — transient command palette.
//! - `workspace_window` (Tier 3) — persistent tabbed workspace.
//!
//! Plus the pencil floating indicator (`pencil_window`).

slint::include_modules!();

pub mod bridge;
pub mod overlay_window;
pub mod palette_window;
pub mod pencil_controller;
pub mod pencil_window;
pub mod workspace_window;
