//! # CLI Configuration
//!
//! Configuration loading and management for the CLI.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

/// CLI configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CliConfig {
    #[serde(default)]
    pub general: GeneralConfig,

    #[serde(default)]
    pub provider: ProviderConfig,

    #[serde(default)]
    pub mcp: McpConfig,

    #[serde(default)]
    pub session: SessionConfig,

    #[serde(default)]
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Default risk level
    #[serde(default = "default_risk")]
    pub risk_level: String,

    /// Default chat mode
    #[serde(default = "default_mode")]
    pub mode: String,

    /// Enable verbose logging
    #[serde(default)]
    pub verbose: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            risk_level: default_risk(),
            mode: default_mode(),
            verbose: false,
        }
    }
}

fn default_risk() -> String {
    "normal".to_string()
}

fn default_mode() -> String {
    "code".to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Preferred providers in order
    #[serde(default)]
    pub priority: Vec<String>,

    /// Default model
    pub model: Option<String>,

    /// API keys (prefer environment variables)
    #[serde(default)]
    pub api_keys: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    /// Auto-connect on startup
    #[serde(default = "default_true")]
    pub auto_connect: bool,

    /// Trusted server IDs
    #[serde(default)]
    pub trusted: Vec<String>,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            auto_connect: true,
            trusted: Vec::new(),
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Auto-save sessions
    #[serde(default = "default_true")]
    pub auto_save: bool,

    /// Session storage path
    pub path: Option<PathBuf>,

    /// Enable session logging to text files
    #[serde(default = "default_true")]
    pub logging_enabled: bool,

    /// Maximum total size of session logs in bytes (default: 512MB)
    #[serde(default = "default_max_log_size")]
    pub max_log_size: u64,
}

fn default_max_log_size() -> u64 {
    512 * 1024 * 1024 // 512 MB
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            auto_save: true,
            path: None,
            logging_enabled: true,
            max_log_size: default_max_log_size(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UiConfig {
    /// Color theme
    #[serde(default)]
    pub theme: String,

    /// Show timestamps
    #[serde(default)]
    pub show_timestamps: bool,

    /// Show token usage
    #[serde(default)]
    pub show_tokens: bool,
}

impl CliConfig {
    /// Load configuration from default locations
    pub async fn load() -> anyhow::Result<Self> {
        let mut config = Self::default();

        // Load from global config
        if let Some(global_path) = Self::global_config_path() {
            if global_path.exists() {
                let content = fs::read_to_string(&global_path).await?;
                let global_config: CliConfig = toml::from_str(&content)?;
                config = config.merge(global_config);
            }
        }

        // Load from project config
        let project_path = Path::new(".ganesha/config.toml");
        if project_path.exists() {
            let content = fs::read_to_string(project_path).await?;
            let project_config: CliConfig = toml::from_str(&content)?;
            config = config.merge(project_config);
        }

        Ok(config)
    }

    /// Merge another config (other takes precedence)
    pub fn merge(mut self, other: CliConfig) -> Self {
        // Merge general
        if other.general.risk_level != default_risk() {
            self.general.risk_level = other.general.risk_level;
        }
        if other.general.mode != default_mode() {
            self.general.mode = other.general.mode;
        }
        if other.general.verbose {
            self.general.verbose = true;
        }

        // Merge provider
        if !other.provider.priority.is_empty() {
            self.provider.priority = other.provider.priority;
        }
        if other.provider.model.is_some() {
            self.provider.model = other.provider.model;
        }
        self.provider.api_keys.extend(other.provider.api_keys);

        // Merge MCP
        self.mcp.trusted.extend(other.mcp.trusted);

        // Merge session
        if other.session.path.is_some() {
            self.session.path = other.session.path;
        }

        self
    }

    /// Get global config path
    pub fn global_config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("ganesha").join("config.toml"))
    }

    /// Get project config path
    pub fn project_config_path() -> PathBuf {
        PathBuf::from(".ganesha/config.toml")
    }
}
