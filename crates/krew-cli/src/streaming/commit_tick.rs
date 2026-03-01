//! Commit tick orchestration.
//!
//! Bridges AdaptiveChunkingPolicy decisions to StreamState drain operations.

use ratatui::text::Line;
use std::time::Instant;

use super::StreamState;
use super::chunking::{AdaptiveChunkingPolicy, DrainPlan, QueueSnapshot};

/// Result of a single commit tick.
pub struct CommitTickOutput {
    /// Lines to render (insert above viewport).
    pub lines: Vec<Line<'static>>,
    /// Whether the stream is idle (queue empty and no pending data).
    pub is_idle: bool,
}

/// Run one commit tick cycle: snapshot → decide → drain.
pub fn run_commit_tick(
    policy: &mut AdaptiveChunkingPolicy,
    state: &mut StreamState,
    now: Instant,
) -> CommitTickOutput {
    let snapshot = QueueSnapshot {
        queued_lines: state.queued_count(),
        oldest_age: state.oldest_queued_age(now),
    };

    let decision = policy.decide(&snapshot, now);

    let lines = match decision.drain_count {
        DrainPlan::Single => state.step().into_iter().collect(),
        DrainPlan::Batch(n) => {
            if n == 0 {
                state.drain_all()
            } else {
                state.drain_n(n)
            }
        }
    };

    let is_idle = state.is_empty();

    CommitTickOutput { lines, is_idle }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::text::Span;

    fn test_line(text: &str) -> Line<'static> {
        Line::from(Span::raw(text.to_string()))
    }

    #[test]
    fn smooth_drains_one() {
        let mut policy = AdaptiveChunkingPolicy::new();
        let mut state = StreamState::new();
        state.enqueue(vec![test_line("a"), test_line("b"), test_line("c")]);

        let output = run_commit_tick(&mut policy, &mut state, Instant::now());
        assert_eq!(output.lines.len(), 1);
        assert_eq!(output.lines[0].spans[0].content, "a");
        assert!(!output.is_idle);
    }

    #[test]
    fn empty_queue_is_idle() {
        let mut policy = AdaptiveChunkingPolicy::new();
        let mut state = StreamState::new();

        let output = run_commit_tick(&mut policy, &mut state, Instant::now());
        assert!(output.lines.is_empty());
        assert!(output.is_idle);
    }

    #[test]
    fn catch_up_drains_all() {
        let mut policy = AdaptiveChunkingPolicy::new();
        let mut state = StreamState::new();

        // Enqueue enough to trigger CatchUp.
        let lines: Vec<_> = (0..10).map(|i| test_line(&format!("line {i}"))).collect();
        state.enqueue(lines);

        let output = run_commit_tick(&mut policy, &mut state, Instant::now());
        // Should drain all 10 lines in CatchUp mode.
        assert_eq!(output.lines.len(), 10);
        assert!(output.is_idle);
    }
}
