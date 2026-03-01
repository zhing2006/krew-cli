## Context

Phase 4 完成了 OpenAI Chat Completions API 的完整实现（`openai_chat.rs`，~615 行），包括消息转换、采样参数映射、SSE 解析、retry 逻辑。这为其他 Provider 的实现提供了成熟的参考模式。

当前 `krew-llm` 中 `anthropic.rs`、`google.rs`、`openai_responses.rs` 均为单行 header stub。`LlmClient` trait 和 `StreamEvent` 等核心类型已稳定，无需修改。

各 Provider 的 API 协议差异显著：SSE 事件格式不同、消息结构不同、thinking/reasoning 启用方式不同、但 retry 逻辑和错误分类高度共通。

## Goals / Non-Goals

**Goals:**
- 实现 Anthropic、Google Gemini、OpenAI Responses 三个 Provider 的完整流式客户端
- 支持 Azure（OpenAI Responses）和 Vertex AI（Google Gemini）云平台变体
- 统一 thinking/reasoning 配置（enable_thinking + thinking_effort）
- 提取公共模块减少代码重复
- 正确处理多 Agent 场景下的消息格式转换（role 交替、消息合并）

**Non-Goals:**
- 不实现 OpenAI-Compatible Provider（兼容第三方服务直接用 OpenAI Chat Client）
- 不实现工具系统的完整 Agent Loop（Phase 8）
- 不实现 Web Search 工具注入（后续 Phase）
- 不修改 `LlmClient` trait 签名
- 不实现 Provider 工厂/自动选择（由调用方根据配置构造具体 Client）

## Decisions

### D1: 公共模块提取策略

**决策**：新建 `common.rs` 模块，提取以下共享逻辑：

| 提取内容 | 函数签名 |
|---|---|
| HTTP 状态码分类 | `classify_status(StatusCode) -> RetryAction` |
| 错误消息提取 | `extract_error_message(Response) -> String` |
| 带重试的请求发送 | `send_with_retry(client, request, retries_config) -> Result<Response>` |
| SSE unfold 模板 | 各 Provider 自行实现（因事件解析格式差异太大） |

**理由**：retry 逻辑和错误分类在三个 Provider 中完全一致，SSE 解析因格式差异必须各自实现。`openai_chat.rs` 中的现有实现迁移到 `common.rs` 后，自身调用公共函数。

**替代方案**：
- 各 Provider 内联（已否决——重复代码约 80 行/Provider）
- trait 化 retry（过度抽象——三个实现结构完全一致，函数即可）

### D2: Thinking 配置到 Provider 参数映射

**决策**：`AgentConfig` 新增 `enable_thinking: bool`（默认 false）和 `thinking_effort: Option<ThinkingEffort>` 枚举（Low/Medium/High，默认 Medium）。各 Provider Client 在构造时接收这两个值，在请求中映射：

| Provider | enable_thinking=true 时的请求参数 |
|---|---|
| Anthropic | `"thinking": {"type": "enabled", "budget_tokens": <mapped>}` 或 `"thinking": {"type": "adaptive"}` + effort |
| Gemini | `"generationConfig.thinkingConfig": {"includeThoughts": true, "thinkingBudget": <mapped>}` |
| OpenAI Responses | `"reasoning": {"effort": "<mapped>", "summary": "auto"}` |
| OpenAI Chat | 忽略（Chat API 无 thinking 支持，reasoning_content 由模型自发） |

Anthropic effort 映射策略：
- 对 Opus 4.6 / Sonnet 4.6：使用 `"type": "adaptive"` + `output_config.effort`
- 对旧模型：使用 `"type": "enabled"` + `budget_tokens`，effort 映射为 low→1024, medium→8192, high→32768

Gemini effort 映射策略（按模型代区分）：
- **Gemini 3.x**（gemini-3*）：使用 `thinkingLevel` 枚举，effort 直接映射为 "low"/"medium"/"high"
- **Gemini 2.5**（gemini-2.5*）：使用 `thinkingBudget` 数值，effort 映射为 low→1024, medium→8192, high→24576
- 两者不可同时设置，按模型名前缀判断使用哪种
- 未知模型默认使用 `thinkingLevel`（面向未来新模型）
- 所有情况均设置 `includeThoughts: true` 以便接收思考内容

OpenAI Responses effort 映射：直接传递（low/medium/high 一一对应）

**理由**：统一的 effort 级别比 provider-specific 的 budget_tokens 更符合用户心智模型。高级用户可通过 SamplingConfig 扩展精细控制（未来工作）。

### D3: 消息格式转换——连续同 Role 合并

