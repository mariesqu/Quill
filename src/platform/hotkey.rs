//! Global hotkey registration and event delivery using the `global-hotkey` crate.
//!
//! API:
//! 1. Call `HotkeyService::new()` once at startup
//! 2. Call `service.register("Ctrl+Shift+Space")` with whatever the user configured
//! 3. Drain `GlobalHotKeyEvent::receiver()` on a thread that pumps Windows
//!    messages (Quill pumps them on the Slint main thread — see `main.rs`
//!    `build_hotkey_listener` for why) and forward presses as app-level commands.
//!
//! Two overlay + palette hotkeys are supported by constructing TWO
//! `HotkeyService` instances and matching incoming `event.id` against each
//! service's `current_id()`.

use anyhow::{anyhow, Context, Result};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

pub struct HotkeyService {
    manager: GlobalHotKeyManager,
    current: Option<HotKey>,
}

impl HotkeyService {
    pub fn new() -> Result<Self> {
        let manager = GlobalHotKeyManager::new().context("create GlobalHotKeyManager")?;
        Ok(Self {
            manager,
            current: None,
        })
    }

    /// Register a hotkey from a spec like `"Ctrl+Shift+Space"`.
    /// Unregisters any previously-registered hotkey first.
    pub fn register(&mut self, spec: &str) -> Result<()> {
        self.unregister_current()?;
        let hotkey = parse_hotkey_spec(spec)?;
        self.manager
            .register(hotkey)
            .with_context(|| format!("register hotkey `{spec}`"))?;
        self.current = Some(hotkey);
        Ok(())
    }

    pub fn unregister_current(&mut self) -> Result<()> {
        if let Some(hotkey) = self.current.take() {
            self.manager
                .unregister(hotkey)
                .context("unregister previous hotkey")?;
        }
        Ok(())
    }

    /// The numeric ID of the currently-registered hotkey, matching the
    /// `id` field on incoming `GlobalHotKeyEvent`s. Callers with more than
    /// one `HotkeyService` use this to decide which service the event
    /// belongs to.
    pub fn current_id(&self) -> Option<u32> {
        self.current.as_ref().map(|h| h.id())
    }
}

/// Parse a spec like `"Ctrl+Shift+Space"` into a `HotKey`.
///
/// Accepted modifier tokens (case-insensitive): `ctrl`, `control`, `shift`,
/// `alt`, `meta`, `super`, `win`, `windows`.
/// Accepted key tokens: `Space`, `Enter/Return`, `Tab`, `Esc/Escape`,
/// `Backspace`, `Delete/Del`, `Home`, `End`, `PageUp/PageDown`,
/// arrow keys, function keys `F1..F12`, letters `A..Z`, digits `0..9`.
pub fn parse_hotkey_spec(spec: &str) -> Result<HotKey> {
    let mut modifiers = Modifiers::empty();
    let mut key: Option<Code> = None;

    for token in spec.split(['+', '-']).map(str::trim) {
        if token.is_empty() {
            continue;
        }
        match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers.insert(Modifiers::CONTROL),
            "shift" => modifiers.insert(Modifiers::SHIFT),
            "alt" => modifiers.insert(Modifiers::ALT),
            "meta" | "super" | "win" | "windows" => modifiers.insert(Modifiers::META),
            other => {
                key =
                    Some(parse_key_token(other).ok_or_else(|| {
                        anyhow!("unknown key token `{token}` in hotkey `{spec}`")
                    })?);
            }
        }
    }

    let key = key.ok_or_else(|| anyhow!("hotkey `{spec}` has no key — only modifiers"))?;
    Ok(HotKey::new(Some(modifiers), key))
}

