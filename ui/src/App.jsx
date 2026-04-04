import React, { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import Overlay from "./components/Overlay";
import FirstRun from "./components/FirstRun";
import Settings from "./components/Settings";
import PermissionPrompt from "./components/PermissionPrompt";
import { useQuillBridge } from "./hooks/useQuillBridge";

export default function App() {
  const [view, setView] = useState("overlay"); // overlay | firstrun | settings | permission
  const bridge = useQuillBridge();

  useEffect(() => {
    // Check if first run
    const hasCompletedSetup = localStorage.getItem("quill_setup_complete");
    if (!hasCompletedSetup) {
      setView("firstrun");
    }
  }, []);

  useEffect(() => {
    const unlisten = listen("quill://permission_required", (event) => {
      if (event.payload === "accessibility") {
        setView("permission");
      }
    });
    return () => unlisten.then((fn) => fn());
  }, []);

  useEffect(() => {
    const unlisten = listen("quill://open_settings", () => {
      setView("settings");
    });
    return () => unlisten.then((fn) => fn());
  }, []);

  if (view === "firstrun") {
    return (
      <FirstRun
        onComplete={() => {
          localStorage.setItem("quill_setup_complete", "true");
          setView("overlay");
        }}
      />
    );
  }

  if (view === "permission") {
    return <PermissionPrompt onDone={() => setView("overlay")} />;
  }

  if (view === "settings") {
    return <Settings onClose={() => setView("overlay")} bridge={bridge} />;
  }

  return <Overlay bridge={bridge} />;
}
