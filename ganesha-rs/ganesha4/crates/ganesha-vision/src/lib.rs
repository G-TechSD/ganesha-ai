//! # Ganesha Vision
//!
//! Vision and VLA (Vision-Language-Action) capabilities for Ganesha.
//!
//! This crate provides GUI automation capabilities for controlling desktop applications
//! through computer vision and AI-powered action planning.
//!
//! ## Features
//!
//! - **Screen Capture**: Platform-abstracted screen capture with multi-monitor support
//! - **Image Analysis**: Vision model integration (GPT-4V, Claude, Gemini) for UI analysis
//! - **Input Simulation**: Mouse and keyboard input simulation across platforms
//! - **Application Control**: Window focus, management, and app-specific action patterns
//! - **Action Planning**: AI-powered task planning with verification and error recovery
//! - **Safety Controls**: Rate limiting, whitelisting, confirmation, and audit logging
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use ganesha_vision::{VisionConfig, VisionSystem};
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create configuration
//!     let config = VisionConfig::default()
//!         .with_enabled(true)
//!         .with_dry_run(true); // Start in dry-run mode for safety
//!
//!     // Create vision system
//!     let system = VisionSystem::new(config);
//!
//!     // Check if the system is ready
//!     if system.is_enabled() {
//!         println!("Vision system is ready!");
//!     }
//! }
//! ```
//!
//! ## Safety
//!
//! The vision system includes multiple safety features:
//!
//! - **Application Whitelist**: Only control explicitly approved applications
//! - **Rate Limiting**: Maximum clicks and keystrokes per minute
//! - **Action Limits**: Maximum actions per task
//! - **Confirmation Dialogs**: Required for destructive operations
//! - **Emergency Stop**: Press Escape to halt all automation
//! - **Audit Logging**: All actions are logged for review
//! - **Dry-Run Mode**: Test plans without executing actions
//!
//! ## Supported Applications
//!
//! The following applications have pre-configured action patterns:
//!
//! - Blender (3D modeling)
//! - Bambu Studio (3D printing)
//! - OBS Studio (video recording/streaming)
//! - CapCut (video editing)
//!
//! Additional applications can be added to the whitelist in configuration.

pub mod analysis;
pub mod apps;
pub mod capture;
pub mod config;
pub mod input;
pub mod planner;
pub mod safety;

// Re-export main types
pub use analysis::{
    AnalysisError, AnalysisResult, AppContext, ElementState, ElementType, ExtractedText,
    ScreenAnalysis, UIElement, VisionAnalyzer,
};
pub use apps::{
    ActionPattern, AppAction, AppActionLibrary, AppController, AppError, AppInfo, AppResult,
    AppState, DefaultAppController,
};
pub use capture::{
    CaptureError, CaptureResult, MonitorInfo, Region, ScreenCapture, Screenshot, WindowInfo,
};
pub use config::{
    AppListConfig, AppListMode, CaptureSettings, ConfigError, ConfirmationSettings, ImageFormat,
    KnownApp, SafetyLimits, VisionConfig, VisionModel,
};
pub use input::{
    ClickType, DragOperation, InputError, InputResult, InputSimulator, Key, KeyInput,
    KeyboardShortcut, Modifier, MouseAction, MouseButton, ScrollAction,
};
pub use planner::{
    ActionPlan, ConfirmationHandler, ConfirmationRequest, ExecutionContext, ExecutionEvent,
    ExecutionEventType, ExecutionStatus, PlanStep, PlannedAction, PlannerError, PlannerResult,
    ScrollDirection, VisionTask,
};
pub use safety::{
    ActionType, AuditEntry, AuditLogger, EmergencyStopMonitor, SafetyError, SafetyGuard,
    SafetyResult, SafetyStats,
};

use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

/// Main error type for the vision system.
#[derive(Error, Debug)]
pub enum VisionError {
    #[error("Vision system not enabled")]
    NotEnabled,

