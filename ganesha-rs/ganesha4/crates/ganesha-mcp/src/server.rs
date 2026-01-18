//! # MCP Server
//!
//! Represents a connected MCP server and its capabilities.

use crate::transport::Transport;
use crate::types::{
    JsonRpcRequest, Prompt, Resource, Result, Tool,
    ToolCallRequest, ToolCallResponse, ContentBlock,
};
use crate::McpProtocolError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Server connection status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerStatus {
    /// Server is disconnected
    Disconnected,
    /// Server is connecting
    Connecting,
    /// Server is connected and ready
    Connected,
    /// Server encountered an error
    Error,
}

/// Capabilities exposed by an MCP server
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerCapabilities {
    /// Available tools
    #[serde(default)]
    pub tools: Vec<Tool>,
    /// Available resources
    #[serde(default)]
    pub resources: Vec<Resource>,
    /// Available prompts
    #[serde(default)]
    pub prompts: Vec<Prompt>,
    /// Protocol version
    pub protocol_version: Option<String>,
    /// Server name
    pub name: Option<String>,
    /// Server version
    pub version: Option<String>,
}

/// An MCP server instance
pub struct McpServer {
    /// Server identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// Transport for communication
    transport: Arc<dyn Transport>,
    /// Server capabilities
    capabilities: RwLock<ServerCapabilities>,
    /// Current status
    status: RwLock<ServerStatus>,
    /// Trust level (auto-approve tool calls)
    trusted: bool,
}