fn parse_key_token(token: &str) -> Option<Code> {
    let upper = token.to_ascii_uppercase();
    match upper.as_str() {
        "SPACE" => Some(Code::Space),
        "ENTER" | "RETURN" => Some(Code::Enter),
        "TAB" => Some(Code::Tab),
        "ESC" | "ESCAPE" => Some(Code::Escape),
        "BACKSPACE" => Some(Code::Backspace),
        "DELETE" | "DEL" => Some(Code::Delete),
        "HOME" => Some(Code::Home),
        "END" => Some(Code::End),
        "PAGEUP" => Some(Code::PageUp),
        "PAGEDOWN" => Some(Code::PageDown),
        "UP" => Some(Code::ArrowUp),
        "DOWN" => Some(Code::ArrowDown),
        "LEFT" => Some(Code::ArrowLeft),
        "RIGHT" => Some(Code::ArrowRight),
        s if s.len() == 1 && s.chars().next().unwrap().is_ascii_alphabetic() => {
            match s.chars().next().unwrap() {
                'A' => Some(Code::KeyA),
                'B' => Some(Code::KeyB),
                'C' => Some(Code::KeyC),
                'D' => Some(Code::KeyD),
                'E' => Some(Code::KeyE),
                'F' => Some(Code::KeyF),
                'G' => Some(Code::KeyG),
                'H' => Some(Code::KeyH),
                'I' => Some(Code::KeyI),
                'J' => Some(Code::KeyJ),
                'K' => Some(Code::KeyK),
                'L' => Some(Code::KeyL),
                'M' => Some(Code::KeyM),
                'N' => Some(Code::KeyN),
                'O' => Some(Code::KeyO),
                'P' => Some(Code::KeyP),
                'Q' => Some(Code::KeyQ),
                'R' => Some(Code::KeyR),
                'S' => Some(Code::KeyS),
                'T' => Some(Code::KeyT),
                'U' => Some(Code::KeyU),
                'V' => Some(Code::KeyV),
                'W' => Some(Code::KeyW),
                'X' => Some(Code::KeyX),
                'Y' => Some(Code::KeyY),
                'Z' => Some(Code::KeyZ),
                _ => None,
            }
        }
        s if s.len() == 1 && s.chars().next().unwrap().is_ascii_digit() => {
            match s.chars().next().unwrap() {
                '0' => Some(Code::Digit0),
                '1' => Some(Code::Digit1),
                '2' => Some(Code::Digit2),
                '3' => Some(Code::Digit3),
                '4' => Some(Code::Digit4),
                '5' => Some(Code::Digit5),
                '6' => Some(Code::Digit6),
                '7' => Some(Code::Digit7),
                '8' => Some(Code::Digit8),
                '9' => Some(Code::Digit9),
                _ => None,
            }
        }
        s if s.starts_with('F') && s.len() >= 2 => {
            let n: u8 = s[1..].parse().ok()?;
            match n {
                1 => Some(Code::F1),
                2 => Some(Code::F2),
                3 => Some(Code::F3),
                4 => Some(Code::F4),
                5 => Some(Code::F5),
                6 => Some(Code::F6),
                7 => Some(Code::F7),
                8 => Some(Code::F8),
                9 => Some(Code::F9),
                10 => Some(Code::F10),
                11 => Some(Code::F11),
                12 => Some(Code::F12),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Convenience: returns true if the event is a real press (not a key-up).
pub fn is_pressed(event: &GlobalHotKeyEvent) -> bool {
    matches!(event.state, HotKeyState::Pressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ctrl_shift_space() {
        let h = parse_hotkey_spec("Ctrl+Shift+Space").unwrap();
        assert!(h.matches(Modifiers::CONTROL | Modifiers::SHIFT, Code::Space));
    }

    #[test]
    fn parses_lowercase_alt_f12() {
        let h = parse_hotkey_spec("alt+f12").unwrap();
        assert!(h.matches(Modifiers::ALT, Code::F12));
    }

    #[test]
    fn rejects_spec_with_no_key() {
        assert!(parse_hotkey_spec("Ctrl+Shift").is_err());
    }

    #[test]
    fn rejects_unknown_token() {
        assert!(parse_hotkey_spec("Ctrl+Banana").is_err());
    }
}
