//! MCP client wrapping the rmcp SDK for stdio and HTTP transports.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, anyhow};
use rmcp::model::CallToolRequestParams;
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::child_process::TokioChildProcess;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::{ClientHandler, service};
use serde_json::Value;
use tokio::process::Command;

use crate::ToolResult;

/// Default timeout for MCP server initialization handshake.
const INIT_TIMEOUT: Duration = Duration::from_secs(10);

/// Annotations extracted from an MCP tool.
#[derive(Debug, Clone)]
pub struct McpToolAnnotations {
    pub destructive_hint: Option<bool>,
    pub read_only_hint: Option<bool>,
    pub open_world_hint: Option<bool>,
    pub idempotent_hint: Option<bool>,
}

/// Information about a tool discovered from an MCP server.
#[derive(Debug, Clone)]
pub struct McpToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub annotations: Option<McpToolAnnotations>,
}

/// MCP client connected to a single MCP server via stdio transport.
pub struct McpClient {
    service: Arc<RunningService<RoleClient, McpClientHandler>>,
}

impl McpClient {
    /// Connect to an MCP server by spawning a child process.
    ///
    /// Performs the MCP handshake (initialize) and returns a connected client.
    pub async fn connect(command: &str, args: &[String], env: &[(String, String)]) -> Result<Self> {
        // Build the child process command.
        let mut cmd = Command::new(command);
        cmd.kill_on_drop(true)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .args(args);

        // Add environment variables.
        for (k, v) in env {
            cmd.env(k, v);
        }

        // Spawn the transport (takes ownership of the Command).
        let transport = TokioChildProcess::new(cmd)
            .map_err(|e| anyhow!("failed to spawn MCP server '{command}': {e}"))?;

        // Create the client handler.
        let handler = McpClientHandler;

        // Perform MCP handshake with timeout.
        let service = tokio::time::timeout(INIT_TIMEOUT, service::serve_client(handler, transport))
            .await
            .map_err(|_| {
                anyhow!(
                    "MCP server '{command}' did not respond within {} seconds. \
                     Check that the command is correct and the server starts quickly.",
                    INIT_TIMEOUT.as_secs()
                )
            })?
            .map_err(|e| anyhow!("MCP handshake with '{command}' failed: {e}"))?;

        Ok(Self {
            service: Arc::new(service),
        })
    }

    /// Connect to an MCP server via Streamable HTTP transport.
    pub async fn connect_http(url: &str, headers: &HashMap<String, String>) -> Result<Self> {
        use rmcp::transport::StreamableHttpClientTransport;

        let mut config = StreamableHttpClientTransportConfig::with_uri(url);

        // Map custom headers (using types re-exported by rmcp's reqwest).
        let mut header_map = HashMap::new();
        for (k, v) in headers {
            let name: http::HeaderName = k
                .parse()
                .map_err(|e| anyhow!("invalid header name '{k}': {e}"))?;
            let value: http::HeaderValue = v
                .parse()
                .map_err(|e| anyhow!("invalid header value for '{k}': {e}"))?;
            header_map.insert(name, value);
        }
        if !header_map.is_empty() {
            config = config.custom_headers(header_map);
        }

        let transport = StreamableHttpClientTransport::from_config(config);
        let handler = McpClientHandler;

        let service = tokio::time::timeout(INIT_TIMEOUT, service::serve_client(handler, transport))
            .await
            .map_err(|_| {
                anyhow!(
                    "MCP server '{url}' did not respond within {} seconds.",
                    INIT_TIMEOUT.as_secs()
                )
            })?
            .map_err(|e| anyhow!("MCP handshake with '{url}' failed: {e}"))?;

        Ok(Self {
            service: Arc::new(service),
        })
    }

    /// Discover all tools provided by this MCP server.
    pub async fn list_tools(&self) -> Result<Vec<McpToolInfo>> {
        let result = self
            .service
            .list_tools(None)
            .await
            .map_err(|e| anyhow!("list_tools failed: {e}"))?;

        let tools = result
            .tools
            .into_iter()
            .map(|tool| {
                let annotations = tool.annotations.as_ref().map(|a| McpToolAnnotations {
                    destructive_hint: a.destructive_hint,
                    read_only_hint: a.read_only_hint,
                    open_world_hint: a.open_world_hint,
                    idempotent_hint: a.idempotent_hint,
                });

                let input_schema = serde_json::to_value(tool.input_schema.as_ref())
                    .unwrap_or(Value::Object(serde_json::Map::new()));

                McpToolInfo {
                    name: tool.name.to_string(),
                    description: tool.description.as_deref().unwrap_or("").to_string(),
                    input_schema,
                    annotations,
                }
            })
            .collect();

        Ok(tools)
    }

    /// Call a tool on this MCP server.
    pub async fn call_tool(&self, tool_name: &str, arguments: Value) -> Result<ToolResult> {
        let args_map = match arguments {
            Value::Object(map) => Some(map),
            Value::Null => None,
            other => {
                return Err(anyhow!(
                    "MCP tool arguments must be a JSON object, got: {other}"
                ));
            }
        };

        let params = CallToolRequestParams::new(tool_name.to_string())
            .with_arguments(args_map.unwrap_or_default());

        let result = self
            .service
            .call_tool(params)
            .await
            .map_err(|e| anyhow!("call_tool '{tool_name}' failed: {e}"))?;

        let is_error = result.is_error.unwrap_or(false);

        // Extract text content from the result.
        let content: String = result
            .content
            .iter()
            .filter_map(|c| c.as_text().map(|t| t.text.as_str()))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolResult { content, is_error })
    }
}

/// Minimal MCP client handler that identifies as krew-cli.
///
/// All notification methods use default no-op implementations from
/// the `ClientHandler` trait.
#[derive(Clone)]
pub(crate) struct McpClientHandler;

impl ClientHandler for McpClientHandler {}
