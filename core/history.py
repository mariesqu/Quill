"""
Local history store — opt-in SQLite log of all Quill transformations.
Enables the AI Tutor to analyse patterns and generate personalised lessons.

Schema:
  history     — one row per transformation
  tutor_lessons — cached daily/weekly lessons
"""
from __future__ import annotations

import sqlite3
import logging
from datetime import datetime, timedelta
from pathlib import Path
from typing import Any

log = logging.getLogger(__name__)

_DB_PATH = Path.home() / ".quill" / "history.db"


def _get_conn() -> sqlite3.Connection:
    _DB_PATH.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(str(_DB_PATH))
    conn.row_factory = sqlite3.Row
    return conn


def init_db() -> None:
    with _get_conn() as conn:
        conn.executescript("""
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
                favorited         INTEGER DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_history_ts   ON history(timestamp);
            CREATE INDEX IF NOT EXISTS idx_history_mode ON history(mode);
            CREATE INDEX IF NOT EXISTS idx_history_lang ON history(language);
            CREATE INDEX IF NOT EXISTS idx_history_fav  ON history(favorited);

            CREATE TABLE IF NOT EXISTS tutor_lessons (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at   DATETIME DEFAULT CURRENT_TIMESTAMP,
                period       TEXT NOT NULL,
                language     TEXT,
                lesson_md    TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_lessons_period ON tutor_lessons(period, created_at);
        """)
        # Migrate existing databases that predate the favorited column
        try:
            conn.execute("ALTER TABLE history ADD COLUMN favorited INTEGER DEFAULT 0")
        except Exception:
            pass  # Column already exists


def save_entry(
    original: str,
    output: str,
    mode: str,
    language: str = "auto",
    app_hint: str = "",
    persona_tone: str = "natural",
) -> int:
    """Save a transformation to history. Returns the new row id."""
    wc_before = len(original.split())
    wc_after  = len(output.split())
    with _get_conn() as conn:
        cur = conn.execute(
            """INSERT INTO history
               (app_hint, mode, language, persona_tone,
                original_text, output_text, word_count_before, word_count_after)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)""",
            (app_hint, mode, language, persona_tone,
             original, output, wc_before, wc_after),
        )
        return cur.lastrowid


def save_tutor_explanation(entry_id: int, explanation: str) -> None:
    with _get_conn() as conn:
        conn.execute(
            "UPDATE history SET tutor_explanation = ? WHERE id = ?",
            (explanation, entry_id),
        )


def get_recent(limit: int = 50, language: str | None = None) -> list[dict]:
    with _get_conn() as conn:
        if language and language != "auto":
            rows = conn.execute(
                "SELECT * FROM history WHERE language = ? ORDER BY timestamp DESC LIMIT ?",
                (language, limit),
            ).fetchall()
        else:
            rows = conn.execute(
                "SELECT * FROM history ORDER BY timestamp DESC LIMIT ?",
                (limit,),
            ).fetchall()
    return [dict(r) for r in rows]


def get_stats(days: int = 7) -> dict[str, Any]:
    """Return usage stats for the last N days — used to generate lessons."""
    since = (datetime.utcnow() - timedelta(days=days)).isoformat()
    with _get_conn() as conn:
        rows = conn.execute(
            "SELECT * FROM history WHERE timestamp >= ?", (since,)
        ).fetchall()

    entries = [dict(r) for r in rows]
    if not entries:
        return {"count": 0, "days": days}

    mode_counts: dict[str, int] = {}
    lang_counts: dict[str, int] = {}
    total_before = total_after = 0

    for e in entries:
        mode_counts[e["mode"]] = mode_counts.get(e["mode"], 0) + 1
        lang = e["language"] or "auto"
        lang_counts[lang] = lang_counts.get(lang, 0) + 1
        total_before += e["word_count_before"] or 0
        total_after  += e["word_count_after"]  or 0

    return {
        "count":       len(entries),
        "days":        days,
        "mode_counts": mode_counts,
        "lang_counts": lang_counts,
        "avg_reduction": round(1 - total_after / max(total_before, 1), 2),
        "top_mode":    max(mode_counts, key=mode_counts.get) if mode_counts else None,
        "top_language": max(lang_counts, key=lang_counts.get) if lang_counts else None,
        "sample_originals": [e["original_text"][:200] for e in entries[:5]],
        "sample_outputs":   [e["output_text"][:200]   for e in entries[:5]],
    }


def save_lesson(period: str, lesson_md: str, language: str = "") -> None:
    with _get_conn() as conn:
        conn.execute(
            "INSERT INTO tutor_lessons (period, language, lesson_md) VALUES (?, ?, ?)",
            (period, language, lesson_md),
        )


def get_latest_lesson(period: str) -> dict | None:
    with _get_conn() as conn:
        row = conn.execute(
            "SELECT * FROM tutor_lessons WHERE period = ? ORDER BY created_at DESC LIMIT 1",
            (period,),
        ).fetchone()
    return dict(row) if row else None


def toggle_favorite(entry_id: int) -> bool:
    """Flip the favorited flag for an entry. Returns the new favorited state."""
    with _get_conn() as conn:
        row = conn.execute("SELECT favorited FROM history WHERE id = ?", (entry_id,)).fetchone()
        if not row:
            return False
        new_val = 0 if row["favorited"] else 1
        conn.execute("UPDATE history SET favorited = ? WHERE id = ?", (new_val, entry_id))
        return bool(new_val)


def get_favorites(limit: int = 100) -> list[dict]:
    with _get_conn() as conn:
        rows = conn.execute(
            "SELECT * FROM history WHERE favorited = 1 ORDER BY timestamp DESC LIMIT ?",
            (limit,),
        ).fetchall()
    return [dict(r) for r in rows]


def get_all_entries() -> list[dict]:
    """Return all history entries for export."""
    with _get_conn() as conn:
        rows = conn.execute("SELECT * FROM history ORDER BY timestamp DESC").fetchall()
    return [dict(r) for r in rows]
