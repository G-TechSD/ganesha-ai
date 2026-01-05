//! Fallback logging for unsupported platforms

use super::{GaneshaEvent, LogLevel};

pub struct SystemLogger;

impl SystemLogger {
    pub fn new() -> Self {
        SystemLogger
    }

    pub fn log(&self, event: GaneshaEvent) {
        let message = event.to_syslog_format();

        match event.level {
            LogLevel::Debug => tracing::debug!("{}", message),
            LogLevel::Info => tracing::info!("{}", message),
            LogLevel::Warning => tracing::warn!("{}", message),
            LogLevel::Error => tracing::error!("{}", message),
            LogLevel::Critical => tracing::error!(critical = true, "{}", message),
        }
    }
}

impl Default for SystemLogger {
    fn default() -> Self {
        Self::new()
    }
}
