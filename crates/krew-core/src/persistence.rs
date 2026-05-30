//! Bridge between runtime session state and TOML persistence.

use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use krew_llm::{ChatMessage, ChatRole, ThinkingBlock, ToolCallInfo};
use krew_storage::session_file::{
    MessageEntry, SessionFile, SessionMeta, ThinkingBlockEntry, ToolCallEntry, UsageEntry,
};

/// Map runtime `ThinkingBlock`s into their persisted form.
///
/// Returns `None` for empty input so the TOML serializer omits the key,
/// keeping pre-thinking session files visually unchanged.
fn thinking_blocks_to_entries(blocks: &[ThinkingBlock]) -> Option<Vec<ThinkingBlockEntry>> {
    if blocks.is_empty() {
        return None;
    }
    Some(
        blocks
            .iter()
            .map(|b| match b {
                ThinkingBlock::Thinking { text, signature } => ThinkingBlockEntry::Thinking {
                    text: text.clone(),
                    signature: signature.clone(),
                },
                ThinkingBlock::Redacted { data } => {
                    ThinkingBlockEntry::RedactedThinking { data: data.clone() }
                }
            })
            .collect(),
    )
}

/// Reverse of `thinking_blocks_to_entries`.
fn thinking_blocks_from_entries(entries: Option<&Vec<ThinkingBlockEntry>>) -> Vec<ThinkingBlock> {
    entries
        .map(|list| {
            list.iter()
                .map(|e| match e {
                    ThinkingBlockEntry::Thinking { text, signature } => ThinkingBlock::Thinking {
                        text: text.clone(),
                        signature: signature.clone(),
                    },
                    ThinkingBlockEntry::RedactedThinking { data } => {
                        ThinkingBlock::Redacted { data: data.clone() }
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Serialize raw ordered content blocks into a single JSON string.
///
/// Returns `None` for empty input so the TOML serializer omits the key,
/// keeping session files without raw blocks visually unchanged.
fn raw_content_blocks_to_json(blocks: &[serde_json::Value]) -> Option<String> {
    if blocks.is_empty() {
        return None;
    }
    serde_json::to_string(blocks).ok()
}

/// Reverse of `raw_content_blocks_to_json`.
///
/// Malformed or absent JSON falls back to an empty vec, which makes
/// `convert_messages` rebuild the assistant content from flattened fields.
fn raw_content_blocks_from_json(json: Option<&String>) -> Vec<serde_json::Value> {
    json.and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default()
}

/// Runtime session state needed for serialization.
///
/// A snapshot of the fields required to persist a session, extracted from
/// the TUI's `App` struct. Using a reference struct avoids coupling to App.
pub struct SessionSnapshot<'a> {
    /// Session identifier.
    pub session_id: &'a str,
    /// Working directory path.
    pub cwd: &'a Path,
    /// Names of active agents (those with LLM clients).
    pub agent_names: Vec<String>,
    /// Conversation message history.
    pub messages: &'a [ChatMessage],
    /// Accumulated token usage per agent: name -> (prompt, completion).
    pub token_usage: &'a HashMap<String, (u32, u32)>,
    /// Session creation timestamp (preserved across saves).
    pub created_at: DateTime<Utc>,
}

/// Build a `SessionFile` from runtime session state.
pub fn build_session_file(snapshot: &SessionSnapshot) -> SessionFile {
    let total_tokens: u64 = snapshot
        .token_usage
        .values()
        .map(|(p, c)| (*p as u64) + (*c as u64))
        .sum();

    let messages: Vec<MessageEntry> = snapshot
        .messages
        .iter()
        .map(|msg| {
            let role = match msg.role {
                ChatRole::System => "system",
                ChatRole::User => "user",
                ChatRole::Assistant => "assistant",
                ChatRole::Tool => "tool",
            };

            let usage = if msg.role == ChatRole::Assistant {
                // Prefer per-message usage; fall back to aggregated total for backward compat.
                msg.usage
                    .as_ref()
                    .map(|u| UsageEntry {
                        prompt_tokens: u.prompt_tokens,
                        completion_tokens: u.completion_tokens,
                        total_tokens: u.total_tokens,
                    })
                    .or_else(|| {
                        msg.name.as_ref().and_then(|name| {
                            snapshot.token_usage.get(name).map(|(p, c)| UsageEntry {
                                prompt_tokens: *p,
                                completion_tokens: *c,
                                total_tokens: *p + *c,
                            })
                        })
                    })
            } else {
                None
            };

            MessageEntry {
                role: role.to_string(),
                agent_name: msg.name.clone(),
                addressee: msg.addressee.clone(),
                content: msg.content.clone(),
                usage,
                tool_calls: msg.tool_calls.as_ref().map(|tcs| {
                    tcs.iter()
                        .map(|tc| ToolCallEntry {
                            id: tc.id.clone(),
                            name: tc.name.clone(),
                            arguments: tc.arguments.clone(),
                            thought_signature: tc.thought_signature.clone(),
                        })
                        .collect()
                }),
                tool_call_id: msg.tool_call_id.clone(),
                server_tool_uses: msg
                    .server_tool_uses
                    .iter()
                    .map(|s| krew_storage::session_file::ServerToolUseEntry {
                        name: s.name.clone(),
                        query: s.query.clone(),
                    })
                    .collect(),
                whisper_targets: msg.whisper_targets.clone(),
                thinking_blocks: thinking_blocks_to_entries(&msg.thinking_blocks),
                raw_content_blocks_json: raw_content_blocks_to_json(&msg.raw_content_blocks),
                created_at: msg.created_at,
            }
        })
        .collect();

    SessionFile {
        session: SessionMeta {
            id: snapshot.session_id.to_string(),
            cwd: snapshot.cwd.display().to_string(),
            agents: snapshot.agent_names.clone(),
            total_tokens_used: total_tokens,
            created_at: snapshot.created_at,
            updated_at: Utc::now(),
        },
        messages,
    }
}

/// Result of restoring a session from disk.
pub struct RestoredSession {
    /// The session ID.
    pub session_id: String,
    /// Restored conversation messages in runtime format.
    pub messages: Vec<ChatMessage>,
    /// Restored token usage per agent: name -> (prompt, completion).
    pub token_usage: HashMap<String, (u32, u32)>,
    /// Name of the last assistant that responded (for LastRespondent routing).
    pub last_respondent: Option<String>,
    /// The raw session file data (for TUI to replay display).
    pub session_file: SessionFile,
    /// Original session creation timestamp.
    pub session_created_at: DateTime<Utc>,
}

/// Load a session from disk and convert it to runtime types.
///
/// Returns the restored session state. The caller is responsible for
/// replaying messages on screen (TUI concern).
pub fn load_session_from_disk(session_path: &Path) -> anyhow::Result<RestoredSession> {
    let session_file = krew_storage::session_file::load_session(session_path)
        .map_err(|e| anyhow::anyhow!("Failed to load session: {e}"))?;

    let mut messages = Vec::new();
    let mut last_respondent = None;

    for msg in &session_file.messages {
        let role = match msg.role.as_str() {
            "system" => ChatRole::System,
            "user" => ChatRole::User,
            "assistant" => ChatRole::Assistant,
            "tool" => ChatRole::Tool,
            _ => continue,
        };

        if role == ChatRole::Assistant
            && let Some(name) = &msg.agent_name
        {
            last_respondent = Some(name.clone());
        }

        messages.push(ChatMessage {
            role,
            content: msg.content.clone(),
            name: msg.agent_name.clone(),
            tool_calls: msg.tool_calls.as_ref().map(|tcs| {
                tcs.iter()
                    .map(|tc| ToolCallInfo {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        arguments: tc.arguments.clone(),
                        thought_signature: tc.thought_signature.clone(),
                    })
                    .collect()
            }),
            tool_call_id: msg.tool_call_id.clone(),
            server_tool_uses: msg
                .server_tool_uses
                .iter()
                .map(|s| krew_llm::ServerToolUseInfo {
                    name: s.name.clone(),
                    query: s.query.clone(),
                })
                .collect(),
            addressee: msg.addressee.clone(),
            whisper_targets: msg.whisper_targets.clone(),
            created_at: msg.created_at,
            usage: msg.usage.as_ref().map(|u| krew_llm::Usage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            }),
            images: Vec::new(),
            thinking_blocks: thinking_blocks_from_entries(msg.thinking_blocks.as_ref()),
            raw_content_blocks: raw_content_blocks_from_json(msg.raw_content_blocks_json.as_ref()),
        });
    }

    // Restore token usage: take the last occurrence per agent.
    let mut token_usage = HashMap::new();
    for msg in session_file.messages.iter().rev() {
        if msg.role == "assistant"
            && let (Some(name), Some(usage)) = (&msg.agent_name, &msg.usage)
        {
            token_usage
                .entry(name.clone())
                .or_insert((usage.prompt_tokens, usage.completion_tokens));
        }
    }

    let session_created_at = session_file.session.created_at;

    Ok(RestoredSession {
        session_id: session_file.session.id.clone(),
        messages,
        token_usage,
        last_respondent,
        session_created_at,
        session_file,
    })
}

/// Load input history from disk, applying the configured entry limit.
///
/// If the history exceeds `limit`, it is truncated to the most recent
/// entries and the file is rewritten.
pub fn load_and_truncate_history(path: &Path, limit: usize) -> Vec<String> {
    match krew_storage::history_file::load_history(path) {
        Ok(mut entries) => {
            if entries.len() > limit {
                entries = entries.split_off(entries.len() - limit);
                if let Err(e) = krew_storage::history_file::save_history(path, &entries) {
                    tracing::warn!(error = %e, "Failed to truncate history file");
                }
            }
            entries
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to load input history");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use krew_storage::session_file::save_session;
    use tempfile::TempDir;

    #[test]
    fn raw_json_helpers_roundtrip() {
        let blocks = vec![serde_json::json!({"type": "text", "text": "hi"})];
        let json = raw_content_blocks_to_json(&blocks).unwrap();
        assert_eq!(raw_content_blocks_from_json(Some(&json)), blocks);

        // Empty input serializes to None and restores to an empty vec.
        assert!(raw_content_blocks_to_json(&[]).is_none());
        assert!(raw_content_blocks_from_json(None).is_empty());

        // Malformed JSON degrades gracefully to an empty vec.
        assert!(raw_content_blocks_from_json(Some(&"not json".to_string())).is_empty());
    }

    #[test]
    fn raw_content_blocks_survive_save_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("raw_roundtrip.toml");

        // Assistant turn with interleaved raw blocks (thinking → server_tool_use
        // → web_search_tool_result → text), including encrypted_content that the
        // flattened fields cannot reconstruct.
        let raw = vec![
            serde_json::json!({"type": "thinking", "thinking": "let me search", "signature": "sig-abc"}),
            serde_json::json!({"type": "server_tool_use", "id": "stu_1", "name": "web_search", "input": {"query": "rust"}}),
            serde_json::json!({"type": "web_search_tool_result", "tool_use_id": "stu_1", "content": [{"type": "web_search_result", "encrypted_content": "ENC=="}]}),
            serde_json::json!({"type": "text", "text": "Here is the answer"}),
        ];
        let mut assistant = ChatMessage::text(
            ChatRole::Assistant,
            "Here is the answer",
            Some("opus".to_string()),
        );
        assistant.raw_content_blocks = raw.clone();

        let messages = vec![
            ChatMessage::user_with_addressee("question", Some("opus".to_string())),
            assistant,
        ];

        let token_usage = HashMap::new();
        let snapshot = SessionSnapshot {
            session_id: "raw123",
            cwd: Path::new("/tmp/project"),
            agent_names: vec!["opus".to_string()],
            messages: &messages,
            token_usage: &token_usage,
            created_at: Utc::now(),
        };

        let session_file = build_session_file(&snapshot);
        save_session(&path, &session_file).unwrap();

        let restored = load_session_from_disk(&path).unwrap();
        assert_eq!(restored.messages.len(), 2);
        // User message carries no raw blocks.
        assert!(restored.messages[0].raw_content_blocks.is_empty());
        // Assistant raw blocks are restored verbatim, preserving order and
        // encrypted_content.
        assert_eq!(restored.messages[1].raw_content_blocks, raw);
    }

    #[test]
    fn empty_raw_content_blocks_omits_key_from_toml() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("no_raw.toml");

        let messages = vec![ChatMessage::text(
            ChatRole::Assistant,
            "plain",
            Some("opus".to_string()),
        )];
        let token_usage = HashMap::new();
        let snapshot = SessionSnapshot {
            session_id: "noraw",
            cwd: Path::new("/tmp"),
            agent_names: vec!["opus".to_string()],
            messages: &messages,
            token_usage: &token_usage,
            created_at: Utc::now(),
        };
        let session_file = build_session_file(&snapshot);
        save_session(&path, &session_file).unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            !text.contains("raw_content_blocks_json"),
            "empty raw blocks must not appear in TOML, got:\n{text}"
        );

        // A session without the key restores to empty raw blocks.
        let restored = load_session_from_disk(&path).unwrap();
        assert!(restored.messages[0].raw_content_blocks.is_empty());
    }
}