    #[error("Screen capture failed: {0}")]
    CaptureError(#[from] CaptureError),

    #[error("Analysis failed: {0}")]
    AnalysisError(#[from] AnalysisError),

    #[error("Input error: {0}")]
    InputError(#[from] InputError),

    #[error("App control error: {0}")]
    AppError(#[from] AppError),

    #[error("Planner error: {0}")]
    PlannerError(#[from] PlannerError),

    #[error("Safety error: {0}")]
    SafetyError(#[from] SafetyError),

    #[error("Configuration error: {0}")]
    ConfigError(#[from] ConfigError),
}

/// Result type for the vision system.
pub type Result<T> = std::result::Result<T, VisionError>;

/// The main Vision/VLA system facade.
///
/// This struct provides a high-level interface for all vision capabilities.
pub struct VisionSystem {
    /// System configuration
    config: VisionConfig,
    /// Safety guard
    safety: Arc<SafetyGuard>,
    /// Emergency stop state
    emergency_stop: Arc<RwLock<bool>>,
}

impl VisionSystem {
    /// Create a new vision system with the given configuration.
    pub fn new(config: VisionConfig) -> Self {
        let safety = Arc::new(SafetyGuard::new(&config));

        Self {
            config,
            safety,
            emergency_stop: Arc::new(RwLock::new(false)),
        }
    }

    /// Create a new vision system with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(VisionConfig::default())
    }

    /// Check if the vision system is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get the current configuration.
    pub fn config(&self) -> &VisionConfig {
        &self.config
    }

    /// Get the safety guard.
    pub fn safety(&self) -> &Arc<SafetyGuard> {
        &self.safety
    }

    /// Trigger emergency stop.
    pub async fn emergency_stop(&self) {
        let mut stop = self.emergency_stop.write().await;
        *stop = true;
        self.safety.trigger_emergency_stop().await;
    }

    /// Reset emergency stop.
    pub async fn reset_emergency_stop(&self) {
        let mut stop = self.emergency_stop.write().await;
        *stop = false;
        self.safety.reset_emergency_stop().await;
    }

    /// Check if emergency stop is active.
    pub async fn is_emergency_stop_active(&self) -> bool {
        *self.emergency_stop.read().await
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<()> {
        self.config.validate()?;
        Ok(())
    }

    /// Create a screen capture instance.
    #[cfg(feature = "gui-automation")]
    pub fn create_capture(&self) -> impl ScreenCapture {
        capture::create_screen_capture(self.config.capture.clone())
    }

    /// Create an input simulator instance.
    #[cfg(feature = "gui-automation")]
    pub fn create_input(&self) -> Result<impl InputSimulator> {
        input::create_input_simulator().map_err(VisionError::InputError)
    }

    /// Create a vision analyzer instance.
    pub fn create_analyzer(&self) -> Result<Box<dyn VisionAnalyzer>> {
        analysis::create_analyzer(&self.config).map_err(VisionError::AnalysisError)
    }

    /// Get safety statistics.
    pub async fn safety_stats(&self) -> SafetyStats {
        self.safety.get_stats().await
    }

    /// Check if an action is allowed by safety rules.
    pub async fn check_action(
        &self,
        action_type: ActionType,
        target_app: Option<&str>,
        description: &str,
    ) -> Result<()> {
        if !self.config.enabled {
            return Err(VisionError::NotEnabled);
        }

        self.safety
            .check_action(action_type, target_app, description)
            .await?;

        Ok(())
    }

    /// Flush audit logs.
    pub async fn flush_audit_logs(&self) -> Result<()> {
        self.safety.flush_audit_logs().await?;
        Ok(())
    }

    /// Check if in dry-run mode.
    pub fn is_dry_run(&self) -> bool {
        self.config.dry_run
    }
}

impl Default for VisionSystem {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Builder for creating a configured VisionSystem.
pub struct VisionSystemBuilder {
    config: VisionConfig,
}

impl VisionSystemBuilder {
    /// Create a new builder with default configuration.
    pub fn new() -> Self {
        Self {
            config: VisionConfig::default(),
        }
    }

    /// Enable the vision system.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    /// Set the vision model.
    pub fn model(mut self, model: VisionModel) -> Self {
        self.config = self.config.with_model(model);
        self
    }

    /// Set dry-run mode.
    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.config.dry_run = dry_run;
        self
    }

    /// Enable audit logging.
    pub fn audit_logging(mut self, enabled: bool) -> Self {
        self.config.audit_logging = enabled;
        self
    }

    /// Set audit log path.
    pub fn audit_log_path(mut self, path: impl Into<String>) -> Self {
        self.config.audit_log_path = Some(path.into());
        self
    }

    /// Set safety limits.
    pub fn safety_limits(mut self, limits: SafetyLimits) -> Self {
        self.config.safety = limits;
        self
    }

    /// Set app list configuration.
    pub fn app_config(mut self, config: AppListConfig) -> Self {
        self.config.apps = config;
        self
    }

    /// Build the VisionSystem.
    pub fn build(self) -> Result<VisionSystem> {
        self.config.validate()?;
        Ok(VisionSystem::new(self.config))
    }
}

impl Default for VisionSystemBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vision_system_creation() {
        let system = VisionSystem::with_defaults();
        assert!(!system.is_enabled()); // Disabled by default for safety
    }

    #[test]
    fn test_vision_system_builder() {
        let system = VisionSystemBuilder::new()
            .enabled(true)
            .dry_run(true)
            .audit_logging(true)
            .build()
            .unwrap();

        assert!(system.is_enabled());
        assert!(system.is_dry_run());
    }

    #[tokio::test]
    async fn test_emergency_stop() {
        let system = VisionSystem::with_defaults();

        assert!(!system.is_emergency_stop_active().await);

        system.emergency_stop().await;
        assert!(system.is_emergency_stop_active().await);

        system.reset_emergency_stop().await;
        assert!(!system.is_emergency_stop_active().await);
    }

    #[test]
    fn test_config_validation() {
        let system = VisionSystem::with_defaults();
        assert!(system.validate().is_ok());
    }
}
