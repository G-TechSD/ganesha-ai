//! Desktop application configuration

use serde::{Deserialize, Serialize};

/// Desktop app configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopConfig {
    /// Window settings
    pub window: WindowConfig,
    /// Tray settings
    pub tray: TrayConfig,
    /// Border overlay settings
    pub border: BorderConfig,
    /// Hotkey settings
    pub hotkeys: HotkeyConfig,
    /// Theme settings
    pub theme: ThemeConfig,
}

impl Default for DesktopConfig {
    fn default() -> Self {
        Self {
            window: WindowConfig::default(),
            tray: TrayConfig::default(),
            border: BorderConfig::default(),
            hotkeys: HotkeyConfig::default(),
            theme: ThemeConfig::default(),
        }
    }
}

/// Window configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    /// Enable glass-like transparency
    pub glass_effect: bool,
    /// Window width
    pub width: u32,
    /// Window height
    pub height: u32,
    /// Start minimized to tray
    pub start_minimized: bool,
    /// Always on top
    pub always_on_top: bool,
    /// Window title
    pub title: String,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            glass_effect: true,
            width: 800,
            height: 600,
            start_minimized: false,
            always_on_top: false,
            title: "Ganesha - Obstacle Remover".to_string(),
        }
    }
}

/// System tray configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayConfig {
    /// Show in system tray
    pub enabled: bool,
    /// Minimize to tray on close
    pub minimize_to_tray: bool,
    /// Show notifications
    pub show_notifications: bool,
}

impl Default for TrayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            minimize_to_tray: true,
            show_notifications: true,
        }
    }
}

/// Border overlay configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BorderConfig {
    /// Show screen border when active
    pub enabled: bool,
    /// Border color (hex)
    pub color: String,
    /// Border width in pixels
    pub width: u32,
    /// Animation enabled
    pub animate: bool,
}

impl Default for BorderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            color: "#00FF00".to_string(),
            width: 4,
            animate: true,
        }
    }
}

/// Hotkey configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    /// Push-to-talk key
    pub push_to_talk: String,
    /// Toggle window visibility
    pub toggle_window: String,
    /// Emergency stop
    pub emergency_stop: String,
    /// Toggle voice mode
    pub toggle_voice: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            push_to_talk: "Ctrl+Space".to_string(),
            toggle_window: "Ctrl+Shift+G".to_string(),
            emergency_stop: "Escape".to_string(),
            toggle_voice: "Ctrl+Shift+V".to_string(),
        }
    }
}

/// Theme configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    /// Dark mode
    pub dark_mode: bool,
    /// Primary color
    pub primary_color: String,
    /// Accent color
    pub accent_color: String,
    /// Background opacity (0.0 - 1.0)
    pub background_opacity: f32,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            dark_mode: true,
            primary_color: "#6366f1".to_string(),
            accent_color: "#22c55e".to_string(),
            background_opacity: 0.85,
        }
    }
}

impl DesktopConfig {
    /// Load configuration from file
    pub fn load(path: &std::path::Path) -> crate::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        toml::from_str(&content)
            .map_err(|e| crate::DesktopError::ConfigError(e.to_string()))
    }

    /// Save configuration to file
    pub fn save(&self, path: &std::path::Path) -> crate::Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| crate::DesktopError::ConfigError(e.to_string()))?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Get default config path
    pub fn default_path() -> Option<std::path::PathBuf> {
        dirs::config_dir().map(|p| p.join("ganesha").join("desktop.toml"))
    }
}
