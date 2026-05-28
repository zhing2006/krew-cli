## 背景

Anthropic Messages API 的 extended thinking 协议要求：启用 thinking + tool use 时，把上一轮 assistant 的 `thinking` / `redacted_thinking` block（含 `signature`）原封不动随 `tool_result` 一起回传，否则服务端会以 400 拒绝整条请求。krew-cli 当前在 `crates/krew-llm/src/anthropic.rs`、`crates/krew-llm/src/vertex_anthropic.rs`、`crates/krew-llm/src/lib.rs::ChatMessage`、`crates/krew-storage/src/session_file.rs::MessageEntry`、`crates/krew-core/src/agent/agent_loop.rs::consume_stream` 这条全链路上都没有保留任何 thinking 相关结构（仅把 `ThinkingDelta` 转给 TUI 显示），导致 Claude agent 同时开启 thinking 与工具调用时必定失败。多 Agent 场景下，每个 Claude agent 各自的工具循环都会撞到同一个错误。

相关约束：

- 项目当前 Anthropic 模型默认 `adaptive` + `display: "summarized"`（`anthropic.rs:334-337`），需要保留 `signature` 才能让服务端解密 summarized 内容。
- 持久化使用 TOML，已有 `MessageEntry` 通过 `skip_serializing_if = "Option::is_none"` 处理可选字段。
- OpenAI Responses 已经独立处理 reasoning，Google 已经处理 `thoughtSignature`，本变更范围不动它们。
- 当前未启用 `interleaved-thinking-2025-05-14` beta header，Claude 4.6+ 模型默认会在 tool round 之间做 thinking，但**单条 assistant message 内 thinking 与 tool_use 交错**不会发生。

## 目标 / 非目标

**目标：**

- 让 `enable_thinking = true` + tool use 在 Anthropic / Vertex Anthropic 两条接入路径上都能正常完成工具循环，不再 400。
- 把 thinking blocks 完整保存进 session TOML，使 `--resume` 后回放给 LLM 的历史包含合法的 `signature`，维持推理连续性。
- 保留现有 TUI 实时 thinking 渲染（`StreamEvent::ThinkingDelta`）行为不变。
- 旧 session 文件零迁移：缺字段时自动当作空列表。

**非目标：**

- 不改 OpenAI Responses 的 reasoning 处理。
- 不改 Google 的 `thoughtSignature` 处理（已在 `ToolCallInfo` 上）。
- 不引入 `clear_thinking_20251015` / context-management beta 等高级裁剪策略；仅做最小可正确性修复。
- 不启用 `interleaved-thinking-2025-05-14` beta（保持当前默认）。
- 不对 OpenAI Chat / Google Provider 做"忽略 thinking_blocks"以外的任何改动。
- 不为旧 session 做 migration 脚本。

## 关键决策

### 决策 1：`thinking_blocks` 放在共享的 `ChatMessage` 上，而不是 Provider 私有结构

`ChatMessage`（`crates/krew-llm/src/lib.rs:99-124`）新增 `thinking_blocks: Vec<ThinkingBlock>` 字段。`ThinkingBlock` 是一个 enum：

```rust
pub enum ThinkingBlock {
    Thinking { text: String, signature: String },
    Redacted { data: String },
}
```

**为什么：** Agent loop（`crates/krew-core/src/agent/agent_loop.rs`）与 persistence（`crates/krew-core/src/persistence.rs`）都要在不知道具体 Provider 的情况下访问这些数据。如果藏在 Anthropic-only 结构里，agent loop 必须做 `Any`/downcast，与现有跨 provider 抽象不兼容。

**备选 A（驳回）：** Provider 自己藏在 trait object 内部。问题：agent loop 拿不到、无法持久化，且多 agent 切换 provider 时数据会丢。

**备选 B（驳回）：** 用 struct + 一组 Option 字段（`text: Option<String>`, `signature: Option<String>`, `redacted_data: Option<String>`）。问题：表达性差，match 时无法穷尽，容易在边界处把"无 signature 的 Thinking"或"有 text 的 Redacted"这种非法状态构造出来。

### 决策 2：新增 `StreamEvent::ThinkingBlockDone(ThinkingBlock)` 与 `ThinkingDelta(String)` 并存

`ThinkingDelta` 继续承担"实时 TUI 渲染"的职责；`ThinkingBlockDone` 在 SSE `content_block_stop` 时由 Provider 一次性发出聚合好的完整 block。

