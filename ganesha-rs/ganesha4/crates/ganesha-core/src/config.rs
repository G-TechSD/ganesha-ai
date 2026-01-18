//! # Configuration System
//!
//! Manages configuration from multiple sources: TOML files, environment variables, and runtime updates.
//!
//! ## Overview
//!
//! The configuration system is responsible for:
//! - Loading configuration from global and project-level TOML files
//! - Supporting environment variable overrides
//! - Providing runtime configuration updates
//! - Validating configuration values
//!
//! ## Configuration Sources (in priority order)
//!
//! 1. Runtime updates (highest priority)
//! 2. Environment variables (GANESHA_* prefix)
//! 3. Project-level config (.ganesha/config.toml)
//! 4. Global config (~/.config/ganesha/config.toml)
//! 5. Default values (lowest priority)
//!
//! ## Example
//!
//! ```ignore
//! let config = CoreConfig::load()?;
//!
//! // Access configuration
//! println!("Risk level: {:?}", config.risk_level);
//! println!("Max tokens: {}", config.ai.max_tokens);
//!
//! // Update at runtime
//! config.set_risk_level(RiskLevel::Trusted);
//! ```

use crate::risk::RiskLevel;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, info, warn};

/// Errors that can occur in configuration
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    TomlError(#[from] toml::de::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Configuration not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, ConfigError>;

/// AI provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    /// Default provider to use
    #[serde(default = "default_provider")]
    pub provider: String,

    /// Default model
    #[serde(default = "default_model")]
    pub model: String,

    /// Maximum tokens for responses
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,

    /// Temperature for generation
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// API base URL (optional override)
    pub api_base_url: Option<String>,

    /// API key (should use env var in practice)
    pub api_key: Option<String>,

    /// Request timeout in seconds
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,

    /// Enable streaming responses
    #[serde(default = "default_true")]
    pub streaming: bool,

    /// Additional provider-specific options
    #[serde(default)]
    pub options: HashMap<String, serde_json::Value>,
}

fn default_provider() -> String {
    "anthropic".to_string()
}

fn default_model() -> String {
    "claude-sonnet-4-20250514".to_string()
}

fn default_max_tokens() -> u32 {
    8192
}

fn default_temperature() -> f32 {
    0.7
}

fn default_timeout_secs() -> u64 {
    120
}

fn default_true() -> bool {
    true
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            model: default_model(),
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
            api_base_url: None,
            api_key: None,
            timeout_secs: default_timeout_secs(),
            streaming: true,
            options: HashMap::new(),
        }
    }
}

/// Execution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    /// Working directory (defaults to current)
    pub working_directory: Option<PathBuf>,

    /// Command timeout in seconds
    #[serde(default = "default_command_timeout")]
    pub command_timeout_secs: u64,

    /// Enable automatic rollback on failure
    #[serde(default = "default_true")]
    pub auto_rollback: bool,

    /// Maximum file size to read (bytes)
    #[serde(default = "default_max_file_size")]
    pub max_file_size: usize,

    /// Shell to use for commands
    #[serde(default = "default_shell")]
    pub shell: String,

    /// Directories to exclude from operations
    #[serde(default)]
    pub excluded_dirs: Vec<String>,

    /// File patterns to exclude
    #[serde(default)]
    pub excluded_patterns: Vec<String>,

    /// Enable dry-run mode by default
    #[serde(default)]
    pub dry_run: bool,
}

fn default_command_timeout() -> u64 {
    120
}

fn default_max_file_size() -> usize {
    10 * 1024 * 1024 // 10 MB
}

fn default_shell() -> String {
    "sh".to_string()
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            working_directory: None,
            command_timeout_secs: default_command_timeout(),
            auto_rollback: true,
            max_file_size: default_max_file_size(),
            shell: default_shell(),
            excluded_dirs: vec![
                ".git".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                "__pycache__".to_string(),
                ".venv".to_string(),
            ],
            excluded_patterns: vec![
                "*.pyc".to_string(),
                "*.o".to_string(),
                "*.so".to_string(),
                "*.dylib".to_string(),
            ],
            dry_run: false,
        }
    }
}

/// Session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Directory to store sessions
    #[serde(default = "default_session_dir")]
    pub storage_dir: PathBuf,

    /// Maximum session age in days before cleanup
    #[serde(default = "default_max_session_age")]
    pub max_age_days: u32,

    /// Auto-save interval in seconds
    #[serde(default = "default_autosave_interval")]
    pub autosave_interval_secs: u64,

    /// Enable automatic checkpoints
    #[serde(default = "default_true")]
    pub auto_checkpoint: bool,

    /// Messages between auto-checkpoints
    #[serde(default = "default_checkpoint_interval")]
    pub checkpoint_interval: usize,
}

