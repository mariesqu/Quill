/**
 * AI Tutor Panel — full-page view with:
 *   Lessons tab  — daily/weekly AI-generated lessons from usage patterns
 *   History tab  — scrollable log of all past transformations with diffs, favorites, export
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

function triggerDownload(content, filename, mime) {
  const blob = new Blob([content], { type: mime });
  const url  = URL.createObjectURL(blob);
  const a    = document.createElement("a");
  a.href = url; a.download = filename; a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

// ── Lesson tab ────────────────────────────────────────────────────────────────

function LessonsTab({ bridge }) {
  const [dailyLesson, setDailyLesson]   = useState(null);
  const [weeklyLesson, setWeeklyLesson] = useState(null);
  const [loading, setLoading]           = useState(null);

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
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <div style={{ fontSize: 11, fontWeight: 700, textTransform: "uppercase",
          letterSpacing: "0.08em", color: "var(--color-text-dim)" }}>Daily insight</div>
        <button className="btn-generate-lesson" onClick={() => generate("daily")}
          disabled={loading === "daily"}>
          {loading === "daily"
            ? <><span style={{ animation: "spin 1s linear infinite", display: "inline-block" }}>⟳</span> Generating…</>
            : <><span>✨</span> Generate today's</>}
        </button>
      </div>

      {dailyLesson ? <LessonCard lesson={dailyLesson} period="daily" />
        : <EmptyLesson period="daily" loading={loading === "daily"} />}

      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginTop: 8 }}>
        <div style={{ fontSize: 11, fontWeight: 700, textTransform: "uppercase",
          letterSpacing: "0.08em", color: "var(--color-text-dim)" }}>Weekly review</div>
        <button className="btn-generate-lesson" onClick={() => generate("weekly")}
          disabled={loading === "weekly"}
          style={{ background: "linear-gradient(135deg,#0ea5e9,#38bdf8)",
            boxShadow: "0 4px 12px rgba(14,165,233,0.3)" }}>
          {loading === "weekly"
            ? <><span style={{ animation: "spin 1s linear infinite", display: "inline-block" }}>⟳</span> Generating…</>
            : <><span>📅</span> This week's review</>}
        </button>
      </div>

      {weeklyLesson ? <LessonCard lesson={weeklyLesson} period="weekly" />
        : <EmptyLesson period="weekly" loading={loading === "weekly"} />}
    </>
  );
}

function LessonCard({ lesson, period }) {
  return (
    <div className="lesson-card">
      <div className="lesson-card-header">
        <div className="lesson-card-title">{period === "daily" ? "📝" : "📅"} Your {period} insight</div>
        <span className="lesson-period-badge">{period}</span>
      </div>
      <div className="lesson-card-body">{renderMarkdown(lesson)}</div>
      <div className="lesson-card-footer">Generated just now · Based on your Quill usage</div>
    </div>
  );
}

function EmptyLesson({ period }) {
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
  const [entries, setEntries]           = useState([]);
  const [expanded, setExpanded]         = useState(null);
  const [showDiff, setShowDiff]         = useState({});
  const [explaining, setExplaining]     = useState(null);
  const [favoritesOnly, setFavoritesOnly] = useState(false);
  const [exportFmt, setExportFmt]       = useState("json");
  const [exporting, setExporting]       = useState(false);

  useEffect(() => {
    bridge.getHistory(100);

    const unsubHistory = bridge.onHistory((newEntries, favoriteUpdate) => {
      if (newEntries !== null) {
        setEntries(newEntries);
      } else if (favoriteUpdate) {
        setEntries((prev) =>
          prev.map((e) => e.id === favoriteUpdate.entry_id
            ? { ...e, favorited: favoriteUpdate.favorited ? 1 : 0 }
            : e
          )
        );
      }
    });

    return unsubHistory;
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

  const handleExport = useCallback(async () => {
    setExporting(true);
    await bridge.exportHistory(exportFmt);
  }, [bridge, exportFmt]);

  useEffect(() => {
    const unsub = bridge.onExportData((data, fmt) => {
      setExporting(false);
      const ts = new Date().toISOString().slice(0, 10);
      if (fmt === "csv") {
        const headers = ["id","timestamp","mode","language","original_text","output_text","favorited"];
        const rows = data.map((e) => headers.map((h) => JSON.stringify(e[h] ?? "")).join(","));
        triggerDownload([headers.join(","), ...rows].join("\n"), `quill-history-${ts}.csv`, "text/csv");
      } else {
        triggerDownload(JSON.stringify(data, null, 2), `quill-history-${ts}.json`, "application/json");
      }
    });
    return unsub;
  }, [bridge]);

  const requestExplain = async (entry) => {
    setExplaining(entry.id);
    await bridge.tutorExplain(entry.id);
  };

  const displayed = favoritesOnly ? entries.filter((e) => e.favorited) : entries;

  if (entries.length === 0) {
    return (
      <div className="history-empty">
        <span style={{ fontSize: 32 }}>📜</span>
        <span>No history yet</span>
        <span style={{ fontSize: 12, opacity: 0.6 }}>
          Enable history in Settings → AI Tutor, then start transforming text.
        </span>
      </div>
    );
  }

  return (
    <>
      {/* Toolbar */}
      <div style={{ display: "flex", gap: 8, alignItems: "center", padding: "0 0 12px", flexWrap: "wrap" }}>
        <button
          onClick={() => setFavoritesOnly((v) => !v)}
          style={{
            padding: "5px 12px",
            background: favoritesOnly ? "rgba(251,191,36,0.15)" : "var(--color-surface-2)",
            border: `1px solid ${favoritesOnly ? "rgba(251,191,36,0.5)" : "var(--color-border)"}`,
            borderRadius: "var(--radius-sm)",
            color: favoritesOnly ? "#fbbf24" : "var(--color-text-muted)",
            fontSize: 12, cursor: "pointer",
          }}
        >
          {favoritesOnly ? "★ Favorites" : "☆ All"}
        </button>
        <div style={{ flex: 1 }} />
        <select value={exportFmt} onChange={(e) => setExportFmt(e.target.value)}
          style={{ background: "var(--color-surface-2)", border: "1px solid var(--color-border)",
            borderRadius: "var(--radius-sm)", color: "var(--color-text-muted)",
            fontSize: 11, padding: "4px 8px" }}>
          <option value="json">JSON</option>
          <option value="csv">CSV</option>
        </select>
        <button onClick={handleExport} disabled={exporting}
          style={{ padding: "5px 12px", background: "var(--color-surface-2)",
            border: "1px solid var(--color-border)", borderRadius: "var(--radius-sm)",
            color: "var(--color-text-muted)", fontSize: 12, cursor: "pointer",
            display: "flex", alignItems: "center", gap: 4 }}>
          {exporting ? "⟳" : "↓"} Export
        </button>
      </div>

      <div className="history-list">
        {displayed.map((entry) => (
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
              {entry.favorited ? <span title="Favorited" style={{ fontSize: 13 }}>★</span> : null}
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

                <div style={{ padding: "8px 14px", display: "flex", gap: 8,
                  borderTop: "1px solid var(--color-border)", flexWrap: "wrap" }}>
                  <button className="btn-copy"
                    style={{ fontSize: 11, padding: "4px 10px" }}
                    onClick={() => setShowDiff((s) => ({ ...s, [entry.id]: !s[entry.id] }))}>
                    {showDiff[entry.id] ? "Hide diff" : "Show diff"}
                  </button>
                  <button className="btn-copy"
                    style={{ fontSize: 11, padding: "4px 10px",
                      color: "var(--color-primary)", borderColor: "rgba(124,110,247,0.3)" }}
                    onClick={() => requestExplain(entry)}
                    disabled={explaining === entry.id}>
                    {explaining === entry.id ? "⟳ Explaining…"
                      : entry.tutor_explanation ? "↻ Re-explain" : "💡 Explain changes"}
                  </button>
                  <button className="btn-copy"
                    style={{ fontSize: 11, padding: "4px 10px",
                      color: entry.favorited ? "#fbbf24" : undefined,
                      borderColor: entry.favorited ? "rgba(251,191,36,0.4)" : undefined }}
                    onClick={() => bridge.toggleFavorite(entry.id)}
                    title={entry.favorited ? "Remove from favorites" : "Save to favorites"}>
                    {entry.favorited ? "★ Saved" : "☆ Save"}
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
    </>
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
        <div className="tutor-topbar-title">🎓 AI Tutor</div>
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
