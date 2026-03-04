## Why

krew-cli 的内置工具（read_file、write_file、edit_file、shell、glob、grep）覆盖了基础文件操作和命令执行场景，但用户需要接入外部工具生态（如 GitHub API、数据库、自定义服务等）。MCP（Model Context Protocol）是连接 LLM 与外部工具的标准协议，通过集成 MCP，Agent 可以动态发现和调用任意 MCP 服务器提供的工具，大幅扩展能力边界。

## What Changes

- 新增 MCP Client 模块，基于 `rmcp` 官方 Rust SDK，通过 stdio 传输与 MCP 服务器通信
- 新增 MCP 服务器生命周期管理：会话启动时初始化子进程，退出时清理
- 新增 MCP 工具发现：`initialize()` → `list_tools()` → 注册到统一 ToolRegistry
- 新增 MCP 工具调用：通过 `rmcp` SDK 的 `call_tool()` 执行
- 扩展审批流程：MCP 工具支持基于 annotations（destructive_hint/read_only_hint/open_world_hint）的智能审批
- MCP 工具的 TUI 显示格式：`mcp:{server}/{tool}`
- LLM 侧工具名格式：`mcp__{server}__{tool}`（兼容所有 Provider）

## Capabilities

### New Capabilities
- `mcp-client`: MCP 客户端实现，包括 stdio 传输、服务器生命周期管理、工具发现与调用
- `mcp-tool-integration`: MCP 工具与现有 ToolRegistry 的集成，包括工具注册、调用分发、审批流程

- `tool-registry`: MCP 工具动态注册和审批查询扩展
- `tool-approval-flow`: MCP 信任级别（auto/confirm）和 annotations 审批策略
- `config-types`: McpServerConfig Clone 支持和 McpTrust 默认值

### Modified Capabilities
<!-- No existing spec requirements are being changed -->

## Impact

- **新增依赖**: `rmcp` crate（官方 MCP Rust SDK）
- **受影响 crate**: krew-tools（新增 mcp 模块）、krew-core（会话初始化/清理时管理 MCP 服务器）、krew-config（已有 McpServerConfig 类型）
- **ToolRegistry 变更**: 需要支持动态注册和 MCP 工具分发
- **审批流程变更**: 新增 MCP 工具的 annotations-based 审批逻辑
- **TUI 变更**: MCP 工具调用的显示格式
