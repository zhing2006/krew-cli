# thinking-blocks Specification

## Purpose
TBD - created by archiving a change. Update Purpose after archive.

## Requirements
### Requirement: Anthropic SSE 流必须聚合 thinking block

Anthropic Provider 的 SSE 解析器 MUST 把同一个 `content_block` 内的所有 `thinking_delta` 文本与 `signature_delta` 签名聚合为一个独立事件，并在 `content_block_stop` 时一次性向上游发出，避免上层只看到流式增量而无法重建完整 block。

#### Scenario: 普通 thinking block 在 stop 时聚合输出

- **WHEN** SSE 流依次到达 `content_block_start{type: "thinking"}`、若干 `content_block_delta{thinking_delta}`、`content_block_delta{signature_delta}`、`content_block_stop`
- **THEN** 解析器 SHALL 发出至少一个 `StreamEvent::ThinkingDelta` 用于实时显示，并在 `content_block_stop` 时再发出一个 `StreamEvent::ThinkingBlockDone`，其中 `text` 为所有 `thinking_delta` 拼接结果，`signature` 为 `signature_delta` 中的字符串

#### Scenario: redacted_thinking block 不暴露明文但保留 data

- **WHEN** SSE 流到达 `content_block_start{type: "redacted_thinking", data: "<opaque>"}` 后立即 `content_block_stop`
- **THEN** 解析器 MUST NOT 发出任何 `StreamEvent::ThinkingDelta`（无明文可显示），并 SHALL 在 stop 时发出一个 `StreamEvent::ThinkingBlockDone`，其中 block 类型为 `Redacted`，保留服务端给出的 `data` 字段原值

### Requirement: convert_messages 必须把 thinking_blocks 作为 assistant 的首组 content block 输出

`convert_messages` 在序列化当前 agent 自己的 assistant 消息时，如果 `ChatMessage.thinking_blocks` 非空，MUST 在 `content` 数组的最前方按原顺序输出对应的 `thinking` / `redacted_thinking` block，然后才是 `text` 与 `tool_use` block；签名字段 MUST 原样回填。

#### Scenario: thinking + tool_use 在同一条 assistant 消息上回放

- **WHEN** 当前 agent 的 `ChatMessage` 同时携带 `thinking_blocks = [Thinking{text: "...", signature: "S"}]` 与 `tool_calls = [...]`
- **THEN** 序列化结果中 `content[0]` 的 `type` 字段 SHALL 等于 `"thinking"` 且 `signature` 字段 SHALL 等于 `"S"`，`tool_use` block SHALL 排在 thinking block 之后

#### Scenario: redacted_thinking 回放只携带 data

- **WHEN** assistant 消息携带 `thinking_blocks = [Redacted{data: "X"}]`
- **THEN** 序列化结果首个 block 的 `type` SHALL 为 `"redacted_thinking"` 且 `data` 字段 SHALL 等于 `"X"`，且 MUST NOT 出现 `thinking` 或 `signature` 字段

#### Scenario: 其他 agent 的消息不携带 thinking blocks

- **WHEN** assistant 消息来自非当前 agent（`is_other_agent = true`）且其 `thinking_blocks` 字段非空
- **THEN** 序列化结果 MUST NOT 在输出中包含任何 `thinking` 或 `redacted_thinking` block（其他 agent 的推理对当前 Claude 来说没有协议意义）

### Requirement: Vertex Anthropic 必须复用同一份转换逻辑

Vertex Anthropic Provider MUST 复用 `convert_messages` 与 SSE 事件解析的同一实现，使两条接入路径上的 thinking blocks 行为完全一致。

#### Scenario: Vertex 路径下 thinking + tool_use 仍能正确回放

- **WHEN** 使用 `VertexAnthropicClient` 发送带 `thinking_blocks` 的历史消息并启用工具
- **THEN** 实际发出的请求体 `messages[i].content` 数组中，首个 block SHALL 与 `AnthropicClient` 对同一输入产生的结果完全相同
