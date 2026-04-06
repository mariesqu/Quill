"""Shared app-name → context mapping, used by all OS backends."""

import re

APP_CONTEXT_MAP: dict[str, dict] = {
    # Email
    "mail": {"tone": "professional", "hint": "email"},
    "outlook": {"tone": "professional", "hint": "email"},
    "thunderbird": {"tone": "professional", "hint": "email"},
    "gmail": {"tone": "professional", "hint": "email"},
    # Code editors / IDEs
    "code": {"tone": "technical", "hint": "code editor"},
    "vscode": {"tone": "technical", "hint": "code editor"},
    "xcode": {"tone": "technical", "hint": "code editor"},
    "pycharm": {"tone": "technical", "hint": "code editor"},
    "intellij": {"tone": "technical", "hint": "code editor"},
    "vim": {"tone": "technical", "hint": "code editor"},
    "nvim": {"tone": "technical", "hint": "code editor"},
    "emacs": {"tone": "technical", "hint": "code editor"},
    "sublime": {"tone": "technical", "hint": "code editor"},
    "cursor": {"tone": "technical", "hint": "code editor"},
    "windsurf": {"tone": "technical", "hint": "code editor"},
    "terminal": {"tone": "technical", "hint": "terminal"},
    "iterm": {"tone": "technical", "hint": "terminal"},
    "warp": {"tone": "technical", "hint": "terminal"},
    "ghostty": {"tone": "technical", "hint": "terminal"},
    "alacritty": {"tone": "technical", "hint": "terminal"},
    "kitty": {"tone": "technical", "hint": "terminal"},
    "konsole": {"tone": "technical", "hint": "terminal"},
    # Browsers
    "safari": {"tone": "neutral", "hint": "browser"},
    "chrome": {"tone": "neutral", "hint": "browser"},
    "firefox": {"tone": "neutral", "hint": "browser"},
    "edge": {"tone": "neutral", "hint": "browser"},
    "arc": {"tone": "neutral", "hint": "browser"},
    "brave": {"tone": "neutral", "hint": "browser"},
    # Chat / messaging
    "slack": {"tone": "casual", "hint": "chat"},
    "discord": {"tone": "casual", "hint": "chat"},
    "messages": {"tone": "casual", "hint": "chat"},
    "telegram": {"tone": "casual", "hint": "chat"},
    "whatsapp": {"tone": "casual", "hint": "chat"},
    "signal": {"tone": "casual", "hint": "chat"},
    "teams": {"tone": "professional", "hint": "chat"},
    "zoom": {"tone": "professional", "hint": "meeting"},
    # Documents / writing
    "word": {"tone": "formal", "hint": "document"},
    "pages": {"tone": "formal", "hint": "document"},
    "docs": {"tone": "formal", "hint": "document"},
    "libreoffice": {"tone": "formal", "hint": "document"},
    "notion": {"tone": "neutral", "hint": "notes"},
    "obsidian": {"tone": "neutral", "hint": "notes"},
    "bear": {"tone": "neutral", "hint": "notes"},
    "typora": {"tone": "neutral", "hint": "notes"},
    "logseq": {"tone": "neutral", "hint": "notes"},
    "jira": {"tone": "professional", "hint": "project management"},
    "linear": {"tone": "professional", "hint": "project management"},
}


def lookup_context(app_name: str) -> dict:
    """Look up context for a given app name (case-insensitive word-boundary match)."""
    name_lower = app_name.lower()
    for key, ctx in APP_CONTEXT_MAP.items():
        if (
            re.search(r"(?:^|[\b_\-. ])" + re.escape(key) + r"(?:$|[\b_\-. ])", name_lower)
            or key == name_lower
        ):
            return {"app": app_name, **ctx}
    # Fallback to substring match for short app names
    for key, ctx in APP_CONTEXT_MAP.items():
        if key in name_lower:
            return {"app": app_name, **ctx}
    return {"app": app_name, "tone": "neutral", "hint": "general app"}
