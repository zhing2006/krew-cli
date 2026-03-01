//! Frame scheduler actor that coalesces multiple draw requests into a single
//! frame, rate-limited to 120 FPS.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Notify;
use tokio::sync::mpsc;

use super::rate_limiter::FrameRateLimiter;

/// Public handle for scheduling frames. Cheap to clone.
#[derive(Clone)]
pub struct FrameRequester {
    tx: mpsc::UnboundedSender<Instant>,
}

impl FrameRequester {
    /// Spawn the frame scheduler background task and return the requester
    /// handle.
    pub fn spawn(draw_signal: Arc<Notify>) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let scheduler = FrameScheduler {
            rx,
            draw_signal,
            rate_limiter: FrameRateLimiter::new(),
        };
        tokio::spawn(scheduler.run());
        Self { tx }
    }

    /// Request a frame to be drawn as soon as possible (subject to rate
    /// limiting).
    pub fn schedule_frame(&self) {
        let _ = self.tx.send(Instant::now());
    }

    /// Request a frame to be drawn after the given delay.
    pub fn schedule_frame_in(&self, dur: Duration) {
        let _ = self.tx.send(Instant::now() + dur);
    }
}

/// Internal actor that receives deadline requests and notifies the main loop
/// when it is time to draw.
struct FrameScheduler {
    rx: mpsc::UnboundedReceiver<Instant>,
    draw_signal: Arc<Notify>,
    rate_limiter: FrameRateLimiter,
}

impl FrameScheduler {
    async fn run(mut self) {
        // Wait for the first scheduling request.
        let Some(first) = self.rx.recv().await else {
            return;
        };
        let mut deadline = self.rate_limiter.clamp_deadline(first);

        loop {
            tokio::select! {
                // A new scheduling request arrived — coalesce by picking the
                // earliest deadline.
                maybe_req = self.rx.recv() => {
                    match maybe_req {
                        Some(requested) => {
                            let clamped = self.rate_limiter.clamp_deadline(requested);
                            deadline = deadline.min(clamped);
                        }
                        None => break, // Channel closed, shut down.
                    }
                }
                // Deadline fired — notify the main loop to draw.
                _ = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)) => {
                    let now = Instant::now();
                    self.rate_limiter.mark_emitted(now);
                    self.draw_signal.notify_one();

                    // Wait for the next scheduling request before looping.
                    match self.rx.recv().await {
                        Some(requested) => {
                            deadline = self.rate_limiter.clamp_deadline(requested);
                        }
                        None => break,
                    }
                }
            }
        }
    }
}
