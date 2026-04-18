//! Claude Code CLI provider — shells out to the local `claude` binary so
//! users can ride their Claude Max / Pro subscription instead of paying
//! the OpenRouter roundtrip.
//!
//! Why: the Claude Code CLI (`claude -p "..."`) authenticates with the
//! user's local Anthropic session (set up once via `claude login`) and
//! talks directly to Anthropic's servers. This avoids the double hop
//! through OpenRouter and lets the user pick between `haiku` (fast),
//! `sonnet` (balanced), and `opus` (max quality) via the usual aliases.
//!
//! Streaming: we read the CLI's stdout in 256-byte bursts and emit them
//! as they arrive, splitting only on UTF-8 boundaries so multi-byte
//! codepoints spanning a chunk boundary don't corrupt into `U+FFFD`.
//! The child process is configured with `kill_on_drop(true)` so a
//! cancelled stream (user hit CANCEL) tears down the spawned CLI
//! instead of leaking it in the background.

use async_trait::async_trait;
use futures_util::stream;
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

use crate::core::config::Config;
use crate::providers::{ChunkStream, Provider};

pub struct ClaudeCliProvider {
    /// Model alias or full ID passed through as `--model <value>`. When
    /// empty, the flag is omitted and the CLI uses its configured default
    /// (typically the latest Sonnet). Common aliases: `haiku`, `sonnet`,
    /// `opus`.
    model: String,
}

impl ClaudeCliProvider {
    pub fn new(cfg: &Config) -> Self {
        Self {
            model: cfg.model.clone(),
        }
    }
}

#[async_trait]
impl Provider for ClaudeCliProvider {
    async fn stream_completion(&self, system: &str, user: &str) -> Result<ChunkStream, String> {
        // We invoke the CLI in a "pure LLM completion" shape, not as an
        // agent. Claude Code's default system prompt turns it into an
        // agentic assistant (tool use, CLAUDE.md discovery, project-aware
        // context). For a rewrite/translate prompt that's wrong — we want
        // the model to act on OUR system prompt alone. Key flags:
        //   --system-prompt  replaces Claude's default system prompt
        //   --tools ""       disables all tools
        //   --permission-mode bypassPermissions  no approval prompts
        //   --output-format text  plain stdout (no JSON envelope)
        //   current_dir(temp)  avoid picking up cwd-level CLAUDE.md
        tracing::info!(
            model = %self.model,
            system_len = system.len(),
            user_len = user.len(),
            "claude-cli: spawning child"
        );

        let mut child = spawn_claude(&self.model, system, user).await.map_err(|e| {
            tracing::error!(error = %e, "claude-cli: spawn failed");
            format!(
                "Failed to launch `claude`: {e}. Install the Claude Code CLI \
                     (`npm install -g @anthropic-ai/claude-code`), sign in with \
                     `claude login`, and make sure it's on PATH."
            )
        })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "claude stdout not captured".to_string())?;
        // Drain stderr into a background task so a full pipe buffer can't
        // block the CLI. Keep the text for deferred error reporting when
        // the child exits non-zero.
        let stderr_task = child.stderr.take().map(|mut s| {
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let _ = s.read_to_end(&mut buf).await;
                String::from_utf8_lossy(&buf).to_string()
            })
        });

        tracing::info!("claude-cli: child spawned, streaming stdout");
        Ok(Box::pin(stdout_chunk_stream(stdout, child, stderr_task)))
    }
}

