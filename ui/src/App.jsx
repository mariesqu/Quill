import React, { useState, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import Overlay from "./components/Overlay";
import FirstRun from "./components/FirstRun";
import Settings from "./components/Settings";
import PermissionPrompt from "./components/PermissionPrompt";
import TutorPanel from "./components/TutorPanel";
import { useQuillBridge } from "./hooks/useQuillBridge";

export default function App() {
  const [view, setView] = useState("overlay"); // overlay | firstrun | settings | permission | tutor
  const bridge = useQuillBridge();

  useEffect(() => {
    const hasSetup = localStorage.getItem("quill_setup_complete");
    if (!hasSetup) setView("firstrun");
  }, []);

  useEffect(() => {
    const unsubs = [];
    listen("quill://permission_required", (e) => {
      if (e.payload === "accessibility") setView("permission");
    }).then((fn) => unsubs.push(fn));
    listen("quill://open_settings", () => {
      setView("settings");
      getCurrentWindow().show();
      getCurrentWindow().setFocus();
    }).then((fn) => unsubs.push(fn));
    listen("quill://open_tutor", () => {
      setView("tutor");
      getCurrentWindow().show();
      getCurrentWindow().setFocus();
    }).then((fn) => unsubs.push(fn));
    return () => unsubs.forEach((fn) => fn?.());
  }, []);

  if (view === "firstrun") {
    return (
      <FirstRun onComplete={() => {
        localStorage.setItem("quill_setup_complete", "true");
        setView("overlay");
      }} />
    );
  }
  if (view === "permission") {
    return <PermissionPrompt onDone={() => setView("overlay")} />;
  }
  if (view === "settings") {
    return <Settings onClose={() => { setView("overlay"); getCurrentWindow().hide(); }} bridge={bridge} />;
  }
  if (view === "tutor") {
    return <TutorPanel onClose={() => { setView("overlay"); getCurrentWindow().hide(); }} bridge={bridge} />;
  }

  return (
    <Overlay
      bridge={bridge}
      onOpenTutor={() => setView("tutor")}
    />
  );
}
