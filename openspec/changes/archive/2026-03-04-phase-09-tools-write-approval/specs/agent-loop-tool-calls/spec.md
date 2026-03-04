## MODIFIED Requirements

### Requirement: 工具调用循环
Agent Loop SHALL 在收到 `StreamEvent::ToolCall` 后进入工具调用循环：收集当前轮次所有 ToolCall → **检查审批策略** → 执行工具 → 将工具结果追加到消息历史 → 再次调用 LLM → 重复，直到 LLM 不再返回 ToolCall 或达到最大轮数。

#### Scenario: 单轮工具调用
- **WHEN** LLM 返回一个 ToolCall 后跟 Done
- **THEN** Agent Loop SHALL 执行该工具，将结果回传 LLM，再次调用 `chat_stream()`，最终输出文本回复

#### Scenario: 多轮工具调用
- **WHEN** LLM 在第一轮返回 ToolCall，第二轮再次返回 ToolCall，第三轮返回纯文本
- **THEN** Agent Loop SHALL 执行 3 轮 `chat_stream()` 调用，最终输出文本回复

#### Scenario: 达到最大轮数
- **WHEN** 工具调用循环达到 `max_tool_rounds`（默认 25）
- **THEN** Agent Loop SHALL 停止循环，发送 `AgentEvent::Error` 告知已达最大轮数

#### Scenario: 写工具需审批 (Suggest 模式)
- **WHEN** LLM 返回 ToolCall("edit_file", ...) 且 ApprovalMode 为 Suggest
- **THEN** Agent Loop SHALL 发送 AgentEvent::ApprovalRequest 并 await oneshot receiver，收到 Approved 后执行，收到 Denied 后返回错误 ToolResult

#### Scenario: 只读工具始终自动执行
- **WHEN** LLM 返回 ToolCall("read_file", ...) 且任意 ApprovalMode
- **THEN** Agent Loop SHALL 直接执行，不发送 ApprovalRequest

### Requirement: 单轮多工具并行
当 LLM 在一次响应中返回多个 ToolCall 时，Agent Loop SHALL 按审批需求分组执行：不需审批的工具并行执行，需审批的工具依次发送 ApprovalRequest 后执行。

#### Scenario: 混合审批工具
- **WHEN** LLM 返回 `[ToolCall("read_file", ...), ToolCall("shell", ...)]` 且 Suggest 模式
- **THEN** read_file SHALL 直接执行，shell SHALL 发送 ApprovalRequest 等待用户决定后执行

#### Scenario: 两个只读工具并行
- **WHEN** LLM 返回 `[ToolCall("read_file", ...), ToolCall("grep", ...)]`
- **THEN** 两个工具 SHALL 被并行执行，两个结果均追加到消息历史后再次调用 LLM
