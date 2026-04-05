## ADDED Requirements

### Requirement: 外部模块可复用 agent loop
`AgentLoopContext`、`run_agent_loop`、`create_tool_context`、`generate_tool_summary`、`ToolContextHandle` SHALL 保持 `pub(crate)` 可见性，允许 `krew-core` 内的其他模块（如 `task`）直接构造和调用。

#### Scenario: task 模块调用 agent loop
- **WHEN** `krew-core::task` 模块需要执行独立的 agent loop
- **THEN** SHALL 能直接构造 `AgentLoopContext` 并调用 `run_agent_loop()`，通过 `pub(crate)` 可见性在 crate 内部访问
