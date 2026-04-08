import { useState, useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import MiniOverlay from './windows/MiniOverlay';
import FullPanel   from './windows/FullPanel';
import FirstRun    from './components/FirstRun';
import { loadConfigOnce } from './common/configCache';
import './styles/globals.css';

// Heuristic: setup is complete when the backend has the minimum config to
// actually call a provider — i.e. at least a provider AND either a local
// provider (Ollama) OR a persisted API key (signalled by the `api_key_set`
// boolean that the Rust `get_config` command adds).
//
// IMPORTANT: `get_config` masks the actual `api_key` value to an empty
// string for security (we never want the plaintext key in JS-land). We
// gate exclusively on `api_key_set` instead.
function isSetupComplete(cfg) {
  if (!cfg || typeof cfg !== 'object') return false;
  const provider = cfg.provider;
  if (!provider) return false;
  if (provider === 'ollama') return true;
  return cfg.api_key_set === true;
}

export default function App() {
  const [windowLabel] = useState(() => {
    try { return getCurrentWindow().label; } catch { return 'mini'; }
  });
  // `null` = loading (we haven't asked the backend yet)
  // `true` = setup complete
  // `false` = show the first-run wizard (full window only)
  const [setupDone, setSetupDone] = useState(null);

  // Apply stored theme on mount
  useEffect(() => {
    const theme = localStorage.getItem('quill_theme') || 'dark';
    document.documentElement.setAttribute('data-theme', theme);
  }, []);

  // Resolve setup state from the Rust backend, NOT from localStorage. This
  // way the wizard gating survives localStorage resets (cleared browser data,
  // WebView2 profile wipes, fresh install over existing `~/.quill`), and we
  // avoid the dual-source-of-truth trap where `main.rs` and the frontend can
  // disagree. `loadConfigOnce` memoises the Rust round-trip so this doesn't
  // double up with the `useQuillBridge` call inside the child tree.
  useEffect(() => {
    loadConfigOnce()
      .then((cfg) => setSetupDone(isSetupComplete(cfg)))
      .catch(() => setSetupDone(false));
  }, []);

  // Still loading the config — render nothing for a single frame to avoid
  // flashing the wizard. Both windows stay blank during this moment.
  if (setupDone === null) return null;

  // Show the first-run wizard ONLY in the full window. The mini window stays
  // blank until setup is complete so we don't render two wizards side by side
  // (and so the mini window doesn't race with the full window on save).
  if (!setupDone) {
    if (windowLabel !== 'full') return null;
    return (
      <FirstRun onComplete={() => setSetupDone(true)} />
    );
  }

  if (windowLabel === 'full') return <FullPanel />;
  return <MiniOverlay />;
}
