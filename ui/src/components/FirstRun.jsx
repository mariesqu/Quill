import React, { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { invalidateConfigCache } from "../common/configCache";
import "../styles/firstrun.css";

const PROVIDERS = [
  {
    id: "openrouter",
    name: "Free cloud (OpenRouter)",
    desc: "Quick setup. Needs a free OpenRouter account. Best quality on free tier.",
    badge: "Free",
    placeholder: "sk-or-xxxxxxxxxxxxxxxxxx",
    link: "https://openrouter.ai/keys",
  },
  {
    id: "ollama",
    name: "Local model (Ollama)",
    desc: "Fully private. No data leaves your machine. Requires Ollama + a model installed.",
    badge: null,
    placeholder: null,
    link: "https://ollama.com",
  },
  {
    id: "openai",
    name: "OpenAI",
    desc: "Use your own OpenAI API key with gpt-4o-mini or any model.",
    badge: null,
    placeholder: "sk-xxxxxxxxxxxxxxxxxxxxxxxx",
    link: "https://platform.openai.com/api-keys",
  },
  {
    id: "generic",
    name: "Custom endpoint",
    desc: "Any OpenAI-compatible URL (LM Studio, Groq, Jan.ai, Anthropic proxy…).",
    badge: null,
    placeholder: "http://localhost:1234/v1",
    link: null,
  },
];

const MODEL_DEFAULTS = {
  openrouter: "google/gemma-3-27b-it",
  ollama: "gemma3:4b",
  openai: "gpt-4o-mini",
  generic: "your-model-name",
};

export default function FirstRun({ onComplete }) {
  const [step, setStep] = useState(0); // 0 = provider, 1 = model/key, 2 = done
  const [selectedProvider, setSelectedProvider] = useState("openrouter");
  const [apiKey, setApiKey] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [model, setModel] = useState(MODEL_DEFAULTS["openrouter"]);
  const [saveError, setSaveError] = useState(null);
  const [saving, setSaving] = useState(false);

  const provider = PROVIDERS.find((p) => p.id === selectedProvider);
  const needsKey = ["openrouter", "openai"].includes(selectedProvider);
  const needsUrl = selectedProvider === "generic";

  const handleProviderChange = (id) => {
    setSelectedProvider(id);
    setModel(MODEL_DEFAULTS[id]);
    setApiKey("");
    setBaseUrl("");
  };

  const handleContinue = async () => {
    if (step === 0) {
      setStep(1);
    } else if (step === 1) {
      setSaveError(null);
      setSaving(true);
      try {
        const configUpdate = { provider: selectedProvider, model };
        if (apiKey) configUpdate.api_key = apiKey;
        if (baseUrl) configUpdate.base_url = baseUrl;
        // Persist to ~/.quill/config/user.yaml via the Rust backend.
        await invoke("save_config", { configUpdate });
        // Invalidate the shared config cache — App.jsx's mount effect already
        // populated it with the empty pre-wizard config, and any subsequent
        // consumer (useQuillBridge, SettingsPanel) would otherwise see the
        // stale snapshot instead of the values we just persisted.
        invalidateConfigCache();
        setStep(2);
      } catch (err) {
        setSaveError(String(err));
      } finally {
        setSaving(false);
      }
    } else {
      onComplete();
    }
  };

  const canContinue = step === 0
    ? true
    : step === 1
    ? (!needsKey || apiKey.length > 4) && (!needsUrl || baseUrl.length > 4) && model.length > 0
    : true;

  return (
    <div className="firstrun-root">
      <div className="firstrun-card">
        {/* Hero */}
        <div className="firstrun-hero">
          <div className="firstrun-logo">🪶</div>
          <h1 className="firstrun-title">Welcome to Quill</h1>
          <p className="firstrun-subtitle">
            {step === 0
              ? "Privacy-first AI writing assistant. Let's connect it to an AI model."
              : step === 1
              ? "Almost done — configure your connection."
              : "You're all set! Press your hotkey anytime to get started."}
          </p>
        </div>

        {/* Step indicator */}
        <div className="firstrun-body">
          <div className="step-indicator">
            {[0, 1, 2].map((s) => (
              <div
                key={s}
                className={`step-dot ${s === step ? "active" : s < step ? "done" : ""}`}
              />
            ))}
          </div>

          {/* Step 0: Provider selection */}
          {step === 0 && (
            <>
              <div className="firstrun-section-label">How would you like to connect?</div>
              <div className="provider-options">
                {PROVIDERS.map((p) => (
                  <button
                    key={p.id}
                    className={`provider-option ${selectedProvider === p.id ? "selected" : ""}`}
                    onClick={() => handleProviderChange(p.id)}
                  >
                    <div className="provider-option-header">
                      <div className="provider-option-radio">
                        <div className="provider-option-radio-dot" />
                      </div>
                      <span className="provider-option-name">{p.name}</span>
                      {p.badge && (
                        <span className="provider-option-badge">{p.badge}</span>
                      )}
                    </div>
                    <div className="provider-option-desc">{p.desc}</div>
                  </button>
                ))}
              </div>
            </>
          )}

          {/* Step 1: Key/URL/model */}
          {step === 1 && (
            <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
              {needsKey && (
                <div className="api-key-field">
                  <div className="api-key-label">
                    API Key
                    {provider?.link && (
                      <span style={{ color: "var(--color-primary)", marginLeft: 6, fontSize: 11 }}>
                        Get one at {provider.link}
                      </span>
                    )}
                  </div>
                  <input
                    className="api-key-input"
                    type="password"
                    placeholder={provider?.placeholder || "sk-..."}
                    value={apiKey}
                    onChange={(e) => setApiKey(e.target.value)}
                    autoFocus
                  />
                </div>
              )}

              {needsUrl && (
                <div className="api-key-field">
                  <div className="api-key-label">Base URL</div>
                  <input
                    className="api-key-input"
                    type="text"
                    placeholder={provider?.placeholder || "http://localhost:1234/v1"}
                    value={baseUrl}
                    onChange={(e) => setBaseUrl(e.target.value)}
                    autoFocus
                  />
                </div>
              )}

              <div className="api-key-field">
                <div className="api-key-label">Model</div>
                <input
                  className="api-key-input"
                  type="text"
                  placeholder={MODEL_DEFAULTS[selectedProvider]}
                  value={model}
                  onChange={(e) => setModel(e.target.value)}
                />
              </div>

              {selectedProvider === "ollama" && (
                <div
                  style={{
                    padding: "10px 12px",
                    background: "rgba(56, 189, 248, 0.08)",
                    border: "1px solid rgba(56, 189, 248, 0.2)",
                    borderRadius: "var(--radius-md)",
                    fontSize: 12,
                    color: "rgba(56, 189, 248, 0.9)",
                    lineHeight: 1.5,
                  }}
                >
                  💡 Make sure Ollama is running and the model is installed:
                  <br />
                  <code style={{ fontFamily: "var(--font-mono)", opacity: 0.8 }}>
                    ollama pull {model}
                  </code>
                </div>
              )}
            </div>
          )}

          {/* Step 2: Done */}
          {step === 2 && (
            <div style={{ textAlign: "center", padding: "12px 0" }}>
              <div style={{ fontSize: 40, marginBottom: 12 }}>🎉</div>
              <p style={{ color: "var(--color-text-muted)", fontSize: 13, lineHeight: 1.6 }}>
                Quill is ready. Select text anywhere and press your hotkey to start writing smarter.
              </p>
              <div
                style={{
                  marginTop: 16,
                  padding: "10px 16px",
                  background: "var(--color-primary-dim)",
                  border: "1px solid rgba(124, 110, 247, 0.3)",
                  borderRadius: "var(--radius-md)",
                  fontSize: 13,
                  color: "var(--color-primary)",
                  display: "inline-flex",
                  gap: 8,
                  alignItems: "center",
                }}
              >
                <kbd style={{ fontFamily: "var(--font-mono)", fontSize: 12 }}>
                  Ctrl+Shift+Space
                </kbd>
                <span style={{ color: "var(--color-text-dim)" }}>or</span>
                <kbd style={{ fontFamily: "var(--font-mono)", fontSize: 12 }}>
                  Cmd+Shift+Space
                </kbd>
                on macOS
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="firstrun-footer">
          <div className="firstrun-privacy">
            🔒 Your API key is stored locally only
          </div>
          {saveError && (
            <div style={{
              color: "var(--color-error, #f87171)",
              fontSize: 12,
              maxWidth: 320,
              textAlign: "right",
            }}>
              Failed to save: {saveError}
            </div>
          )}
          <button
            className="btn-continue"
            onClick={handleContinue}
            disabled={!canContinue || saving}
          >
            {saving ? "Saving…" : step === 2 ? "Start Writing →" : "Continue →"}
          </button>
        </div>
      </div>
    </div>
  );
}
