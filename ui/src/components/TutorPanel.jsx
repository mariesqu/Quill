/**
 * AI Tutor Panel — full-page view with:
 *   Lessons tab  — daily/weekly AI-generated lessons from usage patterns
 *   History tab  — scrollable log of all past transformations with diffs
 */
import React, { useState, useEffect, useCallback } from "react";
import "../styles/tutor.css";
import DiffView from "./DiffView";

// ── Simple markdown renderer (no library dependency) ──────────────────────────
function renderMarkdown(md) {
  if (!md) return null;
  const lines = md.split("\n");
  const elements = [];
  let key = 0;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (line.startsWith("# "))  { elements.push(<h1 key={key++}>{line.slice(2)}</h1>); continue; }
    if (line.startsWith("## ")) { elements.push(<h2 key={key++}>{line.slice(3)}</h2>); continue; }
    if (line.startsWith("- "))  { elements.push(<li key={key++}>{inlineFormat(line.slice(2))}</li>); continue; }
    if (line.trim() === "")     { elements.push(<br key={key++} />); continue; }
    elements.push(<p key={key++}>{inlineFormat(line)}</p>);
  }
  return elements;
}

function inlineFormat(text) {
  // **bold** → <strong>, *italic* → <em>, `code` → <code>
  const parts = text.split(/(\*\*[^*]+\*\*|\*[^*]+\*|`[^`]+`)/g);
  return parts.map((part, i) => {
    if (part.startsWith("**") && part.endsWith("**"))
      return <strong key={i}>{part.slice(2, -2)}</strong>;
    if (part.startsWith("*") && part.endsWith("*"))
      return <em key={i}>{part.slice(1, -1)}</em>;
    if (part.startsWith("`") && part.endsWith("`"))
      return <code key={i}>{part.slice(1, -1)}</code>;
    return part;
  });
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function formatTs(ts) {
  if (!ts) return "";
  try {
    return new Date(ts).toLocaleString(undefined, {
      month: "short", day: "numeric",
      hour: "2-digit", minute: "2-digit",
    });
  } catch { return ts; }
}

function modeLabelFor(mode) {
  if (mode?.startsWith("chain:")) return mode.replace("chain:", "⛓ ");
  return mode || "?";
}

// ── Lesson tab ────────────────────────────────────────────────────────────────

function LessonsTab({ bridge }) {
  const [dailyLesson, setDailyLesson]   = useState(null);
  const [weeklyLesson, setWeeklyLesson] = useState(null);
  const [loading, setLoading]           = useState(null); // 'daily' | 'weekly' | null

  const generate = useCallback(async (period) => {
    setLoading(period);
    await bridge.generateLesson(period);
  }, [bridge]);

  useEffect(() => {
    const unsub = bridge.onTutorLesson((lesson, period) => {
      if (period === "daily")  setDailyLesson(lesson);
      if (period === "weekly") setWeeklyLesson(lesson);
      setLoading(null);
    });
    return unsub;
  }, [bridge]);

  return (
    <>
      {/* Daily */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <div style={{ fontSize: 11, fontWeight: 700, textTransform: "uppercase",
          letterSpacing: "0.08em", color: "var(--color-text-dim)" }}>
          Daily insight
        </div>
        <button className="btn-generate-lesson" onClick={() => generate("daily")}
          disabled={loading === "daily"}>
          {loading === "daily" ? (
            <><span style={{ animation: "spin 1s linear infinite", display: "inline-block" }}>⟳</span> Generating…</>
          ) : (
            <><span>✨</span> Generate today's</>
          )}
        </button>
      </div>

      {dailyLesson ? (
        <LessonCard lesson={dailyLesson} period="daily" />
      ) : (
        <EmptyLesson period="daily" onGenerate={() => generate("daily")} loading={loading === "daily"} />
      )}

      {/* Weekly */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginTop: 8 }}>
        <div style={{ fontSize: 11, fontWeight: 700, textTransform: "uppercase",
          letterSpacing: "0.08em", color: "var(--color-text-dim)" }}>
          Weekly review
        </div>
        <button className="btn-generate-lesson" onClick={() => generate("weekly")}
          disabled={loading === "weekly"}
          style={{ background: "linear-gradient(135deg,#0ea5e9,#38bdf8)",
            boxShadow: "0 4px 12px rgba(14,165,233,0.3)" }}>
          {loading === "weekly" ? (
            <><span style={{ animation: "spin 1s linear infinite", display: "inline-block" }}>⟳</span> Generating…</>
          ) : (
            <><span>📅</span> This week's review</>
          )}
        </button>
      </div>

      {weeklyLesson ? (
        <LessonCard lesson={weeklyLesson} period="weekly" />
      ) : (
        <EmptyLesson period="weekly" onGenerate={() => generate("weekly")} loading={loading === "weekly"} />
      )}
    </>
  );
}

function LessonCard({ lesson, period }) {
  return (
    <div className="lesson-card">
      <div className="lesson-card-header">
        <div className="lesson-card-title">
          {period === "daily" ? "📝" : "📅"} Your {period} insight
        </div>
        <span className="lesson-period-badge">{period}</span>
      </div>
      <div className="lesson-card-body">
        {renderMarkdown(lesson)}
      </div>
      <div className="lesson-card-footer">
        Generated just now · Based on your Quill usage
      </div>
    </div>
  );
}

function EmptyLesson({ period, onGenerate, loading }) {
  return (
    <div className="history-empty" style={{ padding: "24px", background: "var(--color-surface-2)",
      border: "1px solid var(--color-border)", borderRadius: "var(--radius-lg)" }}>
      <span style={{ fontSize: 28 }}>{period === "daily" ? "📝" : "📅"}</span>
      <span>No {period} lesson yet.</span>
      <span style={{ fontSize: 12, opacity: 0.6, maxWidth: 280, textAlign: "center" }}>
        Use Quill a few times, then generate your personalised lesson above.
      </span>
    </div>
  );
}

// ── History tab ───────────────────────────────────────────────────────────────

function HistoryTab({ bridge }) {
  const [entries, setEntries]       = useState([]);
  const [expanded, setExpanded]     = useState(null);
  const [showDiff, setShowDiff]     = useState({});
  const [explaining, setExplaining] = useState(null);

  useEffect(() => {
    bridge.getHistory(50);
    const unsub = bridge.onHistory((e) => setEntries(e));
    return unsub;
  }, [bridge]);

  useEffect(() => {
    const unsub = bridge.onTutorExplanation((explanation, entryId) => {
      setEntries((prev) =>
        prev.map((e) => e.id === entryId ? { ...e, tutor_explanation: explanation } : e)
      );
      setExplaining(null);
    });
    return unsub;
  }, [bridge]);

  const requestExplain = async (entry) => {
    setExplaining(entry.id);
    await bridge.tutorExplain(entry.id);
  };

  if (entries.length === 0) {
    return (
      <div className="history-empty">
        <span style={{ fontSize: 32 }}>📜</span>
        <span>No history yet</span>
        <span style={{ fontSize: 12, opacity: 0.6 }}>
          Enable history in Settings → My Voice → History, then start transforming text.
        </span>
      </div>
    );
  }

  return (
    <div className="history-list">
      {entries.map((entry) => (
        <div key={entry.id} className="history-entry">
          <div className="history-entry-header"
            onClick={() => setExpanded(expanded === entry.id ? null : entry.id)}>
            <span className="history-mode-badge">{modeLabelFor(entry.mode)}</span>
            {entry.language && entry.language !== "auto" && (
              <span className="history-lang-badge">→ {entry.language}</span>
            )}
            {entry.app_hint && (
              <span style={{ fontSize: 11, color: "var(--color-text-dim)" }}>{entry.app_hint}</span>
            )}
            <span className="history-ts">{formatTs(entry.timestamp)}</span>
            <span style={{ fontSize: 11, color: "var(--color-text-dim)", marginLeft: 4 }}>
              {expanded === entry.id ? "▲" : "▼"}
            </span>
          </div>

          {expanded === entry.id && (
            <>
              <div className="history-entry-texts">
                <div className="history-col">
                  <div className="history-col-label">Original</div>
                  <div className="history-col-text">{entry.original_text}</div>
                </div>
                <div className="history-col">
                  <div className="history-col-label">Output</div>
                  <div className="history-col-text">{entry.output_text}</div>
                </div>
              </div>

              {/* Diff toggle */}
              <div style={{ padding: "8px 14px", display: "flex", gap: 8,
                borderTop: "1px solid var(--color-border)", flexWrap: "wrap" }}>
                <button className="btn-copy"
                  style={{ fontSize: 11, padding: "4px 10px" }}
                  onClick={() => setShowDiff((s) => ({ ...s, [entry.id]: !s[entry.id] }))}>
                  {showDiff[entry.id] ? "Hide diff" : "Show diff"}
                </button>
                <button className="btn-copy"
                  style={{ fontSize: 11, padding: "4px 10px",
                    color: "var(--color-primary)",
                    borderColor: "rgba(124,110,247,0.3)" }}
                  onClick={() => requestExplain(entry)}
                  disabled={explaining === entry.id}>
                  {explaining === entry.id ? "⟳ Explaining…" :
                    entry.tutor_explanation ? "↻ Re-explain" : "💡 Explain changes"}
                </button>
              </div>

              {showDiff[entry.id] && (
                <DiffView original={entry.original_text} transformed={entry.output_text} />
              )}

              {entry.tutor_explanation && (
                <div className="history-explanation">
                  <strong style={{ color: "var(--color-primary)", fontSize: 11 }}>💡 Tutor insight</strong>
                  <br /><br />
                  {entry.tutor_explanation}
                </div>
              )}
            </>
          )}
        </div>
      ))}
    </div>
  );
}

// ── Main panel ────────────────────────────────────────────────────────────────

export default function TutorPanel({ onClose, bridge }) {
  const [tab, setTab] = useState("lessons");

  const TABS = [
    { id: "lessons", label: "📚 Lessons" },
    { id: "history", label: "📜 History" },
  ];

  return (
    <div className="tutor-root">
      <div className="tutor-topbar">
        <div className="tutor-topbar-title">
          🎓 AI Tutor
        </div>
        <button className="overlay-close-btn" onClick={onClose}
          style={{ width: 28, height: 28 }}>✕</button>
      </div>

      <div className="tutor-tabs">
        {TABS.map((t) => (
          <button key={t.id}
            className={`tutor-tab ${tab === t.id ? "active" : ""}`}
            onClick={() => setTab(t.id)}>
            {t.label}
          </button>
        ))}
      </div>

      <div className="tutor-body">
        {tab === "lessons" && <LessonsTab bridge={bridge} />}
        {tab === "history" && <HistoryTab bridge={bridge} />}
      </div>
    </div>
  );
}
