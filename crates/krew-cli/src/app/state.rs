//! App state machine and main event loop.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{Event, EventStream, KeyEventKind};
use futures::StreamExt;
use ratatui::style::{Color, Modifier, Style};
use ratatui_textarea::TextArea;

use krew_config::Config;

use crate::completion::ActivePopup;
use crate::custom_terminal;
use crate::render;

use super::paste_burst::{FlushResult, PasteBurst};

/// Duration within which a second Ctrl+C triggers quit.
pub(super) const QUIT_SHORTCUT_TIMEOUT: Duration = Duration::from_secs(1);

/// Top-level application state.
pub struct App<'a> {
    /// Current working directory for the session.
    pub cwd: PathBuf,
    /// Loaded configuration.
    pub config: Config,
    /// Project-level instructions loaded from AGENTS.md files (if any).
    #[allow(dead_code)]
    pub project_instructions: Option<String>,
    /// Multi-line text input component.
    pub textarea: TextArea<'a>,
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
}

impl<'a> App<'a> {
    /// Initialize the application with the given config and working directory.
    pub fn new(cwd: PathBuf, config: Config) -> anyhow::Result<Self> {
        let project_instructions = match krew_config::load_project_instructions(&cwd) {
            Ok(instructions) => instructions,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to load project instructions");
                None
            }
        };

        let textarea = Self::new_textarea();

        Ok(Self {
            cwd,
            config,
            project_instructions,
            textarea,
            should_quit: false,
            quit_shortcut_armed_at: None,
            quit_hint: None,
            popup: ActivePopup::None,
            history: Vec::new(),
            history_index: None,
            history_draft: String::new(),
            paste_burst: PasteBurst::default(),
        })
    }

    /// Create a fresh TextArea with default styling.
    pub(crate) fn new_textarea() -> TextArea<'a> {
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(Style::default());
        textarea.set_cursor_style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::REVERSED),
        );
        textarea
    }

    /// Run the main event loop.
    pub async fn run(&mut self, terminal: &mut custom_terminal::Terminal) -> anyhow::Result<()> {
        // Print the header above the viewport (scrolls into scrollback).
        render::insert_header(terminal, self)?;

        let mut event_stream = EventStream::new();

        loop {
            // Sync completion popup based on current input.
            self.sync_popup();

            // Adjust viewport height to fit textarea + popup.
            let input_lines = self.textarea.lines().len() as u16;
            let needed = input_lines.max(1) + 3 + self.popup.extra_height();
            // separators (2) + status bar or popup bottom row (1) + extra popup rows
            terminal.ensure_viewport_height(needed)?;

            // Render input prompt + status bar inside the inline viewport.
            terminal.draw(|frame| render::render_input_viewport(frame, self))?;

            // Check if quit hint has expired.
            if let Some(armed_at) = self.quit_shortcut_armed_at
                && armed_at.elapsed() >= QUIT_SHORTCUT_TIMEOUT
            {
                self.quit_shortcut_armed_at = None;
                self.quit_hint = None;
            }

            tokio::select! {
                maybe_event = event_stream.next() => {
                    match maybe_event {
                        Some(Ok(event)) => self.handle_event(event, terminal)?,
                        Some(Err(e)) => {
                            tracing::error!(error = %e, "Terminal event stream error");
                            break;
                        }
                        None => break,
                    }
                }
                // Tick for quit hint expiry and paste burst flush.
                _ = tokio::time::sleep(Duration::from_millis(16)) => {
                    self.flush_paste_burst();
                }
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
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
                self.handle_paste(text);
            }
            Event::Resize(..) => {}
            _ => {}
        }
        Ok(())
    }

    /// Handle a paste event (bracketed paste or burst-detected paste) —
    /// insert text into the textarea without triggering auto-send on newlines.
    pub(crate) fn handle_paste(&mut self, text: String) {
        self.paste_burst.clear_after_explicit_paste();
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let mut lines = text.split('\n');
        if let Some(first) = lines.next() {
            self.textarea.insert_str(first);
        }
        for line in lines {
            self.textarea.insert_newline();
            self.textarea.insert_str(line);
        }
    }

    /// Flush any pending paste burst that has timed out.
    fn flush_paste_burst(&mut self) {
        match self.paste_burst.flush_if_due(Instant::now()) {
            FlushResult::Paste(pasted) => {
                self.handle_paste(pasted);
            }
            FlushResult::Typed(ch) => {
                self.textarea.insert_str(ch.to_string().as_str());
            }
            FlushResult::None => {}
        }
    }

    /// Clear the textarea and restore default styles.
    pub(crate) fn clear_textarea(&mut self) {
        self.textarea = Self::new_textarea();
        self.history_index = None;
    }

    /// Replace textarea content with the given text (supports multiline).
    pub(crate) fn set_textarea_content(&mut self, content: &str) {
        let lines: Vec<String> = if content.is_empty() {
            vec![String::new()]
        } else {
            content.lines().map(String::from).collect()
        };
        let mut textarea = TextArea::new(lines);
        textarea.set_cursor_line_style(Style::default());
        textarea.set_cursor_style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::REVERSED),
        );
        // Move cursor to end.
        textarea.move_cursor(ratatui_textarea::CursorMove::Bottom);
        textarea.move_cursor(ratatui_textarea::CursorMove::End);
        self.textarea = textarea;
    }
}
