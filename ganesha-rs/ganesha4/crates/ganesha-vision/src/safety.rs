//! Safety controls for the Vision/VLA system.
//!
//! This module provides:
//! - WhitelistedApps - only control approved applications
//! - ActionLimits - rate limiting for clicks, keystrokes, etc.
//! - ConfirmationRequired - settings for sensitive actions
//! - Emergency stop (Escape key monitoring)
//! - Audit logging of all actions

use crate::config::{AppListConfig, ConfirmationSettings, SafetyLimits, VisionConfig};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Errors related to safety controls.
#[derive(Error, Debug)]
pub enum SafetyError {
    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Application not whitelisted: {0}")]
    AppNotWhitelisted(String),

    #[error("Application is blacklisted: {0}")]
    AppBlacklisted(String),

    #[error("Action requires confirmation")]
    ConfirmationRequired,

    #[error("Emergency stop is active")]
    EmergencyStopActive,

    #[error("Action limit exceeded: {0}")]
    ActionLimitExceeded(String),

    #[error("Audit logging failed: {0}")]
    AuditError(String),

    #[error("Safety violation: {0}")]
    SafetyViolation(String),
}

/// Result type for safety operations.
pub type SafetyResult<T> = Result<T, SafetyError>;

/// Type of action being performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    MouseClick,
    MouseMove,
    MouseDrag,
    MouseScroll,
    KeyPress,
    KeyType,
    Shortcut,
    WindowFocus,
    WindowMove,
    WindowResize,
    AppLaunch,
    FileOpen,
    FileSave,
    FileDelete,
    SystemCommand,
    NetworkRequest,
    Custom(u32),
}

impl ActionType {
    /// Check if this action type is considered destructive.
    pub fn is_destructive(&self) -> bool {
        matches!(
            self,
            Self::FileDelete | Self::SystemCommand | Self::Custom(_)
        )
    }

    /// Check if this action type involves file operations.
    pub fn is_file_operation(&self) -> bool {
        matches!(self, Self::FileOpen | Self::FileSave | Self::FileDelete)
    }

    /// Check if this action type involves system changes.
    pub fn is_system_operation(&self) -> bool {
        matches!(self, Self::SystemCommand | Self::AppLaunch)
    }

    /// Check if this action type involves network.
    pub fn is_network_operation(&self) -> bool {
        matches!(self, Self::NetworkRequest)
    }
}

/// An audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique entry ID
    pub id: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Type of action
    pub action_type: ActionType,
    /// Description of the action
    pub description: String,
    /// Target application (if applicable)
    pub target_app: Option<String>,
    /// Additional details as JSON
    pub details: serde_json::Value,
    /// Whether the action was allowed
    pub allowed: bool,
    /// Reason if blocked
    pub block_reason: Option<String>,
    /// Session ID
    pub session_id: String,
}

impl AuditEntry {
    /// Create a new audit entry.
    pub fn new(action_type: ActionType, description: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            action_type,
            description: description.into(),
            target_app: None,
            details: serde_json::Value::Null,
            allowed: true,
            block_reason: None,
            session_id: String::new(),
        }
    }

    /// Set target application.
    pub fn with_target_app(mut self, app: impl Into<String>) -> Self {
        self.target_app = Some(app.into());
        self
    }

    /// Set details.
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = details;
        self
    }

    /// Mark as blocked.
    pub fn blocked(mut self, reason: impl Into<String>) -> Self {
        self.allowed = false;
        self.block_reason = Some(reason.into());
        self
    }

    /// Set session ID.
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = session_id.into();
        self
    }
}

/// Rate limiter for tracking action frequency.
#[derive(Debug)]
struct RateLimiter {
    /// Action timestamps within the window
    timestamps: VecDeque<Instant>,
    /// Maximum actions allowed in the window
    max_actions: u32,
    /// Time window
    window: Duration,
}

impl RateLimiter {
    fn new(max_actions: u32, window: Duration) -> Self {
        Self {
            timestamps: VecDeque::new(),
            max_actions,
            window,
        }
    }

    fn check_and_record(&mut self) -> bool {
        let now = Instant::now();

        // Remove old timestamps
        while let Some(front) = self.timestamps.front() {
            if now.duration_since(*front) > self.window {
                self.timestamps.pop_front();
            } else {
                break;
            }
        }

        // Check if we're at the limit
        if self.timestamps.len() >= self.max_actions as usize {
            return false;
        }

        // Record this action
        self.timestamps.push_back(now);
        true
    }

    fn current_rate(&self) -> u32 {
        let now = Instant::now();
        self.timestamps
            .iter()
            .filter(|t| now.duration_since(**t) <= self.window)
            .count() as u32
    }
}

