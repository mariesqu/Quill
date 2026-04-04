import React, { useState } from "react";
import "../styles/settings.css";

const PROVIDERS = [
  { id: "openrouter", label: "OpenRouter (free)" },
  { id: "ollama",     label: "Ollama (local)" },
  { id: "openai",     label: "OpenAI" },
  { id: "generic",   label: "Custom endpoint" },
];

const POSITIONS = [
  { id: "near_cursor", label: "Near cursor" },
  { id: "top_right",   label: "Top right" },
  { id: "top_left",    label: "Top left" },
];

const PERSONA_TONES = [
  { id: "natural",      label: "Natural",      desc: "Let mode guide the tone" },
  { id: "casual",       label: "Casual",       desc: "Friendly and conversational" },
  { id: "professional", label: "Professional", desc: "Polished, business-appropriate" },
  { id: "witty",        label: "Witty",        desc: "Clever, light humour" },
  { id: "direct",       label: "Direct",       desc: "Extremely concise, no fluff" },
  { id: "warm",         label: "Warm",         desc: "Empathetic, human" },
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

function ToneSelector({ value, onChange }) {
  return (
    <div className="tone-grid">
      {PERSONA_TONES.map((t) => (
        <button
          key={t.id}
          className={`tone-chip ${value === t.id ? "active" : ""}`}
          onClick={() => onChange(t.id)}
          title={t.desc}
        >
          {t.label}
        </button>
      ))}
    </div>
  );
}

export default function Settings({ onClose, bridge }) {
  const [saved, setSaved] = useState(false);
  const [activeTab, setActiveTab] = useState("provider"); // provider | behaviour | persona | tutor | about

  const stored = (() => {
    try { return JSON.parse(localStorage.getItem("quill_config_pending") || "{}"); }
    catch { return {}; }
  })();
  const storedPersona  = stored.persona  || {};
  const storedHistory  = stored.history  || {};
  const storedTutor    = stored.tutor    || {};

  // Provider
  const [provider, setProvider]   = useState(stored.provider || "openrouter");
  const [model, setModel]         = useState(stored.model || "google/gemma-3-27b-it");
  const [apiKey, setApiKey]       = useState(stored.api_key || "");
  const [baseUrl, setBaseUrl]     = useState(stored.base_url || "");

  // Behaviour
  const [hotkey, setHotkey]       = useState(stored.hotkey || "");
  const [overlayPos, setOverlayPos] = useState(stored.overlay_position || "near_cursor");
  const [stream, setStream]       = useState(stored.stream !== false);

  // Persona
  const [personaEnabled, setPersonaEnabled] = useState(storedPersona.enabled ?? false);
  const [personaTone, setPersonaTone]       = useState(storedPersona.tone || "natural");
  const [personaStyle, setPersonaStyle]     = useState(storedPersona.style || "");
  const [personaAvoid, setPersonaAvoid]     = useState(storedPersona.avoid || "");

  // History & Tutor
  const [historyEnabled, setHistoryEnabled]         = useState(storedHistory.enabled ?? false);
  const [tutorEnabled, setTutorEnabled]             = useState(storedTutor.enabled ?? false);
  const [tutorAutoExplain, setTutorAutoExplain]     = useState(storedTutor.auto_explain ?? false);

  const handleSave = async () => {
    const config = {
      provider, model,
      history: { enabled: historyEnabled },
      tutor:   { enabled: tutorEnabled, auto_explain: tutorAutoExplain },
      overlay_position: overlayPos,
      stream,
      persona: {
        enabled: personaEnabled,
        tone:    personaTone,
        style:   personaStyle.trim(),
        avoid:   personaAvoid.trim(),
      },
    };
    if (apiKey)   config.api_key  = apiKey;
    if (baseUrl)  config.base_url = baseUrl;
    if (hotkey)   config.hotkey   = hotkey;

    localStorage.setItem("quill_config_pending", JSON.stringify(config));
    if (bridge?.saveConfig) await bridge.saveConfig(config);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  const TABS = [
    { id: "provider",  label: "AI Provider" },
    { id: "behaviour", label: "Behaviour" },
    { id: "persona",   label: "My Voice" + (personaEnabled ? " ✦" : "") },
    { id: "tutor",     label: "AI Tutor" + (tutorEnabled ? " ✦" : "") },
    { id: "about",     label: "About" },
  ];

  return (
    <div className="settings-root">
      {/* Top bar */}
      <div className="settings-topbar">
        <div className="settings-topbar-title">🪶 Quill — Settings</div>
        <button className="overlay-close-btn" onClick={onClose} style={{ width: 28, height: 28 }}>
          ✕
        </button>
      </div>

      {/* Tab bar */}
      <div className="settings-tabs">
        {TABS.map((tab) => (
          <button
            key={tab.id}
            className={`settings-tab ${activeTab === tab.id ? "active" : ""}`}
            onClick={() => setActiveTab(tab.id)}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Body */}
      <div className="settings-body">

        {/* ── AI Provider ─────────────────────────────────────────────────── */}
        {activeTab === "provider" && (
          <>
            <div className="settings-section">
              <div className="settings-card">
                <div className="settings-row">
                  <div>
                    <div className="settings-row-label">Provider</div>
                    <div className="settings-row-desc">Where AI requests are sent</div>
                  </div>
                  <select className="settings-select" value={provider} onChange={(e) => setProvider(e.target.value)}>
                    {PROVIDERS.map((p) => <option key={p.id} value={p.id}>{p.label}</option>)}
                  </select>
                </div>

                <div className="settings-row">
                  <div>
                    <div className="settings-row-label">Model</div>
                    <div className="settings-row-desc">Model name for completions</div>
                  </div>
                  <input className="settings-input" type="text" value={model}
                    onChange={(e) => setModel(e.target.value)} placeholder="e.g. gpt-4o-mini" />
                </div>

                {["openrouter", "openai"].includes(provider) && (
                  <div className="settings-row">
                    <div>
                      <div className="settings-row-label">API Key</div>
                      <div className="settings-row-desc">Stored locally, never synced</div>
                    </div>
                    <input className="settings-input" type="password" value={apiKey}
                      onChange={(e) => setApiKey(e.target.value)} placeholder="sk-..." />
                  </div>
                )}

                {["generic", "ollama"].includes(provider) && (
                  <div className="settings-row">
                    <div>
                      <div className="settings-row-label">Base URL</div>
                      <div className="settings-row-desc">API endpoint URL</div>
                    </div>
                    <input className="settings-input" type="text" value={baseUrl}
                      onChange={(e) => setBaseUrl(e.target.value)}
                      placeholder={provider === "ollama" ? "http://localhost:11434" : "http://localhost:1234/v1"} />
                  </div>
                )}
              </div>
            </div>
          </>
        )}

        {/* ── Behaviour ───────────────────────────────────────────────────── */}
        {activeTab === "behaviour" && (
          <div className="settings-section">
            <div className="settings-card">
              <div className="settings-row">
                <div>
                  <div className="settings-row-label">Hotkey</div>
                  <div className="settings-row-desc">Leave empty for OS default</div>
                </div>
                <input className="settings-input" type="text" value={hotkey}
                  onChange={(e) => setHotkey(e.target.value)} placeholder="ctrl+shift+space" />
              </div>

              <div className="settings-row">
                <div><div className="settings-row-label">Overlay position</div></div>
                <select className="settings-select" value={overlayPos} onChange={(e) => setOverlayPos(e.target.value)}>
                  {POSITIONS.map((p) => <option key={p.id} value={p.id}>{p.label}</option>)}
                </select>
              </div>

              <div className="settings-row">
                <div>
                  <div className="settings-row-label">Stream responses</div>
                  <div className="settings-row-desc">Show text as it's generated</div>
                </div>
                <Toggle checked={stream} onChange={setStream} />
              </div>
            </div>
          </div>
        )}

        {/* ── My Voice / Persona ──────────────────────────────────────────── */}
        {activeTab === "persona" && (
          <div className="settings-section">
            {/* Enable toggle */}
            <div className="settings-card">
              <div className="settings-row">
                <div>
                  <div className="settings-row-label">Enable My Voice</div>
                  <div className="settings-row-desc">
                    Apply your style to all AI outputs — rewrites, translations, coaching, everything
                  </div>
                </div>
                <Toggle checked={personaEnabled} onChange={setPersonaEnabled} />
              </div>
            </div>

            {personaEnabled && (
              <>
                {/* Tone */}
                <div className="settings-section-title" style={{ marginTop: 8 }}>Tone</div>
                <div className="settings-card">
                  <div style={{ padding: "14px 16px" }}>
                    <ToneSelector value={personaTone} onChange={setPersonaTone} />
                    <div className="tone-desc">
                      {PERSONA_TONES.find((t) => t.id === personaTone)?.desc}
                    </div>
                  </div>
                </div>

                {/* Style notes */}
                <div className="settings-section-title" style={{ marginTop: 8 }}>Writing style</div>
                <div className="settings-card">
                  <div style={{ padding: "14px 16px", display: "flex", flexDirection: "column", gap: 8 }}>
                    <div className="settings-row-desc" style={{ marginBottom: 4 }}>
                      Describe how you write. The AI will mirror your style.
                    </div>
                    <textarea
                      className="persona-textarea"
                      value={personaStyle}
                      onChange={(e) => setPersonaStyle(e.target.value)}
                      placeholder={
                        "e.g. I write in short punchy sentences. I use em-dashes for emphasis. " +
                        "I avoid jargon and always get to the point quickly."
                      }
                      rows={4}
                    />
                    <div className="persona-char-hint">
                      {personaStyle.length > 0
                        ? `${personaStyle.length} characters`
                        : "Try describing a sentence structure, rhythm, or vocabulary you prefer."}
                    </div>
                  </div>
                </div>

                {/* Avoid */}
                <div className="settings-section-title" style={{ marginTop: 8 }}>Always avoid</div>
                <div className="settings-card">
                  <div style={{ padding: "14px 16px", display: "flex", flexDirection: "column", gap: 8 }}>
                    <div className="settings-row-desc" style={{ marginBottom: 4 }}>
                      Words, phrases, or patterns the AI should never use.
                    </div>
                    <input
                      className="settings-input"
                      type="text"
                      style={{ width: "100%" }}
                      value={personaAvoid}
                      onChange={(e) => setPersonaAvoid(e.target.value)}
                      placeholder="e.g. passive voice, corporate buzzwords, exclamation marks"
                    />
                  </div>
                </div>

                {/* Live preview */}
                <div className="persona-preview">
                  <div className="persona-preview-label">How this affects prompts</div>
                  <div className="persona-preview-box">
                    {[
                      personaTone !== "natural" && `Tone: ${PERSONA_TONES.find(t => t.id === personaTone)?.desc}`,
                      personaStyle.trim() && `Style: ${personaStyle.trim()}`,
                      personaAvoid.trim() && `Never use: ${personaAvoid.trim()}`,
                    ].filter(Boolean).join("\n") || "No constraints set yet."}
                  </div>
                </div>
              </>
            )}
          </div>
        )}

        {/* ── AI Tutor ────────────────────────────────────────────────────── */}
        {activeTab === "tutor" && (
          <div className="settings-section">
            <div className="settings-card">
              <div className="settings-row">
                <div>
                  <div className="settings-row-label">Enable History</div>
                  <div className="settings-row-desc">
                    Save all transformations locally in ~/.quill/history.db (opt-in)
                  </div>
                </div>
                <Toggle checked={historyEnabled} onChange={setHistoryEnabled} />
              </div>
            </div>

            {historyEnabled && (
              <div className="settings-card">
                <div className="settings-row">
                  <div>
                    <div className="settings-row-label">Enable AI Tutor</div>
                    <div className="settings-row-desc">
                      Generate personalised lessons and explain changes using your history
                    </div>
                  </div>
                  <Toggle checked={tutorEnabled} onChange={setTutorEnabled} />
                </div>
                {tutorEnabled && (
                  <div className="settings-row">
                    <div>
                      <div className="settings-row-label">Auto-explain changes</div>
                      <div className="settings-row-desc">
                        Automatically show tutor insight after every transformation
                      </div>
                    </div>
                    <Toggle checked={tutorAutoExplain} onChange={setTutorAutoExplain} />
                  </div>
                )}
              </div>
            )}

            {!historyEnabled && (
              <div style={{ padding: "12px 14px", background: "rgba(124,110,247,0.06)",
                border: "1px solid rgba(124,110,247,0.18)", borderRadius: "var(--radius-md)",
                fontSize: 12, color: "var(--color-text-muted)", lineHeight: 1.6 }}>
                🎓 Enable History above to unlock the AI Tutor. Your data stays
                100% local — nothing is ever sent to the cloud except the AI prompts
                you already send to your chosen provider.
              </div>
            )}
          </div>
        )}

        {/* ── About ───────────────────────────────────────────────────────── */}
        {activeTab === "about" && (
          <div className="settings-section">
            <div className="settings-card">
              <div className="settings-row">
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
        )}
      </div>

      {/* Save bar */}
      <div className="settings-save-bar">
        <button className="btn-copy" onClick={onClose}>Cancel</button>
        <button className="btn-continue" onClick={handleSave}>
          {saved ? "✓ Saved!" : "Save changes"}
        </button>
      </div>
    </div>
  );
}
