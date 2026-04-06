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

## ADDED Requirements

### Requirement: 密语状态传播
App SHALL 维护 `current_whisper_targets: Option<Vec<String>>` 字段，在用户发送密语消息时设置。dispatch queue 中的每个 agent 执行时，其产出的所有消息 SHALL 继承 `current_whisper_targets`。

#### Scenario: 用户发送密语设置状态
- **WHEN** 用户发送 `#opus #gemini hello`
- **THEN** `current_whisper_targets` SHALL 设为 `Some(["opus", "gemini"])`

#### Scenario: 普通消息不设置密语状态
- **WHEN** 用户发送 `@opus hello`（普通消息）
- **THEN** `current_whisper_targets` SHALL 为 `None`

#### Scenario: LastRespondent 不继承密语
- **WHEN** 上一次回复是密语模式下的 opus
- **AND** 用户发送无前缀消息 `continue`
- **THEN** `current_whisper_targets` SHALL 为 `None`，消息以普通模式发送

### Requirement: 密语状态清除——正常完成
密语 dispatch queue 处理完毕且无后续密语 A2A 触发时，`current_whisper_targets` SHALL 被清除为 `None`。

#### Scenario: 密语 dispatch 正常完成后清除状态
- **WHEN** 密语 dispatch queue 中最后一个 agent 完成回复（Done 事件）
- **AND** 没有后续的密语 A2A 触发
- **THEN** `current_whisper_targets` SHALL 被清除为 `None`

### Requirement: 密语状态清除——错误路径
当 agent 在密语模式下发生错误（Error 事件）时，合成的部分文本消息 SHALL 继承 `current_whisper_targets`。如果 pending 队列为空，`current_whisper_targets` SHALL 被清除。

#### Scenario: 密语模式下 agent 错误，有部分文本
- **WHEN** agent "opus" 在密语模式（whisper_targets = ["opus", "gemini"]）下发生错误
- **AND** 已产出部分文本
- **THEN** 合成的 `[Error: ...]` assistant 消息 SHALL 继承 `whisper_targets = Some(["opus", "gemini"])`

#### Scenario: 密语模式下 agent 错误，中间消息继承
- **WHEN** agent 在密语模式下发生错误
- **AND** 有 intermediate_messages
- **THEN** 这些中间消息 SHALL 已带有 `whisper_targets`（由 agent loop 标记）

#### Scenario: 错误后 pending 队列为空时清除状态
- **WHEN** 密语模式下 agent 发生错误
- **AND** pending_agents 队列为空
- **THEN** `current_whisper_targets` SHALL 被清除为 `None`

#### Scenario: 错误后 pending 队列不为空时保留状态
- **WHEN** 密语模式下 agent 发生错误
- **AND** pending_agents 队列仍有其他密语组内 agent
- **THEN** `current_whisper_targets` SHALL 保持不变，继续调度下一个 agent

### Requirement: 密语状态清除——取消路径
当用户按 ESC 取消密语模式下的 agent 回复时，合成的取消消息 SHALL 继承 `current_whisper_targets`，且 `current_whisper_targets` SHALL 在清空 pending 队列后被清除。

#### Scenario: 密语模式下 ESC 取消，有部分文本
- **WHEN** 用户在密语模式下按 ESC 取消 agent 回复
- **AND** 已产出部分文本
- **THEN** 合成的 `[Cancelled by user]` assistant 消息 SHALL 继承当前的 `whisper_targets`

#### Scenario: 密语模式下 ESC 取消后清除状态
- **WHEN** 用户在密语模式下按 ESC 取消
- **THEN** `pending_agents` 被清空后，`current_whisper_targets` SHALL 被清除为 `None`

### Requirement: 用户密语消息 whisper_targets 标记
当 `is_whisper = true` 时，用户的 `ChatMessage` SHALL 设置 `whisper_targets` 为解析出的目标 Agent 列表。

#### Scenario: 密语用户消息标记
- **WHEN** 用户输入 `#opus hello`，`is_whisper = true`
- **THEN** 用户 ChatMessage 的 `whisper_targets` SHALL 为 `Some(["opus"])`
