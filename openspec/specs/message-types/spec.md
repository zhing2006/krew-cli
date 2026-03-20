## MODIFIED Requirements

### Requirement: ChatMessage 结构体
`krew-llm` SHALL 定义 `ChatMessage` 结构体，包含字段：`role: ChatRole`、`content: String`、`name: Option<String>`、`tool_calls: Option<Vec<ToolCallInfo>>`、`tool_call_id: Option<String>`。

#### Scenario: ChatMessage 结构体可导入
- **WHEN** 导入 `krew_llm::ChatMessage`
- **THEN** 该类型 SHALL 可访问并包含所有指定字段（含新增的 tool_calls 和 tool_call_id）

#### Scenario: 普通文本消息
- **WHEN** 构造普通用户或 assistant 消息
- **THEN** `tool_calls` 和 `tool_call_id` SHALL 为 `None`

#### Scenario: 携带工具调用的 assistant 消息
- **WHEN** LLM 返回包含 ToolCall 的响应
- **THEN** SHALL 构造 `ChatMessage { role: Assistant, tool_calls: Some(vec![...]), ... }`

#### Scenario: 工具结果消息
- **WHEN** 工具执行完成
- **THEN** SHALL 构造 `ChatMessage { role: Tool, tool_call_id: Some(id), content: result, ... }`

### Requirement: ToolCallInfo 结构体
`krew-llm` SHALL 定义 `ToolCallInfo` 结构体，包含字段：`id: String`、`name: String`、`arguments: String`（JSON string）。

#### Scenario: ToolCallInfo 结构体字段
- **WHEN** 构造一个 `ToolCallInfo`
- **THEN** 三个字段 SHALL 均存在

### Requirement: convert_messages 支持工具消息
各 Provider 的 `convert_messages()` SHALL 正确处理携带 `tool_calls` 的 assistant 消息和 `role: Tool` 的工具结果消息，转换为各 Provider 的 API 格式。

#### Scenario: OpenAI Chat 工具消息转换
- **WHEN** convert_messages 遇到 `role: Tool` 消息
- **THEN** SHALL 生成 `{ "role": "tool", "tool_call_id": id, "content": result }`

#### Scenario: OpenAI Chat assistant 工具调用转换
- **WHEN** convert_messages 遇到携带 `tool_calls` 的 assistant 消息
- **THEN** SHALL 生成含 `tool_calls` 数组的 assistant 消息

#### Scenario: Anthropic 工具消息转换
- **WHEN** convert_messages 遇到 `role: Tool` 消息
- **THEN** SHALL 生成 `{ "role": "user", "content": [{ "type": "tool_result", "tool_use_id": id, "content": result }] }`

#### Scenario: Google 工具消息转换
- **WHEN** convert_messages 遇到 `role: Tool` 消息
- **THEN** SHALL 生成 `{ "role": "user", "parts": [{ "functionResponse": { "name": name, "response": { "result": content } } }] }`

#### Scenario: OpenAI Responses 工具消息转换
- **WHEN** convert_messages 遇到 `role: Tool` 消息
- **THEN** SHALL 生成 `{ "type": "function_call_output", "call_id": id, "output": result }`

## MODIFIED Requirements

### Requirement: ChatMessage 结构体
`krew-llm` SHALL 定义 `ChatMessage` 结构体，包含字段：`role: ChatRole`、`content: String`、`name: Option<String>`、`tool_calls: Option<Vec<ToolCallInfo>>`、`tool_call_id: Option<String>`、`server_tool_uses: Vec<ServerToolUseInfo>`、`addressee: Option<String>`、`created_at: DateTime<Utc>`、`usage: Option<Usage>`、`whisper_targets: Option<Vec<String>>`。

#### Scenario: ChatMessage 结构体可导入
- **WHEN** 导入 `krew_llm::ChatMessage`
- **THEN** 该类型 SHALL 可访问并包含所有指定字段，含新增的 `whisper_targets`

#### Scenario: 普通文本消息
- **WHEN** 构造普通用户或 assistant 消息
- **THEN** `whisper_targets` SHALL 为 `None`

#### Scenario: 密语消息
- **WHEN** 构造密语用户消息（用户输入 `#opus hello`）
- **THEN** `whisper_targets` SHALL 为 `Some(vec!["opus".to_string()])`

#### Scenario: 密语组消息
- **WHEN** 构造密语组用户消息（用户输入 `#opus #gemini discuss`）
- **THEN** `whisper_targets` SHALL 为 `Some(vec!["opus".to_string(), "gemini".to_string()])`

#### Scenario: 携带工具调用的 assistant 消息
- **WHEN** LLM 返回包含 ToolCall 的响应
- **THEN** SHALL 构造 `ChatMessage { role: Assistant, tool_calls: Some(vec![...]), ... }`

#### Scenario: 工具结果消息
- **WHEN** 工具执行完成
- **THEN** SHALL 构造 `ChatMessage { role: Tool, tool_call_id: Some(id), content: result, ... }`
