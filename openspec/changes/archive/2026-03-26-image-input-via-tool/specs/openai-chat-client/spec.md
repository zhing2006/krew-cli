## MODIFIED Requirements

### Requirement: convert_messages 支持工具消息
各 Provider 的 `convert_messages()` SHALL 正确处理携带 `tool_calls` 的 assistant 消息和 `role: Tool` 的工具结果消息，转换为各 Provider 的 API 格式。当工具结果消息携带 `images` 时，OpenAI Chat Completions API SHALL 降级处理，忽略图片数据。

#### Scenario: OpenAI Chat 工具消息转换（纯文本）
- **WHEN** convert_messages 遇到 `role: Tool` 消息且 `images` 为空
- **THEN** SHALL 生成 `{ "role": "tool", "tool_call_id": id, "content": result }`

#### Scenario: OpenAI Chat 工具消息转换（带图片降级）
- **WHEN** convert_messages 遇到 `role: Tool` 消息且 `images` 非空
- **THEN** SHALL 忽略图片数据，仅使用文本 `content`，生成 `{ "role": "tool", "tool_call_id": id, "content": result }`
