## ADDED Requirements

### Requirement: Dynamic MCP tool registration
ToolRegistry SHALL support dynamic registration of MCP tools after initial creation via `register()`. MCP tools are registered during MCP server startup.

#### Scenario: Dynamic registration of MCP tools
- **WHEN** MCP server discovers 3 tools and calls `registry.register()` 3 times
- **THEN** registry SHALL contain 9 specs total (6 built-in + 3 MCP)

### Requirement: MCP tool approval query
ToolRegistry SHALL correctly report approval status for registered MCP tools via `requires_approval()`.

#### Scenario: Query MCP tool with trust=auto
- **WHEN** registered an MCP tool with trust=auto and calling `registry.requires_approval("mcp__fs__read")`
- **THEN** SHALL return `false`

#### Scenario: Query MCP tool with trust=confirm and destructive
- **WHEN** registered an MCP tool with trust=confirm and destructive_hint=true and calling `registry.requires_approval("mcp__fs__delete")`
- **THEN** SHALL return `true`
