## MODIFIED Requirements

### Requirement: 链式触发下一个 Agent
`handle_agent_event(Done)` 中，当 `pending_agents` 不为空时，SHALL 弹出队首 Agent 并启动其 completion。当 `pending_agents` 为空时，SHALL 触发 pending message auto-drain。

#### Scenario: Done 后触发下一个 Agent
- **WHEN** Agent `"gpt"` 完成回复（Done 事件），且 `pending_agents` 含 `["opus", "gemini"]`
- **THEN** `"gpt"` 的回复追加到 `self.messages`，从队列弹出 `"opus"` 并调用 `start_completion(self.messages)`，`pending_agents` 变为 `["gemini"]`

#### Scenario: 队列清空后触发 pending message drain
- **WHEN** 最后一个 Agent 完成回复（Done 事件），且 `pending_agents` 为空
- **THEN** `agent_event_rx` 设为 None，系统 SHALL 调用 `drain_pending_message()` 尝试提交下一条 pending message

#### Scenario: drain 后有 pending message
- **WHEN** `drain_pending_message()` 被调用，`pending_messages` 含 `["@opus follow up"]`
- **THEN** 系统 SHALL pop_front 该消息，执行 send_message 流程，启动新的 Agent 调度

#### Scenario: drain 后无 pending message
- **WHEN** `drain_pending_message()` 被调用，`pending_messages` 为空
- **THEN** 系统 SHALL 不做任何操作，用户输入正常解锁

### Requirement: 错误隔离
`handle_agent_event(Error)` 中，当 `pending_agents` 不为空时，SHALL 跳过当前失败的 Agent 并继续启动下一个。当 `pending_agents` 为空时，SHALL 触发 pending message auto-drain。

#### Scenario: Agent 失败后继续下一个
- **WHEN** Agent `"gpt"` 发生错误（Error 事件），且 `pending_agents` 含 `["opus"]`
- **THEN** 显示 `"gpt"` 的错误信息，从队列弹出 `"opus"` 并启动其 completion

#### Scenario: 最后一个 Agent 失败后触发 drain
- **WHEN** 最后一个 Agent 发生错误（Error 事件），且 `pending_agents` 为空
- **THEN** 显示错误信息，`agent_event_rx` 设为 None，系统 SHALL 调用 `drain_pending_message()`

#### Scenario: 最后一个 Agent 失败且无 pending
- **WHEN** 最后一个 Agent 发生错误，`pending_agents` 为空，`pending_messages` 为空
- **THEN** 显示错误信息，用户输入正常解锁

### Requirement: ESC 取消后触发 drain
用户按 ESC 取消当前 Agent 响应时，SHALL 清空 `pending_agents`，将 `agent_event_rx` 设为 None，然后触发 `drain_pending_message()`。

#### Scenario: ESC 取消后有 pending message
- **WHEN** 用户按 ESC 取消当前 Agent 响应，`pending_messages` 含 `["@opus follow up"]`
- **THEN** `pending_agents` 被清空，`agent_event_rx` 设为 None，系统 SHALL 调用 `drain_pending_message()` 提交 pending message

#### Scenario: ESC 取消后无 pending message
- **WHEN** 用户按 ESC 取消当前 Agent 响应，`pending_messages` 为空
- **THEN** `pending_agents` 被清空，`agent_event_rx` 设为 None，用户输入正常解锁
