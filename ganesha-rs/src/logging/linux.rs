//! Linux system logging via syslog/journald

use super::{GaneshaEvent, LogLevel};
use syslog::{Facility, Formatter3164};
use std::sync::Mutex;

pub struct SystemLogger {
    logger: Mutex<Option<syslog::Logger<syslog::LoggerBackend, Formatter3164>>>,
}

impl SystemLogger {
    pub fn new() -> Self {
        // Initialize syslog connection
        let formatter = Formatter3164 {
            facility: Facility::LOG_LOCAL0,
            hostname: None,
            process: "ganesha".into(),
            pid: std::process::id(),
        };

        let logger = syslog::unix(formatter).ok();
        SystemLogger { logger: Mutex::new(logger) }
    }

    pub fn log(&self, event: GaneshaEvent) {
        let message = event.to_syslog_format();

        if let Ok(mut guard) = self.logger.lock() {
            if let Some(ref mut logger) = *guard {
                // Use the info/warning/error methods based on level
                let _ = match event.level {
                    LogLevel::Debug => logger.debug(&message),
                    LogLevel::Info => logger.info(&message),
                    LogLevel::Warning => logger.warning(&message),
                    LogLevel::Error => logger.err(&message),
                    LogLevel::Critical => logger.crit(&message),
                };
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
