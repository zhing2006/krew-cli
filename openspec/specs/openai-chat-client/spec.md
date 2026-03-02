## ADDED Requirements

### Requirement: OpenAI Chat Completions 流式请求
`OpenAiChatClient` SHALL 实现 `LlmClient` trait，向 OpenAI Chat Completions API (`POST /v1/chat/completions`) 发送流式请求。请求 SHALL 设置 `stream: true` 和 `stream_options: { include_usage: true }`。

#### Scenario: 基本流式请求
- **WHEN** 调用 `chat_stream()` 传入消息列表和采样参数
- **THEN** SHALL 发送 POST 请求到 `{base_url}/v1/chat/completions`，body 包含 `model`、`messages`、`stream: true`、`stream_options: { include_usage: true }`

#### Scenario: 自定义 base_url
- **WHEN** `ProviderConfig.base_url` 有值
- **THEN** SHALL 使用该 base_url 替代默认的 `https://api.openai.com`

### Requirement: SSE 事件解析与 StreamEvent 映射
客户端 SHALL 使用 `eventsource-stream` 解析 SSE 响应，将 OpenAI 的 SSE 事件映射为统一的 `StreamEvent`。

#### Scenario: TextDelta 映射
- **WHEN** 收到 SSE 事件 `data: {"choices":[{"delta":{"content":"hello"}}]}`
- **THEN** SHALL 产出 `StreamEvent::TextDelta("hello")`

#### Scenario: ToolCall 事件跳过
- **WHEN** 收到 SSE 事件包含 `delta.tool_calls`
- **THEN** SHALL 产出 `StreamEvent::ToolCall`（Phase 4 中调用方会忽略此事件）

#### Scenario: 流结束
- **WHEN** 收到 SSE 事件 `data: [DONE]`
- **THEN** SHALL 产出 `StreamEvent::Done(usage)`，usage 从之前收到的含 `usage` 字段的 chunk 中提取

#### Scenario: usage 信息提取
- **WHEN** 流式 chunk 中包含 `usage: { prompt_tokens, completion_tokens, total_tokens }` 字段
- **THEN** SHALL 缓存该 usage 信息，在 Done 事件中携带

### Requirement: 认证
客户端 SHALL 在请求 header 中添加 `Authorization: Bearer {api_key}`，api_key 从 `ProviderConfig.api_key_env` 指定的环境变量中读取。

#### Scenario: 正常认证
- **WHEN** 环境变量存在且非空
- **THEN** SHALL 在请求 header 添加 `Authorization: Bearer {value}`

#### Scenario: 环境变量缺失
- **WHEN** 指定的环境变量不存在或为空
- **THEN** SHALL 返回 `LlmError::Auth` 错误，消息包含环境变量名称

### Requirement: 采样参数映射
客户端 SHALL 将 `SamplingConfig` 的字段映射为 OpenAI Chat Completions API 的请求参数。未设置的可选字段 SHALL 不包含在请求中。

#### Scenario: temperature 映射
- **WHEN** `SamplingConfig.temperature` 为 `Some(0.7)`
- **THEN** 请求 body SHALL 包含 `"temperature": 0.7`

#### Scenario: max_tokens 映射
- **WHEN** `SamplingConfig.max_tokens` 为 `Some(4096)`
- **THEN** 请求 body SHALL 包含 `"max_completion_tokens": 4096`

#### Scenario: 未设置的参数不发送
- **WHEN** `SamplingConfig.top_k` 为 `Some(value)`
- **THEN** 请求 body SHALL NOT 包含 `top_k`（OpenAI 不支持此参数）

#### Scenario: stop_sequences 映射
- **WHEN** `SamplingConfig.stop_sequences` 为 `Some(vec!["STOP"])`
- **THEN** 请求 body SHALL 包含 `"stop": ["STOP"]`

### Requirement: 错误处理与重试
客户端 SHALL 对可重试的错误实施自动重试策略。

#### Scenario: 429 Rate Limit 指数退避
- **WHEN** API 返回 429 状态码
- **THEN** SHALL 以指数退避重试（1s → 2s → 4s），最多 3 次。若仍失败，返回 `LlmError::Api`

#### Scenario: 5xx 服务端错误重试
- **WHEN** API 返回 500/502/503 状态码
- **THEN** SHALL 重试 2 次，间隔 2s。若仍失败，返回 `LlmError::Api`

#### Scenario: 401/403 认证错误不重试
- **WHEN** API 返回 401 或 403 状态码
- **THEN** SHALL 立即返回 `LlmError::Auth`，不重试

#### Scenario: 网络超时
- **WHEN** 请求发送后 60 秒内未收到首个 SSE 事件
- **THEN** SHALL 重试 1 次。若仍超时，返回 `LlmError::Network` 错误

#### Scenario: 流式中断
- **WHEN** SSE 流在未收到 `[DONE]` 前意外断开
- **THEN** SHALL 产出 `StreamEvent::Error("stream interrupted")` 后结束流

### Requirement: 消息格式转换
客户端 SHALL 将统一的 `ChatMessage` 列表转换为 OpenAI Chat Completions 的 `messages` 数组格式。转换后 SHALL 调用 `merge_consecutive_same_role` 合并连续相同 role 的消息。

#### Scenario: 基本角色映射
- **WHEN** `ChatMessage` 包含 role System/User/Assistant/Tool
- **THEN** 映射为 OpenAI `"system"`/`"user"`/`"assistant"`/`"tool"` role

#### Scenario: 其他 Agent 回复的 role 处理
- **WHEN** `ChatMessage` role 为 Assistant 且 agent_name != 当前 agent
- **THEN** 根据 `other_agent_role` 参数决定使用 `"user"` 或 `"assistant"` role，content 前缀添加 `[agent_name]`

#### Scenario: 连续同 role 消息合并
- **WHEN** 转换后存在连续相同 role 的消息（例如两条连续 user 消息）
- **THEN** 合并为单条消息，content 用 `\n\n` 连接
