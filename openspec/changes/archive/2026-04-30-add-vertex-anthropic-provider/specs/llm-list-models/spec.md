## ADDED Requirements

### Requirement: Vertex Anthropic List Models
When `provider_type` is `ProviderType::VertexAnthropic`, `list_models` SHALL call the Vertex AI Anthropic publisher models endpoint and authenticate with `Authorization: Bearer <api_key>`.

#### Scenario: Google Vertex Anthropic endpoint
- **WHEN** `provider_type = ProviderType::VertexAnthropic`、`vertex_project = "my-project"`、`vertex_location = "global"` and `base_url = None`
- **THEN** `list_models` SHALL request `https://aiplatform.googleapis.com/v1/projects/my-project/locations/global/publishers/anthropic/models`

#### Scenario: Google Vertex Anthropic regional endpoint
- **WHEN** `provider_type = ProviderType::VertexAnthropic`、`vertex_project = "my-project"`、`vertex_location = "us-east5"` and `base_url = None`
- **THEN** `list_models` SHALL request `https://us-east5-aiplatform.googleapis.com/v1/projects/my-project/locations/us-east5/publishers/anthropic/models`

#### Scenario: Google Vertex Anthropic multi-region endpoint
- **WHEN** `provider_type = ProviderType::VertexAnthropic`、`vertex_project = "my-project"`、`vertex_location = "eu"` and `base_url = None`
- **THEN** `list_models` SHALL request `https://aiplatform.eu.rep.googleapis.com/v1/projects/my-project/locations/eu/publishers/anthropic/models`

#### Scenario: LiteLLM passthrough endpoint without v1
- **WHEN** `provider_type = ProviderType::VertexAnthropic` and `base_url = "https://litellm.example.com/vertex_ai"`
- **THEN** `list_models` SHALL request `https://litellm.example.com/vertex_ai/v1/projects/{project}/locations/{location}/publishers/anthropic/models`

#### Scenario: LiteLLM passthrough endpoint with v1
- **WHEN** `provider_type = ProviderType::VertexAnthropic` and `base_url = "https://litellm.example.com/vertex_ai/v1"`
- **THEN** `list_models` SHALL request `https://litellm.example.com/vertex_ai/v1/projects/{project}/locations/{location}/publishers/anthropic/models`

#### Scenario: Generic passthrough root
- **WHEN** `provider_type = ProviderType::VertexAnthropic` and `base_url = "https://proxy.example.com"`
- **THEN** `list_models` SHALL request `https://proxy.example.com/v1/projects/{project}/locations/{location}/publishers/anthropic/models`

#### Scenario: Bearer auth
- **WHEN** sending a Vertex Anthropic List Models request
- **THEN** the request SHALL include `Authorization: Bearer <api_key>`

#### Scenario: Model ID extraction
- **WHEN** Vertex AI returns `name = "publishers/anthropic/models/claude-sonnet-4-5@20250929"`
- **THEN** `list_models` SHALL return model id `claude-sonnet-4-5@20250929`

#### Scenario: Model filtering
- **WHEN** Vertex AI returns Anthropic and non-Anthropic publisher models
- **THEN** `list_models` SHALL only include ids that start with `claude-`

### Requirement: Vertex Anthropic fallback models
`fallback_models(ProviderType::VertexAnthropic)` SHALL return a hardcoded list of current Vertex Anthropic Claude model IDs. The list SHALL use Vertex AI API model IDs, including aliases and versioned IDs as published by Google/Anthropic.

#### Scenario: Fallback list
- **WHEN** calling `fallback_models(ProviderType::VertexAnthropic)`
- **THEN** the returned models SHALL include `claude-opus-4-7`、`claude-opus-4-6`、`claude-sonnet-4-6` and `claude-haiku-4-5@20251001`
