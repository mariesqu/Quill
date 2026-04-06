import React, { useState, useEffect, useCallback } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import DiffView from "./DiffView";

const startDrag = (e) => {
  // Only drag on left-click on the drag target itself (not buttons inside it)
  if (e.button === 0 && e.target === e.currentTarget) getCurrentWindow().startDragging();
};
import ComparisonView from "./ComparisonView";
import { fleschKincaid, gradeLabel } from "../utils/readability";
import { detectLanguage } from "../utils/detectLanguage";
import "../styles/overlay.css";
import "../styles/diff.css";

// ── Constants ─────────────────────────────────────────────────────────────────

const MODE_COLORS = {
  rewrite:     "#7c6ef7",
  translate:   "#38bdf8",
  coach:       "#fb923c",
  shorter:     "#a78bfa",
  formal:      "#64748b",
  fix_grammar: "#34d399",
  expand:      "#f472b6",
};

const QUICK_LANGUAGES = [
  { code: "auto",       label: "Auto",       flag: "🔤" },
  { code: "French",     label: "French",     flag: "🇫🇷" },
  { code: "Spanish",    label: "Spanish",    flag: "🇪🇸" },
  { code: "German",     label: "German",     flag: "🇩🇪" },
  { code: "Portuguese", label: "Portuguese", flag: "🇵🇹" },
  { code: "Italian",    label: "Italian",    flag: "🇮🇹" },
  { code: "Japanese",   label: "Japanese",   flag: "🇯🇵" },
  { code: "Chinese",    label: "Chinese",    flag: "🇨🇳" },
  { code: "Arabic",     label: "Arabic",     flag: "🇸🇦" },
  { code: "Dutch",      label: "Dutch",      flag: "🇳🇱" },
  { code: "Korean",     label: "Korean",     flag: "🇰🇷" },
];

// ── Sub-components ────────────────────────────────────────────────────────────

function ContextBadge({ context, detectedLang }) {
  return (
    <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
      {context?.hint && (
        <div className="overlay-context-badge">
          <div className="overlay-context-dot" />
          {context.hint}
        </div>
      )}
      {detectedLang && (
        <div className="overlay-context-badge" style={{ color: "var(--color-text-dim)", borderColor: "rgba(255,255,255,0.06)" }}>
          🌐 {detectedLang}
        </div>
      )}
    </div>
  );
}

function ReadabilityBadge({ text, label }) {
  const grade = fleschKincaid(text);
  const info  = gradeLabel(grade);
  if (!info) return null;
  return (
    <span
      title={`Flesch-Kincaid Grade Level: ${grade}`}
      style={{
        fontSize: 10,
        fontWeight: 700,
        padding: "2px 6px",
        borderRadius: 4,
        background: `${info.color}18`,
        color: info.color,
        border: `1px solid ${info.color}40`,
        letterSpacing: "0.04em",
        cursor: "default",
      }}
    >
      {label || info.label}
    </span>
  );
}

function WordCount({ original, transformed }) {
  if (!transformed) return null;
  const before = original.trim().split(/\s+/).filter(Boolean).length;
  const after  = transformed.trim().split(/\s+/).filter(Boolean).length;
  const delta  = after - before;
  return (
    <span className="word-count-badge">
      {before}→{after}w
      {delta !== 0 && (
        <span style={{ color: delta < 0 ? "var(--color-success)" : "var(--color-text-dim)" }}>
          {" "}{delta > 0 ? "+" : ""}{delta}
        </span>
      )}
    </span>
  );
}

function SmartSuggestion({ suggestion, modes, onSelect, isStreaming }) {
  if (!suggestion) return null;
  const mode = modes.find((m) => m.id === suggestion.mode_id);
  if (!mode) return null;
  return (
    <button
      className="smart-suggestion"
      onClick={() => onSelect(suggestion.mode_id)}
      disabled={isStreaming}
      title={suggestion.reason}
    >
      <span>💡</span>
      <span>Try <strong>{mode.label}</strong></span>
      <span className="smart-suggestion-reason">{suggestion.reason}</span>
    </button>
  );
}

