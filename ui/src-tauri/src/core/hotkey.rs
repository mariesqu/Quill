//! Global hotkey parsing and (re-)registration.
//!
//! Supports ASCII letters, digits, F-keys, Space, Enter, Tab, Escape,
//! Backspace, Delete, arrow keys, and Home/End/PageUp/PageDown — plus any
//! combination of Ctrl/Cmd/Shift/Alt modifiers.
//!
//! An empty or unparseable hotkey falls back to the OS default:
//!   - Windows / Linux : `Ctrl + Shift + Space`
//!   - macOS           : `Cmd  + Shift + Space`

use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::engine::{handle_hotkey, SharedEngine};

/// Error returned by [`parse_hotkey`] when the descriptor cannot be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyParseError {
    pub input: String,
    pub reason: String,
}

impl std::fmt::Display for HotkeyParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid hotkey {:?}: {}", self.input, self.reason)
    }
}

fn default_shortcut() -> Shortcut {
    #[cfg(target_os = "macos")]
    let default_modifiers = Modifiers::META | Modifiers::SHIFT;
    #[cfg(not(target_os = "macos"))]
    let default_modifiers = Modifiers::CONTROL | Modifiers::SHIFT;
    Shortcut::new(Some(default_modifiers), Code::Space)
}

/// Parse a hotkey descriptor like `"ctrl+shift+space"` or `"cmd+alt+f1"`.
///
/// Returns `Err` if the descriptor is non-empty but contains an unknown token
/// (e.g. `"ctrl+shft+q"` with a typo on `shift`, or `"ctrl+f99"`). An empty
/// or whitespace-only descriptor — or `None` — resolves to the OS default
/// chord as a successful result.
pub fn parse_hotkey(s: Option<&str>) -> Result<Shortcut, HotkeyParseError> {
    let input = s.unwrap_or("").trim();
    if input.is_empty() {
        return Ok(default_shortcut());
    }

    let lower = input.to_lowercase();
    let parts: Vec<&str> = lower.split('+').map(str::trim).collect();

    let mut modifiers = Modifiers::empty();
    let mut code: Option<Code> = None;

    for part in &parts {
        match *part {
            "" => {
                return Err(HotkeyParseError {
                    input: input.to_string(),
                    reason: "empty segment between '+'".into(),
                });
            }
            "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
            "cmd" | "meta" | "super" | "win" => modifiers |= Modifiers::META,
            "shift" => modifiers |= Modifiers::SHIFT,
            "alt" | "option" => modifiers |= Modifiers::ALT,
            key => match key_name_to_code(key) {
                Some(c) => {
                    if code.is_some() {
                        return Err(HotkeyParseError {
                            input: input.to_string(),
                            reason: format!("more than one key specified (saw '{key}')"),
                        });
                    }
                    code = Some(c);
                }
                None => {
                    return Err(HotkeyParseError {
                        input: input.to_string(),
                        reason: format!("unknown key '{key}'"),
                    });
                }
            },
        }
    }

    let code = code.ok_or_else(|| HotkeyParseError {
        input: input.to_string(),
        reason: "no key specified (only modifiers)".into(),
    })?;

    Ok(Shortcut::new(
        if modifiers.is_empty() {
            None
        } else {
            Some(modifiers)
        },
        code,
    ))
}

