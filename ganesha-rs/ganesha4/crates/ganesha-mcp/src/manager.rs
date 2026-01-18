//! # MCP Manager
//!
//! Manages multiple MCP servers with hot-loading support.

use crate::config::{McpConfig, ServerConfig, TransportConfig};
use crate::server::{McpServer, ServerStatus};
use crate::transport::{HttpTransport, SseTransport, StdioTransport, Transport};
use crate::types::{Result, Tool, ToolCallRequest, ToolCallResponse};
use crate::McpProtocolError;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Manages multiple MCP servers
pub struct McpManager {
    /// Connected servers by ID
    servers: RwLock<HashMap<String, Arc<McpServer>>>,
    /// Configuration
    config: RwLock<McpConfig>,
    /// Callback for requesting credentials
    credential_handler: Option<Box<dyn CredentialHandler>>,
}

/// Handler for requesting credentials from the user
pub trait CredentialHandler: Send + Sync {
    /// Request a credential value
    fn request_credential(
        &self,
        name: &str,
        description: &str,
        obtain_url: Option<&str>,
    ) -> Option<String>;
}

impl McpManager {
    /// Create a new MCP manager
    pub fn new() -> Self {
        Self {
            servers: RwLock::new(HashMap::new()),
            config: RwLock::new(McpConfig::default()),
            credential_handler: None,
        }
    }

    /// Set a credential handler for prompting users
    pub fn with_credential_handler(mut self, handler: impl CredentialHandler + 'static) -> Self {
        self.credential_handler = Some(Box::new(handler));
        self
    }

    /// Load configuration from files
    pub async fn load_config(&self) -> Result<()> {
        let paths = McpConfig::default_paths();
        let mut merged_config = McpConfig::default();

        for path in paths {
            if path.exists() {
                info!("Loading MCP config from {:?}", path);
                match McpConfig::load(&path).await {
                    Ok(config) => merged_config.merge(config),
                    Err(e) => warn!("Failed to load config from {:?}: {}", path, e),
                }
            }
        }

        *self.config.write().await = merged_config;
        Ok(())
    }

    /// Load configuration from a specific path
    pub async fn load_config_from(&self, path: &Path) -> Result<()> {
        let config = McpConfig::load(path).await?;
        self.config.write().await.merge(config);
        Ok(())
    }

    /// Save current configuration
    pub async fn save_config(&self, path: &Path) -> Result<()> {
        let config = self.config.read().await;
        config.save(path).await
    }

    /// Add a server configuration
    pub async fn add_server_config(&self, id: &str, config: ServerConfig) {
        self.config
            .write()
            .await
            .servers
            .insert(id.to_string(), config);
    }

    /// Remove a server configuration
    pub async fn remove_server_config(&self, id: &str) -> Option<ServerConfig> {
        self.config.write().await.servers.remove(id)
    }

    /// Connect to all configured servers that have auto_connect enabled
    pub async fn auto_connect(&self) -> Result<()> {
        let config = self.config.read().await;
        let auto_connect: Vec<_> = config
            .servers
            .iter()
            .filter(|(_, c)| c.enabled && c.auto_connect)
            .map(|(id, _)| id.clone())
            .collect();
        drop(config);

        for id in auto_connect {
            if let Err(e) = self.connect(&id).await {
                warn!("Failed to auto-connect to {}: {}", id, e);
            }
        }

        Ok(())
    }

    /// Connect to a server by ID
    pub async fn connect(&self, id: &str) -> Result<()> {
        let config = self.config.read().await;
        let server_config = config.servers.get(id).ok_or_else(|| {
            McpProtocolError::ServerNotFound(format!("Server '{}' not in config", id))
        })?;

        if !server_config.enabled {
            return Err(McpProtocolError::ConfigError(format!(
                "Server '{}' is disabled",
                id
            )));
        }

        // Check for missing environment variables
        let missing = server_config.missing_env_vars();
        if !missing.is_empty() {
            // Try to get credentials via handler
            if let Some(handler) = &self.credential_handler {
                for var in &missing {
                    if let Some(value) = handler.request_credential(var, "", None) {
                        std::env::set_var(var, value);
                    }
                }
            }

            // Check again
            let still_missing = server_config.missing_env_vars();
            if !still_missing.is_empty() {
                return Err(McpProtocolError::AuthRequired(format!(
                    "Missing environment variables: {}",
                    still_missing.join(", ")
                )));
            }
        }

        let server_config = server_config.clone();
        drop(config);

        info!("Connecting to MCP server: {}", id);

        // Create transport based on config
        let transport: Arc<dyn Transport> = match &server_config.transport {
            TransportConfig::Stdio {
                command,
                args,
                env,
                cwd,
            } => {
                let transport = StdioTransport::spawn(
                    command,
                    args,
                    env,
                    cwd.as_deref(),
                )
                .await?;
                Arc::new(transport)
            }
            TransportConfig::Sse { url, auth } => {
                let transport = if let Some(auth) = auth {
                    SseTransport::with_auth(url, auth)
                } else {
                    SseTransport::new(url)
                };
                Arc::new(transport)
            }
            TransportConfig::Http { url, headers: _ } => {
                Arc::new(HttpTransport::new(url))
            }
        };

        // Create and initialize server
        let mut server = McpServer::new(id, &server_config.name, transport);
        server.set_trusted(server_config.trusted);
        server.initialize().await?;

        // Store server
        self.servers
            .write()
            .await
            .insert(id.to_string(), Arc::new(server));

        info!("Connected to MCP server: {}", id);
        Ok(())
    }

