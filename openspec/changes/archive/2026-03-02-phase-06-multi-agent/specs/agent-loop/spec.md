## MODIFIED Requirements

### Requirement: 单 Agent 对话完成
`AgentRuntime` SHALL 提供 `start_completion()` 方法，接收消息历史，启动异步 task 调用 LLM，通过 mpsc channel 发送 `AgentEvent`。AgentRuntime SHALL 持有 `other_agent_role: OtherAgentRole`（从 ProviderConfig 获取）用于消息格式转换。

#### Scenario: 基本对话流程
- **WHEN** 调用 `agent.start_completion(messages)` 并传入消息历史
- **THEN** 返回 `mpsc::UnboundedReceiver<AgentEvent>`，异步发送 `ResponseStart` → 多个 `TextDelta` → `Done(Usage)`

#### Scenario: LLM 错误传播
- **WHEN** `chat_stream()` 返回 `LlmError`
- **THEN** 发送 `AgentEvent::Error(error_message)` 然后关闭 channel

#### Scenario: 流式错误传播
- **WHEN** 流处理收到 `StreamEvent::Error(msg)`
- **THEN** 发送 `AgentEvent::Error(msg)` 然后关闭 channel

### Requirement: System Prompt 构建
agent loop 在调用 LLM 前 SHALL 在消息列表头部插入 system message。system prompt 中关于 other-agent 消息的说明 SHALL 固定为："Their messages are prefixed with [agent_name] in the content."

#### Scenario: 有 project instructions
- **WHEN** Agent 有 system_prompt 且 project instructions 存在
- **THEN** 第一条消息 SHALL 为 `ChatMessage { role: System, content: merged_prompt }`，merged_prompt 包含 agent 身份信息和 other-agent hint

#### Scenario: 无 system prompt
- **WHEN** Agent 无 system_prompt 且无 project instructions
- **THEN** 消息列表中 SHALL 不插入 system message

## REMOVED Requirements

### Requirement: use_name_field 字段
**Reason**: 统一使用 `[agent_name]` content 前缀方式。
**Migration**: 从 `AgentRuntime` 中删除 `use_name_field: bool` 字段。system prompt 中的 other-agent hint 固定为前缀方式描述，不再根据 `use_name_field` 条件切换。
