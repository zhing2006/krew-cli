use krew_llm::{ChatMessage, ChatRole, ToolCallInfo};

/// Identifies a single tool call within the message list that should be pruned.
struct ToolUseRef {
    /// Index of the Assistant message containing the tool call.
    assistant_idx: usize,
    /// Index within the Assistant message's `tool_calls` vec.
    tool_call_idx: usize,
    /// The `id` of the tool call (used to find the matching Tool result).
    tool_call_id: String,
}

/// Scan messages and return references to stale (superseded) tool calls.
///
/// Staleness rules:
/// - `read_file` is stale if a later `read_file` on the same file fully
///   covers its byte range, OR if a later `write_file`/`edit_file` targets
///   the same file.
/// - `glob` / `grep` with identical canonicalized arguments are stale.
fn find_stale_tool_calls(messages: &[ChatMessage]) -> Vec<ToolUseRef> {
    use std::collections::HashMap;

    // (assistant_idx, tool_call_idx, tool_call_id, range_start, range_end)
    type ReadEntry = (usize, usize, String, u64, u64);

    // Track the latest occurrence of each idempotent tool key.
    let mut latest_reads: HashMap<String, Vec<ReadEntry>> = HashMap::new();
    let mut latest_idempotent: HashMap<String, (usize, usize, String)> = HashMap::new();
    // Track files that have been written/edited (all prior reads are stale).
    let mut written_files: HashMap<String, usize> = HashMap::new(); // file_path -> msg_idx of write

    // First pass: collect all tool call locations.
    struct ToolCallLoc {
        assistant_idx: usize,
        tool_call_idx: usize,
        tool_call_id: String,
        name: String,
        args: serde_json::Value,
    }
    let mut all_calls: Vec<ToolCallLoc> = Vec::new();

    for (msg_idx, msg) in messages.iter().enumerate() {
        if msg.role == ChatRole::Assistant
            && let Some(ref tool_calls) = msg.tool_calls
        {
            for (tc_idx, tc) in tool_calls.iter().enumerate() {
                let args: serde_json::Value =
                    serde_json::from_str(&tc.arguments).unwrap_or_default();
                all_calls.push(ToolCallLoc {
                    assistant_idx: msg_idx,
                    tool_call_idx: tc_idx,
                    tool_call_id: tc.id.clone(),
                    name: tc.name.clone(),
                    args,
                });
            }
        }
    }

    // Second pass: determine staleness.
    let mut stale = Vec::new();

    for loc in &all_calls {
        match loc.name.as_str() {
            "read_file" => {
                let file_path = normalize_file_path(
                    loc.args
                        .get("file_path")
                        .or_else(|| loc.args.get("path"))
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                );
                let offset = loc.args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0);
                let limit = loc
                    .args
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(u64::MAX);
                let end = offset.saturating_add(limit);

                // Check if any existing read on same file overlaps.
                let entries = latest_reads.entry(file_path.clone()).or_default();
                // Mark earlier reads as stale only when the new read fully covers them.
                for (prev_aidx, prev_tcidx, prev_id, prev_offset, prev_end) in entries.iter() {
                    if offset <= *prev_offset && end >= *prev_end {
                        stale.push(ToolUseRef {
                            assistant_idx: *prev_aidx,
                            tool_call_idx: *prev_tcidx,
                            tool_call_id: prev_id.clone(),
                        });
                    }
                }
                // Remove stale entries and add current.
                entries.retain(|(_, _, _, po, pe)| !(offset <= *po && end >= *pe));
                entries.push((
                    loc.assistant_idx,
                    loc.tool_call_idx,
                    loc.tool_call_id.clone(),
                    offset,
                    end,
                ));
            }
            "write_file" | "edit_file" => {
                let file_path = normalize_file_path(
                    loc.args
                        .get("file_path")
                        .or_else(|| loc.args.get("path"))
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                );
                // All prior reads on this file are now stale.
                if let Some(entries) = latest_reads.remove(&file_path) {
                    for (prev_aidx, prev_tcidx, prev_id, _, _) in entries {
                        stale.push(ToolUseRef {
                            assistant_idx: prev_aidx,
                            tool_call_idx: prev_tcidx,
                            tool_call_id: prev_id,
                        });
                    }
                }
                written_files.insert(file_path, loc.assistant_idx);
            }
            "glob" | "grep" | "fetch_url" => {
                let key = format!("{}:{}", loc.name, canonicalize_args(&loc.args));
                if let Some((prev_aidx, prev_tcidx, prev_id)) = latest_idempotent.remove(&key) {
                    stale.push(ToolUseRef {
                        assistant_idx: prev_aidx,
                        tool_call_idx: prev_tcidx,
                        tool_call_id: prev_id,
                    });
                }
                latest_idempotent.insert(
                    key,
                    (
                        loc.assistant_idx,
                        loc.tool_call_idx,
                        loc.tool_call_id.clone(),
                    ),
                );
            }
            _ => {}
        }
    }

    stale
}

