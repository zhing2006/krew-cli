## ADDED Requirements

### Requirement: Anthropic Messages 流式请求
`AnthropicClient` SHALL 实现 `LlmClient` trait，向 Anthropic Messages API (`POST /v1/messages`) 发送流式请求。请求 SHALL 设置 `"stream": true`，header SHALL 包含 `anthropic-version: 2023-06-01`、`content-type: application/json`、`x-api-key: {api_key}`。

#### Scenario: 基本流式请求
- **WHEN** 调用 `chat_stream()` 传入消息列表
- **THEN** SHALL 发送 POST 请求到 `{base_url}/v1/messages`，body 包含 `model`、`messages`、`max_tokens`、`stream: true`

#### Scenario: 自定义 base_url
- **WHEN** `ProviderConfig.base_url` 有值
- **THEN** SHALL 使用该 base_url 替代默认的 `https://api.anthropic.com`

#### Scenario: max_tokens 必填
- **WHEN** `SamplingConfig.max_tokens` 为 None
- **THEN** SHALL 根据模型名称自动填入最大值（Opus 4.6+: 128000, Sonnet 4.5/4.6/5: 64000, Haiku 4.5: 64000, 其他: 32000）

### Requirement: Anthropic SSE 事件解析
客户端 SHALL 解析 Anthropic 的 SSE 事件类型，映射为 `StreamEvent`。

#### Scenario: text_delta 映射
- **WHEN** 收到 `event: content_block_delta` 且 delta type 为 `text_delta`
- **THEN** SHALL 产出 `StreamEvent::TextDelta(text)`

#### Scenario: thinking_delta 映射
- **WHEN** 收到 `event: content_block_delta` 且 delta type 为 `thinking_delta`
- **THEN** SHALL 产出 `StreamEvent::ThinkingDelta(thinking)`

#### Scenario: tool_use 输入流式解析
- **WHEN** 收到 `event: content_block_start` 且 content_block type 为 `tool_use`，随后收到 `input_json_delta` 事件
- **THEN** SHALL 累积 JSON 片段，在 `content_block_stop` 时产出 `StreamEvent::ToolCall { id, name, arguments }`

#### Scenario: message_stop 映射为 Done
- **WHEN** 收到 `event: message_delta`（含 usage）和 `event: message_stop`
- **THEN** SHALL 产出 `StreamEvent::Done(usage)`，usage 中 `input_tokens` 映射为 `prompt_tokens`，`output_tokens` 映射为 `completion_tokens`

#### Scenario: ping 事件忽略
- **WHEN** 收到 `event: ping`
- **THEN** SHALL 忽略，不产出任何 StreamEvent

#### Scenario: error 事件
- **WHEN** 收到 `event: error`
- **THEN** SHALL 产出 `StreamEvent::Error(message)`

### Requirement: Anthropic 认证
客户端 SHALL 在请求 header 中添加 `x-api-key: {api_key}`（不使用 Bearer token 方式）。

#### Scenario: 正常认证
- **WHEN** api_key_env 指定的环境变量存在且非空
- **THEN** SHALL 在 header 添加 `x-api-key: {value}` 和 `anthropic-version: 2023-06-01`

#### Scenario: 环境变量缺失
- **WHEN** 指定的环境变量不存在或为空
- **THEN** SHALL 返回 `LlmError::Auth` 错误

### Requirement: Anthropic 采样参数映射
客户端 SHALL 将 `SamplingConfig` 映射为 Anthropic API 参数。

#### Scenario: temperature 映射（范围 0-1）
- **WHEN** `SamplingConfig.temperature` 为 `Some(1.5)`
- **THEN** SHALL clamp 到 1.0 并在请求中设置 `"temperature": 1.0`

#### Scenario: top_k 映射
- **WHEN** `SamplingConfig.top_k` 为 `Some(40)`
- **THEN** SHALL 在请求中设置 `"top_k": 40`

#### Scenario: stop_sequences 映射
- **WHEN** `SamplingConfig.stop_sequences` 为 `Some(vec!["STOP"])`
- **THEN** SHALL 在请求中设置 `"stop_sequences": ["STOP"]`

#### Scenario: 不支持的参数忽略
- **WHEN** `SamplingConfig.frequency_penalty` 或 `presence_penalty` 有值
- **THEN** SHALL 忽略这些参数，不包含在请求中