fn default_session_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ganesha")
        .join("sessions")
}

fn default_max_session_age() -> u32 {
    30
}

fn default_autosave_interval() -> u64 {
    60
}

fn default_checkpoint_interval() -> usize {
    10
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            storage_dir: default_session_dir(),
            max_age_days: default_max_session_age(),
            autosave_interval_secs: default_autosave_interval(),
            auto_checkpoint: true,
            checkpoint_interval: default_checkpoint_interval(),
        }
    }
}

/// Verification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationConfig {
    /// Enable automatic verification after execution
    #[serde(default = "default_true")]
    pub auto_verify: bool,

    /// Run tests after changes
    #[serde(default = "default_true")]
    pub run_tests: bool,

    /// Custom test command
    pub test_command: Option<String>,

    /// Custom build command
    pub build_command: Option<String>,

    /// Verification timeout in seconds
    #[serde(default = "default_verification_timeout")]
    pub timeout_secs: u64,

    /// Checks to enable
    #[serde(default = "default_checks")]
    pub enabled_checks: Vec<String>,
}

fn default_verification_timeout() -> u64 {
    300
}

fn default_checks() -> Vec<String> {
    vec![
        "syntax".to_string(),
        "build".to_string(),
        "file_exists".to_string(),
    ]
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            auto_verify: true,
            run_tests: true,
            test_command: None,
            build_command: None,
            timeout_secs: default_verification_timeout(),
            enabled_checks: default_checks(),
        }
    }
}

/// MCP (Model Context Protocol) configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpConfig {
    /// MCP servers to auto-connect
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,

    /// Enable MCP discovery
    #[serde(default)]
    pub auto_discover: bool,

    /// Connection timeout in seconds
    #[serde(default = "default_mcp_timeout")]
    pub timeout_secs: u64,
}

fn default_mcp_timeout() -> u64 {
    30
}

/// Configuration for a single MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Server name/identifier
    pub name: String,

    /// Server command to run
    pub command: String,

    /// Command arguments
    #[serde(default)]
    pub args: Vec<String>,

    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Auto-connect on startup
    #[serde(default = "default_true")]
    pub auto_connect: bool,
}

/// UI/Display configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    /// Enable colored output
    #[serde(default = "default_true")]
    pub colors: bool,

    /// Enable emoji in output
    #[serde(default = "default_true")]
    pub emoji: bool,

    /// Show timestamps
    #[serde(default)]
    pub timestamps: bool,

    /// Verbose output
    #[serde(default)]
    pub verbose: bool,

    /// Theme (light/dark/auto)
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_theme() -> String {
    "auto".to_string()
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            colors: true,
            emoji: true,
            timestamps: false,
            verbose: false,
            theme: default_theme(),
        }
    }
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    /// Risk level setting
    #[serde(default)]
    pub risk_level: RiskLevel,

    /// AI provider configuration
    #[serde(default)]
    pub ai: AiConfig,

    /// Execution configuration
    #[serde(default)]
    pub execution: ExecutionConfig,

    /// Session configuration
    #[serde(default)]
    pub session: SessionConfig,

    /// Verification configuration
    #[serde(default)]
    pub verification: VerificationConfig,

    /// MCP configuration
    #[serde(default)]
    pub mcp: McpConfig,

    /// Display configuration
    #[serde(default)]
    pub display: DisplayConfig,

    /// Custom key-value settings
    #[serde(default)]
    pub custom: HashMap<String, serde_json::Value>,

    /// Path this config was loaded from
    #[serde(skip)]
    loaded_from: Option<PathBuf>,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            risk_level: RiskLevel::default(),
            ai: AiConfig::default(),
            execution: ExecutionConfig::default(),
            session: SessionConfig::default(),
            verification: VerificationConfig::default(),
            mcp: McpConfig::default(),
            display: DisplayConfig::default(),
            custom: HashMap::new(),
            loaded_from: None,
        }
    }
}

impl CoreConfig {
    /// Create a new default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from default locations
    pub fn load() -> Result<Self> {
        let mut config = Self::default();

        // Load global config
        if let Some(global_path) = Self::global_config_path() {
            if global_path.exists() {
                debug!("Loading global config from {:?}", global_path);
                config = config.merge_from_file(&global_path)?;
            }
        }

        // Load project config
        if let Some(project_path) = Self::project_config_path() {
            if project_path.exists() {
                debug!("Loading project config from {:?}", project_path);
                config = config.merge_from_file(&project_path)?;
            }
        }

        // Apply environment variable overrides
        config = config.apply_env_overrides();

        Ok(config)
    }