function LanguagePicker({ value, onChange, disabled }) {
  const [showCustom, setShowCustom] = useState(false);
  const [custom, setCustom]         = useState("");
  const isCustom = !QUICK_LANGUAGES.find((l) => l.code === value);

  const handleQuick = (code) => { setShowCustom(false); onChange(code); };
  const handleCustomSubmit = (e) => {
    e.preventDefault();
    const sanitized = custom.trim().replace(/[^a-zA-Z\s\-]/g, "").slice(0, 50);
    if (sanitized) { onChange(sanitized); setShowCustom(false); setCustom(""); }
  };

  return (
    <div className="language-picker">
      <span className="language-picker-label">Output in</span>
      <div className="language-chips">
        {QUICK_LANGUAGES.map((lang) => (
          <button key={lang.code}
            className={`lang-chip ${value === lang.code ? "active" : ""}`}
            onClick={() => handleQuick(lang.code)}
            disabled={disabled} title={lang.label}>
            <span className="lang-flag">{lang.flag}</span>
            <span className="lang-name">{lang.label}</span>
          </button>
        ))}
        {showCustom ? (
          <form onSubmit={handleCustomSubmit} className="lang-custom-form">
            <input className="lang-custom-input" type="text" placeholder="e.g. Polish"
              value={custom} onChange={(e) => setCustom(e.target.value)} autoFocus
              onBlur={() => { if (!custom.trim()) setShowCustom(false); }} />
          </form>
        ) : (
          <button className={`lang-chip ${isCustom ? "active" : ""}`}
            onClick={() => setShowCustom(true)} disabled={disabled}>
            {isCustom
              ? <><span className="lang-flag">🌐</span><span className="lang-name">{value}</span></>
              : <span className="lang-name">+ Other</span>}
          </button>
        )}
      </div>
    </div>
  );
}

function InstructionField({ value, onChange, disabled }) {
  const [open, setOpen] = useState(false);
  if (!open) {
    return (
      <button className="instruction-toggle" onClick={() => setOpen(true)} disabled={disabled}>
        <span>✍️</span> Add instruction…
      </button>
    );
  }
  return (
    <div className="instruction-field">
      <input
        className="instruction-input"
        type="text"
        placeholder="e.g. make it more urgent, keep under 50 words…"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        autoFocus
        onKeyDown={(e) => {
          if (e.key === "Escape") { setOpen(false); onChange(""); }
        }}
      />
      {value && (
        <button className="instruction-clear" onClick={() => { onChange(""); setOpen(false); }}
          title="Clear">✕</button>
      )}
    </div>
  );
}

function TemplatePicker({ templates, onSelect, disabled }) {
  if (!templates || templates.length === 0) return null;
  return (
    <div style={{ padding: "0 16px 8px", display: "flex", gap: 6, flexWrap: "wrap" }}>
      {templates.map((tpl) => (
        <button
          key={tpl.name}
          onClick={() => onSelect(tpl)}
          disabled={disabled}
          title={`${tpl.mode}: ${tpl.instruction}`}
          style={{
            padding: "4px 10px",
            background: "var(--color-surface-2)",
            border: "1px solid var(--color-border)",
            borderRadius: "var(--radius-sm)",
            color: "var(--color-text-muted)",
            fontSize: 11,
            cursor: "pointer",
            display: "flex",
            alignItems: "center",
            gap: 4,
          }}
        >
          <span>⚡</span> {tpl.name}
        </button>
      ))}
    </div>
  );
}

