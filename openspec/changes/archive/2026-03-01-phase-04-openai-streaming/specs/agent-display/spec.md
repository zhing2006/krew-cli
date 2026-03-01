## ADDED Requirements

### Requirement: Agent 回复标签
Agent 回复 SHALL 以带颜色的标签头部开始显示，格式为 `[name] DisplayName:`。

#### Scenario: 标签颜色
- **WHEN** 收到 `AgentEvent::ResponseStart { agent_name: "gpt", display_name: "GPT-5.2", color: "green" }`
- **THEN** SHALL 在 viewport 上方插入一行：`[gpt] GPT-5.2:`，其中 `[gpt]` 和 `GPT-5.2:` 使用 agent 配置的颜色（green）渲染

#### Scenario: 标签与内容分离
- **WHEN** Agent 回复开始
- **THEN** 标签行 SHALL 独立一行显示，后续流式内容从下一行开始，带 2 空格缩进

### Requirement: 回复内容缩进
Agent 回复的正文内容 SHALL 以 2 空格缩进显示，与标签行形成视觉层次。

#### Scenario: 正文缩进
- **WHEN** 流式内容行被插入显示
- **THEN** 每行 SHALL 带 2 空格前缀缩进

### Requirement: Token 用量显示
Agent 回复结束后 SHALL 在末尾显示 token 用量信息。

#### Scenario: 用量格式
- **WHEN** 收到 `AgentEvent::Done(Usage { prompt_tokens: 156, completion_tokens: 89, .. })`
- **THEN** SHALL 在回复内容下方插入右对齐的灰色用量行：`── tokens: 156 in / 89 out`

#### Scenario: 千位分隔符
- **WHEN** token 数量 >= 1000
- **THEN** SHALL 使用千位逗号分隔，如 `── tokens: 2,847 in / 1,203 out`

### Requirement: 错误显示
Agent 对话出错时 SHALL 显示错误信息。

#### Scenario: LLM 错误
- **WHEN** 收到 `AgentEvent::Error(message)`
- **THEN** SHALL 在 Agent 标签下方显示红色错误信息：`  ✗ {message}`
