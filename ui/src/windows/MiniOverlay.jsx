import { useState, useEffect, useCallback, useRef } from 'react';
import { getCurrentWindow }  from '@tauri-apps/api/window';
import { useQuillBridge }    from '../hooks/useQuillBridge';
import { fleschKincaid, gradeLabel } from '../utils/readability';
import { detectLanguage }    from '../utils/detectLanguage';
import DiffView              from '../components/DiffView';
import '../styles/globals.css';
import './MiniOverlay.css';

const QUICK_LANGS = [
  { code:'auto', label:'Auto', flag:'' },
  { code:'french',     label:'FR', flag:'🇫🇷' },
  { code:'spanish',    label:'ES', flag:'🇪🇸' },
  { code:'german',     label:'DE', flag:'🇩🇪' },
  { code:'japanese',   label:'JA', flag:'🇯🇵' },
  { code:'portuguese', label:'PT', flag:'🇵🇹' },
  { code:'chinese',    label:'ZH', flag:'🇨🇳' },
];

export default function MiniOverlay() {
  const bridge = useQuillBridge();
  const {
    visible, selectedText, modes, chains, activeMode,
    streamedText, isStreaming, isDone, error, canUndo,
    chainProgress, suggestion, outputLanguage, templates,
    clipboardToast,
    selectMode, selectChain, retry, undo,
    confirmReplace, dismiss, compareModes,
    openFullPanel, setLanguage,
  } = bridge;

  const [showDiff,      setShowDiff]      = useState(false);
  const [instruction,   setInstruction]   = useState('');
  const [showInstInput, setShowInstInput] = useState(false);
  const [chosenText,    setChosenText]    = useState(null);

  const outputRef  = useRef(null);
  const inputRef   = useRef(null);
  const windowRef  = useRef(getCurrentWindow());

  // `instruction` is refreshed on every keystroke. We read it from a ref
  // inside the keydown handler (below) instead of from closure, so the
  // listener can be attached ONCE per overlay session instead of being
  // torn down and re-attached on every character typed in the instruction
  // field.
  const instructionRef = useRef(instruction);
  useEffect(() => { instructionRef.current = instruction; }, [instruction]);

  // Show/hide the OS window in sync with React's `visible` state.
  //
  // We intentionally call window.show() HERE (from JS, after React has
  // rendered the content) rather than from the Rust handle_hotkey path.
  // Calling w.show() in Rust before the IPC event reaches the JS means the
  // transparent window appears EMPTY for ~50-100 ms before React renders,
  // and the bounce-in animation starts from opacity:0 while the window is
  // already visible — producing a jarring transparent flash. Calling
  // show() here guarantees the window is only made visible once the
  // overlay content is already in the DOM and the animation has begun.
  useEffect(() => {
    if (!visible) {
      windowRef.current.hide().catch(() => {});
    } else {
      setShowDiff(false);
      setChosenText(null);
      setInstruction('');
      setShowInstInput(false);
      windowRef.current.show().catch(() => {});
      windowRef.current.setFocus().catch(() => {});
    }
  }, [visible]);

  // Auto-scroll output
  useEffect(() => {
    if (outputRef.current && isStreaming) {
      outputRef.current.scrollTop = outputRef.current.scrollHeight;
    }
  }, [streamedText, isStreaming]);

  // `handleModeClick` reads instruction from the ref for the same reason as
  // the keydown handler — we want stable references so number-key shortcuts
  // and on-click handlers see the latest instruction value.
  const handleModeClick = useCallback((modeId) => {
    setChosenText(null);
    selectMode(modeId, instructionRef.current || undefined);
  }, [selectMode]);

  // Keyboard shortcuts
  //
  // The listener is attached ONCE per visible session (not per keystroke).
  // It reads the current `instruction` via `instructionRef.current` so the
  // closure doesn't need to be re-created as the user types — eliminates
  // the attach/detach churn on every character.
  useEffect(() => {
    const handleKey = (e) => {
      if (!visible) return;
      if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') return;
      const key = e.key.toLowerCase();
      if (key === 'escape')                   { e.preventDefault(); dismiss(); return; }
      if ((e.ctrlKey || e.metaKey) && key === 'z') { e.preventDefault(); undo(); return; }
      if (key === 'r' && !e.ctrlKey && !e.metaKey) { e.preventDefault(); retry(instructionRef.current || undefined); return; }
      const idx = parseInt(key) - 1;
      if (idx >= 0 && idx < modes.length)     { e.preventDefault(); handleModeClick(modes[idx].id); }
    };
    window.addEventListener('keydown', handleKey);
    return () => window.removeEventListener('keydown', handleKey);
  }, [visible, modes, dismiss, undo, retry, handleModeClick]);

  // Read `instruction` from the ref (same as handleModeClick) so this
  // callback has a stable identity — it doesn't need to rebuild on every
  // keystroke in the instruction field.
  const handleChainClick = useCallback((chainId) => {
    setChosenText(null);
    selectChain(chainId, instructionRef.current || undefined);
  }, [selectChain]);

  const handleReplace = useCallback(() => {
    if (chosenText) { bridge.setResultText(chosenText).then(() => bridge.confirmReplace()); }
    else { confirmReplace(); }
    dismiss();
  }, [chosenText, confirmReplace, dismiss]);

  const activeText    = chosenText ?? streamedText;
  const wordsBefore   = selectedText ? selectedText.split(/\s+/).filter(Boolean).length : 0;
  const wordsAfter    = activeText   ? activeText.split(/\s+/).filter(Boolean).length   : 0;
  const wordDelta     = wordsAfter - wordsBefore;
  const readability   = activeText ? gradeLabel(fleschKincaid(activeText)) : null;
  const detectedLang  = selectedText ? detectLanguage(selectedText) : null;

  if (!visible) return null;

  return (
    <div className="mini-overlay animate-bounce-in" data-theme={bridge.theme}>
      {/* ── Drag handle / header ───────────────────────────────────────────── */}
      <div className="mini-header drag-handle">
        <div className="mini-header-left">
          <span className="mini-logo">◆</span>
          <span className="mini-title">Quill</span>
          {detectedLang && detectedLang !== 'en' && (
            <span className="badge" style={{marginLeft:4}}>{detectedLang.toUpperCase()}</span>
          )}
        </div>
        <div className="mini-header-right">
          {chainProgress && (
            <span className="badge badge-violet" style={{gap:3}}>
              {Array.from({length: chainProgress.total}, (_, i) => (
                <span key={i} style={{opacity: i < chainProgress.step ? 1 : 0.3}}>●</span>
              ))}
            </span>
          )}
          <button className="btn btn-icon" title="Open full panel (F)" onClick={openFullPanel}>↗</button>
          <button className="btn btn-icon" title="Close (Esc)" onClick={dismiss}>✕</button>
        </div>
      </div>

      {/* ── Input preview ─────────────────────────────────────────────────── */}
      {selectedText ? (
        <div className="mini-input-preview">
          <p className="mini-input-text">{selectedText}</p>
        </div>
      ) : (
        <div className="mini-empty-state">
          <span className="text-muted text-sm">No text selected — press hotkey with selected text</span>
        </div>
      )}

      {/* ── Language row ──────────────────────────────────────────────────── */}
      <div className="mini-lang-row">
        {QUICK_LANGS.map(l => (
          <button
            key={l.code}
            className={`lang-pill${outputLanguage === l.code ? ' active' : ''}`}
            onClick={() => setLanguage(l.code)}
          >
            {l.flag} {l.label}
          </button>
        ))}
      </div>

      {/* ── Mode bar ──────────────────────────────────────────────────────── */}
      <div className="mini-mode-row">
        {modes.map((m, i) => (
          <button
            key={m.id}
            className={`mode-btn mode-btn-mini${activeMode === m.id ? ' active' : ''}${isStreaming && activeMode === m.id ? ' running' : ''}`}
            title={`${m.label} (${i + 1})`}
            onClick={() => handleModeClick(m.id)}
          >
            <span className="mode-icon">{m.icon}</span>
          </button>
        ))}
      </div>

      {/* ── Chain row ─────────────────────────────────────────────────────── */}
      {chains.length > 0 && (
        <div className="mini-chain-row">
          {chains.map(c => (
            <button
              key={c.id}
              className={`chain-btn${activeMode === `chain:${c.id}` ? ' active' : ''}`}
              title={c.description}
              onClick={() => handleChainClick(c.id)}
            >
              {c.icon} {c.label}
            </button>
          ))}
        </div>
      )}

      {/* ── Instruction field ─────────────────────────────────────────────── */}
      {showInstInput ? (
        <div className="mini-instruction-row">
          <input
            ref={inputRef}
            className="input"
            placeholder="One-off instruction…"
            value={instruction}
            onChange={e => setInstruction(e.target.value)}
            onKeyDown={e => {
              if (e.key === 'Enter') { e.target.blur(); }
              if (e.key === 'Escape') { setShowInstInput(false); setInstruction(''); }
            }}
            autoFocus
          />
          <button className="btn btn-icon" onClick={() => { setShowInstInput(false); setInstruction(''); }}>✕</button>
        </div>
      ) : (
        <button
          className="mini-instruction-trigger text-muted text-xs"
          onClick={() => { setShowInstInput(true); setTimeout(() => inputRef.current?.focus(), 50); }}
        >
          {instruction ? `✍️ ${instruction}` : '+ Add instruction…'}
        </button>
      )}

      {/* ── Smart suggestion ──────────────────────────────────────────────── */}
      {suggestion && !activeMode && !isStreaming && (
        <div className="mini-suggestion animate-fade-in">
          <span className="text-muted text-xs">✨ Suggested:</span>
          <button
            className="btn btn-ghost"
            style={{fontSize:11, padding:'3px 10px'}}
            onClick={() => handleModeClick(suggestion.mode_id)}
          >
            {modes.find(m => m.id === suggestion.mode_id)?.icon} {modes.find(m => m.id === suggestion.mode_id)?.label}
          </button>
          <span className="text-muted text-xs">{suggestion.reason}</span>
        </div>
      )}

      {/* ── Error ─────────────────────────────────────────────────────────── */}
      {error && (
        <div className="mini-error animate-slide-up">
          <span>⚠️ {error}</span>
          <button className="btn btn-icon" style={{fontSize:11}} onClick={() => bridge.clearError()}>✕</button>
        </div>
      )}

      {/* ── Output ────────────────────────────────────────────────────────── */}
      {(streamedText || isStreaming || showDiff) && (
        <div className="mini-output-section animate-slide-up">
          {showDiff && selectedText && activeText ? (
            <DiffView original={selectedText} modified={activeText} />
          ) : (
            <div
              ref={outputRef}
              className={`output-area${isStreaming ? ' streaming' : ''}`}
            >
              <span className={isStreaming ? 'streaming-cursor' : ''}>{activeText}</span>
            </div>
          )}

          {/* Stats row */}
          {isDone && activeText && (
            <div className="mini-stats animate-fade-in">
              {wordsBefore > 0 && (
                <span className="stat-chip">
                  {wordsBefore}→{wordsAfter}
                  <span className={wordDelta < 0 ? 'text-mint' : wordDelta > 0 ? 'text-amber' : ''}>
                    {wordDelta > 0 ? `+${wordDelta}` : wordDelta}
                  </span>
                </span>
              )}
              {readability && (
                <span className="stat-chip" style={{color: readability.color}}>
                  {readability.label}
                </span>
              )}
            </div>
          )}

          {/* Action bar */}
          {isDone && (
            <div className="action-bar animate-fade-in">
              <button className="btn btn-primary" onClick={handleReplace}>
                ↩ Replace
              </button>
              <button className="btn btn-ghost" onClick={() => navigator.clipboard.writeText(activeText)}>
                ⎘ Copy
              </button>
              <button
                className={`btn btn-ghost${showDiff ? ' active' : ''}`}
                title="Toggle diff view"
                onClick={() => setShowDiff(v => !v)}
              >⊞</button>
              <div className="spacer" />
              {canUndo && (
                <button className="btn btn-icon" title="Undo (Ctrl+Z)" onClick={undo}>↩</button>
              )}
              <button className="btn btn-icon" title="Retry (R)" onClick={() => retry(instruction || undefined)}>↻</button>
              <button
                className="btn btn-ghost"
                style={{fontSize:11, padding:'4px 8px'}}
                onClick={openFullPanel}
              >
                Full panel ↗
              </button>
            </div>
          )}
        </div>
      )}

      {/* ── Clipboard toast ───────────────────────────────────────────────── */}
      {clipboardToast && (
        <div className="mini-clipboard-toast animate-slide-up">
          <span>📋 Clipboard: </span>
          <span className="truncate" style={{maxWidth:200}}>{clipboardToast}</span>
          <button
            className="btn btn-ghost"
            style={{fontSize:11, padding:'2px 8px', marginLeft:'auto'}}
            onClick={() => bridge.promoteClipboardToast(clipboardToast)}
          >Use</button>
        </div>
      )}
    </div>
  );
}