### Requirement: Anthropic 消息格式转换
客户端 SHALL 将统一 `ChatMessage` 转换为 Anthropic 格式，system 消息分离到顶层 `system` 字段。`convert_messages` SHALL 接受 `other_agent_role: &OtherAgentRole` 参数，根据该参数决定 other-agent 消息的 role。

#### Scenario: System 消息分离
- **WHEN** Messages 包含 `ChatRole::System`
- **THEN** System 内容放入顶层 `system` 字段，不在 messages 数组中

#### Scenario: 其他 Agent 回复使用 OtherAgentRole
- **WHEN** Message role 为 Assistant 且 agent_name != 当前 agent，`other_agent_role` 为 `User`
- **THEN** 转换为 `"role": "user"` 并添加 `[agent_name]` content 前缀

#### Scenario: OtherAgentRole 为 Assistant
- **WHEN** Message role 为 Assistant 且 agent_name != 当前 agent，`other_agent_role` 为 `Assistant`
- **THEN** 转换为 `"role": "assistant"` 并添加 `[agent_name]` content 前缀

#### Scenario: 连续同 role 消息合并
- **WHEN** 转换后存在连续相同 role 的消息
- **THEN** 使用 `merge_consecutive_same_role` 合并，content 用 `\n\n` 连接

### Requirement: Anthropic 错误处理与重试
客户端 SHALL 使用 `common.rs` 的公共重试逻辑处理 429、5xx、超时等错误。

#### Scenario: 重试行为一致
- **WHEN** API 返回 429 或 5xx
- **THEN** SHALL 使用与 OpenAI Chat 相同的重试策略（429 指数退避 3 次，5xx 固定 2 次）

### Requirement: Anthropic tool 定义格式
客户端 SHALL 将 `ToolDefinition` 转换为 Anthropic 格式的 tools 参数。

#### Scenario: tool 格式转换
- **WHEN** tools 列表非空
- **THEN** SHALL 转换为 `{"name": "...", "description": "...", "input_schema": {...}}` 格式（注意使用 `input_schema` 而非 `parameters`）

## MODIFIED Requirements

### Requirement: convert_messages 支持工具消息
各 Provider 的 `convert_messages()` SHALL 正确处理携带 `tool_calls` 的 assistant 消息和 `role: Tool` 的工具结果消息，转换为各 Provider 的 API 格式。当工具结果消息携带 `images` 时，SHALL 将图片数据序列化为 Anthropic API 的图片 content block。

#### Scenario: Anthropic 工具消息转换（纯文本）
- **WHEN** convert_messages 遇到 `role: Tool` 消息且 `images` 为空
- **THEN** SHALL 生成 `{ "role": "user", "content": [{ "type": "tool_result", "tool_use_id": id, "content": result }] }`

#### Scenario: Anthropic 工具消息转换（带图片）
- **WHEN** convert_messages 遇到 `role: Tool` 消息且 `images` 非空
- **THEN** SHALL 生成 `{ "role": "user", "content": [{ "type": "tool_result", "tool_use_id": id, "content": [{ "type": "image", "source": { "type": "base64", "media_type": media_type, "data": base64_data } }, { "type": "text", "text": content }] }] }`

#### Scenario: Anthropic 多张图片
- **WHEN** convert_messages 遇到 `role: Tool` 消息且 `images` 包含多张图片
- **THEN** 每张图片 SHALL 生成一个独立的 `{ "type": "image", "source": { ... } }` content block，文本 block 放在最后

### Requirement: Anthropic client merges extra headers
The `AnthropicClient` SHALL append user-configured extra headers after its existing hardcoded headers (`anthropic-version`, `content-type`). Users MUST NOT configure header names that conflict with these hardcoded headers; behavior when conflicting headers are configured is undefined.

#### Scenario: User extra headers combined with hardcoded
- **WHEN** `AnthropicClient` is constructed with extra_headers containing `[("X-Custom", "val")]`
- **THEN** `send_with_retry()` SHALL receive headers containing both `anthropic-version`, `content-type`, and `X-Custom`

#### Scenario: No user extra headers
- **WHEN** `AnthropicClient` is constructed without extra_headers (empty vec)
- **THEN** behavior SHALL be identical to current implementation (only hardcoded headers)
