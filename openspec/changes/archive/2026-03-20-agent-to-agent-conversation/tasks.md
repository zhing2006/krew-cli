## 1. 配置

- [x] 1.1 在 `krew-config/src/lib.rs` 的 `Settings` 中新增 `agent_to_agent_routing`（枚举：`immediate` | `queued`，默认 `immediate`）和 `agent_to_agent_max_rounds`（u32，默认 10）字段
- [x] 1.2 在 `krew-config` 中新增 `AgentToAgentRouting` 枚举（`Immediate`, `Queued`），实现 `Default` trait 和 serde 支持

## 2. Agent 回复 @ 检测

- [x] 2.1 在 `krew-core/router.rs` 中新增 `parse_agent_mentions(text: &str, known_agents: &[String], self_name: &str) -> Vec<String>` 函数——按空白分词扫描 `@agent_name` token，剥离尾部标点后匹配已初始化 Agent（`self.agents` 键集），排除自身和 `@all`，按文本出现顺序返回
- [x] 2.2 在 `krew-core/tests/router_test.rs` 中为 `parse_agent_mentions` 添加测试：基本匹配、自身排除、未知 Agent 忽略、`@all` 忽略、多个匹配按顺序返回、无匹配返回空、尾部标点剥离（`@opus,` `@opus：`）

## 3. 队列操作

- [x] 3.1 在 `krew-core/router.rs` 中新增队列操作辅助函数：`apply_immediate_routing(pending: &mut VecDeque<String>, target: &str)` —— 已在队列则移到头部，不在则插入头部；`apply_queued_routing(pending: &mut VecDeque<String>, target: &str)` —— 不在队列则追加尾部，已在则不变
- [x] 3.2 在 `krew-core/tests/router_test.rs` 中为两个路由辅助函数添加测试：immediate 模式下已在队列移到头部、不在队列插入头部、已在头部不变；queued 模式下已在队列不变、不在队列追加尾部

## 4. Done 处理器核心集成

- [x] 4.1 在 `App` state 中新增 `ai_conversation_rounds: u32` 字段，初始化为 0
- [x] 4.2 在 `state.rs` 的 `handle_agent_event(AgentEvent::Done)` 中：持久化消息之后、`start_next_agent()` 之前，对 `final_text` 调用 `parse_agent_mentions` 并传入已知 Agent 列表和当前 Agent 名称
- [x] 4.3 如果检测到目标且 `agent_to_agent_max_rounds > 0` 且 `ai_conversation_rounds < max`：递增计数器，按配置的路由策略操作 `pending_agents`，然后执行 `start_next_agent()`
- [x] 4.4 如果超限：显示 TUI 提示信息（"AI-to-AI 对话轮次已达上限"），跳过路由，让 `start_next_agent()` 正常消耗剩余队列
- [x] 4.5 在 `send_message()` 和 `send_expanded_text()` 中将 `ai_conversation_rounds` 重置为 0

## 5. System Prompt 注入

- [x] 5.1 在 `AgentRuntime::start_completion()`（`krew-core/agent/mod.rs`）中：当 `agent_to_agent_max_rounds > 0` 时，在 identity prompt 中追加当前会话中其他已初始化 Agent 的列表和 @ 用法说明。格式：`[name] display_name`，说明可以 @ 除自己以外的任何 Agent
- [x] 5.2 将 `agent_to_agent_max_rounds` 和已初始化 Agent 列表（`self.agents` 键集 + display_name）传递给 `start_completion()`（或通过 `AgentRuntime` 字段获取）

## 6. ESC 取消与边界情况

- [x] 6.1 在 `cancel_agent_response()` 中重置 `ai_conversation_rounds` 为 0（`pending_agents` 已有清空逻辑）
- [x] 6.2 确保 `agent_to_agent_max_rounds = 0` 时功能完全禁用——不做 @ 检测、不注入 prompt、行为与现有版本一致

## 7. 文档与配置示例

- [x] 7.1 更新 TDD 附录 B 的 v0.4 描述
- [x] 7.2 在 PDD 和 TDD 的配置示例中添加 `agent_to_agent_routing` 和 `agent_to_agent_max_rounds`（注释形式，标注默认值）
