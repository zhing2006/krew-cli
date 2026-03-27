## ADDED Requirements

### Requirement: run_agent tool 注册
当发现了至少一个 Sub-Agent 定义时，系统 SHALL 在所有启用了 tools 的 Peer Agent 的 `ToolRegistry` 中注册 `run_agent` tool。Sub-Agent 自身的 `ToolRegistry` 中 SHALL NOT 注册 `run_agent`（禁止嵌套）。

`run_agent` tool 的 `requires_approval()` SHALL 返回 `false`。

#### Scenario: 有 Sub-Agent 定义时注册 tool
- **WHEN** 发现了至少一个 Sub-Agent 定义
- **THEN** 每个启用 tools 的 Peer Agent 的 `ToolRegistry` SHALL 包含 `run_agent` tool

#### Scenario: 没有 Sub-Agent 定义时不注册
- **WHEN** 没有发现任何 Sub-Agent 定义
- **THEN** 不 SHALL 注册 `run_agent` tool

#### Scenario: Sub-Agent 不能嵌套调用
- **WHEN** Sub-Agent 运行时尝试调用 `run_agent`
- **THEN** 该 tool SHALL 不存在于 Sub-Agent 的可用 tool 列表中

### Requirement: run_agent tool schema
`run_agent` tool SHALL 接受以下参数：
- `agent`（字符串，必需）：Sub-Agent 名称，SHALL 为发现的 Sub-Agent name 之一
- `task`（字符串，必需）：交给 Sub-Agent 的任务描述

tool description SHALL 说明：这是一个同步阻塞调用，Sub-Agent 在隔离上下文中执行任务后返回结果。适合用于需要专注执行且不希望污染主对话上下文的专项任务。

`agent` 参数的 `enum` 值 SHALL 动态填充为所有已发现的 Sub-Agent 名称。

#### Scenario: tool schema 包含可用 agent 列表
- **WHEN** 发现了 `git` 和 `researcher` 两个 Sub-Agent
- **THEN** `run_agent` tool 的 `agent` 参数 enum SHALL 为 `["git", "researcher"]`

### Requirement: run_agent 同步执行
当 `run_agent` tool 被调用时，系统 SHALL：

1. 根据 `agent` 参数查找对应的 `SubAgentDef`
2. 使用父 Agent 的 model、provider、API key、tools（不含 `run_agent`）、MCP tools、sampling config、approval mode、approval cache 构建 Sub-Agent 的 `AgentRuntime`
3. 使用 `SubAgentDef` 的 `system_prompt` 替换 system prompt
4. 构建独立的 messages 列表：`[system_prompt, user(task)]`
5. 调用 `start_completion()` 启动 agent loop
6. 阻塞消费 `AgentEvent` 流直到收到 `Done` 或 `Error`
7. 返回 `final_text` 作为 tool result

整个过程 SHALL 阻塞父 Agent 的 agent loop（tool execute 不返回直到 Sub-Agent 完成）。

#### Scenario: 成功执行 Sub-Agent 任务
- **WHEN** 父 Agent 调用 `run_agent("git", "提交当前修改")`
- **THEN** 系统 SHALL 创建隔离上下文，执行 git agent 的 agent loop，返回最终文本作为 tool result

#### Scenario: Sub-Agent 名称不存在
- **WHEN** 父 Agent 调用 `run_agent("nonexistent", "some task")`
- **THEN** 系统 SHALL 返回 `is_error: true` 的 tool result，内容为 `"Unknown sub-agent: nonexistent"`

#### Scenario: Sub-Agent agent loop 报错
- **WHEN** Sub-Agent 的 agent loop 产生 `AgentEvent::Error`
- **THEN** 系统 SHALL 返回 `is_error: true` 的 tool result，包含错误信息

### Requirement: run_agent 流式展示 Sub-Agent 的 tool 调用过程
在 Sub-Agent 执行期间，系统 SHALL 将 Sub-Agent 的关键 `AgentEvent` 通过父 Agent 的 event channel 转发给 TUI：

- `ToolCallStart { name, arguments }` → 发送为 `ToolCallOutput`，格式: `"🔧 {name}({arguments_summary})"`
- `ToolCallOutput { text }` → 原样转发为 `ToolCallOutput`，带缩进前缀
- `ToolCallDone { name, result_summary }` → 发送为 `ToolCallOutput`，格式: `"  ✓ {result_summary}"`
- `TextDelta(text)` → 累积但不实时转发（只在最终 result 中返回）
- `ThinkingDelta` → 忽略
- `ServerToolStart/Done` → 转发为 `ToolCallOutput`

#### Scenario: 用户看到 Sub-Agent 的 tool 调用过程
- **WHEN** Sub-Agent 执行了 `shell("git status")` 和 `shell("git commit ...")`
- **THEN** TUI SHALL 在 `run_agent` tool 的输出区域内依次显示这些 tool 调用的名称和结果

### Requirement: run_agent Approval 转发
Sub-Agent 执行需要用户审批的 tool 时，`ApprovalRequest` 事件 SHALL 通过父 Agent 的 event channel 转发给 TUI，由用户正常审批。Sub-Agent SHALL 继承父 Agent 的 `ApprovalMode` 和共享同一个 `ApprovalCache`。

#### Scenario: Sub-Agent 的 tool 需要审批
- **WHEN** Sub-Agent 在 suggest 模式下调用 `shell("rm -rf temp/")`
- **THEN** TUI SHALL 弹出审批提示，用户决策后 Sub-Agent 继续执行

#### Scenario: Sub-Agent 共享 ApprovalCache
- **WHEN** 用户在父 Agent 中 ApprovedForSession 了 `shell:git` 前缀
- **THEN** Sub-Agent 调用 `shell("git status")` 时 SHALL 自动通过，无需再次审批

### Requirement: /agents 命令展示 Sub-Agent
`/agents` slash 命令 SHALL 在展示 Peer Agent 列表之后，额外展示已发现的 Sub-Agent 定义列表，包括名称、描述和来源路径。

#### Scenario: 展示 Sub-Agent 列表
- **WHEN** 用户执行 `/agents` 命令，且发现了 `git` 和 `researcher` 两个 Sub-Agent
- **THEN** 输出 SHALL 在 Peer Agent 列表后显示 Sub-Agent 部分，列出每个 Sub-Agent 的名称和描述
