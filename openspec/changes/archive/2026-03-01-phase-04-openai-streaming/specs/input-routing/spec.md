## MODIFIED Requirements

### Requirement: Echo 回复
当消息寻址到的 Agent 有 LLM 客户端时，SHALL 调用 agent loop 进行实际 LLM 对话。仅当 Agent 为 builtin echo 类型时，回显消息 SHALL 以黄色菱形 `◆` 前缀和路由标记显示。

#### Scenario: LLM Agent 调用
- **WHEN** 用户输入 `@gpt explain this` 且 gpt agent 有 LlmClient
- **THEN** 系统 SHALL 调用 `agent.complete(messages)` 启动 LLM 对话，不再 echo 回显

#### Scenario: builtin echo 保持
- **WHEN** 用户输入 `@echo hello` 且 echo agent 的 provider 为 "builtin"
- **THEN** echo 回显 SHALL 显示为 `◆ [→ @echo] echo: @echo hello`，菱形为黄色

#### Scenario: @all 路由标记
- **WHEN** 用户输入 `@all hello`
- **THEN** echo 回显 SHALL 显示为 `◆ [→ @all] echo: @all hello`，菱形为黄色

#### Scenario: 无前缀路由标记
- **WHEN** 用户输入 `just chatting`
- **THEN** echo 回显 SHALL 显示为 `◆ [→ last] echo: just chatting`，菱形为黄色

## ADDED Requirements

### Requirement: 消息历史管理
App SHALL 维护当前会话的消息历史列表（`Vec<ChatMessage>`），用于构建 LLM 请求的上下文。

#### Scenario: 用户消息加入历史
- **WHEN** 用户发送消息
- **THEN** SHALL 构建 `ChatMessage { role: User, content, addressee }` 并追加到消息历史

#### Scenario: Agent 回复加入历史
- **WHEN** Agent 回复完成（收到 Done 事件）
- **THEN** SHALL 构建 `ChatMessage { role: Assistant, agent_name, content: 累积的完整回复, usage }` 并追加到消息历史

#### Scenario: 历史传递给 Agent
- **WHEN** 调用 `agent.complete()`
- **THEN** SHALL 传入完整的消息历史列表，使 LLM 具有上下文
