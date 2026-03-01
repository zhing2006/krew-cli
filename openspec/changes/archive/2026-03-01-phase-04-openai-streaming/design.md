## Context

krew-cli Phase 1-3 已完成：TUI inline viewport、配置加载、@ 寻址解析、Slash 命令。当前用户消息仅 echo 回显，不调用任何 LLM。本阶段要打通 "用户输入 → LLM 请求 → 流式渲染" 的完整链路。

现有基础：
- `LlmClient` trait、`StreamEvent`、`Usage`、`ChatMessage` 类型已定义在 krew-llm
- `AgentRuntime` 结构体已定义在 krew-core（含 `config`、`client`、`tools`、`is_responding`）
- `AgentConfig` 含 `provider`、`model`、`api_type`、`color`、`sampling` 等字段
- `ProviderConfig` 含 `api_key_env`、`base_url`、`azure_endpoint` 等字段
- krew-cli 已有 `insert_lines_above()`、`FrameScheduler`、事件循环

参考实现：codex-rs/tui 的流式渲染管线（MarkdownStreamCollector、AdaptiveChunkingPolicy、StreamController）。

## Goals / Non-Goals

**Goals:**
- 实现 OpenAI Chat Completions API 流式客户端（SSE）
- 实现单 Agent 对话循环（不含工具调用）
- 实现 newline-gated 流式 Markdown 渲染管线（含自适应背压）
- 实现代码块语法高亮（syntect）
- 显示 Agent 标识和 Token 用量

**Non-Goals:**
- 其他 Provider（Anthropic、Google、OpenAI Responses）— Phase 5
- 工具调用和 Agent Loop 多轮 — Phase 8
- 多 Agent @all 串行执行 — Phase 6
- 会话持久化 — Phase 7
- Web Search — Phase 12

## Decisions

### D1: krew-core ↔ krew-cli 通信方式 — tokio mpsc channel

**选择**: krew-core 的 agent loop 通过 `tokio::mpsc::unbounded_channel` 向 krew-cli 发送 `AgentEvent`。

**备选方案**:
- A) Agent loop 返回 `Stream<Item = AgentEvent>` — 需要复杂的生命周期管理
- B) mpsc channel — 解耦生产者和消费者，agent loop 作为 tokio::spawn task 运行

**理由**: channel 方式让 agent loop 在独立 task 中运行，与 TUI 事件循环通过 `select!` 自然整合。codex 也采用类似的 AppEvent channel 模式。

```
krew-core (tokio task)              krew-cli (event loop)
┌──────────────────┐               ┌──────────────────────┐
│ agent.complete() │    mpsc       │ tokio::select! {     │
│   chat_stream()  │──────────────▶│   event = rx.recv()  │
│   emit AgentEvent│   AgentEvent  │   key = crossterm    │
│                  │               │   tick = scheduler   │
└──────────────────┘               └──────────────────────┘
```

### D2: AgentEvent 设计

```rust
pub enum AgentEvent {
    /// Agent 回复头部信息（名称、颜色）
    ResponseStart { agent_name: String, display_name: String, color: String },
    /// 流式文本片段
    TextDelta(String),
    /// 流结束，携带 token 用量
    Done(Usage),
    /// Agent 返回错误
    Error(String),
}
```

独立出 `ResponseStart` 事件，让 TUI 在收到第一个 delta 之前就可以渲染 Agent 标签头部。

### D3: 流式渲染管线 — 简化版 codex 架构

参考 codex 但去掉不需要的层次：

```
TextDelta(text)
     ↓
MarkdownStreamCollector        (累积文本，\n 时渲染为 Vec<Line>)
     ↓
StreamState                    (VecDeque<QueuedLine> 带时间戳)
     ↓
AdaptiveChunkingPolicy         (Smooth: 1行/tick | CatchUp: 全部)
     ↓
insert_lines_above()           (直接用现有基础设施)
```

**省略的 codex 组件**:
- `HistoryCell` trait — 我们直接 insert_lines，不需要多态渲染单元
- `PlanStreamController` — 我们没有 plan 模式
- `StreamController` — 我们把 push/drain 逻辑直接放在 App 方法中

**自适应背压参数**（与 codex 相同）:
- 进入 CatchUp: 队列深度 ≥ 8 行 OR 最老行年龄 ≥ 120ms
- 退出 CatchUp: 队列深度 ≤ 2 行 AND 最老行年龄 ≤ 40ms，持续 250ms
- 重新进入冷却: 250ms
- 严重积压逃逸: 64 行 OR 300ms

### D4: Markdown 渲染技术栈

**选择**: `pulldown_cmark`（CommonMark 解析）+ `syntect`（代码高亮）+ `two-face`（主题包）

