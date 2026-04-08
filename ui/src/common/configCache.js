/**
 * Process-wide memoised `get_config` call.
 *
 * Both `App.jsx` (to decide whether to render the first-run wizard) and
 * `useQuillBridge.js` (to seed `templates` on mount) need the current config
 * shortly after startup. Without this shim they'd each hit the Rust backend
 * independently on every window open. `loadConfigOnce` returns the SAME
 * in-flight promise until a `saveConfig` invalidates the cache.
 */
import { invoke } from '@tauri-apps/api/core';

let cachedPromise = null;

export function loadConfigOnce() {
  if (!cachedPromise) {
    cachedPromise = invoke('get_config').catch((err) => {
      // Don't cache failures — next caller should retry.
      cachedPromise = null;
      throw err;
    });
  }
  return cachedPromise;
}

/** Clear the cache — call after `save_config` so fresh reads see new values. */
export function invalidateConfigCache() {
  cachedPromise = null;
}
