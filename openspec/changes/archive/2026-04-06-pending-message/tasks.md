## 1. 数据结构与常量

- [x] 1.1 在 `app/state.rs` 中定义 `PendingMessage` 结构体（含 `raw_input: String`）和 `MAX_PENDING_MESSAGES` 常量（值为 1）
- [x] 1.2 在 `App` struct 中添加 `pending_messages: VecDeque<PendingMessage>` 字段并初始化

## 2. 入队逻辑

- [x] 2.1 在 `app/message.rs` 中实现 `queue_message()` 方法：验证非空输入 + 必须含 @/# 寻址（LastRespondent 拒绝并提示，保留 textarea）、构建 PendingMessage、push_back 到队列、清空 textarea
- [x] 2.2 修改 `app/input.rs` 中 Enter 键处理：实现三态逻辑（send / queue / newline），根据 `agent_event_rx` 和 `pending_messages.len()` 判断

## 3. 撤销逻辑

- [x] 3.1 修改 `app/input.rs` 中上箭头键处理：光标在第一行时，优先检查 pending_messages 非空则 pop_back 并直接替换 textarea 内容，否则走原有输入历史逻辑

## 4. Auto-drain

- [x] 4.1 在 `app/message.rs` 中实现 `drain_pending_message()` 方法：pop_front 并复用 send_message 的核心逻辑提交消息
- [x] 4.2 修改 `app/state.rs` 中 `handle_agent_event(Done)` 分支：`start_next_agent()` 返回 false 后调用 `drain_pending_message()`
- [x] 4.3 修改 `app/state.rs` 中 `handle_agent_event(Error)` 分支：`pending_agents` 为空时调用 `drain_pending_message()`
- [x] 4.4 修改 `app/state.rs` 中 `cancel_agent_response()` 方法：清空 pending_agents 并置空 agent_event_rx 后调用 `drain_pending_message()`

## 5. Viewport 渲染

- [x] 5.1 在 `render/viewport.rs` 中实现 pending 区域渲染：标题行 + 消息行（带 ⏳ 前缀和路由色点，单行截断 + `…`），位于 viewport 最上方；approval overlay / completion popup 活跃时不渲染
- [x] 5.2 修改 viewport 高度计算：加入 `pending_area_height`（标题行 + 消息条数，无 pending 或 overlay/popup 活跃时为 0）

## 6. 测试与验证

- [x] 6.1 手动测试核心流程：Agent 响应中 Enter 入队 → 显示 pending → Agent 完成后 auto-drain → pending 消息被提交
- [x] 6.2 手动测试撤销流程：有 pending 时按 ↑ 撤销到 textarea → 无 pending 时 ↑ 调取历史
- [x] 6.3 手动测试边界条件：队列满时 Enter 为换行、空输入不入队、无 @/# 输入不入队并保留 textarea、ESC 取消后触发 drain、Error 后触发 drain
- [x] 6.4 手动测试渲染边界：长消息截断显示 `…`、多行消息只显示首行、approval overlay 时 pending 区域隐藏并恢复、completion popup 时 pending 区域隐藏并恢复