/// Rebuild the message list with stale tool calls and their results removed.
///
/// - Tool result messages for stale calls are dropped entirely.
/// - Assistant messages with ALL tool calls stale: if text is empty, drop the
///   message; if text is non-empty, convert to a plain text message.
/// - Assistant messages with SOME tool calls stale: remove only the stale
///   `ToolCallInfo` entries.
pub(super) fn prune_stale_tool_calls(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    use std::collections::{HashMap, HashSet};

    let stale_refs = find_stale_tool_calls(&messages);
    if stale_refs.is_empty() {
        return messages;
    }

    // Build lookup: assistant_idx -> set of stale tool_call_idx.
    let mut stale_by_msg: HashMap<usize, HashSet<usize>> = HashMap::new();
    let mut stale_ids: HashSet<String> = HashSet::new();
    for r in &stale_refs {
        stale_by_msg
            .entry(r.assistant_idx)
            .or_default()
            .insert(r.tool_call_idx);
        stale_ids.insert(r.tool_call_id.clone());
    }

    let mut result = Vec::with_capacity(messages.len());

    for (idx, msg) in messages.into_iter().enumerate() {
        // Drop Tool result messages whose tool_call_id is stale.
        if msg.role == ChatRole::Tool {
            if let Some(ref id) = msg.tool_call_id
                && stale_ids.contains(id)
            {
                continue;
            }
            result.push(msg);
            continue;
        }

        // Handle Assistant messages with stale tool calls.
        if let Some(stale_indices) = stale_by_msg.get(&idx)
            && let Some(ref tool_calls) = msg.tool_calls
        {
            if stale_indices.len() == tool_calls.len() {
                // All tool calls are stale.
                if msg.content.trim().is_empty() {
                    // No text content — drop the message entirely.
                    continue;
                }
                // Has text content — convert to plain text message.
                result.push(ChatMessage::text(
                    ChatRole::Assistant,
                    msg.content,
                    msg.name,
                ));
                continue;
            }
            // Partial prune: keep only non-stale tool calls.
            let filtered: Vec<ToolCallInfo> = tool_calls
                .iter()
                .enumerate()
                .filter(|(i, _)| !stale_indices.contains(i))
                .map(|(_, tc)| tc.clone())
                .collect();
            result.push(ChatMessage {
                role: msg.role,
                content: msg.content,
                name: msg.name,
                tool_calls: Some(filtered),
                tool_call_id: msg.tool_call_id,
                server_tool_uses: msg.server_tool_uses,
                addressee: msg.addressee,
                whisper_targets: msg.whisper_targets,
                created_at: msg.created_at,
                usage: msg.usage,
                images: Vec::new(),
            });
            continue;
        }

        result.push(msg);
    }

    result
}

/// Normalize a file path for comparison: replace backslashes, strip leading `./`.
fn normalize_file_path(path: &str) -> String {
    let p = path.replace('\\', "/");
    p.strip_prefix("./").unwrap_or(&p).to_string()
}

