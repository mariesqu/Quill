use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppContext {
    pub app: String,
    pub tone: String,
    pub hint: String,
}

const APP_MAP: &[(&str, &str, &str)] = &[
    ("mail", "professional", "email"),
    ("outlook", "professional", "email"),
    ("thunderbird", "professional", "email"),
    ("gmail", "professional", "email"),
    ("spark", "professional", "email"),
    ("code", "technical", "code editor"),
    ("vscode", "technical", "code editor"),
    ("nvim", "technical", "code editor"),
    ("vim", "technical", "code editor"),
    ("emacs", "technical", "code editor"),
    ("intellij", "technical", "code editor"),
    ("xcode", "technical", "code editor"),
    ("terminal", "technical", "terminal"),
    ("iterm", "technical", "terminal"),
    ("alacritty", "technical", "terminal"),
    ("wezterm", "technical", "terminal"),
    ("cmd", "technical", "terminal"),
    ("powershell", "technical", "terminal"),
    ("chrome", "casual", "browser"),
    ("firefox", "casual", "browser"),
    ("safari", "casual", "browser"),
    ("arc", "casual", "browser"),
    ("edge", "casual", "browser"),
    ("slack", "casual", "chat"),
    ("discord", "casual", "chat"),
    ("telegram", "casual", "chat"),
    ("whatsapp", "casual", "chat"),
    ("teams", "professional", "meeting"),
    ("zoom", "professional", "meeting"),
    ("meet", "professional", "meeting"),
    ("word", "formal", "document"),
    ("docs", "formal", "document"),
    ("pages", "formal", "document"),
    ("notion", "neutral", "notes"),
    ("obsidian", "neutral", "notes"),
    ("bear", "neutral", "notes"),
    ("jira", "professional", "project management"),
    ("linear", "professional", "project management"),
    ("asana", "professional", "project management"),
];

pub fn get_active_context() -> AppContext {
    let app_name = detect_active_app().to_lowercase();
    for (key, tone, hint) in APP_MAP {
        if app_name.contains(key) {
            return AppContext {
                app: app_name,
                tone: tone.to_string(),
                hint: hint.to_string(),
            };
        }
    }
    AppContext {
        app: app_name,
        tone: "neutral".into(),
        hint: "general".into(),
    }
}

#[cfg(target_os = "windows")]
fn detect_active_app() -> String {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    // Always resolve via the process image name, never via the window title.
    // Titles like "How to use Gmail — Arc" would false-match our APP_MAP,
    // whereas image names like "chrome.exe" / "Code.exe" are stable and
    // unambiguous. This matches the intent of the APP_MAP keys.
    unsafe {
        let hwnd: HWND = GetForegroundWindow();
        if hwnd.is_invalid() {
            return String::new();
        }

        let mut pid = 0u32;
        let _tid = GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return String::new();
        }
        process_name_for_pid(pid)
    }
}

#[cfg(target_os = "windows")]
fn process_name_for_pid(pid: u32) -> String {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows::core::PWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return String::new();
        };

        let mut buf = vec![0u16; 512];
        let mut len = buf.len() as u32;
        let query_result =
            QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(h);

        // If the query failed, don't pretend we have a name — return empty so
        // the caller falls back to the neutral context (rather than matching
        // zeroed buffer contents against APP_MAP).
        if query_result.is_err() || len == 0 {
            return String::new();
        }

        let name = OsString::from_wide(&buf[..len as usize])
            .to_string_lossy()
            .to_string();
        std::path::Path::new(&name)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or(name)
    }
}

#[cfg(target_os = "macos")]
fn detect_active_app() -> String {
    use std::process::Command;
    let out = Command::new("osascript")
        .args(["-e", "tell application \"System Events\" to get name of first application process whose frontmost is true"])
        .output();
    if let Ok(o) = out {
        return String::from_utf8_lossy(&o.stdout).trim().to_string();
    }
    String::new()
}

#[cfg(target_os = "linux")]
fn detect_active_app() -> String {
    // Mirror the Windows strategy: resolve to the PROCESS NAME, never the
    // window title. A Firefox tab titled "How to use Outlook — Firefox" would
    // otherwise false-match both `outlook` and `firefox` in APP_MAP.
    //
    // 1. Ask xdotool for the active window's pid.
    // 2. Read /proc/<pid>/comm for the process image name (stable, short).
    // 3. Only if /proc isn't available, fall back to xdotool window name.
    let pid_out = std::process::Command::new("xdotool")
        .args(["getactivewindow", "getwindowpid"])
        .output();
    if let Ok(pid_result) = pid_out {
        let pid_str = String::from_utf8_lossy(&pid_result.stdout)
            .trim()
            .to_string();
        if let Ok(pid) = pid_str.parse::<u32>() {
            let comm_path = format!("/proc/{pid}/comm");
            if let Ok(name) = std::fs::read_to_string(&comm_path) {
                let trimmed = name.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
    }

    // Last-resort fallback: xdotool's window name. Better than nothing if
    // /proc is unavailable (e.g. flatpak sandbox).
    let name_out = std::process::Command::new("xdotool")
        .args(["getactivewindow", "getwindowname"])
        .output();
    if let Ok(o) = name_out {
        let name = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !name.is_empty() {
            return name;
        }
    }
    String::new()
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn detect_active_app() -> String {
    String::new()
}

// ---------------------------------------------------------------------------
// Trait implementation
// ---------------------------------------------------------------------------

use super::traits::ContextProbe;

/// Production implementation of `ContextProbe`. Wraps the existing
/// `get_active_context` free function.
#[derive(Default)]
pub struct Context;

impl ContextProbe for Context {
    fn active_context(&self) -> AppContext {
        get_active_context()
    }
}
