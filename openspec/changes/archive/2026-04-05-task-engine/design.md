## Context

当前 `run_agent_loop` 和 `AgentLoopContext` 是 `pub(crate)` 可见性，仅限 `krew-core` 内部使用。每次需要"让 AI 自主执行一段工作"的场景（如 dream、sub_agent），都要手动组装完整的 `AgentLoopContext`，包括设置 channel、构建 tool_defs、处理权限等样板代码。

`run_agent_loop` 本身的设计已经是 `context + messages → side-effects via channel` 的纯函数形态，与会话状态无耦合。只需要在其上加一层薄 wrapper 就能成为通用 TaskEngine。

## Goals / Non-Goals

**Goals:**
- 提供 `krew_core::task` 模块，包含 `TaskRequest` / `TaskResult` 类型和 `run_task()` / `run_task_with_events()` 函数
- 作为底层 loop wrapper：封装 channel 管理、消息构建、结果收集等样板逻辑
- 同步 `await` 模式——调用方等待任务完成后获取结果
- 权限策略完全由调用方决定（透传，不做默认假设）
- 工具暴露策略完全由调用方决定（`tools` + `tool_defs` 分离）

**Non-Goals:**
- 不改变任何现有类型/函数的可见性（`pub(crate)` 不需要提升，`task` 模块在同一 crate 内）
- 不改造现有 dream、sub_agent 或任何其他功能
- 不做后台异步执行或 pending message 通知
- 不做任务注册/管理/取消等生命周期管理
- 不做 agent identity/memory/skill prompt 的自动组装（底层 wrapper 只接受裸 system_prompt）

## Decisions

### D1: TaskEngine 放在 `krew-core::task` 模块

**选择**: 新增 `krew-core/src/task/mod.rs` 和 `krew-core/src/task/types.rs`

**理由**: TaskEngine 是 core 层逻辑——它依赖 `agent_loop`、`LlmClient`、`ToolRegistry` 等 core 类型，且与 `task` 模块同属 krew-core crate，天然可访问 `pub(crate)` 类型。

**替代方案**: 直接让调用方自行组装 `AgentLoopContext`。放弃，因为 channel 管理和结果收集是重复的样板代码。

### D2: agent_loop 内部可见性不变，task 模块对外公开

**选择**: `AgentLoopContext`、`run_agent_loop`、`create_tool_context` 等保持 `pub(crate)` 不变。`task` 模块及其类型/函数使用 `pub` 可见性。

**理由**: agent_loop 内部类型含 `run_agent` 专用的 type-erased 注入，不适合作为公共抽象，保持 `pub(crate)`。`task` 模块使用 `pub` 是因为集成测试（`tests/task_test.rs`）需要从 crate 外部访问，同时也自然消除了 dead_code 警告。

### D3: 权限策略由调用方显式传入

**选择**: `TaskRequest` 包含 `approval_mode`、`approval_cache`、`allow_rules`、`deny_rules`、`ask_rules`，由调用方决定权限策略。

**理由**: 底层 wrapper 不应做策略假设。不同场景需要不同策略——系统内部任务可能用 FullAuto，而未来如果 sub_agent 迁移过来则需要继承父 agent 权限。由调用方显式选择，避免隐式绕过用户配置的权限策略。

**替代方案**: 硬编码 FullAuto + 空规则。放弃，因为会在未来迁移其他场景时静默绕过权限约束。

### D4: `tools` 和 `tool_defs` 分离

**选择**: `TaskRequest` 同时接收 `tools: Arc<ToolRegistry>`（dispatch 执行用）和 `tool_defs: Vec<ToolDefinition>`（传给 LLM 的工具定义）。

**理由**: 与 `AgentLoopContext` 保持一致。调用方完全控制暴露给 LLM 的工具集——可以从 registry 中过滤、排除嵌套工具、或只暴露白名单。底层 wrapper 不猜测调用方的意图。

**替代方案 (a)**: 只收 `tools`，由 wrapper 全量转成 `tool_defs`。放弃，因为无法覆盖 dream（白名单过滤）和 sub_agent（排除 run_agent 自身）等场景。

**替代方案 (b)**: 加 `exclude_tools: Vec<String>` 参数。放弃，因为这是中间层抽象——调用方知道自己要什么，不需要 wrapper 代劳过滤。

### D5: 事件通道可选

**选择**: `run_task()` 返回 `TaskResult`（同步 await，内部消费 channel）。`run_task_with_events()` 额外返回 `Receiver<AgentEvent>`。

**理由**: 大多数调用方只关心最终结果。需要进度监听时用 `_with_events` 变体。

### D6: `on_retry` 回调默认 noop

**选择**: wrapper 内部使用空回调 `|_| {}`。调用方可通过事件通道中的 `AgentEvent::Retrying` 获取重试信息。

## Risks / Trade-offs

- **[底层定位限制]** → 作为纯底层 wrapper，TaskEngine 不自动组装 identity/memory/skill prompt，调用方需要自行构建 system_prompt。缓解：这是有意的设计——底层只做 loop 封装，上层 builder 可在未来按需添加。
- **[调用方责任]** → 权限策略和工具暴露都由调用方负责，用错了 wrapper 不兜底。缓解：这比隐式绕过权限更安全，且与现有 `AgentLoopContext` 的使用方式一致。
