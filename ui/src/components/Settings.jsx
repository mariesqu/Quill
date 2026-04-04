import React, { useState } from "react";
import "../styles/settings.css";

const PROVIDERS = [
  { id: "openrouter", label: "OpenRouter (free)" },
  { id: "ollama", label: "Ollama (local)" },
  { id: "openai", label: "OpenAI" },
  { id: "generic", label: "Custom endpoint" },
];

const POSITIONS = [
  { id: "near_cursor", label: "Near cursor" },
  { id: "top_right", label: "Top right" },
  { id: "top_left", label: "Top left" },
];

function Toggle({ checked, onChange }) {
  return (
    <label className="toggle">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <div className="toggle-track">
        <div className="toggle-thumb" />
      </div>
    </label>
  );
}

export default function Settings({ onClose, bridge }) {
  const [saved, setSaved] = useState(false);

  // Load from localStorage as proxy for real config
  const stored = (() => {
    try {
      return JSON.parse(localStorage.getItem("quill_config_pending") || "{}");
    } catch {
      return {};
    }
  })();

  const [provider, setProvider] = useState(stored.provider || "openrouter");
  const [model, setModel] = useState(stored.model || "google/gemma-3-27b-it");
  const [apiKey, setApiKey] = useState(stored.api_key || "");
  const [baseUrl, setBaseUrl] = useState(stored.base_url || "");
  const [hotkey, setHotkey] = useState(stored.hotkey || "");
  const [overlayPos, setOverlayPos] = useState(stored.overlay_position || "near_cursor");
  const [stream, setStream] = useState(stored.stream !== false);
  const [language, setLanguage] = useState(stored.language || "auto");

  const handleSave = async () => {
    const config = { provider, model, overlay_position: overlayPos, stream, language };
    if (apiKey) config.api_key = apiKey;
    if (baseUrl) config.base_url = baseUrl;
    if (hotkey) config.hotkey = hotkey;

    localStorage.setItem("quill_config_pending", JSON.stringify(config));
    if (bridge?.saveConfig) {
      await bridge.saveConfig(config);
    }
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  return (
    <div className="settings-root">
      {/* Top bar */}
      <div className="settings-topbar">
        <div className="settings-topbar-title">
          🪶 Quill — Settings
        </div>
        <button
          className="overlay-close-btn"
          onClick={onClose}
          style={{ width: 28, height: 28 }}
        >
          ✕
        </button>
      </div>

      {/* Body */}
      <div className="settings-body">
        {/* AI Provider */}
        <div className="settings-section">
          <div className="settings-section-title">AI Provider</div>
          <div className="settings-card">
            <div className="settings-row">
              <div>
                <div className="settings-row-label">Provider</div>
                <div className="settings-row-desc">Where AI requests are sent</div>
              </div>
              <select
                className="settings-select"
                value={provider}
                onChange={(e) => setProvider(e.target.value)}
              >
                {PROVIDERS.map((p) => (
                  <option key={p.id} value={p.id}>{p.label}</option>
                ))}
              </select>
            </div>

            <div className="settings-row">
              <div>
                <div className="settings-row-label">Model</div>
                <div className="settings-row-desc">Model name used for completions</div>
              </div>
              <input
                className="settings-input"
                type="text"
                value={model}
                onChange={(e) => setModel(e.target.value)}
                placeholder="e.g. gpt-4o-mini"
              />
            </div>

            {["openrouter", "openai"].includes(provider) && (
              <div className="settings-row">
                <div>
                  <div className="settings-row-label">API Key</div>
                  <div className="settings-row-desc">Stored locally, never synced</div>
                </div>
                <input
                  className="settings-input"
                  type="password"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  placeholder="sk-..."
                />
              </div>
            )}

            {["generic", "ollama"].includes(provider) && (
              <div className="settings-row">
                <div>
                  <div className="settings-row-label">Base URL</div>
                  <div className="settings-row-desc">API endpoint URL</div>
                </div>
                <input
                  className="settings-input"
                  type="text"
                  value={baseUrl}
                  onChange={(e) => setBaseUrl(e.target.value)}
                  placeholder={
                    provider === "ollama"
                      ? "http://localhost:11434"
                      : "http://localhost:1234/v1"
                  }
                />
              </div>
            )}
          </div>
        </div>

        {/* Behaviour */}
        <div className="settings-section">
          <div className="settings-section-title">Behaviour</div>
          <div className="settings-card">
            <div className="settings-row">
              <div>
                <div className="settings-row-label">Hotkey</div>
                <div className="settings-row-desc">Leave empty for OS default</div>
              </div>
              <input
                className="settings-input"
                type="text"
                value={hotkey}
                onChange={(e) => setHotkey(e.target.value)}
                placeholder="ctrl+shift+space"
              />
            </div>

            <div className="settings-row">
              <div>
                <div className="settings-row-label">Overlay position</div>
              </div>
              <select
                className="settings-select"
                value={overlayPos}
                onChange={(e) => setOverlayPos(e.target.value)}
              >
                {POSITIONS.map((p) => (
                  <option key={p.id} value={p.id}>{p.label}</option>
                ))}
              </select>
            </div>

            <div className="settings-row">
              <div>
                <div className="settings-row-label">Stream responses</div>
                <div className="settings-row-desc">Show text as it's generated</div>
              </div>
              <Toggle checked={stream} onChange={setStream} />
            </div>

            <div className="settings-row">
              <div>
                <div className="settings-row-label">Translation language</div>
                <div className="settings-row-desc">Target for Translate mode</div>
              </div>
              <input
                className="settings-input"
                type="text"
                value={language}
                onChange={(e) => setLanguage(e.target.value)}
                placeholder="auto"
                style={{ width: 120 }}
              />
            </div>
          </div>
        </div>

        {/* About */}
        <div className="settings-section">
          <div className="settings-section-title">About</div>
          <div className="settings-card">
            <div className="settings-row" style={{ justifyContent: "space-between" }}>
              <div className="settings-row-label">Version</div>
              <span style={{ color: "var(--color-text-muted)", fontSize: 13 }}>0.1.0</span>
            </div>
            <div className="settings-row">
              <div className="settings-row-label">License</div>
              <span style={{ color: "var(--color-text-muted)", fontSize: 13 }}>MIT</span>
            </div>
            <div className="settings-row">
              <div className="settings-row-label">Source</div>
              <span style={{ color: "var(--color-primary)", fontSize: 13 }}>
                github.com/mariesqu/Quill
              </span>
            </div>
          </div>
        </div>
      </div>

      {/* Save bar */}
      <div className="settings-save-bar">
        <button className="btn-copy" onClick={onClose}>
          Cancel
        </button>
        <button className="btn-continue" onClick={handleSave}>
          {saved ? "✓ Saved!" : "Save changes"}
        </button>
      </div>
    </div>
  );
}
