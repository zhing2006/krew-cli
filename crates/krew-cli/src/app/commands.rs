//! Slash command execution logic.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use krew_core::command::SlashCommand;

use crate::completion::{ActivePopup, CompletionItem, CompletionState};
use crate::custom_terminal;
use crate::render;

use super::App;
use super::agent_display::{format_tool_call_display, render_tool_diff_preview};

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
            SlashCommand::Mcp => {
                self.execute_mcp(terminal)?;
            }
            SlashCommand::Compact(agent_arg) => {
                self.execute_compact(agent_arg, terminal)?;
            }
            SlashCommand::Skills => {
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

    /// Execute /compact: schedule compaction with the specified agent.
    fn execute_compact(
        &mut self,
        agent_arg: String,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
        if self.messages.is_empty() {
            return self.show_info(terminal, "Nothing to compact — session is empty");
        }

        // Resolve agent name: use argument or default to reply_order[0].
        let agent_name = if agent_arg.is_empty() {
            match self.config.settings.reply_order.first() {
                Some(name) => name.clone(),
                None => return self.show_error(terminal, "No agents available for compaction"),
            }
        } else {
            agent_arg
        };

        // Validate agent exists and has an LLM client.
        if !self.agents.contains_key(&agent_name) {
            return self.show_error(terminal, &format!("Agent \"{agent_name}\" not found"));
        }

        // Schedule compact (processed in the main event loop).
        self.pending_compact_agent = Some(agent_name);
        Ok(())
    }

    /// Execute /agents: display agent list with token stats.
    fn execute_agents(&self, terminal: &mut custom_terminal::Terminal) -> anyhow::Result<()> {
        let mut lines: Vec<Line<'static>> = vec![Line::from(Span::styled(
            "Agents:",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ))];

        let mut total_prompt: u32 = 0;
        let mut total_completion: u32 = 0;

        for agent in &self.config.agents {
            let color = render::parse_color(&agent.color);
            let (prompt_tokens, completion_tokens) = self
                .agent_token_usage
                .get(&agent.name)
                .copied()
                .unwrap_or((0, 0));
            total_prompt += prompt_tokens;
            total_completion += completion_tokens;
            let total = prompt_tokens + completion_tokens;
            let token_text = if total > 0 {
                format!(
                    "  {} tokens ({} in / {} out)",
                    format_number(total),
                    format_number(prompt_tokens),
                    format_number(completion_tokens)
                )
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

        // Total line.
        let grand_total = total_prompt + total_completion;
        if grand_total > 0 {
            lines.push(Line::from(Span::styled(
                format!(
                    "  {}\n  Total: {} tokens",
                    "\u{2500}".repeat(50),
                    format_number(grand_total)
                ),
                Style::default().fg(Color::DarkGray),
            )));
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

    /// Execute /mcp: display MCP servers and their tools.
    fn execute_mcp(&self, terminal: &mut custom_terminal::Terminal) -> anyhow::Result<()> {
        let Some(ref manager) = self.mcp_manager else {
            return self.show_info(terminal, "No MCP servers configured");
        };

        let servers = manager.server_info();
        if servers.is_empty() {
            return self.show_info(terminal, "No MCP servers connected");
        }

        let mut lines: Vec<Line<'static>> = vec![Line::from(Span::styled(
            "MCP Servers:",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ))];

        for server in &servers {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!("[{}]", server.name),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!("  {} tool(s)", server.tool_count)),
            ]));

            for tool_name in &server.tool_names {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(tool_name.clone(), Style::default().fg(Color::DarkGray)),
                ]));
            }
        }

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
        let restored = krew_core::persistence::load_session_from_disk(&path)
            .map_err(|e| anyhow::anyhow!("Failed to load session {session_id}: {e}"))?;

        // Apply restored state.
        self.session_id = restored.session_id;
        self.messages = restored.messages;
        self.agent_token_usage = restored.token_usage;
        self.last_respondent = restored.last_respondent;

        // Clear screen and show header with restored session ID.
        terminal.clear()?;
        let size = terminal.size()?;
        terminal.set_viewport_area(ratatui::layout::Rect::new(0, 0, size.width, 0));
        render::insert_header(terminal, self)?;

        // Replay messages visually (TUI concern).
        // Track whether the agent header has been shown for the current agent turn
        // so we don't duplicate it across tool-call rounds.
        let mut header_shown_for: Option<String> = None;

        for msg in &restored.session_file.messages {
            match msg.role.as_str() {
                "user" => {
                    header_shown_for = None;
                    self.insert_user_message(terminal, &[], &msg.content)?;
                }
                "assistant" => {
                    // Show agent header if this is the first assistant message
                    // for this agent (across tool-call rounds).
                    if let Some(agent_name) = &msg.agent_name {
                        let already_shown = header_shown_for
                            .as_ref()
                            .is_some_and(|shown| shown == agent_name);
                        if !already_shown {
                            let agent_cfg =
                                self.config.agents.iter().find(|a| &a.name == agent_name);
                            let display_name = agent_cfg
                                .map(|a| a.display_name.as_str())
                                .unwrap_or(agent_name);
                            let color_name = agent_cfg.map(|a| a.color.as_str()).unwrap_or("white");
                            self.insert_agent_header(
                                terminal,
                                agent_name,
                                display_name,
                                color_name,
                            )?;
                            header_shown_for = Some(agent_name.clone());
                        }
                    }

                    if let Some(ref tool_calls) = msg.tool_calls {
                        // Assistant message with tool calls: show text + tool call lines.
                        if !msg.content.is_empty() {
                            let md_lines = render::markdown::render_markdown(&msg.content);
                            self.insert_indented_lines(terminal, md_lines)?;
                        }
                        for tc in tool_calls {
                            let display = format_tool_call_display(&tc.name, &tc.arguments);
                            let yellow = Style::default().fg(Color::Yellow);
                            self.insert_tool_line(terminal, "\u{26A1} ", yellow, display)?;

                            // Render diff preview for write/edit tools (same as streaming).
                            let width = terminal.size().map(|s| s.width as usize).unwrap_or(80);
                            let preview = render_tool_diff_preview(&tc.name, &tc.arguments, width);
                            if !preview.is_empty() {
                                terminal.insert_lines_above(preview)?;
                            }
                        }
                    } else {
                        // Regular text-only assistant message.
                        // Split server tool uses: "before text" (begin/end) vs "after text" (Gemini).
                        let (before_text, after_text): (Vec<_>, Vec<_>) = msg
                            .server_tool_uses
                            .iter()
                            .partition(|s| s.name != "google_search");

                        // Render before-text server tools (OpenAI/Anthropic) in begin/end format.
                        for stu in &before_text {
                            let bold = Style::default().add_modifier(Modifier::BOLD);
                            let display = vec![Span::styled(stu.name.clone(), bold)];
                            let cyan = Style::default().fg(Color::Cyan);
                            self.insert_tool_line(terminal, "\u{1F310} ", cyan, display)?;
                            let dim = Style::default().fg(Color::DarkGray);
                            let summary = stu
                                .query
                                .as_ref()
                                .map(|q| format!("\"{q}\""))
                                .unwrap_or_default();
                            self.insert_tool_line(
                                terminal,
                                "   \u{23BF}  ",
                                dim,
                                vec![Span::raw(summary)],
                            )?;
                            terminal.insert_lines_above(vec![Line::default()])?;
                        }

                        let md_lines = render::markdown::render_markdown(&msg.content);
                        self.insert_indented_lines(terminal, md_lines)?;

                        // Render after-text server tools (Gemini) as full 🌐 line with query.
                        for stu in &after_text {
                            let bold = Style::default().add_modifier(Modifier::BOLD);
                            let normal = Style::default();
                            let done_name = format!("{}_done", stu.name);
                            let display = if let Some(q) = &stu.query {
                                vec![
                                    Span::styled(done_name, bold),
                                    Span::styled(format!("(\"{q}\")"), normal),
                                ]
                            } else {
                                vec![Span::styled(done_name, bold)]
                            };
                            let cyan = Style::default().fg(Color::Cyan);
                            self.insert_tool_line(terminal, "\u{1F310} ", cyan, display)?;
                            terminal.insert_lines_above(vec![Line::default()])?;
                        }
                    }
                }
                "tool" => {
                    // Tool result message: show shell output and summary line.
                    let tool_name = msg.agent_name.as_deref().unwrap_or("tool");

                    // Render shell/MCP/fetch_url output with separators (same as streaming).
                    if tool_name == "shell"
                        || tool_name == "fetch_url"
                        || krew_tools::mcp::is_mcp_tool(tool_name)
                    {
                        let width = terminal.size().map(|s| s.width as usize).unwrap_or(80);
                        render_resume_shell_output(terminal, &msg.content, width)?;
                    }

                    let summary = generate_tool_result_summary(tool_name, &msg.content);
                    let dim = Style::default().fg(Color::DarkGray);
                    self.insert_tool_line(
                        terminal,
                        "   \u{23BF}  ",
                        dim,
                        vec![Span::raw(summary)],
                    )?;
                    terminal.insert_lines_above(vec![Line::default()])?;
                }
                _ => {}
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

/// Generate a short summary for a tool result during resume replay.
///
/// Extracts the trailing `(N <unit>)` pattern if present, otherwise
/// returns a generic "done" string.
fn generate_tool_result_summary(_tool_name: &str, content: &str) -> String {
    if let Some(summary) = content
        .rsplit_once('(')
        .and_then(|(_, rest)| rest.strip_suffix(')'))
    {
        return summary.to_string();
    }
    "done".to_string()
}

/// Maximum lines to display for tool output during resume replay.
const MAX_RESUME_DISPLAY_LINES: usize = 200;

/// Render shell output with separators during resume replay.
///
/// Extracts the output portion from shell tool result content (stripping
/// the trailing summary like `(exit code N)` or `(no output, ...)`), then
/// renders it with `────` separators and 4-space indentation, matching the
/// streaming display format.
fn render_resume_shell_output(
    terminal: &mut custom_terminal::Terminal,
    content: &str,
    width: usize,
) -> anyhow::Result<()> {
    // Extract output lines by stripping the trailing summary.
    // Content formats:
    //   "(no output, exit code N)"         → no output to render
    //   "output text"                      → full content is output (success)
    //   "output text\n\n(exit code N)"     → strip trailing summary (error)
    //   "User denied execution of shell."  → no output to render
    let output = if content.starts_with('(') && content.ends_with(')') {
        // Summary-only message like "(no output, exit code 0)".
        ""
    } else if let Some(pos) = content.rfind("\n\n(") {
        // Strip trailing "\n\n(exit code N)" from error output.
        if content.ends_with(')') {
            &content[..pos]
        } else {
            content
        }
    } else {
        content
    };

    if output.is_empty() {
        return Ok(());
    }

    let dim = Style::default().fg(Color::DarkGray);
    let sep = "\u{2500}".repeat(width.saturating_sub(6).min(40));

    // Begin separator.
    terminal.insert_lines_above(vec![Line::from(Span::styled(format!("    {sep}"), dim))])?;

    // Output lines with 4-space indent, truncated to match streaming display.
    let total_lines = output.lines().count();
    for line in output.lines().take(MAX_RESUME_DISPLAY_LINES) {
        terminal.insert_lines_above(vec![Line::from(format!("    {line}"))])?;
    }
    if total_lines > MAX_RESUME_DISPLAY_LINES {
        terminal.insert_lines_above(vec![Line::from(Span::styled(
            format!(
                "    ... ({} more lines omitted)",
                total_lines - MAX_RESUME_DISPLAY_LINES
            ),
            dim,
        ))])?;
    }

    // End separator.
    terminal.insert_lines_above(vec![Line::from(Span::styled(format!("    {sep}"), dim))])?;

    Ok(())
}

/// Format a number with comma separators (e.g. 12345 → "12,345").
fn format_number(n: u32) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(c);
    }
    result
}
