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
- **THEN** the system SHALL silently ignore the setting and NOT inject any search tool

#### Scenario: Compatible provider with web search enabled
- **WHEN** an agent uses a Compatible provider with `enable_web_search = true`
- **THEN** the system SHALL silently ignore the setting and NOT inject any search tool

#### Scenario: Web search disabled
- **WHEN** an agent has `enable_web_search = false` (default)
- **THEN** no search tool SHALL be injected regardless of provider type

### Requirement: Web search config propagation
The `enable_web_search` configuration field SHALL be propagated from `AgentConfig` through `LlmClientConfig` to each provider's `chat_stream()` implementation.

#### Scenario: Config flows to LlmClientConfig
- **WHEN** an agent runtime is constructed with `enable_web_search = true`
- **THEN** the `LlmClientConfig` passed to the LLM client constructor SHALL have `enable_web_search = true`
