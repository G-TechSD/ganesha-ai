//! Windows system logging via Event Log
//!
//! View logs with:
//!   Event Viewer → Windows Logs → Application → Source: "Ganesha"
//!   PowerShell: Get-EventLog -LogName Application -Source Ganesha

use super::{GaneshaEvent, LogLevel};
use eventlog::EventLog;
use std::sync::OnceLock;

static LOGGER: OnceLock<EventLog> = OnceLock::new();

const SOURCE_NAME: &str = "Ganesha";

pub struct SystemLogger;

impl SystemLogger {
    pub fn new() -> Self {
        // Try to register event source (may need admin first time)
        if let Ok(log) = EventLog::new(SOURCE_NAME) {
            let _ = LOGGER.set(log);
        }
        SystemLogger
    }

    pub fn log(&self, event: GaneshaEvent) {
        let message = event.to_syslog_format();
        let event_id = event.event_id as u32;

        if let Some(logger) = LOGGER.get() {
            let result = match event.level {
                LogLevel::Debug | LogLevel::Info => {
                    logger.info(&format!("[{}] {}", event_id, message))
                }
                LogLevel::Warning => {
                    logger.warn(&format!("[{}] {}", event_id, message))
                }
                LogLevel::Error | LogLevel::Critical => {
                    logger.error(&format!("[{}] {}", event_id, message))
                }
            };

            if let Err(e) = result {
                eprintln!("EventLog error: {}", e);
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
