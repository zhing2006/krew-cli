use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::message::ChatMessage;

/// A multi-agent conversation session.
#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier (UUID).
    pub id: String,
    /// Working directory for this session (tool path boundary).
    pub cwd: PathBuf,
    /// Names of agents participating in this session.
    pub agents: Vec<String>,
    /// Complete message history.
    pub messages: Vec<ChatMessage>,
    /// Cumulative token usage across all agents.
    pub total_tokens_used: u64,
    /// When the session was created.
    pub created_at: DateTime<Utc>,
    /// When the session was last active.
    pub updated_at: DateTime<Utc>,
}

impl Session {
    /// Create a new session with a generated UUID and current timestamps.
    pub fn new(cwd: PathBuf, agents: Vec<String>) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
            cwd,
            agents,
            messages: Vec::new(),
            total_tokens_used: 0,
            created_at: now,
            updated_at: now,
        }
    }
}
