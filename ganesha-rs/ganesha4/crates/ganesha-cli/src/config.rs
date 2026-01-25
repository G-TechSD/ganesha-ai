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

    #[serde(default)]
    pub agentic: AgenticConfig,
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

/// Agentic behavior configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticConfig {
    /// Path to custom system prompt file (overrides built-in)
    pub system_prompt_path: Option<PathBuf>,

    /// Simple shell commands that should never use puppeteer
    #[serde(default = "default_simple_shell_commands")]
    pub simple_shell_commands: Vec<String>,

    /// Shell command prefixes for plain command detection
    #[serde(default = "default_shell_cmd_prefixes")]
    pub shell_cmd_prefixes: Vec<String>,

    /// Tool ID prefixes to strip (model confusion cleanup)
    #[serde(default = "default_tool_id_prefixes")]
    pub tool_id_prefixes: Vec<String>,

    /// Meaningless echo words to ignore
    #[serde(default = "default_meaningless_echo_words")]
    pub meaningless_echo_words: Vec<String>,

    /// Prompt sent after command execution
    #[serde(default = "default_continuation_prompt")]
    pub continuation_prompt: String,

    /// Enable system status query detection (is X running -> use shell)
    #[serde(default = "default_true")]
    pub detect_system_status_queries: bool,

    /// Enable multi-file continuation (don't stop between files)
    #[serde(default = "default_true")]
    pub enable_multi_file_continuation: bool,

    /// Maximum agentic iterations before stopping
    #[serde(default = "default_max_iterations")]
    pub max_iterations: usize,
}

fn default_simple_shell_commands() -> Vec<String> {
    vec![
        "ls", "pwd", "cd", "cat", "head", "tail", "mkdir", "rm", "cp", "mv",
        "touch", "echo", "whoami", "date", "df", "du", "ps", "top", "htop",
        "free", "uname", "systemctl", "service"
    ].into_iter().map(String::from).collect()
}

fn default_shell_cmd_prefixes() -> Vec<String> {
    vec![
        "curl ", "wget ", "ls ", "cat ", "head ", "tail ", "grep ",
        "find ", "mkdir ", "rm ", "cp ", "mv ", "chmod ", "chown ",
        "touch ", "cd ", "pwd ", "tar ", "zip ", "unzip ", "git ",
        "npm ", "yarn ", "pip ", "python ", "node ", "make ", "cargo ",
        "rustc ", "go ", "java ", "javac "
    ].into_iter().map(String::from).collect()
}

fn default_tool_id_prefixes() -> Vec<String> {
    vec![
        "puppeteer_puppeteer_", "puppeteer:puppeteer_", "tool_puppeteer_",
        "tool_", "tool.", "tool:"
    ].into_iter().map(String::from).collect()
}

fn default_meaningless_echo_words() -> Vec<String> {
    vec![
        "start", "ready", "done", "browsing", "starting", "none", "ok", "begin", "end"
    ].into_iter().map(String::from).collect()
}

fn default_continuation_prompt() -> String {
    "If there are more steps to complete the task, execute the next command immediately. Only provide a summary when ALL steps are done.".to_string()
}

fn default_max_iterations() -> usize {
    50
}

impl Default for AgenticConfig {
    fn default() -> Self {
        Self {
            system_prompt_path: None,
            simple_shell_commands: default_simple_shell_commands(),
            shell_cmd_prefixes: default_shell_cmd_prefixes(),
            tool_id_prefixes: default_tool_id_prefixes(),
            meaningless_echo_words: default_meaningless_echo_words(),
            continuation_prompt: default_continuation_prompt(),
            detect_system_status_queries: true,
            enable_multi_file_continuation: true,
            max_iterations: default_max_iterations(),
        }
    }
}

impl AgenticConfig {
    /// Load from environment variables (overrides config file)
    pub fn apply_env_overrides(&mut self) {
        if let Ok(path) = std::env::var("GANESHA_SYSTEM_PROMPT") {
            self.system_prompt_path = Some(PathBuf::from(path));
        }
        if let Ok(val) = std::env::var("GANESHA_MAX_ITERATIONS") {
            if let Ok(n) = val.parse() {
                self.max_iterations = n;
            }
        }
        if let Ok(val) = std::env::var("GANESHA_DETECT_STATUS_QUERIES") {
            self.detect_system_status_queries = val == "1" || val.to_lowercase() == "true";
        }
        if let Ok(val) = std::env::var("GANESHA_MULTI_FILE_CONTINUATION") {
            self.enable_multi_file_continuation = val == "1" || val.to_lowercase() == "true";
        }
    }

    /// Load custom system prompt from file if configured
    pub fn load_system_prompt(&self) -> Option<String> {
        if let Some(path) = &self.system_prompt_path {
            if path.exists() {
                return std::fs::read_to_string(path).ok();
            }
        }
        // Also check default location
        let default_path = dirs::config_dir()
            .map(|d| d.join("ganesha").join("prompt.md"));
        if let Some(path) = default_path {
            if path.exists() {
                return std::fs::read_to_string(path).ok();
            }
        }
        None
    }
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

        // Merge agentic
        if other.agentic.system_prompt_path.is_some() {
            self.agentic.system_prompt_path = other.agentic.system_prompt_path;
        }
        if other.agentic.simple_shell_commands != default_simple_shell_commands() {
            self.agentic.simple_shell_commands = other.agentic.simple_shell_commands;
        }
        if other.agentic.shell_cmd_prefixes != default_shell_cmd_prefixes() {
            self.agentic.shell_cmd_prefixes = other.agentic.shell_cmd_prefixes;
        }
        if other.agentic.tool_id_prefixes != default_tool_id_prefixes() {
            self.agentic.tool_id_prefixes = other.agentic.tool_id_prefixes;
        }
        if other.agentic.meaningless_echo_words != default_meaningless_echo_words() {
            self.agentic.meaningless_echo_words = other.agentic.meaningless_echo_words;
        }
        if other.agentic.continuation_prompt != default_continuation_prompt() {
            self.agentic.continuation_prompt = other.agentic.continuation_prompt;
        }
        if other.agentic.max_iterations != default_max_iterations() {
            self.agentic.max_iterations = other.agentic.max_iterations;
        }

        self
    }

    /// Apply environment variable overrides
    pub fn apply_env_overrides(&mut self) {
        self.agentic.apply_env_overrides();
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
