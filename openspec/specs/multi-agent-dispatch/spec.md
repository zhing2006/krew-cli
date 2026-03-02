## ADDED Requirements

### Requirement: pending_agents 队列管理
App SHALL 维护 `pending_agents: VecDeque<String>` 字段，用于跟踪等待执行的 Agent 列表。队列在 `send_message()` 中填充，在 `handle_agent_event(Done/Error)` 中消费。

#### Scenario: @all 填充队列
- **WHEN** 用户输入 `@all hello`，且 `reply_order` 为 `["gpt", "opus", "gemini"]`，三个 Agent 均有 LLM client
- **THEN** `pending_agents` 填充为 `["opus", "gemini"]`（除首个外），首个 Agent `"gpt"` 立即启动 completion

#### Scenario: @all 跳过无 client 的 Agent
- **WHEN** 用户输入 `@all hello`，且 `reply_order` 为 `["gpt", "echo", "opus"]`，其中 `"echo"` 为 builtin 无 LLM client
- **THEN** `pending_agents` 仅包含有 LLM client 的 Agent，跳过 `"echo"`

#### Scenario: @multiple 按 @ 出现顺序
- **WHEN** 用户输入 `@opus @gpt 讨论一下`
- **THEN** `pending_agents` 填充为 `["gpt"]`（按用户 @ 出现顺序），首个 `"opus"` 立即启动

#### Scenario: @single 不使用队列
- **WHEN** 用户输入 `@opus hello`
- **THEN** `pending_agents` 为空，直接启动 `"opus"`

### Requirement: 链式触发下一个 Agent
`handle_agent_event(Done)` 中，当 `pending_agents` 不为空时，SHALL 弹出队首 Agent 并启动其 completion。

#### Scenario: Done 后触发下一个
- **WHEN** Agent `"gpt"` 完成回复（Done 事件），且 `pending_agents` 含 `["opus", "gemini"]`
- **THEN** `"gpt"` 的回复追加到 `self.messages`，从队列弹出 `"opus"` 并调用 `start_completion(self.messages)`，`pending_agents` 变为 `["gemini"]`

#### Scenario: 队列清空后解锁输入
- **WHEN** 最后一个 Agent 完成回复（Done 事件），且 `pending_agents` 为空
- **THEN** `agent_event_rx` 设为 None，用户可以输入新消息

### Requirement: 错误隔离
`handle_agent_event(Error)` 中，当 `pending_agents` 不为空时，SHALL 跳过当前失败的 Agent 并继续启动下一个。

#### Scenario: Agent 失败后继续下一个
- **WHEN** Agent `"gpt"` 发生错误（Error 事件），且 `pending_agents` 含 `["opus"]`
- **THEN** 显示 `"gpt"` 的错误信息，从队列弹出 `"opus"` 并启动其 completion

#### Scenario: 最后一个 Agent 失败
- **WHEN** 最后一个 Agent 发生错误（Error 事件），且 `pending_agents` 为空
- **THEN** 显示错误信息，`agent_event_rx` 设为 None，用户可以输入新消息

### Requirement: LastRespondent 追踪
App SHALL 维护 `last_respondent: Option<String>` 字段，记录最近一次成功回复的 Agent 名称。

#### Scenario: Done 更新 last_respondent
- **WHEN** Agent `"opus"` 完成回复（Done 事件）
- **THEN** `last_respondent` 更新为 `Some("opus")`

#### Scenario: Error 不更新 last_respondent
- **WHEN** Agent `"gpt"` 发生错误（Error 事件）
- **THEN** `last_respondent` 保持不变

#### Scenario: 无前缀输入使用 last_respondent
- **WHEN** 用户输入 `继续说说`（无 @ 前缀），且 `last_respondent` 为 `Some("opus")`
- **THEN** 消息发送给 `"opus"`

#### Scenario: 无 last_respondent 时提示用户
- **WHEN** 用户输入 `hello`（无 @ 前缀），且 `last_respondent` 为 None
- **THEN** 显示错误提示："请使用 @name 指定一个 Agent"，消息不发送

### Requirement: Token 用量累计
每个 Agent 完成回复后，SHALL 正确累计 token 用量到 `agent_token_usage` 映射。

#### Scenario: 多 Agent token 分别累计
- **WHEN** @all 执行完毕，`"gpt"` 使用 1000/500 tokens，`"opus"` 使用 2000/800 tokens
- **THEN** `agent_token_usage["gpt"]` 为 (1000, 500)，`agent_token_usage["opus"]` 为 (2000, 800)

### Requirement: 上下文共享
串行执行时，后续 Agent 的 `start_completion()` 调用 SHALL 传入包含前面所有 Agent 回复的完整 `messages` 列表。

#### Scenario: 第二个 Agent 可见第一个的回复
- **WHEN** @all 中 `"gpt"` 完成回复 "Hi from GPT"，下一个 `"opus"` 启动 completion
- **THEN** 传给 `"opus"` 的 messages 中包含 `ChatMessage { role: Assistant, content: "Hi from GPT", name: Some("gpt") }`
