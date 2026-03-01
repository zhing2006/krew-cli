//! App state machine and main event loop.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui_textarea::{Input, Key, TextArea};

use krew_config::Config;
use krew_core::command::SlashCommand;
use krew_core::router::{self, Addressee};

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

        let textarea = Self::new_textarea();

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

    /// Create a fresh TextArea with default styling.
    fn new_textarea() -> TextArea<'a> {
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

    /// Send the current input as a message or execute a slash command.
    fn send_message(&mut self, terminal: &mut custom_terminal::Terminal) -> anyhow::Result<()> {
        let text = self.textarea.lines().join("\n");

        if text.trim().is_empty() {
            return Ok(());
        }

        let trimmed = text.trim();
        tracing::debug!(input = %trimmed, "User sent message");

        // Try slash command first.
        if trimmed.starts_with('/') {
            self.clear_textarea();
            return self.execute_slash_command(trimmed, terminal);
        }

        // Parse @ addressee (only known agents are recognized as addressees).
        let agent_names: Vec<String> = self.config.agents.iter().map(|a| a.name.clone()).collect();
        let (addressee, body) = match router::parse_input(trimmed, &agent_names) {
            Ok(result) => result,
            Err(e) => {
                self.show_error(terminal, &e.to_string())?;
                self.clear_textarea();
                return Ok(());
            }
        };

        // Resolve target agent names for colored dots on user message.
        let target_names: Vec<&str> = match &addressee {
            Addressee::All => self.config.agents.iter().map(|a| a.name.as_str()).collect(),
            Addressee::Single(name) => vec![name.as_str()],
            Addressee::Multiple(names) => names.iter().map(|n| n.as_str()).collect(),
            Addressee::LastRespondent => vec![],
        };

        // Insert user message with colored routing dots: > ●●● message
        self.insert_user_message(terminal, &target_names, trimmed)?;

        // Build route tag for echo display.
        let route_tag = match &addressee {
            Addressee::All => "[→ @all]".to_string(),
            Addressee::Single(name) => format!("[→ @{name}]"),
            Addressee::Multiple(names) => {
                let joined = names.iter().map(|n| format!("@{n}")).collect::<Vec<_>>();
                format!("[→ {}]", joined.join(" "))
            }
            Addressee::LastRespondent => "[→ last]".to_string(),
        };

        // Echo reply with yellow diamond prefix (temporary, replaced by LLM in Phase 4).
        let echo_text = format!("{route_tag} echo: {body}");
        render::insert_system_message(
            terminal,
            vec![Line::from(vec![
                Span::styled(
                    "\u{25c6} ".to_string(), // ◆
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(echo_text),
            ])],
        )?;

        self.clear_textarea();
        Ok(())
    }

    /// Insert user message with colored routing dots showing target agents.
    ///
    /// - Single agent: `> ● message` in agent's color
    /// - Multiple/all agents: `> ●●● message` each dot in its agent's color
    /// - No target (LastRespondent): `> message` (plain, no indicator)
    fn insert_user_message(
        &self,
        terminal: &mut custom_terminal::Terminal,
        target_names: &[&str],
        text: &str,
    ) -> anyhow::Result<()> {
        let green_bold = Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD);

        let mut spans: Vec<Span<'static>> = vec![Span::styled("> ".to_string(), green_bold)];

        if !target_names.is_empty() {
            // Colored dots for each target agent.
            for name in target_names {
                let color = self
                    .config
                    .agents
                    .iter()
                    .find(|a| a.name == *name)
                    .map(|a| render::parse_color(&a.color))
                    .unwrap_or(Color::White);
                spans.push(Span::styled(
                    "\u{25cf}".to_string(), // ●
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ));
            }
            spans.push(Span::raw(" ".to_string()));
        }

        spans.push(Span::raw(text.to_string()));
        render::insert_system_message(terminal, vec![Line::from(spans)])
    }

    /// Execute a slash command.
    fn execute_slash_command(
        &mut self,
        input: &str,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
        let Some(cmd) = SlashCommand::from_input(input) else {
            return self.show_error(terminal, &format!("Unknown command: {input}"));
        };

        match cmd {
            SlashCommand::Quit => {
                self.should_quit = true;
            }
            SlashCommand::Help => {
                self.execute_help(terminal)?;
            }
            SlashCommand::Agents => {
                self.execute_agents(terminal)?;
            }
            SlashCommand::Clear => {
                self.execute_clear(terminal)?;
            }
            SlashCommand::New | SlashCommand::Resume | SlashCommand::Compact(_) => {
                self.show_info(terminal, &format!("{} — not yet implemented", cmd.name()))?;
            }
        }
        Ok(())
    }

    /// Execute /help: display all available commands.
    fn execute_help(&self, terminal: &mut custom_terminal::Terminal) -> anyhow::Result<()> {
        let mut lines: Vec<Line<'static>> = vec![Line::from(Span::styled(
            "Available commands:",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ))];

        for &(name, desc) in SlashCommand::all_help() {
            lines.push(Line::from(vec![
                Span::styled(format!("  {name:<12}"), Style::default().fg(Color::Cyan)),
                Span::styled(desc.to_string(), Style::default().fg(Color::DarkGray)),
            ]));
        }

        render::insert_system_message(terminal, lines)
    }

    /// Execute /agents: display agent list with token stats.
    fn execute_agents(&self, terminal: &mut custom_terminal::Terminal) -> anyhow::Result<()> {
        let mut lines: Vec<Line<'static>> = vec![Line::from(Span::styled(
            "Agents:",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ))];

        for agent in &self.config.agents {
            let color = render::parse_color(&agent.color);
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!("[{}]", agent.name),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(
                    "  {:<16} {}/{}",
                    agent.display_name, agent.provider, agent.model
                )),
                Span::styled("  0 tokens", Style::default().fg(Color::DarkGray)),
            ]));
        }

        render::insert_system_message(terminal, lines)
    }

    /// Execute /clear: clear visible content and re-display header.
    fn execute_clear(&self, terminal: &mut custom_terminal::Terminal) -> anyhow::Result<()> {
        terminal.clear()?;
        // Reset viewport to the top so insert_before has space to render.
        let size = terminal.size()?;
        terminal.set_viewport_area(ratatui::layout::Rect::new(0, 0, size.width, 0));
        render::insert_header(terminal, self)?;
        Ok(())
    }

    /// Display an error message above the viewport.
    fn show_error(
        &self,
        terminal: &mut custom_terminal::Terminal,
        msg: &str,
    ) -> anyhow::Result<()> {
        render::insert_system_message(
            terminal,
            vec![Line::from(Span::styled(
                msg.to_string(),
                Style::default().fg(Color::Red),
            ))],
        )
    }

    /// Display an info message above the viewport.
    fn show_info(&self, terminal: &mut custom_terminal::Terminal, msg: &str) -> anyhow::Result<()> {
        render::insert_system_message(
            terminal,
            vec![Line::from(Span::styled(
                msg.to_string(),
                Style::default().fg(Color::Yellow),
            ))],
        )
    }

    /// Clear the textarea and restore default styles.
    fn clear_textarea(&mut self) {
        self.textarea = Self::new_textarea();
    }
}
