## 1. 基础类型与流事件（krew-llm）

- [x] 1.1 在 `crates/krew-llm/src/lib.rs` 新增 `ThinkingBlock` 枚举（`Thinking { text, signature }` / `Redacted { data }`），`derive(Debug, Clone, PartialEq)`，并加单元测试覆盖 match 区分两个变体
- [x] 1.2 在 `ChatMessage` 增加 `thinking_blocks: Vec<ThinkingBlock>` 字段，更新所有构造函数（`text`、`user_with_addressee`）默认空向量；为新字段加单元测试断言默认为空
- [x] 1.3 在 `crates/krew-llm/src/types.rs` 的 `StreamEvent` 新增 `ThinkingBlockDone(ThinkingBlock)` 变体，更新 `Debug` derive；保留现有 `ThinkingDelta(String)` 不动
- [x] 1.4 全量编译 `cargo check -p krew-llm`，修复所有因 `ChatMessage` 新字段引入的初始化点（用 `..Default::default()` 或显式 `thinking_blocks: Vec::new()`）

## 2. Anthropic SSE 解析（krew-llm）

- [x] 2.1 在 `crates/krew-llm/src/anthropic.rs::SseState` 增加 `thinking_text: String`、`thinking_signature: String`、`redacted_data: Option<String>` 状态字段
- [x] 2.2 在 `content_block_start` 分支处理 `type: "thinking"`：清空 `thinking_text` / `thinking_signature`；处理 `type: "redacted_thinking"`：从 block 中读取 `data` 字段并写入 `redacted_data`，同时把 `current_block_type` 设为 `"redacted_thinking"`
- [x] 2.3 在 `content_block_delta` 分支：`thinking_delta` 把文本追加到 `thinking_text`（保留现有 `StreamEvent::ThinkingDelta` 上抛）；`signature_delta` 把签名字符串写入 `thinking_signature`（替换原"silently ignored"行为）
- [x] 2.4 在 `content_block_stop` 分支：若 `current_block_type == "thinking"`，构造 `ThinkingBlock::Thinking { text: thinking_text.clone(), signature: thinking_signature.clone() }` 并上抛 `StreamEvent::ThinkingBlockDone(...)`；若 `current_block_type == "redacted_thinking"`，构造 `ThinkingBlock::Redacted { data }` 并上抛
- [x] 2.5 添加单元测试：(a) 普通 thinking block 聚合输出，(b) redacted_thinking block 上抛 Redacted 变体且不发 ThinkingDelta，(c) signature_delta 不再被忽略，(d) 流中途断流时半截 thinking 不上抛 ThinkingBlockDone

## 3. Anthropic 请求构造（krew-llm）

- [x] 3.1 在 `crates/krew-llm/src/anthropic.rs::convert_messages` 的 "Assistant messages with tool_calls" 分支前，加判断：若 `msg.role == Assistant && !is_other_agent && !msg.thinking_blocks.is_empty()`，在 `content_blocks` 开头按顺序压入对应的 `thinking` / `redacted_thinking` JSON block
- [x] 3.2 处理"无 tool_calls 但有 thinking_blocks 的 assistant 消息"：让普通 assistant 分支同样能输出 thinking blocks 前缀（保证 turn 末尾的 assistant message 也能带 thinking 回传）
- [x] 3.3 添加单元测试：`convert_assistant_with_thinking_and_tool_use`（thinking block 排在 tool_use 之前且 signature 一致）、`convert_assistant_with_redacted_thinking`（首 block type=redacted_thinking 且只有 data 字段）、`convert_other_agent_thinking_dropped`（is_other_agent=true 时 thinking_blocks 被丢弃）、`convert_assistant_thinking_only_no_tool_use`（终止态 assistant 也带 thinking）

## 4. Vertex Anthropic 复用确认（krew-llm）

- [x] 4.1 在 `crates/krew-llm/src/vertex_anthropic.rs` 增加集成测试：用 `run_capture_server` 模式构造一个带 `thinking_blocks` 的 `ChatMessage`，断言 capture 到的请求体 `messages[i].content[0].type == "thinking"` 且 `signature` 与输入一致
- [x] 4.2 在 `vertex_anthropic.rs` 增加 SSE 回放测试：注入含 `thinking_delta` + `signature_delta` + `content_block_stop` 的 SSE，断言收到 `StreamEvent::ThinkingBlockDone` 且 signature 正确

## 5. Agent loop 聚合（krew-core）

- [x] 5.1 在 `crates/krew-core/src/agent/agent_loop.rs::StreamResult` 新增 `thinking_blocks: Vec<ThinkingBlock>` 字段，默认空向量
- [x] 5.2 在 `consume_stream` 的 match 中新增 `StreamEvent::ThinkingBlockDone(block)` 分支，把 block push 到 `result.thinking_blocks`；`ThinkingDelta` 分支保持现状
- [x] 5.3 在 `agent_loop.rs` 构造 `assistant_msg` 的位置（含 tool_calls 的中间轮和最终轮两处）把 `result.thinking_blocks.clone()` 写入 `ChatMessage.thinking_blocks`；注意同一份要进 `tool_round_messages` 与 `messages` 两个集合
- [x] 5.4 添加单元测试：mock 一个 `consume_stream` 输入流，依次发送 `ThinkingBlockDone` × 2 + `TextDelta` + `ToolCall`，断言返回的 `StreamResult.thinking_blocks` 长度 2 且顺序正确
- [x] 5.5 添加单元测试：mock 一轮"有 thinking + tool_use"+ 第二轮"无内容直接 Done"，断言两轮发出的 `assistant_msg` 都正确带上对应轮的 thinking_blocks

## 6. 跨 Provider 忽略验证（krew-llm）

