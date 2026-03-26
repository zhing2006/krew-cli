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
