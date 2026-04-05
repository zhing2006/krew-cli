## ADDED Requirements

### Requirement: TaskRequest 输入类型
`krew_core::task` 模块 SHALL 定义 `TaskRequest` 结构体，包含以下字段：
- `prompt: String` — 发送给 LLM 的用户提示
- `system_prompt: Option<String>` — 可选的裸 system prompt（不含 identity/memory/skill 等自动组装）
- `client: Arc<dyn LlmClient>` — LLM 客户端
- `tools: Arc<ToolRegistry>` — 工具注册表（用于 dispatch 执行）
- `tool_defs: Vec<ToolDefinition>` — 暴露给 LLM 的工具定义列表（可以是 registry 的子集）
- `sampling: SamplingConfig` — 采样参数
- `max_rounds: u32` — 最大工具调用轮次
- `agent_name: String` — 消息中的 agent 名称标识
- `approval_mode: ApprovalMode` — 工具审批策略
- `approval_cache: ApprovalCache` — 审批缓存
- `allow_rules: Vec<PermissionRule>` — 自动批准规则
- `deny_rules: Vec<PermissionRule>` — 自动拒绝规则
- `ask_rules: Vec<PermissionRule>` — 强制确认规则
- `cwd: String` — 工作目录（用于权限规则路径解析）

#### Scenario: 构造 TaskRequest
- **WHEN** 调用方构造 `TaskRequest`
- **THEN** 所有字段 SHALL 由调用方显式提供，无隐式默认值

### Requirement: TaskResult 输出类型
`krew_core::task` 模块 SHALL 定义 `TaskResult` 结构体，包含以下字段：
- `final_text: String` — LLM 最终文本响应
- `messages: Vec<ChatMessage>` — 完整对话历史（含中间 tool call 消息和最终 assistant 纯文本消息）
- `usage: Usage` — 累计 token 用量
- `is_error: bool` — 是否以错误终止

#### Scenario: 成功完成
- **WHEN** agent loop 正常完成（LLM 返回无 tool call 的最终响应）
- **THEN** `TaskResult::is_error` SHALL 为 `false`，`final_text` SHALL 包含最终文本，`messages` 的最后一条 SHALL 为包含 `final_text` 的 `ChatRole::Assistant` 消息

#### Scenario: 错误终止
- **WHEN** agent loop 因 LLM 错误或超过 max_rounds 终止
- **THEN** `TaskResult::is_error` SHALL 为 `true`，`final_text` SHALL 包含错误信息

### Requirement: run_task 同步执行
`krew_core::task` 模块 SHALL 提供 `pub async fn run_task(req: TaskRequest) -> TaskResult` 函数。该函数 SHALL：
1. 构建独立的消息列表（可选 `system_prompt` + `user(prompt)`）
2. 使用 `TaskRequest` 提供的 `tool_defs` 直接传给 `AgentLoopContext`（不做额外过滤）
3. 使用 `TaskRequest` 提供的权限配置构建 `AgentLoopContext`
4. 内部创建 `mpsc::unbounded_channel`，spawn 事件消费者 task 并发处理 channel 事件
5. await `run_agent_loop()` 完成后 drop tx，等待消费者收集结果
6. 消费者从 `Done` / `Error` 事件中提取 `final_text`、`intermediate_messages`、`usage`
7. 消费者将 `final_text` 补充为最终 `ChatRole::Assistant` 消息追加到 messages 末尾
8. 组装为 `TaskResult` 返回

#### Scenario: 基本调用流程
- **WHEN** 调用 `run_task(req)` 且 LLM 返回纯文本响应
- **THEN** SHALL 返回 `TaskResult { is_error: false, final_text: "...", messages: [...], usage: ... }`

#### Scenario: 带 system_prompt
- **WHEN** `TaskRequest::system_prompt` 为 `Some("你是助手")`
- **THEN** 消息列表 SHALL 以 `ChatMessage::text(ChatRole::System, "你是助手")` 开头

#### Scenario: 无 system_prompt
- **WHEN** `TaskRequest::system_prompt` 为 `None`
- **THEN** 消息列表 SHALL 仅包含 `ChatMessage::text(ChatRole::User, prompt)`

#### Scenario: FullAuto 模式下工具自动批准
- **WHEN** `TaskRequest::approval_mode` 为 `FullAuto` 且无匹配的 `ask_rules`
- **THEN** 所有工具调用 SHALL 自动批准，不产生 `ApprovalRequest`

#### Scenario: ApprovalRequest 自动拒绝
- **WHEN** 调用方配置导致 `run_agent_loop` 发出 `ApprovalRequest` 事件
- **THEN** `run_task()` 的内部消费者 SHALL 自动回复 `ReviewDecision::Denied`，避免死锁。agent loop 将收到拒绝结果并继续执行

### Requirement: run_task_with_events 带事件通道执行
`krew_core::task` 模块 SHALL 提供 `pub fn run_task_with_events(req: TaskRequest) -> (impl Future<Output = TaskResult>, mpsc::UnboundedReceiver<AgentEvent>)` 函数。

该函数 SHALL 创建两个独立的 channel：
- 内部 channel：传给 `run_agent_loop`，接收原始 `AgentEvent`
- 外部 channel：返回给调用方，转发所有事件（包括 `ApprovalRequest`）

调用方收到 `ApprovalRequest` 时，SHALL 通过其 `respond` oneshot 回复审批决定。

#### Scenario: 事件流监听
- **WHEN** 调用 `run_task_with_events()` 并监听事件通道
- **THEN** SHALL 接收到 `TextDelta`、`ToolCallStart`、`ToolCallDone`、`Done`、`ApprovalRequest` 等事件（不含 `ResponseStart`，该事件由上层 `AgentRuntime` 发送，不在 `run_agent_loop` 范围内）

#### Scenario: 调用方处理审批
- **WHEN** `run_task_with_events()` 的事件通道收到 `ApprovalRequest`
- **THEN** 调用方 SHALL 通过 `respond` oneshot 发送 `ReviewDecision`，agent loop 将据此继续执行

#### Scenario: 外部 receiver 被 drop 导致审批自动拒绝
- **WHEN** 调用 `run_task_with_events()` 且外部 `receiver` 被 drop（不再持有），配置产生了 `ApprovalRequest`
- **THEN** 内部 consumer 的 `send()` 失败，`ApprovalRequest` 中的 `respond` sender 被 drop，agent loop 侧的 oneshot receiver 返回默认值 `ReviewDecision::Denied`

#### Scenario: 外部 receiver 持有但未消费审批事件
- **WHEN** 调用 `run_task_with_events()` 且调用方持有 `receiver` 但不读取 `ApprovalRequest` 事件
- **THEN** `ApprovalRequest` 停留在无界队列中，`respond` sender 不会 drop，agent loop SHALL 阻塞等待审批响应。调用方有责任消费审批事件或 drop receiver
