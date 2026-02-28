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
输入区域 SHALL 显示 `› ` 绿色提示符，支持用户输入文本。输入区域上下各有一条灰色分隔线，底部为状态栏。

#### Scenario: 输入框可见
- **WHEN** TUI 界面渲染完成
- **THEN** viewport SHALL 显示：分隔线 + `› ` 提示符和输入框 + 分隔线 + 状态栏

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
用户 SHALL 可以通过 `/quit`、`/exit` 命令或双击 Ctrl+C 退出程序。

#### Scenario: /quit 退出
- **WHEN** 用户在输入框中输入 `/quit` 并按 Enter
- **THEN** 程序 SHALL 正常退出，终端恢复原始状态

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
TUI SHALL 使用基于 tokio 的异步事件循环，使用 `tokio::select!` 同时监听 crossterm 终端事件。

#### Scenario: 响应键盘事件
- **WHEN** 用户按下键盘按键
- **THEN** 事件循环 SHALL 捕获并处理该键盘事件
