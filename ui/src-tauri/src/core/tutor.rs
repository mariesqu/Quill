use super::history::HistoryStats;

pub const EXPLAIN_SYSTEM: &str = "You are a knowledgeable and encouraging language tutor. \
     Your job is to explain writing improvements concisely and educationally. \
     Be specific, practical, and warm. Never patronise. \
     Focus on rules and principles the user can apply themselves next time.";

pub const LESSON_SYSTEM: &str =
    "You are an expert language and writing coach who creates personalised, \
     actionable micro-lessons. Your lessons are concise, concrete, and based on \
     real examples from the learner's own writing. Never be abstract — always \
     anchor advice to examples.";

pub fn build_explain_prompt(original: &str, output: &str, mode: &str, language: &str) -> String {
    let lang_note = if !language.is_empty() && language.to_lowercase() != "auto" {
        format!(" The target language is {language}.")
    } else {
        String::new()
    };

    format!(
        r#"You transformed the following text using "{mode}" mode.{lang_note}

ORIGINAL:
{original}

TRANSFORMED:
{output}

Please explain:
1. The 2–3 most significant changes you made and the specific rule or principle behind each one.
2. One practical tip the writer can apply themselves next time.
3. If a language translation was involved, highlight one interesting linguistic difference between the source and target language that this example illustrates.

Keep it concise — 3–5 short paragraphs max. Use plain language, not jargon."#
    )
}

pub fn build_lesson_prompt(stats: &HistoryStats, period: &str) -> String {
    if stats.count == 0 {
        return format!(
            "The user has no Quill history yet for a {period} lesson. \
             Generate a warm, encouraging 3-sentence welcome message explaining that their \
             personalised lessons will appear here once they start using Quill, and give one \
             universal writing tip to get them started."
        );
    }

    let top_mode = stats.top_mode.as_deref().unwrap_or("rewrite");
    let top_lang = stats.top_language.as_deref().unwrap_or("auto");
    let mode_breakdown: String = {
        let mut pairs: Vec<_> = stats.mode_counts.iter().collect();
        pairs.sort_by(|a, b| b.1.cmp(a.1));
        pairs
            .iter()
            .map(|(m, c)| format!("{m}: {c}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let lang_breakdown: String = {
        let mut pairs: Vec<_> = stats
            .lang_counts
            .iter()
            .filter(|(l, _)| l.as_str() != "auto")
            .collect();
        pairs.sort_by(|a, b| b.1.cmp(a.1));
        pairs
            .iter()
            .map(|(l, c)| format!("{l}: {c}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let samples_block: String = stats
        .sample_originals
        .iter()
        .zip(stats.sample_outputs.iter())
        .enumerate()
        .map(|(i, (orig, out))| {
            format!(
                "\nExample {}:\n  Before: {}\n  After:  {}",
                i + 1,
                orig,
                out
            )
        })
        .collect();

    let lang_focus = if !top_lang.is_empty() && top_lang != "auto" {
        format!("\nThe user has been writing in or translating to **{top_lang}** most often. Include at least one language-specific insight for {top_lang}.")
    } else {
        String::new()
    };

    let period_label = if period == "daily" { "day" } else { "7 days" };
    let lang_corner = if !top_lang.is_empty() && top_lang != "auto" {
        format!("\n## {top_lang} corner\n[One interesting rule or nuance of {top_lang} illustrated by the translations]")
    } else {
        String::new()
    };

    format!(
        r#"Generate a {period} writing lesson for this user based on their Quill usage over the last {period_label}.

USAGE SUMMARY:
- Total transformations: {count}
- Most used mode: {top_mode} ({top_mode_count} times)
- Mode breakdown: {mode_breakdown}
- Languages used: {lang_breakdown}
- Average word count reduction: {avg_pct}%
{lang_focus}

SAMPLE TRANSFORMATIONS (use these as examples in your lesson):
{samples_block}

FORMAT your lesson as:
# {period_cap} Writing Insight

## What you worked on
[1–2 sentences summarising what the user did]

## Key lesson
[The most important principle illustrated by their usage, with a concrete example from their actual text]

## Tip to try
[One specific, actionable thing to try today/this week]
{lang_corner}

Keep the whole lesson under 250 words. Be encouraging and specific."#,
        count = stats.count,
        top_mode_count = stats.mode_counts.get(top_mode).copied().unwrap_or(0),
        avg_pct = (stats.avg_reduction * 100.0) as i64,
        period_cap = {
            let mut c = period.chars();
            c.next()
                .map(|ch| ch.to_uppercase().to_string())
                .unwrap_or_default()
                + c.as_str()
        },
    )
}
