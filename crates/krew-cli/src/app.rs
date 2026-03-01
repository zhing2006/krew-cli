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

use crate::completion::{ActivePopup, CompletionItem, CompletionState};
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
    /// Active completion popup state.
    pub popup: ActivePopup,
    /// Input history (most recent last).
    history: Vec<String>,
    /// Current position in history navigation (None = not browsing).
    history_index: Option<usize>,
    /// Draft input saved when entering history navigation.
    history_draft: String,
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

        // If popup is active, intercept navigation keys.
        if self.popup.is_active() && self.handle_popup_key(key_event, terminal)? {
            return Ok(());
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
            // Up arrow: history prev when cursor is on the first row.
            Input {
                key: Key::Up,
                shift: false,
                ctrl: false,
                alt: false,
            } if self.textarea.cursor().0 == 0 => {
                self.history_prev();
            }
            // Down arrow: history next when cursor is on the last row.
            Input {
                key: Key::Down,
                shift: false,
                ctrl: false,
                alt: false,
            } if self.textarea.cursor().0 == self.textarea.lines().len() - 1 => {
                self.history_next();
            }
            // All other keys => forward to textarea.
            other => {
                self.textarea.input(other);
            }
        }
        Ok(())
    }

    /// Handle keys when popup is active. Returns true if the key was consumed.
    fn handle_popup_key(
        &mut self,
        key_event: KeyEvent,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<bool> {
        match key_event.code {
            KeyCode::Up => {
                match &mut self.popup {
                    ActivePopup::SlashCommand(s) | ActivePopup::AgentName(s) => s.move_up(),
                    ActivePopup::None => {}
                }
                Ok(true)
            }
            KeyCode::Down => {
                match &mut self.popup {
                    ActivePopup::SlashCommand(s) | ActivePopup::AgentName(s) => s.move_down(),
                    ActivePopup::None => {}
                }
                Ok(true)
            }
            KeyCode::Tab => {
                self.accept_completion();
                Ok(true)
            }
            KeyCode::Enter => {
                // For slash commands, execute directly. For agent names, insert.
                match &self.popup {
                    ActivePopup::SlashCommand(state) => {
                        if let Some(item) = state.selected_item() {
                            let cmd_input = item.value.clone();
                            self.popup = ActivePopup::None;
                            self.clear_textarea();
                            self.execute_slash_command(&cmd_input, terminal)?;
                            return Ok(true);
                        }
                    }
                    ActivePopup::AgentName(_) => {
                        self.accept_completion();
                        return Ok(true);
                    }
                    ActivePopup::None => {}
                }
                Ok(false)
            }
            KeyCode::Esc => {
                self.popup = ActivePopup::None;
                Ok(true)
            }
            _ => Ok(false), // Let other keys pass through to textarea.
        }
    }

    /// Accept the currently selected completion item.
    fn accept_completion(&mut self) {
        match &self.popup {
            ActivePopup::SlashCommand(state) => {
                if let Some(item) = state.selected_item() {
                    // Replace entire input with the slash command + trailing space.
                    let text = format!("{} ", item.value);
                    self.set_textarea_content(&text);
                }
            }
            ActivePopup::AgentName(state) => {
                if let Some(item) = state.selected_item() {
                    let insert_text = format!("@{} ", item.value);
                    // Replace the @token at cursor with the selected name.
                    self.replace_at_token(&insert_text);
                }
            }
            ActivePopup::None => {}
        }
        self.popup = ActivePopup::None;
    }

    /// Replace the `@token` at the cursor position with `replacement`.
    fn replace_at_token(&mut self, replacement: &str) {
        let (row, col) = self.textarea.cursor();
        let lines = self.textarea.lines();
        let Some(line) = lines.get(row) else {
            return;
        };
        let line = line.clone();

        // Find the @token boundaries around the cursor.
        let before = &line[..col.min(line.len())];
        let at_pos = match before.rfind('@') {
            Some(pos) => pos,
            None => return,
        };

        // Find the end of the token (next whitespace or end of line).
        let after_at = &line[at_pos..];
        let token_end = after_at
            .find(|c: char| c.is_whitespace())
            .map(|i| at_pos + i)
            .unwrap_or(line.len());

        // Build new line.
        let new_line = format!("{}{}{}", &line[..at_pos], replacement, &line[token_end..]);

        // Rebuild all lines.
        let mut all_lines: Vec<String> = lines.to_vec();
        all_lines[row] = new_line.clone();
        let new_col = at_pos + replacement.len();

        let mut textarea = TextArea::new(all_lines);
        textarea.set_cursor_line_style(Style::default());
        textarea.set_cursor_style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::REVERSED),
        );
        // Move cursor to the correct position.
        for _ in 0..row {
            textarea.move_cursor(ratatui_textarea::CursorMove::Down);
        }
        textarea.move_cursor(ratatui_textarea::CursorMove::Head);
        for _ in 0..new_col {
            textarea.move_cursor(ratatui_textarea::CursorMove::Forward);
        }
        self.textarea = textarea;
    }

    // ── Completion popup sync ────────────────────────────────────────

    /// Detect whether a completion popup should be shown based on current input.
    fn sync_popup(&mut self) {
        let lines = self.textarea.lines();
        let first_line = lines.first().map(|s| s.as_str()).unwrap_or("");

        // Slash command popup: first line starts with `/`.
        if first_line.starts_with('/') && lines.len() == 1 {
            let filter = first_line; // include `/` so it matches "/agents" etc.
            match &mut self.popup {
                ActivePopup::SlashCommand(state) => {
                    state.set_filter(filter);
                    if state.is_empty() {
                        self.popup = ActivePopup::None;
                    }
                }
                _ => {
                    let mut state = CompletionState::new(self.slash_command_items());
                    state.set_filter(filter);
                    if state.is_empty() {
                        self.popup = ActivePopup::None;
                    } else {
                        self.popup = ActivePopup::SlashCommand(state);
                    }
                }
            }
            return;
        }

        // Agent name popup: detect @token at cursor position.
        if let Some(token) = self.current_at_token() {
            let filter = &token; // text after `@`
            match &mut self.popup {
                ActivePopup::AgentName(state) => {
                    state.set_filter(filter);
                    if state.is_empty() {
                        self.popup = ActivePopup::None;
                    }
                }
                _ => {
                    let mut state = CompletionState::new(self.agent_name_items());
                    state.set_filter(filter);
                    if state.is_empty() {
                        self.popup = ActivePopup::None;
                    } else {
                        self.popup = ActivePopup::AgentName(state);
                    }
                }
            }
            return;
        }

        // No trigger condition — close popup.
        self.popup = ActivePopup::None;
    }

    /// Extract the `@token` at the current cursor position, if any.
    /// Returns the text after `@` (may be empty for bare `@`).
    fn current_at_token(&self) -> Option<String> {
        let (row, col) = self.textarea.cursor();
        let lines = self.textarea.lines();
        let line = lines.get(row)?;
        let safe_col = col.min(line.len());
        let before = &line[..safe_col];

        // Find the last `@` not preceded by a non-whitespace char.
        let at_pos = before.rfind('@')?;
        // Check that `@` is at start or preceded by whitespace.
        if at_pos > 0 && !line.as_bytes()[at_pos - 1].is_ascii_whitespace() {
            return None;
        }

        // Extract the token from @ to the next whitespace (or cursor).
        let token_start = at_pos + 1;
        let token = &line[token_start..safe_col];

        // Don't trigger if there's a space between @ and cursor.
        if token.contains(' ') {
            return None;
        }

        Some(token.to_string())
    }

    /// Build completion items for slash commands.
    fn slash_command_items(&self) -> Vec<CompletionItem> {
        SlashCommand::all_help()
            .iter()
            .map(|&(name, desc)| CompletionItem {
                value: name.to_string(),
                description: desc.to_string(),
            })
            .collect()
    }

    /// Build completion items for agent names (including "all").
    fn agent_name_items(&self) -> Vec<CompletionItem> {
        let mut items = vec![CompletionItem {
            value: "all".to_string(),
            description: "Broadcast to all agents".to_string(),
        }];
        for agent in &self.config.agents {
            items.push(CompletionItem {
                value: agent.name.clone(),
                description: format!(
                    "{} ({}/{})",
                    agent.display_name, agent.provider, agent.model
                ),
            });
        }
        items
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

        // Push to input history.
        self.history_push(trimmed.to_string());

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
        let diamond = Span::styled(
            "\u{25c6} ".to_string(), // ◆
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
        let echo_prefix = format!("{route_tag} echo: ");
        let mut body_lines = body.lines();
        let first_body = body_lines.next().unwrap_or("");
        let mut echo_lines: Vec<Line<'static>> = vec![Line::from(vec![
            diamond,
            Span::raw(format!("{echo_prefix}{first_body}")),
        ])];
        for line in body_lines {
            echo_lines.push(Line::from(Span::raw(line.to_string())));
        }
        render::insert_lines(terminal, echo_lines)?;

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

        // Build lines — first line gets the prefix, continuation lines flush left.
        let mut text_lines = text.lines();
        let first_text = text_lines.next().unwrap_or("");
        spans.push(Span::raw(first_text.to_string()));
        let mut lines: Vec<Line<'static>> = vec![Line::from(spans)];
        for line in text_lines {
            lines.push(Line::from(Span::raw(line.to_string())));
        }
        render::insert_lines(terminal, lines)
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
            SlashCommand::Exit => {
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
            SlashCommand::Resume
            | SlashCommand::Compact(_)
            | SlashCommand::Mcp
            | SlashCommand::Skills => {
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

        render::insert_lines(terminal, lines)
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

        render::insert_lines(terminal, lines)
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
        render::insert_lines(
            terminal,
            vec![Line::from(Span::styled(
                msg.to_string(),
                Style::default().fg(Color::Red),
            ))],
        )
    }

    /// Display an info message above the viewport.
    fn show_info(&self, terminal: &mut custom_terminal::Terminal, msg: &str) -> anyhow::Result<()> {
        render::insert_lines(
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
        self.history_index = None;
    }

    // ── Input history ────────────────────────────────────────────────

    /// Push an entry to the input history, respecting the configured limit.
    fn history_push(&mut self, entry: String) {
        // Avoid consecutive duplicates.
        if self.history.last().is_some_and(|last| last == &entry) {
            return;
        }
        self.history.push(entry);
        let limit = self.config.settings.input_history_limit;
        if self.history.len() > limit {
            self.history.drain(..self.history.len() - limit);
        }
    }

    /// Navigate to the previous history entry (Up arrow).
    fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let idx = match self.history_index {
            Some(i) if i > 0 => i - 1,
            Some(_) => return, // already at oldest
            None => {
                // Entering history — save current input as draft.
                self.history_draft = self.textarea.lines().join("\n");
                self.history.len() - 1
            }
        };
        self.history_index = Some(idx);
        self.load_history_entry(idx);
    }

    /// Navigate to the next history entry (Down arrow).
    fn history_next(&mut self) {
        let Some(current) = self.history_index else {
            return; // not browsing history
        };
        if current + 1 < self.history.len() {
            let idx = current + 1;
            self.history_index = Some(idx);
            self.load_history_entry(idx);
        } else {
            // Past the newest entry — restore draft.
            self.history_index = None;
            self.set_textarea_content(&self.history_draft.clone());
        }
    }

    /// Load a history entry into the textarea.
    fn load_history_entry(&mut self, idx: usize) {
        if let Some(entry) = self.history.get(idx) {
            self.set_textarea_content(&entry.clone());
        }
    }

    /// Replace textarea content with the given text (supports multiline).
    fn set_textarea_content(&mut self, content: &str) {
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
