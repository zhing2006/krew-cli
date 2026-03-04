# mcp-tool-integration Specification

## Purpose
TBD - created by archiving change mcp-integration. Update Purpose after archive.
## Requirements
### Requirement: McpManager struct
`krew-tools` SHALL define `McpManager` that manages the lifecycle of multiple MCP server connections. It SHALL provide `start_all()` and `shutdown()` methods.

#### Scenario: Start all MCP servers
- **WHEN** calling `McpManager::start_all(configs, registry)` with a list of McpServerConfig and a mutable ToolRegistry reference
- **THEN** it SHALL start each MCP server concurrently, discover tools, and register them into the ToolRegistry

#### Scenario: Single server failure does not block others
- **WHEN** one MCP server fails to start (e.g., command not found) but others succeed
- **THEN** McpManager SHALL log the error for the failed server and continue with the others
- **AND** the successfully started servers' tools SHALL be registered normally

#### Scenario: Shutdown
- **WHEN** calling `manager.shutdown()` or dropping the McpManager
- **THEN** all MCP server child processes SHALL be terminated

### Requirement: MCP tool qualified name format
MCP tools registered in ToolRegistry SHALL use the qualified name format `mcp__{server}__{tool}` where server and tool names are sanitized to contain only `[a-zA-Z0-9_-]` characters, with other characters replaced by `_`.

#### Scenario: Qualified name generation
- **WHEN** registering a tool named `list_directory` from server `filesystem`
- **THEN** the ToolSpec name SHALL be `mcp__filesystem__list_directory`

#### Scenario: Name sanitization
- **WHEN** a tool name contains characters outside `[a-zA-Z0-9_-]` (e.g., `list.files`)
- **THEN** those characters SHALL be replaced with `_` (e.g., `list_files`)

### Requirement: MCP tool display name
MCP tools SHALL provide a display name in the format `mcp:{server}/{tool}` for TUI rendering purposes, stored in the ToolSpec description or as metadata.

#### Scenario: Display name format
- **WHEN** rendering an MCP tool call in TUI
- **THEN** it SHALL be displayed as `mcp:filesystem/list_directory` (not the qualified LLM name)

### Requirement: McpToolHandler implements ToolHandler
Each discovered MCP tool SHALL be wrapped in a `McpToolHandler` that implements the `ToolHandler` trait. The handler SHALL route `execute()` calls to the corresponding McpClient.

#### Scenario: Execute MCP tool
- **WHEN** ToolRegistry dispatches a call to `mcp__filesystem__list_directory`
- **THEN** McpToolHandler SHALL call `client.call_tool("list_directory", args)` on the correct McpClient

#### Scenario: requires_approval with trust=auto
- **WHEN** the MCP server has `trust = "auto"` and any tool is checked
- **THEN** `requires_approval()` SHALL return `false`

#### Scenario: requires_approval with trust=confirm and read_only tool
- **WHEN** the MCP server has `trust = "confirm"` and the tool has `read_only_hint = true`
- **THEN** `requires_approval()` SHALL return `false`

#### Scenario: requires_approval with trust=confirm and destructive tool
- **WHEN** the MCP server has `trust = "confirm"` and the tool has `destructive_hint = true`
- **THEN** `requires_approval()` SHALL return `true`

#### Scenario: requires_approval with trust=confirm and no annotations
- **WHEN** the MCP server has `trust = "confirm"` and the tool has no annotations
- **THEN** `requires_approval()` SHALL return `true` (safe default)

### Requirement: MCP tool registration into ToolRegistry
McpManager SHALL register each discovered MCP tool as a ToolSpec + McpToolHandler pair in the shared ToolRegistry.

#### Scenario: ToolSpec from MCP tool
- **WHEN** an MCP server exposes a tool with name, description, and input_schema
- **THEN** a ToolSpec SHALL be created with `name = "mcp__{server}__{tool}"`, the tool's description, and the tool's input_schema as parameters

#### Scenario: Duplicate tool names skipped
- **WHEN** two MCP servers expose tools that would produce the same qualified name
- **THEN** the second tool SHALL be skipped with a warning log

### Requirement: McpManager environment variable expansion
McpManager SHALL expand environment variable references in McpServerConfig's `env` field. Values starting with `$` SHALL be resolved from the process environment.

#### Scenario: Env var expansion
- **WHEN** config has `env = { GITHUB_TOKEN = "$GITHUB_TOKEN" }` and the process env has `GITHUB_TOKEN=abc123`
- **THEN** the MCP server process SHALL receive `GITHUB_TOKEN=abc123` in its environment

#### Scenario: Missing env var
- **WHEN** config references `$NONEXISTENT_VAR` and the env var does not exist
- **THEN** the value SHALL be passed as empty string and a warning SHALL be logged

