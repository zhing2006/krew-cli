//! Session and input history persistence.

use krew_core::persistence::{SessionSnapshot, build_session_file};

use super::App;

impl App {
    /// Save the current session to disk. Logs a warning on failure.
    /// Skips saving when in rewound state (fork semantics).
    pub(crate) fn save_session(&self) {
        if self.rewound {
            return;
        }
        let session_path = self.session_dir.join(format!("{}.toml", self.session_id));

        let agent_names: Vec<String> = self
            .config
            .agents
            .iter()
            .filter(|a| self.agents.contains_key(&a.name))
            .map(|a| a.name.clone())
            .collect();

        let snapshot = SessionSnapshot {
            session_id: &self.session_id,
            cwd: &self.cwd,
            agent_names,
            messages: &self.messages,
            token_usage: &self.agent_token_usage,
            created_at: self.session_created_at,
        };

        let session_file = build_session_file(&snapshot);

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
