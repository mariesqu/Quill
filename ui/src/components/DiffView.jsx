/**
 * DiffView — word-level diff between original and transformed text.
 * Uses Myers LCS algorithm for accurate word-level diffing.
 * Green = added, Red/strikethrough = removed, neutral = unchanged.
 */
import React, { useMemo } from "react";
import "../styles/diff.css";

// ── LCS-based word diff ───────────────────────────────────────────────────────

function tokenize(text) {
  // Split into words + whitespace tokens so spacing is preserved
  return text.match(/\S+|\s+/g) || [];
}

function lcs(a, b) {
  const m = a.length, n = b.length;
  const dp = Array.from({ length: m + 1 }, () => new Int32Array(n + 1));
  for (let i = m - 1; i >= 0; i--) {
    for (let j = n - 1; j >= 0; j--) {
      dp[i][j] = a[i] === b[j]
        ? dp[i + 1][j + 1] + 1
        : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }
  return dp;
}

function diff(original, transformed) {
  const a = tokenize(original);
  const b = tokenize(transformed);
  const table = lcs(a, b);

  const ops = []; // { type: 'equal'|'delete'|'insert', value: string }
  let i = 0, j = 0;

  while (i < a.length || j < b.length) {
    if (i < a.length && j < b.length && a[i] === b[j]) {
      ops.push({ type: "equal", value: a[i] });
      i++; j++;
    } else if (j < b.length && (i >= a.length || table[i][j + 1] >= table[i + 1][j])) {
      ops.push({ type: "insert", value: b[j] });
      j++;
    } else {
      ops.push({ type: "delete", value: a[i] });
      i++;
    }
  }
  return ops;
}

// Merge consecutive same-type ops for cleaner rendering
function mergeOps(ops) {
  const merged = [];
  for (const op of ops) {
    const last = merged[merged.length - 1];
    if (last && last.type === op.type) {
      last.value += op.value;
    } else {
      merged.push({ ...op });
    }
  }
  return merged;
}

// ── Component ─────────────────────────────────────────────────────────────────

export default function DiffView({ original, transformed }) {
  const ops = useMemo(
    () => mergeOps(diff(original, transformed)),
    [original, transformed]
  );

  const insertCount = ops.filter((o) => o.type === "insert").length;
  const deleteCount = ops.filter((o) => o.type === "delete").length;
  const origWords   = original.split(/\s+/).filter(Boolean).length;
  const newWords    = transformed.split(/\s+/).filter(Boolean).length;
  const delta       = newWords - origWords;

  return (
    <div className="diff-view">
      <div className="diff-stats">
        {insertCount > 0 && (
          <span className="diff-stat diff-stat-add">+{insertCount} added</span>
        )}
        {deleteCount > 0 && (
          <span className="diff-stat diff-stat-del">−{deleteCount} removed</span>
        )}
        <span className="diff-stat diff-stat-words">
          {origWords}→{newWords} words
          {delta !== 0 && (
            <span style={{ color: delta < 0 ? "var(--color-success)" : "var(--color-text-dim)" }}>
              {" "}({delta > 0 ? "+" : ""}{delta})
            </span>
          )}
        </span>
      </div>

      <div className="diff-body">
        {ops.map((op, i) => {
          if (op.type === "equal") {
            return <span key={i} className="diff-equal">{op.value}</span>;
          }
          if (op.type === "insert") {
            return <span key={i} className="diff-insert">{op.value}</span>;
          }
          // delete — only show if non-whitespace
          if (op.value.trim()) {
            return <span key={i} className="diff-delete">{op.value}</span>;
          }
          return null;
        })}
      </div>
    </div>
  );
}
