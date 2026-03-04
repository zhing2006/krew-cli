## 1. Dependencies & Config

- [x] 1.1 Add `rmcp` dependency to krew-tools Cargo.toml with features `["client", "transport-child-process"]`
- [x] 1.2 Ensure McpServerConfig and McpTrust in krew-config derive Clone/Copy as needed for MCP module usage

## 2. MCP Client

- [x] 2.1 Create `krew-tools/src/mcp/mod.rs` with public module exports
- [x] 2.2 Implement `McpClient` in `krew-tools/src/mcp/client.rs`: spawn child process, initialize via rmcp, list_tools, call_tool, stderr logging
- [x] 2.3 Define `McpToolInfo` and `McpToolAnnotations` structs for tool discovery results

## 3. MCP Tool Handler

- [x] 3.1 Implement `McpToolHandler` in `krew-tools/src/mcp/handler.rs`: impl ToolHandler with annotations-based requires_approval() and execute() routing to McpClient
- [x] 3.2 Implement qualified name generation (`mcp__{server}__{tool}`) and display name (`mcp:{server}/{tool}`) with sanitization

## 4. MCP Manager

- [x] 4.1 Implement `McpManager` in `krew-tools/src/mcp/manager.rs`: start_all() for concurrent server startup, tool discovery, and registration into ToolRegistry
- [x] 4.2 Implement environment variable expansion for McpServerConfig env field
- [x] 4.3 Implement shutdown() and Drop cleanup for McpManager

## 5. Core Integration

- [x] 5.1 Integrate McpManager into krew-core session initialization: start MCP servers after config load, register tools into shared ToolRegistry
- [x] 5.2 Integrate McpManager shutdown into session cleanup
- [x] 5.3 Update agent loop to pass MCP tool display names for TUI rendering

## 6. TUI Display

- [x] 6.1 Update tool call TUI rendering to show MCP tools with `mcp:{server}/{tool}` format instead of qualified name

## 7. Tests

- [x] 7.1 Unit tests for McpToolHandler: requires_approval logic with various trust/annotation combinations
- [x] 7.2 Unit tests for qualified name generation and sanitization
- [x] 7.3 Unit tests for environment variable expansion
- [x] 7.4 Integration test for McpManager with a mock MCP server (if feasible, or test the registration flow)
