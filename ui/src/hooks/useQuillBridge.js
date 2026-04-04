/**
 * Central IPC bridge between React UI and Python sidecar.
 *
 * Python → Tauri events:
 *   show_overlay, stream_chunk, stream_done, chain_step,
 *   error, ready, smart_suggestion,
 *   tutor_explanation, tutor_lesson, history
 *
 * Tauri → Python stdin (via invoke "send_to_python"):
 *   mode_selected, chain_selected, retry, replace_confirmed,
 *   dismissed, tutor_explain, generate_lesson, get_history,
 *   ping, save_config
 */
import { useState, useEffect, useCallback, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

const LS_LANGUAGE = "quill_output_language";

function loadPersistedLanguage() {
  return localStorage.getItem(LS_LANGUAGE) || "auto";
}

async function sendToPython(msg) {
  await invoke("send_to_python", { message: JSON.stringify(msg) });
}

export function useQuillBridge() {
  // ── Overlay state ──────────────────────────────────────────────────────────
  const [visible, setVisible]         = useState(false);
  const [selectedText, setSelectedText] = useState("");
  const [context, setContext]         = useState({});
  const [modes, setModes]             = useState([]);
  const [chains, setChains]           = useState([]);
  const [activeMode, setActiveMode]   = useState(null);
  const [streamedText, setStreamedText] = useState("");
  const [isStreaming, setIsStreaming] = useState(false);
  const [isDone, setIsDone]           = useState(false);
  const [error, setError]             = useState(null);
  const [lastEntryId, setLastEntryId] = useState(null);
  const [chainProgress, setChainProgress] = useState(null); // { step, total, mode }
  const [suggestion, setSuggestion]   = useState(null);     // { mode_id, reason }

  // ── Language picker ────────────────────────────────────────────────────────
  const [outputLanguage, setOutputLanguageState] = useState(loadPersistedLanguage);
  const setOutputLanguage = useCallback((lang) => {
    localStorage.setItem(LS_LANGUAGE, lang);
    setOutputLanguageState(lang);
  }, []);

  // ── Tutor state ────────────────────────────────────────────────────────────
  const [tutorExplanation, setTutorExplanation] = useState(null);
  const [isExplaining, setIsExplaining]         = useState(false);

  // External listener registries (for TutorPanel subscriptions)
  const tutorLessonListeners     = useRef([]);
  const tutorExplainListeners    = useRef([]);
  const historyListeners         = useRef([]);

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
      setStreamedText(e.payload.full_text);
      setIsStreaming(false);
      setIsDone(true);
      setChainProgress(null);
      if (e.payload.entry_id) setLastEntryId(e.payload.entry_id);
    }).then((fn) => unsubs.push(fn));

    listen("quill://chain_step", (e) => {
      setChainProgress(e.payload);
      // Reset buffer for each step's streaming
      streamBuffer.current = "";
      setStreamedText("");
    }).then((fn) => unsubs.push(fn));

    listen("quill://error", (e) => {
      setError(e.payload.message);
      setIsStreaming(false);
      setIsExplaining(false);
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
      historyListeners.current.forEach((fn) => fn(e.payload.entries));
    }).then((fn) => unsubs.push(fn));

    return () => unsubs.forEach((fn) => fn());
  }, []);

  // ── Overlay actions ────────────────────────────────────────────────────────

  const selectMode = useCallback(async (modeId, extraInstruction = "") => {
    setActiveMode(modeId);
    setStreamedText("");
    setIsStreaming(true);
    setIsDone(false);
    setError(null);
    setTutorExplanation(null);
    streamBuffer.current = "";
    await sendToPython({
      type: "mode_selected",
      mode: modeId,
      language: outputLanguage,
      extra_instruction: extraInstruction,
    });
  }, [outputLanguage]);

  const selectChain = useCallback(async (chainId, extraInstruction = "") => {
    setActiveMode(`chain:${chainId}`);
    setStreamedText("");
    setIsStreaming(true);
    setIsDone(false);
    setError(null);
    setTutorExplanation(null);
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
    streamBuffer.current = "";
    await sendToPython({ type: "retry", extra_instruction: extraInstruction });
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
    // Tutor state
    tutorExplanation, isExplaining,
    // Overlay actions
    selectMode, selectChain, retry,
    confirmReplace, dismiss,
    // Tutor actions
    requestTutorExplain, generateLesson,
    getHistory, tutorExplain,
    // External subscriptions
    onTutorLesson, onTutorExplanation, onHistory,
    // Config
    saveConfig,
  };
}
