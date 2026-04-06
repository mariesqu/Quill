import React, { useState, useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "../styles/settings.css";

const startDrag = (e) => {
  if (e.button === 0 && e.target === e.currentTarget) getCurrentWindow().startDragging();
};

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
      <div className="toggle-track"><div className="toggle-thumb" /></div>
    </label>
  );
}

function ToneSelector({ value, onChange }) {
  return (
    <div className="tone-grid">
      {PERSONA_TONES.map((t) => (
        <button key={t.id} className={`tone-chip ${value === t.id ? "active" : ""}`}
          onClick={() => onChange(t.id)} title={t.desc}>
          {t.label}
        </button>
      ))}
    </div>
  );
}

// ── Templates management ───────────────────────────────────────────────────────

// Receives templates as a prop so it re-renders when the parent state updates
function TemplatesEditor({ templates, onSave, onDelete }) {
  const [name, setName]           = useState("");
  const [mode, setMode]           = useState("rewrite");
  const [instruction, setInstruction] = useState("");
  const [saved, setSaved]         = useState(false);

  const MODES = ["rewrite","translate","coach","shorter","formal","fix_grammar","expand"];

  const handleAdd = async () => {
    if (!name.trim()) return;
    await onSave(name.trim(), mode, instruction.trim());
    setName(""); setInstruction("");
    setSaved(true); setTimeout(() => setSaved(false), 1500);
  };

  const handleDelete = async (tplName) => {
    await onDelete(tplName);
  };

  const currentTemplates = templates || [];

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
      {/* Existing templates */}
      {currentTemplates.length > 0 && (
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          {currentTemplates.map((tpl) => (
            <div key={tpl.name} style={{
              display: "flex", alignItems: "center", gap: 8,
              padding: "8px 12px",
              background: "var(--color-surface-2)",
              border: "1px solid var(--color-border)",
              borderRadius: "var(--radius-sm)",
            }}>
              <span style={{ fontSize: 13, fontWeight: 600, flex: 1 }}>⚡ {tpl.name}</span>
              <span style={{ fontSize: 11, color: "var(--color-text-dim)" }}>
                {tpl.mode}{tpl.instruction ? ` · "${tpl.instruction}"` : ""}
              </span>
              <button onClick={() => handleDelete(tpl.name)}
                style={{ background: "none", border: "none",
                  color: "var(--color-error)", cursor: "pointer", fontSize: 13, padding: "0 4px" }}>
                ✕
              </button>
            </div>
          ))}
        </div>
      )}

      {/* Add new */}
      <div style={{ display: "flex", flexDirection: "column", gap: 6,
        padding: "12px", background: "var(--color-surface-2)",
        border: "1px solid var(--color-border)", borderRadius: "var(--radius-md)" }}>
        <div style={{ fontSize: 11, fontWeight: 700, textTransform: "uppercase",
          letterSpacing: "0.08em", color: "var(--color-text-dim)", marginBottom: 2 }}>
          Add template
        </div>
        <input className="settings-input" type="text" placeholder="Template name (e.g. Slack update)"
          value={name} onChange={(e) => setName(e.target.value)} style={{ width: "100%" }} />
        <div style={{ display: "flex", gap: 6 }}>
          <select className="settings-select" value={mode} onChange={(e) => setMode(e.target.value)}>
            {MODES.map((m) => <option key={m} value={m}>{m}</option>)}
          </select>
          <input className="settings-input" type="text"
            placeholder="Instruction (optional)"
            value={instruction} onChange={(e) => setInstruction(e.target.value)}
            style={{ flex: 1 }} />
        </div>
        <button className="btn-continue" onClick={handleAdd} disabled={!name.trim()}
          style={{ alignSelf: "flex-end", padding: "6px 16px" }}>
          {saved ? "✓ Saved" : "+ Add"}
        </button>
      </div>
    </div>
  );
}

// ── Main Settings component ────────────────────────────────────────────────────

