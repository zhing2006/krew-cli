## ADDED Requirements

### Requirement: ListModelsConfig 参数结构体
`krew-llm` SHALL 定义 `ListModelsConfig` 结构体，包含以下字段：
- `provider_type: ProviderType`
- `base_url: Option<String>`
- `api_key: String`
- `vertex_project: Option<String>`
- `vertex_location: Option<String>`

#### Scenario: ListModelsConfig 可构造
- **WHEN** 构造 `ListModelsConfig` 传入所有字段
- **THEN** SHALL 编译通过且所有字段可访问

### Requirement: list_models 公开 API
`krew-llm` SHALL 提供 `pub async fn list_models(config: &ListModelsConfig) -> Result<Vec<ModelInfo>, LlmError>` 函数，根据 provider_type 和配置调用对应供应商的 List Models 端点。

#### Scenario: 函数签名
- **WHEN** 导入 `krew_llm::list_models`
- **THEN** 该函数 SHALL 接受 `&ListModelsConfig` 参数，返回 `Result<Vec<ModelInfo>, LlmError>`

### Requirement: ModelInfo 结构体
`krew-llm` SHALL 定义 `ModelInfo` 结构体，包含 `id: String` 字段。

#### Scenario: ModelInfo 可访问
- **WHEN** 导入 `krew_llm::ModelInfo`
- **THEN** 该类型 SHALL 包含 `id` 字段

### Requirement: OpenAI List Models
当 provider_type 为 `OpenAI` 时，SHALL 调用 `GET {base_url}/v1/models`（默认 base_url 为 `https://api.openai.com`），使用 `Authorization: Bearer <api_key>` 认证。

#### Scenario: OpenAI 默认端点
- **WHEN** provider_type 为 OpenAI 且 base_url 为 None
- **THEN** SHALL 请求 `https://api.openai.com/v1/models`

#### Scenario: OpenAI 自定义端点
- **WHEN** provider_type 为 OpenAI 且 base_url 为 `https://api.deepseek.com`
- **THEN** SHALL 请求 `https://api.deepseek.com/v1/models`

#### Scenario: OpenAI 认证头
- **WHEN** 发送 OpenAI List Models 请求
- **THEN** SHALL 包含 `Authorization: Bearer <api_key>` header

#### Scenario: OpenAI 模型过滤
- **WHEN** OpenAI 返回的模型列表包含 `gpt-5.4`、`gpt-5.4-mini`、`text-embedding-3-small`、`dall-e-3`、`tts-1`、`whisper-1`
- **THEN** SHALL 仅保留 `gpt-5.4` 和 `gpt-5.4-mini`（过滤掉 embedding/dall-e/tts/whisper）

#### Scenario: OpenAI 过滤规则
- **WHEN** 过滤 OpenAI 模型
- **THEN** SHALL 仅保留 id 以 `gpt`、`o`、`chatgpt` 开头的模型

### Requirement: Anthropic List Models
当 provider_type 为 `Anthropic` 时，SHALL 调用 `GET {base_url}/v1/models`（默认 base_url 为 `https://api.anthropic.com`），使用 `X-Api-Key: <api_key>` 和 `anthropic-version: 2023-06-01` header 认证。

#### Scenario: Anthropic 端点和认证
- **WHEN** provider_type 为 Anthropic 且 base_url 为 None
- **THEN** SHALL 请求 `https://api.anthropic.com/v1/models`
- **AND** SHALL 包含 `X-Api-Key` 和 `anthropic-version` header

#### Scenario: Anthropic 自定义端点
- **WHEN** provider_type 为 Anthropic 且 base_url 已设置
- **THEN** SHALL 使用 `{base_url}/v1/models`

#### Scenario: Anthropic 模型过滤
- **WHEN** 过滤 Anthropic 模型
- **THEN** SHALL 仅保留 id 以 `claude-` 开头的模型

### Requirement: Google Gemini API List Models
当 provider_type 为 `Google` 且 vertex_project 为 None 时，SHALL 调用 `GET https://generativelanguage.googleapis.com/v1beta/models?key=<api_key>&pageSize=1000`。

#### Scenario: Google Gemini API 端点和认证
- **WHEN** provider_type 为 Google 且 vertex_project 为 None 且 base_url 为 None
- **THEN** SHALL 请求 `https://generativelanguage.googleapis.com/v1beta/models?key=<api_key>&pageSize=1000`

#### Scenario: Google 模型 id 格式
- **WHEN** Google API 返回 `name: "models/gemini-3.1-pro-preview"`
- **THEN** SHALL 提取为 `gemini-3.1-pro-preview`（去掉 `models/` 前缀）

#### Scenario: Google 模型过滤
- **WHEN** 过滤 Google 模型
- **THEN** SHALL 仅保留 id 以 `gemini-` 开头的模型

### Requirement: Google Vertex AI List Models
当 provider_type 为 `Google` 且 vertex_project 和 vertex_location 均有值时，SHALL 调用 Vertex AI 的 List Publisher Models 端点：`GET https://{location}-aiplatform.googleapis.com/v1/projects/{project}/locations/{location}/publishers/google/models`，使用 `Authorization: Bearer <api_key>` 认证（与现有运行时 Vertex AI 认证方式一致）。

#### Scenario: Vertex AI 端点构造
- **WHEN** provider_type 为 Google 且 vertex_project = "my-project" 且 vertex_location = "us-central1"
- **THEN** SHALL 请求 `https://us-central1-aiplatform.googleapis.com/v1/projects/my-project/locations/us-central1/publishers/google/models`

#### Scenario: Vertex AI 认证
- **WHEN** 发送 Vertex AI List Models 请求
- **THEN** SHALL 使用 `Authorization: Bearer <api_key>` header（与运行时 `GoogleClient` 的 Vertex 模式一致）

