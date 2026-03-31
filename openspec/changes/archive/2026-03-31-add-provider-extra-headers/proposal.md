## Why

用户需要为 Vertex AI 的 Priority PayGo 等功能传递自定义 HTTP headers（如 `X-Vertex-AI-LLM-Request-Type`），但当前 Provider 配置不支持 `extra_headers`。底层的 `send_with_retry` 已经支持 extra_headers 参数，只是配置层和各 provider client 没有接入。

## What Changes

- 在 `ProviderConfig` 中新增 `extra_headers` 可选字段，类型为 `HashMap<String, String>`
- 在 `LlmClientConfig` 中新增 `extra_headers` 字段，用于从配置层传递到各 provider client
- 所有 provider client（Google、Anthropic、OpenAI Chat、OpenAI Responses）支持从配置读取 extra_headers 并传递给 `send_with_retry`
- Anthropic client 的硬编码 headers 与用户 extra_headers 合并
- 更新配置示例和相关文档（README、MANUAL、PDD、TDD）

## Capabilities

### New Capabilities

- `provider-extra-headers`: Provider 级别的自定义 HTTP headers 配置，允许用户在 `[providers.*]` 中通过 `extra_headers` 字段指定额外的 HTTP 请求头，这些 headers 会被附加到发往该 provider 的 chat/inference 请求中（不含 `list_models` 等非推理请求）

### Modified Capabilities

- `config-types`: 新增 `extra_headers` 字段到 `ProviderConfig`
- `google-client`: 支持从配置读取 extra_headers 并传递给 HTTP 请求
- `anthropic-client`: 将用户 extra_headers 与硬编码 headers 合并
- `openai-chat-client`: 支持从配置读取 extra_headers 并传递给 HTTP 请求
- `openai-responses-client`: 支持从配置读取 extra_headers 并传递给 HTTP 请求

## Impact

- **代码**：`krew-config`（ProviderConfig）、`krew-llm`（LlmClientConfig + 4 个 provider client）、`krew-core`（provider 创建逻辑）
- **配置格式**：`settings.toml` 新增可选字段，向后兼容
- **文档**：README.md、README_CN.md、MANUAL.md、MANUAL_CN.md、PDD.md、TDD.md、config.example.toml
