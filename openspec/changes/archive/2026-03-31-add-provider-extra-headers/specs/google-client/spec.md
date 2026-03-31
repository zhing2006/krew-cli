## MODIFIED Requirements

### Requirement: Google client sends extra headers
The `GoogleClient` SHALL accept extra headers during construction and pass them to `send_with_retry()` in `chat_stream()`.

#### Scenario: Extra headers present
- **WHEN** `GoogleClient` is constructed with extra_headers containing `[("X-Foo", "bar")]`
- **THEN** every `chat_stream()` request SHALL include the `X-Foo: bar` HTTP header

#### Scenario: No extra headers
- **WHEN** `GoogleClient` is constructed without extra_headers (empty vec)
- **THEN** `send_with_retry()` SHALL receive `None` for extra_headers, maintaining current behavior
