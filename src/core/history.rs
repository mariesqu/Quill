#![allow(dead_code)] // Some query helpers are only reached from the History tab UI path
use anyhow::{Context, Result};
use dirs::home_dir;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn db_path() -> PathBuf {
    home_dir()
        .unwrap_or_default()
        .join(".quill")
        .join("history.db")
}

fn open() -> Result<Connection> {
    // Short-circuit if init_db previously failed. Without this, every call
    // site (save_entry, get_recent, toggle_favorite, …) re-opens, re-fails,
    // and spams the log. HISTORY_USABLE is set to false by `main.rs` on
    // `init_db` failure.
    if !HISTORY_USABLE.load(std::sync::atomic::Ordering::Acquire) {
        return Err(anyhow::anyhow!(
            "history unavailable: init_db failed earlier in this session"
        ));
    }
    let path = db_path();
    std::fs::create_dir_all(path.parent().unwrap())?;
    let conn = Connection::open(&path).context("opening history.db")?;
    // `journal_mode=WAL` is a DATABASE property — it's persisted in the
    // file header once and recognised on every subsequent open. We only
    // need to set it when bootstrapping the schema (init_db). `foreign_
    // keys=ON` is CONNECTION-scoped (defaults to OFF in SQLite) so it
    // stays here on every open.
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    Ok(conn)
}

/// Tracks whether `init_db` has succeeded. `false` short-circuits every
/// history call so we don't repeatedly hammer a dead filesystem / locked
/// file. Set to false by `main.rs` on `init_db` failure.
pub static HISTORY_USABLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn init_db() -> Result<()> {
    // Open directly — we can't go through `open()` here because that
    // short-circuits on !HISTORY_USABLE, and init_db is the thing that
    // decides usability in the first place.
    let path = db_path();
    std::fs::create_dir_all(path.parent().unwrap())?;
    let conn = Connection::open(&path).context("opening history.db")?;
    // Set WAL here (once) — it's persistent in the file header.
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    conn.execute_batch(
        "
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
    ",
    )?;
    // The `favorited` column is already created by `CREATE TABLE IF NOT
    // EXISTS history` above, so the old `ALTER TABLE ... ADD COLUMN
    // favorited` migration was a guaranteed failure on every fresh DB (it
    // always logged "duplicate column"). The true pre-column-era databases
    // this targeted never existed in the wild.
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub timestamp: String,
    pub app_hint: Option<String>,
    pub mode: Option<String>,
    pub language: Option<String>,
    pub persona_tone: Option<String>,
    pub original_text: String,
    pub output_text: String,
    pub word_count_before: Option<i64>,
    pub word_count_after: Option<i64>,
    pub tutor_explanation: Option<String>,
    pub favorited: bool,
}

pub fn save_entry(
    original: &str,
    output: &str,
    mode: &str,
    language: &str,
    app_hint: &str,
    persona_tone: &str,
    max_entries: usize,
) -> Result<i64> {
    let mut conn = open()?;
    let wc_before = original.split_whitespace().count() as i64;
    let wc_after = output.split_whitespace().count() as i64;

    // Insert + prune MUST be atomic. Previously they were two separate
    // statements outside any transaction, which meant a crash between the
    // INSERT and the prune DELETE could leave us past `max_entries` until
    // the next save pushed us back in range. Worse, a concurrent reader
    // could observe a transient state with N+1 rows. `rusqlite::transaction`
    // wraps both statements in a BEGIN/COMMIT so either both happen or
    // neither does.
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO history
         (app_hint, mode, language, persona_tone, original_text, output_text, word_count_before, word_count_after)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![app_hint, mode, language, persona_tone, original, output, wc_before, wc_after],
    )?;
    let id = tx.last_insert_rowid();

    // Enforce `max_entries` cap — delete the oldest rows that exceed the limit.
    // This runs on every insert, but it's cheap (one index lookup + a bounded
    // delete), so the O(N) amortised cost is acceptable for a per-user DB.
    if max_entries > 0 {
        tx.execute(
            "DELETE FROM history WHERE id IN (
                SELECT id FROM history
                ORDER BY timestamp DESC, id DESC
                LIMIT -1 OFFSET ?1
            )",
            params![max_entries as i64],
        )?;
    }

    tx.commit()?;
    Ok(id)
}

pub fn get_entry(id: i64) -> Result<HistoryEntry> {
    let conn = open()?;
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, app_hint, mode, language, persona_tone,
                original_text, output_text, word_count_before, word_count_after,
                tutor_explanation, favorited
         FROM history WHERE id = ?1",
    )?;
    let entry = stmt.query_row(params![id], |row| {
        Ok(HistoryEntry {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            app_hint: row.get(2)?,
            mode: row.get(3)?,
            language: row.get(4)?,
            persona_tone: row.get(5)?,
            original_text: row.get(6)?,
            output_text: row.get(7)?,
            word_count_before: row.get(8)?,
            word_count_after: row.get(9)?,
            tutor_explanation: row.get(10)?,
            favorited: row.get::<_, i64>(11)? != 0,
        })
    })?;
    Ok(entry)
}

pub fn save_tutor_explanation(entry_id: i64, explanation: &str) -> Result<()> {
    let conn = open()?;
    conn.execute(
        "UPDATE history SET tutor_explanation = ?1 WHERE id = ?2",
        params![explanation, entry_id],
    )?;
    Ok(())
}

