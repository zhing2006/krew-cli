//! MCP server lifecycle manager.

use std::collections::HashMap;
use std::sync::Arc;

use tracing::{error, info, warn};

use super::client::{McpClient, McpToolInfo};
use super::handler::McpToolHandler;
use super::{display_name, qualified_name};
use crate::{ToolRegistry, ToolSpec};
use krew_config::{McpServerConfig, McpTrust};

/// Discovered tools from a single MCP server.
struct ServerTools {
    name: String,
    trust: McpTrust,
    client: Arc<McpClient>,
    tools: Vec<McpToolInfo>,
}

/// Manages the lifecycle of multiple MCP server connections.
pub struct McpManager {
    /// Active server connections with their discovered tools.
    servers: Vec<ServerTools>,
}

impl McpManager {
    /// Start all configured MCP servers concurrently and discover their tools.
    ///
    /// Servers that fail to start are logged and skipped — they do not block
    /// other servers or the session. Call `register_tools()` afterwards to
    /// register discovered tools into agent registries.
    pub async fn start_all(configs: &[McpServerConfig]) -> Self {
        let mut servers = Vec::new();

        if configs.is_empty() {
            return Self { servers };
        }

        // Start all servers concurrently.
        let futs: Vec<_> = configs
            .iter()
            .map(|config| {
                let config = config.clone();
                async move {
                    let env = expand_env(&config.env);
                    let result = McpClient::connect(&config.command, &config.args, &env).await;
                    (config, result)
                }
            })
            .collect();

        let results = futures::future::join_all(futs).await;

        for (config, result) in results {
            match result {
                Ok(client) => {
                    let client: Arc<McpClient> = Arc::new(client);
                    let trust = config.trust.unwrap_or_default();

                    // Discover tools from this server.
                    match client.list_tools().await {
                        Ok(tools) => {
                            info!(
                                "MCP server '{}' started: {} tool(s) discovered",
                                config.name,
                                tools.len()
                            );
                            servers.push(ServerTools {
                                name: config.name.clone(),
                                trust,
                                client,
                                tools,
                            });
                        }
                        Err(e) => {
                            error!(
                                "MCP server '{}' started but list_tools failed: {e}",
                                config.name
                            );
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to start MCP server '{}': {e}", config.name);
                }
            }
        }

        Self { servers }
    }

    /// Register all discovered MCP tools into a ToolRegistry.
    ///
    /// Returns the total number of tools registered.
    pub fn register_tools(&self, registry: &mut ToolRegistry) -> usize {
        let mut count = 0;
        for server in &self.servers {
            count += register_mcp_tools(
                &server.name,
                server.trust,
                &server.tools,
                &server.client,
                registry,
            );
        }
        count
    }

    /// Shut down all MCP server connections.
    ///
    /// Drops all client references, which triggers child process cleanup
    /// via `kill_on_drop`.
    pub fn shutdown(&mut self) {
        let count = self.servers.len();
        self.servers.clear();
        if count > 0 {
            info!("Shut down {count} MCP server(s)");
        }
    }

    /// Get the number of active MCP server connections.
    pub fn server_count(&self) -> usize {
        self.servers.len()
    }
}

impl Drop for McpManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Register discovered MCP tools from one server into a ToolRegistry.
///
/// Returns the number of tools successfully registered.
fn register_mcp_tools(
    server_name: &str,
    trust: McpTrust,
    tools: &[McpToolInfo],
    client: &Arc<McpClient>,
    registry: &mut ToolRegistry,
) -> usize {
    let mut count = 0;

    for tool in tools {
        let qname = qualified_name(server_name, &tool.name);
        let dname = display_name(server_name, &tool.name);

        // Check for duplicate qualified names.
        if registry.specs().iter().any(|s| s.name == qname) {
            warn!(
                "Skipping duplicate MCP tool '{}' from server '{}'",
                tool.name, server_name
            );
            continue;
        }

        let description = format!("[{dname}] {}", tool.description);

        let spec = ToolSpec {
            name: qname.clone(),
            description,
            parameters: tool.input_schema.clone(),
        };

        let handler = McpToolHandler::new(
            qname,
            tool.name.clone(),
            server_name.to_string(),
            Arc::clone(client),
            trust,
            tool.annotations.clone(),
        );

        registry.register(spec, Box::new(handler));
        count += 1;
    }

    count
}

/// Expand environment variable references in the config env map.
///
/// Values starting with `$` are resolved from the process environment.
pub(crate) fn expand_env(env: &Option<HashMap<String, String>>) -> Vec<(String, String)> {
    let Some(env_map) = env else {
        return Vec::new();
    };

    env_map
        .iter()
        .map(|(key, value)| {
            let resolved = if let Some(var_name) = value.strip_prefix('$') {
                match std::env::var(var_name) {
                    Ok(val) => val,
                    Err(_) => {
                        warn!(
                            "MCP env var '{}' references undefined env var '{}'",
                            key, var_name
                        );
                        String::new()
                    }
                }
            } else {
                value.clone()
            };
            (key.clone(), resolved)
        })
        .collect()
}
