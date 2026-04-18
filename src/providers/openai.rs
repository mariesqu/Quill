use async_trait::async_trait;
use serde_json::json;

use super::{post_openai_chat, ChunkStream, Provider};
use crate::core::config::Config;

pub struct OpenAIProvider {
    model: String,
    api_key: String,
    base_url: String,
}

impl OpenAIProvider {
    pub fn new(cfg: &Config) -> Self {
        Self {
            model: cfg.model.clone(),
            api_key: cfg.api_key.clone().unwrap_or_default(),
            base_url: cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".into()),
        }
    }
}

#[async_trait]
impl Provider for OpenAIProvider {
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
        post_openai_chat(&url, body, Some(&self.api_key), &[]).await
    }
}