/// Produce a canonical string from tool arguments for deduplication.
///
/// Serializes to compact JSON with sorted keys.
fn canonicalize_args(args: &serde_json::Value) -> String {
    fn sorted_value(v: &serde_json::Value) -> serde_json::Value {
        match v {
            serde_json::Value::Object(map) => {
                let mut sorted: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                for k in keys {
                    sorted.insert(k.clone(), sorted_value(&map[k]));
                }
                serde_json::Value::Object(sorted)
            }
            serde_json::Value::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(sorted_value).collect())
            }
            other => other.clone(),
        }
    }
    serde_json::to_string(&sorted_value(args)).unwrap_or_default()
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
    fn prune_duplicate_read_same_range() {
        let messages = vec![
            assistant_with_tools(
                "a",
                "",
                vec![tc("1", "read_file", r#"{"file_path":"src/main.rs"}"#)],
            ),
            tool_result("read_file", "old content", "1"),
            assistant_with_tools(
                "a",
                "",
                vec![tc("2", "read_file", r#"{"file_path":"src/main.rs"}"#)],
            ),
            tool_result("read_file", "new content", "2"),
        ];
        let result = prune_stale_tool_calls(messages);
        // First read + result should be pruned, second kept.
        assert_eq!(result.len(), 2);
        assert_eq!(result[1].content, "new content");
    }

    #[test]
    fn prune_read_after_write() {
        let messages = vec![
            assistant_with_tools(
                "a",
                "",
                vec![tc("1", "read_file", r#"{"file_path":"src/lib.rs"}"#)],
            ),
            tool_result("read_file", "old", "1"),
            assistant_with_tools(
                "a",
                "",
                vec![tc(
                    "2",
                    "edit_file",
                    r#"{"file_path":"src/lib.rs","new":"x"}"#,
                )],
            ),
            tool_result("edit_file", "ok", "2"),
        ];
        let result = prune_stale_tool_calls(messages);
        // read_file + result pruned; edit_file + result kept.
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].tool_calls.as_ref().unwrap()[0].name, "edit_file");
    }

    #[test]
    fn prune_duplicate_glob() {
        let messages = vec![
            assistant_with_tools("a", "", vec![tc("1", "glob", r#"{"pattern":"*.rs"}"#)]),
            tool_result("glob", "3 files", "1"),
            assistant_with_tools("a", "", vec![tc("2", "glob", r#"{"pattern":"*.rs"}"#)]),
            tool_result("glob", "3 files", "2"),
        ];
        let result = prune_stale_tool_calls(messages);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].tool_calls.as_ref().unwrap()[0].id, "2");
    }

    #[test]
    fn non_overlapping_reads_preserved() {
        let messages = vec![
            assistant_with_tools(
                "a",
                "",
                vec![tc(
                    "1",
                    "read_file",
                    r#"{"file_path":"f.rs","offset":0,"limit":10}"#,
                )],
            ),
            tool_result("read_file", "first", "1"),
            assistant_with_tools(
                "a",
                "",
                vec![tc(
                    "2",
                    "read_file",
                    r#"{"file_path":"f.rs","offset":100,"limit":10}"#,
                )],
            ),
            tool_result("read_file", "second", "2"),
        ];
        let result = prune_stale_tool_calls(messages);
        // Both should be preserved (non-overlapping ranges).
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn partial_overlap_not_pruned() {
        // Old reads [0,50), new reads [30,80) — partial overlap, should NOT prune.
        let messages = vec![
            assistant_with_tools(
                "a",
                "",
                vec![tc(
                    "1",
                    "read_file",
                    r#"{"file_path":"f.rs","offset":0,"limit":50}"#,
                )],
            ),
            tool_result("read_file", "first", "1"),
            assistant_with_tools(
                "a",
                "",
                vec![tc(
                    "2",
                    "read_file",
                    r#"{"file_path":"f.rs","offset":30,"limit":50}"#,
                )],
            ),
            tool_result("read_file", "second", "2"),
        ];
        let result = prune_stale_tool_calls(messages);
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn old_covers_new_not_pruned() {
        // Old reads [0,100), new reads [20,50) — old is larger, should NOT prune.
        let messages = vec![
            assistant_with_tools(
                "a",
                "",
                vec![tc(
                    "1",
                    "read_file",
                    r#"{"file_path":"f.rs","offset":0,"limit":100}"#,
                )],
            ),
            tool_result("read_file", "full", "1"),
            assistant_with_tools(
                "a",
                "",
                vec![tc(
                    "2",
                    "read_file",
                    r#"{"file_path":"f.rs","offset":20,"limit":30}"#,
                )],
            ),
            tool_result("read_file", "partial", "2"),
        ];
        let result = prune_stale_tool_calls(messages);
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn new_covers_old_pruned() {
        // Old reads [20,50), new reads [0,100) — new fully covers old, should prune.
        let messages = vec![
            assistant_with_tools(
                "a",
                "",
                vec![tc(
                    "1",
                    "read_file",
                    r#"{"file_path":"f.rs","offset":20,"limit":30}"#,
                )],
            ),
            tool_result("read_file", "partial", "1"),
            assistant_with_tools(
                "a",
                "",
                vec![tc(
                    "2",
                    "read_file",
                    r#"{"file_path":"f.rs","offset":0,"limit":100}"#,
                )],
            ),
            tool_result("read_file", "full", "2"),
        ];
        let result = prune_stale_tool_calls(messages);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].tool_calls.as_ref().unwrap()[0].id, "2");
    }

    #[test]
    fn assistant_text_preserved_when_tools_pruned() {
        let messages = vec![
            assistant_with_tools(
                "a",
                "I will read the file",
                vec![tc("1", "read_file", r#"{"file_path":"a.rs"}"#)],
            ),
            tool_result("read_file", "old", "1"),
            assistant_with_tools(
                "a",
                "",
                vec![tc("2", "read_file", r#"{"file_path":"a.rs"}"#)],
            ),
            tool_result("read_file", "new", "2"),
        ];
        let result = prune_stale_tool_calls(messages);
        // First assistant converted to text (has content), tool result dropped.
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].content, "I will read the file");
        assert!(result[0].tool_calls.is_none());
    }

    #[test]
    fn multi_tool_call_partial_prune() {
        let messages = vec![
            assistant_with_tools(
                "a",
                "",
                vec![
                    tc("1", "read_file", r#"{"file_path":"a.rs"}"#),
                    tc("2", "glob", r#"{"pattern":"*.toml"}"#),
                ],
            ),
            tool_result("read_file", "content", "1"),
            tool_result("glob", "3 files", "2"),
            // Later read supersedes the first read_file, but glob is unique.
            assistant_with_tools(
                "a",
                "",
                vec![tc("3", "read_file", r#"{"file_path":"a.rs"}"#)],
            ),
            tool_result("read_file", "new content", "3"),
        ];
        let result = prune_stale_tool_calls(messages);
        // First assistant should keep only glob tool call; read_file result dropped.
        assert_eq!(result[0].tool_calls.as_ref().unwrap().len(), 1);
        assert_eq!(result[0].tool_calls.as_ref().unwrap()[0].name, "glob");
        // glob result kept, read_file result for "1" dropped.
        assert_eq!(result[1].role, ChatRole::Tool);
        assert_eq!(result[1].tool_call_id.as_deref(), Some("2"));
    }

    #[test]
    fn different_files_not_pruned() {
        let messages = vec![
            assistant_with_tools(
                "a",
                "",
                vec![tc("1", "read_file", r#"{"file_path":"a.rs"}"#)],
            ),
            tool_result("read_file", "content_a", "1"),
            assistant_with_tools(
                "a",
                "",
                vec![tc("2", "read_file", r#"{"file_path":"b.rs"}"#)],
            ),
            tool_result("read_file", "content_b", "2"),
        ];
        let result = prune_stale_tool_calls(messages);
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn prune_duplicate_fetch_url() {
        let messages = vec![
            assistant_with_tools(
                "a",
                "",
                vec![tc("1", "fetch_url", r#"{"url":"https://example.com"}"#)],
            ),
            tool_result("fetch_url", "old page content\n\n(5000 chars)", "1"),
            assistant_with_tools(
                "b",
                "",
                vec![tc("2", "fetch_url", r#"{"url":"https://example.com"}"#)],
            ),
            tool_result("fetch_url", "new page content\n\n(6000 chars)", "2"),
        ];
        let result = prune_stale_tool_calls(messages);
        // First fetch + result should be pruned, second kept.
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].tool_calls.as_ref().unwrap()[0].id, "2");
        assert_eq!(result[1].content, "new page content\n\n(6000 chars)");
    }

    #[test]
    fn different_urls_not_pruned() {
        let messages = vec![
            assistant_with_tools(
                "a",
                "",
                vec![tc("1", "fetch_url", r#"{"url":"https://example.com"}"#)],
            ),
            tool_result("fetch_url", "page A\n\n(100 chars)", "1"),
            assistant_with_tools(
                "a",
                "",
                vec![tc("2", "fetch_url", r#"{"url":"https://other.com"}"#)],
            ),
            tool_result("fetch_url", "page B\n\n(200 chars)", "2"),
        ];
        let result = prune_stale_tool_calls(messages);
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn no_tool_calls_passthrough() {
        let messages = vec![
            ChatMessage::text(ChatRole::User, "hello", None),
            assistant_msg("a", "hi"),
            ChatMessage::text(ChatRole::User, "bye", None),
        ];
        let result = prune_stale_tool_calls(messages.clone());
        assert_eq!(result.len(), 3);
        for (orig, pruned) in messages.iter().zip(result.iter()) {
            assert_eq!(orig.content, pruned.content);
        }
    }
}