**决策**：各 Provider 的 `convert_messages()` 在转换 role 后，MUST 合并连续同 role 的消息。合并方式为用 `\n\n` 连接 content，每条消息保留 `[agent_name]` 前缀以区分发言者。

示例（从 Agent C 视角看 Agent A 和 B 的连续回复）：

```
// 转换前（统一格式）：
[
  { role: Assistant, name: "agentA", content: "我认为..." },
  { role: Assistant, name: "agentB", content: "我同意..." },
]

// 转换后（给 Anthropic/Gemini 的请求格式，其他 Agent→user role，然后合并）：
[
  { role: "user", content: "[agentA] 我认为...\n\n[agentB] 我同意..." },
]
```

**理由**：Anthropic 和 Gemini 严格要求 user/assistant 交替。即使其他 Agent 转为 user role，多个 Agent 连续回复后仍会产生连续 user 消息，必须合并。

### D4: Azure 模式实现

**决策**：`OpenAiResponsesClient` 检测 `ProviderConfig.azure_endpoint` 有值时进入 Azure 模式：
- URL：`{azure_endpoint}/openai/v1/responses`
- 认证：使用 `api-key` header（而非 `Authorization: Bearer`）
- 请求体：与标准 OpenAI Responses API 完全一致
- Model：在请求 body 的 `model` 字段指定（不在 URL 路径中）

**理由**：Azure 2025 年 8 月起支持 v1 路径，无需 api-version 参数，简化实现。

### D5: Vertex AI 模式实现

**决策**：`GoogleClient` 检测 `ProviderConfig.vertex_project` 和 `vertex_location` 有值时进入 Vertex AI 模式：
- URL：`https://{location}-aiplatform.googleapis.com/v1/projects/{project}/locations/{location}/publishers/google/models/{model}:streamGenerateContent?alt=sse`
- 认证：`Authorization: Bearer {token}`，token 从 `api_key_env` 指定的环境变量读取
- 请求体：与标准 Gemini API 完全一致

**理由**：Vertex AI 的差异仅在 URL 和认证方式，请求体格式一致。用户需自行通过 `gcloud auth print-access-token` 获取 token 并设置环境变量。

### D6: Anthropic System Prompt 处理

**决策**：Anthropic API 的 system prompt 不在 messages 数组中，而是作为请求体的顶层 `system` 字段。`convert_messages()` SHALL 将 `ChatRole::System` 消息从 messages 中分离出来，返回 `(system_text, converted_messages)` 元组。

**理由**：Anthropic API 设计要求 system 在顶层，不能放在 messages 中。Google Gemini 同理，使用 `systemInstruction` 顶层字段。

### D7: SSE 解析策略

**决策**：各 Provider 的 SSE 解析各自实现，不共享：

| Provider | SSE 特点 | 解析策略 |
|---|---|---|
| Anthropic | 有 `event:` 类型字段（message_start, content_block_delta 等）| 按 event type 分发，需状态机跟踪当前 content_block index |
| Gemini | 无 `event:` 类型，纯 `data:` JSON | 每行解析为完整 GenerateContentResponse，检查 parts 中 thought 标记 |
| OpenAI Responses | 有 `event:` 类型字段（~53 种）| 只处理约 8 种关键事件类型，其余忽略 |

**理由**：三种格式差异过大，强行统一会增加复杂度。每个 Provider 约 50-80 行解析逻辑，可维护性优于强行抽象。

### D8: Anthropic Usage 字段映射

**决策**：Anthropic 使用 `input_tokens` / `output_tokens`（不同于 OpenAI 的 `prompt_tokens` / `completion_tokens`）。映射到 `Usage` 结构体时：
- `input_tokens` → `prompt_tokens`
- `output_tokens` → `completion_tokens`
- 两者之和 → `total_tokens`

**理由**：`Usage` 结构体已定义且稳定，只是字段命名差异，映射即可。

## Risks / Trade-offs

| 风险 | 缓解措施 |
|---|---|
| Vertex AI token 会过期（1小时） | 文档中明确说明，长会话需用户手动刷新 token |
| Anthropic thinking 启用后 temperature 必须为 1.0 | Client 在请求时自动覆盖 temperature，日志警告用户 |
| Gemini SSE 没有 event type 字段，依赖 data-only 解析 | 使用 `alt=sse` 参数确保 SSE 格式，逐行解析 JSON |
| 连续消息合并可能丢失上下文边界 | 每条消息使用 `[agent_name]` 前缀保留发言者身份 |
| OpenAI Responses 事件类型多达 53 种 | 只处理核心事件（text delta, tool call, reasoning, done, error），其余安全忽略 |
| 三家 name 字段全不支持 | 统一走 content prefix 方案，只有 OpenAI Chat 保留 name field 选项 |
