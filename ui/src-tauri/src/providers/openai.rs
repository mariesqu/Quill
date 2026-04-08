use async_trait::async_trait;
use serde_json::json;

use super::{friendly_error, openai_sse_stream, ChunkStream, Provider, HTTP_CLIENT};
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

        let resp = HTTP_CLIENT
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Network error: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(friendly_error(status, &body));
        }

        Ok(openai_sse_stream(resp))
    }
}
