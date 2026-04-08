import { useState, useEffect, useRef, useCallback } from 'react';
import { useQuillBridge }       from '../hooks/useQuillBridge';
import { loadConfigOnce }       from '../common/configCache';
import { fleschKincaid, gradeLabel } from '../utils/readability';
import { detectLanguage }       from '../utils/detectLanguage';
import DiffView                 from '../components/DiffView';
import ComparisonView           from '../components/ComparisonView';
import '../styles/globals.css';
import './FullPanel.css';

const QUICK_LANGS = [
  { code:'auto',       label:'Auto',       flag:'' },
  { code:'french',     label:'French',     flag:'🇫🇷' },
  { code:'spanish',    label:'Spanish',    flag:'🇪🇸' },
  { code:'german',     label:'German',     flag:'🇩🇪' },
  { code:'japanese',   label:'Japanese',   flag:'🇯🇵' },
  { code:'chinese',    label:'Chinese',    flag:'🇨🇳' },
  { code:'portuguese', label:'Portuguese', flag:'🇵🇹' },
  { code:'italian',    label:'Italian',    flag:'🇮🇹' },
  { code:'arabic',     label:'Arabic',     flag:'🇸🇦' },
  { code:'korean',     label:'Korean',     flag:'🇰🇷' },
  { code:'dutch',      label:'Dutch',      flag:'🇳🇱' },
];

const TABS = [
  { id:'write',    icon:'✏️',  label:'Write' },
  { id:'history',  icon:'🕐', label:'History' },
  { id:'tutor',    icon:'🎓',  label:'Tutor' },
  { id:'settings', icon:'⚙️', label:'Settings' },
];

