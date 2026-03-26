## MODIFIED Requirements

### Requirement: ChatMessage 结构体
`krew-llm` SHALL 定义 `ChatMessage` 结构体，包含字段：`role: ChatRole`、`content: String`、`name: Option<String>`、`tool_calls: Option<Vec<ToolCallInfo>>`、`tool_call_id: Option<String>`、`server_tool_uses: Vec<ServerToolUseInfo>`、`addressee: Option<String>`、`created_at: DateTime<Utc>`、`usage: Option<Usage>`、`whisper_targets: Option<Vec<String>>`、`images: Vec<ImageContent>`。

#### Scenario: ChatMessage 结构体可导入
- **WHEN** 导入 `krew_llm::ChatMessage`
- **THEN** 该类型 SHALL 可访问并包含所有指定字段，含新增的 `images`

#### Scenario: 普通文本消息
- **WHEN** 构造普通用户或 assistant 消息
- **THEN** `whisper_targets` SHALL 为 `None`，`images` SHALL 为空 `Vec`

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

#### Scenario: 带图片的工具结果消息
- **WHEN** 工具执行返回图片数据
- **THEN** SHALL 构造 `ChatMessage { role: Tool, tool_call_id: Some(id), content: text, images: vec![...], ... }`

#### Scenario: images 字段不参与序列化
- **WHEN** 对 `ChatMessage` 进行 serde 序列化
- **THEN** `images` 字段 SHALL 被跳过（`#[serde(skip)]`）

#### Scenario: 反序列化时 images 默认为空
- **WHEN** 从 JSON 反序列化 `ChatMessage`（无 `images` 键）
- **THEN** `images` 字段 SHALL 默认为空 `Vec`
