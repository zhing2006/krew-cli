## ADDED Requirements

### Requirement: Pending message 入队
当 Agent 正在响应（`agent_event_rx.is_some()`）且 pending 队列未满（`pending_messages.len() < MAX_PENDING_MESSAGES`）时，用户按 Enter SHALL 将当前 textarea 内容作为 `PendingMessage` 入队到 `pending_messages: VecDeque<PendingMessage>`，并清空 textarea。

#### Scenario: Agent 响应中入队成功
- **WHEN** Agent 正在响应，pending 队列为空，用户在 textarea 输入 `@opus 这bug咋修？` 并按 Enter
- **THEN** 系统 SHALL 将 `PendingMessage { raw_input: "@opus 这bug咋修？" }` push_back 到 `pending_messages`，textarea SHALL 被清空

#### Scenario: 空输入不入队
- **WHEN** Agent 正在响应，textarea 为空（或仅空白字符），用户按 Enter
- **THEN** 系统 SHALL 不执行入队操作，textarea 保持当前状态

#### Scenario: 无寻址目标不入队
- **WHEN** Agent 正在响应，pending 队列为空，用户在 textarea 输入 `continue`（无 `@` 或 `#` 前缀）并按 Enter
- **THEN** 系统 SHALL 不执行入队操作，textarea SHALL 保留当前内容不清空，显示提示信息告知用户需使用 `@name` 或 `#name` 指定目标

#### Scenario: 队列已满回退为换行
- **WHEN** Agent 正在响应，`pending_messages.len() >= MAX_PENDING_MESSAGES`，用户按 Enter
- **THEN** 系统 SHALL 执行 `insert_newline()`（当前行为），不入队

### Requirement: Pending message 撤销
当 `pending_messages` 非空时，用户按上箭头键（光标在 textarea 第一行）SHALL 从队列尾部弹出最后一条消息，并将其 `raw_input` 内容放入 textarea。

#### Scenario: 撤销唯一一条 pending
- **WHEN** `pending_messages` 含 1 条消息 `"@opus hello"`，用户按 ↑（光标在第一行）
- **THEN** 系统 SHALL pop_back 该消息，textarea 内容 SHALL 变为 `"@opus hello"`，`pending_messages` SHALL 变为空

#### Scenario: 撤销多条时逐条弹出
- **WHEN** `pending_messages` 含 2 条消息 `["@gpt first", "@opus second"]`（假设 MAX > 1），用户按 ↑
- **THEN** 系统 SHALL pop_back `"@opus second"` 到 textarea，`pending_messages` 剩余 `["@gpt first"]`

#### Scenario: 无 pending 时保持原有行为
- **WHEN** `pending_messages` 为空，用户按 ↑（光标在第一行）
- **THEN** 系统 SHALL 执行原有的输入历史回调行为

#### Scenario: 光标不在第一行时正常移动
- **WHEN** textarea 有多行内容且光标不在第一行，用户按 ↑
- **THEN** 系统 SHALL 执行正常的光标上移，不触发撤销

#### Scenario: 撤销时 textarea 已有内容
- **WHEN** `pending_messages` 含 1 条消息 `"@opus hello"`，textarea 当前内容为 `"draft text"`，用户按 ↑（光标在第一行）
- **THEN** 系统 SHALL pop_back 该消息，textarea 内容 SHALL 被替换为 `"@opus hello"`（直接替换，不合并）

### Requirement: Pending message auto-drain
当所有 Agent 完成当前调度（`pending_agents` 为空且 `agent_event_rx` 变为 None）后，系统 SHALL 自动从 `pending_messages` 队头弹出一条消息并提交。

#### Scenario: 调度完成后自动提交
- **WHEN** 最后一个 Agent 完成回复（Done 事件），`pending_agents` 为空，`pending_messages` 含 `["@opus hello"]`
- **THEN** 系统 SHALL pop_front `"@opus hello"`，执行与 `send_message` 相同的流程（解析、渲染到 scrollback、加入历史、dispatch、启动 Agent）

#### Scenario: 调度完成但无 pending
- **WHEN** 最后一个 Agent 完成回复，`pending_messages` 为空
- **THEN** 系统 SHALL 不做任何操作，用户输入正常解锁

#### Scenario: 多条 pending 逐条提交
- **WHEN** 第一条 pending message 提交后触发的 Agent 完成回复，`pending_messages` 仍有剩余
- **THEN** 系统 SHALL 继续 pop_front 并提交下一条，形成链式自动提交

#### Scenario: Error 后也触发 drain
- **WHEN** 最后一个 Agent 发生错误（Error 事件），`pending_agents` 为空，`pending_messages` 非空
- **THEN** 系统 SHALL 触发 auto-drain，提交下一条 pending message

#### Scenario: ESC 取消后也触发 drain
- **WHEN** 用户按 ESC 取消当前 Agent 响应，`pending_agents` 被清空，`pending_messages` 非空
- **THEN** 系统 SHALL 触发 auto-drain，提交下一条 pending message

### Requirement: MAX_PENDING_MESSAGES 常量
系统 SHALL 定义 `const MAX_PENDING_MESSAGES: usize = 1` 常量控制队列上限。队列管理逻辑 SHALL 使用此常量而非硬编码数值。

#### Scenario: 常量值为 1
- **WHEN** 系统初始化
- **THEN** `MAX_PENDING_MESSAGES` SHALL 为 `1`

#### Scenario: 修改常量即可扩展
- **WHEN** 将 `MAX_PENDING_MESSAGES` 改为 `3`
- **THEN** 系统 SHALL 允许最多 3 条 pending message 入队，无需修改其他逻辑
