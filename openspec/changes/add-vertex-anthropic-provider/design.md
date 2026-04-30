## Context

krew-cli 的 LLM provider 边界目前分成三类：`anthropic` 负责 Anthropic Messages API，`google` 负责 Gemini API 和 Vertex AI Gemini，`openai` 负责 OpenAI 及 OpenAI-compatible 服务。Claude on Vertex AI 处在两个现有边界之间：请求和响应接近 Anthropic Messages API，但 endpoint、认证、model listing 和部分 server tool 命名属于 Vertex AI。

现有 `AnthropicClient` 已经实现 message conversion、tool use、thinking 参数、SSE parsing、usage 统计和 server-side web search event 解析。新增 `vertex-anthropic` 应最大程度复用这些协议逻辑，只替换真正不同的接入层。

目标使用方式有两种：

```txt
Google 官方 Vertex endpoint
  api_key_env -> Google OAuth access token
  auth        -> Authorization: Bearer <token>
  base_url    -> None

LiteLLM Vertex passthrough
  api_key_env -> LiteLLM virtual key / proxy key
  auth        -> Authorization: Bearer <key>
  base_url    -> https://litellm.example.com/vertex_ai
             或 https://litellm.example.com/vertex_ai/v1
```

## Goals / Non-Goals

**Goals:**

- 提供新的 provider type：`vertex-anthropic`。
- 支持 Google 官方 Vertex AI Claude endpoint 和 LiteLLM Vertex passthrough endpoint。
- 使用 `api_key` / `api_key_env` 作为 Bearer token，不区分 Google token 与 LiteLLM key。
- 支持 `vertex_project` 和 `vertex_location` 配置，并使用 Vertex 原生 model ID。
- 复用 Anthropic Messages 协议转换和 SSE parsing。
- 支持 client tools、thinking、sampling、image tool results、`extra_headers` 和 `enable_web_search`。
- 支持 `krew config init`、`krew config add provider`、`krew config help`、`list_models` 和 fallback models。

**Non-Goals:**

- 不支持 ADC、service account JSON、自动调用 `gcloud auth print-access-token` 或 token refresh。
- 不把 LiteLLM `/v1/messages` unified Anthropic endpoint 纳入 `vertex-anthropic`；该路径继续由现有 `anthropic` provider + `base_url` 覆盖。
- 不新增 prompt caching、batch、count tokens 或 non-streaming API。
- 不处理 GCP Marketplace 启用、IAM 授权、organization policy、VPC-SC 等平台开通问题。

## Decisions

### 1. 新增 `ProviderType::VertexAnthropic`

采用独立 provider type，而不是在 `anthropic` provider 上复用 `vertex_project`。

原因：
- `anthropic` 的认证契约是 `x-api-key` + `anthropic-version` header。
- `vertex-anthropic` 的认证契约是 `Authorization: Bearer <token>`，且 `anthropic_version` 在 body 中。
- 用户配置和 wizard 中明确区分 `Anthropic`、`Google`、`Vertex Anthropic`，可以避免误配。

备选方案：
- `type = "anthropic"` + `vertex_project`：配置短，但 provider 行为变得隐式，且与现有 spec 冲突。
- `type = "google"` + `publisher = "anthropic"`：贴近 Vertex，但请求格式完全不是 Gemini `generateContent`。

### 2. `api_key` / `api_key_env` 统一表示 Bearer token

`vertex-anthropic` 不引入新的 `auth_type` 字段。运行时将 resolved key 放入 `Authorization: Bearer <value>`。

原因：
- Google 官方 curl 示例使用 Bearer token。
- LiteLLM Vertex passthrough 也接受 Bearer 风格的 virtual key / proxy key。
- 当前 config 初始化和 agent 初始化已经围绕 `api_key` / `api_key_env` 构建，复用成本最低。

限制：
- token 过期由用户或 LiteLLM 管理。krew-cli 第一版不刷新 token。

### 3. `base_url` 表示 Vertex passthrough root

默认 endpoint：

```txt
https://{host}/v1/projects/{project}/locations/{location}/publishers/anthropic/models/{model}:streamRawPredict
```

官方 Vertex endpoint 的 host 选择规则：

```txt
location == "global" -> aiplatform.googleapis.com
location == "us"     -> aiplatform.us.rep.googleapis.com
location == "eu"     -> aiplatform.eu.rep.googleapis.com
其他 location        -> {location}-aiplatform.googleapis.com
```

当 `base_url` 有值时，视为 passthrough root，并支持两种写法：

```txt
https://litellm.example.com/vertex_ai
https://litellm.example.com/vertex_ai/v1
```

`/vertex_ai` 是 LiteLLM Vertex passthrough 的路由约定，不是 Google 官方 endpoint 的路径要求。URL builder SHALL 去除尾部 `/`，如果 base 已以 `/v1` 结尾，则拼接 `/projects/...`；否则拼接 `/v1/projects/...`。其他自定义 root（例如反向代理把 Vertex passthrough 挂在域名根路径）也按同一规则处理。中间路径段包含 `v1` 但不以 `/v1` 结尾的 URL 不做特殊处理，例如 `https://proxy.example.com/api/v1/foo` 会拼成 `https://proxy.example.com/api/v1/foo/v1/projects/...`。路径大小写不归一化，按用户提供的 `base_url` 原样使用。

