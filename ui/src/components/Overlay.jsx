import React, { useState, useEffect, useCallback } from "react";
import "../styles/overlay.css";

const MODE_COLORS = {
  rewrite: "#7c6ef7",
  translate: "#38bdf8",
  coach: "#fb923c",
  shorter: "#a78bfa",
  formal: "#64748b",
  fix_grammar: "#34d399",
  expand: "#f472b6",
};

function ContextBadge({ context }) {
  if (!context?.hint) return null;
  return (
    <div className="overlay-context-badge">
      <div className="overlay-context-dot" />
      {context.hint}
    </div>
  );
}

function ModeBar({ modes, activeMode, isStreaming, onSelect }) {
  return (
    <div className="overlay-modes">
      {modes.map((mode) => (
        <button
          key={mode.id}
          className={`mode-btn ${activeMode === mode.id ? "active" : ""}`}
          onClick={() => onSelect(mode.id)}
          disabled={isStreaming}
          style={
            activeMode === mode.id
              ? {
                  "--mode-color": MODE_COLORS[mode.id] || "#7c6ef7",
                  background: `${MODE_COLORS[mode.id] || "#7c6ef7"}18`,
                  borderColor: `${MODE_COLORS[mode.id] || "#7c6ef7"}60`,
                  color: MODE_COLORS[mode.id] || "#7c6ef7",
                }
              : {}
          }
        >
          <span className="mode-btn-icon">{mode.icon}</span>
          {mode.label}
        </button>
      ))}
    </div>
  );
}

function StreamingIndicator({ activeMode, modes }) {
  const mode = modes.find((m) => m.id === activeMode);
  return (
    <div className="streaming-indicator">
      <div className="streaming-dots">
        <div className="streaming-dot" />
        <div className="streaming-dot" />
        <div className="streaming-dot" />
      </div>
      {mode ? `${mode.label}…` : "Working…"}
    </div>
  );
}

function OutputArea({ streamedText, isStreaming, isDone, activeMode }) {
  if (!activeMode && !streamedText) {
    return (
      <div className="overlay-output">
        <div className="overlay-output-empty">
          <span style={{ fontSize: 20 }}>✨</span>
          <span className="overlay-output-hint">
            Choose a mode above to transform your text
          </span>
        </div>
      </div>
    );
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

export default function Overlay({ bridge }) {
  const {
    visible,
    selectedText,
    context,
    modes,
    activeMode,
    streamedText,
    isStreaming,
    isDone,
    error,
    selectMode,
    confirmReplace,
    dismiss,
  } = bridge;

  const [copied, setCopied] = useState(false);

  // Escape key to dismiss
  useEffect(() => {
    const onKey = (e) => {
      if (e.key === "Escape" && visible) dismiss();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [visible, dismiss]);

  const handleCopy = useCallback(() => {
    if (!streamedText) return;
    navigator.clipboard.writeText(streamedText).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  }, [streamedText]);

  if (!visible) {
    return (
      <div
        style={{
          height: "100vh",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "rgba(240,240,248,0.2)",
          fontSize: 13,
        }}
      >
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
          <button className="overlay-close-btn" onClick={dismiss} title="Dismiss (Esc)">
            ✕
          </button>
        </div>

        {/* Source text preview */}
        <div className="overlay-source">
          <div className="overlay-source-label">Selected text</div>
          <div className="overlay-source-text">{selectedText}</div>
        </div>

        {/* Mode selector */}
        <ModeBar
          modes={modes}
          activeMode={activeMode}
          isStreaming={isStreaming}
          onSelect={selectMode}
        />

        {/* Streaming indicator */}
        {isStreaming && (
          <StreamingIndicator activeMode={activeMode} modes={modes} />
        )}

        {/* Error */}
        {error && (
          <div className="overlay-error">
            <span>⚠️</span>
            <span>{error}</span>
          </div>
        )}

        {/* Output */}
        <OutputArea
          streamedText={streamedText}
          isStreaming={isStreaming}
          isDone={isDone}
          activeMode={activeMode}
        />

        {/* Actions */}
        <div className="overlay-actions">
          <button
            className="btn-replace"
            onClick={confirmReplace}
            disabled={!isDone || !streamedText}
          >
            ↩ Replace
          </button>
          <button
            className="btn-copy"
            onClick={handleCopy}
            disabled={!streamedText}
          >
            {copied ? "✓ Copied" : "⎘ Copy"}
          </button>
        </div>

        {/* Copy toast */}
        {copied && <div className="copy-toast">Copied to clipboard</div>}
      </div>
    </div>
  );
}
