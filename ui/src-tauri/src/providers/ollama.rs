use async_trait::async_trait;
use futures_util::{stream, StreamExt};
use reqwest::Client;
use serde_json::{json, Value};

use crate::core::config::Config;
use super::{ChunkStream, Provider};

pub struct OllamaProvider {
    client:   Client,
    model:    String,
    base_url: String,
}

impl OllamaProvider {
    pub fn new(cfg: &Config) -> Self {
        Self {
            client:   Client::new(),
            model:    cfg.model.clone(),
            base_url: cfg.base_url.clone()
                .unwrap_or_else(|| "http://localhost:11434".into()),
        }
    }
}

#[async_trait]
impl Provider for OllamaProvider {
    async fn stream_completion(&self, system: &str, user: &str) -> Result<ChunkStream, String> {
        let url = format!("{}/api/generate", self.base_url.trim_end_matches('/'));
        let body = json!({
            "model":  self.model,
            "system": system,
            "prompt": user,
            "stream": true,
        });

        let resp = self.client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Ollama connection error: {e}. Is Ollama running?"))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Ollama error (HTTP {status}): {body}"));
        }

        let text_stream = stream::unfold(
            (resp.bytes_stream(), String::new()),
            |(mut stream, mut buf)| async move {
                loop {
                    if let Some(nl) = buf.find('\n') {
                        let line = buf[..nl].to_string();
                        buf = buf[nl + 1..].to_string();

                        if let Ok(json) = serde_json::from_str::<Value>(&line) {
                            if json["done"].as_bool().unwrap_or(false) { return None; }
                            if let Some(token) = json["response"].as_str() {
                                if !token.is_empty() {
                                    return Some((token.to_string(), (stream, buf)));
                                }
                            }
                        }
                        continue;
                    }

                    match stream.next().await {
                        Some(Ok(bytes)) => buf.push_str(&String::from_utf8_lossy(&bytes)),
                        _ => return None,
                    }
                }
            },
        );

        Ok(Box::pin(text_stream))
    }
}
