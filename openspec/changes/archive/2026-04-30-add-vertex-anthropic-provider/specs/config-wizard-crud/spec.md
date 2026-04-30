## ADDED Requirements

### Requirement: config add provider supports Vertex Anthropic
`krew config add provider` SHALL support adding a `vertex-anthropic` provider using the same collection flow as User Init.

#### Scenario: Add Vertex Anthropic provider
- **WHEN** the user runs `krew config add provider` and selects `Vertex Anthropic`
- **THEN** the command SHALL collect Bearer token storage, `vertex_project`, `vertex_location` and optional `base_url`
- **AND** SHALL write a provider with `type = "vertex-anthropic"` to `~/.krew/settings.toml`

#### Scenario: Name conflict for Vertex Anthropic
- **WHEN** the default provider name `vertex-anthropic` already exists
- **THEN** the command SHALL suggest a unique name such as `vertex-anthropic-2`

### Requirement: config list providers displays Vertex Anthropic
`krew config list providers` SHALL display `vertex-anthropic` providers with a readable type label and the same key status checks as other providers.

#### Scenario: List Vertex Anthropic provider
- **WHEN** user config contains a provider with `type = "vertex-anthropic"`
- **THEN** `krew config list providers` SHALL include it in the provider table
- **AND** the type column SHALL identify it as `Vertex Anthropic`

#### Scenario: Environment variable status
- **WHEN** a `vertex-anthropic` provider uses `api_key_env`
- **THEN** the command SHALL check that environment variable and mark it set or missing using the existing provider list behavior
