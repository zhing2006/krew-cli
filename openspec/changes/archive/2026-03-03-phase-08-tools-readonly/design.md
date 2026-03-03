## Context

Phase 7 完成后，krew-cli 已具备多 Agent 对话、流式渲染、会话持久化能力。但 Agent 无法访问文件系统，只能进行纯文本对话。Phase 8 引入只读工具和 Agent Loop 工具调用循环，使 Agent 能够读取项目文件。

当前状态：
- `Tool` trait 已定义（krew-tools），但所有内置工具仅为空 stub
- `StreamEvent::ToolCall` 已由所有 LLM Provider 正确解析和发出
- Agent Loop 收到 ToolCall 时直接 `continue` 跳过
- `ChatMessage` 只有 `content: String`，无法携带工具调用元数据
- `MessageEntry` 无工具调用持久化字段

参考实现：codex-rs 项目的工具系统（ToolHandler trait、ToolRegistry、parallel.rs）。

## Goals / Non-Goals

**Goals:**
- 实现 3 个只读内置工具（read_file、glob、grep）
- ToolSpec / ToolHandler 分离架构，建立工具注册表
- Agent Loop 支持多轮工具调用循环（最多 25 轮）
- 单轮多工具调用并行执行
- 路径边界安全校验
- 工具调用 TUI 渲染
- ChatMessage 扩展支持工具调用元数据
- 会话持久化支持工具调用
- 各 Provider 的 convert_messages() 支持 tool result 消息

**Non-Goals:**
- 写入工具（write_file、edit_file、shell）— Phase 9
- 工具审批流程 — Phase 9
- MCP 集成 — Phase 10
- 工具调用的用户中断（Ctrl+C）— Phase 12

## Decisions

### Decision 1: ToolSpec / ToolHandler 分离

**选择**：将 LLM 可见的工具规格（ToolSpec）与执行逻辑（ToolHandler）分离。

**当前 Tool trait**（合一设计）：
```rust
trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Value;
    fn requires_approval(&self) -> bool;
    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError>;
}
```

**新设计**：
```rust
// krew-tools/src/lib.rs

/// Tool specification sent to LLM providers (JSON Schema).
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Trait for tool execution logic.
#[async_trait]
pub trait ToolHandler: Send + Sync {
    fn name(&self) -> &str;
    fn requires_approval(&self) -> bool;
    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError>;
}

/// Registry that pairs specs with handlers.
pub struct ToolRegistry {
    specs: Vec<ToolSpec>,
    handlers: HashMap<String, Box<dyn ToolHandler>>,
}

impl ToolRegistry {
    pub fn register(&mut self, spec: ToolSpec, handler: Box<dyn ToolHandler>);
    pub fn specs(&self) -> &[ToolSpec];
    pub fn dispatch(&self, name: &str, args: Value) -> Result<ToolResult, ToolError>;
}
```

**理由**：后续 MCP 工具只有 Spec 没有内置 Handler（通过 MCP 协议调用），分离设计更灵活。ToolSpec 可直接转换为 `ToolDefinition` 传给 LLM。

**替代方案**：保持合一的 Tool trait。对 Phase 8 够用，但 Phase 10 MCP 集成时需要重构。

### Decision 2: ChatMessage 扩展

**选择**：在 `krew-llm::ChatMessage` 中增加工具调用相关字段。

```rust
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    pub name: Option<String>,
    // New fields:
    pub tool_calls: Option<Vec<ToolCallInfo>>,    // assistant 消息携带的工具调用
    pub tool_call_id: Option<String>,              // tool result 消息的关联 ID
}

pub struct ToolCallInfo {
    pub id: String,
    pub name: String,
    pub arguments: String,  // JSON string
}
```

**理由**：各 Provider 的 convert_messages() 需要知道工具调用的完整信息才能正确构建 API 请求。assistant 消息可能同时包含文本和工具调用，不能只用 content 字段。

### Decision 3: Agent Loop 工具循环

**选择**：Agent Loop 从单次 `chat_stream()` 改为循环模式。

```
loop {
    stream = chat_stream(messages, tools)
    (text, tool_calls) = collect_stream(stream)

    if tool_calls.is_empty() {
        break  // LLM finished, no more tool calls
    }

    // Append assistant message (with text + tool_calls) to messages
    // Execute tools (parallel for readonly)
    // Append tool result messages to messages
    // Continue loop → re-call LLM with tool results

    if loop_count >= MAX_TOOL_ROUNDS {
        break  // Safety limit
    }
}
```

