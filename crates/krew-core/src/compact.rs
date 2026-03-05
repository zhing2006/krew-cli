//! Conversation history compaction: compress older messages into a summary.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::StreamExt;
use krew_llm::{ChatMessage, ChatRole, LlmClient, StreamEvent, Usage};
use krew_storage::session_file::SessionFile;

/// Result of a successful compaction.
pub struct CompactResult {
    /// The compressed summary text.
    pub summary: String,
    /// Number of messages before compaction.
    pub original_count: usize,
    /// Number of messages after compaction (summary + kept).
    pub new_count: usize,
    /// Path to the backup file.
    pub backup_path: PathBuf,
    /// Token usage from the compression LLM call.
    pub usage: Usage,
}

/// System prompt for the compression LLM call.
const COMPACT_SYSTEM_PROMPT: &str = "\
Compress the following conversation history into a concise summary. Preserve:
- Key decisions and conclusions
- Important context and constraints
- Action items and their status
- Technical details that would be needed for continuation

Be concise but comprehensive. The summary will replace the original messages.";

/// Split messages into conversation rounds.
///
/// A round is one user message plus all subsequent non-user messages
/// (assistant replies, tool calls, tool results) until the next user message.
/// Messages before the first user message are treated as round 0 (prefix).
fn split_into_rounds(messages: &[ChatMessage]) -> Vec<&[ChatMessage]> {
    let mut rounds = Vec::new();
    let mut start = 0;

    for (i, msg) in messages.iter().enumerate() {
        if msg.role == ChatRole::User && i > start {
            rounds.push(&messages[start..i]);
            start = i;
        }
    }

    // Last round (from last user message to end).
    if start < messages.len() {
        rounds.push(&messages[start..]);
    }

    rounds
}

/// Create a pre-compact backup of the session file.
fn create_backup(
    session_dir: &Path,
    session_id: &str,
    session_file: &SessionFile,
) -> anyhow::Result<PathBuf> {
    let timestamp = chrono::Utc::now().timestamp();
    let backup_name = format!("{session_id}.pre-compact.{timestamp}.toml");
    let backup_path = session_dir.join(&backup_name);
    krew_storage::session_file::save_session(&backup_path, session_file)?;
    Ok(backup_path)
}

/// Serialize messages to a text representation for the compression prompt.
fn messages_to_text(messages: &[&ChatMessage]) -> String {
    let mut text = String::new();
    for msg in messages {
        let role = match msg.role {
            ChatRole::System => "System",
            ChatRole::User => "User",
            ChatRole::Assistant => "Assistant",
            ChatRole::Tool => "Tool",
        };
        let prefix = match &msg.name {
            Some(name) => format!("[{role} ({name})]"),
            None => format!("[{role}]"),
        };
        text.push_str(&prefix);
        text.push('\n');
        // Truncate very long messages to avoid blowing up the compression prompt.
        const MAX_MSG_CHARS: usize = 2000;
        if msg.content.len() > MAX_MSG_CHARS {
            text.push_str(&msg.content[..MAX_MSG_CHARS]);
            text.push_str("... (truncated)");
        } else {
            text.push_str(&msg.content);
        }
        text.push_str("\n\n");
    }
    text
}

/// Compact the conversation history using an LLM to summarize older messages.
///
/// - `client`: LLM client to use for generating the summary.
/// - `messages`: Current conversation messages.
/// - `keep_rounds`: Number of recent conversation rounds to preserve.
/// - `session_dir`: Directory containing session files.
/// - `session_id`: Current session ID.
/// - `current_session_file`: The current session file for backup.
///
/// Returns `Ok(None)` if there is nothing to compact (too few rounds).
pub async fn compact_session(
    client: &Arc<dyn LlmClient>,
    messages: &[ChatMessage],
    keep_rounds: usize,
    session_dir: &Path,
    session_id: &str,
    current_session_file: &SessionFile,
) -> anyhow::Result<Option<CompactResult>> {
    let rounds = split_into_rounds(messages);

    if rounds.len() <= keep_rounds {
        return Ok(None);
    }

    // Split: compress older rounds, keep recent ones.
    let split_point = rounds.len() - keep_rounds;
    let compress_messages: Vec<&ChatMessage> = rounds[..split_point]
        .iter()
        .flat_map(|r| r.iter())
        .collect();
    let kept_messages: Vec<ChatMessage> = rounds[split_point..]
        .iter()
        .flat_map(|r| r.iter().cloned())
        .collect();

    // Create backup before modifying anything.
    let backup_path = create_backup(session_dir, session_id, current_session_file)?;

    // Build the compression request.
    let history_text = messages_to_text(&compress_messages);
    let compression_messages = vec![
        ChatMessage::text(ChatRole::System, COMPACT_SYSTEM_PROMPT, None),
        ChatMessage::text(ChatRole::User, history_text, None),
    ];

    // Call LLM to generate summary (consume the stream).
    let sampling = krew_config::SamplingConfig::default();
    let stream = client
        .chat_stream(&compression_messages, &[], &sampling, None)
        .await
        .map_err(|e| anyhow::anyhow!("Compact LLM call failed: {e}"))?;

    let (summary, usage) = consume_stream_to_text(stream).await?;

    if summary.trim().is_empty() {
        return Err(anyhow::anyhow!("Compact produced empty summary"));
    }

    let original_count = messages.len();

    // Build new message list: summary as user message + kept messages.
    let mut new_messages = Vec::with_capacity(1 + kept_messages.len());
    new_messages.push(ChatMessage::text(
        ChatRole::User,
        format!("[Session History Summary]\n{summary}"),
        None,
    ));
    new_messages.extend(kept_messages);

    let new_count = new_messages.len();

    Ok(Some(CompactResult {
        summary: summary.clone(),
        original_count,
        new_count,
        backup_path,
        usage,
    }))
}

