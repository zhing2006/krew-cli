## Context

krew-cli 是一个多 AI Agent 协作 CLI 工具，当前的 Agent 以对等（peer）模式工作——共享同一份对话历史 `Vec<ChatMessage>`，通过 `@mention` 和 `#whisper` 路由消息。所有 Agent 的 tool call 中间消息都累积在主对话中，导致上下文膨胀和 token 浪费。

现有基础设施：
- `discovery.rs`：统一的多目录扫描机制（`.krew/` `.agents/` `.claude/`，项目级+用户级共 6 个路径）
- `skill/discovery.rs`：Markdown + YAML frontmatter 解析，可高度复用
- `agent_loop.rs`：Agent 的 tool call 循环，通过 `AgentEvent` channel 与 TUI 通信
- `ToolContext.output_tx`：shell tool 已有的流式输出转发机制
- `.claude/agents/git.md`：已存在的 Claude Code 兼容 Agent 定义文件

**依赖方向约束**: `krew-core` → `krew-tools`（单向）。`krew-tools` 不能依赖 `krew-core`，因此涉及 `AgentRuntime`、`AgentEvent` 等 core 类型的逻辑 MUST 放在 `krew-core` 中。`RunAgentTool` 在 krew-core 中实现（impl `krew_tools::ToolHandler`），注册到 krew-tools 的 `ToolRegistry` 中。

**双入口约束**: krew-cli 有两条启动路径——TUI 模式（`app/state.rs`）和 prompt 模式（`prompt_mode/mod.rs`），两者各自独立执行 `init_agents()` + MCP 初始化。Sub-Agent 功能 MUST 同时覆盖两条路径。

## Goals / Non-Goals

**Goals:**
- 实现同步阻塞式 `run_agent` tool，让父 Agent 将专项任务委派给 Sub-Agent 在隔离上下文中执行
- 兼容 Claude Code 的 `.claude/agents/*.md` 定义格式（解析但忽略不支持的字段）
- Sub-Agent 直接使用父 Agent 的运行时资源（client、tools、approval cache 等），仅替换 system prompt
- Sub-Agent 执行期间的 tool 调用过程（tool events）实时流式展示给用户
- Sub-Agent 遵守父 Agent 的 approval 配置（审批请求转发到 TUI）
- TUI 模式和 prompt 模式（`-p`）均支持 Sub-Agent
- 版本升级至 v0.8.0

**Non-Goals:**
- 异步/后台 Sub-Agent（Phase 2 未来扩展）
- Sub-Agent 嵌套（Sub-Agent 不能再 spawn Sub-Agent）
- Sub-Agent 的 tool/model/provider 限制或覆盖（一律使用父 Agent 的）
- TUI 状态栏（同步模式不需要）
- Sub-Agent 间通信或通知机制
- Sub-Agent 会话持久化和恢复

## Decisions

### D0: 实验性功能，默认关闭

**决策**: 在 `krew-config::Settings` 中新增 `sub_agent_enabled: bool`（默认 `false`）。当 `false` 时，完全跳过 Sub-Agent 发现、catalog 注入、tool 注册——零开销，不读取任何 agent 定义文件。

**理由**: Sub-Agent 是实验性功能，对现有行为有侵入性（新增 tool、修改 system prompt）。默认关闭保证不影响现有用户，显式启用后才激活。

### D1: 单一 `run_agent` tool 而非 spawn + wait 分离

**决策**: 只提供一个 `run_agent` tool，同步阻塞执行。

**理由**: 同步模式下不需要 ID 追踪、并发管理、状态查询。一个 tool 覆盖 spawn + execute + wait + close 的完整生命周期。未来扩展异步时再拆分为 `spawn_agent` + `wait_agent`。

**替代方案**: 即便是同步也拆成 spawn + wait 两个 tool。但这增加了 LLM 的认知负担且没有实际收益。

### D2: Agent 定义文件格式——复用 `.claude/agents/*.md`

**决策**: 每个 `.md` 文件即一个 Sub-Agent 定义。YAML frontmatter 中 `name` 和 `description` 为必需字段，body 为 system prompt。`tools`、`model`、`provider`、`permissionMode` 等 Claude Code 字段解析但忽略。

