## Context

krew-cli 是一个多 AI Agent 协作 CLI 工具，当前的 Agent 以对等（peer）模式工作——共享同一份对话历史 `Vec<ChatMessage>`，通过 `@mention` 和 `#whisper` 路由消息。所有 Agent 的 tool call 中间消息都累积在主对话中，导致上下文膨胀和 token 浪费。

现有基础设施：
- `discovery.rs`：统一的多目录扫描机制（`.krew/` `.agents/` `.claude/`，项目级+用户级共 6 个路径）
- `skill/discovery.rs`：Markdown + YAML frontmatter 解析，可高度复用
- `agent_loop.rs`：Agent 的 tool call 循环，通过 `AgentEvent` channel 与 TUI 通信
- `ToolContext.output_tx`：shell tool 已有的流式输出转发机制
- `.claude/agents/git.md`：已存在的 Claude Code 兼容 Agent 定义文件

## Goals / Non-Goals

**Goals:**
- 实现同步阻塞式 `run_agent` tool，让父 Agent 将专项任务委派给 Sub-Agent 在隔离上下文中执行
- 兼容 Claude Code 的 `.claude/agents/*.md` 定义格式（解析但忽略不支持的字段）
- Sub-Agent 完全继承父 Agent 的 model、provider、tools、MCP、skills、approval 设置
- Sub-Agent 执行期间的 tool 调用过程实时流式展示给用户（复用 `ToolCallOutput` 机制）
- 版本升级至 v0.8.0

**Non-Goals:**
- 异步/后台 Sub-Agent（Phase 2 未来扩展）
- Sub-Agent 嵌套（Sub-Agent 不能再 spawn Sub-Agent）
- Sub-Agent 的 tool/model/provider 限制或覆盖（一律继承父 Agent）
- TUI 状态栏（同步模式不需要）
- Sub-Agent 间通信或通知机制
- Sub-Agent 会话持久化和恢复

## Decisions

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

### D5: 流式输出——复用 `ToolCallOutput` 管线

**决策**: `run_agent` tool 的 execute 内部消费 Sub-Agent 的 `AgentEvent` 流，将关键事件（`ToolCallStart`、`ToolCallOutput`、`ToolCallDone`、`TextDelta`）通过 `ctx.output_tx` 转发给 TUI。

**理由**: `ToolCallOutput` 是现有的流式输出机制（shell tool 使用），TUI 已有完整的渲染支持。无需新增 `AgentEvent` 变体或 TUI 渲染逻辑。

**格式**:
```
🔧 run_agent("git", "提交代码")
  🔧 shell("git status")
    M  src/lib.rs
  🔧 shell("git commit ...")
    [main a1b2c3d] feat: add sub-agent
  ✅ done
```

### D6: Approval 处理——继承父 Agent 的策略和缓存

**决策**: Sub-Agent 继承父 Agent 的 `ApprovalMode` 和 `ApprovalCache`（Arc 共享）。Sub-Agent 执行需要审批的 tool 时，审批请求通过 `output_tx` 无法传递（类型不匹配），因此 Sub-Agent 在 `run_agent` tool 内部直接使用父 Agent 的 event channel 发送 `ApprovalRequest`。

**实现要点**: `run_agent` tool 需要持有父 Agent 的 `mpsc::UnboundedSender<AgentEvent>` 引用，以便将 Sub-Agent 的 `ApprovalRequest` 转发到 TUI。这要求扩展 `ToolContext` 添加一个 `event_tx` 字段。

### D7: Sub-Agent 不能嵌套

**决策**: Sub-Agent 的 `ToolRegistry` 中不注册 `run_agent` tool。

**理由**: 防止递归 spawn 导致的复杂度爆炸。一层隔离已能覆盖绝大多数场景。

### D8: `run_agent` tool 不需要用户审批

**决策**: `run_agent` tool 的 `requires_approval()` 返回 `false`。

**理由**: Sub-Agent 执行的具体 tool（shell、edit_file 等）自身有审批机制。`run_agent` 本身只是一个调度器，不产生副作用。双重审批会严重影响体验。

## Risks / Trade-offs

**[风险] LLM 可能不会合理使用 `run_agent`**
→ 通过 tool description 提供清晰的使用指引，说明何时应该委派 vs 自己做。在 system prompt 的 identity section 中注入可用 Sub-Agent 列表（类似 skill catalog）。

**[风险] Sub-Agent 的 tool 调用可能非常多，ToolCallOutput 显示过长**
→ 复用 `generate_tool_summary` 保持简洁。考虑为 Sub-Agent 的 tool 输出增加行数上限。

**[权衡] 完全继承 vs 可配置 tools**
→ 简化实现选择了完全继承。代价是 Sub-Agent 可能拥有不必要的能力（如 git agent 其实不需要 write_file）。未来可扩展。

**[权衡] 同步阻塞 vs 异步并行**
→ 同步更简单但失去并行能力。这是 Phase 1 的有意取舍，架构上不阻碍 Phase 2 扩展。

**[风险] `ToolContext` 扩展可能影响现有 tool**
→ 新增的 `event_tx` 字段使用 `Option` 类型，默认 `None`，不影响现有 tool 的行为。
