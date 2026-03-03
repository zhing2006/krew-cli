## MODIFIED Requirements

### Requirement: Commit Tick 编排
流式渲染期间 SHALL 以 ~60Hz 频率执行 commit tick，驱动队列 drain 和行插入。commit tick SHALL 同时处理文本行和工具调用行的渲染。

#### Scenario: 流式开始启动 tick
- **WHEN** 收到第一个 `AgentEvent::TextDelta`
- **THEN** SHALL 启动 commit tick 动画（通过 FrameScheduler 持续请求重绘）

#### Scenario: tick 执行 drain
- **WHEN** commit tick 触发
- **THEN** SHALL 调用 `AdaptiveChunkingPolicy::decide()` 获取 drain 数量，从 StreamState 中取出对应行数，调用 `insert_lines_above()` 渲染

#### Scenario: 流结束停止 tick
- **WHEN** 收到 `AgentEvent::Done` 且队列已清空
- **THEN** SHALL 停止 commit tick 动画

#### Scenario: 工具调用行插入
- **WHEN** 收到 `AgentEvent::ToolCallStart`
- **THEN** SHALL 将工具调用行（`⚡ tool_name(args)`）作为独立行插入到流式输出区域

#### Scenario: 工具调用完成更新
- **WHEN** 收到 `AgentEvent::ToolCallDone`
- **THEN** SHALL 更新对应工具调用行，追加结果摘要（`— N lines`）
