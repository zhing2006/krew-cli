//! Message sending and user message rendering.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use krew_core::command::SlashCommand;
use krew_core::router::{self, Addressee};
use krew_llm::ChatMessage;

use crate::custom_terminal;
use crate::render;

use super::App;

impl App {
    /// Send the current input as a message or execute a slash command.
    pub(crate) fn send_message(
        &mut self,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
        // Expand paste placeholders to actual pasted content.
        let text = self.expanded_text();

        if text.trim().is_empty() {
            return Ok(());
        }

        let trimmed = text.trim();
        tracing::debug!(input = %trimmed, "User sent message");

        // Push to input history.
        self.history_push(trimmed.to_string());

        // Try built-in slash command first.
        if trimmed.starts_with('/') && SlashCommand::from_input(trimmed).is_some() {
            self.clear_textarea();
            return self.execute_slash_command(trimmed, terminal);
        }

        // Try custom command — `/name args` where name is in the custom registry.
        // If input starts with `/` but matches neither built-in nor custom, show error.
        if let Some(without_slash) = trimmed.strip_prefix('/') {
            let (cmd_part, args) = match without_slash.split_once(' ') {
                Some((c, a)) => (c, a.trim()),
                None => (without_slash, ""),
            };
            if let Some(cmd) = self.custom_commands.lookup(cmd_part) {
                let expanded = cmd.expand(args);
                self.pending_custom_command = Some(expanded);
                self.clear_textarea();
                return Ok(());
            }
            // Unknown `/` command — show error instead of treating as plain text.
            self.clear_textarea();
            return self.show_error(terminal, &format!("Unknown command: /{cmd_part}"));
        }

        // Reset AI-to-AI round counter on new user message.
        self.ai_conversation_rounds = 0;
        self.a2a_insert_cursor = 0;

        // Parse @ addressee (only known agents are recognized as addressees).
        let agent_names: Vec<String> = self.config.agents.iter().map(|a| a.name.clone()).collect();
        let (addressee, body, is_whisper) = match router::parse_input(trimmed, &agent_names) {
            Ok(result) => result,
            Err(e) => {
                self.show_error(terminal, &e.to_string())?;
                self.clear_textarea();
                return Ok(());
            }
        };

        // Resolve LastRespondent early so we can show colored dots.
        let resolved_last = match &addressee {
            Addressee::LastRespondent => self.last_respondent.clone(),
            _ => None,
        };

        // Task 3.5: Block if LastRespondent has no value.
        if matches!(&addressee, Addressee::LastRespondent) && resolved_last.is_none() {
            self.show_error(
                terminal,
                "No agent has replied yet — use @name to specify a target agent",
            )?;
            self.clear_textarea();
            return Ok(());
        }

        // Fork semantics: generate new session ID on first real message after rewind.
        // All validation has passed at this point — the message will be sent.
        if self.rewound {
            self.session_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
            self.session_created_at = chrono::Utc::now();
            self.rewound = false;
        }

        // Resolve target agent names for colored dots on user message.
        let available: std::collections::HashSet<String> = self.agents.keys().cloned().collect();
        let target_names = router::resolve_target_names(
            &addressee,
            &self.config.settings.reply_order,
            &available,
            resolved_last.as_deref(),
        );

        // Insert user message with colored routing dots: > ●●● message
        self.insert_user_message(terminal, &target_names, trimmed, is_whisper)?;

        // Set whisper state for dispatch lifecycle.
        let whisper_targets = if is_whisper {
            let targets: Vec<String> = target_names.iter().map(|n| n.to_string()).collect();
            self.current_whisper_targets = Some(targets.clone());
            Some(targets)
        } else {
            self.current_whisper_targets = None;
            None
        };

        // Add user message to conversation history with addressee info.
        let addressee_str = match &addressee {
            Addressee::All => Some("all".to_string()),
            Addressee::Single(name) => Some(name.clone()),
            Addressee::Multiple(names) => Some(names.join(",")),
            Addressee::LastRespondent => resolved_last.clone(),
        };
        self.messages.push(
            ChatMessage::user_with_addressee(body, addressee_str)
                .with_whisper_targets(whisper_targets),
        );

        // Persist session after user message.
        self.save_session();

        // Build the agent dispatch queue via krew-core router.
        self.pending_agents = router::resolve_dispatch_queue(
            &addressee,
            &self.config.settings.reply_order,
            &available,
            resolved_last.as_deref(),
        );

        // Check for unavailable agents in the dispatch queue.
        let unavailable: Vec<String> = self
            .pending_agents
            .iter()
            .filter(|name| !self.agents.contains_key(name.as_str()))
            .cloned()
            .collect();
        if !unavailable.is_empty() {
            // Remove unavailable agents from the queue.
            self.pending_agents
                .retain(|name| self.agents.contains_key(name.as_str()));
            let names = unavailable.join(", ");
            self.show_error(
                terminal,
                &format!("Agent unavailable (possibly missing API key): {names}"),
            )?;
            if self.pending_agents.is_empty() {
                self.clear_textarea();
                return Ok(());
            }
        }

        // Start the first agent in the queue.
        self.start_next_agent(terminal)?;

        self.clear_textarea();
        Ok(())
    }

