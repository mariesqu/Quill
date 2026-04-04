"""
Builds prompts from mode templates + app context + persona + output language.
"""
from __future__ import annotations

from typing import Any


SYSTEM_BASE = (
    "You are Quill, a concise and precise AI writing assistant. "
    "Follow the user's instructions exactly. "
    "Return only the requested output — no preamble, no explanation, no markdown unless asked."
)

CONTEXT_SYSTEM_ADDITIONS = {
    "technical":    " The user is working in a technical/code environment. Use technical language where appropriate.",
    "professional": " The user is working in a professional email or business context. Maintain a professional tone.",
    "casual":       " The user is working in a casual chat context. Keep the tone conversational.",
    "formal":       " The user is working in a formal document context. Use formal, structured language.",
    "neutral":      "",
}

PERSONA_TONE_DESCRIPTIONS = {
    "natural":      "",  # no extra instruction — let the mode guide tone
    "casual":       "Write in a casual, conversational, friendly tone.",
    "professional": "Write in a polished, professional tone suitable for business.",
    "witty":        "Write with wit and light humour — clever but never forced.",
    "direct":       "Be extremely direct and concise. No fluff, no filler words.",
    "warm":         "Write in a warm, empathetic, human tone.",
}


def _build_persona_block(persona: dict[str, Any]) -> str:
    """Return a persona instruction block to append to the system prompt."""
    if not persona or not persona.get("enabled"):
        return ""

    parts: list[str] = ["\n\n─── User Voice Constraints (always apply) ───"]

    tone = persona.get("tone", "natural")
    tone_desc = PERSONA_TONE_DESCRIPTIONS.get(tone, "")
    if tone_desc:
        parts.append(tone_desc)

    style = (persona.get("style") or "").strip()
    if style:
        parts.append(f"Style: {style}")

    avoid = (persona.get("avoid") or "").strip()
    if avoid:
        parts.append(f"Never use: {avoid}")

    if len(parts) == 1:
        return ""  # only header, nothing useful

    parts.append("─────────────────────────────────────────────")
    return "\n".join(parts)


def _build_language_instruction(language: str) -> str:
    """Return a language instruction to append to the system prompt."""
    lang = language.strip().lower()
    if not lang or lang == "auto":
        return ""
    # Capitalise first letter for readability in the prompt
    lang_display = language.strip().capitalize()
    return f"\n\nIMPORTANT: Always write your entire response in {lang_display}, regardless of the input language."


def build_prompt(
    text: str,
    mode: str,
    modes: dict[str, Any],
    context: dict[str, Any],
    language: str = "auto",
    persona: dict[str, Any] | None = None,
) -> tuple[str, str]:
    """
    Return (system_prompt, user_prompt) for the given mode, context, language, and persona.

    Args:
        text:     The selected text to transform.
        mode:     Mode key (e.g. 'rewrite', 'translate').
        modes:    Dict of mode configs loaded from modes.yaml.
        context:  Active app context from ContextBackend.
        language: Target output language, e.g. 'French'. 'auto' = keep source language.
        persona:  User persona config dict (from config['persona']).
    """
    if mode not in modes:
        raise ValueError(f"Unknown mode: {mode!r}. Available: {list(modes)}")

    mode_cfg = modes[mode]
    template: str = mode_cfg["prompt"]

    # Resolve {language} placeholder in the mode template
    lang_for_template = language.strip() if language and language.lower() != "auto" else "the source language"
    user_prompt = template.format(language=lang_for_template).strip()
    user_prompt = f"{user_prompt}\n\n---\n{text}"

    # Build system prompt: base + context + language override + persona
    tone = context.get("tone", "neutral")
    hint = context.get("hint", "")

    system = SYSTEM_BASE
    system += CONTEXT_SYSTEM_ADDITIONS.get(tone, "")
    if hint:
        system += f" Current context: {hint}."

    # Language override: if a specific language is chosen, make it a hard constraint
    system += _build_language_instruction(language)

    # Persona voice constraints
    system += _build_persona_block(persona or {})

    return system, user_prompt
