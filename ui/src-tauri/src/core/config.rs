use anyhow::{Context, Result};
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Shipped default configuration — embedded at compile time from `config/default.yaml`.
/// Users override these via `~/.quill/config/user.yaml`.
pub const DEFAULT_YAML: &str = include_str!("../../../../config/default.yaml");

/// User config directory — `~/.quill/config/`. This is where `user.yaml` lives
/// and where mutations from the Settings UI are written.
fn user_config_dir() -> PathBuf {
    home_dir().unwrap_or_default().join(".quill").join("config")
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

fn default_tone() -> String {
    "natural".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HistoryConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,
}

fn default_max_entries() -> usize {
    10_000
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TutorConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub auto_explain: bool,
    #[serde(default = "default_lesson_language")]
    pub lesson_language: String,
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
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_overlay_position")]
    pub overlay_position: String,
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
    pub custom_modes: HashMap<String, serde_yaml::Value>,
    #[serde(default)]
    pub custom_chains: HashMap<String, serde_yaml::Value>,
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
fn default_overlay_position() -> String {
    "near_cursor".into()
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
/// mini overlay.
pub fn config_is_usable(cfg: &Config) -> bool {
    if cfg.provider.is_empty() {
        return false;
    }
    if cfg.provider == "ollama" {
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
            language: default_language(),
            overlay_position: default_overlay_position(),
            stream: default_true(),
            persona: PersonaConfig::default(),
            history: HistoryConfig::default(),
            tutor: TutorConfig::default(),
            clipboard_monitor: ClipboardMonitorConfig::default(),
            templates: Vec::new(),
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
    let user = load_yaml_file(&user_path).unwrap_or(serde_yaml::Value::Null);
    let merged = merge_yaml(defaults, user);
    let mut cfg: Config = serde_yaml::from_value(merged).unwrap_or_default();

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

    let existing = if path.exists() {
        let text = std::fs::read_to_string(&path)?;
        serde_yaml::from_str::<serde_yaml::Value>(&text).unwrap_or(serde_yaml::Value::Null)
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

    // Privacy guard: if a field is currently sourced from an environment
    // variable, DO NOT write it back to user.yaml. Without this, the Settings
    // panel round-trip (load merged config → user edits other fields → save
    // full config back) would persist the env-var secret to disk, leaking it.
    strip_env_overridden_fields(&mut updates_yaml);

    let merged = merge_yaml(existing, updates_yaml);
    let text = serde_yaml::to_string(&merged)?;
    std::fs::write(&path, text)?;

    // Restrict permissions on Unix — file may contain an API key.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)) {
            eprintln!("[config] could not restrict user.yaml permissions: {e}");
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

fn load_yaml_file(path: &PathBuf) -> Option<serde_yaml::Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_yaml::from_str(&text).ok()
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
        assert_eq!(cfg.overlay_position, "near_cursor");
        assert!(cfg.stream);
        assert!(!cfg.history.enabled);
        assert!(!cfg.tutor.enabled);
        assert!(cfg.templates.is_empty());
    }

    #[test]
    fn embedded_defaults_parse_cleanly() {
        // Sanity-check that the DEFAULT_YAML string baked in at compile time is
        // syntactically valid and deserialises into a Config without panics.
        let v: Value = serde_yaml::from_str(DEFAULT_YAML).expect("DEFAULT_YAML parses");
        let _cfg: Config = serde_yaml::from_value(v).expect("DEFAULT_YAML deserialises");
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
