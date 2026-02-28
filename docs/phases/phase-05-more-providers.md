# Phase 5: 更多 LLM Provider

> 目标：接入剩余 Provider，覆盖所有支持的 LLM 服务。

## 实现内容

- **Anthropic Client**：`POST /v1/messages` (stream=true)，处理 content block 结构、ThinkingDelta
- **Google Gemini Client**：`generateContent` (stream=true)，处理 parts 格式、function calling 格式
- **OpenAI Responses Client**：`POST /v1/responses` (stream=true)，处理 response 事件格式
- **OpenAI-Compatible Client**：复用 OpenAI Chat 实现，替换 base_url 和认证
- **Azure 模式**：OpenAI Client 检测 `azure_endpoint`，切换 URL 和认证方式
- **消息格式转换**：各 Provider 的 `convert_messages()` 实现，正确处理 Agent 身份（self vs other）

## 验收标准

```txt
you> @opus 你好
[opus] Claude Opus:
  你好！我是 Claude...

you> @gemini 你好
[gemini] Gemini 3.1 Pro:
  你好！我是 Gemini...
```

## 参考

| 文档 | 位置 | 内容 |
| ---- | ---- | ---- |
| PDD | L37-47 | §2.2 Agent 定义（provider、api_type） |
| TDD | L271-312 | §3.3.2 四种 Provider 实现细节 |
| TDD | L314-357 | §3.3.3 消息格式转换（self_agent_name、OtherAgentRole） |
| TDD | L375-386 | §3.3.5 原生 Web Search 注入方式 |
| TDD | L387-418 | §3.3.6 采样参数映射（Anthropic max_tokens 必填等特殊处理） |
| TDD | L1028-1037 | krew-llm 源码结构 |
