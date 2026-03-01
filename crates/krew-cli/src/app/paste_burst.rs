//! Paste-burst detection for terminals without bracketed paste.
//!
//! On some platforms (notably Windows), pastes arrive as a rapid stream of
//! `KeyCode::Char` and `KeyCode::Enter` key events rather than a single
//! `Event::Paste`.  This module provides a timing-based state machine that
//! detects such bursts and buffers them into a single paste string.
//!
//! # How It Works
//!
//! - Plain characters arriving faster than `PASTE_BURST_CHAR_INTERVAL` are
//!   counted.  After `PASTE_BURST_MIN_CHARS` consecutive fast chars the
//!   stream is classified as a paste burst and buffered.
//! - While buffering, Enter inserts a newline instead of submitting.
//! - After an idle timeout the accumulated buffer is flushed as a single
//!   paste event that the caller feeds into `handle_paste`.
//! - For ASCII input, the very first fast char is held briefly (flicker
//!   suppression) so typing a single character does not flash.
//! - Non-ASCII / IME input is never held (would feel like dropped input)
//!   but still participates in burst detection.

use std::time::Duration;
use std::time::Instant;

// Heuristic thresholds.
const PASTE_BURST_MIN_CHARS: u16 = 3;
const PASTE_ENTER_SUPPRESS_WINDOW: Duration = Duration::from_millis(120);

// Maximum delay between consecutive chars to be considered part of a burst.
// Windows terminals deliver paste events more slowly than native terminals.
#[cfg(not(windows))]
const PASTE_BURST_CHAR_INTERVAL: Duration = Duration::from_millis(8);
#[cfg(windows)]
const PASTE_BURST_CHAR_INTERVAL: Duration = Duration::from_millis(30);

// Idle timeout before flushing buffered paste content.
#[cfg(not(windows))]
const PASTE_BURST_ACTIVE_IDLE_TIMEOUT: Duration = Duration::from_millis(8);
#[cfg(windows)]
const PASTE_BURST_ACTIVE_IDLE_TIMEOUT: Duration = Duration::from_millis(60);

#[derive(Default)]
pub(crate) struct PasteBurst {
    last_plain_char_time: Option<Instant>,
    consecutive_plain_char_burst: u16,
    burst_window_until: Option<Instant>,
    buffer: String,
    active: bool,
    /// Hold first fast char briefly to avoid rendering flicker.
    pending_first_char: Option<(char, Instant)>,
}

pub(crate) enum CharDecision {
    /// Start buffering and retroactively capture some already-inserted chars.
    BeginBuffer { retro_chars: u16 },
    /// We are currently buffering; append the current char into the buffer.
    BufferAppend,
    /// Do not insert/render this char yet; temporarily save the first fast
    /// char while we wait to see if a paste-like burst follows.
    RetainFirstChar,
    /// Begin buffering using the previously saved first char (no retro grab).
    BeginBufferFromPending,
}

pub(crate) enum FlushResult {
    Paste(String),
    Typed(char),
    None,
}

impl PasteBurst {
    /// Entry point for ASCII chars: decide how to treat a plain char.
    pub fn on_plain_char(&mut self, ch: char, now: Instant) -> CharDecision {
        self.note_plain_char(now);

        if self.active {
            self.burst_window_until = Some(now + PASTE_ENTER_SUPPRESS_WINDOW);
            return CharDecision::BufferAppend;
        }

        // If we already held a first char and receive a second fast char,
        // start buffering without retro-grabbing.
        if let Some((held, held_at)) = self.pending_first_char
            && now.duration_since(held_at) <= PASTE_BURST_CHAR_INTERVAL
        {
            self.active = true;
            let _ = self.pending_first_char.take();
            self.buffer.push(held);
            self.burst_window_until = Some(now + PASTE_ENTER_SUPPRESS_WINDOW);
            return CharDecision::BeginBufferFromPending;
        }

        if self.consecutive_plain_char_burst >= PASTE_BURST_MIN_CHARS {
            return CharDecision::BeginBuffer {
                retro_chars: self.consecutive_plain_char_burst.saturating_sub(1),
            };
        }

        // Save the first fast char very briefly.
        self.pending_first_char = Some((ch, now));
        CharDecision::RetainFirstChar
    }

    /// Like `on_plain_char`, but never holds the first char.
    /// Used for non-ASCII / IME input paths.
    pub fn on_plain_char_no_hold(&mut self, now: Instant) -> Option<CharDecision> {
        self.note_plain_char(now);

        if self.active {
            self.burst_window_until = Some(now + PASTE_ENTER_SUPPRESS_WINDOW);
            return Some(CharDecision::BufferAppend);
        }

        if self.consecutive_plain_char_burst >= PASTE_BURST_MIN_CHARS {
            return Some(CharDecision::BeginBuffer {
                retro_chars: self.consecutive_plain_char_burst.saturating_sub(1),
            });
        }

        None
    }

    fn note_plain_char(&mut self, now: Instant) {
        match self.last_plain_char_time {
            Some(prev) if now.duration_since(prev) <= PASTE_BURST_CHAR_INTERVAL => {
                self.consecutive_plain_char_burst =
                    self.consecutive_plain_char_burst.saturating_add(1);
            }
            _ => self.consecutive_plain_char_burst = 1,
        }
        self.last_plain_char_time = Some(now);
    }

