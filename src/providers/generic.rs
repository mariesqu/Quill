// Generic OpenAI-compatible provider — for LM Studio, Groq, Jan.ai, etc.
use async_trait::async_trait;
use serde_json::json;

use super::{post_openai_chat, ChunkStream, Provider};
use crate::core::config::Config;

pub struct GenericProvider {
    model: String,
    api_key: String,
    base_url: String,
}

impl GenericProvider {
    pub fn new(cfg: &Config) -> Self {
        Self {
            model: cfg.model.clone(),
            api_key: cfg.api_key.clone().unwrap_or_default(),
            base_url: cfg
                .base_url
                .clone()
                .unwrap_or_else(|| "http://localhost:1234/v1".into()),
        }
    }
}

#[async_trait]
impl Provider for GenericProvider {
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
        // Pass Some(&api_key) even when empty — post_openai_chat filters
        // empty strings so local endpoints without auth still work.
        post_openai_chat(&url, body, Some(&self.api_key), &[]).await
    }
}
