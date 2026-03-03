//! Session and input history persistence.

use chrono::Utc;
use krew_storage::session_file::{MessageEntry, SessionFile, SessionMeta, UsageEntry};

use super::App;

impl App {
    /// Save the current session to disk. Logs a warning on failure.
    pub(crate) fn save_session(&self) {
        let session_path = self.session_dir.join(format!("{}.toml", self.session_id));

        // Build agent names from config.
        let agents: Vec<String> = self
            .config
            .agents
            .iter()
            .filter(|a| self.agents.contains_key(&a.name))
            .map(|a| a.name.clone())
            .collect();

        // Calculate total tokens.
        let total_tokens: u64 = self
            .agent_token_usage
            .values()
            .map(|(p, c)| (*p as u64) + (*c as u64))
            .sum();

        // Convert runtime messages to storage format.
        let messages: Vec<MessageEntry> = self
            .messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    krew_llm::ChatRole::System => "system",
                    krew_llm::ChatRole::User => "user",
                    krew_llm::ChatRole::Assistant => "assistant",
                    krew_llm::ChatRole::Tool => "tool",
                };

                // Reconstruct per-message usage from agent_token_usage for
                // assistant messages (approximate — we only have totals).
                let usage = if msg.role == krew_llm::ChatRole::Assistant {
                    msg.name.as_ref().and_then(|name| {
                        self.agent_token_usage.get(name).map(|(p, c)| UsageEntry {
                            prompt_tokens: *p,
                            completion_tokens: *c,
                            total_tokens: *p + *c,
                        })
                    })
                } else {
                    None
                };

                MessageEntry {
                    role: role.to_string(),
                    agent_name: msg.name.clone(),
                    addressee: None,
                    content: msg.content.clone(),
                    usage,
                    created_at: Utc::now(),
                }
            })
            .collect();

        let session_file = SessionFile {
            session: SessionMeta {
                id: self.session_id.clone(),
                cwd: self.cwd.display().to_string(),
                agents,
                total_tokens_used: total_tokens,
                created_at: Utc::now(), // Will be overwritten on load
                updated_at: Utc::now(),
            },
            messages,
        };

        if let Err(e) = krew_storage::session_file::save_session(&session_path, &session_file) {
            tracing::warn!(error = %e, "Failed to save session");
        }
    }

    /// Persist a single input history entry to disk.
    pub(crate) fn persist_history_entry(&self, entry: &str) {
        if let Err(e) = krew_storage::history_file::append_history_entry(&self.history_path, entry)
        {
            tracing::warn!(error = %e, "Failed to append history entry");
        }
    }
}
