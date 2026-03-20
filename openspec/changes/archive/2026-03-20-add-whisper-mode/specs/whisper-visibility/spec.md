## ADDED Requirements

### Requirement: 密语消息过滤
`prepare_messages_for_agent()` SHALL 在为特定 agent 准备消息时，对带有 `whisper_targets` 的消息执行可见性过滤。当 agent 的名称不在 `whisper_targets` 列表中时，该消息 SHALL 被替换为占位符消息。

#### Scenario: 组外 agent 看到占位符
- **WHEN** 消息 `whisper_targets = Some(["opus", "gemini"])`
- **AND** 为 agent "gpt" 准备消息
- **THEN** 该消息 SHALL 被替换为包含 `[Whisper to opus, gemini]` 的占位符消息

#### Scenario: 组内 agent 看到原文
- **WHEN** 消息 `whisper_targets = Some(["opus", "gemini"])`
- **AND** 为 agent "opus" 准备消息
- **THEN** 该消息 SHALL 保持原样（内容不变）

#### Scenario: 无 whisper_targets 的消息不受影响
- **WHEN** 消息 `whisper_targets = None`
- **THEN** 该消息 SHALL 不受密语过滤影响，对所有 agent 正常展示

### Requirement: 占位符消息结构
密语占位符 SHALL 保留原消息的 `role` 和 `name` 字段，仅替换 `content`。占位符的 `whisper_targets` SHALL 设为 `None`（已经不需要再被过滤）。占位符的 `tool_calls` 和 `tool_call_id` SHALL 为 `None`。

#### Scenario: User 密语消息的占位符
- **WHEN** 原消息 `role = User`，`whisper_targets = Some(["opus"])`
- **AND** 为组外 agent 生成占位符
- **THEN** 占位符 SHALL 为 `ChatMessage { role: User, content: "[Whisper to opus]", name: None, whisper_targets: None, tool_calls: None }`

#### Scenario: Assistant 密语回复的占位符
- **WHEN** 原消息 `role = Assistant`，`name = Some("opus")`，`whisper_targets = Some(["opus", "gemini"])`
- **AND** 为组外 agent 生成占位符
- **THEN** 占位符 SHALL 为 `ChatMessage { role: Assistant, content: "[Whisper]", name: Some("opus"), whisper_targets: None, tool_calls: None }`

#### Scenario: 连续密语消息折叠
- **WHEN** 连续多条消息属于同一个密语组且对组外 agent 不可见
- **THEN** 每条消息 SHALL 各自独立替换为占位符（不合并为一个），以保持 role 交替正确

### Requirement: 密语工具调用链整体过滤
当密语消息包含 tool_calls 及后续 Tool result 消息时，整个工具调用链 SHALL 作为一个整体被替换为单个 Assistant 占位符（对组外 agent），或保持原样（对组内 agent）。

#### Scenario: 组外 agent 看不到密语的工具调用
- **WHEN** opus 在密语模式下调用了 read_file 工具
- **AND** 为 agent "gpt"（组外）准备消息
- **THEN** opus 的 assistant+tool_calls 消息和后续 Tool result 消息 SHALL 全部被替换为单个 `ChatMessage { role: Assistant, content: "[Whisper]", name: Some("opus") }` 占位符

#### Scenario: 组内 agent 看到密语的工具调用
- **WHEN** opus 在密语模式下调用了 read_file 工具
- **AND** 为 agent "gemini"（组内）准备消息
- **THEN** opus 的工具调用链 SHALL 按现有逻辑处理（转为文本描述，因为是 other agent 的工具调用）

### Requirement: 密语回复自动继承 whisper_targets
agent 在密语模式下产出的所有消息（文本回复、tool_calls、tool results）SHALL 自动继承触发密语的 `whisper_targets`。包括正常完成、错误和取消路径下合成的消息。

#### Scenario: agent 文本回复继承密语标记
- **WHEN** agent "opus" 因密语被触发（whisper_targets = ["opus", "gemini"]）
- **AND** opus 产出文本回复
- **THEN** 该回复的 `whisper_targets` SHALL 为 `Some(["opus", "gemini"])`

#### Scenario: agent 工具调用中间消息继承密语标记
- **WHEN** agent "opus" 在密语模式下执行工具调用
- **THEN** tool_calls 消息和 Tool result 消息的 `whisper_targets` SHALL 均为 `Some(["opus", "gemini"])`

#### Scenario: 错误合成消息继承密语标记
- **WHEN** agent 在密语模式下发生错误并产出部分文本
- **THEN** 合成的 `[Error: ...]` 消息 SHALL 继承 `whisper_targets`

#### Scenario: 取消合成消息继承密语标记
- **WHEN** 用户在密语模式下按 ESC 取消 agent 回复
- **THEN** 合成的 `[Cancelled by user]` 消息 SHALL 继承 `whisper_targets`
