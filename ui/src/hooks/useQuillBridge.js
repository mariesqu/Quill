/**
 * Hook for IPC between the React UI and the Python sidecar via Tauri events.
 * Python → Tauri events: show_overlay, stream_chunk, stream_done, error, ready
 * Tauri → Python stdin:  mode_selected, replace_confirmed, dismissed, ping, save_config
 *
 * Language and persona are managed client-side:
 *   - language: persisted in localStorage, sent with every mode_selected command
 *   - persona: stored in config/user.yaml via save_config
 */
import { useState, useEffect, useCallback, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

const LS_LANGUAGE = "quill_output_language";

function loadPersistedLanguage() {
  return localStorage.getItem(LS_LANGUAGE) || "auto";
}

export function useQuillBridge() {
  const [visible, setVisible] = useState(false);
  const [selectedText, setSelectedText] = useState("");
  const [context, setContext] = useState({});
  const [modes, setModes] = useState([]);
  const [activeMode, setActiveMode] = useState(null);
  const [streamedText, setStreamedText] = useState("");
  const [isStreaming, setIsStreaming] = useState(false);
  const [isDone, setIsDone] = useState(false);
  const [error, setError] = useState(null);

  // Language picker state — persisted across sessions
  const [outputLanguage, setOutputLanguageState] = useState(loadPersistedLanguage);

  const streamBuffer = useRef("");

  const setOutputLanguage = useCallback((lang) => {
    localStorage.setItem(LS_LANGUAGE, lang);
    setOutputLanguageState(lang);
  }, []);

  useEffect(() => {
    const unlisteners = [];

    listen("quill://show_overlay", (event) => {
      const { text, context, modes } = event.payload;
      setSelectedText(text);
      setContext(context);
      setModes(modes);
      setActiveMode(null);
      setStreamedText("");
      setIsStreaming(false);
      setIsDone(false);
      setError(null);
      streamBuffer.current = "";
      setVisible(true);
    }).then((fn) => unlisteners.push(fn));

    listen("quill://stream_chunk", (event) => {
      streamBuffer.current += event.payload.chunk;
      setStreamedText(streamBuffer.current);
      setIsStreaming(true);
    }).then((fn) => unlisteners.push(fn));

    listen("quill://stream_done", (event) => {
      setStreamedText(event.payload.full_text);
      setIsStreaming(false);
      setIsDone(true);
    }).then((fn) => unlisteners.push(fn));

    listen("quill://error", (event) => {
      setError(event.payload.message);
      setIsStreaming(false);
    }).then((fn) => unlisteners.push(fn));

    return () => unlisteners.forEach((fn) => fn());
  }, []);

  const selectMode = useCallback(async (modeId, languageOverride) => {
    const lang = languageOverride ?? outputLanguage;
    setActiveMode(modeId);
    setStreamedText("");
    setIsStreaming(true);
    setIsDone(false);
    setError(null);
    streamBuffer.current = "";
    await invoke("send_to_python", {
      message: JSON.stringify({
        type: "mode_selected",
        mode: modeId,
        language: lang,
      }),
    });
  }, [outputLanguage]);

  const confirmReplace = useCallback(async () => {
    await invoke("send_to_python", {
      message: JSON.stringify({ type: "replace_confirmed" }),
    });
    setVisible(false);
  }, []);

  const dismiss = useCallback(async () => {
    await invoke("send_to_python", {
      message: JSON.stringify({ type: "dismissed" }),
    });
    setVisible(false);
    setActiveMode(null);
    setStreamedText("");
    setIsStreaming(false);
    setIsDone(false);
  }, []);

  const saveConfig = useCallback(async (config) => {
    await invoke("send_to_python", {
      message: JSON.stringify({ type: "save_config", config }),
    });
  }, []);

  return {
    visible,
    selectedText,
    context,
    modes,
    activeMode,
    streamedText,
    isStreaming,
    isDone,
    error,
    outputLanguage,
    setOutputLanguage,
    selectMode,
    confirmReplace,
    dismiss,
    saveConfig,
  };
}
