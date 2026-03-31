## ADDED Requirements

### Requirement: Google Gemini 流式请求
`GoogleClient` SHALL 实现 `LlmClient` trait，向 Gemini API (`POST /models/{model}:streamGenerateContent?alt=sse`) 发送流式请求。

#### Scenario: 标准 Gemini API 请求
- **WHEN** 调用 `chat_stream()` 且未配置 Vertex AI
- **THEN** SHALL 发送 POST 请求到 `https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent?alt=sse&key={api_key}`

#### Scenario: 自定义 base_url
- **WHEN** `ProviderConfig.base_url` 有值
- **THEN** SHALL 使用该 base_url 替代默认 URL

### Requirement: Vertex AI 模式
当 `ProviderConfig` 中 `vertex_project` 和 `vertex_location` 均有值时，客户端 SHALL 切换到 Vertex AI endpoint。

#### Scenario: Vertex AI endpoint
- **WHEN** `vertex_project = "my-project"` 且 `vertex_location = "us-central1"`
- **THEN** SHALL 发送请求到 `https://us-central1-aiplatform.googleapis.com/v1/projects/my-project/locations/us-central1/publishers/google/models/{model}:streamGenerateContent?alt=sse`

#### Scenario: Vertex AI 认证
- **WHEN** Vertex AI 模式启用
- **THEN** SHALL 使用 `Authorization: Bearer {token}` 认证（token 从 api_key_env 环境变量读取），不使用 URL query 参数传递 key

### Requirement: Gemini SSE 事件解析
客户端 SHALL 解析 Gemini 的 SSE 响应（data-only 格式，无 event type），映射为 `StreamEvent`。

#### Scenario: text 内容映射
- **WHEN** 收到 `data: {"candidates":[{"content":{"parts":[{"text":"hello"}]}}]}`
- **THEN** SHALL 产出 `StreamEvent::TextDelta("hello")`

#### Scenario: thinking 内容映射
- **WHEN** 收到 part 中 `"thought": true` 的文本
- **THEN** SHALL 产出 `StreamEvent::ThinkingDelta(text)`

#### Scenario: functionCall 映射
- **WHEN** 收到 part 中包含 `functionCall: {name, args}`
- **THEN** SHALL 产出 `StreamEvent::ToolCall { id: 自动生成, name, arguments: args序列化为JSON字符串 }`

#### Scenario: 流结束与 usage
- **WHEN** 收到最后一个 chunk（含 `finishReason: "STOP"` 和 `usageMetadata`）
- **THEN** SHALL 产出 `StreamEvent::Done(usage)`，其中 `promptTokenCount` 映射为 `prompt_tokens`，`candidatesTokenCount` 映射为 `completion_tokens`

#### Scenario: 流意外断开
- **WHEN** SSE 流在未收到 finishReason 前断开
- **THEN** SHALL 产出 `StreamEvent::Error("stream interrupted")`

### Requirement: Gemini 认证
标准模式使用 API Key，Vertex AI 模式使用 Bearer token。

#### Scenario: 标准 API Key 认证
- **WHEN** 非 Vertex AI 模式
- **THEN** SHALL 在 URL 中附加 `key={api_key}` 查询参数

#### Scenario: 环境变量缺失
- **WHEN** api_key_env 环境变量不存在或为空
- **THEN** SHALL 返回 `LlmError::Auth` 错误

### Requirement: Gemini 采样参数映射
客户端 SHALL 将 `SamplingConfig` 映射为 `generationConfig` 对象中的参数。

#### Scenario: temperature 映射
- **WHEN** `SamplingConfig.temperature` 为 `Some(0.7)`
- **THEN** SHALL 在 `generationConfig` 中设置 `"temperature": 0.7`

#### Scenario: maxOutputTokens 映射
- **WHEN** `SamplingConfig.max_tokens` 为 `Some(8192)`
- **THEN** SHALL 在 `generationConfig` 中设置 `"maxOutputTokens": 8192`

#### Scenario: topK 映射
- **WHEN** `SamplingConfig.top_k` 为 `Some(40)`
- **THEN** SHALL 在 `generationConfig` 中设置 `"topK": 40`

#### Scenario: stopSequences 映射
- **WHEN** `SamplingConfig.stop_sequences` 有值
- **THEN** SHALL 在 `generationConfig` 中设置 `"stopSequences": [...]`

