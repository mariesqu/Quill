#![allow(dead_code)] // save_user_config / config_is_usable live on the lib side for tests
use anyhow::{Context, Result};
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Shipped default configuration — embedded at compile time from `config/default.yaml`.
/// Users override these via `~/.quill/config/user.yaml`.
pub const DEFAULT_YAML: &str = include_str!("../../config/default.yaml");

/// User config directory — `~/.quill/config/`. This is where `user.yaml` lives
/// and where mutations from the Settings UI are written.
fn user_config_dir() -> PathBuf {
    home_dir().unwrap_or_default().join(".quill").join("config")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_tone")]
    pub tone: String,
    #[serde(default)]
    pub style: String,
    #[serde(default)]
    pub avoid: String,
}

impl Default for PersonaConfig {
    fn default() -> Self {
        // Mirror `default_tone()` used by serde so `Config::default()`
        // and loading a YAML with no `persona:` block produce the same
        // value. Before this was `#[derive(Default)]` which gave
        // `tone = ""` and drifted from the YAML's "natural".
        Self {
            enabled: false,
            tone: default_tone(),
            style: String::new(),
            avoid: String::new(),
        }
    }
}

fn default_tone() -> String {
    "natural".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryConfig {
    // History defaults to ON for new users. Quill is explicitly a tool
    // where going back to a previous rewrite/translation is part of the
    // workflow, and there is no sensitive data being stored — just the
    // text the user deliberately ran through a prompt. Users who want
    // strict no-history can flip this to false via Settings or yaml.
    #[serde(default = "default_history_enabled")]
    pub enabled: bool,
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            enabled: default_history_enabled(),
            max_entries: default_max_entries(),
        }
    }
}

fn default_history_enabled() -> bool {
    true
}

fn default_max_entries() -> usize {
    10_000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TutorConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub auto_explain: bool,
    #[serde(default = "default_lesson_language")]
    pub lesson_language: String,
}

impl Default for TutorConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_explain: false,
            lesson_language: default_lesson_language(),
        }
    }
}

