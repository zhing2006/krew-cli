## Why

krew-cli 目前支持 Anthropic API 和 Google Vertex AI Gemini，但缺少 Claude on Vertex AI 的一等 provider。用户需要通过 Google Vertex AI 或 LiteLLM Vertex passthrough 调用 Claude，同时保留 Anthropic Messages 语义、tool use、thinking 和 `enable_web_search` 能力。

从 Claude Opus 4.7 开始，Vertex AI API model ID 与 Anthropic 直连模型命名进一步统一，适合新增一个清晰的 `vertex-anthropic` provider，而不是要求用户绕过到 OpenAI-compatible 或混用现有 `anthropic` / `google` provider。

## What Changes

- 新增 provider type：`vertex-anthropic`。
- 新增 Vertex Anthropic 客户端，使用 Anthropic Messages 请求/响应语义，但通过 Vertex AI `publishers/anthropic/models/{model}:streamRawPredict` 端点发送请求。
- `api_key` / `api_key_env` 对 `vertex-anthropic` 表示 Bearer token，可用于 Google OAuth access token，也可用于 LiteLLM virtual key / proxy key。
- 支持官方 Google Vertex endpoint 和 LiteLLM Vertex passthrough endpoint；`base_url` 可指向 passthrough root，LiteLLM 的固定路由约定通常是以 `/vertex_ai` 或 `/vertex_ai/v1` 结尾。
- 复用 Anthropic message conversion、tool use、thinking、SSE parsing 的行为，避免复制并分叉核心协议逻辑。
- `enable_web_search = true` 时按 Google Vertex AI Claude web search API 注入 `web_search_20250305` server tool，与 Anthropic Messages API 保持一致。
- `list_models` 支持列出 Vertex AI `publishers/anthropic/models` 并返回 Vertex 原生 Claude model ID。
- `krew config init`、`krew config add provider`、`krew config help` 和配置写入流程支持 `vertex-anthropic`。
- 不新增 ADC / gcloud 自动取 token 能力；第一版只支持显式 key/token。

## Capabilities

### New Capabilities

- `vertex-anthropic-client`: 定义 Claude on Vertex AI 的 endpoint、认证、请求体、流式解析、LiteLLM passthrough 兼容和复用边界。

### Modified Capabilities

- `config-types`: `ProviderType` 增加 `vertex-anthropic`，并定义该 provider 对 `api_key`、`base_url`、`vertex_project`、`vertex_location` 的配置语义。
- `config-file-writer`: 配置写入器能够序列化 `vertex-anthropic` provider。
- `config-wizard-init`: 初始化向导能够添加 `vertex-anthropic` provider 并收集 Vertex project、location、Bearer token 环境变量和可选 `base_url`。
- `config-wizard-crud`: `krew config add provider` 支持交互式添加 `vertex-anthropic` provider。
- `config-help-command`: 配置帮助文档展示 `vertex-anthropic` 字段语义和示例。
- `llm-list-models`: `list_models` 支持 Vertex Anthropic publisher model listing，并使用 Bearer token 认证。
- `web-search`: `enable_web_search` 对 `vertex-anthropic` 注入 `web_search_20250305` tool，并继续解析 server-side web search streaming events。

## Impact

- `crates/krew-config`: provider enum、TOML 反序列化测试和配置写入。
- `crates/krew-core`: agent 初始化时根据 `ProviderType::VertexAnthropic` 创建新客户端。
- `crates/krew-llm`: 新客户端、Anthropic 公共协议逻辑抽取、Vertex Anthropic URL 构造、model listing。
- `crates/krew-cli`: config wizard、CRUD、help 文本、相关测试。
- `docs/README.md`、`docs/MANUAL.md`、`docs/MANUAL_CN.md`、`docs/TDD.md`: 新增使用示例和技术说明。
- 不引入新的运行时云认证依赖；Bearer token 仍由用户或 LiteLLM 代理负责提供。
