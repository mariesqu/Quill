use std::collections::HashMap;
use super::config::PersonaConfig;
use super::modes::ModeConfig;
use crate::platform::context::AppContext;

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
        "casual"       => "Write in a casual, conversational, friendly tone.",
        "professional" => "Write in a polished, professional tone suitable for business.",
        "witty"        => "Write with wit and light humour — clever but never forced.",
        "direct"       => "Be extremely direct and concise. No fluff, no filler words.",
        "warm"         => "Write in a warm, empathetic, human tone.",
        _              => "",
    }
}

fn build_persona_block(persona: &PersonaConfig) -> String {
    if !persona.enabled { return String::new(); }
    let mut parts = vec!["\n\n─── User Voice Constraints (always apply) ───".to_string()];
    let tone_desc = persona_tone_description(&persona.tone);
    if !tone_desc.is_empty() { parts.push(tone_desc.to_string()); }
    if !persona.style.trim().is_empty() { parts.push(format!("Style: {}", persona.style.trim())); }
    if !persona.avoid.trim().is_empty() { parts.push(format!("Never use: {}", persona.avoid.trim())); }
    if parts.len() == 1 { return String::new(); }
    parts.push("─────────────────────────────────────────────".to_string());
    parts.join("\n")
}

fn build_language_instruction(language: &str) -> String {
    let lang = language.trim().to_lowercase();
    if lang.is_empty() || lang == "auto" { return String::new(); }
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
    let mode_cfg = modes.get(mode).ok_or_else(|| format!("Unknown mode: {mode}"))?;

    let lang_for_template = if language.trim().to_lowercase() == "auto" || language.trim().is_empty() {
        "the source language".to_string()
    } else {
        language.trim().to_string()
    };

    let mut mode_instruction = mode_cfg.prompt.replace("{language}", &lang_for_template).trim().to_string();

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
    let has_non_ascii = text.chars().any(|c| c as u32 > 127);
    let has_grammar_signals = text.contains("  ")
        || regex::Regex::new(r"\s[,\.;]").map_or(false, |r| r.is_match(text))
        || regex::Regex::new(r"[a-z]\.[A-Z]").map_or(false, |r| r.is_match(text));

    if word_count > 120 {
        return ("shorter".into(), "Your text is long — Shorter will distil it to the key message".into());
    }
    if word_count < 15 && context.tone != "technical" {
        return ("expand".into(), "Your text is brief — Expand will add depth and context".into());
    }
    if context.tone == "casual" && word_count > 30 {
        return ("shorter".into(), "Chat messages land better when concise".into());
    }
    if (context.hint == "email" || context.hint == "document")
        && (context.tone == "professional" || context.tone == "formal")
    {
        return ("formal".into(), "You're in a professional context — Formal will polish the tone".into());
    }
    if context.tone == "technical" {
        return ("rewrite".into(), "Technical writing benefits from a clarity rewrite".into());
    }
    if has_grammar_signals {
        return ("fix_grammar".into(), "Some punctuation or spacing issues detected".into());
    }
    if has_non_ascii {
        return ("translate".into(), "Non-Latin characters detected — Translate might be useful".into());
    }
    ("rewrite".into(), "Rewrite improves clarity for most text".into())
}
