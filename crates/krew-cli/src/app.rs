//! App state machine and main event loop.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use ratatui_textarea::{Input, Key, TextArea};

use krew_config::Config;

use crate::custom_terminal;
use crate::render;

/// Duration within which a second Ctrl+C triggers quit.
const QUIT_SHORTCUT_TIMEOUT: Duration = Duration::from_secs(1);

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
    quit_shortcut_armed_at: Option<Instant>,
    /// Transient hint shown in the status bar.
    pub quit_hint: Option<String>,
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

        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(ratatui::style::Style::default());
        textarea.set_cursor_style(
            ratatui::style::Style::default()
                .fg(ratatui::style::Color::White)
                .add_modifier(ratatui::style::Modifier::REVERSED),
        );

        Ok(Self {
            cwd,
            config,
            project_instructions,
            textarea,
            should_quit: false,
            quit_shortcut_armed_at: None,
            quit_hint: None,
        })
    }

    /// Run the main event loop.
    pub async fn run(&mut self, terminal: &mut custom_terminal::Terminal) -> anyhow::Result<()> {
        // Print the header above the viewport (scrolls into scrollback).
        render::insert_header(terminal, self)?;

        let mut event_stream = EventStream::new();

        loop {
            // Adjust viewport height to fit the current textarea content.
            let input_lines = self.textarea.lines().len() as u16;
            let needed = input_lines.max(1) + 3; // separators (2) + status bar (1)
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
                // Tick so the quit hint can expire.
                _ = tokio::time::sleep(Duration::from_millis(100)) => {}
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
            Event::Resize(..) => {}
            _ => {}
        }
        Ok(())
    }

    /// Handle a key press event.
    fn handle_key(
        &mut self,
        key_event: KeyEvent,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
        // Ctrl+C: double-press to quit.
        if key_event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key_event.code, KeyCode::Char('c') | KeyCode::Char('C'))
        {
            self.on_ctrl_c();
            return Ok(());
        }

        // Clear any quit hint on other key presses.
        if self.quit_hint.is_some() {
            self.quit_shortcut_armed_at = None;
            self.quit_hint = None;
        }

        let input: Input = key_event.into();
        match input {
            // Enter (no modifiers) => send message.
            Input {
                key: Key::Enter,
                shift: false,
                ctrl: false,
                alt: false,
            } => {
                self.send_message(terminal)?;
            }
            // Shift+Enter => insert newline.
            Input {
                key: Key::Enter,
                shift: true,
                ..
            } => {
                self.textarea.insert_newline();
            }
            // Ctrl+J => insert newline (fallback for terminals without
            // keyboard enhancement where Shift+Enter is indistinguishable
            // from Enter).
            Input {
                key: Key::Char('j'),
                ctrl: true,
                shift: false,
                alt: false,
            } => {
                self.textarea.insert_newline();
            }
            // All other keys => forward to textarea.
            other => {
                self.textarea.input(other);
            }
        }
        Ok(())
    }

    /// Handle Ctrl+C press with double-press detection.
    fn on_ctrl_c(&mut self) {
        if let Some(armed_at) = self.quit_shortcut_armed_at
            && armed_at.elapsed() < QUIT_SHORTCUT_TIMEOUT
        {
            self.should_quit = true;
            return;
        }

        self.quit_shortcut_armed_at = Some(Instant::now());
        self.quit_hint = Some("Press Ctrl+C again to quit".to_string());
    }

    /// Send the current input as a message and produce an echo reply.
    fn send_message(&mut self, terminal: &mut custom_terminal::Terminal) -> anyhow::Result<()> {
        let text = self.textarea.lines().join("\n");

        if text.trim().is_empty() {
            return Ok(());
        }

        let trimmed = text.trim();
        if trimmed == "/quit" || trimmed == "/exit" {
            self.should_quit = true;
            return Ok(());
        }

        tracing::debug!(input = %text, "User sent message");

        // Insert the user message + echo reply above the viewport.
        render::insert_message(terminal, "you", &text, "")?;
        let echo_color = self
            .config
            .agents
            .first()
            .map(|a| a.color.as_str())
            .unwrap_or("yellow");
        render::insert_message(terminal, "echo", &text, echo_color)?;

        // Clear the textarea and restore styles.
        self.textarea = TextArea::default();
        self.textarea
            .set_cursor_line_style(ratatui::style::Style::default());
        self.textarea.set_cursor_style(
            ratatui::style::Style::default()
                .fg(ratatui::style::Color::White)
                .add_modifier(ratatui::style::Modifier::REVERSED),
        );

        Ok(())
    }
}
