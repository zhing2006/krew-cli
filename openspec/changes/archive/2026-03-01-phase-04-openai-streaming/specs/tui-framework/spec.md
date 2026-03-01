## MODIFIED Requirements

### Requirement: 事件循环
TUI SHALL 使用基于 tokio 的异步事件循环，使用 `tokio::select!` 同时监听 crossterm 终端事件和 AgentEvent channel。

#### Scenario: 响应键盘事件
- **WHEN** 用户按下键盘按键
- **THEN** 事件循环 SHALL 捕获并处理该键盘事件

#### Scenario: 响应 AgentEvent
- **WHEN** agent loop 通过 mpsc channel 发送 AgentEvent
- **THEN** 事件循环 SHALL 通过 `select!` 接收并处理该事件（TextDelta → 推入流式管线，Done → finalize 渲染，Error → 显示错误）

#### Scenario: 流式期间仍可接受输入
- **WHEN** Agent 正在流式回复中
- **THEN** 用户 SHALL 仍能在输入框中输入文字（事件循环 SHALL 同时处理键盘事件和 AgentEvent）

## ADDED Requirements

### Requirement: Commit Tick 动画集成
TUI 事件循环 SHALL 在流式渲染期间支持 commit tick 动画驱动。

#### Scenario: 启动 commit tick
- **WHEN** 收到第一个 `AgentEvent::TextDelta`
- **THEN** SHALL 启动 commit tick 定时器（~60Hz），通过 FrameScheduler 触发周期性重绘

#### Scenario: 停止 commit tick
- **WHEN** 收到 `AgentEvent::Done` 且流式队列已清空
- **THEN** SHALL 停止 commit tick 定时器

#### Scenario: tick 处理
- **WHEN** commit tick 定时器触发
- **THEN** SHALL 执行 AdaptiveChunkingPolicy 决策，从 StreamState drain 行并 insert_lines_above()
