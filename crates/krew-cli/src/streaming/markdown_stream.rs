//! Newline-gated markdown stream collector.
//!
//! Accumulates raw text deltas and only renders complete lines (on `\n`).
//! Re-renders the full buffer on each commit to correctly handle
//! context-dependent markdown (e.g. code blocks, nested lists).

use ratatui::text::Line;

use crate::render::markdown::render_markdown;

/// Collects streaming text deltas and renders on newline boundaries.
pub struct MarkdownStreamCollector {
    /// Raw text buffer accumulating deltas.
    buffer: String,
    /// Number of lines already committed (rendered and returned).
    committed_line_count: usize,
    /// Byte offset after the last committed newline boundary.
    committed_byte_offset: usize,
    /// Whether there are uncommitted newlines in the buffer.
    pending_newline: bool,
}

impl MarkdownStreamCollector {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            committed_line_count: 0,
            committed_byte_offset: 0,
            pending_newline: false,
        }
    }

    /// Append a text delta to the buffer.
    pub fn push_delta(&mut self, delta: &str) {
        self.buffer.push_str(delta);
        if delta.contains('\n') {
            self.pending_newline = true;
        }
    }

    /// Check if the buffer contains any uncommitted newlines.
    pub fn has_pending_newline(&self) -> bool {
        self.pending_newline
    }

    /// Render complete lines up to the last `\n` and return only the new ones.
    ///
    /// Returns an empty vec if no new complete lines are available.
    pub fn commit_complete_lines(&mut self) -> Vec<Line<'static>> {
        self.pending_newline = false;

        // Find the last newline in the buffer.
        let last_newline = match self.buffer.rfind('\n') {
            Some(pos) => pos,
            None => return Vec::new(),
        };

        // Render the buffer up to (and including) the last newline.
        let renderable = &self.buffer[..=last_newline];
        let all_lines = render_markdown(renderable);

        // Don't commit a trailing blank line — it may be a separator for
        // the next block that hasn't arrived yet. Committing it now would
        // cause a duplicate blank when the next block triggers another
        // separator during re-render.
        let mut complete_count = all_lines.len();
        if complete_count > 0 && is_blank_line(&all_lines[complete_count - 1]) {
            complete_count -= 1;
        }

        // Return only the lines that are new since the last commit.
        let new_lines = if complete_count > self.committed_line_count {
            all_lines[self.committed_line_count..complete_count].to_vec()
        } else {
            Vec::new()
        };

        self.committed_line_count = complete_count;
        self.committed_byte_offset = last_newline + 1;
        new_lines
    }

    /// Render and return any remaining content (called when stream ends).
    pub fn finalize(&mut self) -> Vec<Line<'static>> {
        self.pending_newline = false;

        if self.buffer.is_empty() {
            return Vec::new();
        }

        // Render the entire buffer.
        let all_lines = render_markdown(&self.buffer);

        let new_lines = if all_lines.len() > self.committed_line_count {
            // Normal case: new content produced additional lines.
            all_lines[self.committed_line_count..].to_vec()
        } else if self.committed_byte_offset < self.buffer.len() {
            // Paragraph merge: new content was absorbed into existing lines
            // by markdown soft-break rules. Render the uncommitted tail
            // as standalone text so it doesn't get lost.
            let tail = &self.buffer[self.committed_byte_offset..];
            if tail.trim().is_empty() {
                Vec::new()
            } else {
                render_markdown(tail)
            }
        } else {
            Vec::new()
        };

        // Reset state.
        self.buffer.clear();
        self.committed_line_count = 0;
        self.committed_byte_offset = 0;

        new_lines
    }
}

/// Consider a line blank if it has no spans or only spans with empty/space-only content.
fn is_blank_line(line: &Line<'_>) -> bool {
    line.spans.is_empty()
        || line
            .spans
            .iter()
            .all(|s| s.content.is_empty() || s.content.chars().all(|c| c == ' '))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_newline_no_output() {
        let mut collector = MarkdownStreamCollector::new();
        collector.push_delta("hello ");
        collector.push_delta("world");
        let lines = collector.commit_complete_lines();
        assert!(lines.is_empty());
    }

    #[test]
    fn newline_triggers_render() {
        let mut collector = MarkdownStreamCollector::new();
        collector.push_delta("hello world\n");
        let lines = collector.commit_complete_lines();
        assert!(!lines.is_empty());
        let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("hello world"));
    }

    #[test]
    fn incremental_paragraphs() {
        let mut collector = MarkdownStreamCollector::new();

        // First paragraph (double newline = paragraph break in markdown).
        collector.push_delta("paragraph 1\n\n");
        let first = collector.commit_complete_lines();
        assert!(!first.is_empty());

        // Second paragraph.
        collector.push_delta("paragraph 2\n\n");
        let second = collector.commit_complete_lines();
        assert!(!second.is_empty());
        let text: String = second
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
        assert!(text.contains("paragraph 2"));
    }

    #[test]
    fn finalize_returns_remaining() {
        let mut collector = MarkdownStreamCollector::new();
        collector.push_delta("first paragraph\n\n");
        collector.commit_complete_lines();

        // Push content without trailing newline.
        collector.push_delta("remaining text");
        let remaining = collector.finalize();
        assert!(!remaining.is_empty());
        let text: String = remaining
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
        assert!(text.contains("remaining text"));
    }

    #[test]
    fn finalize_empty_buffer() {
        let mut collector = MarkdownStreamCollector::new();
        let lines = collector.finalize();
        assert!(lines.is_empty());
    }

    #[test]
    fn has_pending_newline_tracking() {
        let mut collector = MarkdownStreamCollector::new();
        collector.push_delta("no newline");
        assert!(!collector.has_pending_newline());

        collector.push_delta("\n");
        assert!(collector.has_pending_newline());

        // After commit, pending is cleared.
        collector.commit_complete_lines();
        assert!(!collector.has_pending_newline());

        // New newline sets it again.
        collector.push_delta("more\n");
        assert!(collector.has_pending_newline());
    }

    #[test]
    fn finalize_after_soft_break() {
        // Regression: when committed text is followed by content without
        // any newline, markdown treats single \n as a soft break and merges
        // them into one paragraph.  finalize() must still return the tail.
        let mut collector = MarkdownStreamCollector::new();
        collector.push_delta("name=doubao\n");
        let first = collector.commit_complete_lines();
        assert!(!first.is_empty());

        // Push a long paragraph without any \n.
        collector.push_delta("hello world this is a long response");
        let remaining = collector.finalize();
        assert!(!remaining.is_empty());
        let text: String = remaining
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
        assert!(text.contains("hello world"));
    }

    #[test]
    fn code_block_streaming() {
        let mut collector = MarkdownStreamCollector::new();

        // Stream a code block incrementally.
        collector.push_delta("```rust\n");
        collector.push_delta("fn main() {\n");
        collector.push_delta("    println!(\"hello\");\n");
        collector.push_delta("}\n");
        collector.push_delta("```\n");

        let lines = collector.commit_complete_lines();
        assert!(!lines.is_empty());
    }
}