export default function FullPanel() {
  const bridge = useQuillBridge();
  const {
    visible, selectedText, modes, chains, activeMode,
    streamedText, isStreaming, isDone, error, canUndo,
    chainProgress, suggestion, outputLanguage, templates,
    comparisonResult, isComparing, pronunciation, isPronouncing,
    tutorExplanation, isExplaining,
    selectMode, selectChain, retry, undo,
    confirmReplace, dismiss, compareModes, getPronunciation,
    requestTutorExplain, generateLesson, getHistory,
    toggleFavorite, exportHistory,
    saveTemplate, deleteTemplate, saveConfig,
    closeFullPanel, setLanguage, setTheme,
    clearComparison,
    onHistory, onTutorLesson, onExportData,
  } = bridge;

  const [tab,           setTab]           = useState('write');
  const [showDiff,      setShowDiff]      = useState(false);
  const [instruction,   setInstruction]   = useState('');
  const [compareMode2,  setCompareMode2]  = useState('');
  const [chosenText,    setChosenText]    = useState(null);
  const [customLang,    setCustomLang]    = useState('');
  const [showCustomLang,setShowCustomLang]= useState(false);

  // History state
  const [historyEntries, setHistoryEntries] = useState([]);
  const [favFilter,      setFavFilter]      = useState(false);
  const [expandedEntry,  setExpandedEntry]  = useState(null);

  // Tutor state
  const [lessons,      setLessons]      = useState({});
  const [lessonLoading,setLessonLoading]= useState(null);

  const outputRef = useRef(null);

  useEffect(() => {
    const unsub1 = onHistory((entries, update) => {
      if (entries) setHistoryEntries(entries);
      else if (update) {
        setHistoryEntries(prev => prev.map(e =>
          e.id === update.entry_id ? {...e, favorited: update.favorited} : e
        ));
      }
    });
    const unsub2 = onTutorLesson((lesson_md, period) => {
      setLessons(prev => ({...prev, [period]: lesson_md}));
      setLessonLoading(null);
    });
    const unsub3 = onExportData((entries, format) => {
      if (format === 'json') {
        triggerDownload(JSON.stringify(entries, null, 2), 'quill-history.json', 'application/json');
      } else {
        const csv = entriesToCsv(entries);
        triggerDownload(csv, 'quill-history.csv', 'text/csv');
      }
    });
    return () => { unsub1(); unsub2(); unsub3(); };
  }, []);

  // Load initial data when tab changes
  useEffect(() => {
    if (tab === 'history') getHistory(100);
  }, [tab]);

  // Auto-scroll output
  useEffect(() => {
    if (outputRef.current && isStreaming) {
      outputRef.current.scrollTop = outputRef.current.scrollHeight;
    }
  }, [streamedText, isStreaming]);

  // Keyboard shortcuts — Write tab only.
  // Escape works from any tab (close panel).
  // Mode-number / undo / retry are gated to the Write tab so that pressing
  // `r` or `Cmd+R` from History/Tutor/Settings doesn't accidentally fire
  // retry and re-run the last AI transformation.
  useEffect(() => {
    const handleKey = (e) => {
      if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA' || e.target.tagName === 'SELECT') return;
      const key = e.key.toLowerCase();

      // Panel-wide shortcuts
      if (key === 'escape') { closeFullPanel(); return; }

      // Write-tab-only shortcuts below
      if (tab !== 'write') return;

      if ((e.ctrlKey || e.metaKey) && key === 'z') { e.preventDefault(); undo(); return; }
      // Plain `r` only — never Ctrl+R or Cmd+R (those are reload reflexes).
      if (key === 'r' && !e.ctrlKey && !e.metaKey) { retry(instruction || undefined); return; }

      const idx = parseInt(key) - 1;
      if (idx >= 0 && idx < modes.length) {
        e.preventDefault();
        selectMode(modes[idx].id, instruction || undefined);
      }
    };
    window.addEventListener('keydown', handleKey);
    return () => window.removeEventListener('keydown', handleKey);
  }, [modes, tab, instruction, closeFullPanel, undo, retry, selectMode]);

  const activeText   = chosenText ?? streamedText;
  const wordsBefore  = selectedText ? selectedText.split(/\s+/).filter(Boolean).length : 0;
  const wordsAfter   = activeText   ? activeText.split(/\s+/).filter(Boolean).length   : 0;
  const wordDelta    = wordsAfter - wordsBefore;
  const readability  = activeText ? gradeLabel(fleschKincaid(activeText)) : null;
  const detectedLang = selectedText ? detectLanguage(selectedText) : null;
  const isTranslate  = typeof activeMode === 'string' && activeMode.includes('translate');

  return (
    <div className="full-panel" data-theme={bridge.theme}>

      {/* ── Gradient top bar ────────────────────────────────────────────── */}
      <div className="full-panel-bar drag-handle">
        <div className="full-panel-brand">
          <span className="full-logo">◆</span>
          <span className="full-title">Quill</span>
        </div>

        <nav className="full-nav">
          {TABS.map(t => (
            <button
              key={t.id}
              className={`full-nav-btn${tab === t.id ? ' active' : ''}`}
              onClick={() => setTab(t.id)}
              title={t.label}
            >
              <span>{t.icon}</span>
              <span className="full-nav-label">{t.label}</span>
            </button>
          ))}
        </nav>

        <div className="full-panel-actions">
          <button className="btn btn-icon" title="Close" onClick={closeFullPanel}>✕</button>
        </div>
      </div>

      {/* ── Content ─────────────────────────────────────────────────────── */}
      <div className="full-content">

        {/* ── WRITE TAB ───────────────────────────────────────────────── */}
        {tab === 'write' && (
          <div className="write-tab animate-fade-in">

            {/* Input preview */}
            {selectedText ? (
              <div className="full-input-section">
                <div className="full-section-label">
                  <span>Selected text</span>
                  {detectedLang && <span className="badge">{detectedLang.toUpperCase()}</span>}
                  {chainProgress && (
                    <span className="badge badge-violet" style={{gap:3}}>
                      {Array.from({length: chainProgress.total}, (_, i) => (
                        <span key={i} style={{opacity: i < chainProgress.step ? 1 : 0.3}}>●</span>
                      ))} {chainProgress.mode}
                    </span>
                  )}
                </div>
                <div className="full-input-box">
                  <p className="full-input-text">{selectedText}</p>
                </div>
              </div>
            ) : (
              <div className="full-empty-state">
                <div className="empty-icon">◆</div>
                <p>Select text in any app and press <kbd>Ctrl+Shift+Space</kbd></p>
              </div>
            )}

            {/* Language picker */}
            <div className="full-section-label">Output language</div>
            <div className="full-lang-row">
              {QUICK_LANGS.map(l => (
                <button
                  key={l.code}
                  className={`lang-pill${outputLanguage === l.code ? ' active' : ''}`}
                  onClick={() => setLanguage(l.code)}
                >
                  {l.flag && <span>{l.flag}</span>} {l.label}
                </button>
              ))}
              {showCustomLang ? (
                <input
                  className="input"
                  style={{width:120, padding:'4px 8px', fontSize:11}}
                  placeholder="e.g. Polish"
                  value={customLang}
                  autoFocus
                  onChange={e => setCustomLang(e.target.value)}
                  onKeyDown={e => {
                    if (e.key === 'Enter' && customLang.trim()) {
                      setLanguage(customLang.trim().toLowerCase());
                      setShowCustomLang(false);
                    }
                    if (e.key === 'Escape') { setShowCustomLang(false); }
                  }}
                />
              ) : (
                <button className="lang-pill" onClick={() => setShowCustomLang(true)}>+ Other</button>
              )}
            </div>

            {/* Instruction + template row */}
            <div className="full-instruction-row">
              <input
                className="input"
                placeholder="✍️ Add instruction (optional)…"
                value={instruction}
                onChange={e => setInstruction(e.target.value)}
              />
              {templates.length > 0 && (
                <select
                  className="input"
                  style={{width:'auto', minWidth:120, flexShrink:0}}
                  value=""
                  onChange={e => {
                    const t = templates.find(t => t.name === e.target.value);
                    if (t) setInstruction(t.instruction);
                  }}
                >
                  <option value="">📋 Templates</option>
                  {templates.map(t => <option key={t.name} value={t.name}>{t.name}</option>)}
                </select>
              )}
            </div>

            {/* Modes grid */}
            <div className="full-section-label">Modes</div>
            <div className="full-mode-grid">
              {modes.map((m, i) => (
                <button
                  key={m.id}
                  className={`mode-btn mode-btn-full${activeMode === m.id ? ' active' : ''}${isStreaming && activeMode === m.id ? ' running' : ''}`}
                  title={`${m.label} (${i + 1})`}
                  onClick={() => { setChosenText(null); selectMode(m.id, instruction || undefined); }}
                >
                  <span className="mode-icon">{m.icon}</span>
                  <span className="mode-label">{m.label}</span>
                </button>
              ))}
            </div>

            {/* Chains */}
            {chains.length > 0 && (
              <div className="full-chain-row">
                {chains.map(c => (
                  <button
                    key={c.id}
                    className={`chain-btn${activeMode === `chain:${c.id}` ? ' active' : ''}`}
                    title={c.description}
                    onClick={() => { setChosenText(null); selectChain(c.id, instruction || undefined); }}
                  >
                    {c.icon} {c.label}
                  </button>
                ))}
              </div>
            )}

            {/* Smart suggestion */}
            {suggestion && !activeMode && !isStreaming && (
              <div className="full-suggestion animate-fade-in">
                <span className="text-xs text-muted">✨ Suggested:</span>
                <button
                  className="btn btn-ghost"
                  style={{fontSize:12, padding:'4px 12px'}}
                  onClick={() => selectMode(suggestion.mode_id, instruction || undefined)}
                >
                  {modes.find(m => m.id === suggestion.mode_id)?.icon} {modes.find(m => m.id === suggestion.mode_id)?.label}
                </button>
                <span className="text-xs text-muted">{suggestion.reason}</span>
              </div>
            )}

            {/* Error */}
            {error && (
              <div className="full-error animate-slide-up">⚠️ {error}</div>
            )}

            {/* Output section */}
            {(streamedText || isStreaming || comparisonResult) && (
              <div className="full-output-section animate-slide-up">
                <div className="full-output-label">
                  <span>Output</span>
                  {isDone && readability && (
                    <span className="badge" style={{color: readability.color, borderColor: readability.color + '40', background: readability.color + '15'}}>
                      {readability.label}
                    </span>
                  )}
                  {isDone && wordsBefore > 0 && (
                    <span className="badge">
                      {wordsBefore}→{wordsAfter} words
                      <span className={wordDelta < 0 ? 'text-mint' : wordDelta > 0 ? 'text-amber' : ''}>
                        ({wordDelta > 0 ? '+' : ''}{wordDelta})
                      </span>
                    </span>
                  )}
                </div>

                {comparisonResult ? (
                  <ComparisonView
                    result={comparisonResult}
                    modes={modes}
                    onUse={(text) => { setChosenText(text); clearComparison(); }}
                  />
                ) : showDiff && selectedText && activeText ? (
                  <DiffView original={selectedText} modified={activeText} />
                ) : (
                  <div
                    ref={outputRef}
                    className={`output-area${isStreaming ? ' streaming' : ''}`}
                    style={{maxHeight: 280}}
                  >
                    <span className={isStreaming ? 'streaming-cursor' : ''}>{activeText}</span>
                  </div>
                )}

                {/* Pronunciation */}
                {isTranslate && isDone && activeText && (
                  <div className="full-pronunciation">
                    {isPronouncing ? (
                      <span className="text-xs text-muted"><span className="spinner" style={{display:'inline-block',marginRight:4}}/> Getting pronunciation…</span>
                    ) : pronunciation ? (
                      <div className="pronunciation-box">{pronunciation}</div>
                    ) : (
                      <button
                        className="btn btn-ghost"
                        style={{fontSize:11}}
                        onClick={() => getPronunciation(activeText, outputLanguage)}
                      >
                        🔊 Pronunciation guide
                      </button>
                    )}
                  </div>
                )}

                {/* Tutor explanation */}
                {isDone && (
                  <div className="full-tutor-section">
                    {isExplaining ? (
                      <span className="text-xs text-muted"><span className="spinner" style={{display:'inline-block',marginRight:4}}/> Analysing changes…</span>
                    ) : tutorExplanation ? (
                      <div className="tutor-explanation animate-fade-in markdown">
                        <p className="text-xs text-violet" style={{marginBottom:4}}>💡 Tutor explanation</p>
                        {tutorExplanation}
                      </div>
                    ) : (
                      <button
                        className="btn btn-ghost"
                        style={{fontSize:11}}
                        onClick={() => requestTutorExplain()}
                      >
                        💡 Explain what changed
                      </button>
                    )}
                  </div>
                )}

                {/* Action bar */}
                {isDone && !isComparing && (
                  <div className="action-bar animate-fade-in">
                    <button className="btn btn-primary" onClick={() => { confirmReplace(); closeFullPanel(); }}>
                      ↩ Replace
                    </button>
                    <button className="btn btn-ghost" onClick={() => navigator.clipboard.writeText(activeText)}>
                      ⎘ Copy
                    </button>
                    <button
                      className={`btn btn-ghost${showDiff ? ' active' : ''}`}
                      onClick={() => setShowDiff(v => !v)}
                      title="Diff view"
                    >⊞ Diff</button>

                    {/* Compare */}
                    <div style={{display:'flex', alignItems:'center', gap:4}}>
                      <select
                        className="input"
                        style={{width:120, padding:'4px 8px', fontSize:11}}
                        value={compareMode2}
                        onChange={e => setCompareMode2(e.target.value)}
                      >
                        <option value="">⚖ Compare with…</option>
                        {modes.filter(m => m.id !== activeMode).map(m => (
                          <option key={m.id} value={m.id}>{m.label}</option>
                        ))}
                      </select>
                      {compareMode2 && (
                        <button
                          className="btn btn-ghost"
                          style={{fontSize:11}}
                          onClick={() => { compareModes(activeMode, compareMode2, instruction || undefined); setCompareMode2(''); }}
                        >
                          {isComparing ? <span className="spinner"/> : 'Go'}
                        </button>
                      )}
                    </div>

                    <div className="spacer" />
                    {canUndo && <button className="btn btn-icon" title="Undo (Ctrl+Z)" onClick={undo}>↩</button>}
                    <button className="btn btn-icon" title="Retry (R)" onClick={() => retry(instruction || undefined)}>↻</button>
                  </div>
                )}
              </div>
            )}
          </div>
        )}

        {/* ── HISTORY TAB ─────────────────────────────────────────────── */}
        {tab === 'history' && (
          <div className="history-tab animate-fade-in">
            <div className="history-toolbar">
              <button
                className={`btn btn-ghost${!favFilter ? ' active' : ''}`}
                style={{fontSize:12}}
                onClick={() => setFavFilter(false)}
              >All</button>
              <button
                className={`btn btn-ghost${favFilter ? ' active' : ''}`}
                style={{fontSize:12}}
                onClick={() => setFavFilter(true)}
              >⭐ Favorites</button>
              <div className="spacer" />
              <button className="btn btn-ghost" style={{fontSize:11}} onClick={() => exportHistory('json')}>⬇ JSON</button>
              <button className="btn btn-ghost" style={{fontSize:11}} onClick={() => exportHistory('csv')}>⬇ CSV</button>
            </div>

            <div className="history-list">
              {historyEntries
                .filter(e => !favFilter || e.favorited)
                .map(e => (
                  <div
                    key={e.id}
                    className={`history-entry${expandedEntry === e.id ? ' expanded' : ''}`}
                    onClick={() => setExpandedEntry(prev => prev === e.id ? null : e.id)}
                  >
                    <div className="history-entry-header">
                      <span className="badge">{e.mode}</span>
                      {e.language && e.language !== 'auto' && <span className="badge">{e.language}</span>}
                      <span className="text-xs text-muted truncate" style={{flex:1}}>{e.original_text?.slice(0,60)}</span>
                      <button
                        className={`btn btn-icon${e.favorited ? ' text-amber' : ''}`}
                        onClick={ev => { ev.stopPropagation(); toggleFavorite(e.id); }}
                        title="Toggle favorite"
                      >☆</button>
                      <span className="text-xs text-muted">{e.timestamp?.slice(0,16)}</span>
                    </div>
                    {expandedEntry === e.id && (
                      <div className="history-entry-body animate-slide-up">
                        <div className="history-col">
                          <p className="text-xs text-muted" style={{marginBottom:4}}>Before</p>
                          <p className="text-sm">{e.original_text}</p>
                        </div>
                        <div className="history-col">
                          <p className="text-xs text-muted" style={{marginBottom:4}}>After</p>
                          <p className="text-sm">{e.output_text}</p>
                        </div>
                        {e.tutor_explanation && (
                          <div className="history-explanation">
                            <p className="text-xs text-violet" style={{marginBottom:4}}>💡 Tutor note</p>
                            <p className="text-sm">{e.tutor_explanation}</p>
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                ))}
              {historyEntries.filter(e => !favFilter || e.favorited).length === 0 && (
                <div className="full-empty-state">
                  <div className="empty-icon">🕐</div>
                  <p>{favFilter ? 'No favorites yet — star entries to save them here' : 'History is empty — enable history in Settings'}</p>
                </div>
              )}
            </div>
          </div>
        )}

        {/* ── TUTOR TAB ───────────────────────────────────────────────── */}
        {tab === 'tutor' && (
          <div className="tutor-tab animate-fade-in">
            {['daily', 'weekly'].map(period => (
              <div key={period} className="tutor-lesson-card">
                <div className="tutor-lesson-header">
                  <span className="text-sm" style={{fontWeight:500, textTransform:'capitalize'}}>
                    {period === 'daily' ? '☀️' : '📅'} {period} insight
                  </span>
                  <button
                    className="btn btn-ghost"
                    style={{fontSize:11}}
                    disabled={lessonLoading === period}
                    onClick={() => { setLessonLoading(period); generateLesson(period); }}
                  >
                    {lessonLoading === period ? <span className="spinner"/> : '↻ Generate'}
                  </button>
                </div>
                {lessons[period] ? (
                  <div className="tutor-lesson-body markdown">
                    {lessons[period].split('\n').map((line, i) => {
                      if (line.startsWith('# '))  return <h1 key={i}>{line.slice(2)}</h1>;
                      if (line.startsWith('## ')) return <h2 key={i}>{line.slice(3)}</h2>;
                      if (line.startsWith('### '))return <h3 key={i}>{line.slice(4)}</h3>;
                      if (line.startsWith('**') && line.endsWith('**')) return <p key={i}><strong>{line.slice(2,-2)}</strong></p>;
                      return line ? <p key={i}>{line}</p> : <br key={i}/>;
                    })}
                  </div>
                ) : (
                  <p className="text-sm text-muted">Click Generate to create your {period} writing insight.</p>
                )}
              </div>
            ))}
          </div>
        )}

        {/* ── SETTINGS TAB ────────────────────────────────────────────── */}
        {tab === 'settings' && (
          <div className="settings-tab animate-fade-in">
            <SettingsPanel bridge={bridge} />
          </div>
        )}

      </div>
    </div>
  );
}

// ── Settings panel ────────────────────────────────────────────────────────────
function SettingsPanel({ bridge }) {
  const { saveConfig, setTheme, templates, saveTemplate, deleteTemplate } = bridge;
  const [cfg, setCfg]       = useState({});
  const [saved, setSaved]   = useState(false);
  const [newTpl, setNewTpl] = useState({ name:'', mode:'rewrite', instruction:'' });
  const [settingsTab, setSettingsTab] = useState('provider');

  useEffect(() => {
    // Load current config from Rust backend on mount via the shared cache.
    // NOTE: `api_key` in the loaded config is ALWAYS masked to an empty
    // string by the backend for security — the real value never leaves
    // Rust. The `api_key_set` boolean tells us whether a key exists. The
    // password input is seeded empty; if the user leaves it blank on save,
    // the backend treats that as "keep the existing value".
    //
    // Seed with explicit defaults for every field the form binds to, so
    // that an empty/partial config from the cache doesn't leave `cfg.provider`
    // or `cfg.model` as `undefined` — which would silently overwrite a
    // freshly-saved value on the first user interaction (the `||` fallbacks
    // in the JSX select/input display the default but the underlying
    // `cfg.provider` is still undefined until the user touches the control).
    const defaults = {
      provider: 'openrouter',
      model: 'google/gemma-3-27b-it',
      api_key: '',
      base_url: '',
      hotkey: '',
      persona: { enabled: false, tone: 'natural', style: '', avoid: '' },
      history: { enabled: false },
      tutor: { enabled: false, auto_explain: false },
      clipboard_monitor: { enabled: false },
    };
    loadConfigOnce()
      .then(c => setCfg({ ...defaults, ...(c || {}) }))
      .catch(() => setCfg(defaults));
  }, []);

  const update = (path, val) => {
    const keys = path.split('.');
    setCfg(prev => {
      const next = {...prev};
      let ref = next;
      for (let i = 0; i < keys.length - 1; i++) {
        ref[keys[i]] = {...(ref[keys[i]] || {})};
        ref = ref[keys[i]];
      }
      ref[keys[keys.length - 1]] = val;
      return next;
    });
    setSaved(false);
  };

  const handleSave = async () => {
    await saveConfig(cfg);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  const STABS = ['provider','persona','history','templates','appearance'];

  return (
    <div className="settings-inner">
      <nav className="settings-nav">
        {STABS.map(t => (
          <button key={t} className={`settings-nav-btn${settingsTab===t?' active':''}`} onClick={()=>setSettingsTab(t)}>
            {t.charAt(0).toUpperCase()+t.slice(1)}
          </button>
        ))}
      </nav>

      <div className="settings-body">
        {settingsTab === 'provider' && (
          <div className="settings-section">
            <label className="settings-label">Provider</label>
            <select className="input" value={cfg.provider||'openrouter'} onChange={e=>update('provider',e.target.value)}>
              <option value="openrouter">OpenRouter (recommended)</option>
              <option value="ollama">Ollama (local)</option>
              <option value="openai">OpenAI</option>
              <option value="generic">Custom endpoint</option>
            </select>
            <label className="settings-label">Model</label>
            <input className="input" value={cfg.model||''} onChange={e=>update('model',e.target.value)} placeholder="e.g. google/gemma-3-27b-it" />
            <label className="settings-label">API Key</label>
            <input
              className="input"
              type="password"
              value={cfg.api_key || ''}
              onChange={e => update('api_key', e.target.value)}
              placeholder={cfg.api_key_set ? '•••••••••  (leave blank to keep current key)' : 'sk-or-…'}
            />
            {cfg.provider === 'generic' && <>
              <label className="settings-label">Base URL</label>
              <input className="input" value={cfg.base_url||''} onChange={e=>update('base_url',e.target.value)} placeholder="http://localhost:1234/v1" />
            </>}
            <label className="settings-label">Hotkey</label>
            <input className="input" value={cfg.hotkey||''} onChange={e=>update('hotkey',e.target.value)} placeholder="ctrl+shift+space" />
          </div>
        )}

        {settingsTab === 'persona' && (
          <div className="settings-section">
            <label className="settings-label">
              <input type="checkbox" checked={cfg.persona?.enabled||false} onChange={e=>update('persona.enabled',e.target.checked)} style={{marginRight:6}}/>
              Enable My Voice
            </label>
            <label className="settings-label">Tone</label>
            <select className="input" value={cfg.persona?.tone||'natural'} onChange={e=>update('persona.tone',e.target.value)}>
              {['natural','casual','professional','witty','direct','warm'].map(t=><option key={t} value={t}>{t}</option>)}
            </select>
            <label className="settings-label">Style notes</label>
            <textarea className="input" rows={3} style={{resize:'vertical'}} value={cfg.persona?.style||''} onChange={e=>update('persona.style',e.target.value)} placeholder="Short punchy sentences. I use em-dashes…" />
            <label className="settings-label">Always avoid</label>
            <input className="input" value={cfg.persona?.avoid||''} onChange={e=>update('persona.avoid',e.target.value)} placeholder="passive voice, corporate buzzwords" />
          </div>
        )}

        {settingsTab === 'history' && (
          <div className="settings-section">
            <label className="settings-label">
              <input type="checkbox" checked={cfg.history?.enabled||false} onChange={e=>update('history.enabled',e.target.checked)} style={{marginRight:6}}/>
              Enable history
            </label>
            <label className="settings-label">
              <input type="checkbox" checked={cfg.tutor?.enabled||false} onChange={e=>update('tutor.enabled',e.target.checked)} style={{marginRight:6}}/>
              Enable AI Tutor (requires history)
            </label>
            <label className="settings-label">
              <input type="checkbox" checked={cfg.tutor?.auto_explain||false} onChange={e=>update('tutor.auto_explain',e.target.checked)} style={{marginRight:6}}/>
              Auto-explain every transformation
            </label>
            <label className="settings-label">
              <input type="checkbox" checked={cfg.clipboard_monitor?.enabled||false} onChange={e=>update('clipboard_monitor.enabled',e.target.checked)} style={{marginRight:6}}/>
              Clipboard monitor (auto-trigger from clipboard)
            </label>
          </div>
        )}

        {settingsTab === 'templates' && (
          <div className="settings-section">
            <p className="text-xs text-muted" style={{marginBottom:8}}>Saved instruction templates shown in the overlay</p>
            {templates.map(t => (
              <div key={t.name} className="template-row">
                <span className="text-sm">{t.name}</span>
                <span className="text-xs text-muted truncate" style={{flex:1}}>{t.instruction}</span>
                <button className="btn btn-danger" style={{fontSize:11,padding:'2px 8px'}} onClick={()=>deleteTemplate(t.name)}>Remove</button>
              </div>
            ))}
            <div className="template-add-row">
              <input className="input" placeholder="Name" value={newTpl.name} onChange={e=>setNewTpl(p=>({...p,name:e.target.value}))} style={{width:100}} />
              <input className="input" placeholder="Instruction…" value={newTpl.instruction} onChange={e=>setNewTpl(p=>({...p,instruction:e.target.value}))} style={{flex:1}} />
              <button className="btn btn-primary" style={{fontSize:11}} onClick={()=>{
                if(newTpl.name&&newTpl.instruction){saveTemplate(newTpl.name,newTpl.mode,newTpl.instruction);setNewTpl({name:'',mode:'rewrite',instruction:''});}
              }}>Add</button>
            </div>
          </div>
        )}

        {settingsTab === 'appearance' && (
          <div className="settings-section">
            <label className="settings-label">Theme</label>
            <div style={{display:'flex',gap:8}}>
              {['dark','light'].map(t=>(
                <button key={t} className={`btn btn-ghost${bridge.theme===t?' active':''}`} onClick={()=>setTheme(t)}>
                  {t==='dark'?'🌙':'☀️'} {t.charAt(0).toUpperCase()+t.slice(1)}
                </button>
              ))}
            </div>
          </div>
        )}

        <div style={{marginTop:'auto',paddingTop:16}}>
          <button className="btn btn-primary" onClick={handleSave} style={{width:'100%'}}>
            {saved ? '✓ Saved' : 'Save settings'}
          </button>
        </div>
      </div>
    </div>
  );
}

// ── Helpers ───────────────────────────────────────────────────────────────────
function triggerDownload(content, filename, type) {
  const blob = new Blob([content], { type });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url; a.download = filename; a.click();
  URL.revokeObjectURL(url);
}

function entriesToCsv(entries) {
  const headers = ['id','timestamp','mode','language','original_text','output_text','word_count_before','word_count_after','favorited'];
  const rows = entries.map(e => headers.map(h => JSON.stringify(e[h] ?? '')).join(','));
  return [headers.join(','), ...rows].join('\n');
}
