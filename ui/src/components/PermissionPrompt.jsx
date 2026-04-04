import React from "react";
import { invoke } from "@tauri-apps/api/core";
import "../styles/firstrun.css";

export default function PermissionPrompt({ onDone }) {
  const handleOpenSettings = async () => {
    await invoke("open_accessibility_settings");
  };

  return (
    <div className="firstrun-root">
      <div className="firstrun-card">
        <div className="firstrun-hero">
          <div className="firstrun-logo">🔐</div>
          <h1 className="firstrun-title">One permission needed</h1>
          <p className="firstrun-subtitle">
            macOS requires Accessibility access to read selected text from other apps.
          </p>
        </div>

        <div className="firstrun-body">
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              gap: 12,
              padding: "4px 0",
            }}
          >
            {[
              ['1', 'Click "Open System Settings" below'],
              ['2', 'Find "Quill" in the Accessibility list'],
              ['3', 'Toggle it ON'],
              ['4', 'Relaunch Quill'],
            ].map(([num, text]) => (
              <div
                key={num}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 12,
                }}
              >
                <div
                  style={{
                    width: 24,
                    height: 24,
                    borderRadius: "50%",
                    background: "var(--color-primary-dim)",
                    border: "1px solid rgba(124, 110, 247, 0.3)",
                    color: "var(--color-primary)",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    fontSize: 12,
                    fontWeight: 700,
                    flexShrink: 0,
                  }}
                >
                  {num}
                </div>
                <span style={{ fontSize: 13, color: "var(--color-text-muted)" }}>{text}</span>
              </div>
            ))}
          </div>

          <div
            style={{
              marginTop: 20,
              padding: "12px 14px",
              background: "rgba(74, 222, 128, 0.06)",
              border: "1px solid rgba(74, 222, 128, 0.18)",
              borderRadius: "var(--radius-md)",
              fontSize: 12,
              color: "rgba(74, 222, 128, 0.8)",
              display: "flex",
              gap: 8,
              alignItems: "flex-start",
            }}
          >
            <span style={{ flexShrink: 0 }}>🔒</span>
            <span style={{ lineHeight: 1.5 }}>
              Your text never leaves your Mac unless you configure a cloud provider.
              Accessibility access is a macOS requirement — Quill reads selected text only.
            </span>
          </div>
        </div>

        <div className="firstrun-footer" style={{ justifyContent: "flex-end", gap: 10 }}>
          <button
            className="btn-copy"
            onClick={onDone}
            style={{ padding: "9px 16px", fontSize: 13 }}
          >
            I'll do it later
          </button>
          <button className="btn-continue" onClick={handleOpenSettings}>
            Open System Settings →
          </button>
        </div>
      </div>
    </div>
  );
}
