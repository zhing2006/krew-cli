## Why

当前 krew-cli 中只有用户能发起消息路由（通过 @ 寻址）。Agent 之间无法直接对话——用户必须手动中转，例如先让 gpt 写代码，再让 opus review。v0.4 目标是让 Agent 在回复中可以 @其他 Agent，自动触发 AI-to-AI 对话链，实现真正的多 Agent 协作。

## What Changes

- Agent 回复文本中的 `@agent_name` 会被检测并自动路由到目标 Agent，形成 AI-to-AI 对话链
- 支持两种路由策略（可配置）：
  - `immediate`（默认）：目标 Agent 移动/插入到队列头部，立即接话
  - `queued`：目标 Agent 仅追加到队列尾部，不改变已有顺序
- AI-to-AI 对话受 `agent_to_agent_max_rounds` 配置限制（默认 10），防止无限循环
- Agent 不 @ 任何人时对话自然结束
- 用户在 AI-to-AI 期间不能发送新消息（可在输入框编辑），ESC 可中断
- System prompt 中注入可 @ 的 Agent 列表，告知 Agent 可以主动发起跨 Agent 对话
- 触发方式：Agent 在回复中自主决定是否 @ 其他 Agent。用户可通过自然语言引导（如 `@gpt 帮我请 opus review 一下`，gpt 理解后在回复中 @opus）

## Capabilities

### New Capabilities
- `agent-to-agent-routing`: Agent 回复中的 @ 检测、队列调度（immediate/queued 两种策略）、轮次计数与终止控制

### Modified Capabilities
- `config-types`: `Settings` 新增 `agent_to_agent_routing` 和 `agent_to_agent_max_rounds` 字段，新增 `AgentToAgentRouting` 枚举

## Impact

- `krew-config`: 新增 `agent_to_agent_routing` 和 `agent_to_agent_max_rounds` 配置字段
- `krew-core/router.rs`: 新增 Agent 回复文本中的 @ 解析函数
- `krew-core/agent/mod.rs`: system prompt 注入可 @ 的 Agent 列表
- `krew-cli/app/state.rs`: `handle_agent_event(Done)` 中新增 AI-to-AI 路由分支逻辑
- `App` state: 新增 `ai_conversation_rounds` 计数器