**理由**: `.claude/agents/git.md` 已经存在且格式成熟。直接兼容意味着用户的 Claude Code Agent 定义无需修改即可在 krew-cli 中使用。

**兼容映射**:
- `name` → Sub-Agent 标识 ✓
- `description` → tool schema 中 `agent` 参数的 enum 描述 ✓
- `color` → TUI 显示颜色（可选）✓
- `maxTurns` → agent loop 最大轮次（可选，默认 30）✓
- 其他字段 → 解析但忽略

### D3: 发现路径——复用 `discovery_paths(cwd, "agents")`

**决策**: 使用 `discovery::discovery_paths(cwd, "agents")` 生成扫描路径，在每个目录下扫描 `*.md` 文件（非递归，只扫描顶层）。

**理由**: 与 skills 使用同一个 discovery 基础设施。Agent 定义是扁平的 `.md` 文件（不像 skill 有子目录结构），所以只需扫描顶层。

**扫描结果**: 6 个路径，first-found-wins 去重：
```
<cwd>/.krew/agents/*.md
<cwd>/.agents/agents/*.md
<cwd>/.claude/agents/*.md
<home>/.krew/agents/*.md
<home>/.agents/agents/*.md
<home>/.claude/agents/*.md
```

### D4: 上下文完全隔离

**决策**: Sub-Agent 启动时只有 `[system_prompt, user_task]` 两条消息，不携带父 Agent 的任何历史。

**理由**: 上下文隔离是 Sub-Agent 的核心价值。如果需要传递上下文，父 Agent 可以在 `task` 参数中以文本形式包含必要信息。这比 fork 机制简单得多，且给 LLM 完全的控制权。

### D5: `RunAgentTool` 在 krew-core 中实现，注册到 krew-tools 的 ToolRegistry

**决策**: `RunAgentTool` struct 和 `ToolHandler` impl 放在 `krew-core::sub_agent::run_agent_tool` 模块中。在初始化阶段注册到 `krew-tools::ToolRegistry` 中（与 MCP tool 注册方式一致）。

**理由**: `RunAgentTool::execute()` 需要访问 `AgentRuntime`、`AgentEvent`、`ApprovalCache` 等 `krew-core` 类型来运行 Sub-Agent。由于依赖方向是 `krew-core` → `krew-tools`（`krew-tools` 不能反向依赖 `krew-core`），tool 的实现必须放在 `krew-core` 中。`krew-core` 已经依赖 `krew-tools`，可以直接 impl `krew_tools::ToolHandler`。Tool 注册进 `ToolRegistry` 和 MCP tool 完全一致，不需要 krew-tools 侧做代码改动（注册是运行时行为）。

### D6: 使用父 Agent 的运行时资源——共享 Arc + 过滤 tool_defs

**决策**: Sub-Agent 直接使用父 Agent 的 `Arc<dyn LlmClient>`、`Arc<ToolRegistry>`（含 MCP tools）、`ApprovalCache` 等运行时资源，不重建。在调用 `start_completion` 时从 `tool_defs`（发送给 LLM 的 tool 定义列表）中过滤掉 `run_agent`。

**理由**:
- 父 Agent 的 `Arc<ToolRegistry>` 在 MCP 初始化后已包含所有 live tools（built-in + MCP），直接共享即可获得完整能力
- 不需要重建 LlmClient（API key、provider 配置等都在里面）
- `ToolRegistry` 没有 clone/filter 方法，也不应该为此修改其接口
- 需要为 `start_completion` 新增 `exclude_tools: Option<&[&str]>` 参数，在构建 `tool_defs` 时跳过指定 tool

**替代方案**: 为 ToolRegistry 添加 `clone_without()` 方法。但这要求 `ToolHandler` 支持 Clone（当前是 `Box<dyn ToolHandler>`，不支持），改动过大。

### D7: Sub-Agent 不能嵌套——tool_defs 过滤 + execute() depth guard 双重保障

**决策**: 两层防护确保 Sub-Agent 绝不能嵌套调用 `run_agent`：

