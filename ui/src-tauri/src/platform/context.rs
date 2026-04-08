use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppContext {
    pub app:  String,
    pub tone: String,
    pub hint: String,
}

const APP_MAP: &[(&str, &str, &str)] = &[
    ("mail",        "professional", "email"),
    ("outlook",     "professional", "email"),
    ("thunderbird", "professional", "email"),
    ("gmail",       "professional", "email"),
    ("spark",       "professional", "email"),
    ("code",        "technical",    "code editor"),
    ("vscode",      "technical",    "code editor"),
    ("nvim",        "technical",    "code editor"),
    ("vim",         "technical",    "code editor"),
    ("emacs",       "technical",    "code editor"),
    ("intellij",    "technical",    "code editor"),
    ("xcode",       "technical",    "code editor"),
    ("terminal",    "technical",    "terminal"),
    ("iterm",       "technical",    "terminal"),
    ("alacritty",   "technical",    "terminal"),
    ("wezterm",     "technical",    "terminal"),
    ("cmd",         "technical",    "terminal"),
    ("powershell",  "technical",    "terminal"),
    ("chrome",      "casual",       "browser"),
    ("firefox",     "casual",       "browser"),
    ("safari",      "casual",       "browser"),
    ("arc",         "casual",       "browser"),
    ("edge",        "casual",       "browser"),
    ("slack",       "casual",       "chat"),
    ("discord",     "casual",       "chat"),
    ("telegram",    "casual",       "chat"),
    ("whatsapp",    "casual",       "chat"),
    ("teams",       "professional", "meeting"),
    ("zoom",        "professional", "meeting"),
    ("meet",        "professional", "meeting"),
    ("word",        "formal",       "document"),
    ("docs",        "formal",       "document"),
    ("pages",       "formal",       "document"),
    ("notion",      "neutral",      "notes"),
    ("obsidian",    "neutral",      "notes"),
    ("bear",        "neutral",      "notes"),
    ("jira",        "professional", "project management"),
    ("linear",      "professional", "project management"),
    ("asana",       "professional", "project management"),
];

pub fn get_active_context() -> AppContext {
    let app_name = detect_active_app().to_lowercase();
    for (key, tone, hint) in APP_MAP {
        if app_name.contains(key) {
            return AppContext {
                app:  app_name,
                tone: tone.to_string(),
                hint: hint.to_string(),
            };
        }
    }
    AppContext { app: app_name, tone: "neutral".into(), hint: "general".into() }
}

#[cfg(target_os = "windows")]
fn detect_active_app() -> String {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    unsafe {
        let hwnd = windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        if hwnd.0 == 0 { return String::new(); }

        let mut buf = vec![0u16; 512];
        let len = windows::Win32::UI::WindowsAndMessaging::GetWindowTextW(hwnd, &mut buf);
        if len > 0 {
            return OsString::from_wide(&buf[..len as usize])
                .to_string_lossy().to_string();
        }

        // Fallback: process name via PID
        let mut pid = 0u32;
        windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(hwnd, Some(&mut pid));
        process_name_for_pid(pid)
    }
}

#[cfg(target_os = "windows")]
fn process_name_for_pid(pid: u32) -> String {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    unsafe {
        let handle = windows::Win32::System::Threading::OpenProcess(
            windows::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION,
            false, pid,
        );
        if let Ok(h) = handle {
            let mut buf = vec![0u16; 512];
            let mut len = buf.len() as u32;
            let _ = windows::Win32::System::Threading::QueryFullProcessImageNameW(
                h, windows::Win32::System::Threading::PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut len,
            );
            let _ = windows::Win32::Foundation::CloseHandle(h);
            let name = OsString::from_wide(&buf[..len as usize]).to_string_lossy().to_string();
            return std::path::Path::new(&name)
                .file_stem().map(|s| s.to_string_lossy().to_string())
                .unwrap_or(name);
        }
    }
    String::new()
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
    // Try xdotool
    let out = std::process::Command::new("xdotool")
        .args(["getactivewindow", "getwindowname"])
        .output();
    if let Ok(o) = out {
        let name = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !name.is_empty() { return name; }
    }
    // Fallback: /proc-based active process (rough)
    String::new()
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn detect_active_app() -> String { String::new() }
