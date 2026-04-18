use async_trait::async_trait;
use serde_json::json;

use super::{post_openai_chat, ChunkStream, Provider};
use crate::core::config::Config;

pub struct OpenRouterProvider {
    model: String,
    api_key: String,
    base_url: String,
}

impl OpenRouterProvider {
    pub fn new(cfg: &Config) -> Self {
        Self {
            model: cfg.model.clone(),
            api_key: cfg.api_key.clone().unwrap_or_default(),
            base_url: cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "https://openrouter.ai/api/v1".into()),
        }
    }
}

#[async_trait]
impl Provider for OpenRouterProvider {
    async fn stream_completion(&self, system: &str, user: &str) -> Result<ChunkStream, String> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let body = json!({
            "model": self.model,
            "stream": true,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user",   "content": user},
            ]
        });
        // OpenRouter uses HTTP-Referer + X-Title for app-attribution;
        // repository URL is read from Cargo.toml so forks identify themselves.
        post_openai_chat(
            &url,
            body,
            Some(&self.api_key),
            &[
                ("HTTP-Referer", env!("CARGO_PKG_REPOSITORY")),
                ("X-Title", "Quill"),
            ],
        )
        .await
    }
}
