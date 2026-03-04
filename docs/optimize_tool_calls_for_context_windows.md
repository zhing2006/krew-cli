# Optimize Tool Calls for Context Windows

## 1. Problem Statement

In the agent loop, every tool call result is permanently appended to the message
history. When an agent reads the same file multiple times, or reads a file and
then writes to it, the **old tool results become stale** but still occupy
context window tokens. This causes:

1. **Wasted context window** — a 200-line `read_file` result consumes ~2000
   tokens. Multiple stale reads of the same file waste thousands of tokens.
2. **Increased API cost** — more prompt tokens = higher billing.
3. **Potential model confusion** — stale file contents coexist with newer
   content, the LLM may reference outdated information.

### 1.1 Concrete Example

```text
[Round 1] Assistant+tool_calls: read_file("src/main.rs", offset=1, limit=200)
[Round 1] Tool result: "L1: fn main() {...}\n...\n(200 lines)"   ← STALE after Round 5

[Round 3] Assistant+tool_calls: read_file("src/lib.rs", offset=1, limit=100)
[Round 3] Tool result: "L1: pub mod foo;\n...\n(100 lines)"      ← still valid

[Round 5] Assistant+tool_calls: read_file("src/main.rs", offset=1, limit=200)
[Round 5] Tool result: "L1: fn main() {...}\n...\n(200 lines)"   ← supersedes Round 1
```

Round 1's `read_file("src/main.rs")` result is superseded by Round 5's
identical read. The old 200 lines (~2000 tokens) are pure waste.

## 2. Industry Research

### 2.1 API Constraints

All major LLM APIs (OpenAI Chat Completions, Anthropic Messages, Google Gemini)
enforce one rule:

> An assistant message with `tool_calls` must have corresponding `role: tool`
> result messages, and vice versa.

**You CAN safely delete an entire pair** (the assistant+tool_calls message and
all its corresponding tool result messages) as long as the remaining message
sequence maintains this pairing integrity. No placeholder is required.

### 2.2 Claude Code Approach

Claude Code uses a two-tier strategy:

1. **Context Editing (server-side)** — The Anthropic API's
   `clear_tool_uses_20250919` beta strategy. When `input_tokens` exceed a
   configurable threshold, the API automatically clears the oldest tool
   use/result pairs, keeping the most recent N pairs. Cleared results are
   replaced with placeholder text.
2. **Compaction** — When approaching context window limits, the entire
   conversation is summarized and replaced.

Key design: **old tool outputs are cleared first**, then conversation is
summarized if still needed.

### 2.3 Codex Approach

