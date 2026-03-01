## Why

krew-cli 已完成 TUI 框架、配置系统和输入解析（Phase 1-3），但用户输入仍然只能 echo 回显，无法真正与 LLM 对话。本阶段接入第一个 LLM Provider（OpenAI Chat Completions），跑通完整的单 Agent 实时对话流程，让产品从"可交互的终端"变成"可对话的 AI 助手"。

## What Changes

- **OpenAI Chat Completions Client**：在 `krew-llm` 中实现 `LlmClient` trait，支持 SSE 流式响应、错误处理与重试、采样参数映射
- **Agent Loop（单 Agent）**：在 `krew-core` 中实现消息构建 → LLM 请求 → 流式事件分发的完整循环，暂跳过工具调用
- **流式渲染管线**：在 `krew-cli` 中实现 newline-gated 的 Markdown 流式收集器 + 带自适应背压的渲染队列（参考 codex 架构）
- **Markdown 渲染**：使用 pulldown_cmark + syntect 实现代码块语法高亮和行内样式渲染
- **Agent 标识与 Token 用量**：回复带颜色标签 `[name] DisplayName:`，末尾显示 token 用量
- **Agent 初始化**：App 启动时根据配置构建 AgentRuntime，创建对应的 LlmClient 实例

## Capabilities

### New Capabilities
- `openai-chat-client`: OpenAI Chat Completions API 客户端实现（SSE 流式、错误重试、采样参数）
- `agent-loop`: 单 Agent 对话循环（消息构建、LLM 调用、事件分发）
- `streaming-pipeline`: 流式渲染管线（Markdown 流式收集、背压队列、自适应吞吐）
- `markdown-render`: Markdown 渲染引擎（行内样式、代码块语法高亮）
- `agent-display`: Agent 回复的标识显示与 Token 用量展示

### Modified Capabilities
- `tui-framework`: 新增流式渲染集成（commit tick 动画、流式事件消费）
- `input-routing`: 从 echo 模式切换为实际 LLM 调用（send_message 流程变更）

## Impact

- **krew-llm crate**: 实现 `openai_chat.rs`，新增 reqwest HTTP 请求和 eventsource-stream SSE 解析
- **krew-core crate**: 新增 agent loop 逻辑，定义 AgentEvent 通信协议
- **krew-cli crate**: 新增 streaming/、render/markdown.rs 模块，修改 app/message.rs 集成 LLM 调用
- **新增依赖**: pulldown_cmark、syntect、two-face（workspace 级别）
- **配置要求**: 用户需在 settings.toml 中配置 OpenAI provider（api_key_env）和至少一个使用该 provider 的 agent