    /// Flush buffered burst if the inter-key timeout has elapsed.
    pub fn flush_if_due(&mut self, now: Instant) -> FlushResult {
        let timeout = if self.is_active_internal() {
            PASTE_BURST_ACTIVE_IDLE_TIMEOUT
        } else {
            PASTE_BURST_CHAR_INTERVAL
        };
        let timed_out = self
            .last_plain_char_time
            .is_some_and(|t| now.duration_since(t) > timeout);
        if timed_out && self.is_active_internal() {
            self.active = false;
            let out = std::mem::take(&mut self.buffer);
            FlushResult::Paste(out)
        } else if timed_out {
            if let Some((ch, _at)) = self.pending_first_char.take() {
                FlushResult::Typed(ch)
            } else {
                FlushResult::None
            }
        } else {
            FlushResult::None
        }
    }

    /// While bursting: accumulate a newline into the buffer instead of submitting.
    /// Returns true if a newline was appended.
    pub fn append_newline_if_active(&mut self, now: Instant) -> bool {
        if self.is_active() {
            self.buffer.push('\n');
            self.burst_window_until = Some(now + PASTE_ENTER_SUPPRESS_WINDOW);
            true
        } else {
            false
        }
    }

    /// Decide if Enter should insert a newline (burst context) vs submit.
    pub fn newline_should_insert_instead_of_submit(&self, now: Instant) -> bool {
        let in_burst_window = self.burst_window_until.is_some_and(|until| now <= until);
        self.is_active() || in_burst_window
    }

    /// Keep the burst window alive.
    pub fn extend_window(&mut self, now: Instant) {
        self.burst_window_until = Some(now + PASTE_ENTER_SUPPRESS_WINDOW);
    }

    /// Append a char into the burst buffer.
    pub fn append_char_to_buffer(&mut self, ch: char, now: Instant) {
        self.buffer.push(ch);
        self.burst_window_until = Some(now + PASTE_ENTER_SUPPRESS_WINDOW);
    }

    /// Try to append a char if a burst is already active.
    /// Returns true when the char was captured.
    pub fn try_append_char_if_active(&mut self, ch: char, now: Instant) -> bool {
        if self.active || !self.buffer.is_empty() {
            self.append_char_to_buffer(ch, now);
            true
        } else {
            false
        }
    }

    /// Decide whether to begin buffering by retroactively capturing recent
    /// chars from the slice before the cursor.
    pub fn decide_begin_buffer(
        &mut self,
        now: Instant,
        before: &str,
        retro_chars: usize,
    ) -> Option<RetroGrab> {
        let start_byte = retro_start_index(before, retro_chars);
        let grabbed = before[start_byte..].to_string();
        let looks_pastey =
            grabbed.chars().any(char::is_whitespace) || grabbed.chars().count() >= 16;
        if looks_pastey {
            self.begin_with_retro_grabbed(grabbed.clone(), now);
            Some(RetroGrab {
                start_byte,
                grabbed,
            })
        } else {
            None
        }
    }

    /// Before applying modified/non-char input: flush buffered burst immediately.
    pub fn flush_before_modified_input(&mut self) -> Option<String> {
        if !self.is_active() {
            return None;
        }
        self.active = false;
        let mut out = std::mem::take(&mut self.buffer);
        if let Some((ch, _at)) = self.pending_first_char.take() {
            out.push(ch);
        }
        Some(out)
    }

    /// Clear only the timing window and any pending first-char.
    pub fn clear_window_after_non_char(&mut self) {
        self.consecutive_plain_char_burst = 0;
        self.last_plain_char_time = None;
        self.burst_window_until = None;
        self.active = false;
        self.pending_first_char = None;
    }

    /// Reset all state after an explicit `Event::Paste`.
    pub fn clear_after_explicit_paste(&mut self) {
        self.last_plain_char_time = None;
        self.consecutive_plain_char_burst = 0;
        self.burst_window_until = None;
        self.active = false;
        self.buffer.clear();
        self.pending_first_char = None;
    }

    /// Returns true if in any paste-burst transient state.
    pub fn is_active(&self) -> bool {
        self.is_active_internal() || self.pending_first_char.is_some()
    }

    fn is_active_internal(&self) -> bool {
        self.active || !self.buffer.is_empty()
    }

    /// Start buffering without retro-grabbing any already-inserted text.
    /// Used when the burst heuristic detects paste-like input speed but the
    /// retro-grab check rejects the already-inserted prefix (e.g. short
    /// CJK text without whitespace).
    pub fn force_start_buffer(&mut self, now: Instant) {
        self.active = true;
        self.burst_window_until = Some(now + PASTE_ENTER_SUPPRESS_WINDOW);
    }

    fn begin_with_retro_grabbed(&mut self, grabbed: String, now: Instant) {
        if !grabbed.is_empty() {
            self.buffer.push_str(&grabbed);
        }
        self.active = true;
        self.burst_window_until = Some(now + PASTE_ENTER_SUPPRESS_WINDOW);
    }
}

pub(crate) struct RetroGrab {
    #[allow(dead_code)]
    pub start_byte: usize,
    #[allow(dead_code)]
    pub grabbed: String,
}

fn retro_start_index(before: &str, retro_chars: usize) -> usize {
    if retro_chars == 0 {
        return before.len();
    }
    before
        .char_indices()
        .rev()
        .nth(retro_chars.saturating_sub(1))
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}
