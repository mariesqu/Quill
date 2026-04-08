// Generic OpenAI-compatible provider — for LM Studio, Groq, Jan.ai, etc.
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;

use crate::core::config::Config;
use super::{ChunkStream, Provider, friendly_error, openai_sse_stream};

pub struct GenericProvider {
    client:   Client,
    model:    String,
    api_key:  String,
    base_url: String,
}

impl GenericProvider {
    pub fn new(cfg: &Config) -> Self {
        Self {
            client:   Client::new(),
            model:    cfg.model.clone(),
            api_key:  cfg.api_key.clone().unwrap_or_default(),
            base_url: cfg.base_url.clone()
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

        let mut req = self.client.post(&url).json(&body);
        if !self.api_key.is_empty() {
            req = req.bearer_auth(&self.api_key);
        }

        let resp = req.send().await.map_err(|e| format!("Network error: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(friendly_error(status, &body));
        }

        Ok(openai_sse_stream(resp))
    }
}