    /// Load configuration from a specific file
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)?;
        let mut config: Self = toml::from_str(&content)?;
        config.loaded_from = Some(path.to_path_buf());
        Ok(config)
    }

    /// Merge configuration from a file
    pub fn merge_from_file(mut self, path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)?;
        let file_config: Self = toml::from_str(&content)?;

        // Merge values (file values take precedence for non-default values)
        self.risk_level = file_config.risk_level;
        self.ai = file_config.ai;
        self.execution = file_config.execution;
        self.session = file_config.session;
        self.verification = file_config.verification;
        self.mcp = file_config.mcp;
        self.display = file_config.display;
        self.custom.extend(file_config.custom);
        self.loaded_from = Some(path.to_path_buf());

        Ok(self)
    }

    /// Apply environment variable overrides
    pub fn apply_env_overrides(mut self) -> Self {
        // Risk level
        if let Ok(level) = std::env::var("GANESHA_RISK_LEVEL") {
            if let Ok(parsed) = level.parse() {
                debug!("Risk level from env: {:?}", parsed);
                self.risk_level = parsed;
            }
        }

        // AI provider
        if let Ok(provider) = std::env::var("GANESHA_AI_PROVIDER") {
            self.ai.provider = provider;
        }

        // AI model
        if let Ok(model) = std::env::var("GANESHA_AI_MODEL") {
            self.ai.model = model;
        }

        // API key
        if let Ok(key) = std::env::var("GANESHA_API_KEY") {
            self.ai.api_key = Some(key);
        }

        // API base URL
        if let Ok(url) = std::env::var("GANESHA_API_BASE_URL") {
            self.ai.api_base_url = Some(url);
        }

        // Max tokens
        if let Ok(tokens) = std::env::var("GANESHA_MAX_TOKENS") {
            if let Ok(parsed) = tokens.parse() {
                self.ai.max_tokens = parsed;
            }
        }

        // Temperature
        if let Ok(temp) = std::env::var("GANESHA_TEMPERATURE") {
            if let Ok(parsed) = temp.parse() {
                self.ai.temperature = parsed;
            }
        }

        // Dry run
        if let Ok(dry_run) = std::env::var("GANESHA_DRY_RUN") {
            self.execution.dry_run = dry_run == "1" || dry_run.to_lowercase() == "true";
        }

        // Verbose
        if let Ok(verbose) = std::env::var("GANESHA_VERBOSE") {
            self.display.verbose = verbose == "1" || verbose.to_lowercase() == "true";
        }

        self
    }

    /// Save configuration to a file
    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)
            .map_err(|e| ConfigError::SerializationError(e.to_string()))?;
        std::fs::write(path, content)?;

        info!("Saved configuration to {:?}", path);
        Ok(())
    }

    /// Save to the loaded location (or default project location)
    pub fn save(&self) -> Result<()> {
        let path = self
            .loaded_from
            .clone()
            .or_else(Self::project_config_path)
            .ok_or_else(|| ConfigError::NotFound("No config path available".to_string()))?;

        self.save_to_file(path)
    }

    /// Get the global config path
    pub fn global_config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("ganesha").join("config.toml"))
    }

    /// Get the project config path (in current directory)
    pub fn project_config_path() -> Option<PathBuf> {
        std::env::current_dir()
            .ok()
            .map(|d| d.join(".ganesha").join("config.toml"))
    }

    /// Set the risk level
    pub fn set_risk_level(&mut self, level: RiskLevel) {
        self.risk_level = level;
    }

    /// Get command timeout as Duration
    pub fn command_timeout(&self) -> Duration {
        Duration::from_secs(self.execution.command_timeout_secs)
    }

    /// Get AI timeout as Duration
    pub fn ai_timeout(&self) -> Duration {
        Duration::from_secs(self.ai.timeout_secs)
    }

    /// Get verification timeout as Duration
    pub fn verification_timeout(&self) -> Duration {
        Duration::from_secs(self.verification.timeout_secs)
    }

    /// Get custom value
    pub fn get_custom<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.custom
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Set custom value
    pub fn set_custom(&mut self, key: impl Into<String>, value: impl Serialize) {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.custom.insert(key.into(), json_value);
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate temperature
        if !(0.0..=2.0).contains(&self.ai.temperature) {
            return Err(ConfigError::InvalidConfig(
                "Temperature must be between 0.0 and 2.0".to_string(),
            ));
        }

        // Validate max tokens
        if self.ai.max_tokens == 0 {
            return Err(ConfigError::InvalidConfig(
                "Max tokens must be greater than 0".to_string(),
            ));
        }

        // Validate timeouts
        if self.execution.command_timeout_secs == 0 {
            return Err(ConfigError::InvalidConfig(
                "Command timeout must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }
}

/// Configuration builder for programmatic configuration
pub struct ConfigBuilder {
    config: CoreConfig,
}

impl ConfigBuilder {
    /// Create a new config builder
    pub fn new() -> Self {
        Self {
            config: CoreConfig::default(),
        }
    }

    /// Set risk level
    pub fn risk_level(mut self, level: RiskLevel) -> Self {
        self.config.risk_level = level;
        self
    }

    /// Set AI provider
    pub fn ai_provider(mut self, provider: impl Into<String>) -> Self {
        self.config.ai.provider = provider.into();
        self
    }

    /// Set AI model
    pub fn ai_model(mut self, model: impl Into<String>) -> Self {
        self.config.ai.model = model.into();
        self
    }

    /// Set max tokens
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.config.ai.max_tokens = tokens;
        self
    }

    /// Set temperature
    pub fn temperature(mut self, temp: f32) -> Self {
        self.config.ai.temperature = temp;
        self
    }

    /// Set working directory
    pub fn working_directory(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.execution.working_directory = Some(path.into());
        self
    }

    /// Enable dry run
    pub fn dry_run(mut self) -> Self {
        self.config.execution.dry_run = true;
        self
    }

    /// Set command timeout
    pub fn command_timeout(mut self, secs: u64) -> Self {
        self.config.execution.command_timeout_secs = secs;
        self
    }

    /// Build the configuration
    pub fn build(self) -> Result<CoreConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = CoreConfig::default();
        assert_eq!(config.risk_level, RiskLevel::Normal);
        assert_eq!(config.ai.provider, "anthropic");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_builder() {
        let config = ConfigBuilder::new()
            .risk_level(RiskLevel::Trusted)
            .ai_model("claude-opus-4-20250514")
            .max_tokens(4096)
            .temperature(0.5)
            .build()
            .unwrap();

        assert_eq!(config.risk_level, RiskLevel::Trusted);
        assert_eq!(config.ai.model, "claude-opus-4-20250514");
        assert_eq!(config.ai.max_tokens, 4096);
        assert_eq!(config.ai.temperature, 0.5);
    }

    #[test]
    fn test_config_validation() {
        let mut config = CoreConfig::default();

        // Invalid temperature
        config.ai.temperature = 3.0;
        assert!(config.validate().is_err());

        // Fix temperature
        config.ai.temperature = 0.7;
        assert!(config.validate().is_ok());

        // Invalid max tokens
        config.ai.max_tokens = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let config = ConfigBuilder::new()
            .risk_level(RiskLevel::Trusted)
            .ai_model("test-model")
            .build()
            .unwrap();

        // Save
        config.save_to_file(&config_path).unwrap();

        // Load
        let loaded = CoreConfig::load_from_file(&config_path).unwrap();
        assert_eq!(loaded.risk_level, RiskLevel::Trusted);
        assert_eq!(loaded.ai.model, "test-model");
    }

    #[test]
    fn test_env_overrides() {
        std::env::set_var("GANESHA_RISK_LEVEL", "yolo");
        std::env::set_var("GANESHA_AI_MODEL", "env-model");
        std::env::set_var("GANESHA_DRY_RUN", "true");

        let config = CoreConfig::default().apply_env_overrides();

        assert_eq!(config.risk_level, RiskLevel::Yolo);
        assert_eq!(config.ai.model, "env-model");
        assert!(config.execution.dry_run);

        // Clean up
        std::env::remove_var("GANESHA_RISK_LEVEL");
        std::env::remove_var("GANESHA_AI_MODEL");
        std::env::remove_var("GANESHA_DRY_RUN");
    }

    #[test]
    fn test_custom_values() {
        let mut config = CoreConfig::default();

        config.set_custom("my_setting", "value");
        config.set_custom("my_number", 42);

        assert_eq!(
            config.get_custom::<String>("my_setting"),
            Some("value".to_string())
        );
        assert_eq!(config.get_custom::<i32>("my_number"), Some(42));
        assert_eq!(config.get_custom::<String>("nonexistent"), None);
    }

    #[test]
    fn test_timeout_durations() {
        let config = CoreConfig::default();

        assert_eq!(config.command_timeout(), Duration::from_secs(120));
        assert_eq!(config.ai_timeout(), Duration::from_secs(120));
        assert_eq!(config.verification_timeout(), Duration::from_secs(300));
    }
}
