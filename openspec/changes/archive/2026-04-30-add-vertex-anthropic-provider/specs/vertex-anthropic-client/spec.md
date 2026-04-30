## ADDED Requirements

### Requirement: Vertex Anthropic streaming request
`VertexAnthropicClient` SHALL implement `LlmClient` and send streaming Claude requests to Vertex AI `publishers/anthropic/models/{model}:streamRawPredict` endpoints. The request body SHALL use Anthropic Messages fields and SHALL include `anthropic_version = "vertex-2023-10-16"` and `stream = true`.

#### Scenario: Google Vertex regional endpoint
- **WHEN** `vertex_project = "my-project"`、`vertex_location = "us-east5"` 且 agent model 为 `claude-opus-4-7`
- **THEN** `chat_stream()` SHALL POST to `https://us-east5-aiplatform.googleapis.com/v1/projects/my-project/locations/us-east5/publishers/anthropic/models/claude-opus-4-7:streamRawPredict`

#### Scenario: Google Vertex global endpoint
- **WHEN** `vertex_project = "my-project"`、`vertex_location = "global"` 且 agent model 为 `claude-opus-4-7`
- **THEN** `chat_stream()` SHALL POST to `https://aiplatform.googleapis.com/v1/projects/my-project/locations/global/publishers/anthropic/models/claude-opus-4-7:streamRawPredict`

#### Scenario: Google Vertex multi-region endpoint
- **WHEN** `vertex_project = "my-project"`、`vertex_location = "us"` 且 agent model 为 `claude-opus-4-7`
- **THEN** `chat_stream()` SHALL POST to `https://aiplatform.us.rep.googleapis.com/v1/projects/my-project/locations/us/publishers/anthropic/models/claude-opus-4-7:streamRawPredict`

#### Scenario: Request body excludes model
- **WHEN** `chat_stream()` 构造 Vertex Anthropic 请求体
- **THEN** body SHALL include `anthropic_version = "vertex-2023-10-16"`、`messages`、`stream = true` 和 `max_tokens`
- **AND** body SHALL NOT include top-level `model`

#### Scenario: System prompt and sampling fields
- **WHEN** messages 包含 system prompt 且 `SamplingConfig` 设置 `temperature`、`top_p`、`top_k` 和 `stop_sequences`
- **THEN** Vertex Anthropic body SHALL use the same `system` and sampling field names as Anthropic Messages API

### Requirement: Vertex Anthropic Bearer authentication
`VertexAnthropicClient` SHALL authenticate chat requests with `Authorization: Bearer <api_key>`. For `vertex-anthropic`, `api_key` and `api_key_env` SHALL represent a Bearer token, which MAY be either a Google OAuth access token or a LiteLLM virtual key / proxy key.

#### Scenario: Bearer token from api_key_env
- **WHEN** provider config has `api_key_env = "LITELLM_API_KEY"` and the environment variable contains `sk-litellm`
- **THEN** chat requests SHALL include `Authorization: Bearer sk-litellm`

#### Scenario: Direct api_key
- **WHEN** provider config has `api_key = "ya29.token"`
- **THEN** chat requests SHALL include `Authorization: Bearer ya29.token`

#### Scenario: Missing token
- **WHEN** neither `api_key` nor `api_key_env` resolves to a non-empty value
- **THEN** agent initialization SHALL skip the agent with a startup warning using the same missing key behavior as other providers

### Requirement: Vertex Anthropic runtime prerequisites
`VertexAnthropicClient` runtime initialization SHALL require both `vertex_project` and `vertex_location` to be present for agents using `vertex-anthropic`.

#### Scenario: Missing vertex_project
- **WHEN** an agent uses a `vertex-anthropic` provider without `vertex_project`
- **THEN** agent initialization SHALL skip the agent with a startup warning

#### Scenario: Missing vertex_location
- **WHEN** an agent uses a `vertex-anthropic` provider without `vertex_location`
- **THEN** agent initialization SHALL skip the agent with a startup warning

### Requirement: Vertex Anthropic passthrough base_url
`VertexAnthropicClient` SHALL support a provider `base_url` that points to a Vertex AI passthrough root. The URL builder SHALL support roots ending with `/vertex_ai`, roots ending with `/vertex_ai/v1`, and generic custom roots.

#### Scenario: LiteLLM base_url without v1
- **WHEN** `base_url = "https://litellm.example.com/vertex_ai"`、`vertex_project = "proj"`、`vertex_location = "global"` and model is `claude-opus-4-7`
- **THEN** the request URL SHALL be `https://litellm.example.com/vertex_ai/v1/projects/proj/locations/global/publishers/anthropic/models/claude-opus-4-7:streamRawPredict`

