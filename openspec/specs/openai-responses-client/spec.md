## ADDED Requirements

### Requirement: OpenAI Responses 流式请求
`OpenAiResponsesClient` SHALL 实现 `LlmClient` trait，向 OpenAI Responses API (`POST /v1/responses`) 发送流式请求。请求 SHALL 设置 `"stream": true`。

#### Scenario: 基本流式请求
- **WHEN** 调用 `chat_stream()` 传入消息列表
- **THEN** SHALL 发送 POST 请求到 `{base_url}/v1/responses`，body 包含 `model`、`input`、`stream: true`

#### Scenario: 自定义 base_url
- **WHEN** `ProviderConfig.base_url` 有值
- **THEN** SHALL 使用该 base_url 替代默认的 `https://api.openai.com`

### Requirement: Azure 模式
当 `ProviderConfig.azure_endpoint` 有值时，客户端 SHALL 切换到 Azure 模式。

#### Scenario: Azure endpoint
- **WHEN** `azure_endpoint = "https://myresource.openai.azure.com"`
- **THEN** SHALL 发送请求到 `https://myresource.openai.azure.com/openai/v1/responses`

#### Scenario: Azure 认证
- **WHEN** Azure 模式启用
- **THEN** SHALL 使用 `api-key: {api_key}` header（不使用 `Authorization: Bearer`）

### Requirement: OpenAI Responses SSE 事件解析
客户端 SHALL 解析 Responses API 的 SSE 事件，只处理核心事件类型，其余安全忽略。

#### Scenario: text delta 映射
- **WHEN** 收到 `event: response.output_text.delta` 且 data 含 `"delta": "hello"`
- **THEN** SHALL 产出 `StreamEvent::TextDelta("hello")`

#### Scenario: reasoning summary 映射
- **WHEN** 收到 `event: response.reasoning_summary_text.delta` 且 data 含 `"delta": "thinking..."`
- **THEN** SHALL 产出 `StreamEvent::ThinkingDelta("thinking...")`

#### Scenario: function call 映射
- **WHEN** 收到 `event: response.function_call_arguments.done` 且 data 含 `"arguments": "{...}"`
- **THEN** SHALL 从对应的 `response.output_item.added` 事件中获取 `call_id` 和 `name`，产出 `StreamEvent::ToolCall { id: call_id, name, arguments }`

#### Scenario: response completed 映射
- **WHEN** 收到 `event: response.completed`
- **THEN** SHALL 从 `response.usage` 中提取 `input_tokens`→`prompt_tokens`、`output_tokens`→`completion_tokens`，产出 `StreamEvent::Done(usage)`

#### Scenario: response failed 映射
- **WHEN** 收到 `event: response.failed`
- **THEN** SHALL 产出 `StreamEvent::Error(error_message)`

#### Scenario: 无关事件忽略
- **WHEN** 收到 `response.queued`、`response.in_progress`、`response.content_part.added` 等非核心事件
- **THEN** SHALL 安全忽略，不产出 StreamEvent

### Requirement: OpenAI Responses 认证
标准模式使用 Bearer token，Azure 模式使用 api-key header。

#### Scenario: 标准 Bearer 认证
- **WHEN** 非 Azure 模式
- **THEN** SHALL 在 header 添加 `Authorization: Bearer {api_key}`

#### Scenario: 环境变量缺失
- **WHEN** api_key_env 环境变量不存在或为空
- **THEN** SHALL 返回 `LlmError::Auth` 错误

### Requirement: OpenAI Responses 采样参数映射
客户端 SHALL 将 `SamplingConfig` 映射为 Responses API 参数。

#### Scenario: temperature 映射
- **WHEN** `SamplingConfig.temperature` 为 `Some(0.7)`
- **THEN** SHALL 在请求中设置 `"temperature": 0.7`

#### Scenario: max_tokens 映射
- **WHEN** `SamplingConfig.max_tokens` 为 `Some(4096)`
- **THEN** SHALL 在请求中设置 `"max_output_tokens": 4096`（注意字段名为 `max_output_tokens`）

#### Scenario: 不支持的参数忽略
- **WHEN** `SamplingConfig.frequency_penalty`、`presence_penalty`、`stop_sequences`、`top_k` 有值
- **THEN** SHALL 忽略这些参数

### Requirement: OpenAI Responses 消息格式转换
客户端 SHALL 将统一 `ChatMessage` 转换为 Responses API 的 `input` 数组格式。

#### Scenario: user 消息转换
- **WHEN** `ChatRole::User` 消息
- **THEN** SHALL 转换为 `{"type": "message", "role": "user", "content": "..."}`

#### Scenario: system 消息转换
- **WHEN** `ChatRole::System` 消息
- **THEN** SHALL 转换为 `{"type": "message", "role": "developer", "content": "..."}`（使用 `developer` role）

#### Scenario: 当前 Agent 回复转换
- **WHEN** `ChatRole::Assistant` 且为当前 agent
- **THEN** SHALL 转换为 `{"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "..."}], "status": "completed"}`

#### Scenario: 其他 Agent 回复转为 user
- **WHEN** `ChatRole::Assistant` 且 agent_name 不等于当前 agent
- **THEN** SHALL 转为 `{"type": "message", "role": "user", "content": "[agent_name] ..."}`

### Requirement: OpenAI Responses tool 定义格式
客户端 SHALL 将 `ToolDefinition` 转换为 Responses API 格式的 tools 参数。

#### Scenario: tool 格式转换
- **WHEN** tools 列表非空
- **THEN** SHALL 转换为 `{"type": "function", "name": "...", "description": "...", "parameters": {...}, "strict": true}` 格式

### Requirement: OpenAI Responses 错误处理与重试
客户端 SHALL 使用 `common.rs` 的公共重试逻辑。

#### Scenario: 重试行为一致
- **WHEN** API 返回 429 或 5xx
- **THEN** SHALL 使用与其他 Provider 相同的重试策略