1. **tool_defs 过滤**（D6）：Sub-Agent 的 LLM 看不到 `run_agent` tool，正常情况下不会尝试调用
2. **execute() depth guard**：`RunAgentTool` 持有一个 `Arc<AtomicBool>` 类型的 `is_running` 标志。execute() 入口处 CAS 设为 true，退出时设回 false。如果已经为 true（说明是嵌套调用），立即返回 error: `"Sub-agent nesting is not allowed"`

**理由**: 仅靠 tool_defs 过滤是"正常模型不会触发"级别的保障，不是系统保证。共享同一个 `ToolRegistry` 意味着 dispatch 层仍然能找到 `run_agent` handler。depth guard 提供了硬保证：即使 LLM 以某种方式调用了 `run_agent`（如 prompt injection），execute() 也会拒绝执行。

### D8: 流式输出——只转发 tool events，不转发 TextDelta

**决策**: `RunAgentTool::execute()` 内部只将 Sub-Agent 的 **tool 相关事件**（`ToolCallStart`、`ToolCallOutput`、`ToolCallDone`、`ServerToolStart`、`ServerToolDone`）通过 `ctx.output_tx` 转发给 TUI。`TextDelta` 不转发，仅累积后作为最终 tool result 返回。

**理由**: `ToolCallOutput` 是行级文本（TUI 按行渲染、加缩进和分隔线），而 `TextDelta` 是 token/chunk 级（碎片化的 streaming tokens）。将 TextDelta 塞入 ToolCallOutput 会导致 TUI 将每个 token 碎片当作独立行渲染，显示效果完全错乱。Sub-Agent 的最终文本通过 tool result 返回父 Agent 即可，无需实时流式展示。

**格式**:
```
🔧 run_agent("git", "提交代码")
  🔧 shell("git status")
    M  src/lib.rs
  🔧 shell("git commit ...")
    [main a1b2c3d] feat: add sub-agent
  ✓ done
```

### D9: Approval 转发——ToolContext 新增类型擦除的 parent event sender

**决策**: 在 `krew-tools::ToolContext` 中新增 `parent_event_tx: Option<Box<dyn Any + Send>>` 字段（默认 `None`）。`krew-core` 的 `create_tool_context()` 在处理 `run_agent` tool 时，将父 Agent 当前 turn 的 `UnboundedSender<AgentEvent>` clone 装箱后设入此字段。`RunAgentTool::execute()` 通过 `downcast_ref::<UnboundedSender<AgentEvent>>()` 取回 sender，用于转发 Sub-Agent 的 `ApprovalRequest`。

**downcast 协议**: 值的具体类型始终为 `tokio::sync::mpsc::UnboundedSender<AgentEvent>`，由 `krew-core::agent::agent_loop::create_tool_context()` 设置。
**所有权**: `Box<dyn Any + Send>` 持有 sender 的 clone；RunAgentTool 通过 `downcast_ref` 借用后 clone。
**失败语义**: 如果 downcast 失败（不应发生），Sub-Agent 的 `ApprovalRequest` 无法转发，Sub-Agent 的 agent_loop 会因 oneshot 超时/drop 而中止该 tool call，返回 denied result 给 Sub-Agent LLM。不会 panic。

**事件转发路径**:
```
Sub-Agent agent_loop
  │ (sub_tx) → AgentEvent
  ▼
RunAgentTool::execute() 消费 sub_rx
  │
  ├── ToolCallStart/Output/Done → ctx.output_tx (ToolCallOutput 管线)
  ├── ApprovalRequest           → parent_event_tx (downcast, 转发到父 channel)
  ├── TextDelta                 → 累积到 final_text (不转发)
  └── Done/Error                → return ToolResult
```

**理由**:
- `parent_event_tx` 是每次 agent turn 中由 `create_tool_context` 设置的，精确对应当前 turn 的 event channel——解决了 "构造时拿不到 parent_tx" 的时序问题
- krew-tools 侧改动极小（一个 `Option<Box<dyn Any + Send>>` 字段，默认 `None`），不影响任何现有 tool
- 类型安全通过 downcast 保证，且 downcast 逻辑完全在 krew-core 中，krew-tools 不感知具体类型
- Sub-Agent 完整遵守父 Agent 的 `ApprovalMode` 和共享的 `ApprovalCache`

