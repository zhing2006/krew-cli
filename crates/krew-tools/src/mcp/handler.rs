//! MCP tool handler implementing the ToolHandler trait.

use std::sync::Arc;

use serde_json::Value;

use super::client::{McpClient, McpToolAnnotations};
use crate::{ToolContext, ToolError, ToolHandler, ToolResult};
use krew_config::McpTrust;

/// Handler that routes tool calls to an MCP server.
pub struct McpToolHandler {
    /// Qualified name for LLM (mcp__{server}__{tool}).
    qualified_name: String,
    /// Original tool name on the MCP server.
    tool_name: String,
    /// Server name (for display; used by TUI rendering).
    #[allow(dead_code)]
    server_name: String,
    /// Shared MCP client connection.
    client: Arc<McpClient>,
    /// Trust level from config.
    trust: McpTrust,
    /// Tool annotations from the MCP server.
    annotations: Option<McpToolAnnotations>,
}

impl McpToolHandler {
    pub fn new(
        qualified_name: String,
        tool_name: String,
        server_name: String,
        client: Arc<McpClient>,
        trust: McpTrust,
        annotations: Option<McpToolAnnotations>,
    ) -> Self {
        Self {
            qualified_name,
            tool_name,
            server_name,
            client,
            trust,
            annotations,
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for McpToolHandler {
    fn name(&self) -> &str {
        &self.qualified_name
    }

    fn requires_approval(&self) -> bool {
        check_mcp_approval(self.trust, self.annotations.as_ref())
    }

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<ToolResult, ToolError> {
        self.client
            .call_tool(&self.tool_name, args)
            .await
            .map_err(|e| ToolError::Execution(e.to_string()))
    }
}

/// Determine whether an MCP tool requires approval based on trust level
/// and tool annotations.
///
/// Rules:
/// - `trust = Auto` → always auto (no approval)
/// - `trust = Confirm` + `read_only_hint = true` → auto
/// - `trust = Confirm` + `destructive_hint = true` → requires approval
/// - `trust = Confirm` + no clear signal → requires approval (safe default)
pub(crate) fn check_mcp_approval(
    trust: McpTrust,
    annotations: Option<&McpToolAnnotations>,
) -> bool {
    match trust {
        McpTrust::Auto => false,
        McpTrust::Confirm => match annotations {
            Some(a) if a.read_only_hint == Some(true) => false,
            Some(a) if a.destructive_hint == Some(true) => true,
            _ => true,
        },
    }
}