#### Scenario: Vertex AI 模型过滤
- **WHEN** Vertex AI 返回模型列表
- **THEN** SHALL 仅保留 id 以 `gemini-` 开头的模型（过滤掉非 Gemini 模型）

#### Scenario: Vertex AI 模型 id 提取
- **WHEN** Vertex AI 返回 `name: "publishers/google/models/gemini-3.1-pro-preview"`
- **THEN** SHALL 提取为 `gemini-3.1-pro-preview`（去掉 `publishers/google/models/` 前缀）

### Requirement: 请求超时
所有 List Models HTTP 请求 SHALL 使用 5 秒超时。

#### Scenario: 超时处理
- **WHEN** 供应商 API 在 5 秒内未响应
- **THEN** SHALL 返回超时错误

### Requirement: Fallback 硬编码模型列表
`krew-llm` SHALL 提供 `pub fn fallback_models(provider_type: ProviderType) -> Vec<ModelInfo>` 函数，返回各供应商的硬编码模型列表。

#### Scenario: Anthropic fallback 列表
- **WHEN** 调用 `fallback_models(ProviderType::Anthropic)`
- **THEN** SHALL 返回 `["claude-opus-4-6", "claude-sonnet-4-6", "claude-haiku-4-5-20251001"]`

#### Scenario: OpenAI fallback 列表
- **WHEN** 调用 `fallback_models(ProviderType::OpenAI)`
- **THEN** SHALL 返回 `["gpt-5.4", "gpt-5.4-mini", "gpt-5.4-nano"]`

#### Scenario: Google fallback 列表
- **WHEN** 调用 `fallback_models(ProviderType::Google)`
- **THEN** SHALL 返回 `["gemini-3.1-pro-preview", "gemini-3.1-flash-lite-preview"]`

### Requirement: 模型列表排序
返回的模型列表 SHALL 按模型 id 字母顺序排序。

#### Scenario: 排序结果
- **WHEN** API 返回无序的模型列表
- **THEN** SHALL 按 id 字母顺序升序排列后返回


### Requirement: Vertex Anthropic List Models
When `provider_type` is `ProviderType::VertexAnthropic`, `list_models` SHALL call the Vertex AI Anthropic publisher models endpoint and authenticate with `Authorization: Bearer <api_key>`.

#### Scenario: Google Vertex Anthropic endpoint
- **WHEN** `provider_type = ProviderType::VertexAnthropic`、`vertex_project = "my-project"`、`vertex_location = "global"` and `base_url = None`
- **THEN** `list_models` SHALL request `https://aiplatform.googleapis.com/v1/projects/my-project/locations/global/publishers/anthropic/models`

#### Scenario: Google Vertex Anthropic regional endpoint
- **WHEN** `provider_type = ProviderType::VertexAnthropic`、`vertex_project = "my-project"`、`vertex_location = "us-east5"` and `base_url = None`
- **THEN** `list_models` SHALL request `https://us-east5-aiplatform.googleapis.com/v1/projects/my-project/locations/us-east5/publishers/anthropic/models`

#### Scenario: Google Vertex Anthropic multi-region endpoint
- **WHEN** `provider_type = ProviderType::VertexAnthropic`、`vertex_project = "my-project"`、`vertex_location = "eu"` and `base_url = None`
- **THEN** `list_models` SHALL request `https://aiplatform.eu.rep.googleapis.com/v1/projects/my-project/locations/eu/publishers/anthropic/models`

#### Scenario: LiteLLM passthrough endpoint without v1
- **WHEN** `provider_type = ProviderType::VertexAnthropic` and `base_url = "https://litellm.example.com/vertex_ai"`
- **THEN** `list_models` SHALL request `https://litellm.example.com/vertex_ai/v1/projects/{project}/locations/{location}/publishers/anthropic/models`

#### Scenario: LiteLLM passthrough endpoint with v1
- **WHEN** `provider_type = ProviderType::VertexAnthropic` and `base_url = "https://litellm.example.com/vertex_ai/v1"`
- **THEN** `list_models` SHALL request `https://litellm.example.com/vertex_ai/v1/projects/{project}/locations/{location}/publishers/anthropic/models`

#### Scenario: Generic passthrough root
- **WHEN** `provider_type = ProviderType::VertexAnthropic` and `base_url = "https://proxy.example.com"`
- **THEN** `list_models` SHALL request `https://proxy.example.com/v1/projects/{project}/locations/{location}/publishers/anthropic/models`

#### Scenario: Bearer auth
- **WHEN** sending a Vertex Anthropic List Models request
- **THEN** the request SHALL include `Authorization: Bearer <api_key>`

#### Scenario: Model ID extraction
- **WHEN** Vertex AI returns `name = "publishers/anthropic/models/claude-sonnet-4-5@20250929"`
- **THEN** `list_models` SHALL return model id `claude-sonnet-4-5@20250929`

#### Scenario: Model filtering
- **WHEN** Vertex AI returns Anthropic and non-Anthropic publisher models
- **THEN** `list_models` SHALL only include ids that start with `claude-`

### Requirement: Vertex Anthropic fallback models
`fallback_models(ProviderType::VertexAnthropic)` SHALL return a hardcoded list of current Vertex Anthropic Claude model IDs. The list SHALL use Vertex AI API model IDs, including aliases and versioned IDs as published by Google/Anthropic.

#### Scenario: Fallback list
- **WHEN** calling `fallback_models(ProviderType::VertexAnthropic)`
- **THEN** the returned models SHALL include `claude-opus-4-7`、`claude-opus-4-6`、`claude-sonnet-4-6` and `claude-haiku-4-5@20251001`
