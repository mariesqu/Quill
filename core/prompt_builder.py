"""
Builds prompts from mode templates + app context + persona + output language.
"""
from __future__ import annotations

import re
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
    "natural":      "",
    "casual":       "Write in a casual, conversational, friendly tone.",
    "professional": "Write in a polished, professional tone suitable for business.",
    "witty":        "Write with wit and light humour — clever but never forced.",
    "direct":       "Be extremely direct and concise. No fluff, no filler words.",
    "warm":         "Write in a warm, empathetic, human tone.",
}


def _build_persona_block(persona: dict[str, Any]) -> str:
    if not persona or not persona.get("enabled"):
        return ""

    parts: list[str] = ["\n\n─── User Voice Constraints (always apply) ───"]
    tone = persona.get("tone", "natural")
    if tone not in PERSONA_TONE_DESCRIPTIONS:
        import logging
        logging.getLogger("quill.prompt").warning("Unknown persona tone: %s, using natural", tone)
        tone = "natural"
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
        return ""
    parts.append("─────────────────────────────────────────────")
    return "\n".join(parts)


def _build_language_instruction(language: str) -> str:
    lang = (language or "").strip().lower()
    if not lang or lang == "auto":
        return ""
    lang_display = language.strip().capitalize()
    return f"\n\nIMPORTANT: Always write your entire response in {lang_display}, regardless of the input language."


def build_prompt(
    text: str,
    mode: str,
    modes: dict[str, Any],
    context: dict[str, Any],
    language: str = "auto",
    persona: dict[str, Any] | None = None,
    extra_instruction: str = "",
) -> tuple[str, str]:
    """
    Return (system_prompt, user_prompt).

    Args:
        text:              The selected text to transform.
        mode:              Mode key (e.g. 'rewrite').
        modes:             Dict of mode configs from modes.yaml.
        context:           Active app context dict.
        language:          Target output language. 'auto' = keep source language.
        persona:           User persona config dict.
        extra_instruction: One-off instruction from the overlay input field.
    """
    if mode not in modes:
        raise ValueError(f"Unknown mode: {mode!r}. Available: {list(modes)}")

    mode_cfg = modes[mode]
    template: str = mode_cfg["prompt"]

    lang_for_template = (
        language.strip() if language and language.lower() != "auto" else "the source language"
    )
    mode_instruction = template.format(language=lang_for_template).strip()

    # Prepend any one-off instruction from the user
    if extra_instruction and extra_instruction.strip():
        mode_instruction = f"Additional instruction: {extra_instruction.strip()}\n\n{mode_instruction}"

    user_prompt = f"{mode_instruction}\n\n---\n{text}"

    # System prompt
    tone = context.get("tone", "neutral")
    hint = context.get("hint", "")
    system = SYSTEM_BASE
    system += CONTEXT_SYSTEM_ADDITIONS.get(tone, "")
    if hint:
        system += f" Current context: {hint}."
    system += _build_language_instruction(language)
    system += _build_persona_block(persona or {})

    return system, user_prompt


# ── Smart mode suggestion ─────────────────────────────────────────────────────

def suggest_mode(text: str, context: dict[str, Any]) -> tuple[str, str]:
    """
    Heuristic-based mode suggestion. Returns (mode_id, reason).
    """
    word_count = len(text.split())
    has_non_ascii = bool(re.search(r"[^\x00-\x7F]", text))
    lower = text.lower()

    # Grammar indicators: multiple punctuation errors, obvious misspellings
    grammar_signals = (
        text.count("  ") > 1 or          # double spaces
        re.search(r"\s[,\.;]", text) or   # space before punctuation
        re.search(r"[a-z]\.[A-Z]", text)  # missing space after period
    )

    # Length signals
    if word_count > 120:
        return "shorter", "Your text is long — Shorter will distil it to the key message"

    if word_count < 15 and context.get("tone") != "technical":
        return "expand", "Your text is brief — Expand will add depth and context"

    # Context signals
    if context.get("tone") == "casual" and word_count > 30:
        return "shorter", "Chat messages land better when concise"

    if context.get("hint") in ("email", "document") and context.get("tone") in ("professional", "formal"):
        return "formal", "You're in a professional context — Formal will polish the tone"

    if context.get("tone") == "technical":
        return "rewrite", "Technical writing benefits from a clarity rewrite"

    # Grammar signals
    if grammar_signals:
        return "fix_grammar", "Some punctuation or spacing issues detected"

    # Non-ASCII (likely foreign text)
    if has_non_ascii:
        return "translate", "Non-Latin characters detected — Translate might be useful"

    # Default
    return "rewrite", "Rewrite improves clarity for most text"
