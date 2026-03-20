## 1. 数据模型

- [x] 1.1 在 `krew-llm/src/lib.rs` 的 `ChatMessage` 中添加 `whisper_targets: Option<Vec<String>>` 字段；添加构造辅助方法（如 `user_with_whisper`、`with_whisper_targets`）
- [x] 1.2 在 `krew-storage/src/session_file.rs` 的 `MessageEntry` 中添加 `whisper_targets: Option<Vec<String>>` 字段（TOML 原生数组，`skip_serializing_if`、`default`）
- [x] 1.3 更新 `krew-core/src/persistence.rs` 中的 `build_session_file` / `restore_messages`，直接映射 `Vec<String>`（无需格式转换，类型一致）

## 2. 配置校验

- [x] 2.1 在 `krew-config` 的 `validate()` 中添加 agent 名称校验：禁止 `"all"` 作为 agent 名称（保留字），配置包含时报错

## 3. 输入解析

- [x] 3.1 扩展 `krew-core/src/router.rs` 中的 `parse_input()`，在扫描 `@name` 的同时扫描 `#name` token；返回 `(Addressee, String, bool)`，第三个元素为 `is_whisper`
- [x] 3.2 在 `parse_input` 中添加 `#all` 拒绝（返回错误）和 `#`/`@` 混用拒绝
- [x] 3.3 更新 `parse_input` 的所有调用点以处理新的三元组返回值：`app/message.rs`（send_message、send_expanded_text）、`prompt_mode/mod.rs`、`custom_command/mod.rs` 及测试文件
- [x] 3.4 添加 `#` 解析的单元测试：单目标、多目标、`#all` 错误、`#`/`@` 混用错误、未知 `#name` 当作普通文本

## 4. 消息可见性过滤

- [x] 4.1 在 `krew-core/src/agent/prepare.rs` 的 `prepare_messages_for_agent()` 中添加密语过滤步骤：当 `whisper_targets.is_some() && !whisper_targets.contains(self_name)` 时替换为占位符
- [x] 4.2 占位符结构：保留原消息的 `role` 和 `name`，替换 `content`（User → `[Whisper to targets]`，Assistant → `[Whisper]`），`whisper_targets`/`tool_calls`/`tool_call_id` 设为 `None`
- [x] 4.3 密语工具调用链（assistant+tool_calls + Tool results）折叠为单个 Assistant 占位符（对组外 agent）
- [x] 4.4 添加密语过滤的单元测试：组内成员看到内容、组外成员看到占位符、工具调用链正确过滤、占位符 role/name 保持正确

## 5. 密语状态与调度

- [x] 5.1 在 `krew-cli/src/app/state.rs` 的 `App` 结构体中添加 `current_whisper_targets: Option<Vec<String>>` 字段
- [x] 5.2 在 `send_message()` 和 `send_expanded_text()` 中：当 `is_whisper = true` 时设置 `current_whisper_targets`；设置用户 ChatMessage 的 `whisper_targets`
- [x] 5.3 在 `start_next_agent()` 中：将 `current_whisper_targets` 传递给 `start_completion`，使 agent loop 能标记产出的消息
- [x] 5.4 扩展 `AgentRuntime::start_completion()` 以接受可选的 `whisper_targets` 参数；传播到 agent loop context
- [x] 5.5 在 `agent_loop.rs` 中：用 context 中的 `whisper_targets` 标记所有产出的消息（assistant 文本、tool_calls、tool results）
- [x] 5.6 在 `handle_agent_event(Done)` 中：当 pending 队列为空且无密语 A2A 等待时，清除 `current_whisper_targets`
- [x] 5.7 在 `handle_agent_event(Error)` 中：合成的 `[Error: ...]` 消息继承 `current_whisper_targets`；pending 队列为空时清除状态
- [x] 5.8 在 `cancel_agent_response()` 中：合成的 `[Cancelled by user]` 消息继承 `current_whisper_targets`；清空 pending 队列后清除状态

## 6. 密语模式下的 A2A 路由

- [x] 6.1 在 TUI `handle_agent_event(Done)` 的 A2A 段中：当 `current_whisper_targets.is_some()` 时，将 `parse_agent_mentions` 结果过滤为仅 `whisper_targets` 成员
- [x] 6.2 在 prompt 模式的 A2A 段中：相同的过滤逻辑
- [x] 6.3 确保 A2A 调度的 agent 继承 `current_whisper_targets`

## 7. System Prompt

- [x] 7.1 在 `AgentRuntime::start_completion()` 中：当 `whisper_targets` 已设置时，始终注入隐私上下文层（私密对话通知、组外 agent 不可见）
- [x] 7.2 当 `whisper_targets` 已设置且 `agent_to_agent_max_rounds > 0` 时，追加 @mention 协作层（组内成员列表、限制 mention 组外 agent）
- [x] 7.3 当密语组仅有一个成员（单目标）时，省略 @mention 协作层（无组内 peer 可列出）

## 8. TUI 显示

- [x] 8.1 在 `insert_user_message()` 中：接受 `is_whisper` 参数，密语模式下在彩色圆点前添加锁图标（🔒）
- [x] 8.2 在 agent 响应 header 渲染中：当响应有 `whisper_targets` 时，在 agent 显示名旁附加锁图标
- [x] 8.3 更新 `ResponseStart` 事件或 App 状态以携带 whisper 标志用于 header 渲染
- [x] 8.4 Resume 重放：更新 `commands.rs` 中的重放逻辑，从 `MessageEntry.whisper_targets` 读取密语标志，用户消息传入目标 agent 列表和 `is_whisper`，agent header 传入密语标志

## 9. Compact

- [x] 9.1 在 `compact.rs` 中添加 `extract_whisper_messages()` 函数（与 `extract_skill_messages()` 相同模式），从压缩区提取密语消息块
- [x] 9.2 在 `build_compacted_messages()` 中：将提取的密语消息插入到 skill 消息之后、kept rounds 之前；从 `messages_to_text()` 压缩输入中排除密语消息

## 10. Prompt 模式

- [x] 10.1 在 `run_prompt_mode()` 中：处理 `parse_input` 返回的 `is_whisper`，在用户消息上设置 whisper_targets，跟踪 `current_whisper_targets` 状态
- [x] 10.2 在 `consume_agent_events` / agent header 输出中：text 格式显示 `[whisper]` 标记；JSON 格式包含 `whisper_targets` 字段
- [x] 10.3 在 prompt 模式中拒绝 `#all`（已由 parse_input 处理，验证错误消息和 exit code 2）
- [x] 10.4 P 模式 Error 路径：合成的部分文本消息继承 `current_whisper_targets`

## 11. 集成测试

- [x] 11.1 添加 `#` 解析结合 `resolve_dispatch_queue` 的 router 测试
- [x] 11.2 添加混合普通消息和密语消息的 prepare_messages 过滤测试（含占位符结构验证）
- [x] 11.3 添加持久化往返测试：保存带 whisper_targets 的 session，加载并验证
- [x] 11.4 添加密语工具调用链折叠为单个占位符的测试
- [x] 11.5 添加 agent 名称 `"all"` 被 config 校验拒绝的测试
