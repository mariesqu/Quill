use super::config::PersonaConfig;
use super::modes::ModeConfig;
use crate::platform::context::AppContext;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Space followed by a comma, period, or semicolon — a classic grammar red flag.
static SPACE_BEFORE_PUNCT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s[,\.;]").expect("SPACE_BEFORE_PUNCT regex"));

/// Lowercase letter, period, uppercase letter with no space — missing sentence boundary.
static MISSING_SENTENCE_SPACE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[a-z]\.[A-Z]").expect("MISSING_SENTENCE_SPACE regex"));

const SYSTEM_BASE: &str = "You are Quill, a concise and precise AI writing assistant. \
Follow the user's instructions exactly. \
Return only the requested output — no preamble, no explanation, no markdown unless asked.";

fn context_addition(tone: &str) -> &'static str {
    match tone {
        "technical"    => " The user is in a technical/code environment. Use technical language where appropriate.",
        "professional" => " The user is in a professional email or business context. Maintain a professional tone.",
        "casual"       => " The user is in a casual chat context. Keep the tone conversational.",
        "formal"       => " The user is in a formal document context. Use formal, structured language.",
        _              => "",
    }
}

fn persona_tone_description(tone: &str) -> &'static str {
    match tone {
        "casual" => "Write in a casual, conversational, friendly tone.",
        "professional" => "Write in a polished, professional tone suitable for business.",
        "witty" => "Write with wit and light humour — clever but never forced.",
        "direct" => "Be extremely direct and concise. No fluff, no filler words.",
        "warm" => "Write in a warm, empathetic, human tone.",
        _ => "",
    }
}

fn build_persona_block(persona: &PersonaConfig) -> String {
    if !persona.enabled {
        return String::new();
    }
    let mut parts = vec!["\n\n─── User Voice Constraints (always apply) ───".to_string()];
    let tone_desc = persona_tone_description(&persona.tone);
    if !tone_desc.is_empty() {
        parts.push(tone_desc.to_string());
    }
    if !persona.style.trim().is_empty() {
        parts.push(format!("Style: {}", persona.style.trim()));
    }
    if !persona.avoid.trim().is_empty() {
        parts.push(format!("Never use: {}", persona.avoid.trim()));
    }
    if parts.len() == 1 {
        return String::new();
    }
    parts.push("─────────────────────────────────────────────".to_string());
    parts.join("\n")
}

fn build_language_instruction(language: &str) -> String {
    let lang = language.trim().to_lowercase();
    if lang.is_empty() || lang == "auto" {
        return String::new();
    }
    let display = {
        let mut chars = language.trim().chars();
        match chars.next() {
            None => String::new(),
            Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        }
    };
    format!("\n\nIMPORTANT: Always write your entire response in {display}, regardless of the input language.")
}

/// Returns (system_prompt, user_prompt).
pub fn build_prompt(
    text: &str,
    mode: &str,
    modes: &HashMap<String, ModeConfig>,
    context: &AppContext,
    language: &str,
    persona: &PersonaConfig,
    extra_instruction: Option<&str>,
) -> Result<(String, String), String> {
    let mode_cfg = modes
        .get(mode)
        .ok_or_else(|| format!("Unknown mode: {mode}"))?;

    let lang_for_template =
        if language.trim().to_lowercase() == "auto" || language.trim().is_empty() {
            "the source language".to_string()
        } else {
            language.trim().to_string()
        };

    let mut mode_instruction = mode_cfg
        .prompt
        .replace("{language}", &lang_for_template)
        .trim()
        .to_string();

    if let Some(extra) = extra_instruction {
        let extra = extra.trim();
        if !extra.is_empty() {
            mode_instruction = format!("Additional instruction: {extra}\n\n{mode_instruction}");
        }
    }

    let user_prompt = format!("{mode_instruction}\n\n---\n{text}");

    let mut system = SYSTEM_BASE.to_string();
    system.push_str(context_addition(&context.tone));
    if !context.hint.is_empty() {
        system.push_str(&format!(" Current context: {}.", context.hint));
    }
    system.push_str(&build_language_instruction(language));
    system.push_str(&build_persona_block(persona));

    Ok((system, user_prompt))
}

