//! Session TOML persistence: save, load, and list sessions.

use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::StorageError;

type Result<T> = std::result::Result<T, StorageError>;

// ── TOML serde structures ────────────────────────────────────────────

/// Top-level session file structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFile {
    pub session: SessionMeta,
    #[serde(default)]
    pub messages: Vec<MessageEntry>,
}

/// Session metadata stored in the `[session]` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub cwd: String,
    pub agents: Vec<String>,
    #[serde(default)]
    pub total_tokens_used: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A single message entry in the `[[messages]]` array.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEntry {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub addressee: Option<String>,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageEntry>,
    /// Tool calls made in this assistant message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallEntry>>,
    /// Tool call ID this tool-result message is responding to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Server-side tool uses (e.g. web_search) for display on resume.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub server_tool_uses: Vec<ServerToolUseEntry>,
    /// Whisper targets: agents that can see this message (TOML native array).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whisper_targets: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
}

/// A single tool call made by an assistant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallEntry {
    pub id: String,
    pub name: String,
    pub arguments: String,
    /// Opaque thought signature (Google thinking mode).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought_signature: Option<String>,
}

/// A server-side tool use (e.g. web_search) recorded for resume display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerToolUseEntry {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
}

/// Token usage for an assistant message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEntry {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Summary returned by `list_sessions()` for display.
#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub id: String,
    pub agents: Vec<String>,
    pub updated_at: DateTime<Utc>,
    pub first_message_preview: Option<String>,
    pub message_count: usize,
}

// ── Public API ───────────────────────────────────────────────────────

/// Save a session to a TOML file using atomic write (write .tmp then rename).
/// Creates the parent directory if it does not exist.
pub fn save_session(path: &Path, session: &SessionFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let toml_str =
        toml::to_string_pretty(session).map_err(|e| StorageError::Other(e.to_string()))?;

    let tmp_path = path.with_extension("toml.tmp");
    fs::write(&tmp_path, toml_str)?;
    fs::rename(&tmp_path, path)?;

    Ok(())
}

/// Load a session from a TOML file.
pub fn load_session(path: &Path) -> Result<SessionFile> {
    let content = fs::read_to_string(path)?;
    let session: SessionFile = toml::from_str(&content)?;
    Ok(session)
}

/// List all sessions in the given directory, sorted by `updated_at` descending.
/// Corrupted files are silently skipped.
pub fn list_sessions(dir: &Path) -> Result<Vec<SessionSummary>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut summaries = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str());
        if ext != Some("toml") {
            continue;
        }

        // Skip corrupted files.
        let session = match load_session(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let first_msg_preview = session.messages.iter().find(|m| m.role == "user").map(|m| {
            let preview: String = m.content.chars().take(40).collect();
            if m.content.chars().count() > 40 {
                format!("{preview}...")
            } else {
                preview
            }
        });

        summaries.push(SessionSummary {
            id: session.session.id,
            agents: session.session.agents,
            updated_at: session.session.updated_at,
            first_message_preview: first_msg_preview,
            message_count: session.messages.len(),
        });
    }

    // Sort by updated_at descending.
    summaries.sort_by_key(|s| std::cmp::Reverse(s.updated_at));

    Ok(summaries)
}
