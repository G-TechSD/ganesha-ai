//! MCP Server Manager
//!
//! Manages Model Context Protocol servers for enhanced capabilities.
//! Default servers: context7, playwright, linear, n8n, desktop-commander
//!
//! MCP servers are GLOBAL - not per-project. They enhance Ganesha's capabilities
//! across all sessions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::fs;

/// MCP Server definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServer {
    pub name: String,
    pub description: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub status: ServerStatus,
    pub auto_start: bool,
    pub category: ServerCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerStatus {
    Stopped,
    Starting,
    Running,
    Failed,
    NotInstalled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerCategory {
    Documentation,  // context7
    Browser,        // playwright
    ProjectMgmt,    // linear
    Automation,     // n8n
    System,         // desktop-commander
    Custom,
}

/// MCP Server Manager
pub struct McpManager {
    config_path: PathBuf,
    servers: HashMap<String, McpServer>,
    running: HashMap<String, Child>,
}

impl McpManager {
    pub fn new() -> Self {
        let config_path = Self::get_config_path();
        let servers = Self::load_config(&config_path);

        Self {
            config_path,
            servers,
            running: HashMap::new(),
        }
    }

    fn get_config_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".ganesha").join("mcp_servers.json")
    }

    fn load_config(path: &PathBuf) -> HashMap<String, McpServer> {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(servers) = serde_json::from_str(&content) {
                    return servers;
                }
            }
        }

        // Return default servers
        Self::default_servers()
    }

    /// Default MCP servers that should be available
    fn default_servers() -> HashMap<String, McpServer> {
        let mut servers = HashMap::new();

        // Context7 - Documentation and library knowledge
        servers.insert("context7".into(), McpServer {
            name: "context7".into(),
            description: "Library documentation and API knowledge".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@anthropic/mcp-server-context7".into()],
            env: HashMap::new(),
            status: ServerStatus::NotInstalled,
            auto_start: true,
            category: ServerCategory::Documentation,
        });

        // Playwright - Browser automation
        servers.insert("playwright".into(), McpServer {
            name: "playwright".into(),
            description: "Browser automation and web scraping".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@anthropic/mcp-server-playwright".into()],
            env: HashMap::new(),
            status: ServerStatus::NotInstalled,
            auto_start: true,
            category: ServerCategory::Browser,
        });

        // Desktop Commander - System control
        servers.insert("desktop-commander".into(), McpServer {
            name: "desktop-commander".into(),
            description: "Desktop automation and system control".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@anthropic/mcp-server-desktop-commander".into()],
            env: HashMap::new(),
            status: ServerStatus::NotInstalled,
            auto_start: true,
            category: ServerCategory::System,
        });

        // Filesystem - Enhanced file operations
        servers.insert("filesystem".into(), McpServer {
            name: "filesystem".into(),
            description: "Enhanced filesystem operations".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@modelcontextprotocol/server-filesystem".into(), "/".into()],
            env: HashMap::new(),
            status: ServerStatus::NotInstalled,
            auto_start: true,
            category: ServerCategory::System,
        });

        // Memory - Persistent knowledge graph
        servers.insert("memory".into(), McpServer {
            name: "memory".into(),
            description: "Persistent memory and knowledge graph".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@modelcontextprotocol/server-memory".into()],
            env: HashMap::new(),
            status: ServerStatus::NotInstalled,
            auto_start: true,
            category: ServerCategory::System,
        });

        // Fetch - Web fetching
        servers.insert("fetch".into(), McpServer {
            name: "fetch".into(),
            description: "Web content fetching and processing".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@modelcontextprotocol/server-fetch".into()],
            env: HashMap::new(),
            status: ServerStatus::NotInstalled,
            auto_start: false,
            category: ServerCategory::Browser,
        });

        // Git - Git operations
        servers.insert("git".into(), McpServer {
            name: "git".into(),
            description: "Git repository operations".into(),
            command: "uvx".into(),
            args: vec!["mcp-server-git".into()],
            env: HashMap::new(),
            status: ServerStatus::NotInstalled,
            auto_start: true,
            category: ServerCategory::System,
        });

        servers
    }

    /// Save configuration
    pub fn save_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&self.servers)?;
        fs::write(&self.config_path, content)?;
        Ok(())
    }

    /// List all servers
    pub fn list_servers(&self) -> Vec<&McpServer> {
        self.servers.values().collect()
    }

    /// Get a specific server
    pub fn get_server(&self, name: &str) -> Option<&McpServer> {
        self.servers.get(name)
    }

    /// Check if a server is installed
    pub fn is_installed(&self, name: &str) -> bool {
        if let Some(server) = self.servers.get(name) {
            server.status != ServerStatus::NotInstalled
        } else {
            false
        }
    }

    /// Install a server
    pub async fn install_server(&mut self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let server = self.servers.get(name)
            .ok_or_else(|| format!("Server {} not found", name))?
            .clone();

        println!("Installing MCP server: {}", name);

        // For npx-based servers, we just need to verify npm/npx is available
        if server.command == "npx" {
            let output = Command::new("npx")
                .arg("--version")
                .output()?;

            if !output.status.success() {
                return Err("npx not found. Please install Node.js".into());
            }

            // Try to install/cache the package
            let mut args = vec!["--yes".to_string()];
            args.extend(server.args.iter().cloned());

            let output = Command::new("npx")
                .args(&args)
                .arg("--help") // Just check if it works
                .output()?;

            if output.status.success() {
                if let Some(s) = self.servers.get_mut(name) {
                    s.status = ServerStatus::Stopped;
                }
                self.save_config()?;
                println!("  ✓ Installed successfully");
            } else {
                return Err(format!("Failed to install {}: {}",
                    name, String::from_utf8_lossy(&output.stderr)).into());
            }
        } else if server.command == "uvx" {
            // Python-based servers via uvx
            let output = Command::new("uvx")
                .arg("--version")
                .output();

            if output.is_err() {
                return Err("uvx not found. Please install uv: curl -LsSf https://astral.sh/uv/install.sh | sh".into());
            }

            if let Some(s) = self.servers.get_mut(name) {
                s.status = ServerStatus::Stopped;
            }
            self.save_config()?;
            println!("  ✓ Registered successfully");
        }

        Ok(())
    }

    /// Install all default servers
    pub async fn install_defaults(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let names: Vec<String> = self.servers.keys()
            .filter(|n| self.servers[*n].auto_start)
            .cloned()
            .collect();

        for name in names {
            if let Err(e) = self.install_server(&name).await {
                eprintln!("  Warning: Failed to install {}: {}", name, e);
            }
        }

        Ok(())
    }

    /// Start a server
    pub fn start_server(&mut self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let server = self.servers.get_mut(name)
            .ok_or_else(|| format!("Server {} not found", name))?;

        if server.status == ServerStatus::NotInstalled {
            return Err(format!("Server {} not installed", name).into());
        }

        if self.running.contains_key(name) {
            return Ok(()); // Already running
        }

        let mut cmd = Command::new(&server.command);
        cmd.args(&server.args);

        for (key, value) in &server.env {
            cmd.env(key, value);
        }

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        match cmd.spawn() {
            Ok(child) => {
                self.running.insert(name.to_string(), child);
                server.status = ServerStatus::Running;
                Ok(())
            }
            Err(e) => {
                server.status = ServerStatus::Failed;
                Err(format!("Failed to start {}: {}", name, e).into())
            }
        }
    }

    /// Stop a server
    pub fn stop_server(&mut self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(mut child) = self.running.remove(name) {
            child.kill()?;
            if let Some(server) = self.servers.get_mut(name) {
                server.status = ServerStatus::Stopped;
            }
        }
        Ok(())
    }

    /// Stop all servers
    pub fn stop_all(&mut self) {
        let names: Vec<String> = self.running.keys().cloned().collect();
        for name in names {
            let _ = self.stop_server(&name);
        }
    }

    /// Start all auto-start servers
    pub fn start_auto_servers(&mut self) -> Vec<String> {
        let auto_start: Vec<String> = self.servers.iter()
            .filter(|(_, s)| s.auto_start && s.status != ServerStatus::NotInstalled)
            .map(|(n, _)| n.clone())
            .collect();

        let mut started = vec![];
        for name in auto_start {
            if self.start_server(&name).is_ok() {
                started.push(name);
            }
        }
        started
    }

    /// Add a custom server
    pub fn add_server(&mut self, server: McpServer) -> Result<(), Box<dyn std::error::Error>> {
        self.servers.insert(server.name.clone(), server);
        self.save_config()?;
        Ok(())
    }

    /// Remove a server
    pub fn remove_server(&mut self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.stop_server(name)?;
        self.servers.remove(name);
        self.save_config()?;
        Ok(())
    }

    /// Get MCP configuration for Claude Code format
    pub fn get_claude_config(&self) -> serde_json::Value {
        let mut mcp_servers = serde_json::Map::new();

        for (name, server) in &self.servers {
            if server.status != ServerStatus::NotInstalled {
                mcp_servers.insert(name.clone(), serde_json::json!({
                    "command": server.command,
                    "args": server.args,
                    "env": server.env
                }));
            }
        }

        serde_json::json!({
            "mcpServers": mcp_servers
        })
    }

    /// Print server status
    pub fn print_status(&self) {
        println!("\n\x1b[1;36mMCP Server Status:\x1b[0m\n");

        let categories = [
            (ServerCategory::Documentation, "Documentation"),
            (ServerCategory::Browser, "Browser"),
            (ServerCategory::System, "System"),
            (ServerCategory::ProjectMgmt, "Project Management"),
            (ServerCategory::Automation, "Automation"),
            (ServerCategory::Custom, "Custom"),
        ];

        for (category, name) in categories {
            let servers: Vec<_> = self.servers.values()
                .filter(|s| s.category == category)
                .collect();

            if !servers.is_empty() {
                println!("  \x1b[1m{}:\x1b[0m", name);
                for server in servers {
                    let status = match server.status {
                        ServerStatus::Running => "\x1b[32m●\x1b[0m",
                        ServerStatus::Stopped => "\x1b[33m○\x1b[0m",
                        ServerStatus::Starting => "\x1b[34m◐\x1b[0m",
                        ServerStatus::Failed => "\x1b[31m✗\x1b[0m",
                        ServerStatus::NotInstalled => "\x1b[2m◌\x1b[0m",
                    };
                    println!("    {} {} - {}", status, server.name, server.description);
                }
                println!();
            }
        }
    }
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for McpManager {
    fn drop(&mut self) {
        self.stop_all();
    }
}