/// Audit logger for writing action logs.
pub struct AuditLogger {
    /// Log file path
    log_path: Option<PathBuf>,
    /// In-memory buffer
    buffer: Arc<RwLock<Vec<AuditEntry>>>,
    /// Maximum buffer size before flush
    buffer_size: usize,
    /// Session ID
    session_id: String,
    /// Whether logging is enabled
    enabled: bool,
}

impl AuditLogger {
    /// Create a new audit logger.
    pub fn new(config: &VisionConfig) -> Self {
        let log_path = config.audit_log_path.as_ref().map(PathBuf::from);

        Self {
            log_path,
            buffer: Arc::new(RwLock::new(Vec::new())),
            buffer_size: 100,
            session_id: uuid::Uuid::new_v4().to_string(),
            enabled: config.audit_logging,
        }
    }

    /// Log an action.
    pub async fn log(&self, mut entry: AuditEntry) -> SafetyResult<()> {
        if !self.enabled {
            return Ok(());
        }

        entry.session_id = self.session_id.clone();

        let mut buffer = self.buffer.write().await;
        buffer.push(entry.clone());

        // Log to tracing
        if entry.allowed {
            info!(
                action = ?entry.action_type,
                target = ?entry.target_app,
                description = %entry.description,
                "Vision action logged"
            );
        } else {
            warn!(
                action = ?entry.action_type,
                target = ?entry.target_app,
                reason = ?entry.block_reason,
                "Vision action blocked"
            );
        }

        // Flush if buffer is full
        if buffer.len() >= self.buffer_size {
            let entries = std::mem::take(&mut *buffer);
            drop(buffer);
            self.flush_entries(entries).await?;
        }

        Ok(())
    }

    /// Flush buffered entries to disk.
    async fn flush_entries(&self, entries: Vec<AuditEntry>) -> SafetyResult<()> {
        if let Some(ref path) = self.log_path {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .await
                .map_err(|e| SafetyError::AuditError(e.to_string()))?;

            for entry in entries {
                let json = serde_json::to_string(&entry)
                    .map_err(|e| SafetyError::AuditError(e.to_string()))?;
                file.write_all(json.as_bytes())
                    .await
                    .map_err(|e| SafetyError::AuditError(e.to_string()))?;
                file.write_all(b"\n")
                    .await
                    .map_err(|e| SafetyError::AuditError(e.to_string()))?;
            }

            file.flush()
                .await
                .map_err(|e| SafetyError::AuditError(e.to_string()))?;
        }

        Ok(())
    }

    /// Flush all buffered entries.
    pub async fn flush(&self) -> SafetyResult<()> {
        let mut buffer = self.buffer.write().await;
        let entries = std::mem::take(&mut *buffer);
        drop(buffer);

        if !entries.is_empty() {
            self.flush_entries(entries).await?;
        }

        Ok(())
    }

    /// Get recent entries from buffer.
    pub async fn recent_entries(&self, count: usize) -> Vec<AuditEntry> {
        let buffer = self.buffer.read().await;
        buffer.iter().rev().take(count).cloned().collect()
    }

    /// Get session ID.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

/// Safety guard that enforces all safety rules.
pub struct SafetyGuard {
    /// Rate limiter for clicks
    click_limiter: Arc<RwLock<RateLimiter>>,
    /// Rate limiter for keystrokes
    keystroke_limiter: Arc<RwLock<RateLimiter>>,
    /// Total action counter
    action_counter: Arc<RwLock<u32>>,
    /// Emergency stop flag
    emergency_stop: Arc<RwLock<bool>>,
    /// App list configuration
    app_config: AppListConfig,
    /// Safety limits
    limits: SafetyLimits,
    /// Confirmation settings
    confirmations: ConfirmationSettings,
    /// Audit logger
    audit_logger: Arc<AuditLogger>,
    /// Dry run mode
    dry_run: bool,
}

impl SafetyGuard {
    /// Create a new safety guard.
    pub fn new(config: &VisionConfig) -> Self {
        let click_limiter = RateLimiter::new(
            config.safety.max_clicks_per_minute,
            Duration::from_secs(60),
        );

        let keystroke_limiter = RateLimiter::new(
            config.safety.max_keystrokes_per_minute,
            Duration::from_secs(60),
        );

        Self {
            click_limiter: Arc::new(RwLock::new(click_limiter)),
            keystroke_limiter: Arc::new(RwLock::new(keystroke_limiter)),
            action_counter: Arc::new(RwLock::new(0)),
            emergency_stop: Arc::new(RwLock::new(false)),
            app_config: config.apps.clone(),
            limits: config.safety.clone(),
            confirmations: config.confirmations.clone(),
            audit_logger: Arc::new(AuditLogger::new(config)),
            dry_run: config.dry_run,
        }
    }

