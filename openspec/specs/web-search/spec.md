## ADDED Requirements

### Requirement: Provider-native web search injection
When `enable_web_search = true` is configured for an agent, the LLM client SHALL inject the provider's native web search tool into the API request's tools array. The model autonomously decides whether to trigger a search.

#### Scenario: OpenAI Responses API with web search enabled
- **WHEN** an agent uses OpenAI Responses API (`api_type: "responses"`) with `enable_web_search = true`
- **THEN** the request body's `tools` array SHALL include `{ "type": "web_search" }`

#### Scenario: Anthropic API with web search enabled
- **WHEN** an agent uses Anthropic provider with `enable_web_search = true`
- **THEN** the request body's `tools` array SHALL include `{ "type": "web_search_20250305", "name": "web_search" }`

#### Scenario: Google Gemini API with web search enabled
- **WHEN** an agent uses Google Gemini provider with `enable_web_search = true`
- **THEN** the request body's `tools` array SHALL include `{ "google_search": {} }`

#### Scenario: OpenAI Chat Completions API with web search enabled
- **WHEN** an agent uses OpenAI Chat Completions API (`api_type: "chat"`) with `enable_web_search = true`
- **THEN** the request body SHALL include `web_search_options: { "search_context_size": "medium" }` to enable OpenAI native web search or LiteLLM proxy search

#### Scenario: Compatible provider with web search enabled
- **WHEN** an agent uses a Compatible provider with `enable_web_search = true`
- **THEN** the request body SHALL include `web_search_options: { "search_context_size": "medium" }` (behavior depends on whether the compatible service supports it)

#### Scenario: Web search disabled
- **WHEN** an agent has `enable_web_search = false` (default)
- **THEN** no search tool SHALL be injected regardless of provider type

### Requirement: Web search config propagation
The `enable_web_search` configuration field SHALL be propagated from `AgentConfig` through `LlmClientConfig` to each provider's `chat_stream()` implementation.

#### Scenario: Config flows to LlmClientConfig
- **WHEN** an agent runtime is constructed with `enable_web_search = true`
- **THEN** the `LlmClientConfig` passed to the LLM client constructor SHALL have `enable_web_search = true`


### Requirement: Vertex Anthropic web search injection
When `enable_web_search = true` is configured for an agent using `vertex-anthropic`, the LLM client SHALL inject the Vertex Claude native web search tool into the request body's `tools` array.

#### Scenario: Vertex Anthropic web search enabled
- **WHEN** an agent uses `vertex-anthropic` with `enable_web_search = true`
- **THEN** the request body's `tools` array SHALL include `{ "type": "web_search_20250305", "name": "web_search" }`

#### Scenario: Vertex Anthropic passthrough web search enabled
- **WHEN** an agent uses `vertex-anthropic` with a LiteLLM Vertex passthrough `base_url` and `enable_web_search = true`
- **THEN** the request body's `tools` array SHALL include `{ "type": "web_search_20250305", "name": "web_search" }`
- **AND** the client SHALL NOT switch to `{ "type": "web_search", "name": "web_search" }`

#### Scenario: Anthropic direct web search unchanged
- **WHEN** an agent uses `anthropic` with `enable_web_search = true`
- **THEN** the request body's `tools` array SHALL continue to include `{ "type": "web_search_20250305", "name": "web_search" }`

#### Scenario: Web search disabled
- **WHEN** an agent uses `vertex-anthropic` with `enable_web_search = false`
- **THEN** no web search tool SHALL be injected