/// Spawn the CLI with `kill_on_drop(true)` so a cancelled ChunkStream
/// terminates the child when the stream future is dropped. If the bare
/// name isn't found on Windows (e.g., claude is installed as a `.cmd`
/// shim and PATHEXT resolution doesn't catch it), retry once with the
/// `.cmd` suffix.
async fn spawn_claude(
    model: &str,
    system: &str,
    user: &str,
) -> std::io::Result<tokio::process::Child> {
    fn build_cmd(bin: &str, model: &str, system: &str, user: &str) -> Command {
        let mut cmd = Command::new(bin);
        cmd.arg("-p")
            .arg("--output-format")
            .arg("text")
            .arg("--permission-mode")
            .arg("bypassPermissions")
            .arg("--tools")
            .arg("");
        if !system.is_empty() {
            cmd.arg("--system-prompt").arg(system);
        }
        if !model.is_empty() {
            cmd.arg("--model").arg(model);
        }
        // The positional prompt arg. Everything after this is the user
        // payload — the CLI treats it as the first user message to the
        // session.
        cmd.arg(user);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            // Running from a neutral directory keeps Claude from
            // inadvertently discovering a project-level CLAUDE.md (Quill's
            // cwd could be the user's dev repo). `--system-prompt` above
            // already replaces the default system prompt, but this is a
            // belt-and-braces isolation against any context leakage.
            .current_dir(std::env::temp_dir());
        cmd
    }

    match build_cmd("claude", model, system, user).spawn() {
        Ok(c) => Ok(c),
        Err(e) if cfg!(windows) && e.kind() == std::io::ErrorKind::NotFound => {
            // npm installs ship a `.cmd` shim on Windows; std's PATHEXT
            // resolution doesn't cover .cmd, so retry explicitly.
            build_cmd("claude.cmd", model, system, user).spawn()
        }
        Err(e) => Err(e),
    }
}

/// Build a `ChunkStream` that reads `stdout` in 256-byte bursts, emits
/// every complete-UTF-8 prefix as a `String` chunk, and preserves any
/// trailing partial codepoint for the next read. On EOF, the child is
/// awaited; if it exited non-zero, a final `[claude error: ...]` chunk
/// is yielded with the captured stderr so the user sees the failure.
fn stdout_chunk_stream<R>(
    reader: R,
    child: tokio::process::Child,
    stderr_task: Option<tokio::task::JoinHandle<String>>,
) -> impl futures_util::Stream<Item = String> + Send
where
    R: AsyncReadExt + Unpin + Send + 'static,
{
    stream::unfold(
        (reader, Vec::<u8>::new(), Some(child), stderr_task, false),
        |(mut reader, mut buf, mut child, mut stderr_task, done)| async move {
            if done {
                return None;
            }
            let mut bytes = [0u8; 256];
            loop {
                match reader.read(&mut bytes).await {
                    Ok(0) => {
                        // EOF — collect exit status + stderr for a clean
                        // terminal error chunk if the child failed.
                        let status = match child.take() {
                            Some(mut c) => c.wait().await.ok(),
                            None => None,
                        };
                        let stderr_text = match stderr_task.take() {
                            Some(t) => t.await.unwrap_or_default(),
                            None => String::new(),
                        };
                        tracing::info!(
                            status = ?status,
                            stderr_len = stderr_text.len(),
                            tail_len = buf.len(),
                            "claude-cli: stdout EOF"
                        );
                        if !stderr_text.trim().is_empty() {
                            tracing::warn!(stderr = %stderr_text.trim(), "claude-cli stderr");
                        }
                        if !buf.is_empty() {
                            let tail = String::from_utf8_lossy(&buf).to_string();
                            return Some((tail, (reader, Vec::new(), None, None, true)));
                        }
                        let failed = status.map(|s| !s.success()).unwrap_or(false);
                        if failed && !stderr_text.trim().is_empty() {
                            let msg = format!("\n[claude error: {}]", stderr_text.trim());
                            return Some((msg, (reader, Vec::new(), None, None, true)));
                        }
                        return None;
                    }
                    Ok(n) => {
                        buf.extend_from_slice(&bytes[..n]);
                        tracing::trace!(bytes = n, buf_len = buf.len(), "claude-cli: read");
                        // Longest valid-UTF-8 prefix becomes the emitted
                        // chunk; the trailing partial codepoint (if any)
                        // carries over to the next read.
                        let valid_up_to = match std::str::from_utf8(&buf) {
                            Ok(_) => buf.len(),
                            Err(e) => e.valid_up_to(),
                        };
                        if valid_up_to == 0 {
                            continue;
                        }
                        let rest = buf.split_off(valid_up_to);
                        let text = String::from_utf8(std::mem::replace(&mut buf, rest))
                            .unwrap_or_default();
                        if text.is_empty() {
                            continue;
                        }
                        tracing::debug!(chunk_len = text.len(), "claude-cli: emit chunk");
                        return Some((text, (reader, buf, child, stderr_task, false)));
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "claude-cli: stdout read error");
                        return None;
                    }
                }
            }
        },
    )
}
