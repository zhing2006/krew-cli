## ADDED Requirements

### Requirement: 密语状态传播
App SHALL 维护 `current_whisper_targets: Option<Vec<String>>` 字段，在用户发送密语消息时设置。dispatch queue 中的每个 agent 执行时，其产出的所有消息 SHALL 继承 `current_whisper_targets`。

#### Scenario: 用户发送密语设置状态
- **WHEN** 用户发送 `#opus #gemini hello`
- **THEN** `current_whisper_targets` SHALL 设为 `Some(["opus", "gemini"])`

#### Scenario: 普通消息不设置密语状态
- **WHEN** 用户发送 `@opus hello`（普通消息）
- **THEN** `current_whisper_targets` SHALL 为 `None`

#### Scenario: LastRespondent 不继承密语
- **WHEN** 上一次回复是密语模式下的 opus
- **AND** 用户发送无前缀消息 `continue`
- **THEN** `current_whisper_targets` SHALL 为 `None`，消息以普通模式发送

### Requirement: 密语状态清除——正常完成
密语 dispatch queue 处理完毕且无后续密语 A2A 触发时，`current_whisper_targets` SHALL 被清除为 `None`。

#### Scenario: 密语 dispatch 正常完成后清除状态
- **WHEN** 密语 dispatch queue 中最后一个 agent 完成回复（Done 事件）
- **AND** 没有后续的密语 A2A 触发
- **THEN** `current_whisper_targets` SHALL 被清除为 `None`

### Requirement: 密语状态清除——错误路径
当 agent 在密语模式下发生错误（Error 事件）时，合成的部分文本消息 SHALL 继承 `current_whisper_targets`。如果 pending 队列为空，`current_whisper_targets` SHALL 被清除。

#### Scenario: 密语模式下 agent 错误，有部分文本
- **WHEN** agent "opus" 在密语模式（whisper_targets = ["opus", "gemini"]）下发生错误
- **AND** 已产出部分文本
- **THEN** 合成的 `[Error: ...]` assistant 消息 SHALL 继承 `whisper_targets = Some(["opus", "gemini"])`

#### Scenario: 密语模式下 agent 错误，中间消息继承
- **WHEN** agent 在密语模式下发生错误
- **AND** 有 intermediate_messages
- **THEN** 这些中间消息 SHALL 已带有 `whisper_targets`（由 agent loop 标记）

#### Scenario: 错误后 pending 队列为空时清除状态
- **WHEN** 密语模式下 agent 发生错误
- **AND** pending_agents 队列为空
- **THEN** `current_whisper_targets` SHALL 被清除为 `None`

#### Scenario: 错误后 pending 队列不为空时保留状态
- **WHEN** 密语模式下 agent 发生错误
- **AND** pending_agents 队列仍有其他密语组内 agent
- **THEN** `current_whisper_targets` SHALL 保持不变，继续调度下一个 agent

### Requirement: 密语状态清除——取消路径
当用户按 ESC 取消密语模式下的 agent 回复时，合成的取消消息 SHALL 继承 `current_whisper_targets`，且 `current_whisper_targets` SHALL 在清空 pending 队列后被清除。

#### Scenario: 密语模式下 ESC 取消，有部分文本
- **WHEN** 用户在密语模式下按 ESC 取消 agent 回复
- **AND** 已产出部分文本
- **THEN** 合成的 `[Cancelled by user]` assistant 消息 SHALL 继承当前的 `whisper_targets`

#### Scenario: 密语模式下 ESC 取消后清除状态
- **WHEN** 用户在密语模式下按 ESC 取消
- **THEN** `pending_agents` 被清空后，`current_whisper_targets` SHALL 被清除为 `None`

### Requirement: 用户密语消息 whisper_targets 标记
当 `is_whisper = true` 时，用户的 `ChatMessage` SHALL 设置 `whisper_targets` 为解析出的目标 Agent 列表。

#### Scenario: 密语用户消息标记
- **WHEN** 用户输入 `#opus hello`，`is_whisper = true`
- **THEN** 用户 ChatMessage 的 `whisper_targets` SHALL 为 `Some(["opus"])`