**替代方案 1**: 修改 `start_completion` 支持外部注入 tx。需要改 API 签名，影响所有调用方。
**替代方案 2**: Sub-Agent 强制 FullAuto 模式。简单但违反用户的审批意图。
**替代方案 3**: 在 ToolContext 中用泛型参数。会扩散到 ToolHandler trait 和 ToolRegistry，改动过大。

### D10: `run_agent` tool 不需要用户审批

**决策**: `run_agent` tool 的 `requires_approval()` 返回 `false`。

**理由**: Sub-Agent 执行的具体 tool（shell、edit_file 等）自身有审批机制（通过 D9 的 ApprovalRequest 转发保证）。`run_agent` 本身只是一个调度器，不产生副作用。双重审批会严重影响体验。

### D11: 注册时机——在 ToolRegistry 最终可变阶段

**决策**: `register_sub_agents()` 在 ToolRegistry 最终可变阶段调用——若有 MCP 则在 MCP 注入之后，否则在 `init_agents()` 之后直接注册。两条启动路径的注册点：

- **TUI 模式**: `App::run()` 中，`init_mcp()` 块之后（无论 MCP 是否配置）
- **prompt 模式**: MCP 初始化块之后（无论 MCP 是否配置）

**理由**: 当前两条入口都是"只有配置了 MCP 才进入 MCP 初始化块"（TUI: `state.rs:255`，prompt: `mod.rs:75`）。如果只在 MCP 块内注册，无 MCP 配置时 `run_agent` 不会被注册。因此注册点必须在 MCP 块**之外**、之后。

**实现**: 将 `register_sub_agents()` 调用放在 MCP 初始化 `if` 块的下方（同级），这样无论是否有 MCP 配置，都会执行注册。

### D12: TUI 模式和 prompt 模式均需覆盖

**决策**: Sub-Agent 的发现、catalog 注入、`RunAgentTool` 注册逻辑 MUST 同时应用于两条启动路径：
- TUI 模式：`app/state.rs`
- prompt 模式：`prompt_mode/mod.rs`

**理由**: prompt 模式（`-p`）有独立的初始化链（`prompt_mode/mod.rs:57-89`），如果只在 TUI 路径注册 `RunAgentTool`，`-p` 模式下 Sub-Agent 不可用，导致行为不一致。

**实现**: 将 Sub-Agent 发现 + RunAgentTool 注册封装为 `register_sub_agents()` 函数，两条路径各自调用。

## Risks / Trade-offs

**[风险] LLM 可能不会合理使用 `run_agent`**
→ 通过 tool description 提供清晰的使用指引，说明何时应该委派 vs 自己做。在 system prompt 的 identity section 中注入可用 Sub-Agent 列表（类似 skill catalog）。

**[风险] Sub-Agent 的 tool 调用可能非常多，ToolCallOutput 显示过长**
→ 复用 `generate_tool_summary` 保持简洁。考虑为 Sub-Agent 的 tool 输出增加行数上限。

**[权衡] 完全使用父 Agent 资源 vs 可配置 tools**
→ 简化实现选择了完全使用父 Agent 资源。代价是 Sub-Agent 可能拥有不必要的能力（如 git agent 其实不需要 write_file）。未来可扩展。

**[权衡] 同步阻塞 vs 异步并行**
→ 同步更简单但失去并行能力。这是 Phase 1 的有意取舍，架构上不阻碍 Phase 2 扩展（Sub-Agent 的 agent_loop 已运行在独立 tokio task 中，未来只需将 rx 存储而非立即消费即可实现异步）。

**[权衡] `Box<dyn Any>` 类型擦除 vs 泛型 ToolContext**
→ `Box<dyn Any>` 虽然牺牲编译期类型安全，但改动局部（一个 Option 字段），且 downcast 在 krew-core 内部完成、有明确的失败语义。泛型方案会扩散到整个 tool 抽象层，代价过高。
