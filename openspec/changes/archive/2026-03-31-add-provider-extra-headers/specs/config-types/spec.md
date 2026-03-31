## MODIFIED Requirements

### Requirement: Provider configuration fields
The `ProviderConfig` struct SHALL include an optional `extra_headers` field of type `Option<HashMap<String, String>>` that deserializes from TOML inline tables. The field SHALL default to `None` when not specified in the configuration file.

#### Scenario: Parse extra_headers from TOML
- **WHEN** a provider config contains `extra_headers = { "X-Custom" = "value" }`
- **THEN** `ProviderConfig.extra_headers` SHALL be `Some(HashMap)` containing the entry `("X-Custom", "value")`

#### Scenario: Parse config without extra_headers
- **WHEN** a provider config does not contain `extra_headers`
- **THEN** `ProviderConfig.extra_headers` SHALL be `None`