fn default_lesson_language() -> String {
    "auto".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClipboardMonitorConfig {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Template {
    pub name: String,
    pub mode: String,
    pub instruction: String,
}

/// Two pinned translation targets shown as quick-action buttons on the
/// Tier 1 overlay. User customisable — for most monolingual users these
/// stay at the defaults, but bilingual / multilingual users (the whole
/// reason Translate is a first-class mode) can swap them to the pair
/// they translate between most.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinnedTranslate {
    #[serde(default = "default_pinned_a")]
    pub a: String,
    #[serde(default = "default_pinned_b")]
    pub b: String,
}

fn default_pinned_a() -> String {
    "en".into()
}
fn default_pinned_b() -> String {
    "fr".into()
}

impl Default for PinnedTranslate {
    fn default() -> Self {
        Self {
            a: default_pinned_a(),
            b: default_pinned_b(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OverlayConfig {
    #[serde(default)]
    pub pinned_translate: PinnedTranslate,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UiConfig {
    #[serde(default)]
    pub overlay: OverlayConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub hotkey: Option<String>,
    /// Global hotkey that summons the Tier 2 command palette. Default
    /// `Ctrl+Shift+P` — a familiar shortcut for anyone coming from
    /// VS Code, Sublime, or Slack. `None` skips registration.
    #[serde(default = "default_palette_hotkey")]
    pub hotkey_palette: Option<String>,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_true")]
    pub stream: bool,
    #[serde(default)]
    pub persona: PersonaConfig,
    #[serde(default)]
    pub history: HistoryConfig,
    #[serde(default)]
    pub tutor: TutorConfig,
    #[serde(default)]
    pub clipboard_monitor: ClipboardMonitorConfig,
    #[serde(default)]
    pub templates: Vec<Template>,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub custom_modes: HashMap<String, serde_yaml::Value>,
    #[serde(default)]
    pub custom_chains: HashMap<String, serde_yaml::Value>,
}

fn default_palette_hotkey() -> Option<String> {
    Some("Ctrl+Shift+P".into())
}

fn default_provider() -> String {
    "openrouter".into()
}
fn default_model() -> String {
    "google/gemma-3-27b-it".into()
}
fn default_language() -> String {
    "auto".into()
}
fn default_true() -> bool {
    true
}

/// Is the loaded config complete enough to actually call a provider?
///
/// Mirrors the frontend's `isSetupComplete` heuristic in `App.jsx`:
///   - A provider must be selected (non-empty).
///   - Non-local providers (anything other than Ollama) need an API key.
///   - Ollama needs no key — it talks to a local HTTP endpoint.
///
/// Used by `main.rs` at startup to decide whether to force-open the
/// first-run wizard instead of leaving the user staring at an empty
/// Tier 1 overlay.
pub fn config_is_usable(cfg: &Config) -> bool {
    if cfg.provider.is_empty() {
        return false;
    }
    // Local providers (no API key needed): Ollama talks to a local daemon;
    // the Claude CLI uses the user's logged-in Anthropic session.
    if cfg.provider == "ollama"
        || cfg.provider == "claude-cli"
        || cfg.provider == "claude_cli"
        || cfg.provider == "claude"
    {
        return true;
    }
    cfg.api_key.as_ref().map(|k| !k.is_empty()).unwrap_or(false)
}

impl Default for Config {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            model: default_model(),
            api_key: None,
            base_url: None,
            hotkey: None,
            hotkey_palette: default_palette_hotkey(),
            language: default_language(),
            stream: default_true(),
            persona: PersonaConfig::default(),
            history: HistoryConfig::default(),
            tutor: TutorConfig::default(),
            clipboard_monitor: ClipboardMonitorConfig::default(),
            templates: Vec::new(),
            ui: UiConfig::default(),
            custom_modes: HashMap::new(),
            custom_chains: HashMap::new(),
        }
    }
}

/// Load and merge: embedded defaults → user.yaml → env vars.
pub fn load_config() -> Config {
    let defaults: serde_yaml::Value =
        serde_yaml::from_str(DEFAULT_YAML).unwrap_or(serde_yaml::Value::Null);
    let user_path = user_config_dir().join("user.yaml");
    // Explicit error handling on user.yaml parse: we intentionally keep the
    // original file on disk if it fails to parse. A silent overwrite (the
    // previous behaviour) would clobber a user's hand-edited config on the
    // next Settings save — catastrophic data loss for something that might
    // just be a stray tab. Instead we log loudly and load defaults; the
    // caller (save_user_config) will also back up before overwriting.
    let mut user = match std::fs::read_to_string(&user_path) {
        Ok(text) => match serde_yaml::from_str::<serde_yaml::Value>(&text) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(
                    path = %user_path.display(),
                    "user.yaml parse error, using defaults: {e}"
                );
                serde_yaml::Value::Null
            }
        },
        Err(_) => serde_yaml::Value::Null,
    };
    // Normalize hand-edited hotkey fields BEFORE merging. Without this, a user
    // who writes `hotkey_palette: null` in user.yaml to disable the palette
    // hotkey would see the default survive: `merge_yaml`'s null-is-noop arm
    // preserves the base's `"Ctrl+Shift+P"`, contradicting the README.
    // Normalization rewrites null → `""` for the palette field so the merge
    // actually propagates the clear; main.rs filters empty strings to None.
    // Idempotent, so save-path normalization still works unchanged.
    normalize_hotkey_fields(&mut user);
    let merged = merge_yaml(defaults, user);
    let mut cfg: Config = match serde_yaml::from_value(merged) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("merged config failed to deserialize into Config, using defaults: {e}");
            Config::default()
        }
    };

    // Env var overrides
    if let Ok(v) = std::env::var("QUILL_API_KEY") {
        cfg.api_key = Some(v);
    }
    if let Ok(v) = std::env::var("QUILL_PROVIDER") {
        cfg.provider = v;
    }
    if let Ok(v) = std::env::var("QUILL_MODEL") {
        cfg.model = v;
    }
    if let Ok(v) = std::env::var("QUILL_BASE_URL") {
        cfg.base_url = Some(v);
    }

    cfg
}

