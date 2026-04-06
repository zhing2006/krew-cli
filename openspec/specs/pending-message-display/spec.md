## ADDED Requirements

### Requirement: Pending 区域渲染
当 `pending_messages` 非空时，viewport SHALL 在顶部（textarea 之上）渲染 pending 区域，包含标题行和每条 pending message。

#### Scenario: 一条 pending message 的显示
- **WHEN** `pending_messages` 含 `["@opus hello"]`
- **THEN** viewport 顶部 SHALL 显示标题行 `┄ 待发送 (1) ┄` 和消息行 `⏳ ● @opus hello`（圆点为 opus 的配置颜色）

#### Scenario: 无 pending 时不显示
- **WHEN** `pending_messages` 为空
- **THEN** viewport SHALL 不渲染 pending 区域，布局与当前完全一致

#### Scenario: 多条 pending message 的显示
- **WHEN** `pending_messages` 含 `["@gpt first", "@all second"]`（假设 MAX > 1）
- **THEN** viewport 顶部 SHALL 显示标题行 `┄ 待发送 (2) ┄`，以及两条消息行，每条带对应的路由色点

### Requirement: Pending 消息路由色点
每条 pending message 的显示 SHALL 使用与已发送用户消息相同的路由色点逻辑：解析 `raw_input` 获取 addressee，显示对应 Agent 的彩色圆点。

#### Scenario: 单 Agent 定向色点
- **WHEN** pending message 为 `"@opus hello"`，opus 的配置颜色为 cyan
- **THEN** 消息行 SHALL 显示为 `⏳ ● @opus hello`，圆点为 cyan 色

#### Scenario: @all 多色点
- **WHEN** pending message 为 `"@all hello"`，agents 为 gpt(green)、opus(cyan)、gemini(yellow)
- **THEN** 消息行 SHALL 显示为 `⏳ ●●● @all hello`，每个圆点对应 Agent 颜色

#### Scenario: 密语消息锁图标
- **WHEN** pending message 为 `"#opus secret"`
- **THEN** 消息行 SHALL 显示为 `⏳ 🔒● #opus secret`，带锁图标和色点

### Requirement: Pending 消息单行截断
每条 pending message SHALL 截断为单行显示。超出终端宽度的内容 SHALL 用 `…` 截断。多行原始输入只显示第一行。

#### Scenario: 长消息截断
- **WHEN** pending message 内容超出终端宽度
- **THEN** 消息行 SHALL 在终端宽度边界截断，末尾显示 `…`

#### Scenario: 多行消息只显示首行
- **WHEN** pending message 的 raw_input 包含换行符
- **THEN** 消息行 SHALL 只显示第一行内容，末尾显示 `…`

### Requirement: Viewport 动态高度
viewport 高度 SHALL 根据 pending 区域大小动态调整。每条 pending 恒定占 1 行（标题行 + N 条消息行）。

#### Scenario: 有 pending 时视口扩展
- **WHEN** `pending_messages` 含 1 条消息
- **THEN** viewport 高度 SHALL 增加 2 行（1 行标题 + 1 行消息）

#### Scenario: pending 撤销后视口收缩
- **WHEN** 用户按 ↑ 撤销最后一条 pending，`pending_messages` 变为空
- **THEN** viewport 高度 SHALL 恢复到原始大小（textarea + status bar）

#### Scenario: auto-drain 后视口收缩
- **WHEN** pending message 被自动提交（渲染到 scrollback），`pending_messages` 变为空
- **THEN** viewport 高度 SHALL 恢复到原始大小

### Requirement: Overlay/Popup 时隐藏 Pending 区域
当 approval overlay 或 completion popup 处于活跃状态时，pending 区域 SHALL 暂时不渲染。队列状态不受影响，overlay/popup 关闭后恢复显示。

#### Scenario: Approval overlay 活跃时
- **WHEN** approval overlay 正在显示（工具审批），`pending_messages` 非空
- **THEN** viewport SHALL 不渲染 pending 区域，pending 队列保持不变

#### Scenario: Completion popup 活跃时
- **WHEN** completion popup 正在显示（@/# 自动补全），`pending_messages` 非空
- **THEN** viewport SHALL 不渲染 pending 区域，pending 队列保持不变

#### Scenario: Overlay/Popup 关闭后恢复
- **WHEN** overlay 或 popup 关闭，`pending_messages` 仍非空
- **THEN** viewport SHALL 恢复渲染 pending 区域
