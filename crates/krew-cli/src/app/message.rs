//! Message sending, user message rendering, and echo display.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use krew_core::router::{self, Addressee};

use crate::custom_terminal;
use crate::render;

use super::App;

impl App<'_> {
    /// Send the current input as a message or execute a slash command.
    pub(crate) fn send_message(
        &mut self,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
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
}
