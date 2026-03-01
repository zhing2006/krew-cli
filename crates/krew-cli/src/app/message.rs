//! Message sending, user message rendering, and echo display.

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

        // Resolve target agent names for colored dots on user message.
        let target_names: Vec<&str> = match &addressee {
            Addressee::All => self.config.agents.iter().map(|a| a.name.as_str()).collect(),
            Addressee::Single(name) => vec![name.as_str()],
            Addressee::Multiple(names) => names.iter().map(|n| n.as_str()).collect(),
            Addressee::LastRespondent => vec![],
        };

        // Insert user message with colored routing dots: > ●●● message
        self.insert_user_message(terminal, &target_names, trimmed)?;

        // Add user message to conversation history.
        self.messages.push(ChatMessage {
            role: ChatRole::User,
            content: body.to_string(),
            name: None,
        });

        // Determine which agent to call.
        let target_agent = match &addressee {
            Addressee::Single(name) => Some(name.clone()),
            Addressee::LastRespondent => {
                // Use the first agent with an LLM client, or fall back to config order.
                self.config
                    .agents
                    .iter()
                    .find(|a| self.agents.contains_key(&a.name))
                    .map(|a| a.name.clone())
            }
            // Phase 4: @all and @multiple not yet supported (Phase 6).
            // For now, use the first agent in reply_order that has a client.
            Addressee::All | Addressee::Multiple(_) => self
                .config
                .settings
                .reply_order
                .iter()
                .find(|name| self.agents.contains_key(*name))
                .cloned(),
        };

        if let Some(ref name) = target_agent {
            if let Some(agent) = self.agents.get(name) {
                // Start completion immediately — the HTTP request runs in a
                // spawned task so the event loop is not blocked.
                let rx = agent
                    .start_completion(self.messages.clone(), self.project_instructions.as_deref());
                self.agent_event_rx = Some(rx);
            } else {
                // Builtin echo fallback.
                self.echo_reply(terminal, &addressee, &body)?;
            }
        } else {
            // No LLM agents available — echo.
            self.echo_reply(terminal, &addressee, &body)?;
        }

        self.clear_textarea();
        Ok(())
    }

    /// Echo reply with yellow diamond prefix (for builtin agents).
    fn echo_reply(
        &self,
        terminal: &mut custom_terminal::Terminal,
        addressee: &Addressee,
        body: &str,
    ) -> anyhow::Result<()> {
        let route_tag = match addressee {
            Addressee::All => "[→ @all]".to_string(),
            Addressee::Single(name) => format!("[→ @{name}]"),
            Addressee::Multiple(names) => {
                let joined = names.iter().map(|n| format!("@{n}")).collect::<Vec<_>>();
                format!("[→ {}]", joined.join(" "))
            }
            Addressee::LastRespondent => "[→ last]".to_string(),
        };

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
        render::insert_lines(terminal, echo_lines)
    }

    /// Insert user message with colored routing dots showing target agents.
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
