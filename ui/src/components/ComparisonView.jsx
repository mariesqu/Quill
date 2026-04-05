/**
 * ComparisonView — shows two mode results side by side.
 * Lets the user copy or replace with whichever they prefer.
 */
import React, { useState } from "react";

const MODE_COLORS = {
  rewrite:     "#7c6ef7",
  translate:   "#38bdf8",
  coach:       "#fb923c",
  shorter:     "#a78bfa",
  formal:      "#64748b",
  fix_grammar: "#34d399",
  expand:      "#f472b6",
};

function wordCount(text) {
  return text?.trim().split(/\s+/).filter(Boolean).length ?? 0;
}

function ResultPanel({ modeId, modeLabel, result, onCopy, onUse, copied }) {
  const color = MODE_COLORS[modeId] || "#7c6ef7";
  return (
    <div style={{
      flex: 1,
      display: "flex",
      flexDirection: "column",
      background: "var(--color-surface-2)",
      border: `1px solid ${color}30`,
      borderRadius: "var(--radius-md)",
      overflow: "hidden",
      minWidth: 0,
    }}>
      {/* Header */}
      <div style={{
        padding: "10px 14px",
        borderBottom: "1px solid var(--color-border)",
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        background: `${color}10`,
      }}>
        <span style={{ fontSize: 12, fontWeight: 700, color, letterSpacing: "0.04em" }}>
          {modeLabel}
        </span>
        <span style={{ fontSize: 11, color: "var(--color-text-dim)" }}>
          {wordCount(result)}w
        </span>
      </div>

      {/* Body */}
      <div style={{
        flex: 1,
        padding: "12px 14px",
        fontSize: 13,
        lineHeight: 1.65,
        color: "var(--color-text)",
        overflowY: "auto",
        whiteSpace: "pre-wrap",
        wordBreak: "break-word",
        minHeight: 80,
      }}>
        {result}
      </div>

      {/* Actions */}
      <div style={{
        padding: "8px 12px",
        borderTop: "1px solid var(--color-border)",
        display: "flex",
        gap: 6,
      }}>
        <button
          onClick={onUse}
          style={{
            flex: 1,
            padding: "6px 10px",
            background: `${color}18`,
            border: `1px solid ${color}50`,
            borderRadius: "var(--radius-sm)",
            color,
            fontSize: 12,
            fontWeight: 600,
            cursor: "pointer",
            transition: "var(--transition-fast)",
          }}
        >
          ↩ Use this
        </button>
        <button
          onClick={onCopy}
          style={{
            padding: "6px 10px",
            background: "var(--color-surface)",
            border: "1px solid var(--color-border)",
            borderRadius: "var(--radius-sm)",
            color: "var(--color-text-muted)",
            fontSize: 12,
            cursor: "pointer",
          }}
        >
          {copied ? "✓" : "⎘"}
        </button>
      </div>
    </div>
  );
}

export default function ComparisonView({ result, modes, onUse }) {
  const [copiedA, setCopiedA] = useState(false);
  const [copiedB, setCopiedB] = useState(false);

  const modeA = modes.find((m) => m.id === result.mode_a);
  const modeB = modes.find((m) => m.id === result.mode_b);

  const copy = (text, setCopied) => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  };

  return (
    <div style={{ padding: "12px 16px" }}>
      <div style={{
        fontSize: 11,
        fontWeight: 700,
        textTransform: "uppercase",
        letterSpacing: "0.08em",
        color: "var(--color-text-dim)",
        marginBottom: 10,
      }}>
        ⊞ Comparison
      </div>
      <div style={{ display: "flex", gap: 10 }}>
        <ResultPanel
          modeId={result.mode_a}
          modeLabel={modeA?.label || result.mode_a}
          result={result.result_a}
          copied={copiedA}
          onCopy={() => copy(result.result_a, setCopiedA)}
          onUse={() => onUse(result.result_a)}
        />
        <ResultPanel
          modeId={result.mode_b}
          modeLabel={modeB?.label || result.mode_b}
          result={result.result_b}
          copied={copiedB}
          onCopy={() => copy(result.result_b, setCopiedB)}
          onUse={() => onUse(result.result_b)}
        />
      </div>
    </div>
  );
}
