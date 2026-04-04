"""Tests for core/prompt_builder.py"""
import pytest
from core.prompt_builder import build_prompt, SYSTEM_BASE


MODES = {
    "rewrite": {
        "label": "Rewrite",
        "icon": "✏️",
        "prompt": "Rewrite the following text to improve clarity and flow.\nReturn only the rewritten text.",
    },
    "translate": {
        "label": "Translate",
        "icon": "🌐",
        "prompt": "Translate the following text to {language}.\nReturn only the translated text.",
    },
    "coach": {
        "label": "Coach",
        "icon": "💡",
        "prompt": "Give 2-3 suggestions to improve this text.",
    },
}

CONTEXT_NEUTRAL = {"app": "unknown", "tone": "neutral", "hint": ""}
CONTEXT_EMAIL = {"app": "outlook", "tone": "professional", "hint": "email"}
CONTEXT_CODE = {"app": "vscode", "tone": "technical", "hint": "code editor"}


def test_build_prompt_returns_tuple():
    system, user = build_prompt("Hello world", "rewrite", MODES, CONTEXT_NEUTRAL)
    assert isinstance(system, str)
    assert isinstance(user, str)


def test_build_prompt_includes_text():
    text = "The quick brown fox"
    _, user = build_prompt(text, "rewrite", MODES, CONTEXT_NEUTRAL)
    assert text in user


def test_build_prompt_unknown_mode():
    with pytest.raises(ValueError, match="Unknown mode"):
        build_prompt("test", "nonexistent_mode", MODES, CONTEXT_NEUTRAL)


def test_build_prompt_language_substitution():
    _, user = build_prompt("Bonjour", "translate", MODES, CONTEXT_NEUTRAL, language="Spanish")
    assert "Spanish" in user


def test_build_prompt_language_auto():
    _, user = build_prompt("test", "translate", MODES, CONTEXT_NEUTRAL, language="auto")
    # Should not contain literal 'auto'
    assert "auto" not in user
    # Should contain the fallback phrase
    assert "source language" in user


def test_build_prompt_context_professional():
    system, _ = build_prompt("test", "rewrite", MODES, CONTEXT_EMAIL)
    assert SYSTEM_BASE in system
    assert "professional" in system.lower() or "business" in system.lower()


def test_build_prompt_context_technical():
    system, _ = build_prompt("test", "rewrite", MODES, CONTEXT_CODE)
    assert "technical" in system.lower()


def test_build_prompt_context_hint_included():
    system, _ = build_prompt("test", "rewrite", MODES, CONTEXT_EMAIL)
    assert "email" in system.lower()