**理由**: 与 codex 相同技术栈，经过生产验证。pulldown_cmark 是 Rust 生态最成熟的 Markdown 解析器，syntect 支持 250+ 语言的语法高亮。

**渲染策略**: Newline-gated — 每次遇到 `\n` 时，对整个 buffer 重新运行 pulldown_cmark 解析（因为 Markdown 上下文可能跨行，如代码块、列表），然后只返回新增的行。

**样式映射**:
| Markdown | ratatui Style |
|----------|--------------|
| `**bold**` | `Style::new().bold()` |
| `*italic*` | `Style::new().italic()` |
| `` `code` `` | `Style::new().cyan()` |
| `# H1` | `Style::new().bold().underlined()` |
| `## H2` | `Style::new().bold()` |
| `> quote` | `Style::new().green()` |
| `[link](url)` | `Style::new().cyan().underlined()` |
| ` ```lang ``` ` | syntect 高亮 |

**安全防护**: 代码块超过 512KB 或 10000 行时 fallback 为纯文本。

### D5: OpenAI Chat Client 实现策略

**请求**: `POST {base_url}/v1/chat/completions` with `stream: true, stream_options: { include_usage: true }`

**SSE 解析**: 使用 `eventsource-stream` 将 `reqwest` 的 `bytes_stream()` 转为 SSE 事件流，解析 `data:` 行为 JSON。

**错误处理与重试**: 在 `chat_stream()` 内部实现重试逻辑：
- 429: 指数退避 1s → 2s → 4s，最多 3 次
- 5xx: 重试 2 次，间隔 2s
- 超时: 首 token 60s / 总时长 300s，超时后重试 1 次
- 401/403: 不重试，返回 `LlmError::Auth`
- 流式中断: 返回已收到的内容 + `StreamEvent::Error`

**消息格式转换**: `convert_messages()` 接收 `self_agent_name` 和 `other_agent_role` 参数，按 TDD §3.3.3 处理其他 Agent 的回复 role。Phase 4 只有单 Agent 场景，此参数暂时不生效但预留接口。

### D6: Agent 初始化流程

App 启动时新增 agent 初始化步骤：

```
Config.agents + Config.providers
     ↓
for each AgentConfig:
  1. 查找 ProviderConfig（按 agent.provider 索引）
  2. 读取 API Key（从环境变量 api_key_env 读取）
  3. 根据 provider 类型创建 LlmClient:
     - provider 含 api_key_env 匹配 "OPENAI*" 或明确 openai → OpenAiChatClient
     - 其他 → 暂返回错误（Phase 5 再支持）
  4. 构建 AgentRuntime { config, client, tools: vec![], is_responding: false }
     ↓
HashMap<String, AgentRuntime>  (agent_name → runtime)
```

**Provider 识别**: 不通过名称猜测，而是在 `ProviderConfig` 中新增 `provider_type` 字段（或根据现有 TDD 设计，provider 名称本身就是类型标识）。Phase 4 暂时通过 provider 名称硬匹配 `"openai"` 来识别。

### D7: Commit Tick 动画驱动

流式渲染期间需要周期性 tick 来驱动队列 drain：

- 收到第一个 `TextDelta` 时启动 commit tick（通过 `FrameScheduler` 请求持续重绘）
- 每个 tick（~16ms / 60Hz）执行一次 `AdaptiveChunkingPolicy::decide()` → drain → insert_lines
- 收到 `Done` 事件后，finalize 剩余内容，停止 commit tick

**与现有 FrameScheduler 集成**: 流式期间设置一个 `streaming_active` 标志，FrameScheduler 在此期间以持续 tick 模式运行。

## Risks / Trade-offs

- **[Markdown 重渲染性能]** → 每次 `\n` 都重新解析整个 buffer 可能在超长回复时变慢。缓解：跟踪已提交行数，只返回增量行。极端情况（>10000 行）可考虑截断。

- **[API Key 泄露风险]** → API Key 从环境变量读取并在内存中持有。缓解：不在日志中打印 API Key，错误消息中脱敏处理。

- **[SSE 解析边界情况]** → 部分 Provider 的 SSE 实现可能不完全标准。缓解：对 `data: [DONE]` 等特殊事件做专门处理。

- **[背压阈值调优]** → 直接使用 codex 的阈值参数，可能需要根据实际使用体验调整。缓解：阈值定义为常量，易于调整。

- **[echo Agent 保留]** → 默认配置的 echo agent（provider: "builtin"）在 Phase 4 中无法创建 LlmClient。缓解：echo agent 跳过 LlmClient 初始化，保留 echo 行为作为 fallback。
