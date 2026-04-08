/**
 * useQuillBridge — central bridge between React UI and the Rust Tauri backend.
 * Uses direct Tauri invoke() commands (no Python sidecar).
 */
import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { loadConfigOnce, invalidateConfigCache } from '../common/configCache';

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
  // Pending auto-dismiss timer for the clipboard toast. Tracked in a ref so
  // rapid successive clipboard events don't leak timers or race each other
  // (the previous setTimeout could otherwise fire and clear the CURRENT toast
  // prematurely, and timers could fire after unmount).
  const clipboardToastTimerRef = useRef(null);

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
    // `listen()` is async — it returns a Promise<UnlistenFn>. If the component
    // unmounts before that Promise resolves (Strict Mode, HMR, fast logout),
    // we need the resolved callback to be invoked anyway. We track a single
    // `cancelled` flag and, in each `.then`, either push to `unsubs` (still
    // alive) or call the unlisten immediately (unmounted in the meantime).
    let cancelled = false;
    const unsubs = [];
    const track = (p) =>
      p.then((fn) => {
        if (cancelled) fn();
        else unsubs.push(fn);
      });

    // Load config on mount via the shared memoised cache so App.jsx and
    // useQuillBridge don't both hit the Rust backend on startup.
    loadConfigOnce().then(cfg => {
      if (cancelled) return;
      if (cfg?.templates) setTemplates(cfg.templates);
    }).catch(() => {});

    track(listen('quill://show_overlay', e => {
      const { text, context: ctx, modes: ms, chains: cs, suggestion: sug } = e.payload;
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
      // Suggestion is part of the show_overlay payload now (folded in by
      // the Rust side) — previously it came as a separate event emitted
      // BEFORE show_overlay, so this listener's reset was wiping it.
      setSuggestion(sug || null);
      setTutorExplanation(null);
      setComparisonResult(null);
      setPronunciation(null);
      // Clear the prior selection's history id so consumers (Tutor "Explain
      // this" button, etc.) don't resolve to an unrelated entry after a
      // clipboard-toast promotion or fresh hotkey trigger.
      setLastEntryId(null);
      outputStack.current = [];
      setCanUndo(false);
      setVisible(true);
    }));

    track(listen('quill://stream_chunk', e => {
      const { chunk } = e.payload;
      setStreamedText(prev => prev + chunk);
      setIsStreaming(true);
      setIsDone(false);
    }));

    track(listen('quill://stream_done', e => {
      const { full_text, entry_id } = e.payload;
      setStreamedText(full_text || '');
      setIsStreaming(false);
      setIsDone(true);
      if (entry_id != null) setLastEntryId(entry_id);

      // Push to undo stack
      const frame = { text: full_text || '', mode: activeModeRef.current, entryId: entry_id };
      outputStack.current = [frame, ...outputStack.current].slice(0, MAX_UNDO);
      setCanUndo(outputStack.current.length > 1);
    }));

    track(listen('quill://chain_step', e => {
      setChainProgress(e.payload);
      setStreamedText('');
    }));

    track(listen('quill://comparison_done', e => {
      setComparisonResult(e.payload);
      setIsComparing(false);
      setIsDone(true);
    }));

    track(listen('quill://pronunciation', e => {
      setPronunciation(e.payload.text);
      setIsPronouncing(false);
    }));

    track(listen('quill://clipboard_change', e => {
      setClipboardToast(e.payload.text);
      // Cancel any pending dismiss so the fresh toast always gets its full
      // 6-second window instead of being cut short by a stale timer.
      if (clipboardToastTimerRef.current) {
        clearTimeout(clipboardToastTimerRef.current);
      }
      clipboardToastTimerRef.current = setTimeout(() => {
        setClipboardToast(null);
        clipboardToastTimerRef.current = null;
      }, 6000);
    }));

    track(listen('quill://templates_updated', e => {
      setTemplates(e.payload.templates || []);
    }));

    track(listen('quill://error', e => {
      setError(e.payload?.message || String(e.payload));
      setIsStreaming(false);
      setIsComparing(false);
    }));

    track(listen('quill://tutor_explanation', e => {
      setTutorExplanation(e.payload.explanation);
      setIsExplaining(false);
      tutorExpListeners.current.forEach(fn => fn(e.payload.explanation, e.payload.entry_id));
    }));

    track(listen('quill://tutor_lesson', e => {
      tutorLsnListeners.current.forEach(fn => fn(e.payload.lesson_md, e.payload.period));
    }));

    track(listen('quill://history', e => {
      historyListeners.current.forEach(fn => fn(e.payload.entries, e.payload.update));
    }));

    track(listen('quill://favorite_toggled', e => {
      historyListeners.current.forEach(fn => fn(null, e.payload));
    }));

    track(listen('quill://export_data', e => {
      exportListeners.current.forEach(fn => fn(e.payload.entries, e.payload.format));
    }));

    return () => {
      cancelled = true;
      unsubs.forEach(fn => fn());
      // Cancel any pending clipboard-toast auto-dismiss timer so it doesn't
      // fire on an unmounted component (React 18 Strict Mode catches this,
      // but it's still a leak in prod).
      if (clipboardToastTimerRef.current) {
        clearTimeout(clipboardToastTimerRef.current);
        clipboardToastTimerRef.current = null;
      }
    };
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
    try {
      await invoke('save_config', { configUpdate });
      // Invalidate the shared config cache so any subsequent `loadConfigOnce`
      // reads see the freshly-persisted values instead of the stale in-flight
      // promise from startup.
      invalidateConfigCache();
    } catch (err) { setError(String(err)); }
  }, []);

  const openFullPanel = useCallback(async () => {
    try { await invoke('open_full_panel'); } catch { /* ignore */ }
  }, []);

  const closeFullPanel = useCallback(async () => {
    try { await invoke('close_full_panel'); } catch { /* ignore */ }
  }, []);

  const setTheme = useCallback((t) => { setThemeState(t); }, []);

  const setLanguage = useCallback((lang) => { setOutputLanguage(lang); }, []);

  // ── Clear transient UI state ──────────────────────────────────────────────
  const clearError       = useCallback(() => setError(null), []);
  const clearComparison  = useCallback(() => setComparisonResult(null), []);

  // Promote a clipboard-monitor toast into a fresh overlay session. Used by
  // the "Use" button in MiniOverlay's clipboard toast.
  //
  // The Rust `set_selected_text` command handles EVERYTHING atomically:
  //   - updates `engine.last_text` (so subsequent mode invocations
  //     transform the new text, not the stale hotkey-captured value),
  //   - resets `last_result` / `last_entry_id` / `last_mode` / `last_app_hint`
  //     so `retry` doesn't silently run a stale mode against fresh text,
  //   - emits a full `quill://show_overlay` payload with `modes` + `chains`,
  //   - shows + focuses the mini window.
  //
  // The existing `show_overlay` listener already resets all React state, so
  // all this hook has to do is clear the toast and kick off the invoke.
  const promoteClipboardToast = useCallback(async (text) => {
    if (!text) return;
    // Cancel the pending auto-dismiss timer before clearing the toast —
    // otherwise it fires 6s later with a redundant setClipboardToast(null).
    if (clipboardToastTimerRef.current) {
      clearTimeout(clipboardToastTimerRef.current);
      clipboardToastTimerRef.current = null;
    }
    setClipboardToast(null);
    try {
      await invoke('set_selected_text', { text });
    } catch (err) {
      setError(String(err));
    }
  }, []);

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
    clearError, clearComparison, promoteClipboardToast,

    // Subscriptions
    onHistory, onTutorLesson, onTutorExplanation, onExportData,
  };
}
