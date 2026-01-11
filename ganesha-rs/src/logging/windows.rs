//! Windows system logging via Event Log
//!
//! View logs with:
//!   Event Viewer → Windows Logs → Application → Source: "Ganesha"
//!   PowerShell: Get-EventLog -LogName Application -Source Ganesha

use super::{GaneshaEvent, LogLevel};

pub struct SystemLogger;

impl SystemLogger {
    pub fn new() -> Self {
        SystemLogger
    }

    pub fn log(&self, event: GaneshaEvent) {
        let message = event.to_syslog_format();

        // Write to tracing (will show in console and can be captured)
        match event.level {
            LogLevel::Debug => tracing::debug!("{}", message),
            LogLevel::Info => tracing::info!("{}", message),
            LogLevel::Warning => tracing::warn!("{}", message),
            LogLevel::Error => tracing::error!("{}", message),
            LogLevel::Critical => tracing::error!(critical = true, "{}", message),
        }

        // TODO: Windows Event Log integration when eventlog crate API stabilizes
        // For now, logs go to stdout/stderr via tracing
    }
}

impl Default for SystemLogger {
    fn default() -> Self {
        Self::new()
    }
}
