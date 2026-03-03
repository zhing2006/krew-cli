## ADDED Requirements

### Requirement: 工具调用循环
Agent Loop SHALL 在收到 `StreamEvent::ToolCall` 后进入工具调用循环：收集当前轮次所有 ToolCall → 执行工具 → 将工具结果追加到消息历史 → 再次调用 LLM → 重复，直到 LLM 不再返回 ToolCall 或达到最大轮数。

#### Scenario: 单轮工具调用
- **WHEN** LLM 返回一个 ToolCall 后跟 Done
- **THEN** Agent Loop SHALL 执行该工具，将结果回传 LLM，再次调用 `chat_stream()`，最终输出文本回复

#### Scenario: 多轮工具调用
- **WHEN** LLM 在第一轮返回 ToolCall，第二轮再次返回 ToolCall，第三轮返回纯文本
- **THEN** Agent Loop SHALL 执行 3 轮 `chat_stream()` 调用，最终输出文本回复

#### Scenario: 达到最大轮数
- **WHEN** 工具调用循环达到 `max_tool_rounds`（默认 25）
- **THEN** Agent Loop SHALL 停止循环，发送 `AgentEvent::Error` 告知已达最大轮数

### Requirement: 单轮多工具并行
当 LLM 在一次响应中返回多个 ToolCall 时，Agent Loop SHALL 并行执行所有工具调用（使用 `futures::future::join_all`）。

#### Scenario: 两个只读工具并行
- **WHEN** LLM 返回 `[ToolCall("read_file", ...), ToolCall("grep", ...)]`
- **THEN** 两个工具 SHALL 被并行执行，两个结果均追加到消息历史后再次调用 LLM

### Requirement: 工具调用消息构建
Agent Loop 在工具调用循环中 SHALL 构建正确的 ChatMessage 序列：
1. assistant 消息：包含文本（如有）和 `tool_calls` 字段
2. 每个工具结果：`ChatMessage { role: Tool, content: result, tool_call_id: id }`

#### Scenario: 工具调用消息格式
- **WHEN** LLM 返回一个 ToolCall
- **THEN** Agent Loop SHALL 先追加一条 `role: Assistant` 的消息（包含 `tool_calls`），再追加一条 `role: Tool` 的消息（包含 `tool_call_id` 和工具输出）

### Requirement: 工具执行错误处理
当工具执行返回 `ToolResult { is_error: true }` 时，Agent Loop SHALL 将错误信息作为 tool result 回传 LLM，而非终止 Agent 回合。LLM 可以根据错误信息决定下一步操作。

#### Scenario: 工具返回错误
- **WHEN** 工具执行返回 `ToolResult { content: "file not found", is_error: true }`
- **THEN** Agent Loop SHALL 将此结果作为 tool result 消息回传 LLM，继续循环

#### Scenario: 工具 dispatch 失败
- **WHEN** 工具名不在注册表中
- **THEN** Agent Loop SHALL 返回错误 tool result（`is_error: true`）回传 LLM

### Requirement: 工具调用 Usage 累积
工具循环中每轮 LLM 调用的 Usage SHALL 累加。最终 `AgentEvent::Done` 携带的 Usage SHALL 是所有轮次的总和。

#### Scenario: 多轮 Usage 累加
- **WHEN** 工具循环执行 3 轮 LLM 调用，每轮返回 `Usage { prompt_tokens: 100, completion_tokens: 50, total_tokens: 150 }`
- **THEN** 最终 `Done(Usage)` SHALL 包含 `{ prompt_tokens: 300, completion_tokens: 150, total_tokens: 450 }`

### Requirement: 工具注册由 Agent 配置控制
`AgentRuntime` SHALL 根据 `AgentConfig.tools` 字段决定是否注册工具。`tools = true`（默认）注册所有内置工具，`tools = false` 不注册任何工具。

#### Scenario: tools = true
- **WHEN** agent 配置 `tools = true`
- **THEN** `start_completion()` SHALL 将工具 specs 传给 `chat_stream()` 的 `tools` 参数

#### Scenario: tools = false
- **WHEN** agent 配置 `tools = false`
- **THEN** `start_completion()` SHALL 传空 tools 列表给 `chat_stream()`

### Requirement: max_tool_rounds 可配置
`Settings` SHALL 新增 `max_tool_rounds: Option<u32>` 字段，默认 25。Agent Loop 使用此值作为工具循环的最大轮数。

#### Scenario: 自定义最大轮数
- **WHEN** settings.toml 配置 `max_tool_rounds = 10`
- **THEN** Agent Loop 的工具循环 SHALL 在第 10 轮后停止
