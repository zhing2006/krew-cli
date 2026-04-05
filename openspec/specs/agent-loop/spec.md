## REMOVED Requirements

### Requirement: 跳过工具调用
**Reason**: Phase 8 实现完整的工具调用循环，不再跳过 ToolCall 事件。
**Migration**: 由 `agent-loop-tool-calls` capability 中的"工具调用循环"需求替代。

## MODIFIED Requirements

### Requirement: AgentEvent 通信协议
`krew-core` SHALL 定义 `AgentEvent` 枚举作为 agent loop 与 TUI 层之间的通信协议。

#### Scenario: 事件类型完整性
- **WHEN** agent loop 运行
- **THEN** SHALL 通过 `AgentEvent` 发送以下事件类型：`ResponseStart`（含 agent_name、display_name、color）、`TextDelta(String)`、`ThinkingDelta(String)`、`Done(Usage)`、`Error(String)`、`Retrying`、`ToolCallStart`（含 name、arguments）、`ToolCallDone`（含 name、result_summary）

### Requirement: 单 Agent 对话完成
`AgentRuntime` SHALL 提供 `start_completion()` 方法，接收消息历史，启动异步 task 调用 LLM，通过 mpsc channel 发送 `AgentEvent`。当 `config.tools = true` 时，SHALL 将注册的 ToolSpec 列表传给 `chat_stream()` 并在收到 ToolCall 时进入工具调用循环。

`build_identity_prompt()` 构建的 identity 块 SHALL 包含以下信息（按顺序）：
1. Agent 身份（display_name、model、agent_name）
2. krew-cli 简介：说明 krew-cli 是一个多 AI agent 协作 CLI 工具，用户在一个终端中同时与多个 LLM 对话
3. 配置帮助提示：告知 agent 当需要帮助用户修改 krew 配置时，可执行 `krew config help` 获取完整配置手册
4. 多 agent 对话规则（其他 agent 消息前缀、不要模仿其他 agent）
5. 当前日期时间
6. 语言指令（如有）
7. Peer agent 协作提示（如有）
8. Whisper 隐私上下文（如有）

`build_system_prompt()` 构建的完整 system prompt SHALL 按以下顺序拼接：
1. Project Instructions（来自 AGENTS.md 或 config）
2. Skill Catalog（可用的 skill 列表）
3. Sub-Agent Catalog（可用的 sub-agent 列表）
4. **Memory Prompt**（Memory 指令 + Global MEMORY.md 内容 + Per-Agent MEMORY.md 内容；当 `config.tools = false` 时仅注入索引内容）
5. Agent Prompt（agent 自定义 system_prompt）

Memory Prompt 在每次 `start_completion()` 调用时随 `build_system_prompt()` 重新加载。

#### Scenario: 基本对话流程（无工具调用）
- **WHEN** 调用 `agent.start_completion(messages)` 且 LLM 不返回 ToolCall
- **THEN** 返回 `mpsc::UnboundedReceiver<AgentEvent>`，异步发送 `ResponseStart` → 多个 `TextDelta` → `Done(Usage)`

#### Scenario: 带工具调用的对话流程
- **WHEN** 调用 `agent.start_completion(messages)` 且 LLM 返回 ToolCall
- **THEN** SHALL 发送 `ResponseStart` → `ToolCallStart` → `ToolCallDone` → 多个 `TextDelta` → `Done(Usage)`

#### Scenario: LLM 错误传播
- **WHEN** `chat_stream()` 返回 `LlmError`
- **THEN** SHALL 发送 `AgentEvent::Error(error_message)` 后关闭 channel

#### Scenario: 流式错误传播
- **WHEN** 流式过程中收到 `StreamEvent::Error(msg)`
- **THEN** SHALL 发送 `AgentEvent::Error(msg)` 后关闭 channel

#### Scenario: identity prompt 包含 krew 简介
- **WHEN** 构建 agent 的 identity prompt
- **THEN** identity 块 SHALL 包含 krew-cli 的简要描述，说明这是一个多 AI agent 协作 CLI 工具

#### Scenario: identity prompt 包含配置帮助提示
- **WHEN** 构建 agent 的 identity prompt
- **THEN** identity 块 SHALL 包含提示文本，告知 agent 可执行 `krew config help` 获取配置手册

#### Scenario: system prompt 包含 Memory 段（tools=true）
- **WHEN** 构建 `config.tools = true` 的 agent 的 system prompt 且 `.krew/memory/` 目录存在
- **THEN** system prompt SHALL 在 Sub-Agent Catalog 之后、Agent Prompt 之前包含完整 Memory Prompt（写入指令 + 索引内容）

#### Scenario: system prompt 包含 Memory 段（tools=false）
- **WHEN** 构建 `config.tools = false` 的 agent 的 system prompt 且 `.krew/memory/` 目录存在
- **THEN** system prompt SHALL 在 Sub-Agent Catalog 之后、Agent Prompt 之前仅包含 Memory 索引内容（不含写入指令）

### Requirement: 外部模块可复用 agent loop
`AgentLoopContext`、`run_agent_loop`、`create_tool_context`、`generate_tool_summary`、`ToolContextHandle` SHALL 保持 `pub(crate)` 可见性，允许 `krew-core` 内的其他模块（如 `task`）直接构造和调用。

#### Scenario: task 模块调用 agent loop
- **WHEN** `krew-core::task` 模块需要执行独立的 agent loop
- **THEN** SHALL 能直接构造 `AgentLoopContext` 并调用 `run_agent_loop()`，通过 `pub(crate)` 可见性在 crate 内部访问
