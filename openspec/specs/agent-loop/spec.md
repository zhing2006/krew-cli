## ADDED Requirements

### Requirement: AgentEvent 通信协议
`krew-core` SHALL 定义 `AgentEvent` 枚举作为 agent loop 与 TUI 层之间的通信协议。

#### Scenario: 事件类型完整性
- **WHEN** agent loop 运行
- **THEN** SHALL 通过 `AgentEvent` 发送以下事件类型：`ResponseStart`（含 agent_name、display_name、color）、`TextDelta(String)`、`Done(Usage)`、`Error(String)`

### Requirement: 单 Agent 对话完成
`AgentRuntime` SHALL 提供 `start_completion()` 方法，接收消息历史，启动异步 task 调用 LLM，通过 mpsc channel 发送 `AgentEvent`。AgentRuntime SHALL 持有 `other_agent_role: OtherAgentRole`（从 ProviderConfig 获取）用于消息格式转换。

#### Scenario: 基本对话流程
- **WHEN** 调用 `agent.start_completion(messages)` 并传入消息历史
- **THEN** 返回 `mpsc::UnboundedReceiver<AgentEvent>`，异步发送 `ResponseStart` → 多个 `TextDelta` → `Done(Usage)`

#### Scenario: LLM 错误传播
- **WHEN** `chat_stream()` 返回 `LlmError`
- **THEN** SHALL 发送 `AgentEvent::Error(error_message)` 后关闭 channel

#### Scenario: 流式错误传播
- **WHEN** 流式过程中收到 `StreamEvent::Error(msg)`
- **THEN** SHALL 发送 `AgentEvent::Error(msg)` 后关闭 channel

### Requirement: System Prompt 构建
agent loop 在调用 LLM 前 SHALL 在消息列表头部插入 system message。system prompt 中关于 other-agent 消息的说明 SHALL 固定为："Their messages are prefixed with [agent_name] in the content."

#### Scenario: 有 project instructions
- **WHEN** Agent 有 system_prompt 且 project instructions 存在
- **THEN** 第一条消息 SHALL 为 `ChatMessage { role: System, content: merged_prompt }`，merged_prompt 包含 agent 身份信息和 other-agent hint

#### Scenario: 无 system prompt
- **WHEN** Agent 无 system_prompt 且无 project instructions
- **THEN** 消息列表中 SHALL 不插入 system message

### Requirement: 跳过工具调用
Phase 4 的 agent loop SHALL 忽略 `StreamEvent::ToolCall` 事件，不执行工具调用，不进入多轮循环。

#### Scenario: ToolCall 被忽略
- **WHEN** LLM 流式响应中包含 `StreamEvent::ToolCall`
- **THEN** agent loop SHALL 忽略该事件，不中断流式处理，不发送对应的 `AgentEvent`

### Requirement: Agent 初始化
App 启动时 SHALL 根据 `Config` 中的 agents 和 providers 配置构建 `AgentRuntime` 实例。

#### Scenario: OpenAI Agent 初始化
- **WHEN** agent 配置的 provider 对应的 ProviderConfig 存在
- **THEN** SHALL 根据 provider 类型创建 `OpenAiChatClient`，构建 `AgentRuntime`

#### Scenario: 未知 Provider 类型
- **WHEN** agent 配置的 provider 类型不是 "openai"
- **THEN** SHALL 记录警告日志，该 agent 不参与对话（或保持 echo 行为）

#### Scenario: builtin echo Agent
- **WHEN** agent 配置的 provider 为 "builtin"
- **THEN** SHALL 保持 echo 行为，不创建 LlmClient
