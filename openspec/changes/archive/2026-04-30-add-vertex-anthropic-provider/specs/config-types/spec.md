## ADDED Requirements

### Requirement: ProviderType supports vertex-anthropic
`krew-config` SHALL add a `ProviderType::VertexAnthropic` variant that deserializes from TOML value `vertex-anthropic`.

#### Scenario: Deserialize vertex-anthropic provider type
- **WHEN** TOML contains `type = "vertex-anthropic"` in a provider block
- **THEN** `ProviderConfig.provider_type` SHALL equal `ProviderType::VertexAnthropic`

#### Scenario: Existing provider types unchanged
- **WHEN** TOML contains `type = "openai"`、`type = "anthropic"` or `type = "google"`
- **THEN** deserialization SHALL continue to map to the existing provider type variants

### Requirement: Vertex Anthropic provider configuration fields
For `ProviderType::VertexAnthropic`, `ProviderConfig` SHALL use existing fields with provider-specific semantics: `api_key` / `api_key_env` are Bearer token sources, `base_url` is an optional Vertex passthrough root, and `vertex_project` / `vertex_location` identify the Vertex AI project and location.

#### Scenario: Parse Vertex Anthropic provider config
- **WHEN** TOML contains `type = "vertex-anthropic"` with `api_key_env`、`base_url`、`vertex_project` and `vertex_location`
- **THEN** `ProviderConfig` SHALL preserve all fields without requiring new config keys

#### Scenario: Vertex fields missing
- **WHEN** `type = "vertex-anthropic"` omits `vertex_project` or `vertex_location`
- **THEN** config deserialization SHALL succeed
- **AND** runtime initialization SHALL be responsible for skipping unusable agents with a warning
