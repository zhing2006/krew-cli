# Phase 10: MCP 集成

> 目标：接入 MCP 服务器，通过外部工具扩展 Agent 能力。

## 实现内容

- **MCP Client**：stdio 传输的 JSON-RPC 客户端
- **服务器生命周期**：会话启动时初始化 MCP 服务器子进程，退出时清理
- **工具发现**：`initialize()` → `list_tools()` → 注册到统一工具系统
- **工具调用**：`call_tool()` 通过 JSON-RPC 调用
- **信任级别**：`trust = "auto"` 跳过审批，`trust = "confirm"` 按策略确认

## 验收标准

```toml
# .krew/settings.toml
[[mcp_servers]]
name = "filesystem"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "."]
trust = "auto"
```

```txt
you> @opus 用 MCP 文件系统工具列出目录
[opus] Claude Opus:
  ⚡ mcp:filesystem/list_directory(".")
  目录内容：...
```

## 参考

| 文档 | 位置 | 内容 |
| ---- | ---- | ---- |
| PDD | L217-226 | §4.4.2 MCP 配置示例 |
| PDD | L228-236 | §4.4.3 MCP 工具审批 |
| TDD | L459-475 | §3.4.3 MCP 集成（McpServer 结构、接口） |
| TDD | L420-427 | §3.3.7 MCP 信任级别 |
| TDD | L755-765 | §3.7.2 McpServerConfig 数据结构 |
