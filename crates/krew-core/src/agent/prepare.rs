use krew_llm::{ChatMessage, ChatRole};

use super::prune::prune_stale_tool_calls;

/// Preprocess messages for a specific agent: keep own tool chains in native
/// format, convert other agents' tool chains to text descriptions.
///
/// Other agents' assistant+tool_calls messages are merged with their
/// subsequent Tool result messages into a single text Assistant message.
/// This allows every agent to see what tools other agents used, without
/// requiring native tool_calls format (which only works for the "self"
/// role).
pub(super) fn prepare_messages_for_agent(
    messages: Vec<ChatMessage>,
    self_name: &str,
) -> Vec<ChatMessage> {
    // Apply whisper filtering: replace messages not visible to self_name with placeholders.
    let messages = apply_whisper_filter(messages, self_name);
    let messages = prune_stale_tool_calls(messages);
    let mut result = Vec::new();
    // Accumulates text for an other-agent's tool call block being folded.
    let mut pending_summary: Option<(String, String)> = None; // (agent_name, text)

    for msg in messages {
        match msg.role {
            ChatRole::Assistant if msg.tool_calls.is_some() => {
                // Flush any pending summary first.
                if let Some((name, text)) = pending_summary.take() {
                    result.push(ChatMessage::text(ChatRole::Assistant, text, Some(name)));
                }

                let is_other = msg.name.as_ref().is_some_and(|n| n != self_name);
                if is_other {
                    // Convert to text description for other agent visibility.
                    let agent_name = msg.name.clone().unwrap_or_default();
                    let mut text = msg.content.clone();
                    for tc in msg.tool_calls.as_ref().unwrap() {
                        let display = format_tool_call_text(&tc.name, &tc.arguments);
                        if !text.is_empty() {
                            text.push('\n');
                        }
                        text.push_str(&format!("[Used tool: {display}]"));
                    }
                    pending_summary = Some((agent_name, text));
                } else {
                    result.push(msg); // Keep native format for self.
                }
            }
            ChatRole::Tool if pending_summary.is_some() => {
                // Fold tool result into pending summary text.
                let (_, text) = pending_summary.as_mut().unwrap();
                let tool_name = msg.name.as_deref().unwrap_or("tool");
                text.push_str(&format!("\n[Result from {tool_name}: {}]", msg.content));
            }
            _ => {
                // Flush pending summary before pushing other messages.
                if let Some((name, text)) = pending_summary.take() {
                    result.push(ChatMessage::text(ChatRole::Assistant, text, Some(name)));
                }
                result.push(msg);
            }
        }
    }

    // Flush remaining pending summary.
    if let Some((name, text)) = pending_summary.take() {
        result.push(ChatMessage::text(ChatRole::Assistant, text, Some(name)));
    }

    result
}

/// Apply whisper filtering: for messages with `whisper_targets` that don't
/// include `self_name`, replace with a placeholder. Tool call chains from
/// whispered assistant messages are collapsed into a single placeholder.
fn apply_whisper_filter(messages: Vec<ChatMessage>, self_name: &str) -> Vec<ChatMessage> {
    let mut result = Vec::new();
    let mut skip_tool_results = false; // When true, we're inside a whispered tool chain.

    for msg in messages {
        let targets = msg.whisper_targets.as_ref();

        // Check if this message is a whisper not visible to self_name.
        let is_hidden_whisper = targets.is_some_and(|t| !t.iter().any(|n| n == self_name));

        if skip_tool_results {
            if msg.role == ChatRole::Tool {
                continue; // Skip tool results belonging to a hidden whispered tool chain.
            }
            skip_tool_results = false;
            // Fall through to process this non-tool message normally.
        }

        if !is_hidden_whisper {
            result.push(msg);
            continue;
        }

        // Hidden whisper: create placeholder.
        match msg.role {
            ChatRole::User => {
                let target_list = targets.unwrap().join(", ");
                result.push(ChatMessage {
                    role: ChatRole::User,
                    content: format!("[Whisper to {target_list}]"),
                    name: msg.name,
                    tool_calls: None,
                    tool_call_id: None,
                    server_tool_uses: Vec::new(),
                    addressee: msg.addressee,
                    whisper_targets: None,
                    created_at: msg.created_at,
                    usage: None,
                    images: Vec::new(),
                    thinking_blocks: Vec::new(),
                    raw_content_blocks: Vec::new(),
                });
            }
            ChatRole::Assistant => {
                // If this assistant message has tool_calls, skip subsequent tool results too.
                if msg.tool_calls.is_some() {
                    skip_tool_results = true;
                }
                result.push(ChatMessage {
                    role: ChatRole::Assistant,
                    content: "[Whisper]".to_string(),
                    name: msg.name,
                    tool_calls: None,
                    tool_call_id: None,
                    server_tool_uses: Vec::new(),
                    addressee: None,
                    whisper_targets: None,
                    created_at: msg.created_at,
                    usage: None,
                    images: Vec::new(),
                    thinking_blocks: Vec::new(),
                    raw_content_blocks: Vec::new(),
                });
            }
            ChatRole::Tool => {
                // Standalone tool result with whisper_targets (shouldn't happen normally,
                // but handle it by skipping since it belongs to a whispered chain).
                continue;
            }
            ChatRole::System => {
                result.push(msg); // System messages are never filtered.
            }
        }
    }

    result
}