/// Map a normalised (lowercased, trimmed) key name to a `Code`.
/// Returns `None` if the name is not recognised.
fn key_name_to_code(key: &str) -> Option<Code> {
    // Single ASCII letter → KeyA..KeyZ
    if key.len() == 1 {
        let c = key.chars().next().unwrap();
        if c.is_ascii_lowercase() {
            return Some(match c {
                'a' => Code::KeyA,
                'b' => Code::KeyB,
                'c' => Code::KeyC,
                'd' => Code::KeyD,
                'e' => Code::KeyE,
                'f' => Code::KeyF,
                'g' => Code::KeyG,
                'h' => Code::KeyH,
                'i' => Code::KeyI,
                'j' => Code::KeyJ,
                'k' => Code::KeyK,
                'l' => Code::KeyL,
                'm' => Code::KeyM,
                'n' => Code::KeyN,
                'o' => Code::KeyO,
                'p' => Code::KeyP,
                'q' => Code::KeyQ,
                'r' => Code::KeyR,
                's' => Code::KeyS,
                't' => Code::KeyT,
                'u' => Code::KeyU,
                'v' => Code::KeyV,
                'w' => Code::KeyW,
                'x' => Code::KeyX,
                'y' => Code::KeyY,
                'z' => Code::KeyZ,
                _ => return None,
            });
        }
        if c.is_ascii_digit() {
            return Some(match c {
                '0' => Code::Digit0,
                '1' => Code::Digit1,
                '2' => Code::Digit2,
                '3' => Code::Digit3,
                '4' => Code::Digit4,
                '5' => Code::Digit5,
                '6' => Code::Digit6,
                '7' => Code::Digit7,
                '8' => Code::Digit8,
                '9' => Code::Digit9,
                _ => return None,
            });
        }
    }

    // F-keys f1..f24
    if let Some(rest) = key.strip_prefix('f') {
        if let Ok(n) = rest.parse::<u8>() {
            return Some(match n {
                1 => Code::F1,
                2 => Code::F2,
                3 => Code::F3,
                4 => Code::F4,
                5 => Code::F5,
                6 => Code::F6,
                7 => Code::F7,
                8 => Code::F8,
                9 => Code::F9,
                10 => Code::F10,
                11 => Code::F11,
                12 => Code::F12,
                13 => Code::F13,
                14 => Code::F14,
                15 => Code::F15,
                16 => Code::F16,
                17 => Code::F17,
                18 => Code::F18,
                19 => Code::F19,
                20 => Code::F20,
                21 => Code::F21,
                22 => Code::F22,
                23 => Code::F23,
                24 => Code::F24,
                _ => return None,
            });
        }
    }

    // Named keys
    Some(match key {
        "space" => Code::Space,
        "enter" | "return" => Code::Enter,
        "tab" => Code::Tab,
        "esc" | "escape" => Code::Escape,
        "backspace" => Code::Backspace,
        "delete" | "del" => Code::Delete,
        "insert" | "ins" => Code::Insert,
        "home" => Code::Home,
        "end" => Code::End,
        "pageup" | "pgup" => Code::PageUp,
        "pagedown" | "pgdn" => Code::PageDown,
        "left" | "arrowleft" => Code::ArrowLeft,
        "right" | "arrowright" => Code::ArrowRight,
        "up" | "arrowup" => Code::ArrowUp,
        "down" | "arrowdown" => Code::ArrowDown,
        _ => return None,
    })
}

