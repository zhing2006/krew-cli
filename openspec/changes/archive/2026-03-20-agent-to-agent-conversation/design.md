## Context

krew-cli 当前的消息路由完全由用户驱动：用户输入 → `parse_input()` 解析 @ 寻址 → `resolve_dispatch_queue()` 构建 `pending_agents` 队列 → `start_next_agent()` 串行执行。Agent 完成后通过 `AgentEvent::Done` 链式触发下一个。

AI-to-AI 对话需要在这个流程中插入一个新的分支：Agent 回复完成后，检测回复文本中的 @，如果命中已知 Agent，则自动将其加入调度队列。

关键代码位置：
- `krew-core/router.rs` — @ 寻址解析
- `krew-core/agent/mod.rs` — system prompt 构建、`start_completion()`
- `krew-cli/app/state.rs` — `handle_agent_event(Done)` 消息处理与队列调度
- `krew-config/src/lib.rs` — 配置数据结构

## Goals / Non-Goals

**目标：**
- Agent 回复中的 `@agent_name` 自动触发目标 Agent 回复
- 支持 `immediate`（头部）和 `queued`（尾部）两种路由策略
- 轮次限制防止无限循环
- 复用现有 `pending_agents` 队列和 `start_next_agent()` 调度机制，最小化改动
- Agent 通过 system prompt 感知可 @ 的 Agent 列表

**非目标：**
- 不支持 Agent @ 用户（触发用户输入）
- 不支持 AI-to-AI 期间用户发送新消息（用户可在输入框编辑但不可提交，ESC 可中断）
- 不引入并行 AI-to-AI 对话（仍为串行）
- 不修改消息 role 模型（其他 Agent 的消息 role 行为保持现状）

## Decisions

### 1. 路由检测位置：`handle_agent_event(Done)` 中

**选择**：在 `state.rs` 的 `AgentEvent::Done` 处理分支中，对 `final_text` 执行 @ 解析。

**理由**：这是 Agent 完成回复后、调用 `start_next_agent()` 之前的唯一汇合点。在此处插入逻辑，不需要修改 `start_next_agent()`、`AgentRuntime` 或 agent loop 本身。

**备选**：在 `run_agent_loop` 内部检测 → 会耦合核心循环，且需要传入 agent 列表等额外上下文。

### 2. 解析函数复用 `parse_input()` 逻辑

**选择**：新增 `parse_agent_mentions(text, known_agents) -> Vec<String>` 函数，提取文本中所有 `@agent_name` 匹配。与 `parse_input()` 共享相同的 token 扫描逻辑，但不返回 `Addressee` 枚举，仅返回匹配到的 agent name 列表。

**理由**：Agent 回复的 @ 语义与用户输入不同——不需要 `@all` 支持（Agent 不应广播），不需要 `LastRespondent` 回退。只需要提取具体 agent name。排除 @ 自己，避免自我触发。

**"已知 Agent"定义**：仅包含当前会话中已成功初始化且持有 runtime 的 Agent（即 `self.agents` 键集），不包含因 API Key 缺失等原因初始化失败的 Agent。这与 system prompt 注入的 Agent 列表保持一致——Agent 只会看到且只能 @ 实际可用的 peer。

**前缀匹配**：LLM 回复中的 `@name` 后常紧跟标点（如 `@opus,`）或 CJK 文本（如 `@助手，你觉得呢`）。`parse_agent_mentions` 在按空白分词后，对每个以 `@` 开头的 token 使用前缀匹配：检查已知 Agent name 是否是 token（去掉 `@` 后）的前缀，且前缀后的下一个字符为非字母数字字符或 token 结尾。这同时支持 ASCII 和非 ASCII agent name，避免了 ASCII-only 裁剪导致的兼容性问题。

### 3. 两种路由策略通过配置切换

**选择**：`agent_to_agent_routing` 配置项，枚举值 `immediate` | `queued`。

- **immediate**（默认）：目标已在队列 → 移到头部；不在队列 → 插入头部。被 @ 的 Agent 立刻接话。
- **queued**：目标已在队列 → 不动；不在队列 → 追加尾部。尊重原始队列顺序。

**理由**：两种策略各有优劣（自然对话流 vs 无饥饿保证），提供配置让用户选择。

### 4. 轮次计数器与终止

**选择**：App state 新增 `ai_conversation_rounds: u32` 计数器。每次 AI-to-AI @ 触发时递增。超过 `agent_to_agent_max_rounds` 时停止插入，显示提示，让队列中剩余 Agent 正常执行。

**重置时机**：下一次用户发送消息时重置为 0。

**理由**：简单有效。`max_rounds = 0` 时整个功能禁用，Agent 回复中的 @ 不触发路由，与当前行为完全一致。

### 5. System prompt 注入 Agent 列表

**选择**：在 `AgentRuntime::start_completion()` 的 identity prompt 中追加可 @ 的其他 Agent 列表。

**注入内容**（追加到现有 identity 段落之后）：

```
To collaborate with other agents, mention them with @name in your response.
Other agents: [opus] Claude Opus, [gemini] Gemini 3.1 Pro.
```

**仅在 `agent_to_agent_max_rounds > 0` 时注入**，功能禁用时不改变现有 prompt。

### 6. 多个 @ 的处理

**选择**：如果 Agent 回复中 @ 了多个 Agent，取第一个匹配的（按文本出现顺序）。不支持一次 @ 多个目标进行扇出。

**理由**：简化实现，避免复杂的多目标扇出调度。Agent 可以通过多轮对话逐一 @ 不同 Agent。

## Risks / Trade-offs

- **[immediate 模式下的队列饥饿]** → `agent_to_agent_max_rounds` 兜底，超限后停止插入，剩余 Agent 正常执行。默认 10 轮足够对话收敛。
- **[Agent 滥用 @]** → system prompt 引导仅在需要协作时使用 @。最坏情况由 max_rounds 限制。
- **[回复文本中的误匹配]** → 代码块或引用中的 `@name` 可能被误检测。初版不做特殊处理，因为 Agent 在代码块中写 `@agent_name` 的概率极低。后续可按需加过滤。
