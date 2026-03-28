## Why

krew-cli 的 Agent 共享同一份对话历史（`Vec<ChatMessage>`），当某个 Agent 执行专项任务（如 git 提交、代码调研）时，产生的大量 tool call 消息会污染主对话上下文，导致 token 浪费和上下文噪声。需要一种**上下文隔离**的子代理机制，让 Agent 能将专项任务委派给专注的 Sub-Agent，在独立上下文中执行后只将最终结果返回主对话。

## What Changes

- 新增 **Sub-Agent 定义发现和解析** — 从 `.krew/agents/`、`.agents/agents/`、`.claude/agents/` 目录发现 Markdown 格式的 Agent 定义文件（兼容 Claude Code 的 `.claude/agents/*.md` 格式）
- 新增 **`run_agent` built-in tool** — 在 `krew-core` 中实现 `RunAgentTool`（impl `krew_tools::ToolHandler`），注册到 `krew-tools` 的 `ToolRegistry` 中。父 Agent 通过 tool call 同步调用 Sub-Agent，Sub-Agent 在完全隔离的上下文中执行任务（独立的 `Vec<ChatMessage>` + 专用 system prompt），执行期间的 tool 调用过程通过 `ToolCallOutput` 实时流式展示给用户，完成后将最终结果作为 tool result 返回父 Agent
- Sub-Agent **直接使用父 Agent 的运行时资源** — client、tools（含 MCP）、sampling config、approval 设置等全部共享
- 新增 **`sub_agent_enabled` 配置开关** — 实验性功能，默认关闭（`false`），关闭时完全不读取 agent 定义文件、不注册 tool、零开销
- 新增 **`/agents` 命令增强** — 展示已发现的 Sub-Agent 定义列表
- **版本升级至 v0.8.0** — 包括所有 6 个 Cargo crate、6 个 npm package、以及双语 README/MANUAL 和 PDD/TDD 文档更新

## Capabilities

### New Capabilities
- `sub-agent-discovery`: Sub-Agent 定义文件的发现、解析和验证（复用 `discovery.rs` 的多目录扫描机制）
- `run-agent-tool`: `run_agent` tool 的实现（在 `krew-core` 中）——构建隔离上下文、启动 agent loop、流式转发 tool events、收集返回结果
- `version-bump`: 版本号升级至 v0.8.0，更新所有 crate/npm/文档

### Modified Capabilities
_(无现有 spec 需要修改)_

## Impact

- **krew-core**: 新增 `sub_agent` 模块（discovery + parse + `RunAgentTool` 实现），修改 `agent/init.rs` 初始化流程，修改 `start_completion` 支持 tool 过滤
- **krew-tools**: `ToolRegistry` 接收 `RunAgentTool` 注册（无代码变更，只是运行时注册）
- **krew-cli**: 修改 TUI 模式和 prompt 模式的初始化链注册 Sub-Agent，修改 `/agents` 命令展示 Sub-Agent 列表
- **文档**: PDD.md、TDD.md、README_CN.md、README.md、MANUAL_CN.md、MANUAL.md 需要补充 Sub-Agent 相关章节
- **版本**: 6 个 Cargo.toml + 6 个 package.json + git tag
- **兼容性**: `.claude/agents/*.md` 的 `tools`、`model` 等字段被解析但忽略，不影响现有 Claude Code 用户的 agent 定义
