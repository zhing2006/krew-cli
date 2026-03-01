//! Adaptive chunking policy with hysteresis-based queue pressure management.
//!
//! Two-gear system: Smooth (1 line/tick for typing effect) and CatchUp
//! (drain all for throughput). Hysteresis prevents mode-flapping.

use std::time::{Duration, Instant};

// --- Thresholds (matching codex values) ---

/// Enter CatchUp when queue depth reaches this.
const ENTER_QUEUE_DEPTH: usize = 8;
/// Enter CatchUp when oldest line is this old.
const ENTER_OLDEST_AGE: Duration = Duration::from_millis(120);

/// Exit CatchUp only when queue depth drops to this.
const EXIT_QUEUE_DEPTH: usize = 2;
/// Exit CatchUp only when oldest line is younger than this.
const EXIT_OLDEST_AGE: Duration = Duration::from_millis(40);

/// Must stay below exit thresholds for this long before actually exiting.
const EXIT_HOLD: Duration = Duration::from_millis(250);

/// After exiting CatchUp, don't re-enter for this long.
const REENTER_HOLD: Duration = Duration::from_millis(250);

/// Severe backlog: bypasses re-enter hold (queue depth).
const SEVERE_QUEUE_DEPTH: usize = 64;
/// Severe backlog: bypasses re-enter hold (oldest age).
const SEVERE_OLDEST_AGE: Duration = Duration::from_millis(300);

/// Current chunking mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkingMode {
    /// Drain 1 line per tick (smooth typing effect).
    Smooth,
    /// Drain all queued lines per tick (catching up to live).
    CatchUp,
}

/// Adaptive chunking policy with hysteresis.
pub struct AdaptiveChunkingPolicy {
    mode: ChunkingMode,
    /// When we first observed below-exit-threshold conditions.
    below_exit_threshold_since: Option<Instant>,
    /// When we last exited CatchUp mode.
    last_catch_up_exit_at: Option<Instant>,
}

/// Result of a chunking decision.
pub struct ChunkingDecision {
    /// How many lines to drain this tick.
    pub drain_count: DrainPlan,
}

/// How many lines to drain.
pub enum DrainPlan {
    /// Drain exactly 1 line.
    Single,
    /// Drain up to this many lines (0 = all).
    Batch(usize),
}

/// Snapshot of current queue pressure.
pub struct QueueSnapshot {
    pub queued_lines: usize,
    pub oldest_age: Option<Duration>,
}

impl AdaptiveChunkingPolicy {
    pub fn new() -> Self {
        Self {
            mode: ChunkingMode::Smooth,
            below_exit_threshold_since: None,
            last_catch_up_exit_at: None,
        }
    }

    /// Make a chunking decision based on current queue state.
    pub fn decide(&mut self, snapshot: &QueueSnapshot, now: Instant) -> ChunkingDecision {
        // Empty queue → reset to Smooth.
        if snapshot.queued_lines == 0 {
            self.reset_to_smooth();
            return ChunkingDecision {
                drain_count: DrainPlan::Single,
            };
        }

        match self.mode {
            ChunkingMode::Smooth => {
                if self.should_enter_catch_up(snapshot, now) {
                    self.mode = ChunkingMode::Smooth; // will be set below
                    self.enter_catch_up(snapshot)
                } else {
                    ChunkingDecision {
                        drain_count: DrainPlan::Single,
                    }
                }
            }
            ChunkingMode::CatchUp => {
                if self.should_exit_catch_up(snapshot, now) {
                    self.exit_catch_up(now);
                    ChunkingDecision {
                        drain_count: DrainPlan::Single,
                    }
                } else {
                    ChunkingDecision {
                        drain_count: DrainPlan::Batch(snapshot.queued_lines),
                    }
                }
            }
        }
    }

    /// Reset all state to Smooth mode.
    pub fn reset(&mut self) {
        self.reset_to_smooth();
        self.last_catch_up_exit_at = None;
    }

    fn reset_to_smooth(&mut self) {
        self.mode = ChunkingMode::Smooth;
        self.below_exit_threshold_since = None;
    }

