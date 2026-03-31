## MODIFIED Requirements

### Requirement: Anthropic client merges extra headers
The `AnthropicClient` SHALL append user-configured extra headers after its existing hardcoded headers (`anthropic-version`, `content-type`). Users MUST NOT configure header names that conflict with these hardcoded headers; behavior when conflicting headers are configured is undefined.

#### Scenario: User extra headers combined with hardcoded
- **WHEN** `AnthropicClient` is constructed with extra_headers containing `[("X-Custom", "val")]`
- **THEN** `send_with_retry()` SHALL receive headers containing both `anthropic-version`, `content-type`, and `X-Custom`

#### Scenario: No user extra headers
- **WHEN** `AnthropicClient` is constructed without extra_headers (empty vec)
- **THEN** behavior SHALL be identical to current implementation (only hardcoded headers)
