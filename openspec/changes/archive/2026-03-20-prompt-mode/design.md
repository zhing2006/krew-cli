## Context

krew-cli 当前仅支持交互式 TUI 模式（ratatui + crossterm raw mode）。核心的 agent 执行逻辑通过 `AgentRuntime.start_completion()` → `mpsc::UnboundedReceiver<AgentEvent>` channel 与 TUI 解耦。`-p` 模式需要一个替代的 AgentEvent 消费者，将输出写到 stdout 而非 TUI。

当前 CLI 入口 (`Cli` struct) 使用 clap，已有 `--config`, `--agents`, `--approval-mode`, `--resume`, `--verbose` 参数。

## Goals / Non-Goals

**Goals:**
- 支持 `krew -p "@agent prompt"` 非交互式执行
- 支持 stdin 管道输入
- 支持 text 和 JSON 两种输出格式
- Text 格式默认 streaming 输出（逐 delta 打印）
- 与 TUI 模式共享所有 krew-core 逻辑（agent loop, routing, tool execution）
- 保持 session 持久化

**Non-Goals:**
- 不支持交互式 approval（`-p` 强制 FullAuto）
- 不支持 `--resume` 与 `-p` 组合（`-p` 始终创建新 session）
- 不支持 `/slash` 命令

## Decisions

### 1. 模块结构：独立的 `prompt_mode.rs`

在 `krew-cli/src/` 下新增 `prompt_mode.rs` 模块，包含：
- `run_prompt_mode()` 异步入口函数
- AgentEvent 消费循环
- 输出格式化逻辑（text / json）

**理由**: 与 TUI 代码完全分离，不污染现有 `app/` 模块。两种模式在 `main()` 层面分支，共享 `load_config()` 和 krew-core。

**备选**: 在 `App` 中加 `headless` flag — 拒绝，因为 App 与 TUI 深度耦合（TextArea, FrameRequester, terminal 引用等）。

### 2. 寻址解析与 stdin 注入的分离

**关键**: 寻址解析 MUST 仅针对原始 `-p` 参数执行，不包含 stdin 内容。流程：

1. 从 `-p` 参数取得 raw prompt
2. 对 raw prompt 调用 `parse_input()` → `(Addressee, body)`
   - `parse_input()` 返回的 body 是完整原始 prompt（含 `@agent` token，不剥离）
   - 这与 TUI 模式行为一致（见 `router.rs:22`）
3. 读取 stdin（如有管道输入）
4. 拼接最终消息 body：`<stdin>{content}</stdin>\n\n{raw_prompt}`（含 @token）

这样 stdin 中出现的 `@agent` 不会污染路由目标。`parse_input()` 只看用户显式写的 prompt 参数。

**理由**: stdin 内容来源不可控（可能是代码文件、日志等），其中的 `@token` 不应影响消息路由。

### 3. 未知 @agent 的处理

不做额外扫描。完全复用 `parse_input()` 的现有语义：

- 未知 `@token`（如 `@nonexistent`、`@dataclass`）被当作普通文本 → 不影响 Addressee
- 如果 prompt 中只有未知 `@token` 而无已知 `@agent`，`parse_input()` 返回 `LastRespondent` → 被决策 7 的寻址强制要求拦截（exit code 2）
- 如果 prompt 中混合了已知和未知 `@token`（如 `@claude explain @dataclass`），已知的正常路由，未知的保留在 body 文本中发给 LLM

**理由**: 额外扫描会误杀正文中的普通 `@` 用法（`@decorator`、`@dataclass`、`@Override` 等）。`parse_input()` 已经按"只匹配已知 agent"的逻辑过滤，足够安全。

### 4. 强制 FullAuto + 自动回复 ApprovalRequest

`-p` 模式下：
- 覆盖 config 中的 `approval_mode` 为 `FullAuto`
- 对收到的 `ApprovalRequest` 事件直接回复 `ReviewDecision::Approved`

**理由**: 非交互模式无法弹出 approval overlay，且用户明确选择了 `-p` 模式即表达了信任。

### 5. Streaming 策略

