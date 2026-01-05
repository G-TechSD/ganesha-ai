//! macOS system logging via Unified Logging (os_log)
//!
//! View logs with:
//!   log show --predicate 'subsystem == "com.gtechsd.ganesha"' --last 1h
//!   log stream --predicate 'subsystem == "com.gtechsd.ganesha"'

use super::{GaneshaEvent, LogLevel};
use oslog::OsLog;
use std::sync::OnceLock;

static LOGGER: OnceLock<OsLog> = OnceLock::new();

const SUBSYSTEM: &str = "com.gtechsd.ganesha";
const CATEGORY: &str = "security";

pub struct SystemLogger;

impl SystemLogger {
    pub fn new() -> Self {
        let log = OsLog::new(SUBSYSTEM, CATEGORY);
        let _ = LOGGER.set(log);
        SystemLogger
    }

    pub fn log(&self, event: GaneshaEvent) {
        let message = event.to_syslog_format();

        if let Some(logger) = LOGGER.get() {
            match event.level {
                LogLevel::Debug => logger.with_level(oslog::Level::Debug, &message),
                LogLevel::Info => logger.with_level(oslog::Level::Info, &message),
                LogLevel::Warning => logger.with_level(oslog::Level::Default, &message),
                LogLevel::Error => logger.with_level(oslog::Level::Error, &message),
                LogLevel::Critical => logger.with_level(oslog::Level::Fault, &message),
            }
        }

        // Also write to tracing for console output
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
