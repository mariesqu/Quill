use anyhow::Result;
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use super::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeConfig {
    pub label:  String,
    pub icon:   String,
    pub prompt: String,
    #[serde(default)]
    pub hotkey: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub label:       String,
    pub icon:        String,
    pub steps:       Vec<String>,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ModesFile {
    #[serde(default)]
    modes:         HashMap<String, ModeConfig>,
    #[serde(default)]
    chains:        HashMap<String, ChainConfig>,
    #[serde(default)]
    custom_modes:  HashMap<String, ModeConfig>,
    #[serde(default)]
    custom_chains: HashMap<String, ChainConfig>,
}

/// Flat representation sent to the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeInfo {
    pub id:    String,
    pub label: String,
    pub icon:  String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainInfo {
    pub id:          String,
    pub label:       String,
    pub icon:        String,
    pub steps:       Vec<String>,
    pub description: String,
}

fn modes_yaml_path() -> PathBuf {
    let repo_cfg = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap_or(&PathBuf::from("."))
        .parent().unwrap_or(&PathBuf::from("."))
        .join("config").join("modes.yaml");
    if repo_cfg.exists() {
        return repo_cfg;
    }
    home_dir().unwrap_or_default().join(".quill").join("config").join("modes.yaml")
}

pub fn load_modes(cfg: &Config) -> (HashMap<String, ModeConfig>, HashMap<String, ChainConfig>) {
    let path = modes_yaml_path();
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut file: ModesFile = serde_yaml::from_str(&text).unwrap_or_default();

    // Merge custom modes from user config
    for (id, raw) in &cfg.custom_modes {
        if let Ok(m) = serde_yaml::from_value::<ModeConfig>(raw.clone()) {
            file.modes.insert(id.clone(), m);
        }
    }
    for (id, raw) in &cfg.custom_chains {
        if let Ok(c) = serde_yaml::from_value::<ChainConfig>(raw.clone()) {
            file.chains.insert(id.clone(), c);
        }
    }

    // Apply per-mode hotkeys from config
    for (mode_id, hotkey) in &cfg.mode_hotkeys {
        if let Some(m) = file.modes.get_mut(mode_id) {
            m.hotkey = Some(hotkey.clone());
        }
    }

    (file.modes, file.chains)
}

pub fn modes_list(modes: &HashMap<String, ModeConfig>) -> Vec<ModeInfo> {
    // Preserve canonical order
    const ORDER: &[&str] = &["rewrite", "translate", "coach", "shorter", "formal", "fix_grammar", "expand"];
    let mut list: Vec<ModeInfo> = ORDER.iter()
        .filter_map(|id| modes.get(*id).map(|m| ModeInfo { id: id.to_string(), label: m.label.clone(), icon: m.icon.clone() }))
        .collect();
    // Append any custom modes not in the canonical order
    for (id, m) in modes {
        if !ORDER.contains(&id.as_str()) {
            list.push(ModeInfo { id: id.clone(), label: m.label.clone(), icon: m.icon.clone() });
        }
    }
    list
}

pub fn chains_list(chains: &HashMap<String, ChainConfig>) -> Vec<ChainInfo> {
    chains.iter().map(|(id, c)| ChainInfo {
        id:          id.clone(),
        label:       c.label.clone(),
        icon:        c.icon.clone(),
        steps:       c.steps.clone(),
        description: c.description.clone(),
    }).collect()
}