pub fn save_user_config(updates: serde_json::Value) -> Result<()> {
    let dir = user_config_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("user.yaml");

    // Load existing user.yaml. If it fails to parse we do NOT treat that as
    // "start from scratch" — a silent overwrite would erase a user's hand-
    // edited config. Instead, back up the broken file to `user.yaml.bak` so
    // the operator can recover, log loudly, then proceed with an empty base.
    let existing = if path.exists() {
        let text = std::fs::read_to_string(&path)?;
        match serde_yaml::from_str::<serde_yaml::Value>(&text) {
            Ok(v) => v,
            Err(e) => {
                let bak = path.with_extension("yaml.bak");
                // rename is atomic on Windows NTFS and on POSIX. If it
                // fails (e.g. because a previous bak exists on some
                // filesystems), fall back to a timestamped copy so the
                // data isn't lost before we overwrite.
                if let Err(re) = std::fs::rename(&path, &bak) {
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let ts_path = path.with_extension(format!("yaml.{ts}.bak"));
                    let _ = std::fs::copy(&path, &ts_path);
                    tracing::error!(
                        "user.yaml parse error ({e}); rename to .bak failed ({re}), \
                         copied to {} before overwrite",
                        ts_path.display()
                    );
                } else {
                    tracing::error!(
                        "user.yaml parse error ({e}); moved to {} before overwrite",
                        bak.display()
                    );
                }
                serde_yaml::Value::Null
            }
        }
    } else {
        serde_yaml::Value::Null
    };

    let mut updates_yaml: serde_yaml::Value =
        serde_json::from_value(updates).context("converting updates to yaml")?;

    // Drop any helper fields the frontend may include (e.g. `api_key_set`,
    // which is emitted by `get_config` as a masking sibling and has no
    // meaning in user.yaml).
    strip_virtual_fields(&mut updates_yaml);

    // "Keep existing" semantics for `api_key`: `get_config` masks the key
    // to an empty string, so if the Settings round-trip sends back `api_key: ""`
    // it means "user didn't change it, preserve what's on disk". We strip
    // the empty field so `merge_yaml` leaves the stored value untouched.
    strip_empty_api_key(&mut updates_yaml);

    // Empty overlay `hotkey` → strip, so the on-disk value stays intact and
    // main.rs' fallback to the compiled-in default kicks in. Rationale: the
    // overlay MUST have a working hotkey — clearing the field is interpreted
    // as "give me the default", never as "run without an overlay hotkey".
    //
    // Empty `hotkey_palette` → DO NOT strip. The palette hotkey is optional
    // and clearing it expresses "disable the palette hotkey". Stripping here
    // preserved the old on-disk value through `merge_yaml`, re-registering
    // the predecessor hotkey on next boot — the user's explicit clear was
    // silently reverted. Writing `hotkey_palette: ""` makes the clear persist;
    // `load_config` yields `Some("")`, and `main.rs` filters empty strings out
    // of `palette_hotkey_spec` so no registration happens.
    //
    // `null` is normalized to `""` for `hotkey_palette` because `merge_yaml`
    // drops null overlays (see the `(_, o) if o != Value::Null => o` arm),
    // which would otherwise let the old on-disk value survive.
    normalize_hotkey_fields(&mut updates_yaml);

    // Privacy guard: if a field is currently sourced from an environment
    // variable, DO NOT write it back to user.yaml. Without this, the Settings
    // panel round-trip (load merged config → user edits other fields → save
    // full config back) would persist the env-var secret to disk, leaking it.
    strip_env_overridden_fields(&mut updates_yaml);

    let merged = merge_yaml(existing, updates_yaml);
    let text = serde_yaml::to_string(&merged)?;

    // Atomic write: write to a .tmp sibling, fsync, then rename. Rename is
    // atomic on Windows NTFS and POSIX. A crash mid-write therefore leaves
    // either the old good file or the new good file — never a truncated
    // user.yaml that the next load would reject.
    let tmp_path = path.with_extension("yaml.tmp");
    {
        use std::io::Write;
        let mut tmp = std::fs::File::create(&tmp_path)
            .with_context(|| format!("create {}", tmp_path.display()))?;
        tmp.write_all(text.as_bytes())?;
        // Durable flush — the rename below is atomic but the ON-DISK state
        // of the tmp file is only guaranteed after sync_all.
        tmp.sync_all()?;
    }
    std::fs::rename(&tmp_path, &path)
        .with_context(|| format!("rename {} -> {}", tmp_path.display(), path.display()))?;

    // Restrict permissions on Unix — file may contain an API key.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)) {
            tracing::warn!("could not restrict user.yaml permissions: {e}");
        }
    }

    Ok(())
}