### 4. 抽取 Anthropic 协议共用逻辑

应将 `anthropic.rs` 中可复用逻辑公开到 crate 内部，例如：

- `ConvertedMessages`
- `convert_messages`
- `build_sampling_params`
- `build_thinking_params`
- `build_output_config`
- `convert_tools`
- `build_event_stream`

`AnthropicClient` 和 `VertexAnthropicClient` 使用同一套 conversion 和 SSE parsing。Vertex 客户端只覆盖：

- URL 构造
- auth mode
- request body 的 `model` / `anthropic_version` 差异
- web search tool type
- provider name 日志

### 5. Request body 差异最小化

Anthropic 直连 body：

```json
{
  "model": "claude-opus-4-7",
  "messages": [],
  "stream": true
}
```

Vertex Anthropic body：

```json
{
  "anthropic_version": "vertex-2023-10-16",
  "messages": [],
  "stream": true
}
```

`model` SHALL NOT 出现在 Vertex Anthropic body，因为 model 已在 URL 中。其余字段，包括 `system`、`max_tokens`、`temperature`、`top_p`、`top_k`、`stop_sequences`、`thinking`、`output_config`、`tools`，SHALL 复用 Anthropic 逻辑。

### 6. Web search tool type 与 Anthropic Messages API 保持一致

`anthropic` 继续注入：

```json
{ "type": "web_search_20250305", "name": "web_search" }
```

`vertex-anthropic` 注入：

```json
{ "type": "web_search_20250305", "name": "web_search" }
```

Google Vertex AI Claude web search 文档和实际 Vertex/LiteLLM passthrough 校验都接受 Anthropic versioned tool type `web_search_20250305`。LiteLLM Vertex passthrough 是 `:rawPredict` / `:streamRawPredict` passthrough，不是 LiteLLM `/v1/messages` unified Anthropic endpoint，因此仍发送 Vertex/Anthropic Messages 契约的 versioned tool type。SSE parsing 继续复用 Anthropic server tool event parser，因为 Vertex streaming events 仍使用 `server_tool_use`、`input_json_delta` 和 `web_search_tool_result` 结构。

### 7. Model listing 使用 Vertex Anthropic publisher

`ProviderType::VertexAnthropic` 的 `list_models` SHALL 调用：

```txt
GET https://{host}/v1/projects/{project}/locations/{location}/publishers/anthropic/models
```

当 `base_url` 指向 LiteLLM Vertex passthrough 时，使用同一 URL 拼接规则请求：

```txt
GET {base}/v1/projects/{project}/locations/{location}/publishers/anthropic/models
```

响应 name 去除 `publishers/anthropic/models/` 前缀，并保留 Vertex 原生 ID，例如 `claude-opus-4-7`、`claude-sonnet-4-5@20250929`。fallback models SHALL mirror official Vertex API model IDs, which are intentionally mixed: some latest models use aliases such as `claude-opus-4-7`, while others use versioned IDs such as `claude-haiku-4-5@20251001`. Google 维护这些 alias，可用性可能与 Anthropic 直连存在时间差。

### 8. Wizard 和 help 暴露一等选项

`krew config init` 和 `krew config add provider` 的 provider type 选项增加 `Vertex Anthropic`。默认建议：

- provider name：`vertex-anthropic`
- key env：`VERTEX_ANTHROPIC_API_KEY`
- location：`global`
- base_url：空值表示 Google 官方 endpoint

CLI 交互文本保持英文；OpenSpec 和项目文档说明可以中文为主。

## Risks / Trade-offs

- [Risk] 用户把 Google access token 放入长期配置后过期导致运行失败。→ Mitigation：文档明确 `api_key_env` 应指向可更新的环境变量；LiteLLM 场景建议使用 LiteLLM virtual key。
- [Risk] LiteLLM passthrough endpoint 对 `Authorization`、`x-litellm-api-key` 的部署配置可能不同。→ Mitigation：第一版规范 `Authorization: Bearer`；用户可通过 `extra_headers` 添加部署要求的辅助 header。
- [Risk] 抽取 `anthropic.rs` 内部函数可能扩大模块可见性。→ Mitigation：使用 `pub(crate)`，不暴露到 crate public API。
- [Risk] Web search tool type 与 Anthropic server tool 版本绑定，若发送未版本化的 `web_search` 会被 Vertex/LiteLLM passthrough 拒绝。→ Mitigation：`anthropic` 和 `vertex-anthropic` 均发送 `web_search_20250305`，测试覆盖两个 provider。
- [Risk] `base_url` 拼接规则可能产生重复 `/v1`。→ Mitigation：URL builder 单测覆盖 `/vertex_ai`、`/vertex_ai/`、`/vertex_ai/v1`、`/vertex_ai/v1/` 和不带 `/vertex_ai` 的自定义 root。
- [Risk] `list_models` 通过 LiteLLM passthrough 时可能被代理禁用。→ Mitigation：沿用现有 fallback model 机制，失败后可手动输入 model。