    fn should_enter_catch_up(&self, snapshot: &QueueSnapshot, now: Instant) -> bool {
        let depth_trigger = snapshot.queued_lines >= ENTER_QUEUE_DEPTH;
        let age_trigger = snapshot
            .oldest_age
            .is_some_and(|age| age >= ENTER_OLDEST_AGE);

        if !depth_trigger && !age_trigger {
            return false;
        }

        // Check re-entry hold (cooldown after last exit).
        if let Some(exit_at) = self.last_catch_up_exit_at {
            let since_exit = now.saturating_duration_since(exit_at);
            if since_exit < REENTER_HOLD {
                // Check severe backlog escape hatch.
                let severe = snapshot.queued_lines >= SEVERE_QUEUE_DEPTH
                    || snapshot
                        .oldest_age
                        .is_some_and(|age| age >= SEVERE_OLDEST_AGE);
                return severe;
            }
        }

        true
    }

    fn enter_catch_up(&mut self, snapshot: &QueueSnapshot) -> ChunkingDecision {
        self.mode = ChunkingMode::CatchUp;
        self.below_exit_threshold_since = None;
        ChunkingDecision {
            drain_count: DrainPlan::Batch(snapshot.queued_lines),
        }
    }

    fn should_exit_catch_up(&mut self, snapshot: &QueueSnapshot, now: Instant) -> bool {
        let below_depth = snapshot.queued_lines <= EXIT_QUEUE_DEPTH;
        let below_age = snapshot.oldest_age.is_none_or(|age| age <= EXIT_OLDEST_AGE);

        if below_depth && below_age {
            // Start or continue the exit hold timer.
            let since = *self.below_exit_threshold_since.get_or_insert(now);
            now.saturating_duration_since(since) >= EXIT_HOLD
        } else {
            // Reset the exit hold timer.
            self.below_exit_threshold_since = None;
            false
        }
    }

    fn exit_catch_up(&mut self, now: Instant) {
        self.mode = ChunkingMode::Smooth;
        self.below_exit_threshold_since = None;
        self.last_catch_up_exit_at = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(lines: usize, age_ms: u64) -> QueueSnapshot {
        QueueSnapshot {
            queued_lines: lines,
            oldest_age: if lines > 0 {
                Some(Duration::from_millis(age_ms))
            } else {
                None
            },
        }
    }

    #[test]
    fn starts_in_smooth() {
        let policy = AdaptiveChunkingPolicy::new();
        assert_eq!(policy.mode, ChunkingMode::Smooth);
    }

    #[test]
    fn stays_smooth_under_threshold() {
        let mut policy = AdaptiveChunkingPolicy::new();
        let now = Instant::now();
        let _decision = policy.decide(&snapshot(3, 50), now);
        assert_eq!(policy.mode, ChunkingMode::Smooth);
    }

    #[test]
    fn enters_catch_up_on_depth() {
        let mut policy = AdaptiveChunkingPolicy::new();
        let now = Instant::now();
        let _decision = policy.decide(&snapshot(10, 50), now);
        assert_eq!(policy.mode, ChunkingMode::CatchUp);
    }

    #[test]
    fn enters_catch_up_on_age() {
        let mut policy = AdaptiveChunkingPolicy::new();
        let now = Instant::now();
        let _decision = policy.decide(&snapshot(3, 150), now);
        assert_eq!(policy.mode, ChunkingMode::CatchUp);
    }

    #[test]
    fn empty_queue_resets_to_smooth() {
        let mut policy = AdaptiveChunkingPolicy::new();
        let now = Instant::now();

        // Enter CatchUp.
        policy.decide(&snapshot(10, 50), now);
        assert_eq!(policy.mode, ChunkingMode::CatchUp);

        // Empty queue → reset.
        policy.decide(&snapshot(0, 0), now);
        assert_eq!(policy.mode, ChunkingMode::Smooth);
    }

    #[test]
    fn severe_backlog_bypasses_reenter_hold() {
        let mut policy = AdaptiveChunkingPolicy::new();
        let now = Instant::now();

        // Enter and immediately exit CatchUp.
        policy.mode = ChunkingMode::CatchUp;
        policy.last_catch_up_exit_at = Some(now);
        policy.mode = ChunkingMode::Smooth;

        // Normal trigger should be blocked by re-enter hold.
        policy.decide(&snapshot(10, 50), now);
        assert_eq!(policy.mode, ChunkingMode::Smooth);

        // Severe backlog should bypass.
        policy.decide(&snapshot(70, 50), now);
        assert_eq!(policy.mode, ChunkingMode::CatchUp);
    }
}
