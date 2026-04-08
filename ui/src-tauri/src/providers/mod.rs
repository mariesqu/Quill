use async_trait::async_trait;
use futures_util::Stream;
use std::pin::Pin;

use crate::core::config::Config;

pub type ChunkStream = Pin<Box<dyn Stream<Item = String> + Send>>;

#[async_trait]
pub trait Provider: Send + Sync {
    async fn stream_completion(&self, system: &str, user: &str) -> Result<ChunkStream, String>;
}

pub mod openrouter;
pub mod openai;
pub mod ollama;
pub mod generic;

pub fn build_provider(cfg: &Config) -> Box<dyn Provider> {
    match cfg.provider.as_str() {
        "openrouter" => Box::new(openrouter::OpenRouterProvider::new(cfg)),
        "ollama"     => Box::new(ollama::OllamaProvider::new(cfg)),
        "openai"     => Box::new(openai::OpenAIProvider::new(cfg)),
        _            => Box::new(generic::GenericProvider::new(cfg)),
    }
}

// ── Shared SSE parsing ────────────────────────────────────────────────────────

use futures_util::{StreamExt, stream};
use reqwest::Response;
use serde_json::Value;

pub fn openai_sse_stream(response: Response) -> ChunkStream {
    let bytes_stream = response.bytes_stream();
    let text_stream = stream::unfold(
        (bytes_stream, String::new()),
        |(mut stream, mut buf)| async move {
            loop {
                if let Some(nl) = buf.find('\n') {
                    let line = buf[..nl].trim().to_string();
                    buf = buf[nl + 1..].to_string();

                    if let Some(data) = line.strip_prefix("data: ") {
                        if data.trim() == "[DONE]" { return None; }
                        if let Ok(json) = serde_json::from_str::<Value>(data) {
                            if let Some(content) = json["choices"][0]["delta"]["content"].as_str() {
                                if !content.is_empty() {
                                    return Some((content.to_string(), (stream, buf)));
                                }
                            }
                        }
                    }
                    continue;
                }

                match stream.next().await {
                    Some(Ok(bytes)) => {
                        buf.push_str(&String::from_utf8_lossy(&bytes));
                    }
                    _ => return None,
                }
            }
        },
    );
    Box::pin(text_stream)
}

/// Friendly error from HTTP status + body.
pub fn friendly_error(status: u16, body: &str) -> String {
    let base = match status {
        400 => "Bad request — check your model name or prompt format",
        401 => "Authentication failed — check your API key",
        403 => "Access denied — your API key may not have permission for this model",
        404 => "Model not found — check the model name in Settings",
        429 => "Rate limit exceeded — try again in a moment",
        500 | 502 | 503 => "Provider server error — try again shortly",
        _ => "Unexpected error from provider",
    };
    if body.is_empty() {
        format!("{base} (HTTP {status})")
    } else {
        // Extract message from JSON error bodies
        let msg = serde_json::from_str::<Value>(body)
            .ok()
            .and_then(|v| {
                v["error"]["message"].as_str()
                    .or(v["message"].as_str())
                    .map(|s| s.to_string())
            });
        match msg {
            Some(m) => format!("{base}: {m}"),
            None    => format!("{base} (HTTP {status})"),
        }
    }
}
