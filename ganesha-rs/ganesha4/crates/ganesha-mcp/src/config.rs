//! # MCP Configuration
//!
//! Configuration types for MCP servers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

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

    /// Puppeteer MCP server for browser automation
    pub fn puppeteer() -> ServerConfig {
        // Set up environment variables for puppeteer
        let mut env = HashMap::new();
        // Enable --no-sandbox for Linux compatibility (AppArmor restrictions)
        env.insert("PUPPETEER_LAUNCH_ARGS".to_string(), "--no-sandbox --disable-setuid-sandbox".to_string());
        // Also set the chromium args directly
        env.insert("CHROMIUM_FLAGS".to_string(), "--no-sandbox --disable-setuid-sandbox".to_string());

        ServerConfig {
            name: "Puppeteer".to_string(),
            transport: TransportConfig::Stdio {
                command: "npx".to_string(),
                args: vec![
                    "-y".to_string(),
                    "@modelcontextprotocol/server-puppeteer".to_string(),
                ],
                env,
                cwd: None,
            },
            enabled: true,
            auto_connect: true,
            trusted: false, // Browser automation requires approval
            include_tools: None,
            exclude_tools: None,
            timeout: 120, // Browser operations can take time
            required_env: Vec::new(),
            description: Some("Browser automation via Puppeteer".to_string()),
        }
    }

    /// Playwright MCP server - use puppeteer-mcp-server as alternative
    pub fn playwright() -> ServerConfig {
        ServerConfig {
            name: "Playwright".to_string(),
            transport: TransportConfig::Stdio {
                command: "npx".to_string(),
                args: vec![
                    "-y".to_string(),
                    "puppeteer-mcp-server".to_string(), // Alternative that's available
                ],
                env: HashMap::new(),
                cwd: None,
            },
            enabled: true,
            auto_connect: true,
            trusted: false,
            include_tools: None,
            exclude_tools: None,
            timeout: 120,
            required_env: Vec::new(),
            description: Some("Browser automation".to_string()),
        }
    }

    /// Memory MCP server for persistent knowledge
    pub fn memory() -> ServerConfig {
        ServerConfig {
            name: "Memory".to_string(),
            transport: TransportConfig::Stdio {
                command: "npx".to_string(),
                args: vec![
                    "-y".to_string(),
                    "@modelcontextprotocol/server-memory".to_string(),
                ],
                env: HashMap::new(),
                cwd: None,
            },
            enabled: true,
            auto_connect: true,
            trusted: true,
            include_tools: None,
            exclude_tools: None,
            timeout: 30,
            required_env: Vec::new(),
            description: Some("Persistent knowledge storage".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_stdio() {
        let config = ServerConfig::stdio("test", "echo");
        assert_eq!(config.name, "test");
        assert!(config.enabled);
        assert!(!config.trusted);
        assert_eq!(config.timeout, 30);
        match config.transport {
            TransportConfig::Stdio { command, .. } => assert_eq!(command, "echo"),
            _ => panic!("Expected Stdio transport"),
        }
    }

    #[test]
    fn test_server_config_sse() {
        let config = ServerConfig::sse("remote", "http://localhost:8080/sse");
        assert_eq!(config.name, "remote");
        match config.transport {
            TransportConfig::Sse { url, .. } => assert_eq!(url, "http://localhost:8080/sse"),
            _ => panic!("Expected SSE transport"),
        }
    }

    #[test]
    fn test_tool_filter_include() {
        let mut config = ServerConfig::stdio("test", "echo");
        config.include_tools = Some(vec!["read".to_string(), "write".to_string()]);
        
        assert!(config.should_include_tool("read"));
        assert!(config.should_include_tool("write"));
        assert!(!config.should_include_tool("delete"));
    }

    #[test]
    fn test_tool_filter_exclude() {
        let mut config = ServerConfig::stdio("test", "echo");
        config.exclude_tools = Some(vec!["dangerous_tool".to_string()]);
        
        assert!(config.should_include_tool("read"));
        assert!(!config.should_include_tool("dangerous_tool"));
    }

    #[test]
    fn test_tool_filter_exclude_takes_precedence() {
        let mut config = ServerConfig::stdio("test", "echo");
        config.include_tools = Some(vec!["tool_a".to_string(), "tool_b".to_string()]);
        config.exclude_tools = Some(vec!["tool_b".to_string()]);
        
        assert!(config.should_include_tool("tool_a"));
        assert!(!config.should_include_tool("tool_b")); // excluded even though included
    }

    #[test]
    fn test_tool_filter_no_filters() {
        let config = ServerConfig::stdio("test", "echo");
        assert!(config.should_include_tool("anything"));
        assert!(config.should_include_tool("everything"));
    }

    #[test]
    fn test_presets_filesystem() {
        let config = presets::filesystem(vec!["/tmp".to_string()]);
        assert_eq!(config.name, "Filesystem");
        assert!(config.auto_connect);
        assert!(!config.trusted);
    }

    #[test]
    fn test_presets_github() {
        let config = presets::github();
        assert_eq!(config.name, "GitHub");
        assert!(config.required_env.contains(&"GITHUB_PERSONAL_ACCESS_TOKEN".to_string()));
    }

    #[test]
    fn test_presets_brave_search() {
        let config = presets::brave_search();
        assert_eq!(config.name, "Brave Search");
        assert!(config.required_env.contains(&"BRAVE_API_KEY".to_string()));
    }

    #[test]
    fn test_presets_puppeteer() {
        let config = presets::puppeteer();
        assert_eq!(config.name, "Puppeteer");
        assert_eq!(config.timeout, 120); // longer timeout for browser
        assert!(config.auto_connect);
    }

    #[test]
    fn test_presets_memory() {
        let config = presets::memory();
        assert_eq!(config.name, "Memory");
        assert!(config.trusted); // memory server is trusted
        assert!(config.auto_connect);
    }

    #[test]
    fn test_mcp_config_merge() {
        let mut config = McpConfig::default();
        config.servers.insert("a".to_string(), ServerConfig::stdio("Server A", "cmd_a"));
        
        let mut other = McpConfig::default();
        other.servers.insert("b".to_string(), ServerConfig::stdio("Server B", "cmd_b"));
        other.servers.insert("a".to_string(), ServerConfig::stdio("Server A Override", "cmd_a2"));
        
        config.merge(other);
        assert_eq!(config.servers.len(), 2);
        assert_eq!(config.servers["a"].name, "Server A Override");
        assert_eq!(config.servers["b"].name, "Server B");
    }

    #[test]
    fn test_mcp_config_serialization() {
        let mut config = McpConfig::default();
        config.servers.insert("test".to_string(), ServerConfig::stdio("Test", "echo"));
        
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: McpConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.servers.len(), 1);
        assert_eq!(deserialized.servers["test"].name, "Test");
    }

    #[test]
    fn test_transport_config_serialization() {
        // Stdio
        let stdio = TransportConfig::Stdio {
            command: "node".to_string(),
            args: vec!["server.js".to_string()],
            env: HashMap::new(),
            cwd: None,
        };
        let json = serde_json::to_string(&stdio).unwrap();
        assert!(json.contains("\"type\":\"stdio\""));
        
        // SSE
        let sse = TransportConfig::Sse {
            url: "http://localhost:8080".to_string(),
            auth: Some("Bearer token123".to_string()),
        };
        let json = serde_json::to_string(&sse).unwrap();
        assert!(json.contains("\"type\":\"sse\""));
    }
}