/// Consume an LLM stream to completion, collecting all text output.
async fn consume_stream_to_text(
    stream: std::pin::Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>,
) -> anyhow::Result<(String, Usage)> {
    let mut text = String::new();
    let mut usage = Usage::default();

    let mut stream = stream;
    while let Some(event) = stream.next().await {
        match event {
            StreamEvent::TextDelta(delta) => text.push_str(&delta),
            StreamEvent::Done(u) => {
                usage = u;
                break;
            }
            StreamEvent::Error(msg) => return Err(anyhow::anyhow!("LLM stream error: {msg}")),
            _ => {} // Ignore thinking, tool calls, etc.
        }
    }

    Ok((text, usage))
}

/// Build the new message list after compaction.
///
/// Returns the messages that should replace the current session messages.
pub fn build_compacted_messages(
    messages: &[ChatMessage],
    keep_rounds: usize,
    summary: &str,
) -> Vec<ChatMessage> {
    let rounds = split_into_rounds(messages);
    let split_point = rounds.len().saturating_sub(keep_rounds);

    let kept_messages: Vec<ChatMessage> = rounds[split_point..]
        .iter()
        .flat_map(|r| r.iter().cloned())
        .collect();

    let mut new_messages = Vec::with_capacity(1 + kept_messages.len());
    new_messages.push(ChatMessage::text(
        ChatRole::User,
        format!("[Session History Summary]\n{summary}"),
        None,
    ));
    new_messages.extend(kept_messages);
    new_messages
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_msg(content: &str) -> ChatMessage {
        ChatMessage::text(ChatRole::User, content, None)
    }

    fn assistant_msg(content: &str, name: &str) -> ChatMessage {
        ChatMessage::text(ChatRole::Assistant, content, Some(name.to_string()))
    }

    fn tool_msg(content: &str) -> ChatMessage {
        ChatMessage {
            role: ChatRole::Tool,
            content: content.to_string(),
            name: Some("read_file".to_string()),
            tool_calls: None,
            tool_call_id: Some("call_1".to_string()),
            server_tool_uses: Vec::new(),
        }
    }

    #[test]
    fn test_split_into_rounds_basic() {
        let messages = vec![
            user_msg("hello"),
            assistant_msg("hi", "gpt"),
            user_msg("how are you"),
            assistant_msg("fine", "gpt"),
        ];

        let rounds = split_into_rounds(&messages);
        assert_eq!(rounds.len(), 2);
        assert_eq!(rounds[0].len(), 2); // user + assistant
        assert_eq!(rounds[1].len(), 2); // user + assistant
    }

    #[test]
    fn test_split_into_rounds_with_tool_calls() {
        let messages = vec![
            user_msg("read file"),
            assistant_msg("calling tool", "gpt"),
            tool_msg("file content"),
            assistant_msg("done", "gpt"),
            user_msg("thanks"),
            assistant_msg("welcome", "gpt"),
        ];

        let rounds = split_into_rounds(&messages);
        assert_eq!(rounds.len(), 2);
        assert_eq!(rounds[0].len(), 4); // user + assistant + tool + assistant
        assert_eq!(rounds[1].len(), 2); // user + assistant
    }

    #[test]
    fn test_build_compacted_messages() {
        let messages = vec![
            user_msg("hello"),
            assistant_msg("hi", "gpt"),
            user_msg("how are you"),
            assistant_msg("fine", "gpt"),
            user_msg("goodbye"),
            assistant_msg("bye", "gpt"),
        ];

        let result = build_compacted_messages(&messages, 2, "Earlier conversation about greetings");
        assert_eq!(result.len(), 5); // summary + 2 rounds (2 msgs each)
        assert!(result[0].content.contains("[Session History Summary]"));
        assert_eq!(result[1].content, "how are you");
    }

    #[test]
    fn test_build_compacted_messages_too_few_rounds() {
        let messages = vec![user_msg("hello"), assistant_msg("hi", "gpt")];

        let result = build_compacted_messages(&messages, 5, "summary");
        // Only 1 round, keep_rounds=5 → all kept + summary
        assert_eq!(result.len(), 3); // summary + original 2
    }
}