function ModeBar({ modes, chains, activeMode, isStreaming, onSelectMode, onSelectChain,
  suggestion, compareMode, comparePick, onComparePick }) {
  return (
    <div className="overlay-modes">
      {modes.map((mode, i) => {
        const isSuggested  = suggestion?.mode_id === mode.id && !activeMode;
        const isPickedForCompare = compareMode && comparePick.includes(mode.id);
        return (
          <button key={mode.id}
            className={`mode-btn ${activeMode === mode.id ? "active" : ""} ${isSuggested ? "suggested" : ""} ${isPickedForCompare ? "compare-picked" : ""}`}
            onClick={() => compareMode ? onComparePick(mode.id) : onSelectMode(mode.id)}
            disabled={isStreaming}
            title={compareMode ? `Compare: ${mode.label}` : `${mode.label}  [${i + 1}]`}
            style={activeMode === mode.id ? {
              background: `${MODE_COLORS[mode.id] || "#7c6ef7"}18`,
              borderColor: `${MODE_COLORS[mode.id] || "#7c6ef7"}60`,
              color: MODE_COLORS[mode.id] || "#7c6ef7",
            } : isPickedForCompare ? {
              background: `${MODE_COLORS[mode.id] || "#7c6ef7"}25`,
              borderColor: `${MODE_COLORS[mode.id] || "#7c6ef7"}80`,
              color: MODE_COLORS[mode.id] || "#7c6ef7",
            } : {}}>
            <span className="mode-btn-icon">{mode.icon}</span>
            {mode.label}
            <span className="mode-shortcut">{i + 1}</span>
            {isSuggested && <span className="mode-suggested-dot" />}
            {isPickedForCompare && <span style={{ fontSize: 9, marginLeft: 2 }}>✓</span>}
          </button>
        );
      })}
      {!compareMode && chains.map((chain) => (
        <button key={chain.id}
          className={`mode-btn mode-btn-chain ${activeMode === `chain:${chain.id}` ? "active" : ""}`}
          onClick={() => onSelectChain(chain.id)}
          disabled={isStreaming}
          title={chain.description || chain.label}>
          <span className="mode-btn-icon">{chain.icon}</span>
          {chain.label}
        </button>
      ))}
    </div>
  );
}

function ChainProgress({ chainProgress, modes }) {
  if (!chainProgress) return null;
  const mode = modes.find((m) => m.id === chainProgress.mode);
  return (
    <div className="chain-progress">
      <div className="chain-progress-steps">
        {Array.from({ length: chainProgress.total }).map((_, i) => (
          <div key={i} className={`chain-step-dot ${i < chainProgress.step ? "done" : i === chainProgress.step - 1 ? "active" : ""}`} />
        ))}
      </div>
      <span>Step {chainProgress.step}/{chainProgress.total}: {mode?.label || chainProgress.mode}…</span>
    </div>
  );
}

function StreamingIndicator({ activeMode, modes, chains, language }) {
  const mode  = modes.find((m) => m.id === activeMode);
  const chain = chains.find((c) => `chain:${c.id}` === activeMode);
  const label = mode?.label || chain?.label || "Working";
  const langSuffix = language && language !== "auto" ? ` → ${language}` : "";
  return (
    <div className="streaming-indicator">
      <div className="streaming-dots">
        <div className="streaming-dot" /><div className="streaming-dot" /><div className="streaming-dot" />
      </div>
      {label}{langSuffix}…
    </div>
  );
}

function OutputArea({ streamedText, isStreaming, activeMode, selectedText, showDiff }) {
  if (!activeMode && !streamedText) {
    return (
      <div className="overlay-output">
        <div className="overlay-output-empty">
          <span className="overlay-output-hint">Pick a mode above or press <kbd style={{
            padding: "1px 5px", borderRadius: 4, fontSize: 10, fontFamily: "var(--font-mono)",
            background: "rgba(255,255,255,0.06)", border: "1px solid var(--color-border)",
          }}>1</kbd>–<kbd style={{
            padding: "1px 5px", borderRadius: 4, fontSize: 10, fontFamily: "var(--font-mono)",
            background: "rgba(255,255,255,0.06)", border: "1px solid var(--color-border)",
          }}>7</kbd></span>
        </div>
      </div>
    );
  }
  if (showDiff && streamedText && selectedText) {
    return <DiffView original={selectedText} transformed={streamedText} />;
  }
  return (
    <div className="overlay-output">
      <p className="streaming-text">
        {streamedText}
        {isStreaming && <span className="streaming-cursor" />}
      </p>
    </div>
  );
}