export default function Settings({ onClose, bridge }) {
  const [saved, setSaved]   = useState(false);
  const [activeTab, setActiveTab] = useState("provider");
  // Templates state — initialized from bridge and updated when bridge.templates changes
  const [templates, setTemplates] = useState(bridge?.templates || []);
  useEffect(() => {
    setTemplates(bridge?.templates || []);
  }, [bridge?.templates]);

  const stored = (() => {
    try { return JSON.parse(localStorage.getItem("quill_config_pending") || "{}"); }
    catch { return {}; }
  })();
  const storedPersona  = stored.persona  || {};
  const storedHistory  = stored.history  || {};
  const storedTutor    = stored.tutor    || {};
  const storedClipboard = stored.clipboard_monitor || {};

  // Provider
  const [provider, setProvider]   = useState(stored.provider || "openrouter");
  const [model, setModel]         = useState(stored.model || "google/gemma-3-27b-it");
  const [apiKey, setApiKey]       = useState(stored.api_key || "");
  const [baseUrl, setBaseUrl]     = useState(stored.base_url || "");
  const [customHeaders, setCustomHeaders] = useState(() => {
    // custom_headers may be a dict (from YAML) or a string — normalize to textarea format
    const ch = stored.custom_headers;
    if (!ch) return "";
    if (typeof ch === "object") return Object.entries(ch).map(([k, v]) => `${k}: ${v}`).join("\n");
    return ch;
  });

  // Behaviour
  const [hotkey, setHotkey]       = useState(stored.hotkey || "");
  const [overlayPos, setOverlayPos] = useState(stored.overlay_position || "near_cursor");
  const [stream, setStream]       = useState(stored.stream !== false);
  const [clipboardEnabled, setClipboardEnabled] = useState(storedClipboard.enabled ?? false);

  // Theme
  const [theme, setThemeLocal] = useState(bridge?.theme || "dark");
  const handleTheme = (t) => {
    setThemeLocal(t);
    if (bridge?.setTheme) bridge.setTheme(t);
  };

  // Persona
  const [personaEnabled, setPersonaEnabled] = useState(storedPersona.enabled ?? false);
  const [personaTone, setPersonaTone]       = useState(storedPersona.tone || "natural");
  const [personaStyle, setPersonaStyle]     = useState(storedPersona.style || "");
  const [personaAvoid, setPersonaAvoid]     = useState(storedPersona.avoid || "");

  // History & Tutor
  const [historyEnabled, setHistoryEnabled]     = useState(storedHistory.enabled ?? false);
  const [tutorEnabled, setTutorEnabled]         = useState(storedTutor.enabled ?? false);
  const [tutorAutoExplain, setTutorAutoExplain] = useState(storedTutor.auto_explain ?? false);

  const handleSave = async () => {
    const config = {
      provider, model,
      history:            { enabled: historyEnabled },
      tutor:              { enabled: tutorEnabled, auto_explain: tutorAutoExplain },
      clipboard_monitor:  { enabled: clipboardEnabled },
      overlay_position:   overlayPos,
      stream,
      persona: {
        enabled: personaEnabled,
        tone:    personaTone,
        style:   personaStyle.trim(),
        avoid:   personaAvoid.trim(),
      },
    };
    if (apiKey)  config.api_key  = apiKey;
    if (baseUrl) config.base_url = baseUrl;
    if (hotkey)  config.hotkey   = hotkey;
    // Parse custom headers from "Name: value" lines into a dict for clean YAML
    // Send empty string to clear when user removes all headers
    if (customHeaders.trim()) {
      const hdrs = {};
      customHeaders.trim().split("\n").forEach((line) => {
        const idx = line.indexOf(":");
        if (idx > 0) hdrs[line.slice(0, idx).trim()] = line.slice(idx + 1).trim();
      });
      config.custom_headers = Object.keys(hdrs).length > 0 ? hdrs : "";
    } else {
      config.custom_headers = "";
    }

    localStorage.setItem("quill_config_pending", JSON.stringify(config));
    if (bridge?.saveConfig) await bridge.saveConfig(config);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  const TABS = [
    { id: "provider",   label: "AI Provider" },
    { id: "behaviour",  label: "Behaviour" },
    { id: "persona",    label: "My Voice" + (personaEnabled ? " ✦" : "") },
    { id: "tutor",      label: "AI Tutor" + (tutorEnabled ? " ✦" : "") },
    { id: "templates",  label: "Templates" },
    { id: "about",      label: "About" },
  ];

  return (
    <div className="settings-root">
      <div className="settings-topbar" onMouseDown={startDrag}>
        <div className="settings-topbar-title" onMouseDown={startDrag}>🪶 Quill — Settings</div>
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          {saved && <span style={{ fontSize: 11, color: "var(--color-success)", fontWeight: 600 }}>✓ Saved</span>}
          <button className="settings-topbar-btn" onClick={handleSave}>Save</button>
          <button className="overlay-close-btn" onClick={onClose} style={{ width: 28, height: 28 }}>✕</button>
        </div>
      </div>

      <div className="settings-tabs">
        {TABS.map((tab) => (
          <button key={tab.id}
            className={`settings-tab ${activeTab === tab.id ? "active" : ""}`}
            onClick={() => setActiveTab(tab.id)}>
            {tab.label}
          </button>
        ))}
      </div>

      <div className="settings-body">

        {/* ── AI Provider ───────────────────────────────────────────────────── */}
        {activeTab === "provider" && (
          <div className="settings-section">
            <div className="settings-card">
              <div className="settings-row">
                <div><div className="settings-row-label">Provider</div>
                  <div className="settings-row-desc">Where AI requests are sent</div></div>
                <select className="settings-select" value={provider} onChange={(e) => setProvider(e.target.value)}>
                  {PROVIDERS.map((p) => <option key={p.id} value={p.id}>{p.label}</option>)}
                </select>
              </div>
              <div className="settings-row">
                <div><div className="settings-row-label">Model</div>
                  <div className="settings-row-desc">Model name for completions</div></div>
                <input className="settings-input" type="text" value={model}
                  onChange={(e) => setModel(e.target.value)} placeholder="e.g. gpt-4o-mini" />
              </div>
              {["openrouter","openai"].includes(provider) && (
                <div className="settings-row">
                  <div><div className="settings-row-label">API Key</div>
                    <div className="settings-row-desc">Stored locally, never synced</div></div>
                  <input className="settings-input" type="password" value={apiKey}
                    onChange={(e) => setApiKey(e.target.value)} placeholder="sk-..." />
                </div>
              )}
              {provider === "ollama" && (
                <div className="settings-row">
                  <div><div className="settings-row-label">Base URL</div>
                    <div className="settings-row-desc">Ollama server URL</div></div>
                  <input className="settings-input" type="text" value={baseUrl}
                    onChange={(e) => setBaseUrl(e.target.value)}
                    placeholder="http://localhost:11434" />
                </div>
              )}
              {provider === "generic" && (
                <div className="settings-row">
                  <div><div className="settings-row-label">Endpoint URL</div>
                    <div className="settings-row-desc">Full URL including path (e.g. /v1/chat/completions)</div></div>
                  <input className="settings-input" type="text" value={baseUrl}
                    onChange={(e) => setBaseUrl(e.target.value)}
                    placeholder="https://api.example.com/v1/chat/completions"
                    style={{ width: 280 }} />
                </div>
              )}
              {provider === "generic" && (
                <div className="settings-row" style={{ flexDirection: "column", alignItems: "stretch", gap: 8 }}>
                  <div><div className="settings-row-label">Custom Headers</div>
                    <div className="settings-row-desc">Extra auth headers — one per line as <code style={{ fontSize: 11, fontFamily: "var(--font-mono)" }}>Header-Name: value</code></div></div>
                  <textarea className="persona-textarea" value={customHeaders}
                    onChange={(e) => setCustomHeaders(e.target.value)}
                    placeholder={"X-Client-Id: my-app-id\nX-Client-Secret: my-secret\nAuthorization: Basic dXNlcjpwYXNz"}
                    style={{ minHeight: 56, fontSize: 12, fontFamily: "var(--font-mono)" }} />
                </div>
              )}
            </div>
          </div>
        )}

        {/* ── Behaviour ─────────────────────────────────────────────────────── */}
        {activeTab === "behaviour" && (
          <div className="settings-section">
            <div className="settings-card">
              <div className="settings-row">
                <div><div className="settings-row-label">Hotkey</div>
                  <div className="settings-row-desc">Leave empty for OS default (Ctrl+Shift+Space)</div></div>
                <input className="settings-input" type="text" value={hotkey}
                  onChange={(e) => setHotkey(e.target.value)} placeholder="ctrl+shift+space" />
              </div>
              <div className="settings-row">
                <div><div className="settings-row-label">Overlay position</div></div>
                <select className="settings-select" value={overlayPos}
                  onChange={(e) => setOverlayPos(e.target.value)}>
                  {POSITIONS.map((p) => <option key={p.id} value={p.id}>{p.label}</option>)}
                </select>
              </div>
              <div className="settings-row">
                <div><div className="settings-row-label">Stream responses</div>
                  <div className="settings-row-desc">Show text as it's generated</div></div>
                <Toggle checked={stream} onChange={setStream} />
              </div>
            </div>

            {/* Theme */}
            <div className="settings-section-title" style={{ marginTop: 8 }}>Appearance</div>
            <div className="settings-card">
              <div className="settings-row">
                <div><div className="settings-row-label">Theme</div>
                  <div className="settings-row-desc">Dark glass or light surface</div></div>
                <div style={{ display: "flex", gap: 6 }}>
                  {["dark","light"].map((t) => (
                    <button key={t}
                      onClick={() => handleTheme(t)}
                      style={{
                        padding: "5px 14px",
                        background: theme === t ? "var(--color-primary-dim)" : "var(--color-surface-2)",
                        border: `1px solid ${theme === t ? "var(--color-primary-glow)" : "var(--color-border)"}`,
                        borderRadius: "var(--radius-sm)",
                        color: theme === t ? "var(--color-primary)" : "var(--color-text-muted)",
                        fontSize: 12, cursor: "pointer",
                        textTransform: "capitalize",
                      }}>
                      {t === "dark" ? "🌙 Dark" : "☀️ Light"}
                    </button>
                  ))}
                </div>
              </div>
            </div>

            {/* Clipboard monitor */}
            <div className="settings-section-title" style={{ marginTop: 8 }}>Clipboard</div>
            <div className="settings-card">
              <div className="settings-row">
                <div>
                  <div className="settings-row-label">Clipboard monitor</div>
                  <div className="settings-row-desc">
                    Watch for new clipboard text and offer to transform it
                  </div>
                </div>
                <Toggle checked={clipboardEnabled} onChange={setClipboardEnabled} />
              </div>
            </div>
          </div>
        )}

        {/* ── My Voice / Persona ────────────────────────────────────────────── */}
        {activeTab === "persona" && (
          <div className="settings-section">
            <div className="settings-card">
              <div className="settings-row">
                <div><div className="settings-row-label">Enable My Voice</div>
                  <div className="settings-row-desc">
                    Apply your style to all AI outputs — rewrites, translations, coaching, everything
                  </div></div>
                <Toggle checked={personaEnabled} onChange={setPersonaEnabled} />
              </div>
            </div>

            {personaEnabled && (
              <>
                <div className="settings-section-title" style={{ marginTop: 8 }}>Tone</div>
                <div className="settings-card">
                  <div style={{ padding: "14px 16px" }}>
                    <ToneSelector value={personaTone} onChange={setPersonaTone} />
                    <div className="tone-desc">
                      {PERSONA_TONES.find((t) => t.id === personaTone)?.desc}
                    </div>
                  </div>
                </div>

                <div className="settings-section-title" style={{ marginTop: 8 }}>Writing style</div>
                <div className="settings-card">
                  <div style={{ padding: "14px 16px", display: "flex", flexDirection: "column", gap: 8 }}>
                    <div className="settings-row-desc" style={{ marginBottom: 4 }}>
                      Describe how you write. The AI will mirror your style.
                    </div>
                    <textarea className="persona-textarea" value={personaStyle}
                      onChange={(e) => setPersonaStyle(e.target.value)}
                      placeholder="e.g. I write in short punchy sentences. I use em-dashes for emphasis." rows={4} />
                    <div className="persona-char-hint">
                      {personaStyle.length > 0 ? `${personaStyle.length} characters`
                        : "Try describing a sentence structure, rhythm, or vocabulary you prefer."}
                    </div>
                  </div>
                </div>

                <div className="settings-section-title" style={{ marginTop: 8 }}>Always avoid</div>
                <div className="settings-card">
                  <div style={{ padding: "14px 16px" }}>
                    <div className="settings-row-desc" style={{ marginBottom: 8 }}>
                      Words, phrases, or patterns the AI should never use.
                    </div>
                    <input className="settings-input" type="text" style={{ width: "100%" }}
                      value={personaAvoid} onChange={(e) => setPersonaAvoid(e.target.value)}
                      placeholder="e.g. passive voice, corporate buzzwords, exclamation marks" />
                  </div>
                </div>

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

        {/* ── AI Tutor ──────────────────────────────────────────────────────── */}
        {activeTab === "tutor" && (
          <div className="settings-section">
            <div className="settings-card">
              <div className="settings-row">
                <div><div className="settings-row-label">Enable History</div>
                  <div className="settings-row-desc">
                    Save all transformations locally in ~/.quill/history.db (opt-in)
                  </div></div>
                <Toggle checked={historyEnabled} onChange={setHistoryEnabled} />
              </div>
            </div>

            {historyEnabled && (
              <div className="settings-card">
                <div className="settings-row">
                  <div><div className="settings-row-label">Enable AI Tutor</div>
                    <div className="settings-row-desc">
                      Generate lessons and explain changes using your history
                    </div></div>
                  <Toggle checked={tutorEnabled} onChange={setTutorEnabled} />
                </div>
                {tutorEnabled && (
                  <div className="settings-row">
                    <div><div className="settings-row-label">Auto-explain changes</div>
                      <div className="settings-row-desc">
                        Automatically show tutor insight after every transformation
                      </div></div>
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

        {/* ── Templates ─────────────────────────────────────────────────────── */}
        {activeTab === "templates" && (
          <div className="settings-section">
            <div style={{ fontSize: 12, color: "var(--color-text-muted)", marginBottom: 12, lineHeight: 1.6 }}>
              Quick templates combine a mode + instruction into a one-click button shown in the overlay.
              Use them for repeat tasks like "Slack update" or "Client email".
            </div>
            <TemplatesEditor
              templates={templates}
              onSave={bridge?.saveTemplate}
              onDelete={bridge?.deleteTemplate}
            />
          </div>
        )}

        {/* ── About ─────────────────────────────────────────────────────────── */}
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

      {/* Save bar removed — Save button is now in the topbar */}
    </div>
  );
}
