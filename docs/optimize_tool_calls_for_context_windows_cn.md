# 优化工具调用以节省上下文窗口

## 1. 问题描述

在 Agent Loop 中，每次工具调用的结果都会被永久追加到消息历史中。当 Agent 多次
读取同一文件，或者先读取文件再写入该文件时，**旧的工具结果已经过期**，但仍然
占用 context window 的 token 额度。这会导致：

1. **浪费上下文窗口** — 一次 200 行的 `read_file` 结果约消耗 ~2000 tokens。
   对同一文件的多次过期读取会浪费数千 tokens。
2. **增加 API 费用** — 更多 prompt tokens = 更高账单。
3. **可能误导模型** — 过期的文件内容与新内容同时存在，LLM 可能引用过时信息。

### 1.1 具体示例

```text
[第1轮] Assistant+tool_calls: read_file("src/main.rs", offset=1, limit=200)
[第1轮] Tool result: "L1: fn main() {...}\n...\n(200 lines)"   ← 在第5轮后已过期

[第3轮] Assistant+tool_calls: read_file("src/lib.rs", offset=1, limit=100)
[第3轮] Tool result: "L1: pub mod foo;\n...\n(100 lines)"      ← 仍然有效

[第5轮] Assistant+tool_calls: read_file("src/main.rs", offset=1, limit=200)
[第5轮] Tool result: "L1: fn main() {...}\n...\n(200 lines)"   ← 取代了第1轮的结果
```

第1轮的 `read_file("src/main.rs")` 结果被第5轮的相同读取取代。旧的 200 行
（~2000 tokens）完全是浪费。

## 2. 行业调研

### 2.1 API 约束

所有主流 LLM API（OpenAI Chat Completions、Anthropic Messages、Google Gemini）
都遵循同一条规则：

> 包含 `tool_calls` 的 assistant 消息必须有对应的 `role: tool` 结果消息，
> 反之亦然。

**可以安全地整对删除**（assistant+tool_calls 消息及其所有对应的 tool result
消息），只要剩余的消息序列维持配对完整性即可。不需要保留 placeholder。

### 2.2 Claude Code 的做法

Claude Code 采用两层策略：

1. **Context Editing（服务端）** — Anthropic API 的
   `clear_tool_uses_20250919` beta 策略。当 `input_tokens` 超过可配置阈值时，
   API 自动清除最旧的工具调用/结果对，保留最近 N 对。被清除的结果会替换为
   placeholder 文本。
2. **Compaction（压缩）** — 当接近 context window 极限时，对整个对话进行摘要
   替换。

核心设计：**优先清除旧的 tool output**，必要时再对对话进行摘要。

### 2.3 Codex 的做法

Codex（OpenAI 的 CLI Agent，源码位于 `../codex`）采用更简单的策略：

- **没有任何去重或过期检测**。
- 所有工具调用/结果对**永久追加**到历史中。
- 单个工具输出在**录入时做长度截断**（通过 `truncate_function_output_payload`
  做字符/token 限制）。
- 仅依靠**整体 compaction**（当 token 限制超出时批量替换）。
- `normalize.rs` 确保调用/结果配对完整性，但不做去重。

### 2.4 krew-cli 的决策

由于 krew-cli 是**多 provider** 工具（OpenAI、Anthropic、Google），不能依赖
Anthropic 的服务端 context editing。我们在**客户端实现 pruning** — 在发送消息
给 LLM 之前检测并移除过期的工具调用对。

## 3. 设计方案

### 3.1 过期规则

当历史中后续的消息使某个工具调用结果失效时，该工具调用对被视为**过期**。
规则按优先级排列：

| # | 过期条件 | 检测方式 | 示例 |
|---|---------|---------|------|
| 1 | **read → read 同一文件（范围重叠）** | 后续 `read_file` 具有相同 `file_path` 且 `[offset, offset+limit)` 范围重叠 | 读 `main.rs:1-200` 后再次读 `main.rs:1-200` |
| 2 | **read → write/edit 同一文件** | 后续 `write_file` 或 `edit_file` 目标为相同 `file_path` | 读 `main.rs` 后执行 `edit_file("main.rs", ...)` |
| 3 | **glob/grep 参数完全相同** | 后续 `glob` 或 `grep` 调用的参数 JSON 完全一致 | `glob("**/*.rs")` 被调用两次 |

**不视为过期的情况：**

- `read_file("main.rs", offset=1, limit=100)` 之后
  `read_file("main.rs", offset=101, limit=100)` — 不同范围，两者都有效。
- 最新一次出现的工具结果 — 始终保留。

### 3.2 范围重叠检测（read_file 专用）

两次对同一文件的读取被视为重叠的条件：

```
file_path_A == file_path_B
AND NOT (end_A <= start_B OR end_B <= start_A)
```

其中 `start = offset`，`end = offset + limit`。

如果读取 B 的范围是读取 A 范围的**超集**，则 A 完全过期。
如果**部分重叠**，同样将 A 标记为过期，因为 B 提供了重叠区域的更新视图，
同时保留两者会造成混淆。