function TutorExplainPanel({ explanation, isExplaining, onRequest, isDone }) {
  if (!isDone) return null;
  if (isExplaining) {
    return (
      <div className="tutor-explain-loading">
        <div className="streaming-dots" style={{ display: "flex", gap: 3 }}>
          <div className="streaming-dot" /><div className="streaming-dot" /><div className="streaming-dot" />
        </div>
        AI Tutor is analysing the changes…
      </div>
    );
  }
  if (!explanation) {
    return (
      <button className="tutor-explain-trigger" onClick={onRequest}>
        💡 Explain what changed & why
      </button>
    );
  }
  return (
    <div className="tutor-explain-panel">
      <div className="tutor-explain-header">
        <div className="tutor-explain-title"><span>💡</span> AI Tutor insight</div>
      </div>
      <div className="tutor-explain-body">{explanation}</div>
    </div>
  );
}

function PronunciationPanel({ pronunciation, isPronouncing, isDone, activeMode, language, onRequest }) {
  const isTranslateMode = typeof activeMode === "string" && activeMode.includes("translate");
  if (!isDone || !isTranslateMode) return null;
  if (isPronouncing) {
    return (
      <div className="tutor-explain-loading">
        <div className="streaming-dots" style={{ display: "flex", gap: 3 }}>
          <div className="streaming-dot" /><div className="streaming-dot" /><div className="streaming-dot" />
        </div>
        Getting pronunciation…
      </div>
    );
  }
  if (!pronunciation) {
    return (
      <button className="tutor-explain-trigger" onClick={onRequest}
        style={{ borderColor: "rgba(56,189,248,0.3)", color: "#38bdf8" }}>
        🔊 Show pronunciation guide
      </button>
    );
  }
  return (
    <div className="tutor-explain-panel" style={{ borderColor: "rgba(56,189,248,0.2)" }}>
      <div className="tutor-explain-header">
        <div className="tutor-explain-title" style={{ color: "#38bdf8" }}><span>🔊</span> Pronunciation</div>
      </div>
      <div className="tutor-explain-body">{pronunciation}</div>
    </div>
  );
}

