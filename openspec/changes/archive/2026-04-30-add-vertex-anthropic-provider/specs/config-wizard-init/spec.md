## ADDED Requirements

### Requirement: User Init supports Vertex Anthropic provider
`krew config init` User Init provider loop SHALL include `Vertex Anthropic` as a provider type option and collect the fields required to call Claude on Vertex AI.

#### Scenario: Select Vertex Anthropic provider type
- **WHEN** User Init shows the provider type selection
- **THEN** the options SHALL include `Vertex Anthropic`

#### Scenario: Default provider name and key env
- **WHEN** the user selects `Vertex Anthropic`
- **THEN** the default provider name SHALL be `vertex-anthropic`
- **AND** the default environment variable name SHALL be `VERTEX_ANTHROPIC_API_KEY`

#### Scenario: Collect Vertex fields
- **WHEN** the user selects `Vertex Anthropic`
- **THEN** User Init SHALL prompt for `Vertex AI project ID`
- **AND** SHALL prompt for `Vertex AI location` with default `global`

#### Scenario: Optional passthrough base_url
- **WHEN** the user selects `Vertex Anthropic`
- **THEN** User Init SHALL allow an empty `Base URL`
- **AND** an empty value SHALL mean Google official Vertex endpoint
- **AND** a non-empty value SHALL be written as `base_url`

#### Scenario: Write Vertex Anthropic provider config
- **WHEN** the user completes Vertex Anthropic provider setup
- **THEN** User Init SHALL write a provider with `type = "vertex-anthropic"`、`api_key_env` or `api_key`、`vertex_project` and `vertex_location`

### Requirement: Smart Preset includes Vertex Anthropic models
Project Init Smart Preset SHALL treat `vertex-anthropic` providers as model sources through `list_models` and SHALL allow selected Claude models to become agents.

#### Scenario: Fetch Vertex Anthropic models
- **WHEN** Smart Preset fetches available models for a configured `vertex-anthropic` provider
- **THEN** it SHALL call `list_models` with `ProviderType::VertexAnthropic`

#### Scenario: Create agent from Vertex Anthropic model
- **WHEN** the user selects model `claude-opus-4-7` from provider `vertex-anthropic`
- **THEN** the generated agent SHALL reference provider `vertex-anthropic` and model `claude-opus-4-7`
