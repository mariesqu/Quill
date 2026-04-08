use async_trait::async_trait;
use futures_util::Stream;
use std::pin::Pin;
use std::sync::LazyLock;

use crate::core::config::Config;

pub type ChunkStream = Pin<Box<dyn Stream<Item = String> + Send>>;

#[async_trait]
pub trait Provider: Send + Sync {
    async fn stream_completion(&self, system: &str, user: &str) -> Result<ChunkStream, String>;
}

pub mod generic;
pub mod ollama;
pub mod openai;
pub mod openrouter;

/// Process-wide shared `reqwest::Client`. Reusing one instance gives connection
/// pooling, DNS caching, and TLS session resumption — the first call pays the
/// handshake cost, subsequent calls reuse the pool.
pub static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent(concat!("quill/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("failed to build shared reqwest client")
});

pub fn build_provider(cfg: &Config) -> Box<dyn Provider> {
    match cfg.provider.as_str() {
        "openrouter" => Box::new(openrouter::OpenRouterProvider::new(cfg)),
        "ollama" => Box::new(ollama::OllamaProvider::new(cfg)),
        "openai" => Box::new(openai::OpenAIProvider::new(cfg)),
        _ => Box::new(generic::GenericProvider::new(cfg)),
    }
}

// ── Shared SSE parsing ────────────────────────────────────────────────────────

use futures_util::{stream, StreamExt};
use reqwest::Response;
use serde_json::Value;

/// Parse an OpenAI-compatible SSE stream.
///
/// Bytes are buffered in a `Vec<u8>` (NOT a `String`) so that multi-byte UTF-8
/// characters split across HTTP chunk boundaries are reassembled correctly
/// instead of being replaced with `U+FFFD` by `String::from_utf8_lossy`.
/// We split on `\n` (0x0A) — because UTF-8 is self-synchronising, a `\n` byte
/// never appears inside a multi-byte sequence, so slicing at newline positions
/// is always safe.
pub fn openai_sse_stream(response: Response) -> ChunkStream {
    let bytes_stream = response.bytes_stream();
    let text_stream = stream::unfold(
        (bytes_stream, Vec::<u8>::new()),
        |(mut stream, mut buf)| async move {
            loop {
                // Try to extract one complete line from the byte buffer.
                if let Some(nl) = buf.iter().position(|&b| b == b'\n') {
                    let line_bytes: Vec<u8> = buf.drain(..=nl).collect();
                    // Decode the line (without the trailing newline).
                    let line = std::str::from_utf8(&line_bytes[..line_bytes.len() - 1])
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default();

                    if let Some(data) = line.strip_prefix("data: ") {
                        if data.trim() == "[DONE]" {
                            return None;
                        }
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

                // No complete line yet — pull more bytes.
                match stream.next().await {
                    Some(Ok(bytes)) => buf.extend_from_slice(&bytes),
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
        let msg = serde_json::from_str::<Value>(body).ok().and_then(|v| {
            v["error"]["message"]
                .as_str()
                .or(v["message"].as_str())
                .map(|s| s.to_string())
        });
        match msg {
            Some(m) => format!("{base}: {m}"),
            None => format!("{base} (HTTP {status})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_statuses_map_to_human_messages() {
        assert!(friendly_error(401, "").contains("Authentication failed"));
        assert!(friendly_error(403, "").contains("Access denied"));
        assert!(friendly_error(404, "").contains("Model not found"));
        assert!(friendly_error(429, "").contains("Rate limit"));
        assert!(friendly_error(500, "").contains("Provider server error"));
        assert!(friendly_error(502, "").contains("Provider server error"));
        assert!(friendly_error(503, "").contains("Provider server error"));
    }

    #[test]
    fn unknown_status_falls_back_to_generic() {
        let msg = friendly_error(418, "");
        assert!(msg.contains("Unexpected"));
        assert!(msg.contains("418"));
    }

    #[test]
    fn empty_body_appends_status_code() {
        let msg = friendly_error(500, "");
        assert!(msg.contains("(HTTP 500)"));
    }

    #[test]
    fn json_error_object_message_is_extracted() {
        let body = r#"{"error":{"message":"your api key is invalid","type":"auth"}}"#;
        let msg = friendly_error(401, body);
        assert!(msg.contains("Authentication failed"));
        assert!(msg.contains("your api key is invalid"));
    }

    #[test]
    fn json_top_level_message_is_extracted() {
        let body = r#"{"message":"too many requests"}"#;
        let msg = friendly_error(429, body);
        assert!(msg.contains("Rate limit"));
        assert!(msg.contains("too many requests"));
    }

    #[test]
    fn non_json_body_falls_back_to_status_code() {
        let msg = friendly_error(500, "<html>gateway timeout</html>");
        assert!(msg.contains("Provider server error"));
        assert!(msg.contains("(HTTP 500)"));
    }
}