### Requirement: Gemini 消息格式转换
客户端 SHALL 将统一 `ChatMessage` 转换为 Gemini `contents` 格式（role 使用 `user`/`model`），system 消息分离到 `systemInstruction` 字段。`convert_messages` SHALL 接受 `other_agent_role: &OtherAgentRole` 参数，根据该参数决定 other-agent 消息的 role。

#### Scenario: role 映射
- **WHEN** `ChatRole::Assistant` 为当前 agent
- **THEN** 映射为 `"role": "model"`

#### Scenario: System 消息分离
- **WHEN** Messages 包含 `ChatRole::System`
- **THEN** 分离为请求体 `systemInstruction: {"parts": [{"text": "..."}]}`

#### Scenario: 其他 Agent 回复使用 OtherAgentRole
- **WHEN** Message role 为 Assistant 且 agent_name != 当前 agent，`other_agent_role` 为 `User`
- **THEN** 转换为 `"role": "user"` 并添加 `[agent_name]` content 前缀

#### Scenario: OtherAgentRole 为 Assistant
- **WHEN** Message role 为 Assistant 且 agent_name != 当前 agent，`other_agent_role` 为 `Assistant`
- **THEN** 转换为 `"role": "model"` 并添加 `[agent_name]` content 前缀

#### Scenario: 连续同 role 消息合并
- **WHEN** 转换后存在连续相同 role 的消息
- **THEN** 使用 `merge_consecutive_same_role` 合并，parts text 用 `\n\n` 连接

### Requirement: Gemini tool 定义格式
客户端 SHALL 将 `ToolDefinition` 转换为 Gemini 的 `functionDeclarations` 格式。

#### Scenario: tool 格式转换
- **WHEN** tools 列表非空
- **THEN** SHALL 转换为 `{"tools": [{"functionDeclarations": [{"name": "...", "description": "...", "parameters": {...}}]}]}` 格式

### Requirement: Gemini 错误处理与重试
客户端 SHALL 使用 `common.rs` 的公共重试逻辑。

#### Scenario: 重试行为一致
- **WHEN** API 返回 429 或 5xx
- **THEN** SHALL 使用与其他 Provider 相同的重试策略

## MODIFIED Requirements

### Requirement: convert_messages 支持工具消息
各 Provider 的 `convert_messages()` SHALL 正确处理携带 `tool_calls` 的 assistant 消息和 `role: Tool` 的工具结果消息，转换为各 Provider 的 API 格式。当工具结果消息携带 `images` 时，SHALL 将图片数据序列化为 Gemini API 的 `inlineData` part，嵌入 `functionResponse` 内部。

#### Scenario: Google 工具消息转换（纯文本）
- **WHEN** convert_messages 遇到 `role: Tool` 消息且 `images` 为空
- **THEN** SHALL 生成 `{ "role": "user", "parts": [{ "functionResponse": { "name": name, "id": tool_call_id, "response": { "result": content } } }] }`

#### Scenario: Google 工具消息转换（带图片）
- **WHEN** convert_messages 遇到 `role: Tool` 消息且 `images` 非空
- **THEN** SHALL 生成 `{ "role": "user", "parts": [{ "functionResponse": { "name": name, "id": tool_call_id, "response": { "image_ref": { "$ref": filename } }, "parts": [{ "inlineData": { "displayName": filename, "mimeType": media_type, "data": base64_data } }] } }] }`

#### Scenario: Google 多张图片
- **WHEN** convert_messages 遇到 `role: Tool` 消息且 `images` 包含多张图片
- **THEN** 每张图片 SHALL 生成一个独立的 `{ "inlineData": { ... } }` 元素，追加在 `functionResponse.parts` 数组中

#### Scenario: Google functionResponse 携带 id
- **WHEN** convert_messages 遇到 `role: Tool` 消息
- **THEN** `functionResponse` SHALL 包含 `id` 字段，值从 `ChatMessage.tool_call_id` 获取，与对应的 `functionCall.id` 匹配

### Requirement: Google client sends extra headers
The `GoogleClient` SHALL accept extra headers during construction and pass them to `send_with_retry()` in `chat_stream()`.

#### Scenario: Extra headers present
- **WHEN** `GoogleClient` is constructed with extra_headers containing `[("X-Foo", "bar")]`
- **THEN** every `chat_stream()` request SHALL include the `X-Foo: bar` HTTP header

#### Scenario: No extra headers
- **WHEN** `GoogleClient` is constructed without extra_headers (empty vec)
- **THEN** `send_with_retry()` SHALL receive `None` for extra_headers, maintaining current behavior
