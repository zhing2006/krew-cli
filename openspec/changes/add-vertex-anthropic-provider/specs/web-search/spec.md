## ADDED Requirements

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
