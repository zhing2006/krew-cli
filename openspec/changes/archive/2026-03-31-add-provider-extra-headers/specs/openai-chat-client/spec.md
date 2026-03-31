## MODIFIED Requirements

### Requirement: OpenAI Chat client sends extra headers
The `OpenAiChatClient` SHALL accept extra headers during construction and pass them to `send_with_retry()` in `chat_stream()`.

#### Scenario: Extra headers present
- **WHEN** `OpenAiChatClient` is constructed with extra_headers containing `[("X-Foo", "bar")]`
- **THEN** every `chat_stream()` request SHALL include the `X-Foo: bar` HTTP header

#### Scenario: No extra headers
- **WHEN** `OpenAiChatClient` is constructed without extra_headers (empty vec)
- **THEN** `send_with_retry()` SHALL receive `None` for extra_headers, maintaining current behavior