impl McpServer {
    /// Create a new MCP server with a transport
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        transport: Arc<dyn Transport>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            transport,
            capabilities: RwLock::new(ServerCapabilities::default()),
            status: RwLock::new(ServerStatus::Disconnected),
            trusted: false,
        }
    }

    /// Set whether this server is trusted (auto-approve calls)
    pub fn set_trusted(&mut self, trusted: bool) {
        self.trusted = trusted;
    }

    /// Check if server is trusted
    pub fn is_trusted(&self) -> bool {
        self.trusted
    }

    /// Initialize the server connection
    pub async fn initialize(&self) -> Result<()> {
        *self.status.write().await = ServerStatus::Connecting;

        // Send initialize request
        let init_params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {},
                "resources": {},
                "prompts": {}
            },
            "clientInfo": {
                "name": "ganesha",
                "version": env!("CARGO_PKG_VERSION")
            }
        });

        let request = JsonRpcRequest::new("initialize", Some(init_params));
        let response = self.transport.request(request).await?;

        if let Some(error) = response.error {
            *self.status.write().await = ServerStatus::Error;
            return Err(McpProtocolError::InvalidResponse(format!(
                "Initialize failed: {}",
                error.message
            )));
        }

        // Parse capabilities from response
        if let Some(result) = response.result {
            let mut caps = self.capabilities.write().await;

            if let Some(name) = result.get("serverInfo").and_then(|s| s.get("name")) {
                caps.name = name.as_str().map(String::from);
            }
            if let Some(version) = result.get("serverInfo").and_then(|s| s.get("version")) {
                caps.version = version.as_str().map(String::from);
            }
            if let Some(protocol) = result.get("protocolVersion") {
                caps.protocol_version = protocol.as_str().map(String::from);
            }
        }

        // Send initialized notification
        self.transport.notify("notifications/initialized", None).await?;

        // Fetch tools, resources, prompts
        self.refresh_capabilities().await?;

        *self.status.write().await = ServerStatus::Connected;
        info!("MCP server '{}' initialized", self.name);

        Ok(())
    }

    /// Refresh server capabilities
    pub async fn refresh_capabilities(&self) -> Result<()> {
        // Get tools
        let tools_req = JsonRpcRequest::new("tools/list", None);
        if let Ok(response) = self.transport.request(tools_req).await {
            if let Some(result) = response.result {
                if let Ok(tools) = serde_json::from_value::<ToolsListResponse>(result) {
                    let mut caps = self.capabilities.write().await;
                    caps.tools = tools.tools;
                    debug!("Loaded {} tools from {}", caps.tools.len(), self.name);
                }
            }
        }

        // Get resources
        let resources_req = JsonRpcRequest::new("resources/list", None);
        if let Ok(response) = self.transport.request(resources_req).await {
            if let Some(result) = response.result {
                if let Ok(resources) = serde_json::from_value::<ResourcesListResponse>(result) {
                    let mut caps = self.capabilities.write().await;
                    caps.resources = resources.resources;
                    debug!("Loaded {} resources from {}", caps.resources.len(), self.name);
                }
            }
        }

        // Get prompts
        let prompts_req = JsonRpcRequest::new("prompts/list", None);
        if let Ok(response) = self.transport.request(prompts_req).await {
            if let Some(result) = response.result {
                if let Ok(prompts) = serde_json::from_value::<PromptsListResponse>(result) {
                    let mut caps = self.capabilities.write().await;
                    caps.prompts = prompts.prompts;
                    debug!("Loaded {} prompts from {}", caps.prompts.len(), self.name);
                }
            }
        }

        Ok(())
    }

    /// Call a tool on this server
    pub async fn call_tool(&self, request: ToolCallRequest) -> Result<ToolCallResponse> {
        let params = serde_json::json!({
            "name": request.name,
            "arguments": request.arguments
        });

        let rpc_request = JsonRpcRequest::new("tools/call", Some(params));
        let response = self.transport.request(rpc_request).await?;

        if let Some(error) = response.error {
            warn!("MCP tool error: {} - {}", error.code, error.message);
            return Ok(ToolCallResponse {
                id: request.id,
                content: None,
                error: Some(crate::types::ToolError {
                    code: error.code,
                    message: error.message,
                    data: error.data,
                }),
                is_running: false,
            });
        }

        if let Some(result) = response.result {
            let content: Vec<ContentBlock> = result
                .get("content")
                .and_then(|c| serde_json::from_value(c.clone()).ok())
                .unwrap_or_default();

            let is_running = result
                .get("isRunning")
                .and_then(|r| r.as_bool())
                .unwrap_or(false);

            return Ok(ToolCallResponse {
                id: request.id,
                content: Some(content),
                error: None,
                is_running,
            });
        }

        Ok(ToolCallResponse {
            id: request.id,
            content: Some(vec![]),
            error: None,
            is_running: false,
        })
    }

    /// Get a resource by URI
    pub async fn read_resource(&self, uri: &str) -> Result<String> {
        let params = serde_json::json!({ "uri": uri });
        let request = JsonRpcRequest::new("resources/read", Some(params));
        let response = self.transport.request(request).await?;

        if let Some(error) = response.error {
            return Err(McpProtocolError::InvalidResponse(error.message));
        }

        if let Some(result) = response.result {
            if let Some(contents) = result.get("contents") {
                if let Some(first) = contents.as_array().and_then(|a| a.first()) {
                    if let Some(text) = first.get("text").and_then(|t| t.as_str()) {
                        return Ok(text.to_string());
                    }
                }
            }
        }

        Err(McpProtocolError::InvalidResponse(
            "Invalid resource response".to_string(),
        ))
    }

    /// Get a prompt by name
    pub async fn get_prompt(
        &self,
        name: &str,
        arguments: Option<HashMap<String, String>>,
    ) -> Result<String> {
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments.unwrap_or_default()
        });

        let request = JsonRpcRequest::new("prompts/get", Some(params));
        let response = self.transport.request(request).await?;

        if let Some(error) = response.error {
            return Err(McpProtocolError::InvalidResponse(error.message));
        }

        if let Some(result) = response.result {
            if let Some(messages) = result.get("messages") {
                // Concatenate all message content
                let content: Vec<String> = messages
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|m| {
                                m.get("content")
                                    .and_then(|c| c.get("text"))
                                    .and_then(|t| t.as_str())
                                    .map(String::from)
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                return Ok(content.join("\n"));
            }
        }

        Err(McpProtocolError::InvalidResponse(
            "Invalid prompt response".to_string(),
        ))
    }

    /// Get current status
    pub async fn status(&self) -> ServerStatus {
        *self.status.read().await
    }

    /// Get capabilities
    pub async fn capabilities(&self) -> ServerCapabilities {
        self.capabilities.read().await.clone()
    }

    /// Get list of tools
    pub async fn tools(&self) -> Vec<Tool> {
        self.capabilities.read().await.tools.clone()
    }

    /// Check if a tool exists on this server
    pub async fn has_tool(&self, name: &str) -> bool {
        self.capabilities
            .read()
            .await
            .tools
            .iter()
            .any(|t| t.name == name)
    }

    /// Disconnect the server
    pub async fn disconnect(&self) -> Result<()> {
        self.transport.close().await?;
        *self.status.write().await = ServerStatus::Disconnected;
        Ok(())
    }
}

// Response types for parsing

#[derive(Debug, Deserialize)]
struct ToolsListResponse {
    #[serde(default)]
    tools: Vec<Tool>,
}

#[derive(Debug, Deserialize)]
struct ResourcesListResponse {
    #[serde(default)]
    resources: Vec<Resource>,
}

#[derive(Debug, Deserialize)]
struct PromptsListResponse {
    #[serde(default)]
    prompts: Vec<Prompt>,
}
