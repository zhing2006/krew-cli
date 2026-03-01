//! App state machine and main event loop.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{Event, EventStream, KeyEventKind};
use futures::StreamExt;
use tokio::sync::Notify;

use krew_config::Config;

use crate::completion::ActivePopup;
use crate::custom_terminal;
use crate::frame_scheduler::FrameRequester;
use crate::render;
use crate::textarea::TextArea;

use super::paste_burst::{FlushResult, PasteBurst};

/// Duration within which a second Ctrl+C triggers quit.
pub(super) const QUIT_SHORTCUT_TIMEOUT: Duration = Duration::from_secs(1);

/// Top-level application state.
pub struct App {
    /// Current working directory for the session.
    pub cwd: PathBuf,
    /// Loaded configuration.
    pub config: Config,
    /// Project-level instructions loaded from AGENTS.md files (if any).
    #[allow(dead_code)]
    pub project_instructions: Option<String>,
    /// Multi-line text input component.
    pub textarea: TextArea,
    /// Whether the app should quit.
    pub should_quit: bool,
    /// Timestamp when the first Ctrl+C was pressed (for double-press detection).
    pub(crate) quit_shortcut_armed_at: Option<Instant>,
    /// Transient hint shown in the status bar.
    pub quit_hint: Option<String>,
    /// Active completion popup state.
    pub popup: ActivePopup,
    /// Input history (most recent last).
    pub(crate) history: Vec<String>,
    /// Current position in history navigation (None = not browsing).
    pub(crate) history_index: Option<usize>,
    /// Draft input saved when entering history navigation.
    pub(crate) history_draft: String,
    /// Non-bracketed paste burst tracker for Windows fallback.
    pub(crate) paste_burst: PasteBurst,
    /// Stored full text for large paste placeholders (element_id → actual text).
    pub(crate) pending_pastes: HashMap<u64, String>,
    /// Counter for paste placeholder display numbering.
    pub(crate) paste_counter: usize,
    /// Frame scheduler handle for coalesced rendering.
    pub(crate) frame_requester: Option<FrameRequester>,
}

impl App {
    /// Initialize the application with the given config and working directory.
    pub fn new(cwd: PathBuf, config: Config) -> anyhow::Result<Self> {
        let project_instructions = match krew_config::load_project_instructions(&cwd) {
            Ok(instructions) => instructions,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to load project instructions");
                None
            }
        };

