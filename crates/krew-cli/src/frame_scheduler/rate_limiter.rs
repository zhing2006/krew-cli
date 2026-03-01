//! Frame rate limiter that clamps draw deadlines to a maximum of 120 FPS.

use std::time::{Duration, Instant};

/// Minimum interval between frames (~120 FPS).
const MIN_FRAME_INTERVAL: Duration = Duration::from_nanos(8_333_334);

pub(super) struct FrameRateLimiter {
    last_emitted_at: Option<Instant>,
}

impl FrameRateLimiter {
    pub fn new() -> Self {
        Self {
            last_emitted_at: None,
        }
    }

    /// Clamp the requested deadline forward if it would exceed the maximum
    /// frame rate.
    pub fn clamp_deadline(&self, requested: Instant) -> Instant {
        match self.last_emitted_at {
            Some(last) => {
                let earliest = last + MIN_FRAME_INTERVAL;
                requested.max(earliest)
            }
            None => requested,
        }
    }

    /// Record that a frame was emitted at the given instant.
    pub fn mark_emitted(&mut self, at: Instant) {
        self.last_emitted_at = Some(at);
    }
}
