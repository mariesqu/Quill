// Per-module `#![allow]` attributes cover the specific helpers kept even
// when unused by the current binary entry point (see state/mod.rs,
// state/app_state.rs, engine/mod.rs). The library root therefore stays
// under the default lint profile so `clippy -D warnings` catches real
// regressions.

pub mod core;
pub mod engine;
pub mod platform;
pub mod providers;
pub mod state;
pub mod ui;
