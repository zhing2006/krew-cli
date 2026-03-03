//! Slash command execution logic.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use krew_core::command::SlashCommand;
use krew_llm::{ChatMessage, ChatRole};

use crate::completion::{ActivePopup, CompletionItem, CompletionState};
use crate::custom_terminal;
use crate::render;

use super::App;

impl App {
    /// Execute a slash command.
    pub(crate) fn execute_slash_command(
        &mut self,
        input: &str,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
        let Some(cmd) = SlashCommand::from_input(input) else {
            return self.show_error(terminal, &format!("Unknown command: {input}"));
        };

        match cmd {
            SlashCommand::Exit => {
                // Save session before quitting.
                if !self.messages.is_empty() {
                    self.save_session();
                }
                self.should_quit = true;
            }
            SlashCommand::Help => {
                self.execute_help(terminal)?;
            }
            SlashCommand::Agents => {
                self.execute_agents(terminal)?;
            }
            SlashCommand::Clear => {
                self.execute_new(terminal)?;
            }
            SlashCommand::Stats => {
                self.execute_stats(terminal)?;
            }
            SlashCommand::Resume => {
                self.execute_resume(terminal)?;
            }
            SlashCommand::Compact(_) | SlashCommand::Mcp | SlashCommand::Skills => {
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
            let (prompt_tokens, completion_tokens) = self
                .agent_token_usage
                .get(&agent.name)
                .copied()
                .unwrap_or((0, 0));
            let total = prompt_tokens + completion_tokens;
            let token_text = if total > 0 {
                format!("  {total} tokens ({prompt_tokens} in / {completion_tokens} out)")
            } else {
                "  0 tokens".to_string()
            };
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
                Span::styled(token_text, Style::default().fg(Color::DarkGray)),
            ]));
        }

        render::insert_lines(terminal, lines)
    }

    /// Execute /stats: display process memory and thread count.
    fn execute_stats(&self, terminal: &mut custom_terminal::Terminal) -> anyhow::Result<()> {
        use krew_core::process_stats::ProcessStats;

        let stats = ProcessStats::collect();
        let thread_text = match stats.thread_count {
            Some(n) => n.to_string(),
            None => "N/A".to_string(),
        };

        let lines: Vec<Line<'static>> = vec![
            Line::from(Span::styled(
                "Process Stats:",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(vec![
                Span::styled("  Memory    ", Style::default().fg(Color::Cyan)),
                Span::styled(stats.format_memory(), Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("  Threads   ", Style::default().fg(Color::Cyan)),
                Span::styled(thread_text, Style::default().fg(Color::White)),
            ]),
        ];

        render::insert_lines(terminal, lines)
    }

    /// Execute /new (also /clear): save current session, start a new one.
    fn execute_new(&mut self, terminal: &mut custom_terminal::Terminal) -> anyhow::Result<()> {
        // Save current session if it has messages.
        if !self.messages.is_empty() {
            self.save_session();
        }

        // Clear conversation state.
        self.messages.clear();
        self.agent_token_usage.clear();
        self.last_respondent = None;

        // Generate new session ID.
        self.session_id = uuid::Uuid::new_v4().to_string()[..8].to_string();

        // Clear screen and re-display header with new session ID.
        terminal.clear()?;
        let size = terminal.size()?;
        terminal.set_viewport_area(ratatui::layout::Rect::new(0, 0, size.width, 0));
        render::insert_header(terminal, self)?;

        self.show_info(
            terminal,
            &format!("New session started: {}", self.session_id),
        )?;

        Ok(())
    }

    /// Execute /resume: open a session picker popup.
    fn execute_resume(&mut self, terminal: &mut custom_terminal::Terminal) -> anyhow::Result<()> {
        let summaries = match krew_storage::session_file::list_sessions(&self.session_dir) {
            Ok(s) => s,
            Err(e) => {
                return self.show_error(terminal, &format!("Failed to list sessions: {e}"));
            }
        };

        if summaries.is_empty() {
            return self.show_info(terminal, "No saved sessions found");
        }

        // Build completion items from session summaries.
        let items: Vec<CompletionItem> = summaries
            .iter()
            .take(20)
            .map(|s| {
                let time_str = s.updated_at.format("%m-%d %H:%M").to_string();
                let agents_str = s.agents.join(",");
                let preview = s.first_message_preview.as_deref().unwrap_or("(empty)");
                CompletionItem {
                    value: s.id.clone(),
                    description: format!("{time_str}  ({agents_str})  \"{preview}\""),
                }
            })
            .collect();

        self.popup = ActivePopup::SessionPicker(CompletionState::new(items));

        Ok(())
    }

    /// Load a session from disk by ID and replay its history on screen.
    pub(crate) fn load_session(
        &mut self,
        session_id: &str,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
        let path = self.session_dir.join(format!("{session_id}.toml"));
        let session_file = krew_storage::session_file::load_session(&path)
            .map_err(|e| anyhow::anyhow!("Failed to load session {session_id}: {e}"))?;

        // Restore session state.
        self.session_id = session_file.session.id.clone();
        self.messages.clear();
        self.agent_token_usage.clear();
        self.last_respondent = None;

        // Clear screen and show header with restored session ID.
        terminal.clear()?;
        let size = terminal.size()?;
        terminal.set_viewport_area(ratatui::layout::Rect::new(0, 0, size.width, 0));
        render::insert_header(terminal, self)?;

        // Convert stored messages back to runtime format and replay visually.
        for msg in &session_file.messages {
            let role = match msg.role.as_str() {
                "system" => ChatRole::System,
                "user" => ChatRole::User,
                "assistant" => ChatRole::Assistant,
                "tool" => ChatRole::Tool,
                _ => continue,
            };

            // Track last assistant as last_respondent.
            if role == ChatRole::Assistant
                && let Some(name) = &msg.agent_name
            {
                self.last_respondent = Some(name.clone());
            }

            // Replay message on screen.
            match role {
                ChatRole::User => {
                    self.insert_user_message(terminal, &[], &msg.content)?;
                }
                ChatRole::Assistant => {
                    if let Some(agent_name) = &msg.agent_name {
                        let agent_cfg = self.config.agents.iter().find(|a| &a.name == agent_name);
                        let display_name = agent_cfg
                            .map(|a| a.display_name.as_str())
                            .unwrap_or(agent_name);
                        let color_name = agent_cfg.map(|a| a.color.as_str()).unwrap_or("white");
                        self.insert_agent_header(terminal, agent_name, display_name, color_name)?;
                    }
                    let md_lines = render::markdown::render_markdown(&msg.content);
                    self.insert_indented_lines(terminal, md_lines)?;
                }
                _ => {}
            }

            self.messages.push(ChatMessage {
                role,
                content: msg.content.clone(),
                name: msg.agent_name.clone(),
            });
        }

        // Restore token usage from session metadata.
        // The stored per-message usage is cumulative, take the last one per agent.
        for msg in session_file.messages.iter().rev() {
            if msg.role == "assistant"
                && let (Some(name), Some(usage)) = (&msg.agent_name, &msg.usage)
            {
                self.agent_token_usage
                    .entry(name.clone())
                    .or_insert((usage.prompt_tokens, usage.completion_tokens));
            }
        }

        // Update session to mark it as resumed.
        self.save_session();

        Ok(())
    }

    /// Display an error message above the viewport.
    pub(crate) fn show_error(
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

    /// Display a warning message above the viewport.
    pub(crate) fn show_warning(
        &self,
        terminal: &mut custom_terminal::Terminal,
        msg: &str,
    ) -> anyhow::Result<()> {
        render::insert_lines(
            terminal,
            vec![Line::from(Span::styled(
                format!("\u{26a0} {msg}"), // ⚠
                Style::default().fg(Color::Yellow),
            ))],
        )
    }

    /// Display an info message above the viewport.
    pub(crate) fn show_info(
        &self,
        terminal: &mut custom_terminal::Terminal,
        msg: &str,
    ) -> anyhow::Result<()> {
        render::insert_lines(
            terminal,
            vec![Line::from(Span::styled(
                msg.to_string(),
                Style::default().fg(Color::Yellow),
            ))],
        )
    }
}