pub fn get_recent(limit: usize) -> Result<Vec<HistoryEntry>> {
    let conn = open()?;
    let mut stmt = conn.prepare("SELECT * FROM history ORDER BY timestamp DESC LIMIT ?1")?;
    let mapped = stmt.query_map(params![limit as i64], row_to_entry)?;
    let mut out = Vec::new();
    for row in mapped {
        out.push(row?);
    }
    Ok(out)
}

pub fn get_by_id(id: i64) -> Result<Option<HistoryEntry>> {
    let conn = open()?;
    let result = conn.query_row(
        "SELECT * FROM history WHERE id = ?1",
        params![id],
        row_to_entry,
    );
    match result {
        Ok(entry) => Ok(Some(entry)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn get_all_entries() -> Result<Vec<HistoryEntry>> {
    let conn = open()?;
    let mut stmt = conn.prepare("SELECT * FROM history ORDER BY timestamp DESC")?;
    let rows = stmt
        .query_map([], row_to_entry)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn toggle_favorite(entry_id: i64) -> Result<bool> {
    let conn = open()?;
    let current: i64 = conn.query_row(
        "SELECT favorited FROM history WHERE id = ?1",
        params![entry_id],
        |r| r.get(0),
    )?;
    let new_val = if current == 0 { 1i64 } else { 0i64 };
    conn.execute(
        "UPDATE history SET favorited = ?1 WHERE id = ?2",
        params![new_val, entry_id],
    )?;
    Ok(new_val == 1)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryStats {
    pub count: usize,
    pub days: usize,
    pub mode_counts: std::collections::HashMap<String, usize>,
    pub lang_counts: std::collections::HashMap<String, usize>,
    pub avg_reduction: f64,
    pub top_mode: Option<String>,
    pub top_language: Option<String>,
    pub sample_originals: Vec<String>,
    pub sample_outputs: Vec<String>,
}

pub fn get_stats(days: usize) -> Result<HistoryStats> {
    let conn = open()?;
    let since = format!("-{days} days");
    let mut stmt = conn.prepare(
        "SELECT mode, language, word_count_before, word_count_after, original_text, output_text
         FROM history WHERE timestamp >= datetime('now', ?1)",
    )?;
    let rows: Vec<(String, String, i64, i64, String, String)> = stmt
        .query_map(params![since], |r| {
            Ok((
                r.get::<_, String>(0).unwrap_or_default(),
                r.get::<_, String>(1).unwrap_or_default(),
                r.get::<_, i64>(2).unwrap_or(0),
                r.get::<_, i64>(3).unwrap_or(0),
                r.get::<_, String>(4).unwrap_or_default(),
                r.get::<_, String>(5).unwrap_or_default(),
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut mode_counts: std::collections::HashMap<String, usize> = Default::default();
    let mut lang_counts: std::collections::HashMap<String, usize> = Default::default();
    let (mut total_before, mut total_after) = (0i64, 0i64);
    let mut sample_originals = vec![];
    let mut sample_outputs = vec![];

    for (i, (mode, lang, wc_b, wc_a, orig, out)) in rows.iter().enumerate() {
        // Skip empty-string keys — they represent rows with no recorded
        // mode/language (legacy / import artefacts). Including them
        // poisons the "top mode" / "top language" picks with a nameless
        // bucket that can't be displayed meaningfully.
        if !mode.is_empty() {
            *mode_counts.entry(mode.clone()).or_default() += 1;
        }
        if !lang.is_empty() {
            *lang_counts.entry(lang.clone()).or_default() += 1;
        }
        total_before += wc_b;
        total_after += wc_a;
        if i < 5 {
            sample_originals.push(orig.chars().take(200).collect());
            sample_outputs.push(out.chars().take(200).collect());
        }
    }

    // Deterministic tie-breaking: `max_by_key` on a HashMap is order-
    // dependent when two keys tie, so the same data can produce different
    // "top" picks across runs. Break ties by preferring the lexicographi-
    // cally-smaller key (the `b.0.cmp(a.0)` arm flips the usual ordering
    // because `max_by` keeps the rightmost element on equal ordering).
    let top_mode = mode_counts
        .iter()
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
        .map(|(k, _)| k.clone());
    let top_language = lang_counts
        .iter()
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
        .map(|(k, _)| k.clone());
    let avg_reduction = if total_before > 0 {
        1.0 - (total_after as f64 / total_before as f64)
    } else {
        0.0
    };

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
    conn.execute(
        "INSERT INTO tutor_lessons (period, language, lesson_md) VALUES (?1,?2,?3)",
        params![period, language, lesson_md],
    )?;
    Ok(())
}

fn row_to_entry(r: &rusqlite::Row) -> rusqlite::Result<HistoryEntry> {
    Ok(HistoryEntry {
        id: r.get("id")?,
        timestamp: r.get("timestamp")?,
        app_hint: r.get("app_hint")?,
        mode: r.get("mode")?,
        language: r.get("language")?,
        persona_tone: r.get("persona_tone")?,
        original_text: r.get("original_text")?,
        output_text: r.get("output_text")?,
        word_count_before: r.get("word_count_before")?,
        word_count_after: r.get("word_count_after")?,
        tutor_explanation: r.get("tutor_explanation")?,
        favorited: r.get::<_, i64>("favorited").map(|v| v == 1)?,
    })
}
