## ADDED Requirements

### Requirement: 用户密语消息显示
用户密语消息 SHALL 在 `> ` 前缀后、彩色圆点前显示锁图标，以区别于普通消息。

#### Scenario: 单目标密语显示
- **WHEN** 用户发送 `#opus hello`
- **THEN** TUI SHALL 显示为 `> 🔒● hello`，锁图标在圆点之前，圆点颜色为 opus 的配置颜色

#### Scenario: 多目标密语显示
- **WHEN** 用户发送 `#opus #gemini discuss`
- **THEN** TUI SHALL 显示为 `> 🔒●● discuss`，锁图标后跟各 agent 颜色的圆点

### Requirement: Agent 密语回复 header 显示
当 agent 的回复属于密语对时，agent 响应的 header 区域 SHALL 包含锁图标标记。

#### Scenario: 密语回复 header
- **WHEN** agent "opus" 在密语模式下产出回复
- **THEN** agent 的 header SHALL 包含锁图标，如 `[opus] 🔒 Opus:`

#### Scenario: 普通回复 header 无锁
- **WHEN** agent "opus" 在普通模式下产出回复
- **THEN** agent 的 header SHALL 不包含锁图标

### Requirement: Resume 重放时密语显示
`--resume` 恢复会话重放历史消息时，SHALL 根据 `MessageEntry` 的 `whisper_targets` 字段正确显示密语标记。

#### Scenario: 重放用户密语消息
- **WHEN** 重放一条 `role = "user"` 且 `whisper_targets = ["opus", "gemini"]` 的消息
- **THEN** TUI SHALL 以密语格式显示：`> 🔒●● content`，圆点颜色根据目标 agent 的配置颜色渲染

#### Scenario: 重放 agent 密语回复
- **WHEN** 重放一条 `role = "assistant"` 且 `whisper_targets = ["opus", "gemini"]` 的消息
- **THEN** agent header SHALL 包含锁图标

#### Scenario: 重放普通消息不受影响
- **WHEN** 重放一条 `whisper_targets` 为空的消息
- **THEN** SHALL 按现有逻辑正常显示，无锁图标

### Requirement: P 模式密语输出标识
`-p` 模式下，密语响应的 agent header SHALL 包含 `[whisper]` 标记。

#### Scenario: P 模式 text 格式密语输出
- **WHEN** P 模式下 agent "opus" 在密语模式下响应
- **THEN** stdout SHALL 输出 `[opus] [whisper]` 作为 header 行

#### Scenario: P 模式 JSON 格式密语标记
- **WHEN** P 模式下以 JSON 格式输出密语 agent 的响应
- **THEN** JSON 对象 SHALL 包含 `"whisper_targets": ["opus", "gemini"]` 字段
