## Why

Phase 4 完成了 OpenAI Chat Completions API 的接入，但 krew-cli 的核心价值在于多 AI 协作——用户需要同时与 Claude、Gemini、GPT 对话。当前只支持一种 Provider，无法实现这一核心体验。本阶段接入 Anthropic、Google Gemini、OpenAI Responses API 三大核心 Provider 及其云平台变体（Azure、Vertex AI），使 krew-cli 真正成为多 LLM 协作工具。

## What Changes

- **Anthropic Messages API Client**：实现 `POST /v1/messages` 流式调用，支持 content block 结构、extended thinking（thinking_delta）、tool_use/tool_result
- **Google Gemini Client**：实现 `generateContent` 流式调用（SSE），支持 parts 格式、thinking（thought 标记）、function calling
- **OpenAI Responses API Client**：实现 `POST /v1/responses` 流式调用，支持 response 事件格式、reasoning summary、function call
- **Azure 模式**：OpenAI Responses Client 检测 `azure_endpoint` 时切换 URL 和认证方式（`api-key` header）
- **Vertex AI 模式**：Google Client 检测 `vertex_project`/`vertex_location` 时切换 endpoint 和认证方式（Bearer token）
- **公共模块提取**：从 openai_chat.rs 提取 retry 逻辑、错误分类、SSE unfold 模式到 common.rs
- **Thinking 配置**：AgentConfig 新增 `enable_thinking` 和 `thinking_effort` 字段，各 Provider 映射到各自的 thinking 参数
- **消息格式转换**：各 Provider 实现 `convert_messages()`，统一处理多 Agent 身份——其他 Agent 的回复转为 user role，连续同 role 消息合并
- **ProviderConfig 扩展**：新增 `vertex_project`、`vertex_location` 字段支持 Vertex AI 配置

## Capabilities

### New Capabilities
- `anthropic-client`: Anthropic Messages API 流式客户端，支持 thinking、tool_use、采样参数映射
- `google-client`: Google Gemini generateContent 流式客户端，支持 thinking、function calling、Vertex AI
- `openai-responses-client`: OpenAI Responses API 流式客户端，支持 reasoning summary、function call、Azure
- `llm-common`: LLM Provider 公共模块，提取 retry、错误分类、SSE 辅助等共享逻辑
- `thinking-config`: Thinking/Reasoning 统一配置，AgentConfig 层面的 enable_thinking + thinking_effort

### Modified Capabilities
- `config-types`: AgentConfig 新增 thinking 相关字段，ProviderConfig 新增 Vertex AI 字段

## Impact

- **代码范围**：主要修改 `krew-llm` crate（anthropic.rs、google.rs、openai_responses.rs、新增 common.rs），少量修改 `krew-config`（类型定义）
- **依赖**：无新外部依赖，复用现有 reqwest、eventsource-stream、futures
- **API 兼容**：LlmClient trait 签名不变，新 Provider 通过构造函数接收 thinking 配置
- **配置文件**：settings.toml 格式向后兼容（新字段均为 optional）
