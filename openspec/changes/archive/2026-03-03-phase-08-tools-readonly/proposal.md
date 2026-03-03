## Why

Agent 目前只能纯文本对话，无法读取项目文件。要实现 AI 编程助手的核心价值——理解代码、分析问题——Agent 必须能够访问文件系统。Phase 8 引入只读工具（read_file、glob、grep）和 Agent Loop 工具调用循环，使 Agent 具备自主读取项目文件的能力。

## What Changes

- **实现 3 个只读内置工具**：`read_file`（读文件内容，支持行号范围）、`glob`（文件名模式匹配）、`grep`（文件内容正则搜索）
- **重构工具架构**：将 ToolSpec（JSON Schema，给 LLM 看）与 ToolHandler（执行器）分离，建立工具注册表
- **扩展 Agent Loop**：处理 `StreamEvent::ToolCall`，执行工具，将结果回传 LLM，支持多轮工具调用循环（最多 25 轮）
- **单轮多工具并行**：LLM 一次返回多个 ToolCall 时，只读工具并行执行
- **路径边界安全**：所有文件工具在执行前校验路径必须在 `session.cwd` 内
- **工具调用 TUI 渲染**：在终端显示 `⚡ read_file("src/main.rs") — 42 lines` 格式
- **扩展消息格式**：ChatMessage 的 tool_calls / tool_results 字段投入使用，各 Provider 的 `convert_messages()` 支持工具结果消息转换
- **扩展会话持久化**：MessageEntry 增加 tool_calls / tool_call_id 字段，工具调用完整持久化

## Capabilities

### New Capabilities
- `builtin-tools-readonly`: 只读内置工具（read_file、glob、grep）的具体实现，包括路径边界校验
- `tool-registry`: 工具注册表，管理 ToolSpec 与 ToolHandler 的配对注册和分发
- `agent-loop-tool-calls`: Agent Loop 工具调用循环，包括多轮调用、并行执行、结果回传
- `tool-rendering`: 工具调用事件在 TUI 中的渲染显示

### Modified Capabilities
- `agent-loop`: 从"跳过工具调用"改为"执行工具调用循环"
- `message-types`: ChatMessage 的 tool_calls / tool_results 字段投入使用
- `tool-trait`: ToolSpec 与 ToolHandler 分离，重构现有 Tool trait
- `streaming-pipeline`: 支持工具调用事件的渲染

## Impact

- **krew-tools**: 主要变更 crate，实现工具注册表和 3 个只读工具
- **krew-core**: Agent Loop 重构，AgentEvent 新增工具事件变体，消息格式转换扩展
- **krew-cli**: TUI 渲染层支持工具调用显示
- **krew-llm**: `convert_messages()` 支持 tool result 消息格式转换（各 Provider 不同格式）
- **krew-storage**: MessageEntry 扩展工具调用字段
- **新依赖**: `grep-searcher`、`grep-regex`（ripgrep 底层引擎）、`walkdir`（目录遍历）、`globset`（已在 TDD 中规划）