Codex (OpenAI's CLI agent, source at `../codex`) takes a simpler approach:

- **No deduplication or staleness detection** at all.
- All tool call/result pairs are **permanently appended** to history.
- Individual tool outputs are **truncated at recording time** (char/token
  limits via `truncate_function_output_payload`).
- Relies solely on **wholesale compaction** when token limits are exceeded.
- `normalize.rs` ensures call/output pairing integrity but does no dedup.

### 2.4 Decision for krew-cli

Since krew-cli is a **multi-provider** tool (OpenAI, Anthropic, Google), we
cannot rely on Anthropic's server-side context editing. We implement
**client-side pruning** — detect and remove stale tool call pairs before
sending messages to the LLM.

## 3. Design

### 3.1 Staleness Rules

A tool call pair is considered **stale** when a later message in the history
makes its result outdated. The rules, ordered by priority:

| # | Stale Condition | Detection | Example |
|---|----------------|-----------|---------|
| 1 | **read → read same file (overlapping range)** | Later `read_file` with same `file_path` and overlapping `[offset, offset+limit)` range | Read `main.rs:1-200` then read `main.rs:1-200` again |
| 2 | **read → write/edit same file** | Later `write_file` or `edit_file` targeting the same `file_path` | Read `main.rs` then `edit_file("main.rs", ...)` |
| 3 | **glob/grep with identical parameters** | Later `glob` or `grep` call with identical arguments JSON | `glob("**/*.rs")` called twice |

**NOT considered stale:**

- `read_file("main.rs", offset=1, limit=100)` followed by
  `read_file("main.rs", offset=101, limit=100)` — different ranges, both valid.
- Tool results from the most recent occurrence — always kept.

### 3.2 Range Overlap Detection (read_file specific)

Two reads of the same file are considered overlapping when:

```
file_path_A == file_path_B
AND NOT (end_A <= start_B OR end_B <= start_A)
```

Where `start = offset` and `end = offset + limit`.

If read B's range is a **superset** of read A's range, read A is entirely stale.
If they **partially overlap**, we still mark read A as stale because read B
provides a more recent view of the overlapping region, and keeping both would
be confusing.

### 3.3 Deletion Strategy

When a stale tool call pair is detected:

1. **Identify the assistant message** — the `ChatMessage` with
   `role: Assistant` and `tool_calls` containing the stale tool call.
2. **Identify all corresponding tool result messages** — `ChatMessage` with
   `role: Tool` and `tool_call_id` matching the stale tool call's `id`.
3. **Handle the assistant message:**
   - If the assistant message has **only one tool call** (the stale one) and
     **no meaningful text content** (empty or whitespace-only `content`):
     → **Delete the entire assistant message.**
   - If the assistant message has **only one tool call** but **has text
     content**:
     → **Convert to a plain text assistant message** (set `tool_calls = None`,
     keep `content`, `name`, `role`).
   - If the assistant message has **multiple tool calls** (some stale, some
     not):
     → **Remove only the stale `ToolCallInfo`** from the `tool_calls` vec.
     Keep the assistant message with remaining tool calls.
4. **Delete the corresponding tool result message(s).**

### 3.4 Architecture: Where to Implement

```
                    Session History (self.messages)
                              │
                              │  unmodified, complete history
                              ▼
                   ┌─────────────────────┐
                   │ prepare_messages_    │
                   │ for_agent()          │
                   │                     │
                   │ Step 1: prune stale  │  ◄── NEW: prune_stale_tool_calls()
                   │ tool call pairs      │
                   │                     │
                   │ Step 2: fold other   │  existing logic
                   │ agents' tool chains  │
                   └─────────────────────┘
                              │
                              │  pruned + folded messages
                              ▼
                         LLM API call
```

**Key decision: prune in `prepare_messages_for_agent()`, NOT in the stored
history.**

Reasons:
- The original `self.messages` stays intact for persistence and UI display.
- Session files retain the full history (useful for debugging/replay).
- The pruning only affects what the LLM sees.
- Different agents could theoretically have different pruning needs in the
  future.

## 4. Implementation Plan

### 4.1 File Changes

Only **one file** needs modification:

```
crates/krew-core/src/agent.rs
```

### 4.2 New Function: `prune_stale_tool_calls`

Add a new function before `prepare_messages_for_agent()`:

```rust
/// Tool names whose results can become stale when the same file is
/// read/written again.
const READ_TOOLS: &[&str] = &["read_file"];
const WRITE_TOOLS: &[&str] = &["write_file", "edit_file"];
const IDEMPOTENT_TOOLS: &[&str] = &["glob", "grep"];

/// Identifies a tool use pair: the index of the assistant message and the
/// index of the specific tool call within that message's `tool_calls` vec.
#[derive(Debug)]
struct ToolUseRef {
    /// Index in the messages vec of the Assistant message.
    assistant_idx: usize,
    /// Index within `tool_calls` of the specific call.
    tool_call_idx: usize,
    /// The tool_call_id for finding the corresponding Tool result message.
    tool_call_id: String,
}

/// Remove stale tool call pairs from the message list.
///
/// A tool call is stale when a later call supersedes it:
/// - `read_file` on the same path with overlapping range
/// - `write_file`/`edit_file` on a path previously read
/// - `glob`/`grep` with identical arguments
///
/// This function does NOT modify the input vec. It returns a new vec with
/// stale pairs removed or converted to plain text.
fn prune_stale_tool_calls(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    // Phase 1: Scan all tool calls and build a list of stale ToolUseRefs.
    let stale_refs = find_stale_tool_calls(&messages);

    if stale_refs.is_empty() {
        return messages;
    }

    // Collect all tool_call_ids that should be removed.
    let stale_ids: HashSet<String> = stale_refs.iter()
        .map(|r| r.tool_call_id.clone())
        .collect();

    // Collect assistant message indices that have stale calls, grouped.
    // Key: assistant_idx, Value: set of tool_call_idx to remove.
    let mut stale_by_assistant: HashMap<usize, HashSet<usize>> = HashMap::new();
    for r in &stale_refs {
        stale_by_assistant
            .entry(r.assistant_idx)
            .or_default()
            .insert(r.tool_call_idx);
    }

    // Phase 2: Rebuild message list, applying deletions/conversions.
    let mut result = Vec::with_capacity(messages.len());
    for (idx, msg) in messages.into_iter().enumerate() {
        // Skip stale Tool result messages.
        if msg.role == ChatRole::Tool {
            if let Some(id) = &msg.tool_call_id {
                if stale_ids.contains(id) {
                    continue; // Drop this tool result.
                }
            }
            result.push(msg);
            continue;
        }

        // Handle Assistant messages that contain stale tool calls.
        if let Some(stale_tc_indices) = stale_by_assistant.get(&idx) {
            let tool_calls = msg.tool_calls.as_ref().unwrap();
            let total_calls = tool_calls.len();
            let stale_count = stale_tc_indices.len();

            if stale_count >= total_calls {
                // ALL tool calls in this message are stale.
                if msg.content.trim().is_empty() {
                    // No text content — drop the entire message.
                    continue;
                } else {
                    // Has text content — convert to plain text message.
                    result.push(ChatMessage::text(
                        ChatRole::Assistant,
                        msg.content,
                        msg.name,
                    ));
                }
            } else {
                // SOME tool calls are stale — keep the rest.
                let remaining: Vec<ToolCallInfo> = tool_calls
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !stale_tc_indices.contains(i))
                    .map(|(_, tc)| tc.clone())
                    .collect();
                result.push(ChatMessage {
                    tool_calls: Some(remaining),
                    ..msg
                });
            }
        } else {
            result.push(msg);
        }
    }

    result
}
```

### 4.3 New Function: `find_stale_tool_calls`

```rust
/// Scan messages and return references to all stale tool call pairs.
///
/// Strategy: iterate messages in order, maintaining a "latest seen" map.
/// When a newer call supersedes an older one, the older one is marked stale.
fn find_stale_tool_calls(messages: &[ChatMessage]) -> Vec<ToolUseRef> {
    let mut stale = Vec::new();

    // Track the latest read_file call per normalized file path.
    // Value: (assistant_idx, tool_call_idx, tool_call_id, offset, limit)
    let mut latest_reads: HashMap<String, (usize, usize, String, usize, usize)> =
        HashMap::new();

    // Track the latest glob/grep call per canonical arguments string.
    // Value: (assistant_idx, tool_call_idx, tool_call_id)
    let mut latest_idempotent: HashMap<(String, String), (usize, usize, String)> =
        HashMap::new();

    for (msg_idx, msg) in messages.iter().enumerate() {
        let tool_calls = match (&msg.role, &msg.tool_calls) {
            (ChatRole::Assistant, Some(tcs)) => tcs,
            _ => continue,
        };

        for (tc_idx, tc) in tool_calls.iter().enumerate() {
            let args: serde_json::Value =
                serde_json::from_str(&tc.arguments).unwrap_or_default();

            if READ_TOOLS.contains(&tc.name.as_str()) {
                let file_path = normalize_file_path(
                    args.get("file_path")
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                );
                let offset = args
                    .get("offset")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1) as usize;
                let limit = args
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(2000) as usize;

                // Check if there's an existing read for this file with
                // overlapping range.
                if let Some(prev) = latest_reads.get(&file_path) {
                    let (prev_offset, prev_limit) = (prev.3, prev.4);
                    if ranges_overlap(prev_offset, prev_limit, offset, limit) {
                        // Previous read is stale.
                        stale.push(ToolUseRef {
                            assistant_idx: prev.0,
                            tool_call_idx: prev.1,
                            tool_call_id: prev.2.clone(),
                        });
                    }
                }

                latest_reads.insert(
                    file_path,
                    (msg_idx, tc_idx, tc.id.clone(), offset, limit),
                );
            } else if WRITE_TOOLS.contains(&tc.name.as_str()) {
                let file_path = normalize_file_path(
                    args.get("file_path")
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                );

                // Any previous read of this file is now stale.
                if let Some(prev) = latest_reads.remove(&file_path) {
                    stale.push(ToolUseRef {
                        assistant_idx: prev.0,
                        tool_call_idx: prev.1,
                        tool_call_id: prev.2,
                    });
                }
            } else if IDEMPOTENT_TOOLS.contains(&tc.name.as_str()) {
                let key = (tc.name.clone(), canonicalize_args(&args));

                if let Some(prev) = latest_idempotent.get(&key) {
                    stale.push(ToolUseRef {
                        assistant_idx: prev.0,
                        tool_call_idx: prev.1,
                        tool_call_id: prev.2.clone(),
                    });
                }

                latest_idempotent.insert(
                    key,
                    (msg_idx, tc_idx, tc.id.clone()),
                );
            }
        }
    }

    stale
}
```

### 4.4 Helper Functions

```rust
/// Check if two 1-indexed line ranges overlap.
///
/// Range A: [offset_a, offset_a + limit_a)
/// Range B: [offset_b, offset_b + limit_b)
fn ranges_overlap(offset_a: usize, limit_a: usize, offset_b: usize, limit_b: usize) -> bool {
    let end_a = offset_a + limit_a;
    let end_b = offset_b + limit_b;
    // NOT (A ends before B starts OR B ends before A starts)
    !(end_a <= offset_b || end_b <= offset_a)
}

/// Normalize a file path for comparison.
///
/// Strips leading `./`, normalizes separators to `/`, and lowercases on
/// Windows for case-insensitive matching.
fn normalize_file_path(path: &str) -> String {
    let p = path
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string();
    #[cfg(windows)]
    {
        p.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        p
    }
}

/// Produce a canonical string representation of tool arguments for
/// equality comparison.
///
/// Sorts object keys to ensure `{"pattern":"*.rs","path":"."}` and
/// `{"path":".","pattern":"*.rs"}` are considered identical.
fn canonicalize_args(args: &serde_json::Value) -> String {
    // serde_json::to_string on a Value with sorted keys.
    // Since serde_json::Map uses BTreeMap internally, keys are already
    // sorted. Just serialize.
    serde_json::to_string(args).unwrap_or_default()
}
```

### 4.5 Integration Point

Modify `prepare_messages_for_agent()` to call `prune_stale_tool_calls()` first:

```rust
fn prepare_messages_for_agent(messages: Vec<ChatMessage>, self_name: &str) -> Vec<ChatMessage> {
    // Step 1: Remove stale tool call pairs.
    let messages = prune_stale_tool_calls(messages);

    // Step 2: Fold other agents' tool chains into text (existing logic).
    let mut result = Vec::new();
    let mut pending_summary: Option<(String, String)> = None;

    for msg in messages {
        // ... existing logic unchanged ...
    }

    // ...
    result
}
```

This is the **only change** to the existing function — one line added at the
top.

## 5. Test Plan

### 5.1 Unit Tests

Add the following tests to the existing `mod tests` in `agent.rs`:

```rust
#[test]
fn prune_duplicate_read_same_range() {
    // read_file("a.rs", 1, 200) → read_file("a.rs", 1, 200)
    // First read should be pruned.
    let messages = vec![
        ChatMessage::text(ChatRole::User, "check the file", None),
        assistant_with_tools("agent_a", "", vec![
            tc("1", "read_file", r#"{"file_path":"src/a.rs","offset":1,"limit":200}"#),
        ]),
        tool_result("read_file", "L1: old content...\n\n(200 lines)", "1"),
        assistant_msg("agent_a", "Let me check again"),
        assistant_with_tools("agent_a", "", vec![
            tc("2", "read_file", r#"{"file_path":"src/a.rs","offset":1,"limit":200}"#),
        ]),
        tool_result("read_file", "L1: new content...\n\n(200 lines)", "2"),
        assistant_msg("agent_a", "Done"),
    ];

    let result = prune_stale_tool_calls(messages);

    // The first read pair (tc "1") should be removed.
    assert_eq!(result.len(), 5); // user + check_again + tc2 + result2 + done
    assert!(!result.iter().any(|m| m.content.contains("old content")));
    assert!(result.iter().any(|m| m.content.contains("new content")));
}

#[test]
fn prune_read_after_write() {
    // read_file("a.rs") → edit_file("a.rs") → the read is stale
    let messages = vec![
        assistant_with_tools("agent_a", "", vec![
            tc("1", "read_file", r#"{"file_path":"src/a.rs"}"#),
        ]),
        tool_result("read_file", "L1: original...\n\n(10 lines)", "1"),
        assistant_with_tools("agent_a", "", vec![
            tc("2", "edit_file", r#"{"file_path":"src/a.rs","old":"x","new":"y"}"#),
        ]),
        tool_result("edit_file", "ok", "2"),
    ];

    let result = prune_stale_tool_calls(messages);

    // The read pair (tc "1") should be removed; edit pair stays.
    assert_eq!(result.len(), 2); // edit tc + edit result
    assert!(!result.iter().any(|m| m.content.contains("original")));
}

#[test]
fn prune_duplicate_glob() {
    // glob("**/*.rs") → glob("**/*.rs")
    let messages = vec![
        assistant_with_tools("agent_a", "", vec![
            tc("1", "glob", r#"{"pattern":"**/*.rs"}"#),
        ]),
        tool_result("glob", "found 5 files", "1"),
        assistant_with_tools("agent_a", "", vec![
            tc("2", "glob", r#"{"pattern":"**/*.rs"}"#),
        ]),
        tool_result("glob", "found 6 files", "2"),
    ];

    let result = prune_stale_tool_calls(messages);

    assert_eq!(result.len(), 2); // second glob pair only
    assert!(result.iter().any(|m| m.content.contains("found 6")));
    assert!(!result.iter().any(|m| m.content.contains("found 5")));
}

#[test]
fn non_overlapping_reads_preserved() {
    // read_file("a.rs", 1, 100) → read_file("a.rs", 101, 100) — different ranges
    let messages = vec![
        assistant_with_tools("agent_a", "", vec![
            tc("1", "read_file", r#"{"file_path":"a.rs","offset":1,"limit":100}"#),
        ]),
        tool_result("read_file", "L1: first chunk", "1"),
        assistant_with_tools("agent_a", "", vec![
            tc("2", "read_file", r#"{"file_path":"a.rs","offset":101,"limit":100}"#),
        ]),
        tool_result("read_file", "L101: second chunk", "2"),
    ];

    let result = prune_stale_tool_calls(messages);

    // Both reads should be preserved (non-overlapping ranges).
    assert_eq!(result.len(), 4);
}

#[test]
fn assistant_text_preserved_when_tools_pruned() {
    // Assistant says "Let me check" with a stale tool call.
    // The text should be kept as a plain text message.
    let messages = vec![
        assistant_with_tools("agent_a", "Let me check the code", vec![
            tc("1", "read_file", r#"{"file_path":"a.rs"}"#),
        ]),
        tool_result("read_file", "L1: old", "1"),
        assistant_with_tools("agent_a", "", vec![
            tc("2", "read_file", r#"{"file_path":"a.rs"}"#),
        ]),
        tool_result("read_file", "L1: new", "2"),
    ];

    let result = prune_stale_tool_calls(messages);

    assert_eq!(result.len(), 3); // text msg + tc2 + result2
    assert_eq!(result[0].content, "Let me check the code");
    assert!(result[0].tool_calls.is_none()); // Converted to plain text.
}

#[test]
fn multi_tool_call_partial_prune() {
    // Assistant calls glob + read_file in one message. Only read_file is stale.
    let messages = vec![
        assistant_with_tools("agent_a", "", vec![
            tc("1", "glob", r#"{"pattern":"*.rs"}"#),
            tc("2", "read_file", r#"{"file_path":"a.rs"}"#),
        ]),
        tool_result("glob", "found 3 files", "1"),
        tool_result("read_file", "L1: old content", "2"),
        assistant_with_tools("agent_a", "", vec![
            tc("3", "read_file", r#"{"file_path":"a.rs"}"#),
        ]),
        tool_result("read_file", "L1: new content", "3"),
    ];

    let result = prune_stale_tool_calls(messages);

    // The assistant message should keep glob but lose read_file.
    // glob result stays, old read_file result removed.
    // New read_file pair stays.
    assert_eq!(result.len(), 4); // modified_assistant + glob_result + tc3 + result3
    let first = &result[0];
    assert!(first.tool_calls.is_some());
    assert_eq!(first.tool_calls.as_ref().unwrap().len(), 1);
    assert_eq!(first.tool_calls.as_ref().unwrap()[0].name, "glob");
}

#[test]
fn different_files_not_pruned() {
    // read_file("a.rs") → read_file("b.rs") — different files
    let messages = vec![
        assistant_with_tools("agent_a", "", vec![
            tc("1", "read_file", r#"{"file_path":"a.rs"}"#),
        ]),
        tool_result("read_file", "a content", "1"),
        assistant_with_tools("agent_a", "", vec![
            tc("2", "read_file", r#"{"file_path":"b.rs"}"#),
        ]),
        tool_result("read_file", "b content", "2"),
    ];

    let result = prune_stale_tool_calls(messages);

    // Both reads are for different files — no pruning.
    assert_eq!(result.len(), 4);
}

#[test]
fn no_tool_calls_passthrough() {
    // Messages without any tool calls should pass through unchanged.
    let messages = vec![
        ChatMessage::text(ChatRole::User, "hello", None),
        assistant_msg("agent_a", "hi"),
    ];

    let result = prune_stale_tool_calls(messages.clone());

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].content, "hello");
    assert_eq!(result[1].content, "hi");
}
```

### 5.2 Integration Tests

Add to existing `prepare_messages_for_agent` tests to verify the full pipeline
(prune + fold) works correctly:

```rust
#[test]
fn prepare_messages_prunes_then_folds() {
    // Agent B's view: agent_a reads file twice, second read supersedes.
    // After pruning, only the second read remains.
    // After folding, that read should appear as text for agent_b.
    let messages = vec![
        ChatMessage::text(ChatRole::User, "analyze", None),
        assistant_with_tools("agent_a", "", vec![
            tc("1", "read_file", r#"{"file_path":"a.rs"}"#),
        ]),
        tool_result("read_file", "old content", "1"),
        assistant_with_tools("agent_a", "", vec![
            tc("2", "read_file", r#"{"file_path":"a.rs"}"#),
        ]),
        tool_result("read_file", "new content", "2"),
        assistant_msg("agent_a", "Done"),
    ];

    let result = prepare_messages_for_agent(messages, "agent_b");

    // Old read pruned, remaining read folded to text for agent_b.
    assert!(!result.iter().any(|m| m.content.contains("old content")));
    assert!(result.iter().any(|m| m.content.contains("new content")));
}
```

### 5.3 Edge Cases to Test

| Case | Expected Behavior |
|------|-------------------|
| Empty messages vec | Return empty vec |
| Only user/assistant text messages | Return unchanged |
| Three reads of same file | Only the last read survives |
| read → write → read same file | First read pruned by write; write stays; second read stays |
| Path normalization: `./src/a.rs` vs `src/a.rs` | Treated as same file |
| Windows path: `src\a.rs` vs `src/a.rs` | Treated as same file |
| Default offset/limit (not specified in args) | Use defaults (offset=1, limit=2000) |
| `serde_json::Map` key ordering for glob/grep | Canonical comparison works regardless of key order |

## 6. Performance Considerations

### 6.1 Time Complexity

- `find_stale_tool_calls`: O(n) where n = number of messages, with HashMap
  lookups for each tool call.
- `prune_stale_tool_calls`: O(n) rebuild pass.
- Total: O(n) — negligible compared to the LLM API call.

### 6.2 Memory

- One HashMap per tracked tool type (reads, idempotent).
- At most one entry per unique file path / argument combination.
- Negligible overhead.

### 6.3 Token Savings Estimate

| Scenario | Tokens Saved |
|----------|-------------|
| 1 duplicate `read_file` (200 lines) | ~2,000 tokens |
| Agent reads 5 files, re-reads 3 | ~6,000 tokens |
| 3 duplicate `glob` calls | ~500 tokens |
| 10-round agent loop with file edits | ~5,000–15,000 tokens |

For a 25-round agent loop with multiple file reads, savings can reach
**10,000–30,000 tokens** — a significant fraction of typical context windows.

## 7. Future Extensions

### 7.1 Configurable Retention

Add a `keep_last_n` parameter to allow keeping the N most recent tool results
for each file (similar to Claude Code's `keep: { type: "tool_uses", value: 3 }`
config). Default could be 1.

### 7.2 Threshold-Based Pruning

Only prune when the total message token count exceeds a threshold (similar to
Claude Code's `trigger: { type: "input_tokens", value: 100000 }`). This avoids
unnecessary pruning in short conversations.

### 7.3 Cross-Agent Staleness

Currently, the pruning only considers tool calls from the perspective of a
single message stream. In the future, if Agent A reads a file and Agent B
writes to it, Agent A's read could be marked stale. This requires tracking
write operations across agents.

### 7.4 Shell Command Results

In the future, when `shell` tool is enabled, shell command outputs (e.g.,
`cargo build` output) could also be pruned after subsequent runs. This is
more complex because shell commands don't have a simple "file path" key.

## 8. Checklist

- [ ] Implement `normalize_file_path()` helper
- [ ] Implement `ranges_overlap()` helper
- [ ] Implement `canonicalize_args()` helper
- [ ] Implement `ToolUseRef` struct
- [ ] Implement `find_stale_tool_calls()` function
- [ ] Implement `prune_stale_tool_calls()` function
- [ ] Add one line to `prepare_messages_for_agent()` to call pruning
- [ ] Add `use std::collections::HashSet` import (HashMap is already imported)
- [ ] Write all unit tests from Section 5.1
- [ ] Write integration test from Section 5.2
- [ ] Run `cargo fmt --all`
- [ ] Run `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] Run `cargo test -p krew-core`