**为什么：** Provider 解析阶段已经能拿到 delta 的累积文本和 `signature_delta` 提供的签名，让 Provider 自己聚合比把责任推给 agent loop 更内聚（每个 Provider 自己定义如何拼装签名）。Agent loop 只需在 `consume_stream` 里把 `ThinkingBlockDone` 顺序收集起来。

**备选（驳回）：** 在 agent loop 里拼装 thinking 文本，让 Provider 同时上抛 text delta 与 signature。问题：会让 agent loop 知道"thinking 和 signature 之间的对应关系"，跨 Provider 时极易出错（Google 也有 thought signature 概念，但语义完全不同）。

### 决策 3：`convert_messages` 把 `thinking_blocks` 按到达顺序整体放在 content 数组最前

只对**当前 agent 自己**的 assistant 消息生效；非当前 agent（`is_other_agent = true`）的 `thinking_blocks` 始终被丢弃（服务端只能验证 assistant 自己生产的 signature，别人的 thinking 没有协议意义）。

**为什么这样足够正确：** 项目未启用 `interleaved-thinking-2025-05-14` beta，单条 assistant message 内不会出现 `[thinking, text, thinking, tool_use]` 这种交错。常见形态是 `[thinking, text?, tool_use*]`，把 vec 整段前置不会破坏顺序约束。

**已知局限：** 若未来启用 interleaved beta 并真的让模型在一条 message 内交错产生 thinking 与 text，本设计会丢失 thinking 与 text/tool_use 的相对位置，可能扰乱后续工具语义。届时需要扩展为 `Vec<ContentBlock>` 统一抽象。本次先记录在"风险 / 取舍"里，不引入这个重构。

**备选（驳回）：** 使用 `Vec<ContentBlock>` 保留 thinking / text / tool_use 的完整原始顺序。问题：会重构现有 `content` / `tool_calls` 表示并波及所有 Provider；本次未启用 interleaved beta，修复成本不匹配。

### 决策 4：TOML 持久化用 tagged enum 形式

`MessageEntry` 新增 `thinking_blocks: Option<Vec<ThinkingBlockEntry>>`，沿用现有 `skip_serializing_if = "Option::is_none"` 模式。`ThinkingBlockEntry` 序列化形态：

```toml
[[messages.thinking_blocks]]
block_type = "thinking"
text = "..."
signature = "..."

[[messages.thinking_blocks]]
block_type = "redacted_thinking"
data = "..."
```

**为什么：** TOML 对 inline tagged union 不友好，但对 `[[array.of.tables]]` 的可选字段处理很自然；用 `block_type` 字段做 tag 与 serde 的 `#[serde(tag = "block_type")]` 配合，能让 `thinking` 与 `redacted_thinking` 两种变体共用一个数组。空向量持久化为 `None`（不写键），与已有 `server_tool_uses: #[serde(default, skip_serializing_if = "Vec::is_empty")]` 风格略不同 —— 此处选 `Option` 是因为未来若新增字段（如 cache marker），保留 `Option` 给"缺字段 vs 实际为空"留出辨认余地。

**备选（驳回）：** 将 TOML 序列化为两个独立数组（如 `thinking_texts` / `redacted_data`）或 JSON 字符串。问题：独立数组会丢失 block 原始顺序（thinking 与 redacted 可能交错出现），JSON 字符串会显著降低 session TOML 的可读性与人工校验能力。

### 决策 5：persistence 双向转换在 ChatMessage（`Vec`）与 MessageEntry（`Option<Vec>`）之间归一

`ChatMessage.thinking_blocks: Vec<ThinkingBlock>` 与 `MessageEntry.thinking_blocks: Option<Vec<ThinkingBlockEntry>>` 的映射：

- `ChatMessage → MessageEntry`：空 vec → `None`；非空 vec → `Some(vec)`。
- `MessageEntry → ChatMessage`：`None` 或 `Some(empty)` → 空 vec；`Some(non-empty)` → 对应 vec。

**为什么：** 让 in-memory 类型保持简单（Vec 永远存在，不需要 unwrap），持久化层负责"空压缩"。

**备选（驳回）：** 让运行时 `ChatMessage.thinking_blocks` 也使用 `Option<Vec<_>>`。问题：会把持久化层的"缺字段语义"泄漏到运行时类型，所有读取点都得多一个 `None` 分支与 `unwrap_or_default()`；in-memory 模型没有"未知"与"已知为空"的区分需求。

### 决策 6：流中途出错时丢弃半截 thinking blocks

