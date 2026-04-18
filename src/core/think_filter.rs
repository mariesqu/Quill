//! Streaming filter that hides `<think>...</think>` reasoning blocks emitted
//! by chain-of-thought models (Qwen3, DeepSeek-R1, etc.).
//!
//! Reasoning models prepend their internal monologue as `<think>...</think>`
//! before the actual answer. Quill's UI should show the answer only, and
//! history should save the answer only — saving the reasoning would bloat
//! storage and leak prompt-engineering details into tutor lessons.
//!
//! The filter is a stateful stream transformer: tokens arrive one chunk at
//! a time, and the tags may be split across chunk boundaries. `push` returns
//! only the user-visible portion of each chunk; `flush` drains any held-back
//! partial-tag content at end-of-stream.
//!
//! Currently only the `<think>` / `</think>` tag pair is recognised. If we
//! add models that use different delimiters, add them as additional tag
//! pairs in `TAGS` rather than duplicating the state machine.

const OPEN_TAG: &str = "<think>";
const CLOSE_TAG: &str = "</think>";

pub struct ThinkFilter {
    inside: bool,
    pending: String,
}

impl ThinkFilter {
    pub fn new() -> Self {
        Self {
            inside: false,
            pending: String::new(),
        }
    }

    /// Feed a chunk in. Returns the portion (possibly empty) that should be
    /// forwarded to the user — i.e. the content OUTSIDE any think block,
    /// with partial tag prefixes held back until the next chunk disambiguates
    /// them.
    pub fn push(&mut self, chunk: &str) -> String {
        self.pending.push_str(chunk);
        let mut out = String::new();
        loop {
            if !self.inside {
                if let Some(idx) = self.pending.find(OPEN_TAG) {
                    out.push_str(&self.pending[..idx]);
                    self.pending.drain(..idx + OPEN_TAG.len());
                    self.inside = true;
                    continue;
                }
                // No complete open tag. Hold back the longest suffix of
                // `pending` that is a non-empty prefix of `OPEN_TAG` — it
                // might become a tag once more bytes arrive. Emit the rest.
                let hold = partial_prefix_end(&self.pending, OPEN_TAG);
                let emit_end = self.pending.len() - hold;
                out.push_str(&self.pending[..emit_end]);
                self.pending.drain(..emit_end);
                break;
            }

            if let Some(idx) = self.pending.find(CLOSE_TAG) {
                // Drop everything up to and through the close tag.
                self.pending.drain(..idx + CLOSE_TAG.len());
                self.inside = false;
                continue;
            }
            // Still inside. Keep any partial-close-tag suffix; drop the rest.
            let hold = partial_prefix_end(&self.pending, CLOSE_TAG);
            let drop_end = self.pending.len() - hold;
            self.pending.drain(..drop_end);
            break;
        }
        out
    }

    /// Emit any remaining held-back content at end-of-stream.
    ///
    /// If we were OUTSIDE a think block when the stream ended, anything we
    /// held back (thinking it might be a partial `<think>`) turned out to be
    /// plain content — emit it. If we were INSIDE, the pending bytes are
    /// partial `</think>` that never completed, so we drop them along with
    /// any other buffered reasoning content.
    pub fn flush(&mut self) -> String {
        if self.inside {
            self.pending.clear();
            String::new()
        } else {
            std::mem::take(&mut self.pending)
        }
    }
}

impl Default for ThinkFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// Longest non-empty prefix of `target` that is also a suffix of `s`.
///
/// Used to decide how many trailing bytes of the pending buffer to hold back
/// because they might be the beginning of a tag once the next chunk arrives.
/// `target` must be ASCII (both real tags are) — this lets us slice by byte
/// indices without worrying about UTF-8 boundaries.
fn partial_prefix_end(s: &str, target: &str) -> usize {
    debug_assert!(target.is_ascii());
    for i in (1..target.len()).rev() {
        let prefix = &target[..i];
        if s.ends_with(prefix) {
            return i;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filter_all(chunks: &[&str]) -> String {
        let mut f = ThinkFilter::new();
        let mut out = String::new();
        for c in chunks {
            out.push_str(&f.push(c));
        }
        out.push_str(&f.flush());
        out
    }

    #[test]
    fn passes_through_plain_text() {
        assert_eq!(filter_all(&["hello world"]), "hello world");
    }

    #[test]
    fn strips_simple_think_block() {
        assert_eq!(
            filter_all(&["<think>reasoning here</think>answer"]),
            "answer"
        );
    }

    #[test]
    fn strips_think_block_before_answer() {
        assert_eq!(
            filter_all(&["<think>\nlet me think...\n</think>\nFinal answer."]),
            "\nFinal answer."
        );
    }

    #[test]
    fn keeps_content_before_and_after_think_block() {
        assert_eq!(
            filter_all(&["before <think>hidden</think> after"]),
            "before  after"
        );
    }

    #[test]
    fn handles_tag_split_across_chunks() {
        // Open tag split 3 ways, close tag split 2 ways.
        assert_eq!(
            filter_all(&["abc<", "thi", "nk>hidden<", "/think>done"]),
            "abcdone"
        );
    }

    #[test]
    fn handles_tag_split_mid_word() {
        assert_eq!(filter_all(&["<thin", "k>reason</thi", "nk>x"]), "x");
    }

    #[test]
    fn handles_multiple_think_blocks() {
        assert_eq!(filter_all(&["a<think>1</think>b<think>2</think>c"]), "abc");
    }

    #[test]
    fn unterminated_think_block_drops_tail() {
        // Model cut off mid-reasoning — the rest is lost, but we don't leak
        // the partial monologue.
        assert_eq!(filter_all(&["start<think>reasoning never ends"]), "start");
    }

    #[test]
    fn partial_open_tag_at_eof_is_emitted() {
        // We held back "<thi" waiting to disambiguate. Stream ends → it's
        // just content after all.
        assert_eq!(filter_all(&["value = <thi"]), "value = <thi");
    }

    #[test]
    fn lone_angle_bracket_is_emitted_eventually() {
        // `<x` can't be the start of `<think>`, so nothing gets held back.
        assert_eq!(filter_all(&["a < b and c"]), "a < b and c");
    }

    #[test]
    fn handles_unicode_around_tags() {
        // Partial prefix detection must not panic on multibyte chars near
        // the tail. Chinese before, emoji after.
        assert_eq!(
            filter_all(&["你好<think>推理</think>🚀 done"]),
            "你好🚀 done"
        );
    }

    #[test]
    fn every_byte_as_its_own_chunk() {
        // Pathological: every char arrives on its own. The filter must
        // still correctly identify and strip the tag.
        let input = "x<think>y</think>z";
        let chunks: Vec<String> = input.chars().map(|c| c.to_string()).collect();
        let refs: Vec<&str> = chunks.iter().map(String::as_str).collect();
        assert_eq!(filter_all(&refs), "xz");
    }

    #[test]
    fn close_tag_without_open_is_plain_text() {
        // We're never inside a think block, so `</think>` is just content.
        assert_eq!(
            filter_all(&["no open tag </think> here"]),
            "no open tag </think> here"
        );
    }
}
