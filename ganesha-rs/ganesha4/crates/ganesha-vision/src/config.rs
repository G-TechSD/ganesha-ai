//! Configuration for the Vision/VLA system.
//!
//! This module provides configuration structures for:
//! - Vision model selection and API settings
//! - Application whitelist/blacklist
//! - Safety limits and confirmation settings
//! - Capture quality settings

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;

/// Vision model provider selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum VisionModel {
    /// OpenAI GPT-4 Vision
    #[default]
    Gpt4Vision,
    /// Anthropic Claude with vision
    ClaudeVision,
    /// Google Gemini Pro Vision
    GeminiVision,
    /// Local model (e.g., LLaVA)
    Local,
    /// Microsoft OmniParser for UI element detection
    OmniParser,
}

impl VisionModel {
    /// Get the model identifier string.
    pub fn model_id(&self) -> &'static str {
        match self {
            Self::Gpt4Vision => "gpt-4-vision-preview",
            Self::ClaudeVision => "claude-3-5-sonnet-latest",
            Self::GeminiVision => "gemini-pro-vision",
            Self::Local => "local",
            Self::OmniParser => "omniparser-v2",
        }
    }
}

/// Image format for screen captures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    /// PNG format (lossless, larger files)
    #[default]
    Png,
    /// JPEG format (lossy, smaller files)
    Jpeg,
    /// WebP format (modern, efficient)
    WebP,
}

impl ImageFormat {
    /// Get the file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::WebP => "webp",
        }
    }

    /// Get the MIME type for this format.
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::WebP => "image/webp",
        }
    }
}

/// Capture quality settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSettings {
    /// Image format for captures
    pub format: ImageFormat,
    /// JPEG quality (1-100), only used for JPEG format
    pub jpeg_quality: u8,
    /// Maximum image dimension (width or height)
    /// Images larger than this will be scaled down
    pub max_dimension: u32,
    /// Whether to include cursor in captures
    pub include_cursor: bool,
}

impl Default for CaptureSettings {
    fn default() -> Self {
        Self {
            format: ImageFormat::Png,
            jpeg_quality: 85,
            max_dimension: 1920,
            include_cursor: true,
        }
    }
}

/// Safety limits for automated actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyLimits {
    /// Maximum mouse clicks per minute
    pub max_clicks_per_minute: u32,
    /// Maximum keystrokes per minute
    pub max_keystrokes_per_minute: u32,
    /// Maximum actions per task
    pub max_actions_per_task: u32,
    /// Delay between actions (milliseconds)
    pub action_delay_ms: u64,
    /// Timeout for entire task
    pub task_timeout: Duration,
    /// Whether emergency stop is enabled (Escape key)
    pub emergency_stop_enabled: bool,
}

impl Default for SafetyLimits {
    fn default() -> Self {
        Self {
            max_clicks_per_minute: 60,
            max_keystrokes_per_minute: 300,
            max_actions_per_task: 100,
            action_delay_ms: 100,
            task_timeout: Duration::from_secs(300), // 5 minutes
            emergency_stop_enabled: true,
        }
    }
}

/// Actions that require explicit user confirmation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfirmationSettings {
    /// Require confirmation for file deletion operations
    pub confirm_file_delete: bool,
    /// Require confirmation for system settings changes
    pub confirm_system_changes: bool,
    /// Require confirmation for network operations
    pub confirm_network_ops: bool,
    /// Require confirmation for all destructive actions
    pub confirm_destructive: bool,
    /// List of custom action patterns requiring confirmation
    pub custom_patterns: Vec<String>,
}

/// Known application that can be controlled.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KnownApp {
    /// Display name of the application
    pub name: String,
    /// Process name(s) to identify the application
    pub process_names: Vec<String>,
    /// Window title patterns (regex)
    pub window_patterns: Vec<String>,
    /// Whether this app is enabled for automation
    pub enabled: bool,
    /// App-specific safety notes
    pub notes: Option<String>,
}

impl KnownApp {
    /// Create a new known app configuration.
    pub fn new(name: impl Into<String>, process_names: Vec<String>) -> Self {
        Self {
            name: name.into(),
            process_names,
            window_patterns: Vec::new(),
            enabled: true,
            notes: None,
        }
    }

    /// Add window title patterns.
    pub fn with_window_patterns(mut self, patterns: Vec<String>) -> Self {
        self.window_patterns = patterns;
        self
    }

    /// Set enabled state.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Add safety notes.
    pub fn with_notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }
}

/// Application whitelist/blacklist configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppListConfig {
    /// Mode of operation
    pub mode: AppListMode,
    /// Known applications with configurations
    pub known_apps: Vec<KnownApp>,
    /// Additional whitelisted process names
    pub whitelist: HashSet<String>,
    /// Blacklisted process names (always blocked)
    pub blacklist: HashSet<String>,
}

/// Application list mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppListMode {
    /// Only allow explicitly whitelisted apps
    #[default]
    Whitelist,
    /// Allow all except blacklisted apps
    Blacklist,
    /// Allow all apps (dangerous!)
    AllowAll,
}