如果 SSE 流在某个 thinking block 的 `content_block_stop` 之前就 `Error` 或断流，该 block 的累积文本不会被上抛为 `ThinkingBlockDone`。半截没有 `signature` 的 thinking block 对 API 而言是非法输入，保留没有价值。

**为什么：** 与现有"工具调用参数没收全就不上抛 ToolCall"的处理风格一致（`anthropic.rs:563-577`）。

**备选（驳回）：** 流错误或断流时仍上抛半截 thinking block（仅有已收到的 text、`signature` 留空）。问题：没有 `signature` 的 thinking block 是非法历史，回传给 Anthropic 会立即再次触发 400；保留这种数据只会污染后续 turn 的回放。

## 风险 / 取舍

- **风险**：interleaved-thinking 未来启用后，单条 assistant message 内 thinking 与 text/tool_use 真正交错。 → 缓解：在 design.md 与 spec 中明确记录"按到达顺序整体前置"的简化假设。未来启用 interleaved beta 时同步扩展为 `Vec<ContentBlock>` 抽象，并新开一个 change。
- **风险**：`signature` 字段较长（base64 编码的加密 blob，约 1–4 KB），写入 session TOML 后单文件体积明显增大，长会话可能涉及上百 KB 级别。 → 缓解：TOML 是文本格式，加载性能可接受；如果未来确实成瓶颈，再引入"超过 N 轮后只保留最后 K 个 thinking"的裁剪策略（参考官方 `clear_thinking_20251015`）。
- **风险**：`other_agent_role = Assistant` 配置下，多个 agent 的消息会被 `merge_consecutive_same_role` 合并；若当前 agent 自己的两条 assistant 直接相邻（理论上 agent loop 不会产生这种序列），合并后会丢 thinking_blocks。 → 缓解：实际 agent loop 每条 assistant 后必然跟 user/tool 消息，不会触发该路径；测试中加一条防御性回归用例。
- **风险**：旧 session 文件加载后 `MessageEntry.thinking_blocks = None`；如果用户 `--resume` 续聊 Claude + 工具，第一轮接续的 assistant 历史里没有 thinking → 这本来就是历史现状，不引入新问题，但要避免误判为已修复后又出错。 → 缓解：在 release note 中说明"`--resume` 旧会话仍会面临首轮缺 thinking 的退化路径，建议从新 session 开始"。
- **取舍**：选用 `Vec<ThinkingBlock>` 而非 `Vec<ContentBlock>` 统一抽象，本次省去了对 `tool_calls` 与 `content` 字段的重构，代价是未来若真启用 interleaved thinking 需要再开 change。**接受**该取舍。
- **取舍**：未引入跨 Provider 的"thinking 能力探测"机制（如让 Provider trait 暴露 `supports_thinking_blocks()`），目前由 Provider 自行决定是否消费 `ChatMessage.thinking_blocks`。**接受**：当前只有 Anthropic + Vertex Anthropic 需要消费，写死"非 Anthropic provider 忽略"的实现成本更低。

## 迁移计划

无 schema migration。上线步骤：

1. 单元测试覆盖：SSE 聚合（含 redacted）、`convert_messages` 输出顺序、persistence 往返、跨 provider 忽略行为。
2. 集成测试覆盖：构造一个"Claude Opus 4.7 + enable_thinking=true + 一次工具循环"的最小端到端用例（可用 mock HTTP server，参考 `vertex_anthropic.rs` 中现有的 `run_capture_server` 模式）。
3. 手工验证：在本地 `.krew/settings.toml` 配置一个开了 thinking 的 Claude agent，跑一次 `read_file` + `shell` 工具循环，确认无 400。
4. 灰度：直接合入 `arch/wasm-plugins` 分支（线下分支），随后续版本一起发布。无需 feature flag。

**回滚方案：** 该变更纯加字段 + 加事件，未删任何旧路径。如出问题可直接 revert 提交。

## 遗留问题

- 是否需要对 `summarized` vs `omitted` display 模式做差异化处理？目前两者在我们的存储里都按 `Thinking { text, signature }` 落库；`omitted` 模式下 `text` 可能为空但 `signature` 仍有效，已能正常工作，但需要测试覆盖一次。
- `redacted_thinking` block 在哪些场景会出现？官方文档语焉不详（只说"安全过滤触发"）。需要在实现时找一个能稳定触发的 prompt 用于集成测试；如果找不到，可以用伪造 SSE 流的方式测 SSE 解析与往返。
