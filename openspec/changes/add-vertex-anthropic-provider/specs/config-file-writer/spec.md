## ADDED Requirements

### Requirement: add_provider writes vertex-anthropic providers
`krew-config` writer SHALL serialize `ProviderType::VertexAnthropic` as `type = "vertex-anthropic"` and SHALL write existing optional fields using the same rules as other providers.

#### Scenario: Write Vertex Anthropic provider
- **WHEN** `add_provider()` receives `ProviderWriteData` with `provider_type = ProviderType::VertexAnthropic`
- **THEN** the generated provider table SHALL contain `type = "vertex-anthropic"`

#### Scenario: Write Vertex Anthropic fields
- **WHEN** Vertex Anthropic provider data includes `api_key_env`、`base_url`、`vertex_project`、`vertex_location` and `extra_headers`
- **THEN** `add_provider()` SHALL write those fields to the provider table

#### Scenario: List Vertex Anthropic provider
- **WHEN** `list_providers()` reads a provider with `type = "vertex-anthropic"`
- **THEN** it SHALL return the provider with `ProviderType::VertexAnthropic`