### 3.3 删除策略

当检测到过期的工具调用对时：

1. **定位 assistant 消息** — `role: Assistant` 且 `tool_calls` 包含过期工具调用
   的 `ChatMessage`。
2. **定位所有对应的 tool result 消息** — `role: Tool` 且 `tool_call_id`
   匹配过期工具调用 `id` 的 `ChatMessage`。
3. **处理 assistant 消息：**
   - 如果该 assistant 消息**只有一个工具调用**（即过期的那个）且**没有有意义的
     文本内容**（`content` 为空或仅含空白字符）：
     → **删除整个 assistant 消息。**
   - 如果该 assistant 消息**只有一个工具调用**但**有文本内容**：
     → **转换为纯文本 assistant 消息**（`tool_calls` 设为 `None`，保留
     `content`、`name`、`role`）。
   - 如果该 assistant 消息**有多个工具调用**（部分过期，部分不过期）：
     → **仅从 `tool_calls` 向量中移除过期的 `ToolCallInfo`**。保留剩余工具调用
     的 assistant 消息。
4. **删除对应的 tool result 消息。**

### 3.4 架构：实现位置

```
                    Session History (self.messages)
                              │
                              │  未修改的完整历史
                              ▼
                   ┌─────────────────────┐
                   │ prepare_messages_    │
                   │ for_agent()          │
                   │                     │
                   │ 步骤1: 裁剪过期的    │  ◄── 新增: prune_stale_tool_calls()
                   │ 工具调用对           │
                   │                     │
                   │ 步骤2: 折叠其他      │  现有逻辑
                   │ Agent 的工具链       │
                   └─────────────────────┘
                              │
                              │  裁剪 + 折叠后的消息
                              ▼
                         LLM API 调用
```

**关键决策：在 `prepare_messages_for_agent()` 中裁剪，而不是修改存储的历史。**

原因：
- 原始 `self.messages` 保持完整，用于持久化和 UI 显示。
- 会话文件保留完整历史（便于调试/回放）。
- 裁剪只影响 LLM 实际看到的内容。
- 未来不同 Agent 可能有不同的裁剪需求。

## 4. 实现计划

### 4.1 需要修改的文件

只需修改**一个文件**：

```
crates/krew-core/src/agent.rs
```

### 4.2 新函数：`prune_stale_tool_calls`

在 `prepare_messages_for_agent()` 之前添加新函数：

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

### 4.3 新函数：`find_stale_tool_calls`

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

### 4.4 辅助函数

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

### 4.5 集成点

修改 `prepare_messages_for_agent()` 使其先调用 `prune_stale_tool_calls()`：

```rust
fn prepare_messages_for_agent(messages: Vec<ChatMessage>, self_name: &str) -> Vec<ChatMessage> {
    // Step 1: Remove stale tool call pairs.
    let messages = prune_stale_tool_calls(messages);

    // Step 2: Fold other agents' tool chains into text (existing logic).
    let mut result = Vec::new();
    let mut pending_summary: Option<(String, String)> = None;

    for msg in messages {
        // ... 现有逻辑不变 ...
    }

    // ...
    result
}
```

这是对现有函数的**唯一修改** — 在顶部添加一行。

## 5. 测试计划

### 5.1 单元测试

在 `agent.rs` 现有的 `mod tests` 中添加以下测试：

