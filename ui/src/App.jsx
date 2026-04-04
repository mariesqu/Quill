import React, { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
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
    const unsubs = [
      listen("quill://permission_required", (e) => {
        if (e.payload === "accessibility") setView("permission");
      }),
      listen("quill://open_settings", () => setView("settings")),
      listen("quill://open_tutor",    () => setView("tutor")),
    ];
    return () => unsubs.forEach((p) => p.then((fn) => fn()));
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
    return <Settings onClose={() => setView("overlay")} bridge={bridge} />;
  }
  if (view === "tutor") {
    return <TutorPanel onClose={() => setView("overlay")} bridge={bridge} />;
  }

  return (
    <Overlay
      bridge={bridge}
      onOpenTutor={() => setView("tutor")}
    />
  );
}