**最大轮数**：25 轮（可通过 settings 配置 `max_tool_rounds`）。超过限制时发送 `AgentEvent::Error` 通知 TUI。

**理由**：参考 claude-code 的 25 轮默认值。足够复杂任务使用，同时防止失控循环消耗 token。

### Decision 4: 多工具并行执行

**选择**：Phase 8 的只读工具全部并行执行。

```rust
let results = futures::future::join_all(
    tool_calls.iter().map(|tc| registry.dispatch(&tc.name, tc.args.clone()))
).await;
```

**理由**：只读工具无副作用，并行执行更快。Phase 9 引入写入工具时再加串行/互斥逻辑。

### Decision 5: 只读工具实现

**grep 工具**：使用 ripgrep 底层 crate（`grep-searcher` + `grep-regex`），编译进二进制，零外部依赖。

**glob 工具**：使用 `globset` + `walkdir`。

**read_file 工具**：使用 `tokio::fs::read_to_string`，支持可选 `offset` / `limit` 行号参数。

**理由**：单文件零依赖发布目标，不能依赖外部命令（如 rg）。ripgrep 底层 crate 就是 rg 的引擎，性能一致。

### Decision 6: 路径边界校验

**选择**：在 `krew-tools/src/lib.rs` 中提供共用 `validate_path()` 函数。

```rust
pub fn validate_path(path: &str, cwd: &Path) -> Result<PathBuf, ToolError> {
    let resolved = cwd.join(path).canonicalize()?;
    let cwd_canonical = cwd.canonicalize()?;
    if !resolved.starts_with(&cwd_canonical) {
        return Err(ToolError::Execution("path outside workspace boundary"));
    }
    Ok(resolved)
}
```

所有文件工具在 execute() 前调用此函数。拒绝 `..` 穿越和符号链接逃逸。

### Decision 7: convert_messages() 扩展

**选择**：各 Provider 的 `convert_messages()` 统一处理 tool result 消息。Agent Loop 只构建统一的 `ChatMessage`（包含 tool_calls / tool_call_id 字段），由各 Provider 转换为自己的格式。

各 Provider 的工具结果消息格式：
- **OpenAI Chat**: `{ role: "tool", tool_call_id, content }`
- **OpenAI Responses**: `{ type: "function_call_output", call_id, output }`
- **Anthropic**: `{ role: "user", content: [{ type: "tool_result", tool_use_id, content }] }`
- **Google**: `{ role: "user", parts: [{ functionResponse: { name, response } }] }`

**理由**：保持 convert_messages() 作为唯一的格式转换层，符合现有架构。Agent Loop 不关心 Provider 差异。

### Decision 8: AgentEvent 新增变体

```rust
pub enum AgentEvent {
    // ... existing variants ...

    /// A tool call is starting execution.
    ToolCallStart {
        name: String,
        arguments: String,
    },
    /// A tool call has completed.
    ToolCallDone {
        name: String,
        result_summary: String,  // e.g., "42 lines", "5 matches"
    },
}
```

TUI 渲染格式：`⚡ read_file("src/main.rs") — 42 lines`

### Decision 9: 会话持久化扩展

`MessageEntry` 增加字段：

```rust
pub struct MessageEntry {
    // ... existing fields ...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

pub struct ToolCallEntry {
    pub id: String,
    pub name: String,
    pub arguments: String,
}
```

工具调用完整持久化，恢复会话时能还原工具调用上下文。

### Decision 10: 工具注册时机

在 `init_agents()` 中创建 `AgentRuntime` 时，根据 `agent_config.tools` 字段决定是否注册工具。默认 `true`。

```rust
let tools = if agent_config.tools {
    create_builtin_readonly_tools(cwd)
} else {
    ToolRegistry::empty()
};
```

工具实例需要 `cwd`（路径边界），在 App 层创建后传入。

## Risks / Trade-offs

- **[Risk] ripgrep crate 可能引入大量传递依赖** → 仔细检查 `grep-searcher` / `grep-regex` 的依赖树，确认无 dup crates。如果依赖树过重，fallback 到简单的 `regex` + `walkdir` 实现。
- **[Risk] canonicalize() 在 Windows 上返回 `\\?\` 前缀路径** → 使用 `dunce::canonicalize()` 或手动 strip 前缀，确保路径比较正确。
- **[Risk] 25 轮工具循环可能消耗大量 token** → 每轮累计 usage，超过阈值时提前终止并通知用户。
- **[Trade-off] ChatMessage 增加字段会影响所有现有代码** → 新字段全部为 Option，默认 None，现有代码无需修改。