    /// Send pre-expanded custom command text as a user message.
    ///
    /// Called from the event loop after async bash preprocessing completes.
    /// Follows the same routing logic as `send_message` but with already-resolved text.
    pub(crate) fn send_expanded_text(
        &mut self,
        text: &str,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        // Reset AI-to-AI round counter on new user message.
        self.ai_conversation_rounds = 0;
        self.a2a_insert_cursor = 0;

        let agent_names: Vec<String> = self.config.agents.iter().map(|a| a.name.clone()).collect();
        let (addressee, body, is_whisper) = match router::parse_input(trimmed, &agent_names) {
            Ok(result) => result,
            Err(e) => {
                return self.show_error(terminal, &e.to_string());
            }
        };

        let resolved_last = match &addressee {
            Addressee::LastRespondent => self.last_respondent.clone(),
            _ => None,
        };

        if matches!(&addressee, Addressee::LastRespondent) && resolved_last.is_none() {
            return self.show_error(
                terminal,
                "No agent has replied yet — use @name to specify a target agent",
            );
        }

        // Fork semantics: generate new session ID on first real message after rewind.
        // All validation has passed at this point — the message will be sent.
        if self.rewound {
            self.session_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
            self.session_created_at = chrono::Utc::now();
            self.rewound = false;
        }

        let available: std::collections::HashSet<String> = self.agents.keys().cloned().collect();
        let target_names = router::resolve_target_names(
            &addressee,
            &self.config.settings.reply_order,
            &available,
            resolved_last.as_deref(),
        );

        self.insert_user_message(terminal, &target_names, trimmed, is_whisper)?;

        // Set whisper state for dispatch lifecycle.
        let whisper_targets = if is_whisper {
            let targets: Vec<String> = target_names.iter().map(|n| n.to_string()).collect();
            self.current_whisper_targets = Some(targets.clone());
            Some(targets)
        } else {
            self.current_whisper_targets = None;
            None
        };

        let addressee_str = match &addressee {
            Addressee::All => Some("all".to_string()),
            Addressee::Single(name) => Some(name.clone()),
            Addressee::Multiple(names) => Some(names.join(",")),
            Addressee::LastRespondent => resolved_last.clone(),
        };
        self.messages.push(
            ChatMessage::user_with_addressee(body, addressee_str)
                .with_whisper_targets(whisper_targets),
        );

        self.save_session();

        self.pending_agents = router::resolve_dispatch_queue(
            &addressee,
            &self.config.settings.reply_order,
            &available,
            resolved_last.as_deref(),
        );

        let unavailable: Vec<String> = self
            .pending_agents
            .iter()
            .filter(|name| !self.agents.contains_key(name.as_str()))
            .cloned()
            .collect();
        if !unavailable.is_empty() {
            self.pending_agents
                .retain(|name| self.agents.contains_key(name.as_str()));
            let names = unavailable.join(", ");
            self.show_error(
                terminal,
                &format!("Agent unavailable (possibly missing API key): {names}"),
            )?;
            if self.pending_agents.is_empty() {
                return Ok(());
            }
        }

        self.start_next_agent(terminal)?;
        Ok(())
    }

    /// Start the next pending agent. Returns Ok(true) if an agent was started.
    pub(crate) fn start_next_agent(
        &mut self,
        _terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<bool> {
        while let Some(name) = self.pending_agents.pop_front() {
            // Adjust a2a insertion cursor when popping from the front.
            self.a2a_insert_cursor = self.a2a_insert_cursor.saturating_sub(1);
            if let Some(agent) = self.agents.get(&name) {
                // Build peer agent list for AI-to-AI prompt injection.
                let peers = if self.config.settings.agent_to_agent_max_rounds > 0 {
                    Some(
                        self.agents
                            .values()
                            .filter(|a| a.config.name != name)
                            .map(|a| krew_core::agent::PeerAgent {
                                name: a.config.name.clone(),
                                display_name: a.config.display_name.clone(),
                            })
                            .collect::<Vec<_>>(),
                    )
                } else {
                    None
                };
                let exclude_refs: Option<Vec<&str>> = self
                    .current_exclude_tools
                    .as_ref()
                    .map(|v| v.iter().map(|s| s.as_str()).collect());
                let rx = agent.start_completion(
                    self.messages.clone(),
                    self.project_instructions.as_deref(),
                    None,
                    peers.as_deref(),
                    self.current_whisper_targets.clone(),
                    exclude_refs.as_deref(),
                );
                self.agent_event_rx = Some(rx);
                return Ok(true);
            }
            // Agent not found — skip and try next.
        }
        Ok(false)
    }

    /// Insert user message with colored routing dots showing target agents.
    pub(crate) fn insert_user_message(
        &self,
        terminal: &mut custom_terminal::Terminal,
        target_names: &[&str],
        text: &str,
        is_whisper: bool,
    ) -> anyhow::Result<()> {
        let green_bold = Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD);

        let mut spans: Vec<Span<'static>> = vec![Span::styled("> ".to_string(), green_bold)];

        // Show lock icon for whisper messages.
        if is_whisper {
            spans.push(Span::styled(
                "\u{1F512}".to_string(), // 🔒
                Style::default().add_modifier(Modifier::BOLD),
            ));
        }

        if !target_names.is_empty() {
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

        let mut text_lines = text.lines();
        let first_text = text_lines.next().unwrap_or("");
        spans.push(Span::raw(first_text.to_string()));
        let mut lines: Vec<Line<'static>> = vec![Line::from(spans)];
        for line in text_lines {
            lines.push(Line::from(Span::raw(line.to_string())));
        }
        render::insert_lines(terminal, lines)
    }
}
