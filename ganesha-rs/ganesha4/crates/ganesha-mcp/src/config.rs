//! # MCP Configuration
//!
//! Configuration types for MCP servers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};

use crate::McpProtocolError;
use crate::types::Result;

/// MCP configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpConfig {
    /// Server configurations by ID
    #[serde(default)]
    pub servers: HashMap<String, ServerConfig>,
}

impl McpConfig {
    /// Load config from a file
    pub async fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)
            .await
            .map_err(|e| McpProtocolError::ConfigError(format!("Failed to read config: {}", e)))?;

        let config: McpConfig = if path.extension().map(|e| e == "json").unwrap_or(false) {
            serde_json::from_str(&content)?
        } else {
            toml::from_str(&content)
                .map_err(|e| McpProtocolError::ConfigError(format!("TOML parse error: {}", e)))?
        };

        Ok(config)
    }

    /// Save config to a file
    pub async fn save(&self, path: &Path) -> Result<()> {
        let content = if path.extension().map(|e| e == "json").unwrap_or(false) {
            serde_json::to_string_pretty(self)?
        } else {
            toml::to_string_pretty(self)
                .map_err(|e| McpProtocolError::ConfigError(format!("TOML serialize error: {}", e)))?
        };

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(path, content).await?;
        Ok(())
    }

    /// Merge another config (other takes precedence)
    pub fn merge(&mut self, other: McpConfig) {
        for (id, config) in other.servers {
            self.servers.insert(id, config);
        }
    }

    /// Get default config paths
    pub fn default_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // User config
        if let Some(config_dir) = dirs::config_dir() {
            paths.push(config_dir.join("ganesha").join("mcp.toml"));
        }

        // Project config
        paths.push(PathBuf::from(".ganesha").join("mcp.toml"));

        paths
    }
}

/// Configuration for a single MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Display name
    pub name: String,

    /// Transport type
    pub transport: TransportConfig,

    /// Whether the server is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Whether to auto-connect on startup
    #[serde(default)]
    pub auto_connect: bool,

    /// Whether to trust this server (auto-approve tool calls)
    #[serde(default)]
    pub trusted: bool,

    /// Tool name whitelist (if set, only these tools are exposed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_tools: Option<Vec<String>>,

    /// Tool name blacklist
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_tools: Option<Vec<String>>,

    /// Timeout for requests in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,

    /// Environment variables required (for prompting)
    #[serde(default)]
    pub required_env: Vec<String>,

    /// Description of what this server provides
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_timeout() -> u64 {
    30
}

/// Transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TransportConfig {
    /// Stdio (local process)
    Stdio {
        /// Command to run
        command: String,
        /// Arguments
        #[serde(default)]
        args: Vec<String>,
        /// Environment variables
        #[serde(default)]
        env: HashMap<String, String>,
        /// Working directory
        #[serde(skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
    },

    /// SSE (Server-Sent Events)
    Sse {
        /// URL of the SSE endpoint
        url: String,
        /// Authorization header
        #[serde(skip_serializing_if = "Option::is_none")]
        auth: Option<String>,
    },

    /// HTTP (stateless)
    Http {
        /// URL of the HTTP endpoint
        url: String,
        /// Headers to include
        #[serde(default)]
        headers: HashMap<String, String>,
    },
}

impl ServerConfig {
    /// Create a stdio server config
    pub fn stdio(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            transport: TransportConfig::Stdio {
                command: command.into(),
                args: Vec::new(),
                env: HashMap::new(),
                cwd: None,
            },
            enabled: true,
            auto_connect: false,
            trusted: false,
            include_tools: None,
            exclude_tools: None,
            timeout: 30,
            required_env: Vec::new(),
            description: None,
        }
    }

    /// Create an SSE server config
    pub fn sse(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            transport: TransportConfig::Sse {
                url: url.into(),
                auth: None,
            },
            enabled: true,
            auto_connect: false,
            trusted: false,
            include_tools: None,
            exclude_tools: None,
            timeout: 30,
            required_env: Vec::new(),
            description: None,
        }
    }

    /// Check if a tool should be included based on filters
    pub fn should_include_tool(&self, tool_name: &str) -> bool {
        // Check exclude list first
        if let Some(exclude) = &self.exclude_tools {
            if exclude.iter().any(|e| e == tool_name) {
                return false;
            }
        }

        // Check include list
        if let Some(include) = &self.include_tools {
            return include.iter().any(|i| i == tool_name);
        }

        true
    }

    /// Get missing required environment variables
    pub fn missing_env_vars(&self) -> Vec<String> {
        self.required_env
            .iter()
            .filter(|var| std::env::var(var).is_err())
            .cloned()
            .collect()
    }
}

/// Common MCP server presets
pub mod presets {
    use super::*;

    /// Filesystem MCP server
    pub fn filesystem(allowed_dirs: Vec<String>) -> ServerConfig {
        ServerConfig {
            name: "Filesystem".to_string(),
            transport: TransportConfig::Stdio {
                command: "npx".to_string(),
                args: vec![
                    "-y".to_string(),
                    "@modelcontextprotocol/server-filesystem".to_string(),
                ]
                .into_iter()
                .chain(allowed_dirs)
                .collect(),
                env: HashMap::new(),
                cwd: None,
            },
            enabled: true,
            auto_connect: true,
            trusted: false,
            include_tools: None,
            exclude_tools: None,
            timeout: 30,
            required_env: Vec::new(),
            description: Some("Access to local filesystem".to_string()),
        }
    }

    /// GitHub MCP server
    pub fn github() -> ServerConfig {
        ServerConfig {
            name: "GitHub".to_string(),
            transport: TransportConfig::Stdio {
                command: "npx".to_string(),
                args: vec![
                    "-y".to_string(),
                    "@modelcontextprotocol/server-github".to_string(),
                ],
                env: HashMap::new(),
                cwd: None,
            },
            enabled: true,
            auto_connect: false,
            trusted: false,
            include_tools: None,
            exclude_tools: None,
            timeout: 30,
            required_env: vec!["GITHUB_PERSONAL_ACCESS_TOKEN".to_string()],
            description: Some("GitHub repository access".to_string()),
        }
    }

    /// Brave Search MCP server
    pub fn brave_search() -> ServerConfig {
        ServerConfig {
            name: "Brave Search".to_string(),
            transport: TransportConfig::Stdio {
                command: "npx".to_string(),
                args: vec![
                    "-y".to_string(),
                    "@anthropics/mcp-server-brave-search".to_string(),
                ],
                env: HashMap::new(),
                cwd: None,
            },
            enabled: true,
            auto_connect: false,
            trusted: false,
            include_tools: None,
            exclude_tools: None,
            timeout: 30,
            required_env: vec!["BRAVE_API_KEY".to_string()],
            description: Some("Web search via Brave".to_string()),
        }
    }

    /// Fetch (web content) MCP server
    pub fn fetch() -> ServerConfig {
        ServerConfig {
            name: "Fetch".to_string(),
            transport: TransportConfig::Stdio {
                command: "npx".to_string(),
                args: vec![
                    "-y".to_string(),
                    "@anthropics/mcp-server-fetch".to_string(),
                ],
                env: HashMap::new(),
                cwd: None,
            },
            enabled: true,
            auto_connect: true,
            trusted: true, // Read-only, safe to trust
            include_tools: None,
            exclude_tools: None,
            timeout: 60, // Web requests may take longer
            required_env: Vec::new(),
            description: Some("Fetch web content".to_string()),
        }
    }
}
