## Why

krew-cli 当前在 Anthropic Provider 上有协议层缺陷：`ChatMessage` 没有 `thinking_blocks` 字段，SSE 解析显式忽略 `signature_delta`，`convert_messages` 也不会把上一轮 assistant 的 thinking blocks 回传给 API。按 Anthropic 官方协议，启用 extended thinking + tool use 时，必须把上一轮 assistant 的 thinking blocks（含签名）原封不动地随 `tool_result` 一起回传，否则服务端会拒绝请求。当前实现一旦同时开启 thinking 与工具调用就会直接 400 报错，多 Agent 场景下每个 Claude agent 各自的工具循环都会失败，是阻塞 Anthropic + 工具调用线上可用性的关键问题。

## What Changes

- **`Provider/Anthropic/thinking-blocks`：** Anthropic Provider 的 SSE 解析新增对 `thinking_delta` / `signature_delta` / `redacted_thinking` 的聚合，`convert_messages` 在序列化 assistant 消息时把 `thinking_blocks` 作为第一组 content block 输出，Vertex Anthropic 自动复用。
- **`LLM/Message/thinking-blocks`：** 在 `ChatMessage` 上新增 `thinking_blocks: Vec<ThinkingBlock>` 字段，定义 `ThinkingBlock` 类型（区分 `Thinking { text, signature }` 与 `Redacted { data }`），并新增 `StreamEvent::ThinkingBlockDone` 流事件用于上层聚合。
- **`Agent/Loop/thinking-aggregation`：** Agent loop 的 `consume_stream` 阶段累积每个 thinking block，构造下一轮 assistant `ChatMessage` 时把聚合好的 `thinking_blocks` 一起写入；不影响 `ThinkingDelta` 仍然实时转发给 TUI 渲染。
- **`Storage/Session/thinking-blocks`：** `MessageEntry` 所在的 session TOML schema 增加可选 `thinking_blocks` 数组，重新加载会话时能还原完整推理上下文；旧 session 文件缺字段时反序列化为 `None`，恢复为 `ChatMessage` 时归一为空列表，不需要 migration。

## 涉及能力

### 新增能力

- `Provider/Anthropic/thinking-blocks`：Anthropic Messages API 的 extended thinking block 在 SSE 流解析与历史回放两个方向上的完整协议支持，覆盖 `thinking` / `redacted_thinking` 两种 block 类型与 Vertex Anthropic 复用。
- `LLM/Message/thinking-blocks`：跨 Provider 的 `ChatMessage` 抽象上对 thinking blocks 的数据契约，包括 `ThinkingBlock` 类型定义、`ChatMessage.thinking_blocks` 字段、以及 `StreamEvent::ThinkingBlockDone` 聚合事件。
- `Agent/Loop/thinking-aggregation`：Agent loop 在单轮 LLM 调用结束、构造下一轮上下文时，把流式收到的 thinking block 文本 / 签名聚合并挂到 assistant `ChatMessage` 上的运行时行为。
- `Storage/Session/thinking-blocks`：Session TOML 持久化 schema 对 thinking blocks 的承载，确保 `--resume` 后回放给 LLM 的历史包含完整 thinking 记录。

### 修改能力

（无 —— 当前项目尚无任何已存在的 spec，所有能力均为新增。）

## 影响范围

- 代码（核心）：
  - `crates/krew-llm/src/lib.rs`（`ChatMessage` / `ThinkingBlock` 新类型）
  - `crates/krew-llm/src/types.rs`（`StreamEvent::ThinkingBlockDone` 新事件）
  - `crates/krew-llm/src/anthropic.rs`（SSE 解析、`convert_messages`、回归测试）
  - `crates/krew-llm/src/vertex_anthropic.rs`（自动通过复用获益，仅需新增测试）
  - `crates/krew-core/src/agent/agent_loop.rs`（`consume_stream` 聚合、`assistant_msg` 写入）
  - `crates/krew-storage/src/session_file.rs`（`MessageEntry` 新增可选字段）
  - `crates/krew-core/src/persistence.rs`（往返序列化 thinking blocks）
- 代码（无需改动）：OpenAI Responses 的 reasoning、Google 的 `thoughtSignature` 处理保持不动。
- API 协议：Anthropic Messages API、Vertex AI `:streamRawPredict` —— 仅消费现有协议字段，无外部接口变化。
- 持久化兼容：旧的 session TOML 缺 `thinking_blocks` 字段时自动当作空数组，无需 migration 脚本。
- 其他 Provider：OpenAI Chat / OpenAI Responses / Google 在 `convert_messages` 中遇到非空 `thinking_blocks` 时直接忽略（不构造对应 block），保持现有行为。
