//! Slash command execution logic.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use krew_core::command::{DreamScope, SlashCommand};
use krew_core::dream;
use krew_llm::{ChatMessage, ChatRole};

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
                self.execute_skills(terminal)?;
            }
            SlashCommand::Tools => {
                self.execute_tools(terminal)?;
            }
            SlashCommand::Rewind => {
                self.execute_rewind(terminal)?;
            }
            SlashCommand::Dream(scope, agent_name) => {
                self.execute_dream(scope, agent_name, terminal)?;
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

        // Append custom commands (excluding those shadowed by built-in commands).
        let custom_cmds: Vec<_> = self
            .custom_commands
            .list()
            .into_iter()
            .filter(|cmd| SlashCommand::from_input(&format!("/{}", cmd.name)).is_none())
            .collect();
        if !custom_cmds.is_empty() {
            lines.push(Line::from(Span::styled(
                "Custom commands:",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));
            for cmd in custom_cmds {
                let name = format!("/{}", cmd.name);
                let desc = if cmd.description.is_empty() {
                    cmd.name.clone()
                } else {
                    cmd.description.clone()
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("  {name:<12}"), Style::default().fg(Color::Cyan)),
                    Span::styled(desc, Style::default().fg(Color::DarkGray)),
                ]));
            }
        }

        render::insert_lines(terminal, lines)
    }

    /// Execute /compact: schedule compaction with the specified agent.
    fn execute_compact(
        &mut self,
        agent_arg: String,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
        if self.rewound {
            return self.show_info(
                terminal,
                "Cannot compact in rewound state — send a new message first",
            );
        }
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

    /// Execute /dream: trigger memory consolidation with a specific agent.
    fn execute_dream(
        &mut self,
        scope: DreamScope,
        agent_name: String,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
        // Show usage hint if agent name is empty.
        if agent_name.is_empty() {
            return self.show_info(
                terminal,
                "Usage: /dream <scope> @<agent>  (scope: global, agent, all)",
            );
        }

        // Reject @all.
        if agent_name == "all" {
            return self.show_error(
                terminal,
                "/dream does not support @all — specify a single agent",
            );
        }

        // Validate agent exists.
        if !self.agents.contains_key(&agent_name) {
            return self.show_error(terminal, &format!("Agent \"{agent_name}\" not found"));
        }

        // Validate agent has tools enabled.
        let has_tools = self
            .config
            .agents
            .iter()
            .find(|a| a.name == agent_name)
            .is_some_and(|a| a.tools);
        if !has_tools {
            return self.show_error(
                terminal,
                &format!("Agent \"{agent_name}\" has tools disabled — cannot execute dream"),
            );
        }

        // Fork semantics: generate new session ID on first action after rewind.
        if self.rewound {
            self.session_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
            self.session_created_at = chrono::Utc::now();
            self.rewound = false;
        }

        // Build dream prompt.
        let prompt = dream::build_dream_prompt(scope, &agent_name);

        let scope_label = match scope {
            DreamScope::Global => "global",
            DreamScope::Agent => "agent",
            DreamScope::All => "all",
        };

        // Show status.
        self.show_info(
            terminal,
            &format!("Dreaming with [{agent_name}] ({scope_label})..."),
        )?;

        // Inject as whisper message.
        let whisper_targets = vec![agent_name.clone()];
        self.messages.push(
            ChatMessage::user_with_addressee(prompt, Some(agent_name.clone()))
                .with_whisper_targets(Some(whisper_targets.clone())),
        );

        // Build exclude list from whitelist: exclude all tools NOT in the
        // allowed set. This covers MCP tools and any future built-in tools.
        let exclude_tools: Vec<String> = self
            .agents
            .get(&agent_name)
            .map(|agent| {
                agent
                    .tools
                    .specs()
                    .iter()
                    .filter(|spec| !dream::DREAM_ALLOWED_TOOLS.contains(&spec.name.as_str()))
                    .map(|spec| spec.name.clone())
                    .collect()
            })
            .unwrap_or_default();

        // Set dispatch state.
        self.current_whisper_targets = Some(whisper_targets);
        self.current_exclude_tools = Some(exclude_tools);
        self.is_dreaming = true;
        self.pending_agents.push_back(agent_name);

        // Start the agent.
        self.start_next_agent(terminal)?;

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

        // Show Sub-Agent definitions if any.
        if !self.sub_agent_defs.is_empty() {
            lines.push(Line::default());
            lines.push(Line::from(Span::styled(
                "Sub-Agents:",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));
            for def in &self.sub_agent_defs {
                let source = def.source_path.to_string_lossy().replace('\\', "/");
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        format!("[{}]", def.name),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!("  {}", def.description)),
                ]));
                lines.push(Line::from(Span::styled(
                    format!("         {source}"),
                    Style::default().fg(Color::DarkGray),
                )));
            }
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

    /// Execute /skills: display discovered Agent Skills.
    fn execute_skills(&self, terminal: &mut custom_terminal::Terminal) -> anyhow::Result<()> {
        if !self.config.skills.enabled {
            return self.show_info(terminal, "Skills feature is disabled");
        }

        if self.skills.is_empty() {
            return self.show_info(terminal, "No skills available");
        }

        let mut lines: Vec<Line<'static>> = vec![Line::from(Span::styled(
            format!("Skills ({}):", self.skills.len()),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ))];

        for skill in &self.skills {
            let location = skill.location.to_string_lossy().replace('\\', "/");
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!("[{}]", skill.name),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!("  {}", skill.description)),
            ]));
            lines.push(Line::from(Span::styled(
                format!("    {location}"),
                Style::default().fg(Color::DarkGray),
            )));
        }

        render::insert_lines(terminal, lines)
    }

    /// Execute /tools: display available tools per agent.
    fn execute_tools(&self, terminal: &mut custom_terminal::Terminal) -> anyhow::Result<()> {
        let mut lines: Vec<Line<'static>> = vec![Line::from(Span::styled(
            "Tools:",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ))];

        for (i, agent) in self.config.agents.iter().enumerate() {
            if i > 0 {
                lines.push(Line::default());
            }

            let color = render::parse_color(&agent.color);

            match self.agents.get(&agent.name) {
                None => {
                    // Agent failed to initialize (provider/API key issue).
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(
                            format!("[{}]", agent.name),
                            Style::default().fg(color).add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(format!("  {} ─── ", agent.display_name)),
                        Span::styled("unavailable", Style::default().fg(Color::Red)),
                    ]));
                }
                Some(runtime) => {
                    // Filter out MCP tools.
                    let tools: Vec<_> = runtime
                        .tools
                        .specs()
                        .iter()
                        .filter(|s| !krew_tools::mcp::is_mcp_tool(&s.name))
                        .collect();

                    let server_tool_count = if agent.enable_web_search { 1 } else { 0 };
                    let total = tools.len() + server_tool_count;
                    let count_text = if total == 0 {
                        "no tool(s)".to_string()
                    } else {
                        format!("{} tool(s)", total)
                    };

                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(
                            format!("[{}]", agent.name),
                            Style::default().fg(color).add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(format!("  {} ─── {count_text}", agent.display_name)),
                    ]));

                    for tool in &tools {
                        lines.push(Line::from(vec![
                            Span::styled(
                                format!("    {:<16}", tool.name),
                                Style::default().fg(Color::Cyan),
                            ),
                            Span::styled(
                                tool.description.clone(),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]));
                    }

                    // Show server-side tools (provider-native, not in ToolRegistry).
                    if agent.enable_web_search {
                        lines.push(Line::from(vec![
                            Span::styled(
                                format!("    {:<16}", "web_search"),
                                Style::default().fg(Color::Cyan),
                            ),
                            Span::styled(
                                "Provider-native web search",
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]));
                    }
                }
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

        // Reset session-scoped tool state (e.g. skill activation tracking).
        for agent in self.agents.values() {
            agent.tools.reset_session_state();
        }

        // Generate new session ID and reset creation time.
        self.session_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
        self.session_created_at = chrono::Utc::now();
        self.rewound = false;

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

    /// Execute /rewind: open a rewind picker popup showing all user messages.
    fn execute_rewind(&mut self, terminal: &mut custom_terminal::Terminal) -> anyhow::Result<()> {
        // Collect user messages with their original indices.
        let user_msgs: Vec<(usize, &ChatMessage)> = self
            .messages
            .iter()
            .enumerate()
            .filter(|(_, m)| m.role == ChatRole::User)
            .collect();

        if user_msgs.is_empty() {
            return self.show_info(terminal, "Nothing to rewind \u{2014} no messages yet");
        }

        // Build popup items in chronological order.
        let items: Vec<CompletionItem> = user_msgs
            .iter()
            .map(|&(idx, msg)| {
                let time_str = msg.created_at.format("%H:%M:%S").to_string();
                let preview: String = msg.content.chars().take(40).collect();
                let preview = preview.replace('\n', " ");
                CompletionItem {
                    value: idx.to_string(),
                    description: format!("{time_str}  \"{preview}\""),
                }
            })
            .collect();

        // Default selection: last item (most recent user message).
        let mut state = CompletionState::new(items);
        let last_idx = state.visible_items().len().saturating_sub(1);
        state.selected = last_idx;

        self.popup = ActivePopup::RewindPicker(state);
        Ok(())
    }

    /// Apply rewind: truncate messages to the given index and replay.
    pub(crate) fn apply_rewind(
        &mut self,
        msg_index: usize,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
        // If selecting the first user message (index 0), treat as /clear.
        if msg_index == 0 {
            return self.execute_new(terminal);
        }

        // Truncate messages.
        self.messages.truncate(msg_index);

        // Rebuild token usage from remaining messages (last occurrence per agent).
        self.agent_token_usage.clear();
        for msg in self.messages.iter().rev() {
            if msg.role == ChatRole::Assistant
                && let Some(name) = &msg.name
                && let Some(usage) = &msg.usage
            {
                self.agent_token_usage
                    .entry(name.clone())
                    .or_insert((usage.prompt_tokens, usage.completion_tokens));
            }
        }

        // Rebuild last_respondent.
        self.last_respondent = self
            .messages
            .iter()
            .rev()
            .find(|m| m.role == ChatRole::Assistant && m.name.is_some())
            .and_then(|m| m.name.clone());

        // Reset session-scoped tool state and rebuild skill activation.
        let activated_skills: Vec<String> = self
            .messages
            .iter()
            .filter(|m| m.role == ChatRole::Tool && m.content.contains("<skill_content"))
            .filter_map(|m| {
                m.content.find("name=\"").and_then(|start| {
                    let rest = &m.content[start + 6..];
                    rest.find('"').map(|end| rest[..end].to_string())
                })
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        for agent in self.agents.values() {
            agent.tools.restore_skill_state(&activated_skills);
        }

        // Set rewound state (fork semantics).
        self.rewound = true;

        // Replay truncated messages on screen.
        let agent_names: Vec<String> = self
            .config
            .agents
            .iter()
            .filter(|a| self.agents.contains_key(&a.name))
            .map(|a| a.name.clone())
            .collect();
        let snapshot = krew_core::persistence::SessionSnapshot {
            session_id: &self.session_id,
            cwd: &self.cwd,
            agent_names,
            messages: &self.messages,
            token_usage: &self.agent_token_usage,
            created_at: self.session_created_at,
        };
        let session_file = krew_core::persistence::build_session_file(&snapshot);

        // Clear screen and replay.
        terminal.clear()?;
        let size = terminal.size()?;
        terminal.set_viewport_area(ratatui::layout::Rect::new(0, 0, size.width, 0));
        render::insert_header(terminal, self)?;
        self.replay_messages(&session_file.messages, terminal)?;

        self.show_info(terminal, "Rewound to selected message")?;
        Ok(())
    }

    /// Replay messages on screen from serialized MessageEntry list.
    ///
    /// Extracted from `load_session()` for reuse by rewind.
    fn replay_messages(
        &self,
        messages: &[krew_storage::session_file::MessageEntry],
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
        let mut header_shown_for: Option<String> = None;

        for msg in messages {
            match msg.role.as_str() {
                "user" => {
                    header_shown_for = None;
                    let is_whisper = msg.whisper_targets.is_some();
                    let target_refs: Vec<&str> = if let Some(ref wt) = msg.whisper_targets {
                        wt.iter().map(|s| s.as_str()).collect()
                    } else if let Some(ref addr) = msg.addressee {
                        if addr == "all" {
                            self.config.agents.iter().map(|a| a.name.as_str()).collect()
                        } else {
                            addr.split(',').collect()
                        }
                    } else {
                        vec![]
                    };
                    self.insert_user_message(terminal, &target_refs, &msg.content, is_whisper)?;
                }
                "assistant" => {
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
                            let is_whisper = msg.whisper_targets.is_some();
                            self.insert_agent_header(
                                terminal,
                                agent_name,
                                display_name,
                                color_name,
                                is_whisper,
                            )?;
                            header_shown_for = Some(agent_name.clone());
                        }
                    }

                    if let Some(ref tool_calls) = msg.tool_calls {
                        if !msg.content.is_empty() {
                            let md_lines = render::markdown::render_markdown(&msg.content);
                            self.insert_indented_lines(terminal, md_lines)?;
                        }
                        for tc in tool_calls {
                            let display = format_tool_call_display(&tc.name, &tc.arguments);
                            let yellow = Style::default().fg(Color::Yellow);
                            self.insert_tool_line(terminal, "\u{26A1} ", yellow, display)?;

                            let width = terminal.size().map(|s| s.width as usize).unwrap_or(80);
                            let preview = render_tool_diff_preview(&tc.name, &tc.arguments, width);
                            if !preview.is_empty() {
                                terminal.insert_lines_above(preview)?;
                            }
                        }
                    } else {
                        let (before_text, after_text): (Vec<_>, Vec<_>) = msg
                            .server_tool_uses
                            .iter()
                            .partition(|s| s.name != "google_search");

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
                    let tool_name = msg.agent_name.as_deref().unwrap_or("tool");
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
        self.session_created_at = restored.session_created_at;

        // Sync session-scoped tool state (e.g. skill activation tracking)
        // with the restored messages to avoid stale state from previous session
        // or missing state for skills already activated in the restored session.
        let activated_skills: Vec<String> = self
            .messages
            .iter()
            .filter(|m| m.role == ChatRole::Tool && m.content.contains("<skill_content"))
            .filter_map(|m| {
                m.content.find("name=\"").and_then(|start| {
                    let rest = &m.content[start + 6..];
                    rest.find('"').map(|end| rest[..end].to_string())
                })
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        for agent in self.agents.values() {
            agent.tools.restore_skill_state(&activated_skills);
        }

        // Session loaded successfully — clear rewound so save_session() works normally.
        self.rewound = false;

        // Clear screen and show header with restored session ID.
        terminal.clear()?;
        let size = terminal.size()?;
        terminal.set_viewport_area(ratatui::layout::Rect::new(0, 0, size.width, 0));
        render::insert_header(terminal, self)?;

        // Replay messages visually (TUI concern).
        self.replay_messages(&restored.session_file.messages, terminal)?;

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
