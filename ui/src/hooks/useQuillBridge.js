/**
 * useQuillBridge — central bridge between React UI and the Rust Tauri backend.
 * Uses direct Tauri invoke() commands (no Python sidecar).
 */
import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

const MAX_UNDO = 20;

export function useQuillBridge() {
  // ── Overlay state ──────────────────────────────────────────────────────────
  const [visible,         setVisible]         = useState(false);
  const [selectedText,    setSelectedText]     = useState('');
  const [context,         setContext]          = useState({});
  const [modes,           setModes]            = useState([]);
  const [chains,          setChains]           = useState([]);
  const [activeMode,      setActiveMode]       = useState(null);
  const [streamedText,    setStreamedText]     = useState('');
  const [isStreaming,     setIsStreaming]       = useState(false);
  const [isDone,          setIsDone]           = useState(false);
  const [error,           setError]            = useState(null);
  const [lastEntryId,     setLastEntryId]      = useState(null);
  const [chainProgress,   setChainProgress]    = useState(null);
  const [suggestion,      setSuggestion]       = useState(null);

  // ── Language & theme ───────────────────────────────────────────────────────
  const [outputLanguage, setOutputLanguage] = useState(
    () => localStorage.getItem('quill_output_language') || 'auto'
  );
  const [theme, setThemeState] = useState(
    () => localStorage.getItem('quill_theme') || 'dark'
  );

  // ── Undo stack ─────────────────────────────────────────────────────────────
  const outputStack = useRef([]);
  const [canUndo, setCanUndo] = useState(false);

  // ── Tutor ──────────────────────────────────────────────────────────────────
  const [tutorExplanation, setTutorExplanation] = useState(null);
  const [isExplaining,     setIsExplaining]     = useState(false);

  // ── Comparison ────────────────────────────────────────────────────────────
  const [comparisonResult, setComparisonResult] = useState(null);
  const [isComparing,      setIsComparing]      = useState(false);

  // ── Pronunciation ─────────────────────────────────────────────────────────
  const [pronunciation,    setPronunciation]    = useState(null);
  const [isPronouncing,    setIsPronouncing]    = useState(false);

  // ── Clipboard & Templates ─────────────────────────────────────────────────
  const [clipboardToast,   setClipboardToast]  = useState(null);
  const [templates,        setTemplates]       = useState([]);

  // ── History ───────────────────────────────────────────────────────────────
  const historyListeners  = useRef([]);
  const tutorLsnListeners = useRef([]);
  const tutorExpListeners = useRef([]);
  const exportListeners   = useRef([]);

  // ── Active mode ref (for closures) ────────────────────────────────────────
  const activeModeRef = useRef(null);
  useEffect(() => { activeModeRef.current = activeMode; }, [activeMode]);

  // ── Apply theme ───────────────────────────────────────────────────────────
  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('quill_theme', theme);
  }, [theme]);

  // ── Persist language ──────────────────────────────────────────────────────
  useEffect(() => {
    localStorage.setItem('quill_output_language', outputLanguage);
  }, [outputLanguage]);

  // ── Event listeners ───────────────────────────────────────────────────────
  useEffect(() => {
    const unsubs = [];

    // Load config on mount
    invoke('get_config').then(cfg => {
      if (cfg?.templates) setTemplates(cfg.templates);
    }).catch(() => {});

    listen('quill://show_overlay', e => {
      const { text, context: ctx, modes: ms, chains: cs } = e.payload;
      setSelectedText(text || '');
      setContext(ctx || {});
      setModes(ms || []);
      setChains(cs || []);
      setStreamedText('');
      setIsStreaming(false);
      setIsDone(false);
      setError(null);
      setActiveMode(null);
      setChainProgress(null);
      setSuggestion(null);
      setTutorExplanation(null);
      setComparisonResult(null);
      setPronunciation(null);
      outputStack.current = [];
      setCanUndo(false);
      setVisible(true);
    }).then(fn => unsubs.push(fn));

    listen('quill://stream_chunk', e => {
      const { chunk } = e.payload;
      setStreamedText(prev => prev + chunk);
      setIsStreaming(true);
      setIsDone(false);
    }).then(fn => unsubs.push(fn));

    listen('quill://stream_done', e => {
      const { full_text, entry_id } = e.payload;
      setStreamedText(full_text || '');
      setIsStreaming(false);
      setIsDone(true);
      if (entry_id != null) setLastEntryId(entry_id);

      // Push to undo stack
      const frame = { text: full_text || '', mode: activeModeRef.current, entryId: entry_id };
      outputStack.current = [frame, ...outputStack.current].slice(0, MAX_UNDO);
      setCanUndo(outputStack.current.length > 1);
    }).then(fn => unsubs.push(fn));

    listen('quill://chain_step', e => {
      setChainProgress(e.payload);
      setStreamedText('');
    }).then(fn => unsubs.push(fn));

    listen('quill://smart_suggestion', e => {
      setSuggestion(e.payload);
    }).then(fn => unsubs.push(fn));

    listen('quill://comparison_done', e => {
      setComparisonResult(e.payload);
      setIsComparing(false);
      setIsDone(true);
    }).then(fn => unsubs.push(fn));

    listen('quill://pronunciation', e => {
      setPronunciation(e.payload.text);
      setIsPronouncing(false);
    }).then(fn => unsubs.push(fn));

    listen('quill://clipboard_change', e => {
      setClipboardToast(e.payload.text);
      setTimeout(() => setClipboardToast(null), 6000);
    }).then(fn => unsubs.push(fn));

    listen('quill://templates_updated', e => {
      setTemplates(e.payload.templates || []);
    }).then(fn => unsubs.push(fn));

    listen('quill://error', e => {
      setError(e.payload?.message || String(e.payload));
      setIsStreaming(false);
      setIsComparing(false);
    }).then(fn => unsubs.push(fn));

    listen('quill://tutor_explanation', e => {
      setTutorExplanation(e.payload.explanation);
      setIsExplaining(false);
      tutorExpListeners.current.forEach(fn => fn(e.payload.explanation, e.payload.entry_id));
    }).then(fn => unsubs.push(fn));

    listen('quill://tutor_lesson', e => {
      tutorLsnListeners.current.forEach(fn => fn(e.payload.lesson_md, e.payload.period));
    }).then(fn => unsubs.push(fn));

    listen('quill://history', e => {
      historyListeners.current.forEach(fn => fn(e.payload.entries, e.payload.update));
    }).then(fn => unsubs.push(fn));

    listen('quill://favorite_toggled', e => {
      historyListeners.current.forEach(fn => fn(null, e.payload));
    }).then(fn => unsubs.push(fn));

    listen('quill://export_data', e => {
      exportListeners.current.forEach(fn => fn(e.payload.entries, e.payload.format));
    }).then(fn => unsubs.push(fn));

    return () => unsubs.forEach(fn => fn());
  }, []);

  // ── Actions ───────────────────────────────────────────────────────────────

  const selectMode = useCallback(async (modeId, extraInstruction) => {
    setActiveMode(modeId);
    setStreamedText('');
    setIsStreaming(true);
    setIsDone(false);
    setError(null);
    setComparisonResult(null);
    setTutorExplanation(null);
    setPronunciation(null);
    try {
      await invoke('execute_mode', {
        mode: modeId,
        language: outputLanguage,
        extraInstruction: extraInstruction || null,
      });
    } catch (err) { setError(String(err)); setIsStreaming(false); }
  }, [outputLanguage]);

  const selectChain = useCallback(async (chainId, extraInstruction) => {
    setActiveMode(`chain:${chainId}`);
    setStreamedText('');
    setIsStreaming(true);
    setIsDone(false);
    setError(null);
    try {
      await invoke('execute_chain', {
        chainId,
        language: outputLanguage,
        extraInstruction: extraInstruction || null,
      });
    } catch (err) { setError(String(err)); setIsStreaming(false); }
  }, [outputLanguage]);

  const retry = useCallback(async (extraInstruction) => {
    setStreamedText('');
    setIsStreaming(true);
    setIsDone(false);
    setError(null);
    try {
      await invoke('retry', { extraInstruction: extraInstruction || null });
    } catch (err) { setError(String(err)); setIsStreaming(false); }
  }, []);

  const undo = useCallback(async () => {
    if (outputStack.current.length < 2) return;
    outputStack.current = outputStack.current.slice(1);
    const prev = outputStack.current[0];
    setStreamedText(prev.text);
    setIsDone(true);
    setCanUndo(outputStack.current.length > 1);
    // Sync undone text to Rust so confirm_replace pastes correctly
    await invoke('set_result', { text: prev.text }).catch(() => {});
  }, []);

  const confirmReplace = useCallback(async () => {
    try { await invoke('confirm_replace'); } catch (err) { setError(String(err)); }
  }, []);

  const dismiss = useCallback(async () => {
    setVisible(false);
    setIsStreaming(false);
    try { await invoke('dismiss'); } catch { /* ignore */ }
  }, []);

  const setResultText = useCallback(async (text) => {
    setStreamedText(text);
    await invoke('set_result', { text }).catch(() => {});
  }, []);

  const compareModes = useCallback(async (modeA, modeB, extraInstruction) => {
    setIsComparing(true);
    setComparisonResult(null);
    try {
      await invoke('compare_modes_cmd', {
        modeA, modeB,
        language: outputLanguage,
        extraInstruction: extraInstruction || null,
      });
    } catch (err) { setError(String(err)); setIsComparing(false); }
  }, [outputLanguage]);

  const getPronunciation = useCallback(async (text, language) => {
    setIsPronouncing(true);
    try {
      await invoke('get_pronunciation', { text, language });
    } catch (err) { setError(String(err)); setIsPronouncing(false); }
  }, []);

  const requestTutorExplain = useCallback(async (entryId) => {
    setIsExplaining(true);
    try { await invoke('request_tutor_explain', { entryId: entryId ?? null }); }
    catch (err) { setError(String(err)); setIsExplaining(false); }
  }, []);

  const generateLesson = useCallback(async (period) => {
    try { await invoke('generate_lesson', { period }); }
    catch (err) { setError(String(err)); }
  }, []);

  const getHistory = useCallback(async (limit, language) => {
    try { await invoke('get_history', { limit: limit ?? 50, language: language ?? null }); }
    catch (err) { setError(String(err)); }
  }, []);

  const toggleFavorite = useCallback(async (entryId) => {
    try { await invoke('toggle_favorite', { entryId }); }
    catch (err) { setError(String(err)); }
  }, []);

  const exportHistory = useCallback(async (format) => {
    try { await invoke('export_history', { format }); }
    catch (err) { setError(String(err)); }
  }, []);

  const saveTemplate = useCallback(async (name, mode, instruction) => {
    try { await invoke('save_template', { name, mode, instruction }); }
    catch (err) { setError(String(err)); }
  }, []);

  const deleteTemplate = useCallback(async (name) => {
    try { await invoke('delete_template', { name }); }
    catch (err) { setError(String(err)); }
  }, []);

  const saveConfig = useCallback(async (configUpdate) => {
    try { await invoke('save_config', { configUpdate }); }
    catch (err) { setError(String(err)); }
  }, []);

  const openFullPanel = useCallback(async () => {
    try { await invoke('open_full_panel'); } catch { /* ignore */ }
  }, []);

  const closeFullPanel = useCallback(async () => {
    try { await invoke('close_full_panel'); } catch { /* ignore */ }
  }, []);

  const setTheme = useCallback((t) => { setThemeState(t); }, []);

  const setLanguage = useCallback((lang) => { setOutputLanguage(lang); }, []);

  // ── Subscription helpers for panels ───────────────────────────────────────
  const onHistory        = useCallback(fn => { historyListeners.current.push(fn);  return () => { historyListeners.current  = historyListeners.current.filter(f => f !== fn); }; }, []);
  const onTutorLesson    = useCallback(fn => { tutorLsnListeners.current.push(fn); return () => { tutorLsnListeners.current = tutorLsnListeners.current.filter(f => f !== fn); }; }, []);
  const onTutorExplanation = useCallback(fn => { tutorExpListeners.current.push(fn); return () => { tutorExpListeners.current = tutorExpListeners.current.filter(f => f !== fn); }; }, []);
  const onExportData     = useCallback(fn => { exportListeners.current.push(fn);   return () => { exportListeners.current   = exportListeners.current.filter(f => f !== fn); }; }, []);

  return {
    // State
    visible, selectedText, context, modes, chains,
    activeMode, streamedText, isStreaming, isDone, error,
    lastEntryId, chainProgress, suggestion,
    outputLanguage, theme,
    canUndo,
    tutorExplanation, isExplaining,
    comparisonResult, isComparing,
    pronunciation, isPronouncing,
    clipboardToast, templates,

    // Actions
    selectMode, selectChain, retry, undo,
    confirmReplace, dismiss, setResultText,
    compareModes, getPronunciation,
    requestTutorExplain, generateLesson,
    getHistory, toggleFavorite, exportHistory,
    saveTemplate, deleteTemplate, saveConfig,
    openFullPanel, closeFullPanel,
    setTheme, setLanguage,

    // Subscriptions
    onHistory, onTutorLesson, onTutorExplanation, onExportData,
  };
}
