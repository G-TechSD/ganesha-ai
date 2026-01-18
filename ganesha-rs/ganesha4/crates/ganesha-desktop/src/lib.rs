//! # Ganesha Desktop
//!
//! Desktop GUI application "Obstacle Remover" built with Tauri 2.0.
//!
//! ## Features
//!
//! - Glass-like transparent UI
//! - Push-to-talk voice activation
//! - System tray integration
//! - Screen border indicator when Ganesha has control
//! - Cross-platform (Windows, macOS, Linux)
//!
//! ## Coming Soon
//!
//! This crate is a placeholder for the desktop application.
//! The actual implementation will use Tauri 2.0 for the native wrapper
//! and a web frontend for the UI.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DesktopError {
    #[error("Window error: {0}")]
    WindowError(String),

    #[error("Tray error: {0}")]
    TrayError(String),
}

pub type Result<T> = std::result::Result<T, DesktopError>;

/// Desktop app configuration
#[derive(Debug, Clone)]
pub struct DesktopConfig {
    /// Enable glass-like transparency
    pub glass_effect: bool,
    /// Show in system tray
    pub show_tray: bool,
    /// Start minimized to tray
    pub start_minimized: bool,
    /// Show screen border when active
    pub show_border_indicator: bool,
    /// Border color when active (green)
    pub border_color: String,
}

impl Default for DesktopConfig {
    fn default() -> Self {
        Self {
            glass_effect: true,
            show_tray: true,
            start_minimized: false,
            show_border_indicator: true,
            border_color: "#00FF00".to_string(),
        }
    }
}
