use chrono::{DateTime, Utc};
use std::path::PathBuf;

use crate::message::ChatMessage;

/// A multi-agent conversation session.
#[derive(Debug)]
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
