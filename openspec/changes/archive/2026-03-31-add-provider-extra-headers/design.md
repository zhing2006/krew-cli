## Context

当前 `ProviderConfig` 仅支持 `api_key`、`base_url`、`vertex_project`、`vertex_location` 等固定字段。底层 `common::send_with_retry()` 已支持 `extra_headers: Option<&[(String, String)]>` 参数，但配置层和各 provider client 未接入。

用户场景：Vertex AI Priority PayGo 需要传递 `X-Vertex-AI-LLM-Request-Type` 和 `X-Vertex-AI-LLM-Shared-Request-Type` 自定义 headers。

## Goals / Non-Goals

**Goals:**
- 允许用户在 `[providers.*]` 配置中通过 `extra_headers` 指定额外 HTTP headers
- 所有 provider（Google、Anthropic、OpenAI Chat、OpenAI Responses）统一支持
- 向后兼容，`extra_headers` 为可选字段

**Non-Goals:**
- 不支持 agent 级别的 extra_headers（仅 provider 级别）
- 不支持动态/运行时修改 headers
- 不处理 headers 冲突校验（用户自行保证不覆盖认证 headers）

## Decisions

### D1: extra_headers 放在 ProviderConfig 而非 AgentConfig

**选择**：provider 级别

**理由**：extra_headers 是 provider API 层面的配置（如 Vertex AI 的流量类型），与 agent 的模型、采样参数无关。同一 provider 下的所有 agent 共享相同的 HTTP headers。

**替代方案**：agent 级别或两级合并——增加复杂度但无明确场景需求。

### D2: 配置类型使用 `HashMap<String, String>`

**选择**：`Option<HashMap<String, String>>`

**理由**：TOML 内联表天然对应 HashMap，语法简洁：
```toml
extra_headers = { "X-Custom" = "value", "X-Other" = "value2" }
```

### D3: 通过 LlmClientConfig 传递到 client

**选择**：在 `LlmClientConfig` 中新增 `extra_headers: Vec<(String, String)>` 字段，各 client struct 存储并在 `chat_stream()` 中传递给 `send_with_retry()`

**理由**：复用现有数据流路径 `ProviderConfig → LlmClientConfig → XxxClient → send_with_retry()`，改动最小。HashMap 在传入 LlmClientConfig 时转为 `Vec<(String, String)>` 以匹配 `send_with_retry` 的签名。

### D4: Anthropic 硬编码 headers 与用户 extra_headers 合并

**选择**：先放硬编码 headers，再追加用户 extra_headers。不支持覆盖硬编码 headers。

**理由**：`reqwest` 的 `.header()` 方法使用 append 语义（同名 header 会产生重复值，而非替换）。因此用户不应配置与 provider 内部 headers 冲突的 header 名。文档中明确标注此限制。

**替代方案**：使用 `HeaderMap` 的 `insert`（替换语义）——但需要重构 `send_with_retry` 的接口，收益不大。

### D5: 作用范围限定为 chat/inference 请求

**选择**：extra_headers 仅应用于 `chat_stream()` 路径（通过 `send_with_retry()`），不覆盖 `list_models` 等非推理请求。

**理由**：`list_models` 使用独立的 GET 请求路径，不经过 `send_with_retry()`。Priority PayGo 等场景只需要在推理请求上设置 headers。扩展到 `list_models` 需改动更多代码且无明确需求。

## Risks / Trade-offs

- **[冲突 headers]** → 文档明确警告用户不要配置与 provider 内部或认证 headers 冲突的名称（如 `Authorization`、`x-api-key`、`anthropic-version`、`content-type`），冲突行为未定义
- **[headers 值包含特殊字符]** → TOML 字符串原生支持，无额外处理
- **[list_models 不受 extra_headers 影响]** → 文档说明作用范围仅限 chat/inference 请求