/// MCP Protocol message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpMessage {
    #[serde(rename = "request")]
    Request {
        id: String,
        method: String,
        params: Option<serde_json::Value>,
    },
    #[serde(rename = "response")]
    Response {
        id: String,
        result: Option<serde_json::Value>,
        error: Option<McpError>,
    },
    #[serde(rename = "notification")]
    Notification {
        method: String,
        params: Option<serde_json::Value>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
}

/// Call an MCP tool
pub async fn call_mcp_tool(
    server_name: &str,
    tool_name: &str,
    args: serde_json::Value,
    manager: &McpManager,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // For now, this is a placeholder - actual MCP protocol implementation
    // would involve JSON-RPC over stdio to the server process

    Err(format!(
        "MCP tool call not yet implemented: {}::{}",
        server_name, tool_name
    ).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_servers() {
        let servers = McpManager::default_servers();
        assert!(servers.contains_key("context7"));
        assert!(servers.contains_key("playwright"));
        assert!(servers.contains_key("desktop-commander"));
    }

    #[test]
    fn test_manager_creation() {
        let manager = McpManager::new();
        assert!(!manager.list_servers().is_empty());
    }

    #[test]
    fn test_claude_config_format() {
        let mut manager = McpManager::new();
        // Mark a server as installed
        if let Some(server) = manager.servers.get_mut("context7") {
            server.status = ServerStatus::Stopped;
        }

        let config = manager.get_claude_config();
        assert!(config.get("mcpServers").is_some());
    }
}
