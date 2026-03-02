## 1. 清理 use_name_field

- [x] 1.1 从 `ProviderConfig`（krew-config/src/lib.rs）中删除 `use_name_field: bool` 字段
- [x] 1.2 在 `Settings` 中新增 `other_agent_role: OtherAgentRole` 字段（默认 User）；在 krew-config 中定义 `OtherAgentRole` 枚举（User, Assistant），派生 Deserialize/Clone/Debug，并从 lib.rs 公开导出
- [x] 1.3 从 `AgentRuntime`（krew-core/src/agent.rs）中删除 `use_name_field: bool` 字段；新增 `other_agent_role: OtherAgentRole` 字段
- [x] 1.4 修改 `AgentRuntime::start_completion()` 中的 system prompt hint，移除 `use_name_field` 条件分支，固定为 "Their messages are prefixed with [agent_name] in the content."
- [x] 1.5 修改 `init_agents()`（krew-cli/src/app/state.rs）：删除 `use_name_field` 传递，改为传递 `other_agent_role`（从 `config.settings.other_agent_role` 获取）

## 2. Provider convert_messages 统一 OtherAgentRole

- [x] 2.1 修改 `openai_chat::convert_messages`：删除 `use_name_field` 参数；统一使用 `[agent_name]` 前缀；添加 `merge_consecutive_same_role` 调用
- [x] 2.2 修改 `anthropic::convert_messages`：签名增加 `other_agent_role: &OtherAgentRole` 参数，将硬编码的 `"user"` 替换为根据参数决定 role
- [x] 2.3 修改 `google::convert_messages`：签名增加 `other_agent_role: &OtherAgentRole` 参数，将硬编码的 `"user"` 替换为根据参数决定 role（注意 Google 用 `"model"` 而非 `"assistant"`）
- [x] 2.4 修改 `openai_responses::convert_messages`：签名增加 `other_agent_role: &OtherAgentRole` 参数，将硬编码的 `"user"` 替换为根据参数决定 role
- [x] 2.5 修改各 LLM client struct，将 `OtherAgentRole` 作为字段持有，在 `chat_stream()` 中传递给 `convert_messages`
- [x] 2.6 更新所有 `convert_messages` 相关的单元测试，覆盖 `OtherAgentRole::User` 和 `OtherAgentRole::Assistant` 两种场景

## 3. 多 Agent 串行调度

- [x] 3.1 在 App struct（krew-cli/src/app/state.rs）中新增 `pending_agents: VecDeque<String>` 和 `last_respondent: Option<String>` 字段
- [x] 3.2 修改 `send_message()`（krew-cli/src/app/message.rs）中的路由逻辑：@all 按 reply_order 填充队列；@multiple 按 @ 出现顺序填充队列；@single 直接启动；LastRespondent 使用 last_respondent 或提示用户
- [x] 3.3 修改 `handle_agent_event(Done)`：追加回复到 messages 后检查 `pending_agents`，有则弹出下一个并启动 `start_completion`；更新 `last_respondent`
- [x] 3.4 修改 `handle_agent_event(Error)`：显示错误后检查 `pending_agents`，有则继续下一个；不更新 `last_respondent`
- [x] 3.5 在 `send_message()` 中为 LastRespondent 无值场景添加错误提示，阻止消息发送

## 4. 测试与验证

- [x] 4.1 运行 `cargo fmt --all` 和 `cargo clippy --all-targets --all-features -- -D warnings` 确保代码规范
- [x] 4.2 运行 `cargo test` 确保所有现有测试通过（253 tests passed）
- [x] 4.3 手动测试：启动 krew，配置多个 Agent，验证 @all、@name1 @name2、无前缀延续、错误隔离等场景
