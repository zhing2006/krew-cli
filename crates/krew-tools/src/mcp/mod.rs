//! MCP (Model Context Protocol) client for discovering and invoking external tools.

mod client;
mod handler;
mod manager;

pub use client::{McpClient, McpToolAnnotations, McpToolInfo};
pub use handler::{McpToolHandler, check_mcp_approval};
pub use manager::{McpManager, McpServerInfo, expand_env};

/// Generate a qualified tool name for LLM consumption.
///
/// Format: `mcp__{server}__{tool}` — only `[a-zA-Z0-9_-]` allowed.
pub fn qualified_name(server: &str, tool: &str) -> String {
    format!("mcp__{}_{}", sanitize(server), sanitize(tool))
}

/// Generate a human-readable display name for TUI rendering.
///
/// Format: `mcp:{server}/{tool}`
pub fn display_name(server: &str, tool: &str) -> String {
    format!("mcp:{server}/{tool}")
}

/// Check whether a tool name is an MCP tool (starts with `mcp__`).
pub fn is_mcp_tool(name: &str) -> bool {
    name.starts_with("mcp__")
}

/// Extract display name from a qualified MCP tool name.
///
/// Converts `mcp__{server}__{tool}` → `mcp:{server}/{tool}`.
/// Returns `None` if the name is not an MCP tool.
pub fn display_name_from_qualified(qualified: &str) -> Option<String> {
    let rest = qualified.strip_prefix("mcp__")?;
    // Split on first `_` only (server__tool)
    let (server, tool) = rest.split_once('_')?;
    Some(format!("mcp:{server}/{tool}"))
}

/// Sanitize a name to only contain `[a-zA-Z0-9_-]`.
pub fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