#### Scenario: LiteLLM base_url with v1
- **WHEN** `base_url = "https://litellm.example.com/vertex_ai/v1"`、`vertex_project = "proj"`、`vertex_location = "global"` and model is `claude-opus-4-7`
- **THEN** the request URL SHALL be `https://litellm.example.com/vertex_ai/v1/projects/proj/locations/global/publishers/anthropic/models/claude-opus-4-7:streamRawPredict`

#### Scenario: Generic passthrough root without vertex_ai
- **WHEN** `base_url = "https://proxy.example.com"`、`vertex_project = "proj"`、`vertex_location = "global"` and model is `claude-opus-4-7`
- **THEN** the request URL SHALL be `https://proxy.example.com/v1/projects/proj/locations/global/publishers/anthropic/models/claude-opus-4-7:streamRawPredict`

#### Scenario: Trailing slash normalization
- **WHEN** `base_url` ends with `/`
- **THEN** the URL builder SHALL remove redundant trailing slashes before appending `/v1/projects/...`

#### Scenario: Case-sensitive base_url path
- **WHEN** `base_url` contains path segments with uppercase letters
- **THEN** the URL builder SHALL NOT rewrite the path casing

### Requirement: Vertex Anthropic reuses Anthropic protocol conversion
`VertexAnthropicClient` SHALL reuse Anthropic Messages conversion and stream parsing for messages, client tools, tool results, image tool results, thinking, sampling and usage mapping.

#### Scenario: Tool definitions
- **WHEN** `chat_stream()` receives client `ToolDefinition` values
- **THEN** Vertex Anthropic body SHALL include tools using Anthropic `input_schema` format

#### Scenario: Tool calls in stream
- **WHEN** Vertex Anthropic streaming response contains `content_block_start` with `tool_use` followed by `input_json_delta`
- **THEN** the stream SHALL emit `StreamEvent::ToolCall` with the accumulated JSON arguments

#### Scenario: Thinking deltas
- **WHEN** Vertex Anthropic streaming response contains `thinking_delta`
- **THEN** the stream SHALL emit `StreamEvent::ThinkingDelta`

#### Scenario: Usage mapping
- **WHEN** Vertex Anthropic streaming response ends with Anthropic `message_delta` usage and `message_stop`
- **THEN** the stream SHALL emit `StreamEvent::Done(Usage)` using `input_tokens` as `prompt_tokens` and `output_tokens` as `completion_tokens`

### Requirement: Vertex Anthropic web search tool
When `enable_web_search = true`, `VertexAnthropicClient` SHALL inject the Vertex Claude server-side web search tool using `{ "type": "web_search_20250305", "name": "web_search" }` for both Google official endpoints and Vertex passthrough endpoints.

#### Scenario: Web search enabled
- **WHEN** an agent uses `vertex-anthropic` with `enable_web_search = true`
- **THEN** the request body's `tools` array SHALL include `{ "type": "web_search_20250305", "name": "web_search" }`

#### Scenario: LiteLLM passthrough web search
- **WHEN** an agent uses `vertex-anthropic` with a LiteLLM Vertex passthrough `base_url` and `enable_web_search = true`
- **THEN** the request body's `tools` array SHALL still include `{ "type": "web_search_20250305", "name": "web_search" }`
- **AND** it SHALL NOT use the unversioned tool type `web_search`

#### Scenario: Web search streaming events
- **WHEN** Vertex Anthropic streaming response contains `server_tool_use` and `input_json_delta` for `web_search`
- **THEN** the stream SHALL emit `StreamEvent::ServerToolStart` and `StreamEvent::ServerToolDone`

### Requirement: Vertex Anthropic extra headers
`VertexAnthropicClient` SHALL append provider `extra_headers` to chat/inference requests after internal headers are configured. Users MUST NOT configure header names that conflict with internal auth or content headers. If conflicting header names are configured, behavior is undefined.

#### Scenario: Extra headers present
- **WHEN** provider config has `extra_headers = { "x-pass-anthropic-beta" = "context-1m-2025-08-07" }`
- **THEN** Vertex Anthropic chat requests SHALL include that header

#### Scenario: No extra headers
- **WHEN** provider config has no `extra_headers`
- **THEN** Vertex Anthropic chat requests SHALL behave as normal without user-defined headers
