//! # Server Registry
//!
//! Registry of known MCP servers for auto-discovery and installation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Registry of known MCP servers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerRegistry {
    /// Known servers by ID
    pub servers: HashMap<String, RegistryEntry>,
}

/// Entry in the server registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    /// Display name
    pub name: String,
    /// Description
    pub description: String,
    /// Category
    pub category: ServerCategory,
    /// Installation command
    pub install_command: String,
    /// NPM package (if applicable)
    pub npm_package: Option<String>,
    /// Required environment variables
    #[serde(default)]
    pub required_env: Vec<RequiredEnvVar>,
    /// Homepage/docs URL
    pub homepage: Option<String>,
    /// Whether this is an official/verified server
    #[serde(default)]
    pub verified: bool,
}

/// Required environment variable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredEnvVar {
    /// Variable name
    pub name: String,
    /// Description
    pub description: String,
    /// How to obtain this value
    pub obtain_url: Option<String>,
    /// Whether it's optional
    #[serde(default)]
    pub optional: bool,
}

/// Server category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerCategory {
    /// File and code access
    Filesystem,
    /// Web and search
    Web,
    /// Database access
    Database,
    /// Version control
    Git,
    /// Communication (Slack, Discord, etc)
    Communication,
    /// Cloud services (AWS, GCP, etc)
    Cloud,
    /// Development tools
    DevTools,
    /// Other
    Other,
}

