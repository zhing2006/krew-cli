## MODIFIED Requirements

### Requirement: 对话轮次保留
系统 SHALL 在压缩时保留最近 N 个对话轮次，N 由 `compact_keep_rounds` 配置（默认 10）。一个对话轮次由一条用户消息和其后所有非用户消息（直到下一条用户消息）组成。带有 `whisper_targets` 的消息 SHALL 从压缩区提取并在 summary 之后保留，遵循与 skill activation 消息相同的提取-重插入模式。密语消息不会被送入压缩 LLM，也不包含在 summary 文本中。

#### Scenario: 默认保留最近 10 轮
- **WHEN** 会话有 25 个对话轮次，用户执行 `/compact`
- **THEN** 第 1-15 轮被压缩为 summary，第 16-25 轮原样保留

#### Scenario: 从压缩区提取密语消息
- **WHEN** 会话中密语消息与普通消息交错出现在压缩区
- **AND** 用户执行 `/compact`
- **THEN** 密语消息（带 `whisper_targets` 的用户消息及其对应的 agent 回复）SHALL 从压缩区提取，插入到 summary 和 skill 消息之后，完整保留其内容和 `whisper_targets` 元数据

#### Scenario: 保留区中的密语消息不变
- **WHEN** 密语消息存在于保留区（最近 N 轮）中
- **THEN** 它们 SHALL 在保留轮次中保持原位不变

#### Scenario: 压缩后消息顺序
- **WHEN** 压缩完成，压缩区中同时存在 skill 和密语消息
- **THEN** 最终消息列表 SHALL 为：`[Summary] + [skill messages] + [whisper messages] + [kept rounds]`

#### Scenario: 轮次少于保留阈值
- **WHEN** 会话有 5 个对话轮次且 `compact_keep_rounds` 为 10
- **THEN** 系统显示无内容可压缩的提示
