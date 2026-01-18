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
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Obstacle Remover                          │
//! │                                                             │
//! │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐   │
//! │  │  Window  │  │   Tray   │  │  Border  │  │  Hotkey  │   │
//! │  │ Manager  │  │ Manager  │  │ Overlay  │  │ Handler  │   │
//! │  └──────────┘  └──────────┘  └──────────┘  └──────────┘   │
//! │                                                             │
//! │  ┌──────────┐  ┌──────────┐  ┌──────────┐                  │
//! │  │   IPC    │  │  State   │  │  Config  │                  │
//! │  │ Commands │  │  Store   │  │  Manager │                  │
//! │  └──────────┘  └──────────┘  └──────────┘                  │
//! │                                                             │
//! └─────────────────────────────────────────────────────────────┘
//! ```

pub mod config;
pub mod window;
pub mod tray;
pub mod border;
pub mod hotkey;
pub mod state;
pub mod commands;

pub use config::DesktopConfig;
pub use window::WindowManager;
pub use tray::TrayManager;
pub use border::BorderOverlay;
pub use hotkey::HotkeyManager;
pub use state::AppState;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DesktopError {
    #[error("Window error: {0}")]
    WindowError(String),

    #[error("Tray error: {0}")]
    TrayError(String),

    #[error("Hotkey error: {0}")]
    HotkeyError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("IPC error: {0}")]
    IpcError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, DesktopError>;

/// Main desktop application
#[allow(dead_code)]
pub struct DesktopApp {
    config: DesktopConfig,
    state: AppState,
    window_manager: WindowManager,
    tray_manager: TrayManager,
    border_overlay: BorderOverlay,
    hotkey_manager: HotkeyManager,
}

impl DesktopApp {
    /// Create a new desktop application instance
    pub fn new(config: DesktopConfig) -> Result<Self> {
        Ok(Self {
            state: AppState::new(),
            window_manager: WindowManager::new(&config)?,
            tray_manager: TrayManager::new(&config)?,
            border_overlay: BorderOverlay::new(&config)?,
            hotkey_manager: HotkeyManager::new(&config)?,
            config,
        })
    }

    /// Run the desktop application
    pub async fn run(&mut self) -> Result<()> {
        // Initialize components
        self.window_manager.initialize().await?;
        self.tray_manager.initialize().await?;
        self.hotkey_manager.start_listening().await?;

        // Main event loop would go here
        // In actual Tauri app, this is handled by Tauri's event loop

        Ok(())
    }

    /// Show the main window
    pub fn show_window(&mut self) -> Result<()> {
        self.window_manager.show()
    }

    /// Hide the main window
    pub fn hide_window(&mut self) -> Result<()> {
        self.window_manager.hide()
    }

    /// Toggle window visibility
    pub fn toggle_window(&mut self) -> Result<()> {
        if self.state.window_visible {
            self.hide_window()
        } else {
            self.show_window()
        }
    }

    /// Show the border overlay (when Ganesha has control)
    pub fn show_border(&mut self) -> Result<()> {
        self.border_overlay.show()?;
        self.state.border_visible = true;
        Ok(())
    }

    /// Hide the border overlay
    pub fn hide_border(&mut self) -> Result<()> {
        self.border_overlay.hide()?;
        self.state.border_visible = false;
        Ok(())
    }

    /// Handle push-to-talk activation
    pub fn on_push_to_talk_start(&mut self) {
        self.state.is_listening = true;
        // Notify voice system to start recording
    }

    /// Handle push-to-talk release
    pub fn on_push_to_talk_end(&mut self) {
        self.state.is_listening = false;
        // Notify voice system to stop recording and process
    }

    /// Get current application state
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Get mutable application state
    pub fn state_mut(&mut self) -> &mut AppState {
        &mut self.state
    }
}
