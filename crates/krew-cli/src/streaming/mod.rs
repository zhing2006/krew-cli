//! Streaming rendering pipeline with adaptive backpressure.
//!
//! - MarkdownStreamCollector: newline-gated markdown rendering
//! - StreamState: FIFO queue with timestamps
//! - AdaptiveChunkingPolicy: smooth/catch-up mode switching
//! - CommitTick: orchestrates drain decisions

pub mod chunking;
pub mod commit_tick;
pub mod markdown_stream;

use ratatui::text::Line;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// A rendered line waiting in the queue with its enqueue timestamp.
pub struct QueuedLine {
    pub line: Line<'static>,
    pub enqueued_at: Instant,
}

/// FIFO queue of rendered lines with timestamp tracking.
pub struct StreamState {
    queue: VecDeque<QueuedLine>,
}

impl StreamState {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    /// Enqueue rendered lines with current timestamp.
    pub fn enqueue(&mut self, lines: Vec<Line<'static>>) {
        let now = Instant::now();
        for line in lines {
            self.queue.push_back(QueuedLine {
                line,
                enqueued_at: now,
            });
        }
    }

    /// Pop one line from the front.
    pub fn step(&mut self) -> Option<Line<'static>> {
        self.queue.pop_front().map(|q| q.line)
    }

    /// Pop up to `max` lines from the front.
    pub fn drain_n(&mut self, max: usize) -> Vec<Line<'static>> {
        let n = max.min(self.queue.len());
        self.queue.drain(..n).map(|q| q.line).collect()
    }

    /// Pop all queued lines.
    pub fn drain_all(&mut self) -> Vec<Line<'static>> {
        self.queue.drain(..).map(|q| q.line).collect()
    }

    /// Number of lines currently queued.
    pub fn queued_count(&self) -> usize {
        self.queue.len()
    }

    /// Age of the oldest queued line (front of queue).
    pub fn oldest_queued_age(&self, now: Instant) -> Option<Duration> {
        self.queue
            .front()
            .map(|q| now.saturating_duration_since(q.enqueued_at))
    }

    /// Whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::text::Span;

    fn test_line(text: &str) -> Line<'static> {
        Line::from(Span::raw(text.to_string()))
    }

    #[test]
    fn enqueue_and_step() {
        let mut state = StreamState::new();
        state.enqueue(vec![test_line("a"), test_line("b")]);
        assert_eq!(state.queued_count(), 2);

        let line = state.step().unwrap();
        assert_eq!(line.spans[0].content, "a");
        assert_eq!(state.queued_count(), 1);
    }

    #[test]
    fn drain_n_partial() {
        let mut state = StreamState::new();
        state.enqueue(vec![
            test_line("1"),
            test_line("2"),
            test_line("3"),
            test_line("4"),
        ]);

        let drained = state.drain_n(2);
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].spans[0].content, "1");
        assert_eq!(drained[1].spans[0].content, "2");
        assert_eq!(state.queued_count(), 2);
    }

    #[test]
    fn drain_n_exceeds_queue() {
        let mut state = StreamState::new();
        state.enqueue(vec![test_line("only")]);
        let drained = state.drain_n(10);
        assert_eq!(drained.len(), 1);
        assert!(state.is_empty());
    }

    #[test]
    fn drain_all() {
        let mut state = StreamState::new();
        state.enqueue(vec![test_line("a"), test_line("b"), test_line("c")]);
        let all = state.drain_all();
        assert_eq!(all.len(), 3);
        assert!(state.is_empty());
    }

    #[test]
    fn oldest_queued_age() {
        let mut state = StreamState::new();
        assert!(state.oldest_queued_age(Instant::now()).is_none());

        state.enqueue(vec![test_line("x")]);
        let age = state.oldest_queued_age(Instant::now()).unwrap();
        // Should be very recent (< 1 second).
        assert!(age < Duration::from_secs(1));
    }
}
