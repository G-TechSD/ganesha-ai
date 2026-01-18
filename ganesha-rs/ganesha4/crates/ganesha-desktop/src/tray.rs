//! System tray integration

use crate::{config::DesktopConfig, Result};

/// Manages the system tray icon and menu
pub struct TrayManager {
    enabled: bool,
    minimize_to_tray: bool,
    show_notifications: bool,
}

impl TrayManager {
    /// Create a new tray manager
    pub fn new(config: &DesktopConfig) -> Result<Self> {
        Ok(Self {
            enabled: config.tray.enabled,
            minimize_to_tray: config.tray.minimize_to_tray,
            show_notifications: config.tray.show_notifications,
        })
    }

    /// Initialize the system tray
    pub async fn initialize(&mut self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // In Tauri:
        // - Create tray icon
        // - Set tooltip
        // - Create menu
        // - Register click handlers

        tracing::info!("System tray initialized");
        Ok(())
    }

    /// Build the tray menu
    fn build_menu(&self) -> TrayMenu {
        TrayMenu {
            items: vec![
                TrayMenuItem::item("Show Window", TrayAction::ShowWindow),
                TrayMenuItem::separator(),
                TrayMenuItem::submenu(
                    "Risk Level",
                    vec![
                        TrayMenuItem::radio("Safe", TrayAction::SetRiskLevel("Safe".to_string()), false),
                        TrayMenuItem::radio("Normal", TrayAction::SetRiskLevel("Normal".to_string()), true),
                        TrayMenuItem::radio("Trusted", TrayAction::SetRiskLevel("Trusted".to_string()), false),
                        TrayMenuItem::radio("Yolo", TrayAction::SetRiskLevel("Yolo".to_string()), false),
                    ],
                ),
                TrayMenuItem::submenu(
                    "Personality",
                    vec![
                        TrayMenuItem::radio("Professional", TrayAction::SetPersonality("Professional".to_string()), true),
                        TrayMenuItem::radio("Friendly", TrayAction::SetPersonality("Friendly".to_string()), false),
                        TrayMenuItem::radio("Mentor", TrayAction::SetPersonality("Mentor".to_string()), false),
                        TrayMenuItem::radio("Pirate", TrayAction::SetPersonality("Pirate".to_string()), false),
                    ],
                ),
                TrayMenuItem::separator(),
                TrayMenuItem::check("Voice Mode", TrayAction::ToggleVoice, false),
                TrayMenuItem::check("Always on Top", TrayAction::ToggleAlwaysOnTop, false),
                TrayMenuItem::separator(),
                TrayMenuItem::item("Settings", TrayAction::OpenSettings),
                TrayMenuItem::item("About", TrayAction::ShowAbout),
                TrayMenuItem::separator(),
                TrayMenuItem::item("Quit", TrayAction::Quit),
            ],
        }
    }

    /// Update the tray icon
    pub fn set_icon(&mut self, icon: TrayIcon) -> Result<()> {
        // In Tauri: tray.set_icon(icon)
        tracing::debug!("Tray icon set to {:?}", icon);
        Ok(())
    }

    /// Update the tray tooltip
    pub fn set_tooltip(&mut self, tooltip: &str) -> Result<()> {
        // In Tauri: tray.set_tooltip(tooltip)
        tracing::debug!("Tray tooltip: {}", tooltip);
        Ok(())
    }

    /// Show a notification
    pub fn show_notification(&self, title: &str, body: &str) -> Result<()> {
        if !self.show_notifications {
            return Ok(());
        }

        // In Tauri: Notification::new().title(title).body(body).show()
        tracing::info!("Notification: {} - {}", title, body);
        Ok(())
    }

    /// Show notification for processing complete
    pub fn notify_processing_complete(&self, summary: &str) -> Result<()> {
        self.show_notification("Ganesha", &format!("Task complete: {}", summary))
    }

    /// Show notification for error
    pub fn notify_error(&self, error: &str) -> Result<()> {
        self.show_notification("Ganesha Error", error)
    }

    /// Check if tray is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Check if should minimize to tray
    pub fn should_minimize_to_tray(&self) -> bool {
        self.minimize_to_tray
    }
}

/// Tray icon states
#[derive(Debug, Clone, Copy)]
pub enum TrayIcon {
    /// Default idle icon
    Idle,
    /// Processing/working icon
    Working,
    /// Listening for voice input
    Listening,
    /// Error state
    Error,
    /// Success/complete
    Success,
}

/// Tray menu structure
#[derive(Debug, Clone)]
pub struct TrayMenu {
    pub items: Vec<TrayMenuItem>,
}

/// Individual menu item
#[derive(Debug, Clone)]
pub enum TrayMenuItem {
    /// Regular clickable item
    Item { label: String, action: TrayAction },
    /// Separator line
    Separator,
    /// Submenu
    Submenu { label: String, items: Vec<TrayMenuItem> },
    /// Checkable item
    Check { label: String, action: TrayAction, checked: bool },
    /// Radio item (mutually exclusive in group)
    Radio { label: String, action: TrayAction, selected: bool },
}

impl TrayMenuItem {
    pub fn item(label: &str, action: TrayAction) -> Self {
        Self::Item {
            label: label.to_string(),
            action,
        }
    }

    pub fn separator() -> Self {
        Self::Separator
    }

    pub fn submenu(label: &str, items: Vec<TrayMenuItem>) -> Self {
        Self::Submenu {
            label: label.to_string(),
            items,
        }
    }

    pub fn check(label: &str, action: TrayAction, checked: bool) -> Self {
        Self::Check {
            label: label.to_string(),
            action,
            checked,
        }
    }

    pub fn radio(label: &str, action: TrayAction, selected: bool) -> Self {
        Self::Radio {
            label: label.to_string(),
            action,
            selected,
        }
    }
}

/// Actions triggered by tray menu
#[derive(Debug, Clone)]
pub enum TrayAction {
    ShowWindow,
    HideWindow,
    ToggleVoice,
    ToggleAlwaysOnTop,
    SetRiskLevel(String),
    SetPersonality(String),
    OpenSettings,
    ShowAbout,
    Quit,
}
