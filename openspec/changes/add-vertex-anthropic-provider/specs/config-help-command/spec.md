## ADDED Requirements

### Requirement: config help documents Vertex Anthropic
`krew config help` SHALL document `vertex-anthropic` as a supported provider type and explain its field semantics.

#### Scenario: Provider type list includes vertex-anthropic
- **WHEN** executing `krew config help`
- **THEN** the providers section SHALL list `vertex-anthropic` alongside `openai`、`anthropic` and `google`

#### Scenario: Vertex Anthropic field semantics
- **WHEN** executing `krew config help`
- **THEN** the help text SHALL explain that `api_key` / `api_key_env` are Bearer token sources for `vertex-anthropic`
- **AND** SHALL explain that `base_url` is optional and can point to a LiteLLM Vertex passthrough root
- **AND** SHALL explain that `vertex_project` and `vertex_location` are required for runtime use

#### Scenario: Vertex Anthropic example
- **WHEN** executing `krew config help`
- **THEN** the example configurations SHALL include a `[providers.vertex-anthropic]` block using `type = "vertex-anthropic"`
