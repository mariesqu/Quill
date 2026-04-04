"""
AI Tutor engine.
Builds prompts for:
  - Explaining a specific transformation (what changed & why)
  - Generating daily/weekly lessons from usage patterns
"""
from __future__ import annotations

from typing import Any

# ── Explanation prompt ────────────────────────────────────────────────────────

EXPLAIN_SYSTEM = (
    "You are a knowledgeable and encouraging language tutor. "
    "Your job is to explain writing improvements concisely and educationally. "
    "Be specific, practical, and warm. Never patronise. "
    "Focus on rules and principles the user can apply themselves next time."
)


def build_explain_prompt(
    original: str,
    output: str,
    mode: str,
    language: str = "auto",
) -> tuple[str, str]:
    """
    Build a prompt asking the AI to explain what changed and why.
    Returns (system, user_prompt).
    """
    lang_note = ""
    if language and language.lower() not in ("auto", ""):
        lang_note = f" The target language is {language}."

    user_prompt = f"""You transformed the following text using "{mode}" mode.{lang_note}

ORIGINAL:
{original}

TRANSFORMED:
{output}

Please explain:
1. The 2–3 most significant changes you made and the specific rule or principle behind each one.
2. One practical tip the writer can apply themselves next time.
3. If a language translation was involved, highlight one interesting linguistic difference between the source and target language that this example illustrates.

Keep it concise — 3–5 short paragraphs max. Use plain language, not jargon."""

    return EXPLAIN_SYSTEM, user_prompt


# ── Lesson generation prompt ──────────────────────────────────────────────────

LESSON_SYSTEM = (
    "You are an expert language and writing coach who creates personalised, "
    "actionable micro-lessons. Your lessons are concise, concrete, and based on "
    "real examples from the learner's own writing. Never be abstract — always "
    "anchor advice to examples."
)


def build_lesson_prompt(stats: dict[str, Any], period: str = "daily") -> tuple[str, str]:
    """
    Build a prompt to generate a lesson from usage stats.
    Returns (system, user_prompt).
    """
    if stats.get("count", 0) == 0:
        return LESSON_SYSTEM, _no_data_prompt(period)

    top_mode = stats.get("top_mode", "rewrite")
    top_lang = stats.get("top_language", "auto")
    mode_counts = stats.get("mode_counts", {})
    lang_counts = stats.get("lang_counts", {})
    avg_reduction = stats.get("avg_reduction", 0)
    sample_originals = stats.get("sample_originals", [])
    sample_outputs   = stats.get("sample_outputs", [])

    samples_block = ""
    for i, (orig, out) in enumerate(zip(sample_originals[:3], sample_outputs[:3])):
        samples_block += f"\nExample {i+1}:\n  Before: {orig}\n  After:  {out}\n"

    lang_focus = ""
    if top_lang and top_lang not in ("auto", ""):
        lang_focus = (
            f"\nThe user has been writing in or translating to **{top_lang}** most often. "
            f"Include at least one language-specific insight for {top_lang}."
        )

    user_prompt = f"""Generate a {period} writing lesson for this user based on their Quill usage over the last {'day' if period == 'daily' else '7 days'}.

USAGE SUMMARY:
- Total transformations: {stats['count']}
- Most used mode: {top_mode} ({mode_counts.get(top_mode, 0)} times)
- Mode breakdown: {', '.join(f'{m}: {c}' for m, c in sorted(mode_counts.items(), key=lambda x: -x[1]))}
- Languages used: {', '.join(f'{l}: {c}' for l, c in sorted(lang_counts.items(), key=lambda x: -x[1]) if l != 'auto')}
- Average word count reduction: {int(avg_reduction * 100)}%
{lang_focus}

SAMPLE TRANSFORMATIONS (use these as examples in your lesson):
{samples_block}

FORMAT your lesson as:
# {period.capitalize()} Writing Insight

## What you worked on
[1–2 sentences summarising what the user did]

## Key lesson
[The most important principle illustrated by their usage, with a concrete example from their actual text]

## Tip to try
[One specific, actionable thing to try today/this week]

{"## " + top_lang + " corner" + chr(10) + "[One interesting rule or nuance of " + top_lang + " illustrated by the translations]" if top_lang and top_lang not in ("auto", "") else ""}

Keep the whole lesson under 250 words. Be encouraging and specific."""

    return LESSON_SYSTEM, user_prompt


def _no_data_prompt(period: str) -> str:
    return (
        f"The user has no Quill history yet for a {period} lesson. "
        "Generate a warm, encouraging 3-sentence welcome message explaining that their "
        "personalised lessons will appear here once they start using Quill, and give one "
        "universal writing tip to get them started."
    )
