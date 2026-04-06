/**
 * Central IPC bridge between React UI and Python sidecar.
 *
 * Python → Tauri events:
 *   show_overlay, stream_chunk, stream_done, chain_step,
 *   error, ready, smart_suggestion,
 *   tutor_explanation, tutor_lesson, history,
 *   favorite_toggled, export_data, comparison_done,
 *   pronunciation, clipboard_change, templates_updated
 *
 * Tauri → Python stdin (via invoke "send_to_python"):
 *   mode_selected, chain_selected, retry, replace_confirmed,
 *   dismissed, tutor_explain, generate_lesson, get_history,
 *   toggle_favorite, export_history, compare_modes,
 *   get_pronunciation, save_template, delete_template,
 *   ping, save_config
 */
import { useState, useEffect, useCallback, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

const LS_LANGUAGE = "quill_output_language";
const LS_THEME    = "quill_theme";
const UNDO_STACK_LIMIT = 20;

export function loadPersistedTheme() {
  return localStorage.getItem(LS_THEME) || "dark";
}

function loadPersistedLanguage() {
  return localStorage.getItem(LS_LANGUAGE) || "auto";
}

async function sendToPython(msg) {
  await invoke("send_to_python", { message: JSON.stringify(msg) });
}

export function useQuillBridge() {
  // ── Overlay state ──────────────────────────────────────────────────────────
  const [visible, setVisible]           = useState(false);
  const [selectedText, setSelectedText] = useState("");
  const [context, setContext]           = useState({});
  const [modes, setModes]               = useState([]);
  const [chains, setChains]             = useState([]);
  const [activeMode, setActiveMode]     = useState(null);
  const [streamedText, setStreamedText] = useState("");
  const [isStreaming, setIsStreaming]   = useState(false);
  const [isDone, setIsDone]             = useState(false);
  const [error, setError]               = useState(null);
  const [lastEntryId, setLastEntryId]   = useState(null);
  const [chainProgress, setChainProgress] = useState(null);
  const [suggestion, setSuggestion]     = useState(null);

  // ── Language picker ────────────────────────────────────────────────────────
  const [outputLanguage, setOutputLanguageState] = useState(loadPersistedLanguage);
  const setOutputLanguage = useCallback((lang) => {
    localStorage.setItem(LS_LANGUAGE, lang);
    setOutputLanguageState(lang);
  }, []);

  // ── Undo stack ─────────────────────────────────────────────────────────────
  const outputStack  = useRef([]); // [{text, mode, entryId}]
  const activeModeRef = useRef(null); // mirrors activeMode state for use in event closures
  const [canUndo, setCanUndo] = useState(false);

  // ── Tutor state ────────────────────────────────────────────────────────────
  const [tutorExplanation, setTutorExplanation] = useState(null);
  const [isExplaining, setIsExplaining]         = useState(false);

  // ── Comparison state ───────────────────────────────────────────────────────
  const [comparisonResult, setComparisonResult] = useState(null); // {mode_a, result_a, mode_b, result_b}
  const [isComparing, setIsComparing]           = useState(false);
  const [compareMode, setCompareMode]           = useState(false); // UI toggle

  // ── Pronunciation state ────────────────────────────────────────────────────
  const [pronunciation, setPronunciation]   = useState(null);
  const [isPronouncing, setIsPronouncing]   = useState(false);

  // ── Clipboard notification ─────────────────────────────────────────────────
  const [clipboardToast, setClipboardToast] = useState(null); // text | null

  // ── Templates ─────────────────────────────────────────────────────────────
  const [templates, setTemplates] = useState([]);

  // ── Theme ─────────────────────────────────────────────────────────────────
  const [theme, setThemeState] = useState(loadPersistedTheme);
  const setTheme = useCallback((t) => {
    localStorage.setItem(LS_THEME, t);
    setThemeState(t);
    document.documentElement.setAttribute("data-theme", t);
  }, []);

  // Apply theme whenever it changes
  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
  }, [theme]);

  // External listener registries
  const tutorLessonListeners  = useRef([]);
  const tutorExplainListeners = useRef([]);
  const historyListeners      = useRef([]);
  const exportListeners       = useRef([]);

  const streamBuffer = useRef("");

  // ── Listen to Python events ────────────────────────────────────────────────
  useEffect(() => {
    const unsubs = [];

    listen("quill://show_overlay", (e) => {
      const { text, context, modes, chains } = e.payload;
      setSelectedText(text);
      setContext(context);
      setModes(modes);
      setChains(chains || []);
      setActiveMode(null);
      setStreamedText("");
      setIsStreaming(false);
      setIsDone(false);
      setError(null);
      setTutorExplanation(null);
      setChainProgress(null);
      setSuggestion(null);
      setComparisonResult(null);
      setIsComparing(false);
      setPronunciation(null);
      outputStack.current = [];
      setCanUndo(false);
      streamBuffer.current = "";
      setVisible(true);
    }).then((fn) => unsubs.push(fn));

    listen("quill://smart_suggestion", (e) => {
      setSuggestion(e.payload);
    }).then((fn) => unsubs.push(fn));

    listen("quill://stream_chunk", (e) => {
      streamBuffer.current += e.payload.chunk;
      setStreamedText(streamBuffer.current);
      setIsStreaming(true);
    }).then((fn) => unsubs.push(fn));

    listen("quill://stream_done", (e) => {
      const text = e.payload.full_text;
      const eid  = e.payload.entry_id;
      setStreamedText(text);
      setIsStreaming(false);
      setIsDone(true);
      setChainProgress(null);
      if (eid) setLastEntryId(eid);
      // Push to undo stack — capture current mode via ref (safe in event closure)
      outputStack.current = [
        { text, mode: activeModeRef.current, entryId: eid },
        ...outputStack.current,
      ].slice(0, UNDO_STACK_LIMIT);
      setCanUndo(outputStack.current.length > 1);
    }).then((fn) => unsubs.push(fn));

    listen("quill://chain_step", (e) => {
      setChainProgress(e.payload);
      streamBuffer.current = "";
      setStreamedText("");
    }).then((fn) => unsubs.push(fn));

    listen("quill://error", (e) => {
      setError(e.payload.message);
      setIsStreaming(false);
      setIsExplaining(false);
      setIsComparing(false);
      setIsPronouncing(false);
    }).then((fn) => unsubs.push(fn));

    listen("quill://tutor_explanation", (e) => {
      const { explanation, entry_id } = e.payload;
      setTutorExplanation(explanation);
      setIsExplaining(false);
      tutorExplainListeners.current.forEach((fn) => fn(explanation, entry_id));
    }).then((fn) => unsubs.push(fn));

    listen("quill://tutor_lesson", (e) => {
      const { lesson_md, period } = e.payload;
      tutorLessonListeners.current.forEach((fn) => fn(lesson_md, period));
    }).then((fn) => unsubs.push(fn));

    listen("quill://history", (e) => {
      // Call with (entries, null) — consistent two-arg signature
      historyListeners.current.forEach((fn) => fn(e.payload.entries, null));
    }).then((fn) => unsubs.push(fn));

    listen("quill://favorite_toggled", (e) => {
      const { entry_id, favorited } = e.payload;
      // Call with (null, {entry_id, favorited}) — history listeners handle both cases
      historyListeners.current.forEach((fn) => fn(null, { entry_id, favorited }));
    }).then((fn) => unsubs.push(fn));

    listen("quill://export_data", (e) => {
      exportListeners.current.forEach((fn) => fn(e.payload.entries, e.payload.format));
    }).then((fn) => unsubs.push(fn));

    listen("quill://comparison_done", (e) => {
      setComparisonResult(e.payload);
      setIsComparing(false);
      setIsDone(true);
    }).then((fn) => unsubs.push(fn));

    listen("quill://pronunciation", (e) => {
      setPronunciation(e.payload.text);
      setIsPronouncing(false);
    }).then((fn) => unsubs.push(fn));

    listen("quill://clipboard_change", (e) => {
      setClipboardToast(e.payload.text);
    }).then((fn) => unsubs.push(fn));

    listen("quill://templates_updated", (e) => {
      setTemplates(e.payload.templates || []);
    }).then((fn) => unsubs.push(fn));

    return () => unsubs.forEach((fn) => fn());
  }, []);

  // ── Overlay actions ────────────────────────────────────────────────────────

  const selectMode = useCallback(async (modeId, extraInstruction = "") => {
    activeModeRef.current = modeId;
    setActiveMode(modeId);
    setStreamedText("");
    setIsStreaming(true);
    setIsDone(false);
    setError(null);
    setTutorExplanation(null);
    setComparisonResult(null);
    setPronunciation(null);
    streamBuffer.current = "";
    await sendToPython({
      type: "mode_selected",
      mode: modeId,
      language: outputLanguage,
      extra_instruction: extraInstruction,
    });
  }, [outputLanguage]);

  const selectChain = useCallback(async (chainId, extraInstruction = "") => {
    activeModeRef.current = `chain:${chainId}`;
    setActiveMode(`chain:${chainId}`);
    setStreamedText("");
    setIsStreaming(true);
    setIsDone(false);
    setError(null);
    setTutorExplanation(null);
    setComparisonResult(null);
    streamBuffer.current = "";
    await sendToPython({
      type: "chain_selected",
      chain_id: chainId,
      language: outputLanguage,
      extra_instruction: extraInstruction,
    });
  }, [outputLanguage]);

  const retry = useCallback(async (extraInstruction = "") => {
    setStreamedText("");
    setIsStreaming(true);
    setIsDone(false);
    setError(null);
    setTutorExplanation(null);
    setPronunciation(null);
    streamBuffer.current = "";
    await sendToPython({ type: "retry", extra_instruction: extraInstruction });
  }, []);

  const undo = useCallback(async () => {
    if (outputStack.current.length < 2) return;
    const newStack = outputStack.current.slice(1);
    const prev = newStack[0];
    try {
      await sendToPython({ type: "set_result", text: prev.text });
      // Only mutate state after successful sync to Python
      outputStack.current = newStack;
      setStreamedText(prev.text);
      setIsDone(true);
      setCanUndo(newStack.length > 1);
    } catch (err) {
      console.error("Undo sync failed:", err);
    }
  }, []);

  const compareModes = useCallback(async (modeA, modeB, extraInstruction = "") => {
    setIsComparing(true);
    setComparisonResult(null);
    setError(null);
    await sendToPython({
      type: "compare_modes",
      mode_a: modeA,
      mode_b: modeB,
      language: outputLanguage,
      extra_instruction: extraInstruction,
    });
  }, [outputLanguage]);

  const getPronunciation = useCallback(async (text, language) => {
    setIsPronouncing(true);
    setPronunciation(null);
    await sendToPython({ type: "get_pronunciation", text, language });
  }, []);

  const setResultText = useCallback(async (text) => {
    await sendToPython({ type: "set_result", text });
  }, []);

  const confirmReplace = useCallback(async () => {
    await sendToPython({ type: "replace_confirmed" });
    setVisible(false);
  }, []);

  const dismiss = useCallback(async () => {
    await sendToPython({ type: "dismissed" });
    setVisible(false);
    setActiveMode(null);
    setStreamedText("");
    setIsStreaming(false);
    setIsDone(false);
    setTutorExplanation(null);
    setComparisonResult(null);
    setPronunciation(null);
    setCompareMode(false);
  }, []);

  // ── Tutor actions ──────────────────────────────────────────────────────────

  const requestTutorExplain = useCallback(async (entryId) => {
    setIsExplaining(true);
    setTutorExplanation(null);
    await sendToPython({ type: "tutor_explain", entry_id: entryId ?? lastEntryId });
  }, [lastEntryId]);

  const generateLesson = useCallback(async (period = "daily") => {
    await sendToPython({ type: "generate_lesson", period });
  }, []);

  const getHistory = useCallback(async (limit = 50, language = null) => {
    await sendToPython({ type: "get_history", limit, language });
  }, []);

  const tutorExplain = useCallback(async (entryId) => {
    await sendToPython({ type: "tutor_explain", entry_id: entryId });
  }, []);

  // ── Favorites ──────────────────────────────────────────────────────────────

  const toggleFavorite = useCallback(async (entryId) => {
    await sendToPython({ type: "toggle_favorite", entry_id: entryId });
  }, []);

  // ── Export ─────────────────────────────────────────────────────────────────

  const exportHistory = useCallback(async (format = "json") => {
    await sendToPython({ type: "export_history", format });
  }, []);

  // ── Templates ──────────────────────────────────────────────────────────────

  const saveTemplate = useCallback(async (name, mode, instruction) => {
    await sendToPython({ type: "save_template", name, mode, instruction });
  }, []);

  const deleteTemplate = useCallback(async (name) => {
    await sendToPython({ type: "delete_template", name });
  }, []);

  const dismissClipboardToast = useCallback(() => setClipboardToast(null), []);

  // ── External subscriptions (for TutorPanel) ────────────────────────────────

  const onTutorLesson = useCallback((fn) => {
    tutorLessonListeners.current.push(fn);
    return () => {
      tutorLessonListeners.current = tutorLessonListeners.current.filter((f) => f !== fn);
    };
  }, []);

  const onTutorExplanation = useCallback((fn) => {
    tutorExplainListeners.current.push(fn);
    return () => {
      tutorExplainListeners.current = tutorExplainListeners.current.filter((f) => f !== fn);
    };
  }, []);

  const onHistory = useCallback((fn) => {
    historyListeners.current.push(fn);
    return () => {
      historyListeners.current = historyListeners.current.filter((f) => f !== fn);
    };
  }, []);

  const onExportData = useCallback((fn) => {
    exportListeners.current.push(fn);
    return () => {
      exportListeners.current = exportListeners.current.filter((f) => f !== fn);
    };
  }, []);

  const saveConfig = useCallback(async (config) => {
    await sendToPython({ type: "save_config", config });
  }, []);

  return {
    // Overlay state
    visible, selectedText, context, modes, chains,
    activeMode, streamedText, isStreaming, isDone,
    error, lastEntryId, chainProgress, suggestion,
    // Language
    outputLanguage, setOutputLanguage,
    // Undo
    canUndo, undo,
    // Tutor state
    tutorExplanation, isExplaining,
    // Comparison
    comparisonResult, isComparing, compareMode, setCompareMode, compareModes,
    // Pronunciation
    pronunciation, isPronouncing, getPronunciation,
    // Clipboard
    clipboardToast, dismissClipboardToast,
    // Templates
    templates, saveTemplate, deleteTemplate,
    // Theme
    theme, setTheme,
    // Overlay actions
    selectMode, selectChain, retry,
    setResultText, confirmReplace, dismiss,
    // Tutor actions
    requestTutorExplain, generateLesson,
    getHistory, tutorExplain,
    // Favorites
    toggleFavorite,
    // Export
    exportHistory,
    // External subscriptions
    onTutorLesson, onTutorExplanation, onHistory, onExportData,
    // Config
    saveConfig,
  };
}
