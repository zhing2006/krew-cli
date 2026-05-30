//! Conversation history compaction: compress older messages into a summary.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::StreamExt;
use krew_llm::{ChatMessage, ChatRole, LlmClient, StreamEvent, Usage};
use krew_storage::session_file::SessionFile;

/// Result of a successful compaction.
pub struct CompactResult {
    /// The compacted message list, ready to replace the original.
    pub messages: Vec<ChatMessage>,
    /// Number of messages before compaction.
    pub original_count: usize,
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
        // Use floor_char_boundary to avoid panicking on multi-byte UTF-8 chars.
        const MAX_MSG_CHARS: usize = 2000;
        if msg.content.len() > MAX_MSG_CHARS {
            let boundary = msg.content.floor_char_boundary(MAX_MSG_CHARS);
            text.push_str(&msg.content[..boundary]);
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
    // Exclude whisper messages from compression input (privacy preservation).
    let compress_messages: Vec<&ChatMessage> = rounds[..split_point]
        .iter()
        .flat_map(|r| r.iter())
        .filter(|m| m.whisper_targets.is_none())
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
    let compacted = build_compacted_messages(messages, keep_rounds, &summary);

    Ok(Some(CompactResult {
        messages: compacted,
        original_count,
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

/// Skill content marker used in `<skill_content>` tags by activate_skill tool.
const SKILL_CONTENT_TAG: &str = "<skill_content";

/// Check whether a message is an activate_skill tool call.
fn is_skill_tool_call(msg: &ChatMessage) -> bool {
    msg.role == ChatRole::Assistant
        && msg
            .tool_calls
            .as_ref()
            .is_some_and(|calls| calls.iter().any(|tc| tc.name == "activate_skill"))
}

/// Check whether a message is a skill activation tool result.
fn is_skill_tool_result(msg: &ChatMessage) -> bool {
    msg.role == ChatRole::Tool && msg.content.contains(SKILL_CONTENT_TAG)
}

/// Extract skill activation message blocks from a slice of messages.
///
/// Returns cloned messages that form complete skill activation pairs
/// (assistant tool_call + tool result). These are preserved across compaction
/// so the model retains activated skill instructions.
fn extract_skill_messages(messages: &[ChatMessage]) -> Vec<ChatMessage> {
    let mut protected = Vec::new();
    let mut i = 0;

    while i < messages.len() {
        if is_skill_tool_call(&messages[i]) {
            // Collect this assistant message and all following tool results
            // that are skill content (handles single and multi-tool-call cases).
            protected.push(messages[i].clone());
            let mut j = i + 1;
            while j < messages.len() && is_skill_tool_result(&messages[j]) {
                protected.push(messages[j].clone());
                j += 1;
            }
        }
        i += 1;
    }

    protected
}

/// Extract whisper messages from a slice of messages.
///
/// Returns cloned messages that have `whisper_targets` set. These are
/// preserved across compaction so whisper privacy is maintained.
fn extract_whisper_messages(messages: &[ChatMessage]) -> Vec<ChatMessage> {
    messages
        .iter()
        .filter(|m| m.whisper_targets.is_some())
        .cloned()
        .collect()
}

/// Build the new message list after compaction.
///
/// Returns the messages that should replace the current session messages.
/// Skill activation messages and whisper messages from the compressed region
/// are preserved and inserted between the summary and the kept rounds.
pub fn build_compacted_messages(
    messages: &[ChatMessage],
    keep_rounds: usize,
    summary: &str,
) -> Vec<ChatMessage> {
    let rounds = split_into_rounds(messages);
    let split_point = rounds.len().saturating_sub(keep_rounds);

    // Collect compressed portion as owned messages.
    let compressed_owned: Vec<ChatMessage> = rounds[..split_point]
        .iter()
        .flat_map(|r| r.iter().cloned())
        .collect();

    // Extract skill activation messages from the compressed portion.
    let skill_messages = extract_skill_messages(&compressed_owned);

    // Extract whisper messages from the compressed portion.
    let whisper_messages = extract_whisper_messages(&compressed_owned);

    let kept_messages: Vec<ChatMessage> = rounds[split_point..]
        .iter()
        .flat_map(|r| r.iter().cloned())
        .collect();

    // Also check kept messages for skills (avoid duplicates).
    let kept_skill_names: std::collections::HashSet<String> = kept_messages
        .iter()
        .filter(|m| is_skill_tool_result(m))
        .filter_map(|m| {
            // Extract skill name from <skill_content name="...">
            m.content.find("name=\"").and_then(|start| {
                let rest = &m.content[start + 6..];
                rest.find('"').map(|end| rest[..end].to_string())
            })
        })
        .collect();

    // Filter out skill messages already present in kept rounds.
    // Must filter as complete blocks (assistant tool_call + following tool results)
    // to avoid orphaned tool_call messages without matching results.
    let unique_skill_messages: Vec<ChatMessage> = if kept_skill_names.is_empty() {
        skill_messages
    } else {
        filter_duplicate_skill_blocks(skill_messages, &kept_skill_names)
    };

    let mut new_messages = Vec::with_capacity(
        1 + unique_skill_messages.len() + whisper_messages.len() + kept_messages.len(),
    );
    new_messages.push(ChatMessage::text(
        ChatRole::User,
        format!("[Session History Summary]\n{summary}"),
        None,
    ));
    // Insert protected skill messages: User(summary) → Assistant(tool_call) → Tool(result) → ...
    new_messages.extend(unique_skill_messages);
    // Insert preserved whisper messages after skill messages.
    new_messages.extend(whisper_messages);
    new_messages.extend(kept_messages);
    new_messages
}

/// Filter skill message blocks, removing blocks whose skill is already in kept rounds.
///
/// Processes messages as complete blocks: each block starts with an assistant
/// tool_call message followed by zero or more tool result messages. If the
/// tool_call's skill name is in `duplicates`, the entire block is dropped.
fn filter_duplicate_skill_blocks(
    skill_messages: Vec<ChatMessage>,
    duplicates: &std::collections::HashSet<String>,
) -> Vec<ChatMessage> {
    let mut filtered = Vec::new();
    let mut i = 0;

    while i < skill_messages.len() {
        if is_skill_tool_call(&skill_messages[i]) {
            // Extract skill name from tool call arguments.
            let skill_name = skill_messages[i]
                .tool_calls
                .as_ref()
                .and_then(|calls| calls.iter().find(|tc| tc.name == "activate_skill"))
                .and_then(|tc| {
                    serde_json::from_str::<serde_json::Value>(&tc.arguments)
                        .ok()
                        .and_then(|v| v.get("name")?.as_str().map(String::from))
                });

            let is_dup = skill_name.as_ref().is_some_and(|n| duplicates.contains(n));

            // Collect the block: tool_call + following tool results.
            let block_start = i;
            i += 1;
            while i < skill_messages.len() && is_skill_tool_result(&skill_messages[i]) {
                i += 1;
            }

            if !is_dup {
                filtered.extend_from_slice(&skill_messages[block_start..i]);
            }
        } else {
            i += 1;
        }
    }

    filtered
}

#[cfg(test)]
mod tests {
    use super::*;
    use krew_llm::ToolCallInfo;

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
            addressee: None,
            whisper_targets: None,
            created_at: chrono::Utc::now(),
            usage: None,
            images: Vec::new(),
            thinking_blocks: Vec::new(),
            raw_content_blocks: Vec::new(),
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

    fn skill_tool_call_msg(skill_name: &str, agent: &str) -> ChatMessage {
        ChatMessage {
            role: ChatRole::Assistant,
            content: String::new(),
            name: Some(agent.to_string()),
            tool_calls: Some(vec![ToolCallInfo {
                id: format!("call_{skill_name}"),
                name: "activate_skill".to_string(),
                arguments: format!("{{\"name\":\"{skill_name}\"}}"),
                thought_signature: None,
            }]),
            tool_call_id: None,
            server_tool_uses: Vec::new(),
            addressee: None,
            whisper_targets: None,
            created_at: chrono::Utc::now(),
            usage: None,
            images: Vec::new(),
            thinking_blocks: Vec::new(),
            raw_content_blocks: Vec::new(),
        }
    }

    fn skill_tool_result_msg(skill_name: &str) -> ChatMessage {
        ChatMessage {
            role: ChatRole::Tool,
            content: format!(
                "<skill_content name=\"{skill_name}\">\n# Skill instructions\nDo something.\n</skill_content>"
            ),
            name: Some("activate_skill".to_string()),
            tool_calls: None,
            tool_call_id: Some(format!("call_{skill_name}")),
            server_tool_uses: Vec::new(),
            addressee: None,
            whisper_targets: None,
            created_at: chrono::Utc::now(),
            usage: None,
            images: Vec::new(),
            thinking_blocks: Vec::new(),
            raw_content_blocks: Vec::new(),
        }
    }

    #[test]
    fn test_build_compacted_preserves_skill_messages() {
        let messages = vec![
            // Round 1: user asks, agent activates skill
            user_msg("review my code"),
            skill_tool_call_msg("code-review", "gpt"),
            skill_tool_result_msg("code-review"),
            assistant_msg("I'll review using the skill instructions.", "gpt"),
            // Round 2
            user_msg("how about this file?"),
            assistant_msg("looks good", "gpt"),
            // Round 3 (will be kept)
            user_msg("thanks"),
            assistant_msg("you're welcome", "gpt"),
        ];

        // keep_rounds=1 → rounds 1 and 2 get compressed, round 3 kept
        let result = build_compacted_messages(&messages, 1, "Reviewed code with skill");

        // Should be: summary + skill_tool_call + skill_result + kept round 3
        assert_eq!(result.len(), 5);
        assert!(result[0].content.contains("[Session History Summary]"));
        // Skill messages preserved.
        assert!(
            result[1]
                .tool_calls
                .as_ref()
                .is_some_and(|tc| tc[0].name == "activate_skill")
        );
        assert!(result[2].content.contains("<skill_content"));
        // Kept messages follow.
        assert_eq!(result[3].content, "thanks");
        assert_eq!(result[4].content, "you're welcome");
    }

    #[test]
    fn test_build_compacted_no_duplicate_skills() {
        let messages = vec![
            // Round 1: activate skill
            user_msg("review"),
            skill_tool_call_msg("review", "gpt"),
            skill_tool_result_msg("review"),
            assistant_msg("reviewed", "gpt"),
            // Round 2: activate same skill again (kept round)
            user_msg("review again"),
            skill_tool_call_msg("review", "gpt"),
            skill_tool_result_msg("review"),
            assistant_msg("reviewed again", "gpt"),
        ];

        // keep_rounds=1 → round 1 compressed, round 2 kept
        let result = build_compacted_messages(&messages, 1, "Earlier review");

        // Skill already in kept round, so compressed skill should NOT be duplicated.
        let skill_result_count = result.iter().filter(|m| is_skill_tool_result(m)).count();
        assert_eq!(skill_result_count, 1); // Only the one in kept round
    }

    #[test]
    fn test_build_compacted_no_orphaned_tool_calls() {
        // Verify that when a duplicate skill is filtered, BOTH the assistant
        // tool_call AND the tool result are removed (no orphaned tool_call).
        let messages = vec![
            // Round 1: activate skill (will be compressed)
            user_msg("review"),
            skill_tool_call_msg("review", "gpt"),
            skill_tool_result_msg("review"),
            assistant_msg("reviewed", "gpt"),
            // Round 2: same skill again (kept round)
            user_msg("review again"),
            skill_tool_call_msg("review", "gpt"),
            skill_tool_result_msg("review"),
            assistant_msg("reviewed again", "gpt"),
        ];

        let result = build_compacted_messages(&messages, 1, "Earlier review");

        // The compressed round's skill_tool_call should also be removed.
        let tool_call_count = result.iter().filter(|m| is_skill_tool_call(m)).count();
        assert_eq!(tool_call_count, 1); // Only the one in kept round

        // Verify no orphaned assistant messages with tool_calls exist
        // before the kept round starts.
        // Structure should be: summary(User) → kept_round(User, Assistant(tc), Tool, Assistant)
        assert_eq!(result[0].role, ChatRole::User); // summary
        assert_eq!(result[1].role, ChatRole::User); // kept round start
    }

    /// Helper: assistant message with multiple activate_skill tool calls.
    fn multi_skill_tool_call_msg(names: &[&str], agent: &str) -> ChatMessage {
        ChatMessage {
            role: ChatRole::Assistant,
            content: String::new(),
            name: Some(agent.to_string()),
            tool_calls: Some(
                names
                    .iter()
                    .map(|n| ToolCallInfo {
                        id: format!("call_{n}"),
                        name: "activate_skill".to_string(),
                        arguments: format!("{{\"name\":\"{n}\"}}"),
                        thought_signature: None,
                    })
                    .collect(),
            ),
            tool_call_id: None,
            server_tool_uses: Vec::new(),
            addressee: None,
            whisper_targets: None,
            created_at: chrono::Utc::now(),
            usage: None,
            images: Vec::new(),
            thinking_blocks: Vec::new(),
            raw_content_blocks: Vec::new(),
        }
    }

    #[test]
    fn test_build_compacted_multi_skill_same_message() {
        // Edge case: model activates two skills in a single assistant message.
        // Current behavior: the block is kept/dropped based on the FIRST
        // activate_skill call's name. This test documents the behavior.
        let messages = vec![
            // Round 1: activate two skills in one message (compressed)
            user_msg("review and search"),
            multi_skill_tool_call_msg(&["review", "search"], "gpt"),
            skill_tool_result_msg("review"),
            skill_tool_result_msg("search"),
            assistant_msg("done both", "gpt"),
            // Round 2 (kept): has "review" only
            user_msg("review again"),
            skill_tool_call_msg("review", "gpt"),
            skill_tool_result_msg("review"),
            assistant_msg("reviewed", "gpt"),
        ];

        let result = build_compacted_messages(&messages, 1, "Earlier work");

        // The compressed block has first skill name "review" which IS in kept,
        // so the entire block (including "search" result) gets dropped.
        // This is a known limitation — documented here for awareness.
        let search_count = result
            .iter()
            .filter(|m| m.content.contains("name=\"search\""))
            .count();
        assert_eq!(
            search_count, 0,
            "multi-skill block dropped as a unit when first skill is duplicate"
        );
    }

    #[test]
    fn test_build_compacted_multi_skill_no_duplicate() {
        // When the multi-skill block's first name is NOT in kept rounds,
        // the entire block is preserved.
        let messages = vec![
            user_msg("review and search"),
            multi_skill_tool_call_msg(&["review", "search"], "gpt"),
            skill_tool_result_msg("review"),
            skill_tool_result_msg("search"),
            assistant_msg("done both", "gpt"),
            // Kept round: no skill activation
            user_msg("thanks"),
            assistant_msg("welcome", "gpt"),
        ];

        let result = build_compacted_messages(&messages, 1, "Earlier work");

        // No duplicates, so the entire multi-skill block is preserved.
        let skill_result_count = result.iter().filter(|m| is_skill_tool_result(m)).count();
        assert_eq!(skill_result_count, 2); // both review and search preserved
    }

    #[test]
    fn test_build_compacted_no_skills() {
        // Verify normal compaction still works when no skills are involved.
        let messages = vec![
            user_msg("hello"),
            assistant_msg("hi", "gpt"),
            user_msg("bye"),
            assistant_msg("goodbye", "gpt"),
        ];

        let result = build_compacted_messages(&messages, 1, "Greeted each other");
        assert_eq!(result.len(), 3); // summary + 1 kept round (2 msgs)
        assert!(result[0].content.contains("[Session History Summary]"));
        assert_eq!(result[1].content, "bye");
    }
}
