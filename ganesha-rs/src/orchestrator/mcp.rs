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

        // Context7 - Documentation and library knowledge (by Upstash)
        servers.insert("context7".into(), McpServer {
            name: "context7".into(),
            description: "Up-to-date library documentation for LLMs".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@upstash/context7-mcp@latest".into()],
            env: HashMap::new(),
            status: ServerStatus::NotInstalled,
            auto_start: false,  // Needs API key for best results
            category: ServerCategory::Documentation,
        });

        // Playwright - Browser automation (official Microsoft)
        servers.insert("playwright".into(), McpServer {
            name: "playwright".into(),
            description: "Browser automation via accessibility tree (no vision needed)".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@playwright/mcp@latest".into()],
            env: HashMap::new(),
            status: ServerStatus::NotInstalled,
            auto_start: true,
            category: ServerCategory::Browser,
        });

        // Playwright Execute Automation (alternative with more features)
        servers.insert("playwright-ea".into(), McpServer {
            name: "playwright-ea".into(),
            description: "Playwright with device emulation (iPhone, Android, etc)".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@executeautomation/playwright-mcp-server".into()],
            env: HashMap::new(),
            status: ServerStatus::NotInstalled,
            auto_start: false,
            category: ServerCategory::Browser,
        });

        // Filesystem - Enhanced file operations
        // Note: Path arg is added at runtime, not during install check
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        servers.insert("filesystem".into(), McpServer {
            name: "filesystem".into(),
            description: "Enhanced filesystem operations".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@modelcontextprotocol/server-filesystem".into(), home_dir],
            env: HashMap::new(),
            status: ServerStatus::NotInstalled,
            auto_start: false,  // Requires path configuration
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

        // Fetch - Web fetching (Python-based, uses uvx)
        servers.insert("fetch".into(), McpServer {
            name: "fetch".into(),
            description: "Web content fetching - simpler than browser for static pages".into(),
            command: "uvx".into(),
            args: vec!["mcp-server-fetch".into()],
            env: HashMap::new(),
            status: ServerStatus::NotInstalled,
            auto_start: true,
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


        // Desktop Commander - System control and file operations (by wonderwhy-er)
        servers.insert("desktop-commander".into(), McpServer {
            name: "desktop-commander".into(),
            description: "System-level control: terminal, file ops, process management".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@wonderwhy-er/desktop-commander@latest".into()],
            env: HashMap::new(),
            status: ServerStatus::NotInstalled,
            auto_start: false,
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

    /// Install a server (synchronous - runs shell commands)
    pub fn install_server(&mut self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let server = self.servers.get(name)
            .ok_or_else(|| format!("Server {} not found", name))?
            .clone();

        println!("Installing MCP server: {}", name);

        // For npx-based servers, we need to verify npm/npx is available
        if server.command == "npx" {
            let output = Command::new("npx")
                .arg("--version")
                .output()?;

            if !output.status.success() {
                return Err("npx not found. Please install Node.js first:\n  macOS: brew install node\n  Linux: curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash - && sudo apt-get install -y nodejs\n  Windows: https://nodejs.org/".into());
            }

            println!("  Downloading package...");

            // For install check, only use the package name (first 2 args: -y and package)
            // Don't pass path arguments to --help
            let install_args: Vec<String> = server.args.iter()
                .take(2)  // Just -y and package name
                .cloned()
                .collect();

            let output = Command::new("npx")
                .args(&install_args)
                .arg("--help") // Just check if it installs
                .output()?;

            if output.status.success() {
                // Special handling for playwright - need to install browsers
                if name == "playwright" || name == "playwright-ea" {
                    println!("  Installing Playwright browsers (this may take a few minutes)...");
                    let browser_install = Command::new("npx")
                        .args(["playwright", "install", "chromium"])
                        .output();

                    match browser_install {
                        Ok(out) if out.status.success() => {
                            println!("  ✓ Chromium browser installed");
                        }
                        Ok(out) => {
                            println!("  ⚠ Browser install warning: {}", String::from_utf8_lossy(&out.stderr));
                            println!("    You may need to run: npx playwright install");
                        }
                        Err(e) => {
                            println!("  ⚠ Could not install browsers: {}", e);
                            println!("    Run manually: npx playwright install");
                        }
                    }
                }

                if let Some(s) = self.servers.get_mut(name) {
                    s.status = ServerStatus::Stopped;
                }
                self.save_config()?;
                println!("  ✓ {} installed successfully", name);
            } else {
                return Err(format!("Failed to install {}: {}",
                    name, String::from_utf8_lossy(&output.stderr)).into());
            }
        } else if server.command == "uvx" {
            // Python-based servers via uvx
            let version_check = Command::new("uvx")
                .arg("--version")
                .output();

            if version_check.is_err() || !version_check.as_ref().unwrap().status.success() {
                return Err("uvx not found. Please install uv:\n  curl -LsSf https://astral.sh/uv/install.sh | sh\n  Then restart your terminal.".into());
            }

            println!("  Downloading package...");

            // Try to run the package with --help to verify it installs
            let output = Command::new("uvx")
                .args(&server.args)
                .arg("--help")
                .output();

            match output {
                Ok(o) if o.status.success() => {
                    if let Some(s) = self.servers.get_mut(name) {
                        s.status = ServerStatus::Stopped;
                    }
                    self.save_config()?;
                    println!("  ✓ {} installed successfully", name);
                }
                Ok(o) => {
                    // Some packages don't support --help, just mark as installed
                    if let Some(s) = self.servers.get_mut(name) {
                        s.status = ServerStatus::Stopped;
                    }
                    self.save_config()?;
                    println!("  ✓ {} registered (will verify on first run)", name);
                }
                Err(e) => {
                    return Err(format!("Failed to install {}: {}", name, e).into());
                }
            }
        }

        Ok(())
    }

    /// Install all default servers (synchronous)
    pub fn install_defaults(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let names: Vec<String> = self.servers.keys()
            .filter(|n| self.servers[*n].auto_start)
            .cloned()
            .collect();

        let mut installed: Vec<String> = Vec::new();

        for name in names {
            if let Err(e) = self.install_server(&name) {
                eprintln!("  Warning: Failed to install {}: {}", name, e);
            } else {
                installed.push(name);
            }
        }

        // Auto-start successfully installed servers (use proper MCP protocol connection)
        println!("\n  Starting installed servers...");
        for name in &installed {
            if let Some(server) = self.servers.get(name).cloned() {
                match connect_mcp_server_verbose(&server, false) {
                    Ok(_) => {
                        if let Some(s) = self.servers.get_mut(name) {
                            s.status = ServerStatus::Running;
                        }
                        println!("  ✓ {} started", name);
                    }
                    Err(e) => println!("  ⚠ {} failed to start: {}", name, e),
                }
            }
        }

        Ok(())
    }

    /// Auto-connect installed servers on startup (silent, for main.rs)
    /// Uses the proper MCP protocol connection (not just process spawn)
    pub fn auto_connect_installed(&mut self) -> usize {
        let installed: Vec<(String, McpServer)> = self.servers.iter()
            .filter(|(_, s)| s.status == ServerStatus::Stopped && s.auto_start)
            .map(|(n, s)| (n.clone(), s.clone()))
            .collect();

        let mut connected = 0;
        for (name, server) in installed {
            // Use connect_mcp_server_verbose which properly initializes MCP protocol
            // and adds to global client registry (quiet mode)
            if connect_mcp_server_verbose(&server, false).is_ok() {
                // Update status in our local tracking
                if let Some(s) = self.servers.get_mut(&name) {
                    s.status = ServerStatus::Running;
                }
                connected += 1;
            }
        }
        connected
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

/// MCP Client for communicating with an MCP server via JSON-RPC over stdio
pub struct McpClient {
    stdin: Option<std::process::ChildStdin>,
    stdout: Option<std::io::BufReader<std::process::ChildStdout>>,
    request_id: std::sync::atomic::AtomicU64,
    pub tools: Vec<McpToolDef>,
    pub server_name: String,
}

impl Drop for McpClient {
    fn drop(&mut self) {
        // Take ownership of stdin/stdout and forget them to prevent blocking drops
        // The child process will be killed when its handles are closed on process exit
        if let Some(stdin) = self.stdin.take() {
            std::mem::forget(stdin);
        }
        if let Some(stdout) = self.stdout.take() {
            std::mem::forget(stdout);
        }
    }
}

/// MCP Tool definition from the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDef {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: Option<serde_json::Value>,
}

impl McpClient {
    /// Connect to an MCP server
    pub fn connect(server: &McpServer) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        use std::io::BufReader;
        use std::process::{Command, Stdio};

        let mut cmd = Command::new(&server.command);
        cmd.args(&server.args);

        for (key, value) in &server.env {
            cmd.env(key, value);
        }

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let mut child = cmd.spawn()
            .map_err(|e| format!("Failed to spawn MCP server {}: {}", server.name, e))?;

        let stdin = child.stdin.take()
            .ok_or("Failed to get stdin for MCP server")?;
        let stdout = child.stdout.take()
            .ok_or("Failed to get stdout for MCP server")?;

        Ok(Self {
            stdin: Some(stdin),
            stdout: Some(BufReader::new(stdout)),
            request_id: std::sync::atomic::AtomicU64::new(1),
            tools: vec![],
            server_name: server.name.clone(),
        })
    }

    /// Get next request ID
    fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    /// Send a JSON-RPC request and get response
    /// Note: This is a blocking call with no timeout. For browser operations,
    /// the caller should implement their own timeout handling if needed.
    fn send_request(&mut self, method: &str, params: Option<serde_json::Value>)
        -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>>
    {
        use std::io::{BufRead, Write};

        let id = self.next_id();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params.unwrap_or(serde_json::json!({}))
        });

        // Write request
        let request_str = serde_json::to_string(&request)?;
        let stdin = self.stdin.as_mut().ok_or("MCP stdin not available")?;
        writeln!(stdin, "{}", request_str)?;
        stdin.flush()?;

        // Read response (blocking)
        let stdout = self.stdout.as_mut().ok_or("MCP stdout not available")?;
        let mut line = String::new();
        loop {
            line.clear();
            if stdout.read_line(&mut line)? == 0 {
                return Err("MCP server closed connection".into());
            }

            if line.trim().is_empty() {
                continue;
            }

            // Try to parse as JSON-RPC response
            if let Ok(response) = serde_json::from_str::<serde_json::Value>(&line) {
                // Check if this is our response
                if response.get("id").and_then(|v| v.as_u64()) == Some(id) {
                    if let Some(error) = response.get("error") {
                        return Err(format!("MCP error: {}", error).into());
                    }
                    return Ok(response.get("result").cloned().unwrap_or(serde_json::json!(null)));
                }
                // If it's a notification, continue reading
            }
        }
    }

    /// Initialize the MCP connection
    pub fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "roots": { "listChanged": true },
                "sampling": {}
            },
            "clientInfo": {
                "name": "ganesha",
                "version": "3.0.0"
            }
        });

        let _result = self.send_request("initialize", Some(params))?;

        // Send initialized notification
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        use std::io::Write;
        let stdin = self.stdin.as_mut().ok_or("MCP stdin not available")?;
        writeln!(stdin, "{}", serde_json::to_string(&notification)?)?;
        stdin.flush()?;

        Ok(())
    }

    /// List available tools from the server
    pub fn list_tools(&mut self) -> Result<Vec<McpToolDef>, Box<dyn std::error::Error + Send + Sync>> {
        let result = self.send_request("tools/list", None)?;

        let tools: Vec<McpToolDef> = if let Some(tools_array) = result.get("tools") {
            serde_json::from_value(tools_array.clone()).unwrap_or_default()
        } else {
            vec![]
        };

        self.tools = tools.clone();
        Ok(tools)
    }

    /// Call a tool
    pub fn call_tool(&mut self, name: &str, arguments: serde_json::Value)
        -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>>
    {
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments
        });

        self.send_request("tools/call", Some(params))
    }
}