impl ServerRegistry {
    /// Create a new registry with built-in servers
    pub fn with_builtin() -> Self {
        let mut servers = HashMap::new();

        // Filesystem
        servers.insert(
            "filesystem".to_string(),
            RegistryEntry {
                name: "Filesystem".to_string(),
                description: "Access to local filesystem for reading and writing files".to_string(),
                category: ServerCategory::Filesystem,
                install_command: "npx -y @modelcontextprotocol/server-filesystem".to_string(),
                npm_package: Some("@modelcontextprotocol/server-filesystem".to_string()),
                required_env: Vec::new(),
                homepage: Some("https://github.com/modelcontextprotocol/servers".to_string()),
                verified: true,
            },
        );

        // GitHub
        servers.insert(
            "github".to_string(),
            RegistryEntry {
                name: "GitHub".to_string(),
                description: "Access to GitHub repositories, issues, and PRs".to_string(),
                category: ServerCategory::Git,
                install_command: "npx -y @modelcontextprotocol/server-github".to_string(),
                npm_package: Some("@modelcontextprotocol/server-github".to_string()),
                required_env: vec![RequiredEnvVar {
                    name: "GITHUB_PERSONAL_ACCESS_TOKEN".to_string(),
                    description: "GitHub Personal Access Token".to_string(),
                    obtain_url: Some("https://github.com/settings/tokens".to_string()),
                    optional: false,
                }],
                homepage: Some("https://github.com/modelcontextprotocol/servers".to_string()),
                verified: true,
            },
        );

        // Brave Search
        servers.insert(
            "brave-search".to_string(),
            RegistryEntry {
                name: "Brave Search".to_string(),
                description: "Web search using Brave Search API".to_string(),
                category: ServerCategory::Web,
                install_command: "npx -y @anthropics/mcp-server-brave-search".to_string(),
                npm_package: Some("@anthropics/mcp-server-brave-search".to_string()),
                required_env: vec![RequiredEnvVar {
                    name: "BRAVE_API_KEY".to_string(),
                    description: "Brave Search API Key".to_string(),
                    obtain_url: Some("https://brave.com/search/api/".to_string()),
                    optional: false,
                }],
                homepage: Some("https://github.com/anthropics/anthropic-mcp-servers".to_string()),
                verified: true,
            },
        );

        // Fetch
        servers.insert(
            "fetch".to_string(),
            RegistryEntry {
                name: "Fetch".to_string(),
                description: "Fetch and extract content from web pages".to_string(),
                category: ServerCategory::Web,
                install_command: "npx -y @anthropics/mcp-server-fetch".to_string(),
                npm_package: Some("@anthropics/mcp-server-fetch".to_string()),
                required_env: Vec::new(),
                homepage: Some("https://github.com/anthropics/anthropic-mcp-servers".to_string()),
                verified: true,
            },
        );

        // PostgreSQL
        servers.insert(
            "postgres".to_string(),
            RegistryEntry {
                name: "PostgreSQL".to_string(),
                description: "Query PostgreSQL databases".to_string(),
                category: ServerCategory::Database,
                install_command: "npx -y @modelcontextprotocol/server-postgres".to_string(),
                npm_package: Some("@modelcontextprotocol/server-postgres".to_string()),
                required_env: vec![RequiredEnvVar {
                    name: "POSTGRES_CONNECTION_STRING".to_string(),
                    description: "PostgreSQL connection string".to_string(),
                    obtain_url: None,
                    optional: false,
                }],
                homepage: Some("https://github.com/modelcontextprotocol/servers".to_string()),
                verified: true,
            },
        );

        // SQLite
        servers.insert(
            "sqlite".to_string(),
            RegistryEntry {
                name: "SQLite".to_string(),
                description: "Query SQLite databases".to_string(),
                category: ServerCategory::Database,
                install_command: "npx -y @modelcontextprotocol/server-sqlite".to_string(),
                npm_package: Some("@modelcontextprotocol/server-sqlite".to_string()),
                required_env: Vec::new(),
                homepage: Some("https://github.com/modelcontextprotocol/servers".to_string()),
                verified: true,
            },
        );

        // Slack
        servers.insert(
            "slack".to_string(),
            RegistryEntry {
                name: "Slack".to_string(),
                description: "Interact with Slack workspaces".to_string(),
                category: ServerCategory::Communication,
                install_command: "npx -y @modelcontextprotocol/server-slack".to_string(),
                npm_package: Some("@modelcontextprotocol/server-slack".to_string()),
                required_env: vec![RequiredEnvVar {
                    name: "SLACK_BOT_TOKEN".to_string(),
                    description: "Slack Bot Token".to_string(),
                    obtain_url: Some("https://api.slack.com/apps".to_string()),
                    optional: false,
                }],
                homepage: Some("https://github.com/modelcontextprotocol/servers".to_string()),
                verified: true,
            },
        );

        // Memory (knowledge graph)
        servers.insert(
            "memory".to_string(),
            RegistryEntry {
                name: "Memory".to_string(),
                description: "Persistent memory using a knowledge graph".to_string(),
                category: ServerCategory::Other,
                install_command: "npx -y @modelcontextprotocol/server-memory".to_string(),
                npm_package: Some("@modelcontextprotocol/server-memory".to_string()),
                required_env: Vec::new(),
                homepage: Some("https://github.com/modelcontextprotocol/servers".to_string()),
                verified: true,
            },
        );

        // Puppeteer (official package is deprecated but still works)
        servers.insert(
            "puppeteer".to_string(),
            RegistryEntry {
                name: "Puppeteer".to_string(),
                description: "Browser automation and web scraping".to_string(),
                category: ServerCategory::Web,
                install_command: "npx -y @modelcontextprotocol/server-puppeteer".to_string(),
                npm_package: Some("@modelcontextprotocol/server-puppeteer".to_string()),
                required_env: Vec::new(),
                homepage: Some("https://github.com/modelcontextprotocol/servers".to_string()),
                verified: true,
            },
        );

        Self { servers }
    }

    /// Get a server entry by ID
    pub fn get(&self, id: &str) -> Option<&RegistryEntry> {
        self.servers.get(id)
    }

    /// List servers by category
    pub fn by_category(&self, category: ServerCategory) -> Vec<(&String, &RegistryEntry)> {
        self.servers
            .iter()
            .filter(|(_, entry)| entry.category == category)
            .collect()
    }

    /// Search servers by name or description
    pub fn search(&self, query: &str) -> Vec<(&String, &RegistryEntry)> {
        let query_lower = query.to_lowercase();
        self.servers
            .iter()
            .filter(|(id, entry)| {
                id.contains(&query_lower)
                    || entry.name.to_lowercase().contains(&query_lower)
                    || entry.description.to_lowercase().contains(&query_lower)
            })
            .collect()
    }

    /// List all verified servers
    pub fn verified(&self) -> Vec<(&String, &RegistryEntry)> {
        self.servers
            .iter()
            .filter(|(_, entry)| entry.verified)
            .collect()
    }
}

impl Default for ServerRegistry {
    fn default() -> Self {
        Self::with_builtin()
    }
}