function ClipboardToast({ text, onDismiss, onUse }) {
  if (!text) return null;
  return (
    <div style={{
      position: "fixed",
      bottom: 16,
      right: 16,
      background: "var(--color-surface)",
      border: "1px solid var(--color-border-hover)",
      borderRadius: "var(--radius-md)",
      padding: "10px 14px",
      boxShadow: "var(--shadow-overlay)",
      display: "flex",
      alignItems: "center",
      gap: 10,
      fontSize: 12,
      zIndex: 9999,
      maxWidth: 320,
      animation: "slide-up 200ms ease",
    }}>
      <span>📋</span>
      <span style={{ flex: 1, color: "var(--color-text-muted)", overflow: "hidden",
        textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
        New clipboard text
      </span>
      <button onClick={onUse} style={{
        padding: "4px 10px",
        background: "var(--color-primary-dim)",
        border: "1px solid var(--color-primary-glow)",
        borderRadius: "var(--radius-sm)",
        color: "var(--color-primary)",
        fontSize: 11,
        cursor: "pointer",
      }}>Transform</button>
      <button onClick={onDismiss} style={{
        background: "none", border: "none", color: "var(--color-text-dim)",
        cursor: "pointer", fontSize: 13, padding: "2px 4px",
      }}>✕</button>
    </div>
  );
}

// ── Main overlay component ─────────────────────────────────────────────────────

export default function Overlay({ bridge, onOpenTutor }) {
  const {
    visible, selectedText, context, modes, chains,
    activeMode, streamedText, isStreaming, isDone,
    error, chainProgress, suggestion,
    outputLanguage, setOutputLanguage,
    tutorExplanation, isExplaining,
    comparisonResult, isComparing, compareMode, setCompareMode, compareModes,
    pronunciation, isPronouncing, getPronunciation,
    clipboardToast, dismissClipboardToast,
    templates,
    canUndo, undo,
    selectMode, selectChain, retry,
    setResultText, confirmReplace, dismiss, requestTutorExplain,
    lastEntryId,
  } = bridge;

  const [copied, setCopied]               = useState(false);
  const [showDiff, setShowDiff]           = useState(false);
  const [extraInstruction, setExtraInstruction] = useState("");
  const [comparePick, setComparePick]     = useState([]); // up to 2 mode ids
  // When user picks a side in comparison view, store the chosen text locally so
  // Replace/Copy work correctly without mutating bridge state.
  const [chosenText, setChosenText]       = useState(null);

  // Reset local display state when a new mode starts streaming
  useEffect(() => {
    if (isStreaming) {
      setShowDiff(false);
      setChosenText(null);
    }
  }, [isStreaming]);

  // Handle comparison mode picks
  const handleComparePick = useCallback((modeId) => {
    setComparePick((prev) => {
      if (prev.includes(modeId)) return prev.filter((m) => m !== modeId);
      const next = [...prev, modeId].slice(-2);
      if (next.length === 2) {
        compareModes(next[0], next[1], extraInstruction);
        return [];
      }
      return next;
    });
  }, [compareModes, extraInstruction]);

  // Exit compare mode when result arrives; clear any previous chosen text
  useEffect(() => {
    if (comparisonResult) { setCompareMode(false); setChosenText(null); }
  }, [comparisonResult, setCompareMode]);

  // Handle clipboard toast "Transform" click
  const handleClipboardUse = useCallback(() => {
    // Not yet visible — clipboard text will become the new selectedText when hotkey fires
    // Just dismiss for now; user still needs to trigger hotkey on the clipboard text
    dismissClipboardToast();
  }, [dismissClipboardToast]);

  // Keyboard shortcuts
  useEffect(() => {
    const onKey = (e) => {
      if (!visible) return;
      if (e.target.tagName === "INPUT" || e.target.tagName === "TEXTAREA") return;

      if (e.key === "Escape") { dismiss(); return; }

      // Ctrl+Z → undo
      if ((e.ctrlKey || e.metaKey) && e.key === "z" && canUndo) {
        e.preventDefault();
        undo();
        return;
      }

      // 1–7 → trigger mode by index
      const idx = parseInt(e.key, 10);
      if (!isNaN(idx) && idx >= 1 && idx <= modes.length && !isStreaming) {
        if (compareMode) handleComparePick(modes[idx - 1].id);
        else selectMode(modes[idx - 1].id, extraInstruction);
      }
      // r → retry
      if (e.key === "r" && isDone && !isStreaming) {
        retry(extraInstruction);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [visible, modes, isStreaming, isDone, extraInstruction, canUndo,
    compareMode, handleComparePick, selectMode, retry, dismiss, undo]);

  // The active output — chosen comparison side takes priority over streamed text
  const activeText = chosenText ?? streamedText;

  const handleCopy = useCallback(() => {
    if (!activeText) return;
    navigator.clipboard.writeText(activeText)
      .then(() => {
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
      })
      .catch((err) => console.error("Copy failed:", err));
  }, [activeText]);

  const handleModeSelect = useCallback((modeId) => {
    selectMode(modeId, extraInstruction);
  }, [selectMode, extraInstruction]);

  const handleChainSelect = useCallback((chainId) => {
    selectChain(chainId, extraInstruction);
  }, [selectChain, extraInstruction]);

  const handleTemplateSelect = useCallback((tpl) => {
    setExtraInstruction(tpl.instruction || "");
    selectMode(tpl.mode, tpl.instruction || "");
  }, [selectMode]);

  // User picked a side in comparison view — store locally so Replace/Copy act on it
  const handleComparisonUse = useCallback((text) => {
    setChosenText(text);
  }, []);

  // Detected language from selected text
  const detectedLang = selectedText ? detectLanguage(selectedText) : null;

  if (!visible) {
    return (
      <div onMouseDown={startDrag} style={{ height: "100vh", display: "flex",
        flexDirection: "column", alignItems: "center", justifyContent: "center", gap: 12,
        color: "var(--color-text-muted, rgba(240,240,248,0.5))", fontSize: 13,
        background: "var(--color-bg, rgba(20,20,30,0.92))", borderRadius: 12,
        cursor: "grab", position: "relative" }}>
        <button onClick={() => getCurrentWindow().hide()} style={{
          position: "absolute", top: 10, right: 14, background: "none",
          border: "none", color: "rgba(255,255,255,0.35)", cursor: "pointer",
          fontSize: 16, padding: 4 }} aria-label="Hide window">✕</button>
        <div style={{ fontSize: 32 }}>🪶</div>
        <div>Select text anywhere, then press</div>
        <kbd style={{ padding: "4px 12px", borderRadius: 6, fontSize: 12,
          background: "rgba(255,255,255,0.08)", border: "1px solid rgba(255,255,255,0.12)" }}>
          Ctrl+Shift+Space
        </kbd>
      </div>
    );
  }

  return (
    <>
      <div className="overlay-root">
        <div className="overlay-card">
          {/* Header — drag region */}
          <div className="overlay-header" onMouseDown={startDrag}>
            <div className="overlay-logo">
              <div className="overlay-logo-icon">🪶</div>
              Quill
            </div>
            <ContextBadge context={context} detectedLang={detectedLang} />
            <div style={{ display: "flex", gap: 4, alignItems: "center" }}>
              {onOpenTutor && (
                <button className="overlay-close-btn" onClick={onOpenTutor} title="AI Tutor"
                  style={{ width: 24, height: 24, fontSize: 13 }}>🎓</button>
              )}
              <button className="overlay-close-btn" onClick={dismiss} title="Dismiss (Esc)" aria-label="Close overlay">✕</button>
            </div>
          </div>

          {/* Source text + readability + word count */}
          <div className="overlay-source">
            <div className="overlay-source-label" style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 6 }}>
              <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                <span>Selected text</span>
                <ReadabilityBadge text={selectedText} />
              </div>
              <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                {isDone && streamedText && (
                  <ReadabilityBadge text={streamedText} label="→" />
                )}
                <WordCount original={selectedText} transformed={isDone ? streamedText : null} />
              </div>
            </div>
            <div className="overlay-source-text">{selectedText}</div>
          </div>

          {/* Language picker */}
          <LanguagePicker value={outputLanguage} onChange={setOutputLanguage} disabled={isStreaming} />

          {/* Smart suggestion */}
          {suggestion && !activeMode && !compareMode && (
            <SmartSuggestion suggestion={suggestion} modes={modes}
              onSelect={handleModeSelect} isStreaming={isStreaming} />
          )}

          {/* Compare mode hint */}
          {compareMode && (
            <div style={{ padding: "6px 16px", fontSize: 12,
              color: "var(--color-primary)", opacity: 0.85 }}>
              ⊞ Pick 2 modes to compare{comparePick.length > 0 ? ` (${comparePick.length}/2 selected)` : ""}
            </div>
          )}

          {/* One-off instruction field */}
          <div style={{ padding: "4px 16px 8px" }}>
            <InstructionField
              value={extraInstruction}
              onChange={setExtraInstruction}
              disabled={isStreaming}
            />
          </div>

          {/* Quick templates */}
          <TemplatePicker templates={templates} onSelect={handleTemplateSelect} disabled={isStreaming} />

          {/* Mode + chain bar */}
          <ModeBar modes={modes} chains={chains} activeMode={activeMode}
            isStreaming={isStreaming} onSelectMode={handleModeSelect}
            onSelectChain={handleChainSelect} suggestion={suggestion}
            compareMode={compareMode} comparePick={comparePick}
            onComparePick={handleComparePick} />

          {/* Chain progress */}
          {chainProgress && <ChainProgress chainProgress={chainProgress} modes={modes} />}

          {/* Comparison loading */}
          {isComparing && (
            <div className="streaming-indicator">
              <div className="streaming-dots">
                <div className="streaming-dot" /><div className="streaming-dot" /><div className="streaming-dot" />
              </div>
              Comparing modes…
            </div>
          )}

          {/* Streaming indicator */}
          {isStreaming && !chainProgress && (
            <StreamingIndicator activeMode={activeMode} modes={modes} chains={chains}
              language={outputLanguage !== "auto" ? outputLanguage : null} />
          )}

          {/* Error */}
          {error && (
            <div className="overlay-error"><span>⚠️</span><span>{error}</span></div>
          )}

          {/* Comparison result */}
          {comparisonResult && (
            <ComparisonView result={comparisonResult} modes={modes} onUse={handleComparisonUse} />
          )}

          {/* Normal output */}
          {!comparisonResult && (
            <OutputArea streamedText={streamedText} isStreaming={isStreaming}
              activeMode={activeMode} selectedText={selectedText} showDiff={showDiff} />
          )}

          {/* Pronunciation guide (translate mode only) */}
          {!comparisonResult && (
            <PronunciationPanel
              pronunciation={pronunciation}
              isPronouncing={isPronouncing}
              isDone={isDone}
              activeMode={activeMode}
              language={outputLanguage}
              onRequest={() => getPronunciation(streamedText, outputLanguage)}
            />
          )}

          {/* Tutor explain panel */}
          {!comparisonResult && (
            <TutorExplainPanel explanation={tutorExplanation} isExplaining={isExplaining}
              isDone={isDone} onRequest={() => requestTutorExplain(lastEntryId)} />
          )}

          {/* Action bar — clear text labels */}
          <div className="overlay-actions">
            <button className="btn-replace"
              onClick={async () => {
                if (chosenText) await setResultText(chosenText);
                confirmReplace();
              }}
              disabled={!isDone || (!activeText) || (!!comparisonResult && !chosenText)}>
              ↩ Replace
            </button>
            <button className="btn-copy" onClick={handleCopy}
              disabled={!activeText}>
              {copied ? "✓ Copied" : "Copy"}
            </button>
            {isDone && streamedText && !comparisonResult && (
              <button className={`btn-copy${showDiff ? " active" : ""}`}
                onClick={() => setShowDiff((v) => !v)}>
                Diff
              </button>
            )}
            <button className={`btn-copy${compareMode ? " active" : ""}`}
              onClick={() => { setCompareMode((v) => !v); setComparePick([]); }}
              disabled={isStreaming}
              style={compareMode ? { color: "var(--color-warning)", borderColor: "rgba(252,211,77,0.3)" } : undefined}>
              {compareMode ? "Cancel" : "Compare"}
            </button>
            {canUndo && (
              <button className="btn-copy" onClick={undo} disabled={isStreaming}>
                Undo
              </button>
            )}
            {(isDone || error) && !compareMode && (
              <button className="btn-copy" onClick={() => retry(extraInstruction)}
                disabled={isStreaming}>
                Retry
              </button>
            )}
          </div>

          {copied && <div className="copy-toast">Copied to clipboard</div>}
        </div>
      </div>

      {/* Clipboard toast — outside the main card */}
      <ClipboardToast
        text={clipboardToast}
        onDismiss={dismissClipboardToast}
        onUse={handleClipboardUse}
      />
    </>
  );
}