/// Global MCP client registry
static MCP_CLIENTS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, McpClient>>>
    = std::sync::OnceLock::new();

fn get_clients() -> &'static std::sync::Mutex<std::collections::HashMap<String, McpClient>> {
    MCP_CLIENTS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// Connect to an MCP server and initialize it
pub fn connect_mcp_server(server: &McpServer) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    connect_mcp_server_verbose(server, true)
}

/// Connect to an MCP server with optional verbose output
pub fn connect_mcp_server_verbose(server: &McpServer, verbose: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut client = McpClient::connect(server)?;
    client.initialize()?;
    let tools = client.list_tools()?;

    if verbose {
        println!("  Connected to {} with {} tools:", server.name, tools.len());
        for tool in &tools {
            println!("    - {}: {}", tool.name, tool.description.as_deref().unwrap_or(""));
        }
    }

    let mut clients = get_clients().lock().unwrap();
    clients.insert(server.name.clone(), client);
    Ok(())
}

/// List tools from a connected MCP server
pub fn list_mcp_tools(server_name: &str) -> Option<Vec<McpToolDef>> {
    let clients = get_clients().lock().unwrap();
    clients.get(server_name).map(|c| c.tools.clone())
}

/// Call an MCP tool
pub fn call_mcp_tool(
    server_name: &str,
    tool_name: &str,
    args: serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let mut clients = get_clients().lock().unwrap();
    let client = clients.get_mut(server_name)
        .ok_or_else(|| format!("MCP server {} not connected", server_name))?;

    client.call_tool(tool_name, args)
}

/// Get all connected MCP servers and their tools
pub fn get_all_mcp_tools() -> Vec<(String, Vec<McpToolDef>)> {
    let clients = get_clients().lock().unwrap();
    clients.iter()
        .map(|(name, client)| (name.clone(), client.tools.clone()))
        .collect()
}

/// Shutdown all MCP clients cleanly
/// Call this before program exit to avoid tokio runtime panics
pub fn shutdown_mcp_clients() {
    let mut clients = get_clients().lock().unwrap();
    // Clear all clients - their child processes will be killed
    clients.clear();
}

/// Leak MCP clients to prevent drop during tokio shutdown
/// The child processes will be cleaned up when the main process exits
pub fn leak_mcp_clients() {
    let mut clients = get_clients().lock().unwrap();
    // Take ownership and forget each client to prevent Drop from running
    for (_, client) in clients.drain() {
        std::mem::forget(client);
    }
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
