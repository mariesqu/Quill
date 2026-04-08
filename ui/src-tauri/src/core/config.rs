use anyhow::{Context, Result};
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

fn config_dir() -> PathBuf {
    // Try repo-relative config first (dev mode), then ~/.quill
    let repo_cfg = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(&PathBuf::from("."))
        .parent()
        .unwrap_or(&PathBuf::from("."))
        .join("config");
    if repo_cfg.exists() {
        return repo_cfg;
    }
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

fn default_tone() -> String { "natural".into() }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HistoryConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,
}

fn default_max_entries() -> usize { 10_000 }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TutorConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub auto_explain: bool,
    #[serde(default = "default_lesson_language")]
    pub lesson_language: String,
}

fn default_lesson_language() -> String { "auto".into() }

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
    pub mode_hotkeys: HashMap<String, String>,
    #[serde(default)]
    pub custom_modes: HashMap<String, serde_yaml::Value>,
    #[serde(default)]
    pub custom_chains: HashMap<String, serde_yaml::Value>,
}

fn default_provider()         -> String { "openrouter".into() }
fn default_model()            -> String { "google/gemma-3-27b-it".into() }
fn default_language()         -> String { "auto".into() }
fn default_overlay_position() -> String { "near_cursor".into() }
fn default_true()             -> bool   { true }

impl Default for Config {
    fn default() -> Self {
        serde_yaml::from_str("{}").unwrap_or_else(|_| Self {
            provider:          default_provider(),
            model:             default_model(),
            api_key:           None,
            base_url:          None,
            hotkey:            None,
            language:          default_language(),
            overlay_position:  default_overlay_position(),
            stream:            true,
            persona:           Default::default(),
            history:           Default::default(),
            tutor:             Default::default(),
            clipboard_monitor: Default::default(),
            templates:         vec![],
            mode_hotkeys:      Default::default(),
            custom_modes:      Default::default(),
            custom_chains:     Default::default(),
        })
    }
}

/// Load and merge: default.yaml → user.yaml → env vars.
pub fn load_config() -> Config {
    let dir = config_dir();
    let defaults = load_yaml_file(&dir.join("default.yaml")).unwrap_or_default();
    let user = load_yaml_file(&dir.join("user.yaml")).unwrap_or_default();
    let merged = merge_yaml(defaults, user);
    let mut cfg: Config = serde_yaml::from_value(merged).unwrap_or_default();

    // Env var overrides
    if let Ok(v) = std::env::var("QUILL_API_KEY")  { cfg.api_key  = Some(v); }
    if let Ok(v) = std::env::var("QUILL_PROVIDER")  { cfg.provider = v; }
    if let Ok(v) = std::env::var("QUILL_MODEL")     { cfg.model    = v; }
    if let Ok(v) = std::env::var("QUILL_BASE_URL")  { cfg.base_url = Some(v); }

    cfg
}

pub fn save_user_config(updates: serde_json::Value) -> Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("user.yaml");

    let existing = if path.exists() {
        let text = std::fs::read_to_string(&path)?;
        serde_yaml::from_str::<serde_yaml::Value>(&text).unwrap_or(serde_yaml::Value::Null)
    } else {
        serde_yaml::Value::Null
    };

    let updates_yaml: serde_yaml::Value = serde_json::from_value(updates)
        .context("converting updates to yaml")?;
    let merged = merge_yaml(existing, updates_yaml);
    let text = serde_yaml::to_string(&merged)?;
    std::fs::write(path, text)?;
    Ok(())
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
