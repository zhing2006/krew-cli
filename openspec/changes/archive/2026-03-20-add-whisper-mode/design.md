## Context

krew-cli 目前将所有消息广播给每个 agent——没有用户与特定 agent（或 agent 子集）之间私密通信的机制。现有的 `@agent` 寻址控制的是*谁来回复*，但每个 agent 仍然能看到完整的对话历史。本次变更引入 `#agent` 密语语法，创建基于可见性范围的消息组。

核心路由基础设施（`parse_input`、`Addressee`、`resolve_dispatch_queue`）和串行 agent 执行模型已经成熟。密语功能在此之上叠加，不替换任何现有行为。

## Goals / Non-Goals

**Goals:**
- 允许用户使用 `#agent` 语法与一个或多个 agent 进行私密对话
- 确保组外 agent 仅看到占位符文本
- 支持组内 A2A 路由（密语组内的 agent 可以互相 `@mention`）
- 跨 session save/resume 持久化密语元数据
- TUI 和 prompt 模式行为一致

**Non-Goals:**
- Agent 发起的密语（仅用户可以发起密语对话）
- 嵌套密语组（密语中的密语）
- 加密或真正安全的密语存储（占位符提供的是 LLM 级别的隐私，非密码学级别）
- 密语功能的开关配置（始终可用）

## Decisions

### 1. `ChatMessage` 上使用 `Option<Vec<String>>` 存储密语目标

**决策**：在 `ChatMessage` 上添加 `whisper_targets: Option<Vec<String>>`，在 `MessageEntry` 中序列化为 TOML 原生数组 `Option<Vec<String>>`。

**备选方案**：
- `Option<String>`（仅单目标）——更简单但无法支持密语组
- `Option<HashSet<String>>`——无序，对小列表而言不必要的复杂
- 独立的 `WhisperGroup` 结构体——对本质上只是名字列表的东西过度设计
- 逗号分隔字符串（如 `"opus,gemini"`）——与现有 `addressee` 字段模式一致，但存在编码歧义：agent 名称未禁止包含逗号，反序列化时会误拆

**理由**：`Vec<String>` 是同时支持单目标和多目标密语的最小结构。使用 TOML 原生数组避免了逗号分隔的编码歧义问题，且 `whisper_targets` 是新字段，无向后兼容负担。

### 2. 在 `parse_input` 中与 `@` 共享解析

**决策**：扩展 `parse_input` 在单次遍历中同时扫描 `@` 和 `#` 前缀。返回类型从 `(Addressee, String)` 变为 `(Addressee, String, bool)`，第三个元素为 `is_whisper`。

**备选方案**：
- 独立的 `parse_whisper_input` 函数——复制所有扫描逻辑
- 新的 `Addressee` 变体如 `WhisperSingle(String)`——将路由关注点和可见性关注点混在一起

**理由**：密语是一个正交的可见性标志，不是路由模式。同一个 `Addressee` 枚举处理调度；`is_whisper` 仅影响消息标记和过滤。独立函数会复制整个 token 扫描循环。

### 3. 在 `prepare_messages_for_agent` 中过滤

**决策**：在 `prepare_messages_for_agent` 中添加密语过滤步骤。当消息设置了 `whisper_targets` 且 `self_name` 不在目标列表中时，替换为单个占位符 `ChatMessage`，如 `[Whisper to opus, gemini]`。

**理由**：这是唯一一个按 agent 产生不同消息视图的地方——它已经将其他 agent 的工具调用链转为文本。在这里添加密语过滤保持关注点集中。

### 4. 通过 `App` 字段传播密语状态

**决策**：在 `App` 状态中添加 `current_whisper_targets: Option<Vec<String>>`。在调度密语时设置，由 agent loop 消费以标记所有产出的消息。在密语调度队列完成时清除。

**备选方案**：
- 通过 `start_completion` 参数传递 whisper_targets——需要穿过多层
- 存储在 `AgentRuntime` 上——可变共享状态，更难管理

**理由**：`App` 已经拥有调度生命周期（`pending_agents`、`a2a_insert_cursor`）。增加一个字段使密语状态与调度状态共存。

### 5. 通过 mention 过滤实现组内 A2A

**决策**：处理密语回复的 A2A mention 时，将 `parse_agent_mentions` 的结果过滤为仅包含 `whisper_targets` 中的 agent。被 A2A 调度的 agent 继承相同的 `whisper_targets`。

**理由**：A2A 路由机制（`apply_immediate_routing_at`、`apply_queued_routing`、`agent_to_agent_max_rounds`）无需修改。仅 mention 列表被过滤——最小代码增量。

### 6. System prompt 中的密语上下文（两层分离）

**决策**：密语的 system prompt 注入分为两个独立层，各有独立的触发条件：

1. **隐私上下文层**（`whisper_targets.is_some()` 时始终注入）：
   - 告知 agent 当前处于私密密语对话
   - 列出密语组内其他 agent（如果有）
   - 说明组外 agent 无法看到对话内容

