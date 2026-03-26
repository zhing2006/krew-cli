## MODIFIED Requirements

### Requirement: convert_messages 支持工具消息
各 Provider 的 `convert_messages()` SHALL 正确处理携带 `tool_calls` 的 assistant 消息和 `role: Tool` 的工具结果消息，转换为各 Provider 的 API 格式。当工具结果消息携带 `images` 时，SHALL 将图片数据序列化为 OpenAI Responses API 的 `input_image` content block。

#### Scenario: OpenAI Responses 工具消息转换（纯文本）
- **WHEN** convert_messages 遇到 `role: Tool` 消息且 `images` 为空
- **THEN** SHALL 生成 `{ "type": "function_call_output", "call_id": id, "output": result }`

#### Scenario: OpenAI Responses 工具消息转换（带图片）
- **WHEN** convert_messages 遇到 `role: Tool` 消息且 `images` 非空
- **THEN** SHALL 生成 `{ "type": "function_call_output", "call_id": id, "output": [{ "type": "input_image", "image_url": "data:{media_type};base64,{base64_data}", "detail": "auto" }, { "type": "input_text", "text": content }] }`

#### Scenario: OpenAI Responses 多张图片
- **WHEN** convert_messages 遇到 `role: Tool` 消息且 `images` 包含多张图片
- **THEN** 每张图片 SHALL 生成一个独立的 `{ "type": "input_image", ... }` content block，文本 block（`type: "input_text"`）放在最后