/// Drop synthetic fields that `get_config` adds for the frontend but which
/// have no place in `user.yaml` (currently just `api_key_set`).
fn strip_virtual_fields(value: &mut serde_yaml::Value) {
    use serde_yaml::Value;
    if let Value::Mapping(map) = value {
        map.remove(Value::String("api_key_set".into()));
    }
}

/// Treat `api_key: ""` as "user didn't change it" — remove the empty field
/// so `merge_yaml` preserves whatever is currently on disk.
fn strip_empty_api_key(value: &mut serde_yaml::Value) {
    use serde_yaml::Value;
    if let Value::Mapping(map) = value {
        let key = Value::String("api_key".into());
        let should_strip = matches!(map.get(&key), Some(Value::String(s)) if s.is_empty())
            || matches!(map.get(&key), Some(Value::Null));
        if should_strip {
            map.remove(&key);
        }
    }
}

/// Asymmetric normalization for the two hotkey fields:
///
/// * `hotkey` (overlay) — strip empty/null. The overlay requires a working
///   binding; main.rs falls back to the compiled-in default when the on-disk
///   value is absent. Leaving `""` on disk would reach `HotkeyService::register`
///   which rejects unparseable specs, silently producing no overlay hotkey.
///
/// * `hotkey_palette` — PRESERVE empty as `""`, and normalize null → `""`.
///   The palette hotkey is optional; clearing it in Settings means "disable
///   the palette hotkey entirely". If we stripped the field, `merge_yaml`
///   would keep the previous on-disk value and the next boot would re-register
///   the old hotkey — clearing in Settings had no persistent effect. The
///   null→`""` conversion is required because `merge_yaml` drops null overlays
///   (see its `o != Value::Null` arm), which would ALSO let the stale base
///   survive. `load_config` reads `""` as `Some("")`, and main.rs filters
///   empty strings out of the palette registration path.
fn normalize_hotkey_fields(value: &mut serde_yaml::Value) {
    use serde_yaml::Value;
    if let Value::Mapping(map) = value {
        let hotkey = Value::String("hotkey".into());
        let strip_hotkey = matches!(map.get(&hotkey), Some(Value::String(s)) if s.trim().is_empty())
            || matches!(map.get(&hotkey), Some(Value::Null));
        if strip_hotkey {
            map.remove(&hotkey);
        }

        let palette = Value::String("hotkey_palette".into());
        let empty_or_null = matches!(map.get(&palette), Some(Value::String(s)) if s.trim().is_empty())
            || matches!(map.get(&palette), Some(Value::Null));
        if empty_or_null {
            map.insert(palette, Value::String(String::new()));
        }
    }
}

/// Remove fields from the update payload that are currently being supplied by
/// an environment variable. Prevents the Settings UI from persisting a
/// transient env-var secret into the on-disk user.yaml.
fn strip_env_overridden_fields(value: &mut serde_yaml::Value) {
    use serde_yaml::Value;
    if let Value::Mapping(map) = value {
        const ENV_MAPPED: &[(&str, &str)] = &[
            ("api_key", "QUILL_API_KEY"),
            ("provider", "QUILL_PROVIDER"),
            ("model", "QUILL_MODEL"),
            ("base_url", "QUILL_BASE_URL"),
        ];
        for (field, env_name) in ENV_MAPPED {
            if std::env::var(env_name).is_ok() {
                map.remove(Value::String((*field).into()));
            }
        }
    }
}

