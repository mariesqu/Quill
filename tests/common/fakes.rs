//! In-memory platform + provider fakes used by Tier 2 engine integration tests.

use std::sync::Mutex;

use async_trait::async_trait;
use futures_util::stream;

use quill::platform::context::AppContext;
use quill::platform::traits::{
    CaptureResult, CaptureSource, ContextProbe, ScreenRect, TextCapture, TextReplace,
};
use quill::providers::{ChunkStream, Provider};

// ── FakeCapture ───────────────────────────────────────────────────────────

#[derive(Default)]
pub struct FakeCapture {
    queue: Mutex<Vec<CaptureResult>>,
}

impl FakeCapture {
    pub fn with_text(text: &str) -> Self {
        Self {
            queue: Mutex::new(vec![CaptureResult {
                text: text.to_string(),
                anchor: Some(ScreenRect {
                    left: 100,
                    top: 200,
                    right: 400,
                    bottom: 240,
                }),
                source: CaptureSource::Uia,
            }]),
        }
    }

    pub fn empty() -> Self {
        Self {
            queue: Mutex::new(vec![CaptureResult::default()]),
        }
    }
}

#[async_trait]
impl TextCapture for FakeCapture {
    async fn capture(&self) -> CaptureResult {
        self.queue.lock().unwrap().pop().unwrap_or_default()
    }
}

// ── FakeReplace ───────────────────────────────────────────────────────────

#[derive(Default)]
pub struct FakeReplace {
    pub pasted: Mutex<Vec<String>>,
}

impl FakeReplace {
    pub fn last(&self) -> Option<String> {
        self.pasted.lock().unwrap().last().cloned()
    }
}

#[async_trait]
impl TextReplace for FakeReplace {
    async fn paste(&self, text: &str) -> anyhow::Result<()> {
        self.pasted.lock().unwrap().push(text.to_string());
        Ok(())
    }
}

// ── FakeContext ───────────────────────────────────────────────────────────

#[derive(Default)]
pub struct FakeContext {
    pub ctx: AppContext,
}

impl FakeContext {
    pub fn with_app(app: &str, tone: &str, hint: &str) -> Self {
        Self {
            ctx: AppContext {
                app: app.into(),
                tone: tone.into(),
                hint: hint.into(),
            },
        }
    }
}

impl ContextProbe for FakeContext {
    fn active_context(&self) -> AppContext {
        self.ctx.clone()
    }
}

// ── FakeProvider ──────────────────────────────────────────────────────────

pub struct FakeProvider {
    chunks: Vec<String>,
    err: Option<String>,
}

impl FakeProvider {
    pub fn with_chunks(chunks: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            chunks: chunks.into_iter().map(Into::into).collect(),
            err: None,
        }
    }

    pub fn failing(message: impl Into<String>) -> Self {
        Self {
            chunks: vec![],
            err: Some(message.into()),
        }
    }
}

#[async_trait]
impl Provider for FakeProvider {
    async fn stream_completion(&self, _system: &str, _user: &str) -> Result<ChunkStream, String> {
        if let Some(err) = &self.err {
            return Err(err.clone());
        }
        let chunks = self.chunks.clone();
        Ok(Box::pin(stream::iter(chunks)))
    }
}
