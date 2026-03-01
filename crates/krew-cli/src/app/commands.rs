//! Slash command execution logic.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use krew_core::command::SlashCommand;

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
            SlashCommand::Stats => {
                self.execute_stats(terminal)?;
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

    /// Execute /clear: clear visible content and re-display header.
    fn execute_clear(&self, terminal: &mut custom_terminal::Terminal) -> anyhow::Result<()> {
        terminal.clear()?;
        // Reset viewport to the top so the header has space to render.
        let size = terminal.size()?;
        terminal.set_viewport_area(ratatui::layout::Rect::new(0, 0, size.width, 0));
        render::insert_header(terminal, self)?;
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