/// Register the Quill hotkey with the OS, replacing any previously registered
/// shortcut this app owns. Intended to be called once from `setup()` and again
/// from the `save_config` command whenever the user changes their hotkey.
///
/// Error handling — all failure modes emit EXACTLY ONE consolidated error
/// toast (never two stacked ones) and fall back to the OS default when
/// possible:
///
///   - **Parse failure** (`ctrl+shft+q`, `f99`, modifier-only, multi-key):
///     logs a warning, tries the default chord. A single toast is emitted
///     only AFTER we know whether the default succeeded.
///   - **Bind failure of a user-chosen chord** (OS refuses because another
///     app claimed it — very common with `Ctrl+Shift+Space` on Windows
///     when an IME / Office / third-party tool owns it): retries once with
///     the default, emits a single toast with the actual conflict message.
///   - **Bind failure of the default chord**: surfaces a single toast
///     telling the user the app is usable via the tray.
///   - **User explicitly configured the default chord** and bind fails:
///     we do NOT mislabel the error as "couldn't register its default" —
///     we say "your configured hotkey is claimed by another app". We still
///     don't retry, because retrying the same chord won't help.
pub fn register_hotkey(
    app: &AppHandle,
    engine: SharedEngine,
    hotkey_str: Option<&str>,
) -> Result<(), String> {
    let default = default_shortcut();

    // Parse step. We track two things:
    //   - the parse error (if any), so we can include it in a consolidated toast
    //   - whether the caller explicitly chose the default chord
    let (requested, parse_error): (Shortcut, Option<HotkeyParseError>) =
        match parse_hotkey(hotkey_str) {
            Ok(s) => (s, None),
            Err(err) => {
                eprintln!("[hotkey] {err}; falling back to OS default chord");
                // Fall back to default; no toast here — wait until we know
                // whether the default actually binds, then emit ONE message.
                (default, Some(err))
            }
        };
    let user_explicitly_chose_default = parse_error.is_none() && requested == default;

    let gs = app.global_shortcut();
    // Clear any previously registered shortcut(s) this app owns. We only ever
    // register one shortcut at a time, so `unregister_all` is safe and
    // sidesteps needing to track the "old" Shortcut ourselves.
    let _ = gs.unregister_all();

    let bind = |sc: Shortcut| -> Result<(), String> {
        let engine_hk = engine.clone();
        let handle_hk = app.clone();
        gs.on_shortcut(sc, move |_app, _sc, event| {
            if event.state() == ShortcutState::Pressed {
                let eng = engine_hk.clone();
                let app = handle_hk.clone();
                tauri::async_runtime::spawn(async move {
                    handle_hotkey(eng, app).await;
                });
            }
        })
        .map_err(|e| e.to_string())
    };

    // Try the (possibly defaulted) requested chord first.
    let primary_result = bind(requested);

    match primary_result {
        Ok(()) => {
            // Primary bind succeeded. If we got here via a parse fallback,
            // emit exactly ONE toast explaining that we corrected the input.
            if let Some(err) = parse_error {
                let _ = app.emit(
                    "quill://error",
                    serde_json::json!({
                        "message": format!(
                            "Unrecognised hotkey {:?} — using the default shortcut instead. \
                             Check your Settings → Hotkey field. ({err})",
                            err.input
                        ),
                    }),
                );
            }
            Ok(())
        }
        Err(primary_err) => {
            // Primary bind failed. Decide whether to retry.
            // - If the primary WAS the default (either because the user chose
            //   it or because parse fallback put us there), don't retry —
            //   we have no "next best" to try.
            // - Otherwise, retry with the default.
            let tried_default_already = user_explicitly_chose_default || parse_error.is_some();

            if tried_default_already {
                // Single consolidated toast explaining the situation.
                eprintln!("[hotkey] bind failed: {primary_err}");
                let msg = if let Some(err) = parse_error {
                    format!(
                        "Couldn't register a hotkey: your configured shortcut {:?} was \
                         unrecognised ({err}) and the default fallback also failed ({primary_err}). \
                         You can still open Quill from the tray icon."
                    , err.input)
                } else {
                    format!(
                        "Your configured hotkey is already claimed by another app ({primary_err}). \
                         Try a different chord in Settings, or open Quill from the tray icon."
                    )
                };
                let _ = app.emit("quill://error", serde_json::json!({ "message": msg }));
                return Err(primary_err);
            }

            // Retry with the OS default.
            eprintln!(
                "[hotkey] requested chord bind failed ({primary_err}); \
                 retrying with OS default"
            );
            let _ = gs.unregister_all();
            match bind(default) {
                Ok(()) => {
                    let _ = app.emit(
                        "quill://error",
                        serde_json::json!({
                            "message": format!(
                                "Your configured hotkey {:?} is already claimed by another app \
                                 ({primary_err}). Quill fell back to its default shortcut — you \
                                 can change it in Settings.",
                                hotkey_str.unwrap_or("")
                            ),
                        }),
                    );
                    Ok(())
                }
                Err(default_err) => {
                    eprintln!("[hotkey] default fallback also failed: {default_err}");
                    let _ = app.emit(
                        "quill://error",
                        serde_json::json!({
                            "message": format!(
                                "Quill couldn't register any global hotkey. You can still use \
                                 the app from the tray icon. (configured: {primary_err}; \
                                 default: {default_err})"
                            ),
                        }),
                    );
                    Err(default_err)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sc(mods: Modifiers, code: Code) -> Shortcut {
        Shortcut::new(if mods.is_empty() { None } else { Some(mods) }, code)
    }

    #[test]
    fn parses_letters() {
        assert_eq!(
            parse_hotkey(Some("ctrl+shift+q")).unwrap(),
            sc(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyQ)
        );
        assert_eq!(
            parse_hotkey(Some("cmd+alt+a")).unwrap(),
            sc(Modifiers::META | Modifiers::ALT, Code::KeyA)
        );
    }

    #[test]
    fn parses_digits() {
        assert_eq!(
            parse_hotkey(Some("ctrl+1")).unwrap(),
            sc(Modifiers::CONTROL, Code::Digit1)
        );
        assert_eq!(
            parse_hotkey(Some("ctrl+shift+0")).unwrap(),
            sc(Modifiers::CONTROL | Modifiers::SHIFT, Code::Digit0)
        );
    }

    #[test]
    fn parses_f_keys() {
        assert_eq!(
            parse_hotkey(Some("ctrl+f1")).unwrap(),
            sc(Modifiers::CONTROL, Code::F1)
        );
        assert_eq!(
            parse_hotkey(Some("ctrl+f12")).unwrap(),
            sc(Modifiers::CONTROL, Code::F12)
        );
        assert_eq!(
            parse_hotkey(Some("f24")).unwrap(),
            sc(Modifiers::empty(), Code::F24)
        );
    }

    #[test]
    fn parses_named_keys() {
        assert_eq!(
            parse_hotkey(Some("ctrl+enter")).unwrap(),
            sc(Modifiers::CONTROL, Code::Enter)
        );
        assert_eq!(
            parse_hotkey(Some("alt+tab")).unwrap(),
            sc(Modifiers::ALT, Code::Tab)
        );
        assert_eq!(
            parse_hotkey(Some("shift+escape")).unwrap(),
            sc(Modifiers::SHIFT, Code::Escape)
        );
        assert_eq!(
            parse_hotkey(Some("ctrl+home")).unwrap(),
            sc(Modifiers::CONTROL, Code::Home)
        );
        assert_eq!(
            parse_hotkey(Some("ctrl+pageup")).unwrap(),
            sc(Modifiers::CONTROL, Code::PageUp)
        );
        assert_eq!(
            parse_hotkey(Some("ctrl+left")).unwrap(),
            sc(Modifiers::CONTROL, Code::ArrowLeft)
        );
    }

    #[test]
    fn empty_resolves_to_os_default() {
        // Empty string or None → Code::Space with OS-default modifiers.
        #[cfg(target_os = "macos")]
        let default_mods = Modifiers::META | Modifiers::SHIFT;
        #[cfg(not(target_os = "macos"))]
        let default_mods = Modifiers::CONTROL | Modifiers::SHIFT;

        assert_eq!(
            parse_hotkey(Some("")).unwrap(),
            sc(default_mods, Code::Space)
        );
        assert_eq!(parse_hotkey(None).unwrap(), sc(default_mods, Code::Space));
        assert_eq!(
            parse_hotkey(Some("   ")).unwrap(),
            sc(default_mods, Code::Space)
        );
    }

    #[test]
    fn cmd_meta_super_win_are_aliases() {
        let base = parse_hotkey(Some("cmd+q")).unwrap();
        assert_eq!(parse_hotkey(Some("meta+q")).unwrap(), base);
        assert_eq!(parse_hotkey(Some("super+q")).unwrap(), base);
        assert_eq!(parse_hotkey(Some("win+q")).unwrap(), base);
    }

    #[test]
    fn case_insensitive_and_whitespace_tolerant() {
        assert_eq!(
            parse_hotkey(Some("CTRL+SHIFT+Q")).unwrap(),
            sc(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyQ)
        );
        assert_eq!(
            parse_hotkey(Some("  Ctrl + Shift + Space  ")).unwrap(),
            sc(Modifiers::CONTROL | Modifiers::SHIFT, Code::Space)
        );
    }

    #[test]
    fn unknown_key_returns_error() {
        // Unknown key names must NOT silently fall back — they must produce a
        // parse error so `register_hotkey` can log + toast + keep the default.
        let err = parse_hotkey(Some("ctrl+unknownkey")).unwrap_err();
        assert!(err.reason.contains("unknown key"));
        assert!(err.reason.contains("unknownkey"));
    }

    #[test]
    fn modifier_only_returns_error() {
        // "ctrl+shift" with no key is meaningless — error rather than fallback.
        let err = parse_hotkey(Some("ctrl+shift")).unwrap_err();
        assert!(err.reason.contains("no key specified"));
    }

    #[test]
    fn multiple_keys_returns_error() {
        // "ctrl+a+b" has two keys — ambiguous, error out.
        let err = parse_hotkey(Some("ctrl+a+b")).unwrap_err();
        assert!(err.reason.contains("more than one key"));
    }

    #[test]
    fn f99_returns_error() {
        // F-key out of range — error, not silent fallback.
        assert!(parse_hotkey(Some("ctrl+f99")).is_err());
    }
}