fn merge_yaml(base: serde_yaml::Value, overlay: serde_yaml::Value) -> serde_yaml::Value {
    use serde_yaml::Value;
    match (base, overlay) {
        (Value::Mapping(mut b), Value::Mapping(o)) => {
            for (k, v) in o {
                let entry = b.entry(k).or_insert(Value::Null);
                *entry = merge_yaml(entry.clone(), v);
            }
            Value::Mapping(b)
        }
        (_, o) if o != Value::Null => o,
        (b, _) => b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml::Value;

    fn yaml(s: &str) -> Value {
        serde_yaml::from_str(s).expect("valid yaml fixture")
    }

    #[test]
    fn merge_overlays_scalars() {
        let base = yaml("provider: openrouter\nmodel: gemma");
        let overlay = yaml("model: gpt-4o");
        let merged = merge_yaml(base, overlay);
        assert_eq!(merged["provider"], Value::String("openrouter".into()));
        assert_eq!(merged["model"], Value::String("gpt-4o".into()));
    }

    #[test]
    fn merge_is_deep_for_nested_maps() {
        let base = yaml("persona:\n  enabled: false\n  tone: natural\n  style: ''\n");
        let overlay = yaml("persona:\n  enabled: true\n  tone: witty\n");
        let merged = merge_yaml(base, overlay);
        // Overlaid fields take precedence
        assert_eq!(merged["persona"]["enabled"], Value::Bool(true));
        assert_eq!(merged["persona"]["tone"], Value::String("witty".into()));
        // Base fields not in overlay survive
        assert_eq!(merged["persona"]["style"], Value::String("".into()));
    }

    #[test]
    fn overlay_null_does_not_clobber_base() {
        let base = yaml("provider: openrouter");
        let overlay = yaml("provider: null");
        let merged = merge_yaml(base, overlay);
        assert_eq!(merged["provider"], Value::String("openrouter".into()));
    }

    #[test]
    fn default_config_has_sane_fields() {
        let cfg = Config::default();
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.language, "auto");
        assert!(cfg.stream);
        // History now defaults to ON — see `default_history_enabled`
        // above for rationale (going back to a prior rewrite is a
        // first-class workflow, not an opt-in).
        assert!(cfg.history.enabled);
        assert!(!cfg.tutor.enabled);
        assert!(cfg.templates.is_empty());
        // Palette hotkey registered by default so Ctrl+Shift+P works
        // out of the box — see `default_palette_hotkey`.
        assert_eq!(cfg.hotkey_palette.as_deref(), Some("Ctrl+Shift+P"));
        // Pinned translate pair defaults for the overlay quick actions.
        assert_eq!(cfg.ui.overlay.pinned_translate.a, "en");
        assert_eq!(cfg.ui.overlay.pinned_translate.b, "fr");
    }

    #[test]
    fn embedded_defaults_parse_cleanly() {
        // Sanity-check that the DEFAULT_YAML string baked in at compile time is
        // syntactically valid and deserialises into a Config without panics.
        let v: Value = serde_yaml::from_str(DEFAULT_YAML).expect("DEFAULT_YAML parses");
        let _cfg: Config = serde_yaml::from_value(v).expect("DEFAULT_YAML deserialises");
    }

    #[test]
    fn default_yaml_matches_rust_defaults() {
        // Guardrail: DEFAULT_YAML and Config::default() must agree on every
        // field. Drift between them has already caused user-visible bugs
        // (e.g. history.enabled appearing differently from each source).
        // Any future field added to Config that isn't mirrored into the
        // YAML (or vice-versa) trips this test.
        let v: Value = serde_yaml::from_str(DEFAULT_YAML).expect("DEFAULT_YAML parses");
        let from_yaml: Config =
            serde_yaml::from_value(v).expect("DEFAULT_YAML deserialises into Config");
        let from_rust = Config::default();

        assert_eq!(from_yaml.provider, from_rust.provider);
        assert_eq!(from_yaml.model, from_rust.model);
        assert_eq!(from_yaml.api_key, from_rust.api_key);
        // base_url differs intentionally — the Rust default is None (let
        // each provider fall through to its built-in URL), but the YAML
        // ships with openrouter's URL explicitly so users can edit it.
        // Assert the YAML field is *some* string to catch accidental nulls.
        assert!(from_yaml.base_url.is_some());
        assert_eq!(from_yaml.hotkey, from_rust.hotkey);
        assert_eq!(from_yaml.hotkey_palette, from_rust.hotkey_palette);
        assert_eq!(from_yaml.language, from_rust.language);
        assert_eq!(from_yaml.stream, from_rust.stream);
        assert_eq!(from_yaml.persona.enabled, from_rust.persona.enabled);
        assert_eq!(from_yaml.persona.tone, from_rust.persona.tone);
        assert_eq!(from_yaml.history.enabled, from_rust.history.enabled);
        assert_eq!(from_yaml.history.max_entries, from_rust.history.max_entries);
        assert_eq!(from_yaml.tutor.enabled, from_rust.tutor.enabled);
        assert_eq!(from_yaml.tutor.auto_explain, from_rust.tutor.auto_explain);
        assert_eq!(
            from_yaml.tutor.lesson_language,
            from_rust.tutor.lesson_language
        );
        assert_eq!(
            from_yaml.clipboard_monitor.enabled,
            from_rust.clipboard_monitor.enabled
        );
        assert_eq!(from_yaml.templates.len(), from_rust.templates.len());
        assert_eq!(
            from_yaml.ui.overlay.pinned_translate.a,
            from_rust.ui.overlay.pinned_translate.a
        );
        assert_eq!(
            from_yaml.ui.overlay.pinned_translate.b,
            from_rust.ui.overlay.pinned_translate.b
        );
    }

    #[test]
    fn strip_virtual_fields_removes_api_key_set() {
        let mut v = yaml("api_key: foo\napi_key_set: true\nprovider: openai");
        strip_virtual_fields(&mut v);
        let map = v.as_mapping().unwrap();
        assert!(!map.contains_key(Value::String("api_key_set".into())));
        assert!(map.contains_key(Value::String("api_key".into())));
        assert!(map.contains_key(Value::String("provider".into())));
    }

    #[test]
    fn strip_virtual_fields_is_noop_when_absent() {
        let mut v = yaml("api_key: foo\nprovider: openai");
        let before = v.clone();
        strip_virtual_fields(&mut v);
        assert_eq!(v, before);
    }

    #[test]
    fn strip_empty_api_key_drops_empty_string() {
        let mut v = yaml("api_key: ''\nprovider: openai");
        strip_empty_api_key(&mut v);
        let map = v.as_mapping().unwrap();
        assert!(!map.contains_key(Value::String("api_key".into())));
        assert!(map.contains_key(Value::String("provider".into())));
    }

    #[test]
    fn strip_empty_api_key_drops_null() {
        let mut v = yaml("api_key: null\nprovider: openai");
        strip_empty_api_key(&mut v);
        let map = v.as_mapping().unwrap();
        assert!(!map.contains_key(Value::String("api_key".into())));
    }

    #[test]
    fn strip_empty_api_key_preserves_real_value() {
        let mut v = yaml("api_key: sk-real-value\nprovider: openai");
        strip_empty_api_key(&mut v);
        let map = v.as_mapping().unwrap();
        assert_eq!(
            map.get(Value::String("api_key".into())),
            Some(&Value::String("sk-real-value".into()))
        );
    }

    #[test]
    fn strip_empty_api_key_noop_when_field_missing() {
        let mut v = yaml("provider: openai");
        let before = v.clone();
        strip_empty_api_key(&mut v);
        assert_eq!(v, before);
    }

    #[test]
    fn normalize_hotkey_strips_empty_overlay_but_preserves_empty_palette() {
        let mut v = yaml("hotkey: ''\nhotkey_palette: ''\nprovider: openai");
        normalize_hotkey_fields(&mut v);
        let map = v.as_mapping().unwrap();
        // Overlay hotkey stripped → main.rs falls back to default.
        assert!(!map.contains_key(Value::String("hotkey".into())));
        // Palette empty-string persists → merge_yaml overwrites the on-disk
        // value, expressing the user's explicit clear.
        assert_eq!(
            map.get(Value::String("hotkey_palette".into())),
            Some(&Value::String(String::new()))
        );
        assert!(map.contains_key(Value::String("provider".into())));
    }

    #[test]
    fn normalize_hotkey_strips_null_overlay_and_rewrites_null_palette_to_empty() {
        let mut v = yaml("hotkey: null\nhotkey_palette: null");
        normalize_hotkey_fields(&mut v);
        let map = v.as_mapping().unwrap();
        assert!(!map.contains_key(Value::String("hotkey".into())));
        // null alone would be dropped by merge_yaml — normalize to "" so the
        // user's clear actually reaches disk.
        assert_eq!(
            map.get(Value::String("hotkey_palette".into())),
            Some(&Value::String(String::new()))
        );
    }

    #[test]
    fn normalize_hotkey_preserves_real_values() {
        let mut v = yaml("hotkey: 'Ctrl+Shift+Space'\nhotkey_palette: 'Ctrl+Shift+P'");
        normalize_hotkey_fields(&mut v);
        let map = v.as_mapping().unwrap();
        assert_eq!(
            map.get(Value::String("hotkey".into())),
            Some(&Value::String("Ctrl+Shift+Space".into()))
        );
        assert_eq!(
            map.get(Value::String("hotkey_palette".into())),
            Some(&Value::String("Ctrl+Shift+P".into()))
        );
    }

    #[test]
    fn normalize_hotkey_strips_whitespace_only_overlay() {
        let mut v = yaml("hotkey: '   '\nprovider: openai");
        normalize_hotkey_fields(&mut v);
        let map = v.as_mapping().unwrap();
        assert!(!map.contains_key(Value::String("hotkey".into())));
    }

    #[test]
    fn normalize_hotkey_rewrites_whitespace_only_palette_to_empty() {
        let mut v = yaml("hotkey_palette: '   '\n");
        normalize_hotkey_fields(&mut v);
        let map = v.as_mapping().unwrap();
        assert_eq!(
            map.get(Value::String("hotkey_palette".into())),
            Some(&Value::String(String::new()))
        );
    }

    #[test]
    fn normalize_hotkey_clear_palette_overrides_stored_value() {
        // End-to-end: user clears palette in Settings (empty string arrives)
        // and merge_yaml must overwrite the stored "Ctrl+Shift+P" so load_config
        // reads back Some("") which main.rs filters to no registration.
        let existing = yaml("hotkey_palette: 'Ctrl+Shift+P'\nprovider: openai");
        let mut updates = yaml("hotkey_palette: ''");
        normalize_hotkey_fields(&mut updates);
        let merged = merge_yaml(existing, updates);
        assert_eq!(
            merged["hotkey_palette"],
            Value::String(String::new()),
            "user's clear must survive the merge"
        );
    }

    // ── config_is_usable ────────────────────────────────────────────────────

    fn cfg_with(provider: &str, api_key: Option<&str>) -> Config {
        Config {
            provider: provider.into(),
            api_key: api_key.map(String::from),
            ..Config::default()
        }
    }

    #[test]
    fn usable_requires_non_empty_provider() {
        assert!(!config_is_usable(&cfg_with("", Some("sk-xyz"))));
    }

    #[test]
    fn ollama_is_usable_without_api_key() {
        assert!(config_is_usable(&cfg_with("ollama", None)));
        assert!(config_is_usable(&cfg_with("ollama", Some(""))));
    }

    #[test]
    fn claude_cli_is_usable_without_api_key() {
        // Accept every alias — users may spell it with hyphen, underscore,
        // or the bare "claude" short form.
        assert!(config_is_usable(&cfg_with("claude-cli", None)));
        assert!(config_is_usable(&cfg_with("claude_cli", None)));
        assert!(config_is_usable(&cfg_with("claude", None)));
        assert!(config_is_usable(&cfg_with("claude-cli", Some(""))));
    }

    #[test]
    fn openrouter_needs_api_key() {
        assert!(!config_is_usable(&cfg_with("openrouter", None)));
        assert!(!config_is_usable(&cfg_with("openrouter", Some(""))));
        assert!(config_is_usable(&cfg_with("openrouter", Some("sk-or-xyz"))));
    }

    #[test]
    fn openai_needs_api_key() {
        assert!(!config_is_usable(&cfg_with("openai", None)));
        assert!(config_is_usable(&cfg_with("openai", Some("sk-xyz"))));
    }

    #[test]
    fn generic_provider_needs_api_key() {
        assert!(!config_is_usable(&cfg_with("generic", None)));
        assert!(config_is_usable(&cfg_with("generic", Some("token"))));
    }
}
