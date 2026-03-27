## ADDED Requirements

### Requirement: run_agent tool 注册
当发现了至少一个 Sub-Agent 定义时，系统 SHALL 在所有启用了 tools 的 Peer Agent 的 `ToolRegistry` 中注册 `run_agent` tool。该注册逻辑 SHALL 同时覆盖 TUI 模式和 prompt 模式（`-p`）两条启动路径。

`run_agent` tool 的 `requires_approval()` SHALL 返回 `false`。

#### Scenario: 有 Sub-Agent 定义时注册 tool（TUI 模式）
- **WHEN** TUI 模式下发现了至少一个 Sub-Agent 定义
- **THEN** 每个启用 tools 的 Peer Agent 的 `ToolRegistry` SHALL 包含 `run_agent` tool

#### Scenario: 有 Sub-Agent 定义时注册 tool（prompt 模式）
- **WHEN** prompt 模式（`-p`）下发现了至少一个 Sub-Agent 定义
- **THEN** 每个启用 tools 的 Peer Agent 的 `ToolRegistry` SHALL 包含 `run_agent` tool

#### Scenario: 没有 Sub-Agent 定义时不注册
- **WHEN** 没有发现任何 Sub-Agent 定义
- **THEN** 不 SHALL 注册 `run_agent` tool

### Requirement: Sub-Agent 禁止嵌套调用——双重保障
系统 SHALL 通过两层机制防止 Sub-Agent 嵌套调用 `run_agent`：

1. **tool_defs 过滤**: Sub-Agent 调用 `start_completion` 时，传递给 LLM 的 tool 定义列表 SHALL 排除 `run_agent`（LLM 看不到该 tool）
2. **execute() depth guard**: `RunAgentTool` SHALL 持有一个共享的运行标志（`Arc<AtomicBool>`）。execute() 入口处 SHALL 使用 CAS 操作将标志设为 true；如果标志已经为 true，SHALL 立即返回错误 `"Sub-agent nesting is not allowed"`；execute() 退出时 SHALL 将标志设回 false

#### Scenario: Sub-Agent 的 LLM 看不到 run_agent tool
- **WHEN** Sub-Agent 的 agent_loop 向 LLM 发送可用 tool 列表
- **THEN** 该列表 SHALL NOT 包含 `run_agent`

#### Scenario: 即使 dispatch 层存在 handler 也拒绝执行
- **WHEN** 在 Sub-Agent 执行期间，`run_agent` 的 execute() 被调用（如 prompt injection）
- **THEN** execute() SHALL 检测到 depth guard 标志为 true，返回 `is_error: true` 的 tool result

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
2. 直接使用父 Agent 的运行时资源：`Arc<dyn LlmClient>`（同 client）、`Arc<ToolRegistry>`（同 tools，含 MCP）、`ApprovalCache`（同 cache）、`SamplingConfig`（同 sampling）、`ApprovalMode`（同 approval mode）
3. 使用 `SubAgentDef` 的 `system_prompt` 替换 system prompt
4. 构建独立的 messages 列表：`[system_prompt, user(task)]`
5. 调用 `start_completion()`（通过 `exclude_tools` 排除 `run_agent`）启动 agent loop
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
在 Sub-Agent 执行期间，系统 SHALL 将 Sub-Agent 的 **tool 相关** `AgentEvent` 通过 `ctx.output_tx` 转发给 TUI：

- `ToolCallStart { name, arguments }` → 发送为 `ToolCallOutput`，格式: `"🔧 {name}({arguments_summary})"`
- `ToolCallOutput { text }` → 原样转发为 `ToolCallOutput`
- `ToolCallDone { name, result_summary }` → 发送为 `ToolCallOutput`，格式: `"  ✓ {result_summary}"`
- `ServerToolStart/Done` → 转发为 `ToolCallOutput`
- `TextDelta(text)` → SHALL NOT 转发，仅累积后作为最终 tool result 返回（因为 `ToolCallOutput` 是行级文本，`TextDelta` 是 token 级碎片，混用会导致 TUI 渲染错乱）
- `ThinkingDelta` → 忽略

#### Scenario: 用户看到 Sub-Agent 的 tool 调用过程
- **WHEN** Sub-Agent 执行了 `shell("git status")` 和 `shell("git commit ...")`
- **THEN** TUI SHALL 在 `run_agent` tool 的输出区域内依次显示这些 tool 调用的名称和结果

#### Scenario: Sub-Agent 的 TextDelta 不实时展示
- **WHEN** Sub-Agent 产生 `TextDelta` 事件
- **THEN** 系统 SHALL NOT 将其转发为 `ToolCallOutput`，而是累积文本并在 Sub-Agent 完成后作为 `run_agent` 的 tool result 返回

### Requirement: run_agent Approval 转发
`krew-tools::ToolContext` SHALL 新增 `parent_event_tx: Option<Box<dyn Any + Send>>` 字段（默认 `None`）。`krew-core` 的 `create_tool_context()` 在处理 `run_agent` tool 时 SHALL 将父 Agent 当前 turn 的 `UnboundedSender<AgentEvent>` clone 装箱后设入此字段。

`RunAgentTool::execute()` SHALL 通过 `downcast_ref::<UnboundedSender<AgentEvent>>()` 取回 sender。Sub-Agent 的 `start_completion()` SHALL 创建独立的 `(tx, rx)` 对。`RunAgentTool::execute()` 消费 Sub-Agent 的 `rx` 时，遇到 `ApprovalRequest` 事件 SHALL 通过取回的 parent sender 转发给 TUI。

Sub-Agent SHALL 使用与父 Agent 相同的 `ApprovalMode` 和共享同一个 `ApprovalCache`（Arc 共享）。

事件转发路径：
- `ToolCallStart/Output/Done` → 通过 `ctx.output_tx` 转发（ToolCallOutput 管线）
- `ApprovalRequest` → 通过 `ctx.parent_event_tx` downcast 后转发（父 Agent event channel）
- `TextDelta` → 累积到 final_text（不转发）
- `Done/Error` → 作为 tool result 返回

#### Scenario: Sub-Agent 的 tool 需要审批
- **WHEN** Sub-Agent 在 suggest 模式下调用 `shell("rm -rf temp/")`
- **THEN** `RunAgentTool` SHALL 通过 `ctx.parent_event_tx` downcast 取得父 Agent sender，将 `ApprovalRequest`（含 `oneshot::Sender`）转发给 TUI，TUI 弹出审批提示，用户决策通过 oneshot 回到 Sub-Agent 的 agent_loop

#### Scenario: Sub-Agent 共享 ApprovalCache
- **WHEN** 用户在父 Agent 中 ApprovedForSession 了 `shell:git` 前缀
- **THEN** Sub-Agent 调用 `shell("git status")` 时 SHALL 自动通过，无需再次审批

### Requirement: /agents 命令展示 Sub-Agent
`/agents` slash 命令 SHALL 在展示 Peer Agent 列表之后，额外展示已发现的 Sub-Agent 定义列表，包括名称、描述和来源路径。

#### Scenario: 展示 Sub-Agent 列表
- **WHEN** 用户执行 `/agents` 命令，且发现了 `git` 和 `researcher` 两个 Sub-Agent
- **THEN** 输出 SHALL 在 Peer Agent 列表后显示 Sub-Agent 部分，列出每个 Sub-Agent 的名称和描述
