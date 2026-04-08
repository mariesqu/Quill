use anyhow::{Context, Result};
use dirs::home_dir;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn db_path() -> PathBuf {
    home_dir().unwrap_or_default().join(".quill").join("history.db")
}

fn open() -> Result<Connection> {
    let path = db_path();
    std::fs::create_dir_all(path.parent().unwrap())?;
    let conn = Connection::open(&path).context("opening history.db")?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    Ok(conn)
}

pub fn init_db() -> Result<()> {
    let conn = open()?;
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS history (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp         DATETIME DEFAULT CURRENT_TIMESTAMP,
            app_hint          TEXT,
            mode              TEXT,
            language          TEXT,
            persona_tone      TEXT,
            original_text     TEXT NOT NULL,
            output_text       TEXT NOT NULL,
            word_count_before INTEGER,
            word_count_after  INTEGER,
            tutor_explanation TEXT,
            favorited         INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_history_ts   ON history(timestamp);
        CREATE INDEX IF NOT EXISTS idx_history_mode ON history(mode);
        CREATE INDEX IF NOT EXISTS idx_history_lang ON history(language);
        CREATE INDEX IF NOT EXISTS idx_history_fav  ON history(favorited);

        CREATE TABLE IF NOT EXISTS tutor_lessons (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            period     TEXT NOT NULL,
            language   TEXT,
            lesson_md  TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_lessons_period ON tutor_lessons(period, created_at);
    ")?;
    // Non-destructive migration for pre-existing DBs
    let _ = conn.execute("ALTER TABLE history ADD COLUMN favorited INTEGER NOT NULL DEFAULT 0", []);
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id:                i64,
    pub timestamp:         String,
    pub app_hint:          Option<String>,
    pub mode:              Option<String>,
    pub language:          Option<String>,
    pub persona_tone:      Option<String>,
    pub original_text:     String,
    pub output_text:       String,
    pub word_count_before: Option<i64>,
    pub word_count_after:  Option<i64>,
    pub tutor_explanation: Option<String>,
    pub favorited:         bool,
}

pub fn save_entry(
    original: &str,
    output: &str,
    mode: &str,
    language: &str,
    app_hint: &str,
    persona_tone: &str,
) -> Result<i64> {
    let conn = open()?;
    let wc_before = original.split_whitespace().count() as i64;
    let wc_after  = output.split_whitespace().count() as i64;
    conn.execute(
        "INSERT INTO history
         (app_hint, mode, language, persona_tone, original_text, output_text, word_count_before, word_count_after)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![app_hint, mode, language, persona_tone, original, output, wc_before, wc_after],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn save_tutor_explanation(entry_id: i64, explanation: &str) -> Result<()> {
    let conn = open()?;
    conn.execute("UPDATE history SET tutor_explanation = ?1 WHERE id = ?2", params![explanation, entry_id])?;
    Ok(())
}

pub fn get_recent(limit: usize, language: Option<&str>) -> Result<Vec<HistoryEntry>> {
    let conn = open()?;
    let rows = if let Some(lang) = language.filter(|l| *l != "auto") {
        let mut stmt = conn.prepare(
            "SELECT * FROM history WHERE language = ?1 ORDER BY timestamp DESC LIMIT ?2")?;
        stmt.query_map(params![lang, limit as i64], row_to_entry)?.collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        let mut stmt = conn.prepare("SELECT * FROM history ORDER BY timestamp DESC LIMIT ?1")?;
        stmt.query_map(params![limit as i64], row_to_entry)?.collect::<rusqlite::Result<Vec<_>>>()?
    };
    Ok(rows)
}

pub fn get_all_entries() -> Result<Vec<HistoryEntry>> {
    let conn = open()?;
    let mut stmt = conn.prepare("SELECT * FROM history ORDER BY timestamp DESC")?;
    let rows = stmt.query_map([], row_to_entry)?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn toggle_favorite(entry_id: i64) -> Result<bool> {
    let conn = open()?;
    let current: i64 = conn.query_row(
        "SELECT favorited FROM history WHERE id = ?1", params![entry_id], |r| r.get(0))?;
    let new_val = if current == 0 { 1i64 } else { 0i64 };
    conn.execute("UPDATE history SET favorited = ?1 WHERE id = ?2", params![new_val, entry_id])?;
    Ok(new_val == 1)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryStats {
    pub count:           usize,
    pub days:            usize,
    pub mode_counts:     std::collections::HashMap<String, usize>,
    pub lang_counts:     std::collections::HashMap<String, usize>,
    pub avg_reduction:   f64,
    pub top_mode:        Option<String>,
    pub top_language:    Option<String>,
    pub sample_originals: Vec<String>,
    pub sample_outputs:   Vec<String>,
}

pub fn get_stats(days: usize) -> Result<HistoryStats> {
    let conn = open()?;
    let since = format!("-{days} days");
    let mut stmt = conn.prepare(
        "SELECT mode, language, word_count_before, word_count_after, original_text, output_text
         FROM history WHERE timestamp >= datetime('now', ?1)")?;
    let rows: Vec<(String, String, i64, i64, String, String)> = stmt
        .query_map(params![since], |r| Ok((
            r.get::<_, String>(0).unwrap_or_default(),
            r.get::<_, String>(1).unwrap_or_default(),
            r.get::<_, i64>(2).unwrap_or(0),
            r.get::<_, i64>(3).unwrap_or(0),
            r.get::<_, String>(4).unwrap_or_default(),
            r.get::<_, String>(5).unwrap_or_default(),
        )))?.collect::<rusqlite::Result<Vec<_>>>()?;

    let mut mode_counts: std::collections::HashMap<String, usize> = Default::default();
    let mut lang_counts: std::collections::HashMap<String, usize> = Default::default();
    let (mut total_before, mut total_after) = (0i64, 0i64);
    let mut sample_originals = vec![];
    let mut sample_outputs = vec![];

    for (i, (mode, lang, wc_b, wc_a, orig, out)) in rows.iter().enumerate() {
        *mode_counts.entry(mode.clone()).or_default() += 1;
        *lang_counts.entry(lang.clone()).or_default() += 1;
        total_before += wc_b;
        total_after  += wc_a;
        if i < 5 {
            sample_originals.push(orig.chars().take(200).collect());
            sample_outputs.push(out.chars().take(200).collect());
        }
    }

    let top_mode = mode_counts.iter().max_by_key(|(_, v)| *v).map(|(k, _)| k.clone());
    let top_language = lang_counts.iter().max_by_key(|(_, v)| *v).map(|(k, _)| k.clone());
    let avg_reduction = if total_before > 0 { 1.0 - (total_after as f64 / total_before as f64) } else { 0.0 };

    Ok(HistoryStats {
        count: rows.len(),
        days,
        mode_counts,
        lang_counts,
        avg_reduction,
        top_mode,
        top_language,
        sample_originals,
        sample_outputs,
    })
}

pub fn save_lesson(period: &str, lesson_md: &str, language: &str) -> Result<()> {
    let conn = open()?;
    conn.execute("INSERT INTO tutor_lessons (period, language, lesson_md) VALUES (?1,?2,?3)",
        params![period, language, lesson_md])?;
    Ok(())
}

pub fn get_latest_lesson(period: &str) -> Result<Option<String>> {
    let conn = open()?;
    let result = conn.query_row(
        "SELECT lesson_md FROM tutor_lessons WHERE period = ?1 ORDER BY created_at DESC LIMIT 1",
        params![period],
        |r| r.get::<_, String>(0),
    );
    match result {
        Ok(s) => Ok(Some(s)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn row_to_entry(r: &rusqlite::Row) -> rusqlite::Result<HistoryEntry> {
    Ok(HistoryEntry {
        id:                r.get("id")?,
        timestamp:         r.get("timestamp")?,
        app_hint:          r.get("app_hint")?,
        mode:              r.get("mode")?,
        language:          r.get("language")?,
        persona_tone:      r.get("persona_tone")?,
        original_text:     r.get("original_text")?,
        output_text:       r.get("output_text")?,
        word_count_before: r.get("word_count_before")?,
        word_count_after:  r.get("word_count_after")?,
        tutor_explanation: r.get("tutor_explanation")?,
        favorited:         r.get::<_, i64>("favorited").map(|v| v == 1)?,
    })
}
