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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_desktop_config_defaults() {
        let config = DesktopConfig::default();
        assert!(config.window.glass_effect);
        assert_eq!(config.window.width, 800);
        assert_eq!(config.window.height, 600);
        assert!(!config.window.start_minimized);
        assert!(config.tray.enabled);
        assert!(config.tray.minimize_to_tray);
        assert!(config.border.enabled);
        assert_eq!(config.border.color, "#00FF00");
        assert_eq!(config.border.width, 4);
    }

    #[test]
    fn test_hotkey_defaults() {
        let config = HotkeyConfig::default();
        assert_eq!(config.push_to_talk, "Ctrl+Space");
        assert_eq!(config.toggle_window, "Ctrl+Shift+G");
        assert_eq!(config.emergency_stop, "Escape");
        assert_eq!(config.toggle_voice, "Ctrl+Shift+V");
    }

    #[test]
    fn test_theme_defaults() {
        let config = ThemeConfig::default();
        assert!(config.dark_mode);
        assert_eq!(config.primary_color, "#6366f1");
        assert_eq!(config.accent_color, "#22c55e");
        assert!(config.background_opacity > 0.0 && config.background_opacity <= 1.0);
    }

    #[test]
    fn test_config_serialization() {
        let config = DesktopConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let deserialized: DesktopConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(deserialized.window.width, 800);
        assert_eq!(deserialized.hotkeys.push_to_talk, "Ctrl+Space");
    }

    #[test]
    fn test_config_json_serialization() {
        let config = DesktopConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: DesktopConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.theme.primary_color, "#6366f1");
    }

    #[test]
    fn test_window_config() {
        let config = WindowConfig {
            glass_effect: false,
            width: 1200,
            height: 800,
            start_minimized: true,
            always_on_top: true,
            title: "Custom Title".to_string(),
        };
        assert!(!config.glass_effect);
        assert_eq!(config.width, 1200);
        assert!(config.always_on_top);
    }

    #[test]
    fn test_border_config_customization() {
        let config = BorderConfig {
            enabled: false,
            color: "#FF0000".to_string(),
            width: 8,
            animate: false,
        };
        assert!(!config.enabled);
        assert_eq!(config.color, "#FF0000");
        assert_eq!(config.width, 8);
    }

    #[test]
    fn test_tray_config() {
        let config = TrayConfig {
            enabled: false,
            minimize_to_tray: false,
            show_notifications: false,
        };
        assert!(!config.enabled);
        assert!(!config.minimize_to_tray);
    }
}
