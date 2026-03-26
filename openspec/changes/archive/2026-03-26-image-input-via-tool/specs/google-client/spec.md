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
