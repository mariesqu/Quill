use async_trait::async_trait;
use futures_util::{stream, StreamExt};
use serde_json::{json, Value};

use super::{ChunkStream, Provider, HTTP_CLIENT};
use crate::core::config::Config;

pub struct OllamaProvider {
    model: String,
    base_url: String,
}

impl OllamaProvider {
    pub fn new(cfg: &Config) -> Self {
        Self {
            model: cfg.model.clone(),
            base_url: cfg
                .base_url
                .clone()
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

        let resp = HTTP_CLIENT
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

        // Byte-buffered line reader — identical reasoning to `openai_sse_stream`:
        // we never decode partial UTF-8 sequences, which would otherwise corrupt
        // non-Latin characters split across HTTP chunk boundaries.
        let text_stream = stream::unfold(
            (resp.bytes_stream(), Vec::<u8>::new()),
            |(mut stream, mut buf)| async move {
                loop {
                    if let Some(nl) = buf.iter().position(|&b| b == b'\n') {
                        let line_bytes: Vec<u8> = buf.drain(..=nl).collect();
                        let line = std::str::from_utf8(&line_bytes[..line_bytes.len() - 1])
                            .unwrap_or("")
                            .to_string();

                        if let Ok(json) = serde_json::from_str::<Value>(&line) {
                            if json["done"].as_bool().unwrap_or(false) {
                                return None;
                            }
                            if let Some(token) = json["response"].as_str() {
                                if !token.is_empty() {
                                    return Some((token.to_string(), (stream, buf)));
                                }
                            }
                        }
                        continue;
                    }

                    match stream.next().await {
                        Some(Ok(bytes)) => buf.extend_from_slice(&bytes),
                        _ => return None,
                    }
                }
            },
        );

        Ok(Box::pin(text_stream))
    }
}
