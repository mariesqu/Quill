use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::config::Config;

/// Built-in mode and chain definitions — embedded at compile time from `config/modes.yaml`.
/// User custom modes/chains from `user.yaml` are merged on top.
const MODES_YAML: &str = include_str!("../../../../config/modes.yaml");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeConfig {
    pub label: String,
    pub icon: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub label: String,
    pub icon: String,
    pub steps: Vec<String>,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ModesFile {
    #[serde(default)]
    modes: HashMap<String, ModeConfig>,
    #[serde(default)]
    chains: HashMap<String, ChainConfig>,
    #[serde(default)]
    custom_modes: HashMap<String, ModeConfig>,
    #[serde(default)]
    custom_chains: HashMap<String, ChainConfig>,
}

/// Flat representation sent to the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeInfo {
    pub id: String,
    pub label: String,
    pub icon: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainInfo {
    pub id: String,
    pub label: String,
    pub icon: String,
    pub steps: Vec<String>,
    pub description: String,
}

pub fn load_modes(cfg: &Config) -> (HashMap<String, ModeConfig>, HashMap<String, ChainConfig>) {
    let mut file: ModesFile = serde_yaml::from_str(MODES_YAML).unwrap_or_default();

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

    (file.modes, file.chains)
}

pub fn modes_list(modes: &HashMap<String, ModeConfig>) -> Vec<ModeInfo> {
    // Preserve canonical order
    const ORDER: &[&str] = &[
        "rewrite",
        "translate",
        "coach",
        "shorter",
        "formal",
        "fix_grammar",
        "expand",
    ];
    let mut list: Vec<ModeInfo> = ORDER
        .iter()
        .filter_map(|id| {
            modes.get(*id).map(|m| ModeInfo {
                id: id.to_string(),
                label: m.label.clone(),
                icon: m.icon.clone(),
            })
        })
        .collect();
    // Append any custom modes not in the canonical order, sorted by id for
    // a deterministic UI across process restarts (HashMap iteration order is
    // randomised by default).
    let mut custom: Vec<(&String, &ModeConfig)> = modes
        .iter()
        .filter(|(id, _)| !ORDER.contains(&id.as_str()))
        .collect();
    custom.sort_by(|(a, _), (b, _)| a.cmp(b));
    for (id, m) in custom {
        list.push(ModeInfo {
            id: id.clone(),
            label: m.label.clone(),
            icon: m.icon.clone(),
        });
    }
    list
}

pub fn chains_list(chains: &HashMap<String, ChainConfig>) -> Vec<ChainInfo> {
    // Sort by id so the chain row has a stable order across launches.
    // Iterating a HashMap directly gives non-deterministic order, which made
    // custom chains shuffle between process restarts.
    let mut sorted: Vec<(&String, &ChainConfig)> = chains.iter().collect();
    sorted.sort_by(|(a, _), (b, _)| a.cmp(b));
    sorted
        .into_iter()
        .map(|(id, c)| ChainInfo {
            id: id.clone(),
            label: c.label.clone(),
            icon: c.icon.clone(),
            steps: c.steps.clone(),
            description: c.description.clone(),
        })
        .collect()
}
