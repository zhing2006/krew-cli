## Context

krew-cli 已有完整的工具系统（ToolSpec/ToolHandler/ToolRegistry），支持 6 个内置工具和基于 ApprovalMode 的审批流程。MCP（Model Context Protocol）是 Anthropic 主导的 LLM 工具调用标准协议，通过 JSON-RPC over stdio 让 LLM 调用外部工具服务器。

现有基础：
- `krew-config` 已定义 `McpServerConfig` 和 `McpTrust` 类型
- `krew-tools/src/mcp.rs` 存在但只有注释 stub
- 审批系统（ApprovalMode + ApprovalCache + overlay）已完整实现
- Codex 项目（../codex）有成熟的 MCP 实现可参考

## Goals / Non-Goals

**Goals:**
- 使用 `rmcp` 官方 Rust SDK 实现 MCP 客户端（stdio 传输）
- MCP 服务器生命周期管理（启动、工具发现、清理）
- MCP 工具与现有 ToolRegistry 统一注册和分发
- 基于 MCP 工具 annotations 的智能审批（trust=confirm 时）
- MCP 工具调用的 TUI 显示

**Non-Goals:**
- HTTP/SSE 传输（v2 考虑）
- OAuth 认证流程（v2 考虑）
- MCP Resources/Prompts 功能（只做 Tools）
- 工具列表动态刷新（tool_list_changed 通知）
- MCP 工具结果中的图片内容处理

## Decisions

### D1: 使用 rmcp SDK（而非手写 JSON-RPC）

**选择**: 使用 `rmcp` crate（官方 MCP Rust SDK）

**理由**: rmcp 封装了完整的 MCP 协议细节（JSON-RPC 2.0、handshake、capability negotiation），且是 Codex 项目验证过的方案。手写 JSON-RPC 在协议合规性和边界情况处理上需要大量额外工作。

**rmcp 功能配置**:
```toml
rmcp = { version = "0.15", default-features = false, features = [
    "client",
    "transport-child-process",
] }
```

只启用 `client` 和 `transport-child-process` 两个 feature，最小化依赖。

**替代方案**: 手写 stdio JSON-RPC — 依赖少但维护成本高，协议更新时需跟进。

### D2: 模块组织在 krew-tools 内

**选择**: `krew-tools/src/mcp/` 目录下组织

```
krew-tools/src/mcp/
  ├── mod.rs          # 公开接口
  ├── client.rs       # McpClient: rmcp SDK 封装
  ├── manager.rs      # McpManager: 多服务器生命周期管理
  └── handler.rs      # McpToolHandler: impl ToolHandler for MCP tools
```

**理由**: MCP 本质是工具系统的扩展。生命周期管理虽偏 core，但 McpManager 作为工具基础设施放在 tools crate 中更内聚。krew-core 在 session init/cleanup 时调用 McpManager 的 start/stop 接口。

### D3: 工具命名规范

**LLM 侧**: `mcp__{server}__{tool}`（双下划线分隔，只含 `[a-zA-Z0-9_-]`）
- 兼容所有 LLM Provider 的 tool name 限制（尤其 OpenAI）
- 例：`mcp__filesystem__list_directory`

**TUI 显示**: `mcp:{server}/{tool}`
- 更易读的格式
- 例：`mcp:filesystem/list_directory`

**名称转换**: McpToolHandler 内部维护 qualified_name → (server, tool_name) 映射。

### D4: 基于 annotations 的智能审批

**选择**: trust=confirm 时，根据 MCP 工具的 `ToolAnnotations` 智能决策

审批规则：

| trust | annotations | 审批行为 |
|-------|-------------|---------|
| `auto` | 任意 | 自动执行 |
| `confirm` | `read_only_hint = true` | 自动执行 |
| `confirm` | `destructive_hint = true` | 需要审批 |
| `confirm` | 其他（无 annotations 或未标注） | 需要审批（安全默认） |

**理由**: 比 server 级一刀切更智能。一个 MCP 服务器可能同时提供只读查询和写入操作，只审批有风险的操作可以减少用户疲劳。

**实现**: McpToolHandler 的 `requires_approval()` 根据缓存的 annotations 返回判断结果。但 ToolHandler trait 的 `requires_approval()` 是无参数的，需要在 handler 内部存储每个工具的 annotations。

### D5: McpManager 生命周期

```
Session::init()
  └─ McpManager::start_all(configs)
       ├─ 对每个 McpServerConfig 并发启动
       │    ├─ spawn 子进程
       │    ├─ rmcp serve_client() 握手
       │    ├─ list_tools() 发现工具
       │    └─ 注册到 ToolRegistry
       └─ 返回 McpManager（持有所有 client 连接）

Session::cleanup()
  └─ McpManager::shutdown()
       └─ drop 所有 client（触发进程清理）
```

**启动超时**: 默认 10 秒（每个服务器独立计时）。
**启动失败**: 单个 MCP 服务器启动失败不阻止其他服务器和会话启动，错误在 TUI 中显示。

### D6: ToolRegistry 动态注册

当前 ToolRegistry 只在创建时注册内置工具。需要扩展支持 MCP 启动后的动态注册。

**选择**: 在 ToolRegistry 上增加 `register()` 公开方法（当前已有但需确保对外可用），MCP 工具发现后调用此方法注册。

### D7: McpToolHandler 设计

每个 MCP 服务器的每个工具创建一个 McpToolHandler 实例：

```rust
struct McpToolHandler {
    server_name: String,
    tool_name: String,
    client: Arc<McpClient>,  // 共享同一服务器的连接
    trust: McpTrust,
    annotations: Option<ToolAnnotations>,
}
```

`execute()` 实现通过 `client.call_tool()` 调用 MCP 服务器。

## Risks / Trade-offs

**[rmcp 版本锁定]** → rmcp 0.15 是当前稳定版本，Codex 项目已验证。后续版本升级可能需要适配 API 变化。Mitigation: 在 McpClient 中封装 rmcp 调用，隔离变化面。

**[子进程泄露]** → MCP 服务器是子进程，异常退出可能未清理。Mitigation: 使用 `kill_on_drop(true)` + Drop trait 清理。Windows 上不支持 process group，直接 kill 子进程。

**[启动延迟]** → MCP 服务器（特别是 npx 启动的 Node.js 服务器）启动可能需要数秒。Mitigation: 并发启动所有服务器，且不阻塞用户输入。

**[工具名冲突]** → 不同 MCP 服务器可能暴露同名工具。Mitigation: qualified name 包含 server 名称前缀，天然避免冲突。