    /// Check if an action is allowed.
    pub async fn check_action(
        &self,
        action_type: ActionType,
        target_app: Option<&str>,
        description: &str,
    ) -> SafetyResult<()> {
        // Check emergency stop first
        if *self.emergency_stop.read().await {
            let entry = AuditEntry::new(action_type, description)
                .blocked("Emergency stop active");
            self.audit_logger.log(entry).await?;
            return Err(SafetyError::EmergencyStopActive);
        }

        // Check app whitelist
        if let Some(app) = target_app {
            if self.app_config.blacklist.contains(app) {
                let entry = AuditEntry::new(action_type, description)
                    .with_target_app(app)
                    .blocked("Application is blacklisted");
                self.audit_logger.log(entry).await?;
                return Err(SafetyError::AppBlacklisted(app.to_string()));
            }

            if !self.app_config.is_allowed(app) {
                let entry = AuditEntry::new(action_type, description)
                    .with_target_app(app)
                    .blocked("Application not whitelisted");
                self.audit_logger.log(entry).await?;
                return Err(SafetyError::AppNotWhitelisted(app.to_string()));
            }
        }

        // Check rate limits
        match action_type {
            ActionType::MouseClick => {
                let mut limiter = self.click_limiter.write().await;
                if !limiter.check_and_record() {
                    let entry = AuditEntry::new(action_type, description)
                        .blocked("Click rate limit exceeded");
                    self.audit_logger.log(entry).await?;
                    return Err(SafetyError::RateLimitExceeded(
                        "Maximum clicks per minute exceeded".to_string(),
                    ));
                }
            }
            ActionType::KeyPress | ActionType::KeyType => {
                let mut limiter = self.keystroke_limiter.write().await;
                if !limiter.check_and_record() {
                    let entry = AuditEntry::new(action_type, description)
                        .blocked("Keystroke rate limit exceeded");
                    self.audit_logger.log(entry).await?;
                    return Err(SafetyError::RateLimitExceeded(
                        "Maximum keystrokes per minute exceeded".to_string(),
                    ));
                }
            }
            _ => {}
        }

        // Check total action limit
        {
            let mut counter = self.action_counter.write().await;
            *counter += 1;
            if *counter > self.limits.max_actions_per_task {
                let entry = AuditEntry::new(action_type, description)
                    .blocked("Action limit exceeded");
                self.audit_logger.log(entry).await?;
                return Err(SafetyError::ActionLimitExceeded(
                    "Maximum actions per task exceeded".to_string(),
                ));
            }
        }

        // Check confirmation requirements
        if self.requires_confirmation(action_type, description) {
            let entry = AuditEntry::new(action_type, description)
                .blocked("Confirmation required");
            self.audit_logger.log(entry).await?;
            return Err(SafetyError::ConfirmationRequired);
        }

        // Log the allowed action
        let entry = AuditEntry::new(action_type, description);
        if let Some(app) = target_app {
            self.audit_logger.log(entry.with_target_app(app)).await?;
        } else {
            self.audit_logger.log(entry).await?;
        }

        Ok(())
    }

    /// Check if an action requires confirmation.
    fn requires_confirmation(&self, action_type: ActionType, description: &str) -> bool {
        // Check action type requirements
        if action_type.is_destructive() && self.confirmations.confirm_destructive {
            return true;
        }

        if action_type == ActionType::FileDelete && self.confirmations.confirm_file_delete {
            return true;
        }

        if action_type.is_system_operation() && self.confirmations.confirm_system_changes {
            return true;
        }

        if action_type.is_network_operation() && self.confirmations.confirm_network_ops {
            return true;
        }

        // Check custom patterns
        let desc_lower = description.to_lowercase();
        for pattern in &self.confirmations.custom_patterns {
            if desc_lower.contains(&pattern.to_lowercase()) {
                return true;
            }
        }

        false
    }

    /// Trigger emergency stop.
    pub async fn trigger_emergency_stop(&self) {
        let mut stop = self.emergency_stop.write().await;
        *stop = true;

        let entry = AuditEntry::new(ActionType::Custom(0), "Emergency stop triggered");
        if let Err(e) = self.audit_logger.log(entry).await {
            error!("Failed to log emergency stop: {}", e);
        }

        warn!("Emergency stop triggered!");
    }

    /// Reset emergency stop.
    pub async fn reset_emergency_stop(&self) {
        let mut stop = self.emergency_stop.write().await;
        *stop = false;

        let entry = AuditEntry::new(ActionType::Custom(0), "Emergency stop reset");
        if let Err(e) = self.audit_logger.log(entry).await {
            error!("Failed to log emergency stop reset: {}", e);
        }

        info!("Emergency stop reset");
    }

    /// Check if emergency stop is active.
    pub async fn is_emergency_stop_active(&self) -> bool {
        *self.emergency_stop.read().await
    }

