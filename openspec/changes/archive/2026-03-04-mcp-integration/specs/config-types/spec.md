## ADDED Requirements

### Requirement: McpServerConfig Clone support
`McpServerConfig` SHALL derive `Clone` to allow MCP manager to clone configs during concurrent server startup.

#### Scenario: McpServerConfig can be cloned
- **WHEN** cloning a McpServerConfig instance
- **THEN** it SHALL produce an identical copy with all fields preserved

### Requirement: McpTrust default value
`McpTrust` SHALL default to `Confirm` when not specified in the MCP server config.

#### Scenario: McpTrust defaults to Confirm
- **WHEN** MCP server config does not specify a trust field
- **THEN** `trust.unwrap_or_default()` SHALL return `McpTrust::Confirm`
