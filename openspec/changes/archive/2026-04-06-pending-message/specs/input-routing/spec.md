## MODIFIED Requirements

### Requirement: 消息历史管理
App SHALL 维护当前会话的消息历史列表（`Vec<ChatMessage>`）和 pending 消息队列（`VecDeque<PendingMessage>`），用于构建 LLM 请求的上下文和管理待发送消息。

#### Scenario: 用户消息加入历史
- **WHEN** 用户发送消息（直接发送或 pending auto-drain）
- **THEN** SHALL 构建 `ChatMessage { role: User, content, addressee }` 并追加到消息历史

#### Scenario: Agent 回复加入历史
- **WHEN** Agent 回复完成（收到 Done 事件）
- **THEN** SHALL 构建 `ChatMessage { role: Assistant, name: agent_name, content: 累积的完整回复, usage }` 并追加到消息历史

#### Scenario: 历史传递给 Agent
- **WHEN** 调用 `agent.complete()`
- **THEN** SHALL 传入完整的消息历史列表，使 LLM 具有上下文

#### Scenario: Agent 响应中 Enter 入队
- **WHEN** Agent 正在响应且 pending 队列未满，用户输入含 `@`/`#` 寻址的内容并按 Enter
- **THEN** SHALL 将 textarea 内容入队到 `pending_messages`，不加入消息历史（等 auto-drain 提交时才加入）

#### Scenario: Agent 响应中 Enter 无寻址目标
- **WHEN** Agent 正在响应且 pending 队列未满，用户输入无 `@`/`#` 的内容并按 Enter
- **THEN** SHALL 不入队，textarea 保留当前内容，显示提示信息

#### Scenario: Agent 响应中 Enter 队列已满
- **WHEN** Agent 正在响应且 pending 队列已满，用户按 Enter
- **THEN** SHALL 执行 `insert_newline()`，与当前行为一致

#### Scenario: 上箭头撤销 pending
- **WHEN** `pending_messages` 非空，光标在 textarea 第一行，用户按 ↑
- **THEN** SHALL 从 `pending_messages` pop_back 最后一条，直接替换 textarea 内容

#### Scenario: 上箭头调取历史
- **WHEN** `pending_messages` 为空，光标在 textarea 第一行，用户按 ↑
- **THEN** SHALL 执行原有的输入历史回调行为