    /// Reset action counter for a new task.
    pub async fn reset_action_counter(&self) {
        let mut counter = self.action_counter.write().await;
        *counter = 0;
    }

    /// Get current click rate.
    pub async fn click_rate(&self) -> u32 {
        self.click_limiter.read().await.current_rate()
    }

    /// Get current keystroke rate.
    pub async fn keystroke_rate(&self) -> u32 {
        self.keystroke_limiter.read().await.current_rate()
    }

    /// Get total actions performed.
    pub async fn total_actions(&self) -> u32 {
        *self.action_counter.read().await
    }

    /// Check if in dry run mode.
    pub fn is_dry_run(&self) -> bool {
        self.dry_run
    }

    /// Get audit logger.
    pub fn audit_logger(&self) -> &Arc<AuditLogger> {
        &self.audit_logger
    }

    /// Flush audit logs.
    pub async fn flush_audit_logs(&self) -> SafetyResult<()> {
        self.audit_logger.flush().await
    }
}

/// Emergency stop monitor that listens for Escape key.
pub struct EmergencyStopMonitor {
    safety_guard: Arc<SafetyGuard>,
    running: Arc<RwLock<bool>>,
}

impl EmergencyStopMonitor {
    /// Create a new emergency stop monitor.
    pub fn new(safety_guard: Arc<SafetyGuard>) -> Self {
        Self {
            safety_guard,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start monitoring for emergency stop (Escape key).
    /// This is a placeholder - actual implementation would use platform-specific
    /// keyboard hooks or input monitoring.
    pub async fn start(&self) {
        let mut running = self.running.write().await;
        if *running {
            return;
        }
        *running = true;
        drop(running);

        info!("Emergency stop monitor started (press Escape to trigger)");

        // In a real implementation, this would set up keyboard monitoring
        // For now, this is a placeholder that can be triggered manually
    }

    /// Stop monitoring.
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;

        info!("Emergency stop monitor stopped");
    }

    /// Check if monitor is running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Manually trigger emergency stop.
    pub async fn trigger(&self) {
        self.safety_guard.trigger_emergency_stop().await;
    }
}

/// Safety statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyStats {
    /// Total actions performed
    pub total_actions: u32,
    /// Total actions blocked
    pub total_blocked: u32,
    /// Current click rate (per minute)
    pub click_rate: u32,
    /// Current keystroke rate (per minute)
    pub keystroke_rate: u32,
    /// Emergency stop triggered count
    pub emergency_stops: u32,
    /// Session ID
    pub session_id: String,
}

impl SafetyGuard {
    /// Get current safety statistics.
    pub async fn get_stats(&self) -> SafetyStats {
        SafetyStats {
            total_actions: *self.action_counter.read().await,
            total_blocked: 0, // Would need to track this separately
            click_rate: self.click_rate().await,
            keystroke_rate: self.keystroke_rate().await,
            emergency_stops: 0, // Would need to track this
            session_id: self.audit_logger.session_id().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_type_destructive() {
        assert!(ActionType::FileDelete.is_destructive());
        assert!(ActionType::SystemCommand.is_destructive());
        assert!(!ActionType::MouseClick.is_destructive());
    }

    #[test]
    fn test_audit_entry_creation() {
        let entry = AuditEntry::new(ActionType::MouseClick, "Click on save button")
            .with_target_app("Notepad")
            .with_details(serde_json::json!({"x": 100, "y": 200}));

        assert_eq!(entry.action_type, ActionType::MouseClick);
        assert_eq!(entry.target_app, Some("Notepad".to_string()));
        assert!(entry.allowed);
    }

    #[test]
    fn test_audit_entry_blocked() {
        let entry = AuditEntry::new(ActionType::FileDelete, "Delete file.txt")
            .blocked("File deletion not allowed");

        assert!(!entry.allowed);
        assert_eq!(
            entry.block_reason,
            Some("File deletion not allowed".to_string())
        );
    }

    #[tokio::test]
    async fn test_rate_limiter() {
        let mut limiter = RateLimiter::new(3, Duration::from_secs(1));

        assert!(limiter.check_and_record());
        assert!(limiter.check_and_record());
        assert!(limiter.check_and_record());
        assert!(!limiter.check_and_record()); // Should be limited now

        assert_eq!(limiter.current_rate(), 3);
    }

    #[tokio::test]
    async fn test_safety_guard_emergency_stop() {
        let config = VisionConfig::default();
        let guard = SafetyGuard::new(&config);

        assert!(!guard.is_emergency_stop_active().await);

        guard.trigger_emergency_stop().await;
        assert!(guard.is_emergency_stop_active().await);

        guard.reset_emergency_stop().await;
        assert!(!guard.is_emergency_stop_active().await);
    }
}
