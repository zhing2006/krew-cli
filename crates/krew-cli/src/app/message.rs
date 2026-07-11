//! Message sending and user message rendering.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use krew_core::command::SlashCommand;
use krew_core::router::{self, Addressee};
use krew_llm::{ChatMessage, ChatRole};

use crate::completion::{ActivePopup, CompletionState};
use crate::custom_terminal;
use crate::render;

use super::App;
use super::state::{MAX_PENDING_MESSAGES, PendingMessage};

/// Whisper targets of the most recent user message, if it was a whisper.
///
/// An untargeted follow-up continues that whisper group; a public user
/// message breaks the chain. Deriving from message history (instead of
/// per-round state) keeps this correct across resume, rewind, and drain
/// (round whisper state is cleared before pending messages drain).
fn whisper_continuation_targets(messages: &[ChatMessage]) -> Option<Vec<String>> {
    messages
        .iter()
        .rev()
        .find(|m| m.role == ChatRole::User)
        .and_then(|m| m.whisper_targets.clone())
        .filter(|targets| !targets.is_empty())
}

fn resolve_implicit_target(
    last_respondent: Option<&str>,
    reply_order: &[String],
    available: &std::collections::HashSet<String>,
) -> Option<String> {
    if let Some(name) = last_respondent {
        return Some(name.to_string());
    }
    reply_order
        .iter()
        .find(|name| available.contains(name.as_str()))
        .cloned()
}

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

        let trimmed = text.trim().to_string();
        tracing::debug!(input = %trimmed, "User sent message");

        // Try built-in slash command first.
        if trimmed.starts_with('/') && SlashCommand::from_input(&trimmed).is_some() {
            self.history_push(trimmed.clone());
            self.clear_textarea();
            return self.execute_slash_command(&trimmed, terminal);
        }

        // Try custom command — `/name args` where name is in the custom registry.
        if let Some(without_slash) = trimmed.strip_prefix('/') {
            let (cmd_part, args) = match without_slash.split_once(' ') {
                Some((c, a)) => (c, a.trim()),
                None => (without_slash, ""),
            };
            if let Some(cmd) = self.custom_commands.lookup(cmd_part) {
                let expanded = cmd.expand(args);
                self.pending_custom_command = Some(expanded);
                self.history_push(trimmed.clone());
                self.clear_textarea();
                return Ok(());
            }
            self.clear_textarea();
            return self.show_error(terminal, &format!("Unknown command: /{cmd_part}"));
        }

        self.clear_textarea();
        self.submit_raw_input(&trimmed, terminal)
    }

    /// Queue the current textarea content as a pending message.
    ///
    /// Validates that input is non-empty. Input without @/# addressing opens
    /// the pending-target picker popup instead of being enqueued directly.
    /// On success, clears the textarea. Otherwise, preserves textarea content.
    pub(crate) fn queue_message(
        &mut self,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
        let text = self.expanded_text();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        // Reject if queue is full (caller should check, but defensive).
        if self.pending_messages.len() >= MAX_PENDING_MESSAGES {
            return Ok(());
        }

        // Untargeted input (LastRespondent) opens the target picker instead of
        // enqueuing — unless a whisper round is being continued, in which case
        // the message drains back into the same whisper group and needs no picker.
        let agent_names: Vec<String> = self.config.agents.iter().map(|a| a.name.clone()).collect();
        match router::parse_input(trimmed, &agent_names) {
            Ok((Addressee::LastRespondent, _, _))
                if self.whisper_continuation_targets().is_none() =>
            {
                // No @/# target — open the agent picker instead of rejecting.
                if !self.open_pending_target_popup() {
                    self.show_error(
                        terminal,
                        "No agents available — use @name to specify a target agent",
                    )?;
                }
                return Ok(()); // Textarea preserved either way.
            }
            Err(e) => {
                self.show_error(terminal, &e.to_string())?;
                return Ok(());
            }
            Ok(_) => {} // Valid addressing, proceed.
        }

        self.pending_messages.push_back(PendingMessage {
            raw_input: trimmed.to_string(),
        });
        self.clear_textarea();
        Ok(())
    }

    /// Whisper targets to continue for an untargeted message, if the most
    /// recent user message was a whisper.
    pub(crate) fn whisper_continuation_targets(&self) -> Option<Vec<String>> {
        whisper_continuation_targets(&self.messages)
    }

    /// Open the pending-target picker popup for an untargeted queued message.
    ///
    /// Returns false if there are no candidates to show.
    pub(crate) fn open_pending_target_popup(&mut self) -> bool {
        let items = self.agent_name_items();
        if items.is_empty() {
            return false;
        }
        let mut state = CompletionState::new(items);
        // Default-highlight the agent currently speaking, falling back to the
        // last respondent when no completion is in flight.
        if let Some(name) = self
            .current_agent_name
            .clone()
            .or_else(|| self.last_respondent.clone())
        {
            state.select_value(&name);
        }
        self.popup = ActivePopup::PendingTarget(state);
        true
    }

    /// Confirm the pending-target popup: enqueue the message as `@{name} {text}`.
    ///
    /// Reads the textarea at confirm time (content may change via paste while
    /// the popup is open). If the round finished while the popup was open,
    /// the message is sent immediately instead of being queued.
    pub(crate) fn confirm_pending_target(
        &mut self,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
        let name = match &self.popup {
            ActivePopup::PendingTarget(state) => state.selected_item().map(|i| i.value.clone()),
            _ => None,
        };
        self.popup = ActivePopup::None;
        let Some(name) = name else {
            return Ok(());
        };

        let text = self.expanded_text();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        let raw = format!("@{name} {trimmed}");

        if self.agent_event_rx.is_some() {
            if self.pending_messages.len() < MAX_PENDING_MESSAGES {
                self.pending_messages
                    .push_back(PendingMessage { raw_input: raw });
                self.clear_textarea();
            }
            // Queue full is unreachable here (popup only opens with room);
            // defensively keep the textarea untouched.
        } else {
            // Round finished while the popup was open — send immediately.
            self.clear_textarea();
            self.submit_raw_input(&raw, terminal)?;
        }
        Ok(())
    }

    /// Drain one pending message from the queue and submit it.
    ///
    /// Called after all pending agents finish (Done/Error/Cancel paths).
    pub(crate) fn drain_pending_message(
        &mut self,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
        let pending = match self.pending_messages.pop_front() {
            Some(p) => p,
            None => return Ok(()),
        };
        self.submit_raw_input(&pending.raw_input, terminal)
    }

    /// Submit a raw input string as a user message (shared by send_message and drain).
    fn submit_raw_input(
        &mut self,
        trimmed: &str,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
        self.history_push(trimmed.to_string());
        self.submit_user_message(trimmed, terminal)
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

        self.submit_user_message(trimmed, terminal)
    }

    /// Submit already-trimmed user text through the shared routing pipeline.
    fn submit_user_message(
        &mut self,
        trimmed: &str,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
        // Reset AI-to-AI round counter on new user message.
        self.ai_conversation_rounds = 0;
        self.a2a_insert_cursor = 0;

        // Parse @ addressee.
        let agent_names: Vec<String> = self.config.agents.iter().map(|a| a.name.clone()).collect();
        let (addressee, body, is_whisper) = match router::parse_input(trimmed, &agent_names) {
            Ok(result) => result,
            Err(e) => {
                self.show_error(terminal, &e.to_string())?;
                return Ok(());
            }
        };

        // Whisper continuation: an untargeted message following a whisper
        // round stays within the same whisper group.
        let (addressee, is_whisper) = match addressee {
            Addressee::LastRespondent => match self.whisper_continuation_targets() {
                Some(mut targets) if targets.len() == 1 => {
                    (Addressee::Single(targets.remove(0)), true)
                }
                Some(targets) => (Addressee::Multiple(targets), true),
                None => (Addressee::LastRespondent, is_whisper),
            },
            other => (other, is_whisper),
        };

        let available: std::collections::HashSet<String> = self.agents.keys().cloned().collect();

        let resolved_last = match &addressee {
            Addressee::LastRespondent => resolve_implicit_target(
                self.last_respondent.as_deref(),
                &self.config.settings.reply_order,
                &available,
            ),
            _ => None,
        };

        if matches!(&addressee, Addressee::LastRespondent) && resolved_last.is_none() {
            self.show_error(
                terminal,
                "No agents available — use @name to specify a target agent",
            )?;
            return Ok(());
        }

        // Fork semantics.
        if self.rewound {
            self.session_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
            self.session_created_at = chrono::Utc::now();
            self.rewound = false;
        }

        let target_names = router::resolve_target_names(
            &addressee,
            &self.config.settings.reply_order,
            &available,
            resolved_last.as_deref(),
        );

        self.insert_user_message(terminal, &target_names, trimmed, is_whisper)?;

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

#[cfg(test)]
mod tests {
    use super::{resolve_implicit_target, whisper_continuation_targets};
    use krew_core::router::{self, Addressee};
    use krew_llm::{ChatMessage, ChatRole};
    use std::collections::HashSet;

    fn names(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn available(values: &[&str]) -> HashSet<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn implicit_target_prefers_last_respondent() {
        let target = resolve_implicit_target(
            Some("opus"),
            &names(&["gpt", "opus"]),
            &available(&["gpt", "opus"]),
        );

        assert_eq!(target.as_deref(), Some("opus"));
    }

    #[test]
    fn implicit_target_uses_first_reply_order_agent_without_last() {
        let target =
            resolve_implicit_target(None, &names(&["gpt", "opus"]), &available(&["gpt", "opus"]));

        assert_eq!(target.as_deref(), Some("gpt"));
    }

    #[test]
    fn implicit_target_skips_unavailable_reply_order_entries() {
        let target =
            resolve_implicit_target(None, &names(&["missing", "opus"]), &available(&["opus"]));

        assert_eq!(target.as_deref(), Some("opus"));
    }

    #[test]
    fn implicit_target_returns_none_without_available_agent() {
        let target = resolve_implicit_target(None, &names(&["missing"]), &available(&["opus"]));

        assert_eq!(target, None);
    }

    #[test]
    fn implicit_target_returns_none_with_empty_reply_order() {
        let target = resolve_implicit_target(None, &[], &available(&["gpt"]));

        assert_eq!(target, None);
    }

    #[test]
    fn pending_target_prefix_parses_as_single_addressee() {
        // Locks in the confirm_pending_target contract: prepending "@{name} "
        // to an untargeted input parses like a manually typed @name message,
        // with the full prefixed input kept as the body.
        let raw = format!("@{} {}", "opus", "hello there");
        let (addressee, body, is_whisper) =
            router::parse_input(&raw, &names(&["gpt", "opus"])).unwrap();

        assert!(matches!(addressee, Addressee::Single(name) if name == "opus"));
        assert_eq!(body, "@opus hello there");
        assert!(!is_whisper);
    }

    #[test]
    fn pending_target_prefix_supports_all() {
        let raw = format!("@{} {}", "all", "hello there");
        let (addressee, _, _) = router::parse_input(&raw, &names(&["gpt", "opus"])).unwrap();

        assert!(matches!(addressee, Addressee::All));
    }

    fn user_msg(whisper_targets: Option<&[&str]>) -> ChatMessage {
        ChatMessage::user_with_addressee("hi", Some("gpt".to_string()))
            .with_whisper_targets(whisper_targets.map(names))
    }

    fn assistant_msg(agent: &str) -> ChatMessage {
        ChatMessage::text(ChatRole::Assistant, "reply", Some(agent.to_string()))
    }

    #[test]
    fn whisper_continuation_returns_group_after_whisper_round() {
        let messages = vec![
            user_msg(Some(&["gpt", "opus"])),
            assistant_msg("gpt"),
            assistant_msg("opus"),
        ];

        let targets = whisper_continuation_targets(&messages);
        assert_eq!(targets, Some(names(&["gpt", "opus"])));
    }

    #[test]
    fn whisper_continuation_none_after_public_round() {
        let messages = vec![user_msg(None), assistant_msg("gpt")];

        assert_eq!(whisper_continuation_targets(&messages), None);
    }

    #[test]
    fn whisper_continuation_broken_by_later_public_message() {
        let messages = vec![
            user_msg(Some(&["opus"])),
            assistant_msg("opus"),
            user_msg(None),
            assistant_msg("gpt"),
        ];

        assert_eq!(whisper_continuation_targets(&messages), None);
    }

    #[test]
    fn whisper_continuation_ignores_empty_targets_and_history() {
        assert_eq!(whisper_continuation_targets(&[]), None);

        let messages = vec![user_msg(Some(&[]))];
        assert_eq!(whisper_continuation_targets(&messages), None);
    }
}
