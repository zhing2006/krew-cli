# thinking-blocks Specification

## Purpose
TBD - created by archiving a change. Update Purpose after archive.

## Requirements
### Requirement: ChatMessage 必须能携带 thinking blocks

`ChatMessage` 类型 MUST 暴露一个 `thinking_blocks: Vec<ThinkingBlock>` 字段，默认值为空向量。`ThinkingBlock` 类型 MUST 区分两种变体：`Thinking { text: String, signature: String }` 与 `Redacted { data: String }`，使下游（Provider 与持久化层）能完整还原 Anthropic 协议要求的 block 形态。

#### Scenario: 新建 ChatMessage 时 thinking_blocks 默认为空

- **WHEN** 使用 `ChatMessage::text(role, content, name)` 创建消息
- **THEN** 返回的 `ChatMessage.thinking_blocks` SHALL 是一个空的 `Vec<ThinkingBlock>`，不影响现有调用点

#### Scenario: ThinkingBlock 区分签名块与脱敏块

- **WHEN** 上层构造 `ThinkingBlock::Thinking{text, signature}` 与 `ThinkingBlock::Redacted{data}` 两种实例
- **THEN** 两种变体 MUST 互相可区分（match 时能落入不同分支），且 MUST 都能被 `Debug` 与 `Clone`

### Requirement: StreamEvent 必须能上抛聚合好的 thinking block

`StreamEvent` 枚举 MUST 新增一个 `ThinkingBlockDone(ThinkingBlock)` 变体，用于 Provider 在一个 thinking block 完整结束时把聚合好的内容传递给上层 agent loop。该事件 MUST 与现有 `ThinkingDelta(String)` 并存（后者继续负责实时 UI 渲染）。

#### Scenario: 普通 thinking block 完成时上抛聚合事件

- **WHEN** Provider 完成对一个 `type: "thinking"` block 的 SSE 聚合
- **THEN** Provider SHALL 通过 `StreamEvent::ThinkingBlockDone(ThinkingBlock::Thinking{text, signature})` 向上游发送，其中 `text` 与 `signature` 与服务端原始内容完全一致

#### Scenario: redacted block 完成时上抛 Redacted 变体

- **WHEN** Provider 完成对一个 `type: "redacted_thinking"` block 的接收
- **THEN** Provider SHALL 上抛 `StreamEvent::ThinkingBlockDone(ThinkingBlock::Redacted{data})`，且对应 `data` 字段 SHALL 与服务端原值字节对齐

### Requirement: 非 Anthropic Provider 必须在序列化时忽略 thinking_blocks

OpenAI Chat / OpenAI Responses / Google Provider 的请求构造逻辑 MUST 在遇到 `ChatMessage.thinking_blocks` 非空时直接忽略该字段，不向各自的 API 发送任何 thinking 相关 block，保持各 provider 现有协议行为不变。

#### Scenario: 同一条带 thinking_blocks 的消息发给 OpenAI 不报错

- **WHEN** 一条 `ChatMessage` 同时带有 `thinking_blocks` 与 `tool_calls`，被传入 `OpenAiChatClient::chat_stream`
- **THEN** 发出的请求体 MUST NOT 包含任何来自 `thinking_blocks` 的内容，且消息其他部分（text、tool_calls）SHALL 与未携带 thinking_blocks 时一致