```rust
#[test]
fn prune_duplicate_read_same_range() {
    // read_file("a.rs", 1, 200) → read_file("a.rs", 1, 200)
    // 第一次读取应被裁剪。
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

    // 第一次读取对 (tc "1") 应被移除。
    assert_eq!(result.len(), 5); // user + check_again + tc2 + result2 + done
    assert!(!result.iter().any(|m| m.content.contains("old content")));
    assert!(result.iter().any(|m| m.content.contains("new content")));
}

#[test]
fn prune_read_after_write() {
    // read_file("a.rs") → edit_file("a.rs") → read 已过期
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

    // 读取对 (tc "1") 应被移除；编辑对保留。
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

    assert_eq!(result.len(), 2); // 只保留第二个 glob 对
    assert!(result.iter().any(|m| m.content.contains("found 6")));
    assert!(!result.iter().any(|m| m.content.contains("found 5")));
}

#[test]
fn non_overlapping_reads_preserved() {
    // read_file("a.rs", 1, 100) → read_file("a.rs", 101, 100) — 不同范围
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

    // 两次读取范围不重叠 — 不裁剪。
    assert_eq!(result.len(), 4);
}

#[test]
fn assistant_text_preserved_when_tools_pruned() {
    // Assistant 说 "Let me check" 同时有过期的工具调用。
    // 文本应作为纯文本消息保留。
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
    assert!(result[0].tool_calls.is_none()); // 已转换为纯文本。
}

#[test]
fn multi_tool_call_partial_prune() {
    // Assistant 在一条消息中同时调用 glob + read_file。只有 read_file 过期。
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

    // assistant 消息应保留 glob 但移除 read_file。
    // glob 结果保留，旧 read_file 结果移除。
    // 新 read_file 对保留。
    assert_eq!(result.len(), 4); // modified_assistant + glob_result + tc3 + result3
    let first = &result[0];
    assert!(first.tool_calls.is_some());
    assert_eq!(first.tool_calls.as_ref().unwrap().len(), 1);
    assert_eq!(first.tool_calls.as_ref().unwrap()[0].name, "glob");
}

#[test]
fn different_files_not_pruned() {
    // read_file("a.rs") → read_file("b.rs") — 不同文件
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

    // 读取不同文件 — 不裁剪。
    assert_eq!(result.len(), 4);
}

#[test]
fn no_tool_calls_passthrough() {
    // 没有工具调用的消息应原样通过。
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

### 5.2 集成测试

在现有的 `prepare_messages_for_agent` 测试中添加，验证完整流水线
（裁剪 + 折叠）的正确性：

```rust
#[test]
fn prepare_messages_prunes_then_folds() {
    // Agent B 的视角：agent_a 读取文件两次，第二次取代第一次。
    // 裁剪后只剩第二次读取。
    // 折叠后该读取应以文本形式呈现给 agent_b。
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

    // 旧读取被裁剪，剩余读取被折叠为文本给 agent_b。
    assert!(!result.iter().any(|m| m.content.contains("old content")));
    assert!(result.iter().any(|m| m.content.contains("new content")));
}
```

### 5.3 边界用例

| 用例 | 预期行为 |
|------|---------|
| 空消息向量 | 返回空向量 |
| 仅有 user/assistant 纯文本消息 | 原样返回 |
| 同一文件被读取三次 | 只有最后一次读取保留 |
| read → write → read 同一文件 | 第一次 read 被 write 判定为过期并裁剪；write 保留；第二次 read 保留 |
| 路径规范化：`./src/a.rs` vs `src/a.rs` | 视为同一文件 |
| Windows 路径：`src\a.rs` vs `src/a.rs` | 视为同一文件 |
| 默认 offset/limit（参数中未指定） | 使用默认值（offset=1, limit=2000） |
| `serde_json::Map` key 顺序对 glob/grep 的影响 | 规范化比较确保与 key 顺序无关 |

## 6. 性能分析

### 6.1 时间复杂度

- `find_stale_tool_calls`：O(n)，n 为消息数量，每次工具调用做 HashMap 查找。
- `prune_stale_tool_calls`：O(n) 重建遍历。
- 总计：O(n) — 相比 LLM API 调用可忽略不计。

### 6.2 内存

- 每种跟踪的工具类型一个 HashMap（reads、idempotent）。
- 每个唯一文件路径/参数组合最多一个条目。
- 开销可忽略不计。

### 6.3 Token 节省估算

| 场景 | 节省的 Tokens |
|------|-------------|
| 1 次重复 `read_file`（200 行） | ~2,000 tokens |
| Agent 读取 5 个文件，重读其中 3 个 | ~6,000 tokens |
| 3 次重复 `glob` 调用 | ~500 tokens |
| 10 轮包含文件编辑的 Agent Loop | ~5,000–15,000 tokens |

对于 25 轮包含多次文件读取的 Agent Loop，节省可达
**10,000–30,000 tokens** — 占典型 context window 的显著比例。

## 7. 未来扩展

### 7.1 可配置的保留数量

添加 `keep_last_n` 参数，允许为每个文件保留最近 N 次工具结果（类似 Claude Code
的 `keep: { type: "tool_uses", value: 3 }` 配置）。默认值可设为 1。

### 7.2 基于阈值的裁剪

仅在消息总 token 数超过阈值时才进行裁剪（类似 Claude Code 的
`trigger: { type: "input_tokens", value: 100000 }`）。这可以避免在短对话中
进行不必要的裁剪。

### 7.3 跨 Agent 的过期检测

当前裁剪只从单一消息流的角度考虑工具调用。未来，如果 Agent A 读取了某个文件而
Agent B 写入了该文件，Agent A 的读取可以被标记为过期。这需要跨 Agent 跟踪
写操作。

### 7.4 Shell 命令结果

未来当 `shell` 工具启用时，shell 命令的输出（如 `cargo build` 的输出）也可以
在后续运行后被裁剪。这更复杂，因为 shell 命令没有简单的 `file_path` 键。

## 8. 执行清单

- [ ] 实现 `normalize_file_path()` 辅助函数
- [ ] 实现 `ranges_overlap()` 辅助函数
- [ ] 实现 `canonicalize_args()` 辅助函数
- [ ] 实现 `ToolUseRef` 结构体
- [ ] 实现 `find_stale_tool_calls()` 函数
- [ ] 实现 `prune_stale_tool_calls()` 函数
- [ ] 在 `prepare_messages_for_agent()` 顶部添加一行调用裁剪
- [ ] 添加 `use std::collections::HashSet` 导入（HashMap 已存在）
- [ ] 编写第 5.1 节的所有单元测试
- [ ] 编写第 5.2 节的集成测试
- [ ] 运行 `cargo fmt --all`
- [ ] 运行 `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] 运行 `cargo test -p krew-core`
