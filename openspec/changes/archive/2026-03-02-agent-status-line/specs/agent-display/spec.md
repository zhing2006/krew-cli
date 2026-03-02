## ADDED Requirements

### Requirement: Agent 状态生命周期管理
Agent 事件处理 SHALL 驱动状态指示器的生命周期——在 ResponseStart 时记录开始时间和显示名，在 Done/Error 时清除。

#### Scenario: ResponseStart 启动状态
- **WHEN** 收到 `AgentEvent::ResponseStart { agent_name, display_name, color }`
- **THEN** SHALL 记录 `agent_start_time = Instant::now()`、`agent_display_name = display_name`、`agent_color = color`

#### Scenario: Done 清除状态
- **WHEN** 收到 `AgentEvent::Done`
- **THEN** SHALL 清除 `agent_start_time`、`agent_display_name`、`agent_color`（设为 None）

#### Scenario: Error 清除状态
- **WHEN** 收到 `AgentEvent::Error`
- **THEN** SHALL 清除 `agent_start_time`、`agent_display_name`、`agent_color`（设为 None）

#### Scenario: 多 agent 串行更新
- **WHEN** 第一个 agent Done 后立即收到第二个 agent 的 ResponseStart
- **THEN** 状态行 SHALL 无缝切换为第二个 agent 的名称，计时器重新从 0 开始