**Text 格式**: 默认 streaming — 每收到 `TextDelta` 立即 `print!()` + `flush()`，用户实时看到文字流出。`ThinkingDelta` 不输出（thinking 内容对脚本无用）。

**JSON 格式**: 默认非 streaming — 每个 agent 完成后输出一条完整 `{"type":"text","content":"..."}` JSONL。便于下游程序解析完整内容。

### 6. 输出格式与 AgentEvent 全集映射

prompt 模式需要处理 AgentEvent 的所有变体。映射规则：

| AgentEvent | Text 模式 | JSON 模式 |
|---|---|---|
| `ResponseStart` | 输出 `[agent_name]` header | 不输出（agent 名在后续事件中） |
| `ThinkingDelta` | 静默丢弃 | 静默丢弃 |
| `TextDelta` | 立即打印（streaming） | 缓存到 buffer |
| `ServerToolStart` | 输出 `🌐 tool_name` | 输出 `{"type":"server_tool_start",...}` |
| `ServerToolDone` | 输出 `   ⎿  "query"` | 输出 `{"type":"server_tool_done",...}` |
| `ToolCallStart` | 输出 `⚡ tool(args)` | 输出 `{"type":"tool_start",...}` |
| `ToolCallOutput` | 输出 `    {text}`（缩进） | 输出 `{"type":"tool_output",...}` |
| `ToolCallDone` | 输出 `   ⎿  summary` | 输出 `{"type":"tool_done",...}` |
| `Done` | 换行收尾 | 输出 `{"type":"text","content":"full text"}` |
| `Error` | stderr 输出错误 | stderr 输出错误 |
| `ApprovalRequest` | 自动 Approved | 自动 Approved |
| `Retrying` | stderr 输出重试信息 | stderr 输出重试信息 |

### 7. 寻址强制要求

`parse_input()` 返回 `Addressee::LastRespondent` 时（即无 `@` 前缀），报错退出 exit code 2。`-p` 模式下 `LastRespondent` 没有意义（无之前的对话）。

### 8. 多 Agent 错误处理策略：继续执行

与 TUI 模式行为一致：某个 agent 出错后，继续执行队列中的下一个 agent。最终 exit code 取决于是否有任何 agent 出错：

- 全部成功 → exit code 0
- 部分或全部出错 → exit code 1
- 出错 agent 的错误信息输出到 stderr，不影响其他 agent 的 stdout 输出

**理由**: fail-fast 会导致 `@all` 场景下因一个 agent 的临时错误丢失其他 agent 的有效输出。继续执行更符合多 agent 协作的语义。

### 9. Exit Code 实现

当前 `fn main() -> anyhow::Result<()>` 无法精确控制 exit code（anyhow 错误统一返回 1）。改为：

```rust
fn main() {
    let code = run();
    std::process::exit(code);
}

fn run() -> i32 {
    // ... 返回 0, 1, 或 2
}
```

仅在 `-p` 模式路径上需要精确 exit code。TUI 模式路径保持现有 `anyhow::Result` 行为不变（错误即 exit 1）。

**实现**: `run()` 内部对 TUI 分支直接 `?` 传播（错误 → 1），对 prompt 分支返回精确的 0/1/2。

### 10. AI-to-AI Routing

复用 `krew-core::router` 的 `parse_agent_mentions()` 和 `apply_immediate_routing_at()` / `apply_queued_routing()`。行为与 TUI 模式一致，遵循 `agent_to_agent_max_rounds` 配置。

### 11. MCP 初始化

如果 config 中配置了 MCP servers，在 `-p` 模式下也需要初始化并注册工具到 agent registries。执行完成后通过 `McpManager` 的 Drop 清理进程。

## Risks / Trade-offs

- **[FullAuto 安全风险]** → `-p` 模式的使用者需要信任 prompt 来源。这与 Claude Code `-p` 的策略一致，用户通过选择 `-p` 模式隐含接受了这一风险。
- **[stdin 无限读取]** → 大文件管道可能导致内存问题。→ 可以后续加 `--stdin-limit` 参数，当前不做限制（与 Claude Code 行为一致）。
- **[stdin @agent 误路由]** → 已通过"仅从 -p 参数解析寻址"彻底解决。
