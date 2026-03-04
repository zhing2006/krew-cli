## ADDED Requirements

### Requirement: McpClient struct
`krew-tools` SHALL define `McpClient` that wraps an `rmcp` RunningService over stdio transport. It SHALL provide methods: `initialize()`, `list_tools()`, `call_tool()`.

#### Scenario: Create and initialize McpClient
- **WHEN** calling `McpClient::new(config)` with a valid McpServerConfig pointing to a working MCP server
- **THEN** it SHALL spawn the child process, perform MCP handshake, and return an initialized McpClient

#### Scenario: Initialize timeout
- **WHEN** MCP server does not respond within the startup timeout (10 seconds)
- **THEN** `McpClient::new()` SHALL return an error indicating timeout

### Requirement: McpClient list_tools
McpClient SHALL provide `async fn list_tools(&self) -> Result<Vec<McpToolInfo>>` that returns all tools discovered from the MCP server.

#### Scenario: List tools from server
- **WHEN** calling `client.list_tools()` on an initialized client
- **THEN** it SHALL return a Vec of McpToolInfo containing each tool's name, description, input_schema, and annotations

### Requirement: McpToolInfo struct
`krew-tools` SHALL define `McpToolInfo` struct containing: `name: String`, `description: String`, `input_schema: serde_json::Value`, `annotations: Option<McpToolAnnotations>`.

#### Scenario: McpToolInfo fields
- **WHEN** constructing McpToolInfo from an rmcp Tool
- **THEN** all fields SHALL be populated from the corresponding rmcp Tool fields

### Requirement: McpToolAnnotations struct
`krew-tools` SHALL define `McpToolAnnotations` struct containing: `destructive_hint: Option<bool>`, `read_only_hint: Option<bool>`, `open_world_hint: Option<bool>`, `idempotent_hint: Option<bool>`.

#### Scenario: Annotations from rmcp
- **WHEN** an rmcp Tool has annotations with `destructive_hint = true`
- **THEN** McpToolAnnotations SHALL reflect `destructive_hint = Some(true)`

#### Scenario: No annotations
- **WHEN** an rmcp Tool has no annotations
- **THEN** McpToolAnnotations SHALL be None

### Requirement: McpClient call_tool
McpClient SHALL provide `async fn call_tool(&self, tool_name: &str, arguments: serde_json::Value) -> Result<ToolResult>` that invokes a tool on the MCP server and returns the result.

#### Scenario: Successful tool call
- **WHEN** calling `client.call_tool("list_directory", json!({"path": "."}))` on a server that supports this tool
- **THEN** it SHALL return a ToolResult with the tool's output content and `is_error = false`

#### Scenario: Tool call error from server
- **WHEN** the MCP server returns an error result (is_error = true)
- **THEN** it SHALL return a ToolResult with `is_error = true` and the error content

#### Scenario: Tool call with empty arguments
- **WHEN** calling `client.call_tool("some_tool", json!({}))` with empty arguments
- **THEN** it SHALL pass empty arguments to the MCP server without error

### Requirement: McpClient child process cleanup
When McpClient is dropped, it SHALL ensure the MCP server child process is terminated.

#### Scenario: Drop cleanup
- **WHEN** McpClient is dropped (goes out of scope)
- **THEN** the child process SHALL be killed via `kill_on_drop(true)` on the tokio Command

### Requirement: McpClient stderr logging
McpClient SHALL spawn a background task to read and log the MCP server's stderr output using the `tracing` crate.

#### Scenario: Server stderr output
- **WHEN** the MCP server writes to stderr
- **THEN** the output SHALL be logged at info level with the server name prefix