- [x] 6.1 在 `crates/krew-llm/src/openai_chat.rs` 测试中新增 `convert_assistant_with_thinking_blocks_is_ignored`：输入带 `thinking_blocks` 的 ChatMessage，断言生成的 OpenAI 请求体不含任何 thinking 字段、其余字段与不带 thinking_blocks 时完全一致
- [x] 6.2 在 `crates/krew-llm/src/openai_responses.rs` 测试中新增同名忽略测试
- [x] 6.3 在 `crates/krew-llm/src/google.rs` 测试中新增同名忽略测试

## 7. 持久化（krew-storage + krew-core）

- [x] 7.1 在 `crates/krew-storage/src/session_file.rs` 新增 `ThinkingBlockEntry` 类型：用 `#[serde(tag = "block_type")]` 标 `thinking` / `redacted_thinking`，包含相应字段；`derive(Debug, Clone, Serialize, Deserialize)`
- [x] 7.2 在 `MessageEntry` 增加 `thinking_blocks: Option<Vec<ThinkingBlockEntry>>`，加 `#[serde(default, skip_serializing_if = "Option::is_none")]`
- [x] 7.3 在 `crates/krew-storage/tests/session_file_test.rs` 增加测试：(a) 写入带两种 block 的消息并 roundtrip 读回，(b) 旧文件（无 thinking_blocks 键）能正常加载且字段为 None，(c) 空 vec 不出现在 TOML 输出
- [x] 7.4 在 `crates/krew-core/src/persistence.rs` 的 ChatMessage→MessageEntry 转换处加入 thinking_blocks 映射：空 vec → None，非空 vec → Some(转换后的 entries)
- [x] 7.5 在 `persistence.rs` 的 MessageEntry→ChatMessage 转换处加入反向映射：None / Some(empty) → 空 vec，Some(non-empty) → 对应 ThinkingBlock vec
- [x] 7.6 在 `crates/krew-core/tests` 下增加 persistence 往返测试：构造带两种 thinking_blocks 变体的 ChatMessage，转 MessageEntry → 写 TOML → 读 TOML → 转回 ChatMessage，断言完全相等

## 8. 端到端验证

- [x] 8.1 在 `crates/krew-llm/src/anthropic.rs` 测试模块（或新增 tests/ 文件）增加一个端到端用例：用 `run_capture_server` 启动一个 mock Anthropic server，第一次响应返回 `[thinking, tool_use]`，客户端执行后构造 `[user_msg, assistant_msg_with_thinking_and_tool_use, tool_result]` 再次请求，断言第二次请求体的 `messages[1].content[0].type == "thinking"` 且 signature 与第一次响应一致
- [x] 8.2 跑全量 `cargo test --workspace` 与 `cargo clippy --all-targets --all-features -- -D warnings` 与 `cargo fmt --all -- --check`，零警告通过
- [x] 8.3 手工验证：本地 `.krew/settings.toml` 启用一个 Claude Opus 4.7 agent 并打开 `enable_thinking = true` + 配置 `read_file` 与 `shell` 工具，跑一次需要工具的对话，确认无 400 报错且 session TOML 内出现 thinking_blocks 字段

## 9. Dependency & Parallelism

### Dependencies

- 1.1 → 1.2
- 1.1 → 1.3
- 1.2 → 1.4
- 1.3 → 1.4
- 1.4 → 2.1
- 1.4 → 3.1
- 1.4 → 5.1
- 1.4 → 7.1
- 2.1 → 2.2
- 2.2 → 2.3
- 2.3 → 2.4
- 2.4 → 2.5
- 2.4 → 4.2
- 3.1 → 3.2
- 3.2 → 3.3
- 3.3 → 4.1
- 5.1 → 5.2
- 5.2 → 5.3
- 5.3 → 5.4
- 5.3 → 5.5
- 5.3 → 7.4
- 7.1 → 7.2
- 7.2 → 7.3
- 7.4 → 7.5
- 7.5 → 7.6
- 4.1 → 8.1
- 5.5 → 8.1
- 7.6 → 8.1
- 8.1 → 8.2
- 8.2 → 8.3
- 6.1 → 8.2
- 6.2 → 8.2
- 6.3 → 8.2
- 2.5 → 3.1
- 4.1 → 4.2
- 5.4 → 5.5

### Parallel sets

- {2.1, 5.1, 7.1} —— 基础类型就绪后，SSE 状态、agent loop、storage 三个不同文件可并行启动；3.1 因同样修改 `anthropic.rs`，必须与 2.x 串行
- {6.1, 6.2, 6.3} —— 三个 provider 的"忽略 thinking_blocks"测试在三个独立文件中

### Resource contention

- 1.1 / 1.2 / 1.3 都改 `crates/krew-llm/src/lib.rs` 或 `types.rs`，强制串行以避免合并冲突
- 2.1–2.5 全部改 `anthropic.rs` 同一个 SSE 状态机，强制串行
- 3.1–3.3 全部改 `anthropic.rs::convert_messages`，强制串行
- 4.1 / 4.2 都改 `crates/krew-llm/src/vertex_anthropic.rs`，强制串行
- 5.1–5.3 全部改 `agent_loop.rs::consume_stream` / `assistant_msg` 构造点，强制串行
- 5.4 / 5.5 都添加 agent_loop 测试；除非显式拆到不同测试文件，否则强制串行
- 7.1 / 7.2 改 `session_file.rs`，7.4 / 7.5 改 `persistence.rs`，组内串行但 7.x 与 6.x 不同 crate 可并行
- 8.2 是 workspace 级编译 / 测试，与所有其他任务在 cargo `target/` 锁上互斥，必须在所有改动落地后单跑