        Ok(Self {
            cwd,
            config,
            project_instructions,
            textarea: TextArea::new(),
            should_quit: false,
            quit_shortcut_armed_at: None,
            quit_hint: None,
            popup: ActivePopup::None,
            history: Vec::new(),
            history_index: None,
            history_draft: String::new(),
            paste_burst: PasteBurst::default(),
            pending_pastes: HashMap::new(),
            paste_counter: 0,
            frame_requester: None,
        })
    }

    /// Run the main event loop.
    pub async fn run(&mut self, terminal: &mut custom_terminal::Terminal) -> anyhow::Result<()> {
        // Print the header above the viewport (scrolls into scrollback).
        render::insert_header(terminal, self)?;

        // Set up the frame scheduler for coalesced rendering (max 120 FPS).
        let draw_signal = Arc::new(Notify::new());
        let frame_requester = FrameRequester::spawn(Arc::clone(&draw_signal));
        self.frame_requester = Some(frame_requester);

        // Schedule the initial frame.
        self.request_redraw();

        let mut event_stream = EventStream::new();

        loop {
            tokio::select! {
                // Branch 1: Terminal events (key, paste, resize).
                maybe_event = event_stream.next() => {
                    match maybe_event {
                        Some(Ok(event)) => {
                            self.handle_event(event, terminal)?;
                            self.request_redraw();
                        }
                        Some(Err(e)) => {
                            tracing::error!(error = %e, "Terminal event stream error");
                            break;
                        }
                        None => break,
                    }
                }
                // Branch 2: Draw frame (coalesced by scheduler, max 120 FPS).
                _ = draw_signal.notified() => {
                    // Skip render during active paste burst.
                    if self.handle_paste_burst_tick() {
                        continue;
                    }

                    // Check if quit hint has expired.
                    if let Some(armed_at) = self.quit_shortcut_armed_at
                        && armed_at.elapsed() >= QUIT_SHORTCUT_TIMEOUT
                    {
                        self.quit_shortcut_armed_at = None;
                        self.quit_hint = None;
                    }

                    // Sync completion popup based on current input.
                    self.sync_popup();

                    // Adjust viewport height to fit textarea + popup.
                    let term_width = terminal.size()?.width.saturating_sub(2);
                    let input_lines = self.textarea.desired_height(term_width.max(1));
                    let needed = input_lines.max(1) + 3 + self.popup.extra_height();
                    terminal.ensure_viewport_height(needed)?;

                    // Render input prompt + status bar inside the inline viewport.
                    terminal.draw(|frame| render::render_input_viewport(frame, self))?;
                }
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    /// Request a redraw via the frame scheduler.
    fn request_redraw(&self) {
        if let Some(fr) = &self.frame_requester {
            fr.schedule_frame();
        }
    }

    /// Handle paste burst tick during draw. Returns true to skip rendering.
    fn handle_paste_burst_tick(&mut self) -> bool {
        if !self.config.settings.paste_burst_detection {
            return false;
        }
        // Try flushing timed-out burst.
        let flushed = match self.paste_burst.flush_if_due(Instant::now()) {
            FlushResult::Paste(p) => {
                self.handle_paste(p);
                true
            }
            FlushResult::Typed(c) => {
                self.textarea.insert_str(c.to_string().as_str());
                true
            }
            FlushResult::None => false,
        };
        if flushed {
            self.request_redraw();
            return true;
        }
        if self.paste_burst.is_active() {
            // Still buffering — schedule follow-up tick, skip render.
            if let Some(fr) = &self.frame_requester {
                fr.schedule_frame_in(self.paste_burst.recommended_flush_delay());
            }
            return true;
        }
        false
    }

    /// Handle a single terminal event.
    fn handle_event(
        &mut self,
        event: Event,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
        match event {
            Event::Key(key_event) => {
                if key_event.kind != KeyEventKind::Press {
                    return Ok(());
                }
                self.handle_key(key_event, terminal)?;
            }
            Event::Paste(text) => {
                // Receiving Event::Paste means the terminal supports
                // bracketed paste — auto-disable burst detection.
                if self.config.settings.paste_burst_detection {
                    tracing::info!("Bracketed paste detected, disabling paste burst detection");
                    self.config.settings.paste_burst_detection = false;
                }
                self.handle_paste(text);
            }
            Event::Resize(..) => {}
            _ => {}
        }
        Ok(())
    }

    /// Threshold in chars above which pasted text is collapsed into a
    /// placeholder element.
    const PASTE_PLACEHOLDER_THRESHOLD: usize = 100;

    /// Handle a paste event (bracketed paste or burst-detected paste) —
    /// insert text into the textarea without triggering auto-send on newlines.
    pub(crate) fn handle_paste(&mut self, text: String) {
        self.paste_burst.clear_after_explicit_paste();
        let text = text.replace("\r\n", "\n").replace('\r', "\n");

        if text.chars().count() > Self::PASTE_PLACEHOLDER_THRESHOLD {
            self.paste_counter += 1;
            let n = self.paste_counter;
            let char_count = text.chars().count();
            let placeholder = format!("[Pasted text #{n} ({char_count} chars)]");
            let elem_id = self.textarea.insert_element(&placeholder);
            self.pending_pastes.insert(elem_id, text);
        } else {
            self.textarea.insert_str(&text);
        }
    }

    /// Clear the textarea and any pending paste placeholders.
    pub(crate) fn clear_textarea(&mut self) {
        self.textarea = TextArea::new();
        self.history_index = None;
        self.pending_pastes.clear();
        self.paste_counter = 0;
    }

    /// Return the textarea text with paste placeholders expanded to
    /// their actual pasted content.
    pub(crate) fn expanded_text(&self) -> String {
        if self.pending_pastes.is_empty() {
            return self.textarea.text().to_string();
        }

        let mut result = self.textarea.text().to_string();
        // Expand in reverse order of element position so that earlier
        // replacements don't shift later byte ranges.
        let mut replacements: Vec<_> = self
            .textarea
            .elements_snapshot()
            .into_iter()
            .filter_map(|(id, range)| {
                self.pending_pastes
                    .get(&id)
                    .map(|real| (range, real.clone()))
            })
            .collect();
        replacements.sort_by(|a, b| b.0.start.cmp(&a.0.start));

        for (range, real_text) in replacements {
            if range.end <= result.len() {
                result.replace_range(range, &real_text);
            }
        }
        result
    }

    /// Replace textarea content with the given text (supports multiline).
    pub(crate) fn set_textarea_content(&mut self, content: &str) {
        self.textarea.set_text_clearing_elements(content);
        self.textarea.set_cursor(content.len());
    }
}