2. **@mention 协作层**（`whisper_targets.is_some() && agent_to_agent_max_rounds > 0` 时注入）：
   - 说明仅可 `@mention` 组内成员
   - 组外 agent 的 mention 将被忽略

**理由**：隐私上下文和 A2A 协作是正交的关注点。当 A2A 关闭（`max_rounds = 0`）但 whisper 开启时，agent 仍需知道这是私密对话（影响其回复风格和内容），但不应被指引使用 `@mention`（因为 A2A 根本不可用）。

### 7. Compact 提取密语消息（与 skill 消息相同模式）

**决策**：在 `compact.rs` 中，从压缩区提取密语消息并在 summary 之后重新插入，遵循现有的 `extract_skill_messages` 模式。最终消息顺序为：`[Summary] + [skill messages] + [whisper messages] + [kept rounds]`。

**备选方案**：
- "原位保留"——与当前模型不兼容，因为压缩区被替换为单个 summary
- "密语轮次不计入 keep_rounds"——将密语推入保留区，但密语频繁时降低压缩效果
- "仅压缩到第一条密语之前"——太保守，如果密语出现得早几乎无法压缩

**理由**：提取-重插入模式已被 skill 消息保留所验证。密语消息失去了相对于被压缩的普通消息的位置上下文，但其内容和 `whisper_targets` 元数据完整保留——这才是可见性过滤所需要的。压缩 LLM 永远不会看到密语内容，维持隐私性。

### 8. `#all` 在解析时拒绝

**决策**：`parse_input` 检测到 `#all` 时返回错误。

**理由**：对所有 agent 密语在语义上等同于普通消息。提前拒绝避免用户困惑。

### 9. `LastRespondent` 不继承密语

**决策**：不带 `#` 前缀的后续消息始终使用普通（非密语）模式，无论上一个回复者是否在密语对话中。

**理由**：隐式密语继承会令人意外。用户每次都必须使用 `#` 显式选择密语。

### 10. Error / Cancel 路径的密语生命周期

**决策**：`current_whisper_targets` 的清理覆盖三条路径：
- **Done**：pending 队列为空且无密语 A2A 时清除
- **Error**：合成的 `[Error: ...]` 消息继承 `whisper_targets`；若 pending 队列为空则清除状态，否则保留状态继续调度下一个 agent
- **Cancel (ESC)**：合成的 `[Cancelled by user]` 消息继承 `whisper_targets`；清空 pending 队列后清除状态

**理由**：现有 TUI 的 Error 和 Cancel 路径都会合成 assistant `ChatMessage` 并持久化。如果这些消息不带 `whisper_targets`，组外 agent 在后续轮次中会直接看到密语内容，破坏隐私边界。P 模式的 Error 路径同理。

### 11. 占位符消息的结构语义

**决策**：占位符消息保留原消息的 `role` 和 `name`（assistant 占位符保留 agent name），仅替换 `content`。占位符的 `whisper_targets` 设为 `None`，`tool_calls` 和 `tool_call_id` 设为 `None`。密语工具调用链（assistant+tool_calls + 后续 Tool results）折叠为单个 Assistant 占位符。

**理由**：保留 `role` 确保 User/Assistant 交替不被打破（多数 LLM API 要求严格交替）。保留 assistant 的 `name` 让组外 agent 知道是谁在密语。`whisper_targets` 设为 `None` 因为占位符本身不需要再被过滤——它已经是过滤后的产物。

### 12. Resume 重放时的密语显示

**决策**：`--resume` 恢复会话时，重放逻辑根据 `MessageEntry` 的 `whisper_targets` 字段决定是否显示锁图标。用户消息根据 `whisper_targets` 解析目标 agent 列表并渲染彩色圆点+锁图标。Agent 消息根据 `whisper_targets` 在 header 中附加锁图标。

**理由**：现有重放路径对用户消息使用 `insert_user_message(terminal, &[], &msg.content)` 不传入目标信息，对 agent header 不传入密语标志。需要扩展重放逻辑读取 `whisper_targets` 字段。

## Risks / Trade-offs

- **[风险] 密语消息膨胀组外 agent 的上下文** → 缓解：占位符文本很小（每个密语对约 10 tokens）。对于大量密语的会话，`/compact` 会保留它们但它们很短。

- **[风险] 用户可能混淆 `@` 和 `#` 语法** → 缓解：TUI 为密语消息显示独特的锁图标，使模式在视觉上一目了然。错误消息清楚地解释区别。

- **[风险] 密语模式下的工具调用结果可能泄露信息** → 缓解：密语 agent loop 期间产出的所有消息（包括 tool_calls 和 tool results）都继承 `whisper_targets`，因此组外 agent 仅看到整个交换的占位符。

- **[权衡] 密语消息不参与 compact 压缩** → 已接受：这意味着很长的密语对话可能会累积。实际上密语交换是短的、有针对性的讨论。如果需要，用户可以开始新会话。