    /// Disconnect from a server
    pub async fn disconnect(&self, id: &str) -> Result<()> {
        if let Some(server) = self.servers.write().await.remove(id) {
            server.disconnect().await?;
            info!("Disconnected from MCP server: {}", id);
        }
        Ok(())
    }

    /// Disconnect from all servers
    pub async fn disconnect_all(&self) -> Result<()> {
        let servers: Vec<_> = self.servers.write().await.drain().collect();
        for (id, server) in servers {
            if let Err(e) = server.disconnect().await {
                warn!("Error disconnecting from {}: {}", id, e);
            }
        }
        Ok(())
    }

    /// Get a connected server
    pub async fn get_server(&self, id: &str) -> Option<Arc<McpServer>> {
        self.servers.read().await.get(id).cloned()
    }

    /// List all connected servers
    pub async fn list_connected(&self) -> Vec<String> {
        self.servers.read().await.keys().cloned().collect()
    }

    /// List all configured servers
    pub async fn list_configured(&self) -> Vec<(String, ServerConfig)> {
        self.config
            .read()
            .await
            .servers
            .iter()
            .map(|(id, config)| (id.clone(), config.clone()))
            .collect()
    }

    /// Get all available tools from all connected servers
    pub async fn list_tools(&self) -> Vec<(String, Tool)> {
        let servers = self.servers.read().await;
        let config = self.config.read().await;
        let mut tools = Vec::new();

        for (id, server) in servers.iter() {
            let server_config = config.servers.get(id);
            let server_tools = server.tools().await;

            for tool in server_tools {
                // Apply tool filters if configured
                if let Some(cfg) = server_config {
                    if !cfg.should_include_tool(&tool.name) {
                        continue;
                    }
                }

                // Prefix tool name with server ID for disambiguation
                tools.push((format!("{}:{}", id, tool.name), tool));
            }
        }

        tools
    }

    /// Call a tool (format: "server_id:tool_name")
    pub async fn call_tool(&self, tool_id: &str, arguments: serde_json::Value) -> Result<ToolCallResponse> {
        let parts: Vec<&str> = tool_id.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(McpProtocolError::InvalidRequest(format!(
                "Invalid tool ID format: {} (expected 'server:tool')",
                tool_id
            )));
        }

        let server_id = parts[0];
        let tool_name = parts[1];

        let servers = self.servers.read().await;
        let server = servers.get(server_id).ok_or_else(|| {
            McpProtocolError::ServerNotConnected(format!("Server '{}' not connected", server_id))
        })?;

        // Check if tool exists
        if !server.has_tool(tool_name).await {
            return Err(McpProtocolError::ToolNotFound(format!(
                "Tool '{}' not found on server '{}'",
                tool_name, server_id
            )));
        }

        let request = ToolCallRequest::new(tool_name, arguments);
        server.call_tool(request).await
    }

    /// Find which server has a specific tool
    pub async fn find_tool(&self, tool_name: &str) -> Option<(String, Tool)> {
        let servers = self.servers.read().await;

        for (id, server) in servers.iter() {
            let tools = server.tools().await;
            if let Some(tool) = tools.into_iter().find(|t| t.name == tool_name) {
                return Some((id.clone(), tool));
            }
        }

        None
    }

    /// Get server status
    pub async fn server_status(&self, id: &str) -> Option<ServerStatus> {
        if let Some(server) = self.servers.read().await.get(id) {
            Some(server.status().await)
        } else {
            None
        }
    }
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_manager_creation() {
        let manager = McpManager::new();
        assert!(manager.list_connected().await.is_empty());
    }
}
