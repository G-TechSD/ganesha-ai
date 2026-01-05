//! Cross-Platform System Logging for Ganesha
//!
//! Provides unified logging interface that writes to:
//! - Linux: journald/syslog
//! - macOS: Unified Log (os_log)
//! - Windows: Event Viewer
//!
//! Filter commands:
//! - Linux: `journalctl -t ganesha`
//! - macOS: `log show --predicate 'subsystem == "com.gtechsd.ganesha"'`
//! - Windows: Event Viewer → Applications → Source: "Ganesha"

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Event IDs for filtering in system logs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum EventId {
    // Informational (1000-1099)
    DaemonStart = 1000,
    DaemonStop = 1001,
    CommandExecuted = 1010,
    CommandPlanned = 1011,
    SessionStart = 1020,
    SessionEnd = 1021,

    // Warnings (1100-1199)
    HighRiskApproved = 1100,
    ConfigChanged = 1110,
    AccessLevelChanged = 1111,
    ElevatedAccessUsed = 1130,

    // Errors (1200-1299)
    CommandDenied = 1200,
    AccessViolation = 1201,
    ExecutionFailed = 1230,
    Timeout = 1240,

    // Critical (1300-1399)
    ManipulationDetected = 1300,
    SelfInvocationBlocked = 1301,
    SecurityBreachAttempt = 1310,
    CriticalCommandBlocked = 1320,
    LogTamperingAttempt = 1330,
}

/// Log severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warning => write!(f, "WARNING"),
            LogLevel::Error => write!(f, "ERROR"),
            LogLevel::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Structured log event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaneshaEvent {
    pub timestamp: DateTime<Utc>,
    pub event_id: EventId,
    pub level: LogLevel,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

impl GaneshaEvent {
    pub fn new(event_id: EventId, level: LogLevel, message: impl Into<String>) -> Self {
        Self {
            timestamp: Utc::now(),
            event_id,
            level,
            message: message.into(),
            user: None,
            command: None,
            risk_level: None,
            allowed: None,
            reason: None,
            session_id: None,
        }
    }

    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    pub fn with_command(mut self, cmd: impl Into<String>) -> Self {
        let cmd = cmd.into();
        // Truncate for log safety
        self.command = Some(if cmd.len() > 500 { cmd[..500].to_string() } else { cmd });
        self
    }

    pub fn with_risk(mut self, risk: impl Into<String>) -> Self {
        self.risk_level = Some(risk.into());
        self
    }

    pub fn with_allowed(mut self, allowed: bool) -> Self {
        self.allowed = Some(allowed);
        self
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Format for syslog-style output
    pub fn to_syslog_format(&self) -> String {
        let mut parts = vec![
            format!("GANESHA[{}]", self.event_id as u32),
            format!("level={}", self.level),
        ];

        if let Some(ref user) = self.user {
            parts.push(format!("user={}", user));
        }
        if let Some(ref cmd) = self.command {
            let escaped = cmd.replace('"', "\\\"").replace('\n', " ");
            parts.push(format!("cmd=\"{}\"", escaped));
        }
        if let Some(ref risk) = self.risk_level {
            parts.push(format!("risk={}", risk));
        }
        if let Some(allowed) = self.allowed {
            parts.push(format!("allowed={}", if allowed { "yes" } else { "no" }));
        }
        if let Some(ref reason) = self.reason {
            parts.push(format!("reason=\"{}\"", reason));
        }
        if let Some(ref session) = self.session_id {
            parts.push(format!("session={}", &session[..8.min(session.len())]));
        }

        parts.push(format!("msg={}", self.message));
        parts.join(" ")
    }
}

// Platform-specific implementations
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

// Re-export the platform-specific logger
#[cfg(target_os = "linux")]
pub use linux::SystemLogger;
#[cfg(target_os = "macos")]
pub use macos::SystemLogger;
#[cfg(target_os = "windows")]
pub use windows::SystemLogger;

// Fallback for other platforms
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
mod fallback;
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub use fallback::SystemLogger;

/// Convenience functions
impl SystemLogger {
    pub fn command_executed(&self, user: &str, command: &str, risk: &str, session: &str) {
        self.log(
            GaneshaEvent::new(EventId::CommandExecuted, LogLevel::Info, "Command executed")
                .with_user(user)
                .with_command(command)
                .with_risk(risk)
                .with_allowed(true)
                .with_session(session),
        );
    }

    pub fn command_denied(&self, user: &str, command: &str, reason: &str) {
        self.log(
            GaneshaEvent::new(EventId::CommandDenied, LogLevel::Warning, "Command denied")
                .with_user(user)
                .with_command(command)
                .with_allowed(false)
                .with_reason(reason),
        );
    }

    pub fn self_invocation_blocked(&self, user: &str, command: &str) {
        self.log(
            GaneshaEvent::new(
                EventId::SelfInvocationBlocked,
                LogLevel::Critical,
                "SECURITY: Self-invocation blocked",
            )
            .with_user(user)
            .with_command(command)
            .with_risk("critical")
            .with_allowed(false)
            .with_reason("Ganesha cannot call itself with bypass flags"),
        );
    }

    pub fn manipulation_detected(&self, user: &str, command: &str, indicator: &str) {
        self.log(
            GaneshaEvent::new(
                EventId::ManipulationDetected,
                LogLevel::Critical,
                "SECURITY: Manipulation attempt detected",
            )
            .with_user(user)
            .with_command(command)
            .with_risk("critical")
            .with_allowed(false)
            .with_reason(format!("Matched: {}", indicator)),
        );
    }

    pub fn daemon_start(&self, access_level: &str) {
        self.log(GaneshaEvent::new(
            EventId::DaemonStart,
            LogLevel::Info,
            format!("Daemon started with access level: {}", access_level),
        ));
    }

    pub fn daemon_stop(&self) {
        self.log(GaneshaEvent::new(
            EventId::DaemonStop,
            LogLevel::Info,
            "Daemon stopped",
        ));
    }
}