/// Heuristic mode suggestion. Returns (mode_id, reason).
pub fn suggest_mode(text: &str, context: &AppContext) -> (String, String) {
    let word_count = text.split_whitespace().count();

    // Translate suggestion guard. A single `é`, `—`, curly quote, or "café"
    // must NOT trigger a Translate suggestion — those are common in English.
    // We require BOTH:
    //   - at least 3 non-ASCII chars (so one diacritic doesn't count), AND
    //   - non-ASCII chars make up > 10% of the text (so a long English
    //     paragraph with one em-dash doesn't count either).
    let total_chars = text.chars().count();
    let non_ascii_count = text.chars().filter(|c| *c as u32 > 127).count();
    let non_ascii_ratio = if total_chars > 0 {
        non_ascii_count as f64 / total_chars as f64
    } else {
        0.0
    };
    let looks_non_latin = non_ascii_count >= 3 && non_ascii_ratio > 0.10;

    let has_grammar_signals = text.contains("  ")
        || SPACE_BEFORE_PUNCT.is_match(text)
        || MISSING_SENTENCE_SPACE.is_match(text);

    if word_count > 120 {
        return (
            "shorter".into(),
            "Your text is long — Shorter will distil it to the key message".into(),
        );
    }
    if word_count < 15 && context.tone != "technical" {
        return (
            "expand".into(),
            "Your text is brief — Expand will add depth and context".into(),
        );
    }
    if context.tone == "casual" && word_count > 30 {
        return (
            "shorter".into(),
            "Chat messages land better when concise".into(),
        );
    }
    if (context.hint == "email" || context.hint == "document")
        && (context.tone == "professional" || context.tone == "formal")
    {
        return (
            "formal".into(),
            "You're in a professional context — Formal will polish the tone".into(),
        );
    }
    if context.tone == "technical" {
        return (
            "rewrite".into(),
            "Technical writing benefits from a clarity rewrite".into(),
        );
    }
    if has_grammar_signals {
        return (
            "fix_grammar".into(),
            "Some punctuation or spacing issues detected".into(),
        );
    }
    if looks_non_latin {
        return (
            "translate".into(),
            "Non-Latin script detected — Translate might be useful".into(),
        );
    }
    (
        "rewrite".into(),
        "Rewrite improves clarity for most text".into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(tone: &str, hint: &str) -> AppContext {
        AppContext {
            app: "test".into(),
            tone: tone.into(),
            hint: hint.into(),
        }
    }

    fn mode(prompt: &str) -> ModeConfig {
        ModeConfig {
            label: "Test".into(),
            icon: "🔧".into(),
            prompt: prompt.into(),
        }
    }

    fn fixture_modes() -> HashMap<String, ModeConfig> {
        let mut m = HashMap::new();
        m.insert("rewrite".into(), mode("Rewrite this: {language}"));
        m.insert("translate".into(), mode("Translate to {language}"));
        m
    }

    // ── suggest_mode ────────────────────────────────────────────────────────
    #[test]
    fn suggest_long_text_returns_shorter() {
        let text = "word ".repeat(130);
        let (m, _) = suggest_mode(&text, &ctx("neutral", "general"));
        assert_eq!(m, "shorter");
    }

    #[test]
    fn suggest_short_text_returns_expand() {
        let (m, _) = suggest_mode("just a few words here", &ctx("neutral", "general"));
        assert_eq!(m, "expand");
    }

    #[test]
    fn suggest_email_formal_context_returns_formal() {
        // 20 words — avoids the `word_count < 15` expand branch.
        let text = "I wanted to follow up on the proposal we discussed earlier this week and share some additional thoughts on it.";
        let (m, _) = suggest_mode(text, &ctx("professional", "email"));
        assert_eq!(m, "formal");
    }

    #[test]
    fn suggest_technical_context_returns_rewrite() {
        // technical tone takes priority regardless of word count.
        let text = "The function takes a pointer and returns an optional result type that must be checked before use.";
        let (m, _) = suggest_mode(text, &ctx("technical", "code editor"));
        assert_eq!(m, "rewrite");
    }

    #[test]
    fn suggest_grammar_signals_return_fix_grammar() {
        // 16 words with a space-before-comma and double space — grammar red flags.
        let text = "Hello there  , this draft has some small issues ,and could use a quick grammar pass overall.";
        let (m, _) = suggest_mode(text, &ctx("neutral", "general"));
        assert_eq!(m, "fix_grammar");
    }

    #[test]
    fn suggest_single_diacritic_does_not_trigger_translate() {
        // A single `é` in an English sentence must NOT trigger Translate.
        let text = "Her résumé looks great and she seems highly qualified for the role we are trying to fill.";
        let (m, _) = suggest_mode(text, &ctx("neutral", "general"));
        assert_ne!(m, "translate");
    }

    #[test]
    fn suggest_em_dash_does_not_trigger_translate() {
        // An em-dash is a single non-ASCII char — should never trigger Translate.
        let text = "This feature is great — users love it and adoption is climbing steadily across every segment.";
        let (m, _) = suggest_mode(text, &ctx("neutral", "general"));
        assert_ne!(m, "translate");
    }

    #[test]
    fn suggest_dense_non_latin_triggers_translate() {
        // Substantive non-Latin content → propose Translate. Note that
        // `suggest_mode` counts whitespace-separated tokens, and Japanese runs
        // together without spaces; we pad with enough English tokens to clear
        // the `word_count < 15` expand branch, while keeping > 10 % non-ASCII
        // so the translate heuristic fires.
        let text = "here is some text with a lot of non latin script mixed in for a good test: \
                    こんにちは、これは日本語のテスト文章です。さらに文字を追加してテストを確実にします。";
        let (m, _) = suggest_mode(text, &ctx("neutral", "general"));
        assert_eq!(m, "translate");
    }

    #[test]
    fn suggest_default_is_rewrite() {
        // Long enough (>15 words) and neutral — no other branch fires → rewrite.
        let text = "Medium length text that does not match any of the other heuristic branches so it should fall through to the default rewrite suggestion.";
        let (m, _) = suggest_mode(text, &ctx("neutral", "general"));
        assert_eq!(m, "rewrite");
    }

    // ── build_prompt ────────────────────────────────────────────────────────
    #[test]
    fn build_prompt_returns_system_and_user() {
        let modes = fixture_modes();
        let persona = PersonaConfig::default();
        let ctx = ctx("neutral", "general");
        let (system, user) =
            build_prompt("hello", "rewrite", &modes, &ctx, "auto", &persona, None).unwrap();
        assert!(system.contains("Quill"));
        assert!(user.contains("hello"));
    }

    #[test]
    fn build_prompt_unknown_mode_is_error() {
        let modes = fixture_modes();
        let persona = PersonaConfig::default();
        let err = build_prompt(
            "hello",
            "nonexistent_mode",
            &modes,
            &ctx("neutral", "general"),
            "auto",
            &persona,
            None,
        )
        .unwrap_err();
        assert!(err.contains("nonexistent_mode"));
    }

    #[test]
    fn build_prompt_substitutes_language_placeholder() {
        let modes = fixture_modes();
        let persona = PersonaConfig::default();
        let (_system, user) = build_prompt(
            "bonjour",
            "translate",
            &modes,
            &ctx("neutral", "general"),
            "Spanish",
            &persona,
            None,
        )
        .unwrap();
        assert!(user.contains("Spanish"));
    }

    #[test]
    fn build_prompt_prepends_extra_instruction() {
        let modes = fixture_modes();
        let persona = PersonaConfig::default();
        let (_system, user) = build_prompt(
            "hello",
            "rewrite",
            &modes,
            &ctx("neutral", "general"),
            "auto",
            &persona,
            Some("make it witty"),
        )
        .unwrap();
        assert!(user.contains("make it witty"));
    }

    #[test]
    fn build_prompt_includes_language_instruction_when_not_auto() {
        let modes = fixture_modes();
        let persona = PersonaConfig::default();
        let (system, _) = build_prompt(
            "hello",
            "rewrite",
            &modes,
            &ctx("neutral", "general"),
            "French",
            &persona,
            None,
        )
        .unwrap();
        assert!(system.contains("French"));
    }

    #[test]
    fn build_prompt_persona_block_appears_when_enabled() {
        let modes = fixture_modes();
        let persona = PersonaConfig {
            enabled: true,
            tone: "witty".into(),
            style: "Short punchy sentences.".into(),
            avoid: "buzzwords".into(),
        };
        let (system, _) = build_prompt(
            "hello",
            "rewrite",
            &modes,
            &ctx("neutral", "general"),
            "auto",
            &persona,
            None,
        )
        .unwrap();
        assert!(system.contains("User Voice"));
        assert!(system.contains("Short punchy sentences"));
        assert!(system.contains("buzzwords"));
    }
}
