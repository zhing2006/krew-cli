## ADDED Requirements

### Requirement: Inline TUI 界面
`krew-cli` SHALL 基于 ratatui + crossterm 提供 inline 终端界面（不使用 alternate screen）。界面 SHALL 使用自定义 Terminal 实现（`custom_terminal.rs`），支持动态 viewport 高度调整。消息内容通过 `insert_before` 插入到 viewport 上方，自然滚入终端 scrollback 历史。

#### Scenario: 启动后显示 inline 界面
- **WHEN** 用户执行 `krew`
- **THEN** 终端 SHALL 进入 raw mode 并在当前光标位置显示 inline viewport（输入区域 + 状态栏），不切换 alternate screen

#### Scenario: 退出后恢复终端
- **WHEN** 用户退出 krew
- **THEN** 终端 SHALL 关闭 raw mode 和 keyboard enhancement，viewport 内容保留在终端中可见

### Requirement: 头部框显示
启动时，SHALL 在 viewport 上方插入一个圆角边框头部框，包含标题、工作目录和帮助提示。

#### Scenario: 启动显示头部框
- **WHEN** krew 启动完成
- **THEN** viewport 上方 SHALL 显示：
  - 第一行：`>_ ` (绿色加粗) + `Krew CLI ` (加粗) + `(v0.1.0)` (灰色)
  - 第二行：`Directory: ` (灰色) + 当前工作目录
  - 第三行：`Type ` (灰色) + `/help` (青色) + ` for commands` (灰色)
  - 外框为灰色圆角边框

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

### Requirement: 状态栏
输入区域下方 SHALL 显示状态栏，包含审批模式、auto-compact 状态和当前工作目录。

#### Scenario: 默认状态栏
- **WHEN** TUI 界面渲染完成
- **THEN** 状态栏 SHALL 显示灰色文字：`suggest · auto-compact off · <工作目录>`

#### Scenario: 退出提示覆盖状态栏
- **WHEN** 用户按下一次 Ctrl+C
- **THEN** 状态栏 SHALL 暂时显示红色加粗的退出提示文字

### Requirement: 多行输入
输入框 SHALL 支持多行输入。Shift+Enter 或 Ctrl+J 换行，Enter 发送。viewport 高度 SHALL 随输入行数动态调整。

#### Scenario: Shift+Enter 换行
- **WHEN** 用户在输入框中按下 Shift+Enter（需终端支持 keyboard enhancement）
- **THEN** 输入框 SHALL 在当前光标位置插入换行符，不发送消息

#### Scenario: Ctrl+J 换行
- **WHEN** 用户在输入框中按下 Ctrl+J
- **THEN** 输入框 SHALL 在当前光标位置插入换行符，不发送消息（作为 Shift+Enter 的 fallback）

#### Scenario: Enter 发送
- **WHEN** 用户在输入框中按下 Enter（无修饰键）
- **THEN** 输入框中的文本 SHALL 被发送，输入框 SHALL 清空

#### Scenario: 动态 viewport 高度
- **WHEN** 用户通过换行增加输入行数
- **THEN** viewport 高度 SHALL 自动增大以显示所有输入行。如果 viewport 超出屏幕底部，SHALL 自动 scroll up 腾出空间

### Requirement: 消息显示
消息 SHALL 通过 `insert_before` 插入到 viewport 上方，自然滚入终端 scrollback 历史。用户可使用终端原生滚动查看历史消息。

#### Scenario: 消息插入到 viewport 上方
- **WHEN** 用户发送消息
- **THEN** 用户消息和回复 SHALL 插入到 viewport 上方并滚入终端 scrollback 历史

### Requirement: 退出方式
用户 SHALL 可以通过 `/quit`、`/exit` 命令或双击 Ctrl+C 退出程序。退出命令 SHALL 通过统一的 Slash 命令系统处理，不再硬编码。

#### Scenario: /quit 退出
- **WHEN** 用户在输入框中输入 `/quit` 并按 Enter
- **THEN** 程序 SHALL 通过 SlashCommand 系统识别并正常退出

#### Scenario: /exit 退出
- **WHEN** 用户在输入框中输入 `/exit` 并按 Enter
- **THEN** 程序 SHALL 正常退出，终端恢复原始状态

#### Scenario: 单次 Ctrl+C 不退出
- **WHEN** 用户按下一次 Ctrl+C
- **THEN** 程序 SHALL NOT 退出，状态栏 SHALL 显示提示信息 "Press Ctrl+C again to quit"，提示 SHALL 在 1 秒后自动消失

#### Scenario: 1 秒内双击 Ctrl+C 退出
- **WHEN** 用户在 1 秒内连续按下两次 Ctrl+C
- **THEN** 程序 SHALL 正常退出，终端恢复原始状态

#### Scenario: 超时后 Ctrl+C 不退出
- **WHEN** 用户按下第一次 Ctrl+C 后超过 1 秒再按第二次
- **THEN** 程序 SHALL NOT 退出，SHALL 重新显示 "Press Ctrl+C again to quit" 提示并重置 1 秒计时

### Requirement: Keyboard Enhancement
TUI SHALL 在启动时尝试启用 crossterm keyboard enhancement（`DISAMBIGUATE_ESCAPE_CODES` + `REPORT_EVENT_TYPES`），用于区分 Shift+Enter 和 Enter。若终端不支持，SHALL 静默降级继续运行（此时用户可使用 Ctrl+J 换行）。

#### Scenario: 支持的终端
- **WHEN** 终端支持 keyboard enhancement
- **THEN** Shift+Enter SHALL 正常工作

#### Scenario: 不支持的终端
- **WHEN** 终端不支持 keyboard enhancement（如 legacy Windows console）
- **THEN** 系统 SHALL 静默继续，用户可使用 Ctrl+J 换行

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
