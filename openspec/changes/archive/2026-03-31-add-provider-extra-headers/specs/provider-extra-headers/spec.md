## ADDED Requirements

### Requirement: Provider extra_headers configuration
The system SHALL allow users to configure optional `extra_headers` in `[providers.*]` sections of `settings.toml`. The `extra_headers` field SHALL be a key-value map of HTTP header names to values. When present, these headers SHALL be appended to chat/inference requests (i.e. `chat_stream()` via `send_with_retry()`). Non-inference requests such as `list_models` are NOT covered. Users MUST NOT configure header names that conflict with provider-internal or authentication headers (e.g. `Authorization`, `x-api-key`, `anthropic-version`, `content-type`); behavior when conflicting headers are configured is undefined.

#### Scenario: Vertex AI Priority PayGo headers
- **WHEN** user configures `extra_headers = { "X-Vertex-AI-LLM-Request-Type" = "shared", "X-Vertex-AI-LLM-Shared-Request-Type" = "priority" }` in a Google provider
- **THEN** chat/inference requests to Vertex AI SHALL include both headers

#### Scenario: No extra_headers configured
- **WHEN** user does not specify `extra_headers` in a provider config
- **THEN** provider behavior SHALL be unchanged from current behavior

#### Scenario: Extra headers with any provider type
- **WHEN** user configures `extra_headers` on any provider type (openai, anthropic, google)
- **THEN** the headers SHALL be included in chat/inference requests regardless of provider type

#### Scenario: Conflicting header names
- **WHEN** user configures `extra_headers` with a name that conflicts with a provider-internal header (e.g. `anthropic-version`)
- **THEN** behavior is undefined; documentation SHALL warn users not to use conflicting header names
