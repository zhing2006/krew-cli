//! Message sending and user message rendering.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use krew_core::command::SlashCommand;
use krew_core::router::{self, Addressee};
use krew_llm::{ChatMessage, ChatRole};

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

        // Try slash command first — only if it matches a known command.
        // Unknown `/...` falls through and is treated as plain text.
        if trimmed.starts_with('/') && SlashCommand::from_input(trimmed).is_some() {
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

        // Resolve LastRespondent early so we can show colored dots.
        let resolved_last = match &addressee {
            Addressee::LastRespondent => self.last_respondent.clone(),
            _ => None,
        };

        // Task 3.5: Block if LastRespondent has no value.
        if matches!(&addressee, Addressee::LastRespondent) && resolved_last.is_none() {
            self.show_error(terminal, "还没有 Agent 回复过，请使用 @name 指定目标 Agent")?;
            self.clear_textarea();
            return Ok(());
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
        self.insert_user_message(terminal, &target_names, trimmed)?;

        // Add user message to conversation history.
        self.messages
            .push(ChatMessage::text(ChatRole::User, body, None));

        // Persist session after user message.
        self.save_session();

        // Build the agent dispatch queue via krew-core router.
        self.pending_agents = router::resolve_dispatch_queue(
            &addressee,
            &self.config.settings.reply_order,
            &available,
            resolved_last.as_deref(),
        );

        // Start the first agent in the queue.
        self.start_next_agent(terminal)?;

        self.clear_textarea();
        Ok(())
    }

    /// Start the next pending agent. Returns Ok(true) if an agent was started.
    pub(crate) fn start_next_agent(
        &mut self,
        _terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<bool> {
        while let Some(name) = self.pending_agents.pop_front() {
            if let Some(agent) = self.agents.get(&name) {
                let rx = agent.start_completion(
                    self.messages.clone(),
                    self.project_instructions.as_deref(),
                    None,
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
    ) -> anyhow::Result<()> {
        let green_bold = Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD);

        let mut spans: Vec<Span<'static>> = vec![Span::styled("> ".to_string(), green_bold)];

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
