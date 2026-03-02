## MODIFIED Requirements

### Requirement: 输入区域
输入区域 SHALL 显示 `› ` 绿色提示符，支持用户输入文本。输入区域上方有一条灰色分隔线（agent 活跃时分隔线上方还有状态行），下方为分隔线和状态栏（或补全弹窗）。输入处理流程 SHALL 区分 Slash 命令和普通消息：Slash 命令直接执行，普通消息经过 @ 寻址解析后处理。

#### Scenario: 输入框可见（空闲态）
- **WHEN** TUI 界面渲染完成且无 agent 活跃
- **THEN** viewport SHALL 显示 4 行：分隔线 + `› ` 提示符和输入框 + 分隔线 + 状态栏

#### Scenario: 输入框可见（agent 活跃态）
- **WHEN** TUI 界面渲染完成且有 agent 正在处理
- **THEN** viewport SHALL 显示 5 行：状态行 + 分隔线 + `› ` 提示符和输入框 + 分隔线 + 状态栏

#### Scenario: Slash 命令优先处理
- **WHEN** 用户输入以 `/` 开头的文本并按 Enter
- **THEN** 系统 SHALL 优先通过 `SlashCommand::from_input()` 识别并执行命令，不经过 @ 寻址解析

#### Scenario: 普通消息解析路由
- **WHEN** 用户输入非 `/` 开头的文本并按 Enter
- **THEN** 系统 SHALL 通过 `parse_input()` 解析寻址目标，然后通过校验函数校验 Agent 名称合法性

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

#### Scenario: 状态行期间保持帧刷新
- **WHEN** 状态行可见且 commit tick 未激活（如 ResponseStart 后 TextDelta 尚未到达）
- **THEN** FrameRequester SHALL 保持周期性帧刷新以更新 spinner 动画和计时器