/// Format a tool call as a plain text string: `tool_name("arg1", key="arg2")`
fn format_tool_call_text(name: &str, arguments: &str) -> String {
    let args: serde_json::Value = serde_json::from_str(arguments).unwrap_or_default();
    let params = match args.as_object() {
        Some(obj) => {
            let parts: Vec<String> = obj
                .iter()
                .map(|(key, val)| {
                    let display = match val {
                        serde_json::Value::String(s) => format!("\"{s}\""),
                        other => other.to_string(),
                    };
                    if obj.keys().next() == Some(key) {
                        display
                    } else {
                        format!("{key}={display}")
                    }
                })
                .collect();
            parts.join(", ")
        }
        None => String::new(),
    };
    format!("{name}({params})")
}

#[cfg(test)]
mod tests {
    use super::*;
    use krew_llm::ToolCallInfo;

    fn assistant_msg(name: &str, text: &str) -> ChatMessage {
        ChatMessage::text(ChatRole::Assistant, text, Some(name.to_string()))
    }

    fn assistant_with_tools(name: &str, text: &str, tools: Vec<ToolCallInfo>) -> ChatMessage {
        ChatMessage {
            role: ChatRole::Assistant,
            content: text.to_string(),
            name: Some(name.to_string()),
            tool_calls: Some(tools),
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

    fn tool_result(tool_name: &str, content: &str, call_id: &str) -> ChatMessage {
        ChatMessage {
            role: ChatRole::Tool,
            content: content.to_string(),
            name: Some(tool_name.to_string()),
            tool_calls: None,
            tool_call_id: Some(call_id.to_string()),
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

    fn tc(id: &str, name: &str, args: &str) -> ToolCallInfo {
        ToolCallInfo {
            id: id.to_string(),
            name: name.to_string(),
            arguments: args.to_string(),
            thought_signature: None,
        }
    }

    #[test]
    fn own_tool_chain_preserved_native() {
        let messages = vec![
            ChatMessage::text(ChatRole::User, "read the file", None),
            assistant_with_tools(
                "agent_a",
                "Let me check",
                vec![tc("1", "read_file", r#"{"path":"src/main.rs"}"#)],
            ),
            tool_result("read_file", "fn main() {}", "1"),
            assistant_msg("agent_a", "The file has 1 line"),
        ];

        let result = prepare_messages_for_agent(messages, "agent_a");

        assert_eq!(result.len(), 4);
        // Assistant with tool_calls should be preserved as-is.
        assert!(result[1].tool_calls.is_some());
        assert_eq!(result[1].tool_calls.as_ref().unwrap()[0].name, "read_file");
        // Tool result should be preserved as-is.
        assert_eq!(result[2].role, ChatRole::Tool);
        assert_eq!(result[2].content, "fn main() {}");
        // Final text message should be preserved.
        assert_eq!(result[3].content, "The file has 1 line");
    }

    #[test]
    fn other_agent_tool_chain_converted_to_text() {
        let messages = vec![
            ChatMessage::text(ChatRole::User, "read the file", None),
            assistant_with_tools(
                "agent_a",
                "Let me check",
                vec![tc("1", "read_file", r#"{"path":"src/main.rs"}"#)],
            ),
            tool_result("read_file", "fn main() {}", "1"),
            assistant_msg("agent_a", "The file has 1 line"),
        ];

        let result = prepare_messages_for_agent(messages, "agent_b");

        assert_eq!(result.len(), 3); // user, folded assistant, final text
        // Folded message should be text-only (no tool_calls).
        assert!(result[1].tool_calls.is_none());
        assert_eq!(result[1].role, ChatRole::Assistant);
        assert!(result[1].content.contains("[Used tool:"));
        assert!(result[1].content.contains("read_file"));
        assert!(result[1].content.contains("[Result from read_file:"));
        assert!(result[1].content.contains("fn main() {}"));
        assert_eq!(result[1].name.as_deref(), Some("agent_a"));
        // Final text preserved.
        assert_eq!(result[2].content, "The file has 1 line");
    }

    #[test]
    fn messages_without_tools_unaffected() {
        let messages = vec![
            ChatMessage::text(ChatRole::User, "hello", None),
            assistant_msg("agent_a", "hi there"),
            ChatMessage::text(ChatRole::User, "how are you?", None),
            assistant_msg("agent_b", "I am fine"),
        ];

        let result = prepare_messages_for_agent(messages.clone(), "agent_a");

        assert_eq!(result.len(), 4);
        for (orig, processed) in messages.iter().zip(result.iter()) {
            assert_eq!(orig.content, processed.content);
            assert_eq!(orig.role, processed.role);
        }
    }

    #[test]
    fn multiple_tool_calls_folded_correctly() {
        let messages = vec![
            assistant_with_tools(
                "agent_a",
                "",
                vec![
                    tc("1", "glob", r#"{"pattern":"*.rs"}"#),
                    tc("2", "grep", r#"{"pattern":"main"}"#),
                ],
            ),
            tool_result("glob", "found 3 files", "1"),
            tool_result("grep", "2 matches", "2"),
            assistant_msg("agent_a", "Done scanning"),
        ];

        let result = prepare_messages_for_agent(messages, "agent_b");

        assert_eq!(result.len(), 2); // folded + final text
        let folded = &result[0];
        assert!(folded.content.contains("[Used tool: glob"));
        assert!(folded.content.contains("[Used tool: grep"));
        assert!(folded.content.contains("[Result from glob: found 3 files]"));
        assert!(folded.content.contains("[Result from grep: 2 matches]"));
    }

    // ── Whisper filtering tests ───────────────────────────────────────

    fn user_msg(content: &str) -> ChatMessage {
        ChatMessage::text(ChatRole::User, content, None)
    }

    fn user_whisper(content: &str, targets: Vec<&str>) -> ChatMessage {
        ChatMessage::text(ChatRole::User, content, None)
            .with_whisper_targets(Some(targets.into_iter().map(String::from).collect()))
    }

    fn assistant_whisper(name: &str, content: &str, targets: Vec<&str>) -> ChatMessage {
        ChatMessage::text(ChatRole::Assistant, content, Some(name.to_string()))
            .with_whisper_targets(Some(targets.into_iter().map(String::from).collect()))
    }

    fn assistant_whisper_with_tools(
        name: &str,
        tools: Vec<ToolCallInfo>,
        targets: Vec<&str>,
    ) -> ChatMessage {
        ChatMessage {
            role: ChatRole::Assistant,
            content: String::new(),
            name: Some(name.to_string()),
            tool_calls: Some(tools),
            tool_call_id: None,
            server_tool_uses: Vec::new(),
            addressee: None,
            whisper_targets: Some(targets.into_iter().map(String::from).collect()),
            created_at: chrono::Utc::now(),
            usage: None,
            images: Vec::new(),
            thinking_blocks: Vec::new(),
            raw_content_blocks: Vec::new(),
        }
    }

    fn tool_whisper(
        tool_name: &str,
        content: &str,
        call_id: &str,
        targets: Vec<&str>,
    ) -> ChatMessage {
        ChatMessage {
            role: ChatRole::Tool,
            content: content.to_string(),
            name: Some(tool_name.to_string()),
            tool_calls: None,
            tool_call_id: Some(call_id.to_string()),
            server_tool_uses: Vec::new(),
            addressee: None,
            whisper_targets: Some(targets.into_iter().map(String::from).collect()),
            created_at: chrono::Utc::now(),
            usage: None,
            images: Vec::new(),
            thinking_blocks: Vec::new(),
            raw_content_blocks: Vec::new(),
        }
    }

    #[test]
    fn whisper_group_member_sees_content() {
        let messages = vec![
            user_whisper("secret", vec!["agent_a", "agent_b"]),
            assistant_whisper("agent_a", "got it", vec!["agent_a", "agent_b"]),
        ];
        let result = prepare_messages_for_agent(messages, "agent_a");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].content, "secret");
        assert_eq!(result[1].content, "got it");
    }

    #[test]
    fn whisper_non_member_sees_placeholder() {
        let messages = vec![
            user_whisper("secret", vec!["agent_a"]),
            assistant_whisper("agent_a", "reply", vec!["agent_a"]),
        ];
        let result = prepare_messages_for_agent(messages, "agent_b");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].content, "[Whisper to agent_a]");
        assert_eq!(result[0].role, ChatRole::User);
        assert_eq!(result[1].content, "[Whisper]");
        assert_eq!(result[1].role, ChatRole::Assistant);
        assert_eq!(result[1].name.as_deref(), Some("agent_a"));
        // Placeholder should not carry whisper_targets.
        assert!(result[0].whisper_targets.is_none());
        assert!(result[1].whisper_targets.is_none());
    }

    #[test]
    fn whisper_tool_chain_collapsed_for_non_member() {
        let messages = vec![
            user_whisper("check file", vec!["agent_a"]),
            assistant_whisper_with_tools(
                "agent_a",
                vec![tc("1", "read_file", r#"{"path":"secret.rs"}"#)],
                vec!["agent_a"],
            ),
            tool_whisper("read_file", "secret content", "1", vec!["agent_a"]),
            assistant_whisper("agent_a", "done", vec!["agent_a"]),
        ];
        let result = prepare_messages_for_agent(messages, "agent_b");
        // user placeholder + assistant placeholder (tool chain collapsed) + assistant placeholder
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].content, "[Whisper to agent_a]");
        assert_eq!(result[1].content, "[Whisper]");
        assert_eq!(result[2].content, "[Whisper]");
        // No tool messages visible.
        assert!(result.iter().all(|m| m.role != ChatRole::Tool));
    }

    #[test]
    fn whisper_tool_chain_visible_for_member() {
        let messages = vec![
            user_whisper("check file", vec!["agent_a"]),
            assistant_whisper_with_tools(
                "agent_a",
                vec![tc("1", "read_file", r#"{"path":"secret.rs"}"#)],
                vec!["agent_a"],
            ),
            tool_whisper("read_file", "secret content", "1", vec!["agent_a"]),
            assistant_whisper("agent_a", "done", vec!["agent_a"]),
        ];
        let result = prepare_messages_for_agent(messages, "agent_a");
        // All messages visible: user + assistant(tool_calls) + tool + assistant
        assert_eq!(result.len(), 4);
        assert!(result[1].tool_calls.is_some());
        assert_eq!(result[2].role, ChatRole::Tool);
    }

    #[test]
    fn mixed_whisper_and_normal_messages() {
        let messages = vec![
            user_msg("hello everyone"),
            assistant_msg("agent_a", "hi"),
            user_whisper("secret to a", vec!["agent_a"]),
            assistant_whisper("agent_a", "secret reply", vec!["agent_a"]),
            user_msg("back to normal"),
        ];
        let result = prepare_messages_for_agent(messages, "agent_b");
        assert_eq!(result.len(), 5);
        assert_eq!(result[0].content, "hello everyone");
        assert_eq!(result[1].content, "hi");
        assert_eq!(result[2].content, "[Whisper to agent_a]");
        assert_eq!(result[3].content, "[Whisper]");
        assert_eq!(result[4].content, "back to normal");
    }

    #[test]
    fn mixed_agents_own_and_other() {
        let messages = vec![
            ChatMessage::text(ChatRole::User, "read files", None),
            // Agent A uses a tool (other agent for agent_b).
            assistant_with_tools(
                "agent_a",
                "Checking",
                vec![tc("1", "read_file", r#"{"path":"a.rs"}"#)],
            ),
            tool_result("read_file", "content_a", "1"),
            assistant_msg("agent_a", "Found it"),
            // Agent B uses a tool (self for agent_b).
            assistant_with_tools(
                "agent_b",
                "Let me also check",
                vec![tc("2", "read_file", r#"{"path":"b.rs"}"#)],
            ),
            tool_result("read_file", "content_b", "2"),
            assistant_msg("agent_b", "Got it"),
        ];

        let result = prepare_messages_for_agent(messages, "agent_b");

        // user + folded(agent_a) + text(agent_a) + native_tc(agent_b) + tool(agent_b) + text(agent_b)
        assert_eq!(result.len(), 6);
        // Agent A's tool call should be folded to text.
        assert!(result[1].tool_calls.is_none());
        assert!(result[1].content.contains("[Used tool:"));
        // Agent B's tool call should be native.
        assert!(result[3].tool_calls.is_some());
        assert_eq!(result[4].role, ChatRole::Tool);
    }
}
