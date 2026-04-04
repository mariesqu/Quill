import React, { useState, useEffect, useCallback, useRef } from "react";
import DiffView from "./DiffView";
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

function ContextBadge({ context }) {
  if (!context?.hint) return null;
  return (
    <div className="overlay-context-badge">
      <div className="overlay-context-dot" />
      {context.hint}
    </div>
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
    if (custom.trim()) { onChange(custom.trim()); setShowCustom(false); setCustom(""); }
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

function InstructionField({ value, onChange, onSubmit, disabled }) {
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

function ModeBar({ modes, chains, activeMode, isStreaming, onSelectMode, onSelectChain, suggestion }) {
  return (
    <div className="overlay-modes">
      {modes.map((mode, i) => {
        const isSuggested = suggestion?.mode_id === mode.id && !activeMode;
        return (
          <button key={mode.id}
            className={`mode-btn ${activeMode === mode.id ? "active" : ""} ${isSuggested ? "suggested" : ""}`}
            onClick={() => onSelectMode(mode.id)}
            disabled={isStreaming}
            title={`${mode.label}  [${i + 1}]`}
            style={activeMode === mode.id ? {
              background: `${MODE_COLORS[mode.id] || "#7c6ef7"}18`,
              borderColor: `${MODE_COLORS[mode.id] || "#7c6ef7"}60`,
              color: MODE_COLORS[mode.id] || "#7c6ef7",
            } : {}}>
            <span className="mode-btn-icon">{mode.icon}</span>
            {mode.label}
            {isSuggested && <span className="mode-suggested-dot" />}
          </button>
        );
      })}
      {chains.map((chain) => (
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
          <span style={{ fontSize: 20 }}>✨</span>
          <span className="overlay-output-hint">Pick a mode above — or press 1–7</span>
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

// ── Main overlay component ─────────────────────────────────────────────────────

export default function Overlay({ bridge, onOpenTutor }) {
  const {
    visible, selectedText, context, modes, chains,
    activeMode, streamedText, isStreaming, isDone,
    error, chainProgress, suggestion,
    outputLanguage, setOutputLanguage,
    tutorExplanation, isExplaining,
    selectMode, selectChain, retry,
    confirmReplace, dismiss, requestTutorExplain,
  } = bridge;

  const [copied, setCopied]             = useState(false);
  const [showDiff, setShowDiff]         = useState(false);
  const [extraInstruction, setExtraInstruction] = useState("");

  // Reset diff view when a new mode starts
  useEffect(() => { if (isStreaming) setShowDiff(false); }, [isStreaming]);

  // Keyboard shortcuts
  useEffect(() => {
    const onKey = (e) => {
      if (!visible) return;
      if (e.target.tagName === "INPUT" || e.target.tagName === "TEXTAREA") return;

      if (e.key === "Escape") { dismiss(); return; }

      // 1–7 → trigger mode by index
      const idx = parseInt(e.key, 10);
      if (!isNaN(idx) && idx >= 1 && idx <= modes.length && !isStreaming) {
        selectMode(modes[idx - 1].id, extraInstruction);
      }
      // r → retry
      if (e.key === "r" && isDone && !isStreaming) {
        retry(extraInstruction);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [visible, modes, isStreaming, isDone, extraInstruction, selectMode, retry, dismiss]);

  const handleCopy = useCallback(() => {
    if (!streamedText) return;
    navigator.clipboard.writeText(streamedText).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  }, [streamedText]);

  const handleModeSelect = useCallback((modeId) => {
    selectMode(modeId, extraInstruction);
  }, [selectMode, extraInstruction]);

  const handleChainSelect = useCallback((chainId) => {
    selectChain(chainId, extraInstruction);
  }, [selectChain, extraInstruction]);

  if (!visible) {
    return (
      <div style={{ height: "100vh", display: "flex", alignItems: "center",
        justifyContent: "center", color: "rgba(240,240,248,0.18)", fontSize: 13 }}>
        Waiting for hotkey…
      </div>
    );
  }

  return (
    <div className="overlay-root">
      <div className="overlay-card">
        {/* Header */}
        <div className="overlay-header">
          <div className="overlay-logo">
            <div className="overlay-logo-icon">🪶</div>
            Quill
          </div>
          <ContextBadge context={context} />
          <div style={{ display: "flex", gap: 4, alignItems: "center" }}>
            {onOpenTutor && (
              <button className="overlay-close-btn" onClick={onOpenTutor} title="AI Tutor"
                style={{ width: 24, height: 24, fontSize: 13 }}>🎓</button>
            )}
            <button className="overlay-close-btn" onClick={dismiss} title="Dismiss (Esc)">✕</button>
          </div>
        </div>

        {/* Source text + word count */}
        <div className="overlay-source">
          <div className="overlay-source-label" style={{ display: "flex", justifyContent: "space-between" }}>
            <span>Selected text</span>
            <WordCount original={selectedText} transformed={isDone ? streamedText : null} />
          </div>
          <div className="overlay-source-text">{selectedText}</div>
        </div>

        {/* Language picker */}
        <LanguagePicker value={outputLanguage} onChange={setOutputLanguage} disabled={isStreaming} />

        {/* Smart suggestion */}
        {suggestion && !activeMode && (
          <SmartSuggestion suggestion={suggestion} modes={modes}
            onSelect={handleModeSelect} isStreaming={isStreaming} />
        )}

        {/* One-off instruction field */}
        <div style={{ padding: "4px 16px 8px" }}>
          <InstructionField
            value={extraInstruction}
            onChange={setExtraInstruction}
            disabled={isStreaming}
          />
        </div>

        {/* Mode + chain bar */}
        <ModeBar modes={modes} chains={chains} activeMode={activeMode}
          isStreaming={isStreaming} onSelectMode={handleModeSelect}
          onSelectChain={handleChainSelect} suggestion={suggestion} />

        {/* Chain progress */}
        {chainProgress && <ChainProgress chainProgress={chainProgress} modes={modes} />}

        {/* Streaming indicator */}
        {isStreaming && !chainProgress && (
          <StreamingIndicator activeMode={activeMode} modes={modes} chains={chains}
            language={outputLanguage !== "auto" ? outputLanguage : null} />
        )}

        {/* Error */}
        {error && (
          <div className="overlay-error"><span>⚠️</span><span>{error}</span></div>
        )}

        {/* Output */}
        <OutputArea streamedText={streamedText} isStreaming={isStreaming}
          activeMode={activeMode} selectedText={selectedText} showDiff={showDiff} />

        {/* Tutor explain panel */}
        <TutorExplainPanel explanation={tutorExplanation} isExplaining={isExplaining}
          isDone={isDone} onRequest={requestTutorExplain} />

        {/* Action bar */}
        <div className="overlay-actions">
          <button className="btn-replace" onClick={confirmReplace}
            disabled={!isDone || !streamedText}>
            ↩ Replace
          </button>
          <button className="btn-copy" onClick={handleCopy} disabled={!streamedText}>
            {copied ? "✓" : "⎘"}
          </button>
          {isDone && streamedText && (
            <button className="btn-copy" title="Toggle diff view"
              onClick={() => setShowDiff((v) => !v)}
              style={{ color: showDiff ? "var(--color-primary)" : undefined }}>
              ⊞
            </button>
          )}
          {(isDone || error) && (
            <button className="btn-copy" onClick={() => retry(extraInstruction)}
              title="Try again  [r]" disabled={isStreaming}>
              ↻
            </button>
          )}
        </div>

        {copied && <div className="copy-toast">Copied to clipboard</div>}
      </div>
    </div>
  );
}
