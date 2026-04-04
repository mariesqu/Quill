"""
Builds prompts from mode templates + app context.
"""
from __future__ import annotations

from typing import Any


SYSTEM_BASE = (
    "You are Quill, a concise and precise AI writing assistant. "
    "Follow the user's instructions exactly. "
    "Return only the requested output — no preamble, no explanation, no markdown unless asked."
)

CONTEXT_SYSTEM_ADDITIONS = {
    "technical": " The user is working in a technical/code environment. Use technical language where appropriate.",
    "professional": " The user is working in a professional email or business context. Maintain a professional tone.",
    "casual": " The user is working in a casual chat context. Keep the tone conversational.",
    "formal": " The user is working in a formal document context. Use formal, structured language.",
    "neutral": "",
}


def build_prompt(
    text: str,
    mode: str,
    modes: dict[str, Any],
    context: dict[str, Any],
    language: str = "auto",
) -> tuple[str, str]:
    """
    Return (system_prompt, user_prompt) for the given mode and context.
    """
    if mode not in modes:
        raise ValueError(f"Unknown mode: {mode!r}. Available: {list(modes)}")

    mode_cfg = modes[mode]
    template: str = mode_cfg["prompt"]

    # Resolve language
    lang = language if language != "auto" else "the source language"
    user_prompt = template.format(language=lang).strip()
    user_prompt = f"{user_prompt}\n\n---\n{text}"

    # Build system prompt with context hints
    tone = context.get("tone", "neutral")
    hint = context.get("hint", "")
    system = SYSTEM_BASE
    system += CONTEXT_SYSTEM_ADDITIONS.get(tone, "")
    if hint:
        system += f" Current context: {hint}."

    return system, user_prompt