impl AppListConfig {
    /// Create default configuration with common creative apps.
    pub fn with_defaults() -> Self {
        let known_apps = vec![
            KnownApp::new("Blender", vec!["blender".to_string(), "blender-bin".to_string()])
                .with_window_patterns(vec!["Blender".to_string()])
                .with_notes("3D modeling and animation software"),
            KnownApp::new(
                "Bambu Studio",
                vec!["bambu-studio".to_string(), "BambuStudio".to_string()],
            )
            .with_window_patterns(vec!["Bambu Studio".to_string()])
            .with_notes("3D printer slicer software"),
            KnownApp::new("OBS Studio", vec!["obs".to_string(), "obs-studio".to_string()])
                .with_window_patterns(vec!["OBS".to_string()])
                .with_notes("Video recording and streaming"),
            KnownApp::new("CapCut", vec!["capcut".to_string(), "CapCut".to_string()])
                .with_window_patterns(vec!["CapCut".to_string()])
                .with_notes("Video editing software"),
            KnownApp::new(
                "VS Code",
                vec!["code".to_string(), "code-oss".to_string()],
            )
            .with_window_patterns(vec!["Visual Studio Code".to_string()])
            .with_enabled(false) // Disabled by default for safety
            .with_notes("Code editor - disabled by default for safety"),
        ];

        let whitelist: HashSet<String> = known_apps
            .iter()
            .filter(|app| app.enabled)
            .flat_map(|app| app.process_names.clone())
            .collect();

        Self {
            mode: AppListMode::Whitelist,
            known_apps,
            whitelist,
            blacklist: HashSet::new(),
        }
    }

    /// Check if an app is allowed.
    pub fn is_allowed(&self, process_name: &str) -> bool {
        // Always check blacklist first
        if self.blacklist.contains(process_name) {
            return false;
        }

        match self.mode {
            AppListMode::Whitelist => self.whitelist.contains(process_name),
            AppListMode::Blacklist => !self.blacklist.contains(process_name),
            AppListMode::AllowAll => true,
        }
    }

    /// Get known app by process name.
    pub fn get_known_app(&self, process_name: &str) -> Option<&KnownApp> {
        self.known_apps
            .iter()
            .find(|app| app.process_names.contains(&process_name.to_string()))
    }
}

/// Main configuration for the Vision/VLA system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionConfig {
    /// Whether the vision system is enabled
    pub enabled: bool,
    /// Vision model to use for analysis
    pub model: VisionModel,
    /// API endpoint for vision model (if custom)
    pub api_endpoint: Option<String>,
    /// API key environment variable name
    pub api_key_env: String,
    /// Capture settings
    pub capture: CaptureSettings,
    /// Safety limits
    pub safety: SafetyLimits,
    /// Confirmation settings
    pub confirmations: ConfirmationSettings,
    /// Application whitelist/blacklist
    pub apps: AppListConfig,
    /// Enable audit logging
    pub audit_logging: bool,
    /// Audit log file path
    pub audit_log_path: Option<String>,
    /// Enable dry-run mode (no actual input simulation)
    pub dry_run: bool,
}

impl Default for VisionConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Disabled by default for safety
            model: VisionModel::default(),
            api_endpoint: None,
            api_key_env: "OPENAI_API_KEY".to_string(),
            capture: CaptureSettings::default(),
            safety: SafetyLimits::default(),
            confirmations: ConfirmationSettings::default(),
            apps: AppListConfig::with_defaults(),
            audit_logging: true,
            audit_log_path: None,
            dry_run: false,
        }
    }
}

impl VisionConfig {
    /// Create a new configuration with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable the vision system.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set the vision model.
    pub fn with_model(mut self, model: VisionModel) -> Self {
        self.model = model;
        // Update API key env based on model
        self.api_key_env = match model {
            VisionModel::Gpt4Vision => "OPENAI_API_KEY",
            VisionModel::ClaudeVision => "ANTHROPIC_API_KEY",
            VisionModel::GeminiVision => "GOOGLE_API_KEY",
            VisionModel::Local => "LOCAL_VISION_API_KEY",
            VisionModel::OmniParser => "OMNIPARSER_API_KEY",
        }
        .to_string();
        self
    }

    /// Set custom API endpoint.
    pub fn with_api_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.api_endpoint = Some(endpoint.into());
        self
    }

    /// Enable dry-run mode.
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Check safety limits are reasonable
        if self.safety.max_clicks_per_minute == 0 {
            return Err(ConfigError::InvalidValue(
                "max_clicks_per_minute must be > 0".to_string(),
            ));
        }

        if self.safety.max_keystrokes_per_minute == 0 {
            return Err(ConfigError::InvalidValue(
                "max_keystrokes_per_minute must be > 0".to_string(),
            ));
        }

        if self.capture.jpeg_quality == 0 || self.capture.jpeg_quality > 100 {
            return Err(ConfigError::InvalidValue(
                "jpeg_quality must be between 1 and 100".to_string(),
            ));
        }

        if self.capture.max_dimension < 100 {
            return Err(ConfigError::InvalidValue(
                "max_dimension must be at least 100".to_string(),
            ));
        }

        // Warn if allow all mode is enabled
        if self.apps.mode == AppListMode::AllowAll && !self.dry_run {
            tracing::warn!("Vision system configured to allow all apps without dry-run mode");
        }

        Ok(())
    }
}

/// Configuration error types.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid configuration value: {0}")]
    InvalidValue(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Configuration file error: {0}")]
    FileError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = VisionConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.model, VisionModel::Gpt4Vision);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_app_whitelist() {
        let config = AppListConfig::with_defaults();
        assert!(config.is_allowed("blender"));
        assert!(config.is_allowed("obs"));
        assert!(!config.is_allowed("unknown-app"));
    }

    #[test]
    fn test_image_format() {
        assert_eq!(ImageFormat::Png.extension(), "png");
        assert_eq!(ImageFormat::Jpeg.mime_type(), "image/jpeg");
    }

    #[test]
    fn test_vision_model_id() {
        assert_eq!(VisionModel::ClaudeVision.model_id(), "claude-3-5-sonnet-latest");
    }
}
