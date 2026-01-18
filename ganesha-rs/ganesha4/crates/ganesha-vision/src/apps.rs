//! Application control for the Vision/VLA system.
//!
//! This module provides:
//! - AppController for managing target applications
//! - Window focus and management
//! - Application whitelist/blacklist enforcement
//! - App-specific action patterns
//! - Support for: Blender, Bambu Studio, OBS, CapCut

use crate::capture::{ScreenCapture, WindowInfo};
use crate::config::{AppListConfig, KnownApp};
use crate::input::{InputSimulator, KeyboardShortcut};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;

/// Errors that can occur during application control.
#[derive(Error, Debug)]
pub enum AppError {
    #[error("Application not found: {0}")]
    NotFound(String),

    #[error("Application not whitelisted: {0}")]
    NotWhitelisted(String),

    #[error("Application is blacklisted: {0}")]
    Blacklisted(String),

    #[error("Failed to focus window: {0}")]
    FocusFailed(String),

    #[error("Failed to launch application: {0}")]
    LaunchFailed(String),

    #[error("Application operation failed: {0}")]
    OperationFailed(String),

    #[error("Window not found: {0}")]
    WindowNotFound(String),

    #[error("Timeout waiting for application")]
    Timeout,
}

/// Result type for app operations.
pub type AppResult<T> = Result<T, AppError>;

/// Application state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppState {
    /// Application is not running
    NotRunning,
    /// Application is running but minimized
    Minimized,
    /// Application is running and visible
    Visible,
    /// Application is running and has focus
    Focused,
    /// Application is in an unknown state
    Unknown,
}

/// Information about a running application.
#[derive(Debug, Clone)]
pub struct AppInfo {
    /// Application name
    pub name: String,
    /// Process name
    pub process_name: String,
    /// Process ID
    pub pid: u32,
    /// Window information (if visible)
    pub window: Option<WindowInfo>,
    /// Current state
    pub state: AppState,
    /// Known app configuration (if available)
    pub known_config: Option<KnownApp>,
}

impl AppInfo {
    /// Check if this app is whitelisted.
    pub fn is_allowed(&self, config: &AppListConfig) -> bool {
        config.is_allowed(&self.process_name)
    }
}

/// Common UI action patterns for applications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionPattern {
    /// Name of the action
    pub name: String,
    /// Description of what the action does
    pub description: String,
    /// The action to perform
    pub action: AppAction,
    /// Verification step (optional)
    pub verify: Option<String>,
}

/// Types of actions that can be performed on applications.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppAction {
    /// Click a button by label/text
    ClickButton { label: String },
    /// Click at specific coordinates
    ClickAt { x: i32, y: i32 },
    /// Type text into the focused field
    TypeText { text: String },
    /// Execute a keyboard shortcut
    Shortcut { shortcut: String },
    /// Select a menu item
    SelectMenu { path: Vec<String> },
    /// Open a file dialog and select a file
    OpenFile { path: String },
    /// Save to a file path
    SaveFile { path: String },
    /// Wait for a condition
    Wait { milliseconds: u64 },
    /// Sequence of actions
    Sequence { actions: Vec<AppAction> },
}

/// App-specific action library.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppActionLibrary {
    /// Actions organized by application name
    pub apps: HashMap<String, Vec<ActionPattern>>,
}

impl AppActionLibrary {
    /// Create a new action library with default patterns.
    pub fn with_defaults() -> Self {
        let mut apps = HashMap::new();

        // Blender actions
        apps.insert(
            "Blender".to_string(),
            vec![
                ActionPattern {
                    name: "new_file".to_string(),
                    description: "Create a new Blender file".to_string(),
                    action: AppAction::Shortcut {
                        shortcut: "Ctrl+N".to_string(),
                    },
                    verify: Some("Check for 'New File' dialog".to_string()),
                },
                ActionPattern {
                    name: "save".to_string(),
                    description: "Save the current file".to_string(),
                    action: AppAction::Shortcut {
                        shortcut: "Ctrl+S".to_string(),
                    },
                    verify: None,
                },
                ActionPattern {
                    name: "render".to_string(),
                    description: "Render the current frame".to_string(),
                    action: AppAction::Shortcut {
                        shortcut: "F12".to_string(),
                    },
                    verify: Some("Check for render window".to_string()),
                },
                ActionPattern {
                    name: "toggle_edit_mode".to_string(),
                    description: "Toggle between Object and Edit mode".to_string(),
                    action: AppAction::Shortcut {
                        shortcut: "Tab".to_string(),
                    },
                    verify: None,
                },
                ActionPattern {
                    name: "add_mesh".to_string(),
                    description: "Open Add Mesh menu".to_string(),
                    action: AppAction::Shortcut {
                        shortcut: "Shift+A".to_string(),
                    },
                    verify: Some("Check for Add menu".to_string()),
                },
            ],
        );

        // Bambu Studio actions
        apps.insert(
            "Bambu Studio".to_string(),
            vec![
                ActionPattern {
                    name: "import_model".to_string(),
                    description: "Import a 3D model file".to_string(),
                    action: AppAction::Shortcut {
                        shortcut: "Ctrl+I".to_string(),
                    },
                    verify: Some("Check for file dialog".to_string()),
                },
                ActionPattern {
                    name: "slice".to_string(),
                    description: "Slice the model for printing".to_string(),
                    action: AppAction::ClickButton {
                        label: "Slice".to_string(),
                    },
                    verify: Some("Check for slicing progress".to_string()),
                },
                ActionPattern {
                    name: "export_gcode".to_string(),
                    description: "Export G-code file".to_string(),
                    action: AppAction::Shortcut {
                        shortcut: "Ctrl+G".to_string(),
                    },
                    verify: Some("Check for export dialog".to_string()),
                },
            ],
        );

        // OBS Studio actions
        apps.insert(
            "OBS Studio".to_string(),
            vec![
                ActionPattern {
                    name: "start_recording".to_string(),
                    description: "Start recording".to_string(),
                    action: AppAction::ClickButton {
                        label: "Start Recording".to_string(),
                    },
                    verify: Some("Check recording indicator".to_string()),
                },
                ActionPattern {
                    name: "stop_recording".to_string(),
                    description: "Stop recording".to_string(),
                    action: AppAction::ClickButton {
                        label: "Stop Recording".to_string(),
                    },
                    verify: Some("Check recording stopped".to_string()),
                },
                ActionPattern {
                    name: "start_streaming".to_string(),
                    description: "Start streaming".to_string(),
                    action: AppAction::ClickButton {
                        label: "Start Streaming".to_string(),
                    },
                    verify: Some("Check streaming indicator".to_string()),
                },
                ActionPattern {
                    name: "stop_streaming".to_string(),
                    description: "Stop streaming".to_string(),
                    action: AppAction::ClickButton {
                        label: "Stop Streaming".to_string(),
                    },
                    verify: Some("Check streaming stopped".to_string()),
                },
            ],
        );

        // CapCut actions
        apps.insert(
            "CapCut".to_string(),
            vec![
                ActionPattern {
                    name: "new_project".to_string(),
                    description: "Create a new project".to_string(),
                    action: AppAction::Shortcut {
                        shortcut: "Ctrl+N".to_string(),
                    },
                    verify: Some("Check for new project dialog".to_string()),
                },
                ActionPattern {
                    name: "import_media".to_string(),
                    description: "Import media files".to_string(),
                    action: AppAction::Shortcut {
                        shortcut: "Ctrl+I".to_string(),
                    },
                    verify: Some("Check for import dialog".to_string()),
                },
                ActionPattern {
                    name: "export".to_string(),
                    description: "Export the project".to_string(),
                    action: AppAction::Shortcut {
                        shortcut: "Ctrl+E".to_string(),
                    },
                    verify: Some("Check for export dialog".to_string()),
                },
                ActionPattern {
                    name: "play_pause".to_string(),
                    description: "Play or pause playback".to_string(),
                    action: AppAction::Shortcut {
                        shortcut: "Space".to_string(),
                    },
                    verify: None,
                },
            ],
        );

        Self { apps }
    }

    /// Get action patterns for an application.
    pub fn get_patterns(&self, app_name: &str) -> Option<&Vec<ActionPattern>> {
        self.apps.get(app_name)
    }

    /// Find a specific action pattern.
    pub fn find_action(&self, app_name: &str, action_name: &str) -> Option<&ActionPattern> {
        self.apps
            .get(app_name)?
            .iter()
            .find(|p| p.name == action_name)
    }
}

/// Trait for application control operations.
#[async_trait]
pub trait AppController: Send + Sync {
    /// List all running applications.
    async fn list_running(&self) -> AppResult<Vec<AppInfo>>;

    /// Find an application by name.
    async fn find_app(&self, name: &str) -> AppResult<Option<AppInfo>>;

    /// Find an application by process name.
    async fn find_by_process(&self, process_name: &str) -> AppResult<Option<AppInfo>>;

    /// Focus an application window.
    async fn focus(&self, app: &AppInfo) -> AppResult<()>;

    /// Minimize an application window.
    async fn minimize(&self, app: &AppInfo) -> AppResult<()>;

    /// Maximize an application window.
    async fn maximize(&self, app: &AppInfo) -> AppResult<()>;

    /// Restore a minimized window.
    async fn restore(&self, app: &AppInfo) -> AppResult<()>;

    /// Move a window to specific coordinates.
    async fn move_window(&self, app: &AppInfo, x: i32, y: i32) -> AppResult<()>;

    /// Resize a window.
    async fn resize_window(&self, app: &AppInfo, width: u32, height: u32) -> AppResult<()>;

    /// Wait for an application to be ready.
    async fn wait_for_ready(&self, app: &AppInfo, timeout: Duration) -> AppResult<()>;

    /// Execute an action pattern on an application.
    async fn execute_action(&self, app: &AppInfo, action: &AppAction) -> AppResult<()>;

    /// Get the currently focused application.
    async fn get_focused(&self) -> AppResult<Option<AppInfo>>;
}

/// Default application controller implementation.
pub struct DefaultAppController<C, I>
where
    C: ScreenCapture,
    I: InputSimulator,
{
    capture: C,
    input: I,
    config: AppListConfig,
    action_library: AppActionLibrary,
}

impl<C, I> DefaultAppController<C, I>
where
    C: ScreenCapture,
    I: InputSimulator,
{
    /// Create a new app controller.
    pub fn new(capture: C, input: I, config: AppListConfig) -> Self {
        Self {
            capture,
            input,
            config,
            action_library: AppActionLibrary::with_defaults(),
        }
    }

    /// Set a custom action library.
    pub fn with_action_library(mut self, library: AppActionLibrary) -> Self {
        self.action_library = library;
        self
    }

    /// Check if an app is allowed to be controlled.
    fn check_allowed(&self, process_name: &str) -> AppResult<()> {
        if self.config.blacklist.contains(process_name) {
            return Err(AppError::Blacklisted(process_name.to_string()));
        }

        if !self.config.is_allowed(process_name) {
            return Err(AppError::NotWhitelisted(process_name.to_string()));
        }

        Ok(())
    }

    /// Convert window info to app info.
    fn window_to_app(&self, window: WindowInfo) -> AppInfo {
        let known_config = self.config.get_known_app(&window.process_name).cloned();

        let state = if window.is_minimized {
            AppState::Minimized
        } else if window.is_visible {
            AppState::Visible
        } else {
            AppState::Unknown
        };

        AppInfo {
            name: known_config
                .as_ref()
                .map(|k| k.name.clone())
                .unwrap_or_else(|| window.title.clone()),
            process_name: window.process_name.clone(),
            pid: window.pid,
            window: Some(window),
            state,
            known_config,
        }
    }
}

#[async_trait]
impl<C, I> AppController for DefaultAppController<C, I>
where
    C: ScreenCapture + Send + Sync,
    I: InputSimulator + Send + Sync,
{
    async fn list_running(&self) -> AppResult<Vec<AppInfo>> {
        let windows = self
            .capture
            .get_windows()
            .await
            .map_err(|e| AppError::OperationFailed(e.to_string()))?;

        Ok(windows
            .into_iter()
            .map(|w| self.window_to_app(w))
            .collect())
    }

    async fn find_app(&self, name: &str) -> AppResult<Option<AppInfo>> {
        let name_lower = name.to_lowercase();
        let apps = self.list_running().await?;

        Ok(apps
            .into_iter()
            .find(|a| a.name.to_lowercase().contains(&name_lower)))
    }

    async fn find_by_process(&self, process_name: &str) -> AppResult<Option<AppInfo>> {
        let name_lower = process_name.to_lowercase();
        let apps = self.list_running().await?;

        Ok(apps
            .into_iter()
            .find(|a| a.process_name.to_lowercase().contains(&name_lower)))
    }

    async fn focus(&self, app: &AppInfo) -> AppResult<()> {
        self.check_allowed(&app.process_name)?;

        let window = app
            .window
            .as_ref()
            .ok_or_else(|| AppError::WindowNotFound(app.name.clone()))?;

        // Click on the window to focus it
        let (cx, cy) = window.region.center();
        self.input
            .click(cx, cy)
            .await
            .map_err(|e| AppError::FocusFailed(e.to_string()))?;

        // Small delay to allow focus to take effect
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(())
    }

    async fn minimize(&self, app: &AppInfo) -> AppResult<()> {
        self.check_allowed(&app.process_name)?;

        // Focus first, then minimize with keyboard shortcut
        self.focus(app).await?;

        // Platform-specific minimize (Super+D or Alt+F9 on Linux, Win+D on Windows)
        #[cfg(target_os = "linux")]
        {
            self.input
                .shortcut(&KeyboardShortcut::meta('d'.into()))
                .await
                .map_err(|e| AppError::OperationFailed(e.to_string()))?;
        }

        #[cfg(target_os = "windows")]
        {
            self.input
                .shortcut(&KeyboardShortcut::meta('d'.into()))
                .await
                .map_err(|e| AppError::OperationFailed(e.to_string()))?;
        }

        #[cfg(target_os = "macos")]
        {
            self.input
                .shortcut(&KeyboardShortcut::meta('m'.into()))
                .await
                .map_err(|e| AppError::OperationFailed(e.to_string()))?;
        }

        Ok(())
    }

    async fn maximize(&self, app: &AppInfo) -> AppResult<()> {
        self.check_allowed(&app.process_name)?;
        self.focus(app).await?;

        // Platform-specific maximize
        #[cfg(target_os = "linux")]
        {
            self.input
                .shortcut(&KeyboardShortcut::meta(crate::input::Key::Up.into()))
                .await
                .map_err(|e| AppError::OperationFailed(e.to_string()))?;
        }

        #[cfg(target_os = "windows")]
        {
            self.input
                .shortcut(&KeyboardShortcut::meta(crate::input::Key::Up.into()))
                .await
                .map_err(|e| AppError::OperationFailed(e.to_string()))?;
        }

        #[cfg(target_os = "macos")]
        {
            // macOS uses green button or Ctrl+Cmd+F
            self.input
                .shortcut(
                    &KeyboardShortcut::new('f'.into())
                        .with_modifier(crate::input::Modifier::Control)
                        .with_modifier(crate::input::Modifier::Meta),
                )
                .await
                .map_err(|e| AppError::OperationFailed(e.to_string()))?;
        }

        Ok(())
    }

    async fn restore(&self, app: &AppInfo) -> AppResult<()> {
        self.check_allowed(&app.process_name)?;

        // Find and click on the window (may need to click taskbar on some platforms)
        if let Some(window) = &app.window {
            let (cx, cy) = window.region.center();
            self.input
                .click(cx, cy)
                .await
                .map_err(|e| AppError::OperationFailed(e.to_string()))?;
        }

        Ok(())
    }

    async fn move_window(&self, app: &AppInfo, x: i32, y: i32) -> AppResult<()> {
        self.check_allowed(&app.process_name)?;
        self.focus(app).await?;

        // Alt+F7 on Linux to start move, then arrow keys or mouse
        // On Windows/macOS, would need to drag title bar
        #[cfg(target_os = "linux")]
        {
            self.input
                .shortcut(&KeyboardShortcut::alt(crate::input::Key::F7.into()))
                .await
                .map_err(|e| AppError::OperationFailed(e.to_string()))?;

            self.input
                .mouse_move(x, y)
                .await
                .map_err(|e| AppError::OperationFailed(e.to_string()))?;

            self.input
                .press_enter()
                .await
                .map_err(|e| AppError::OperationFailed(e.to_string()))?;
        }

        #[cfg(not(target_os = "linux"))]
        {
            // On other platforms, drag the title bar
            if let Some(window) = &app.window {
                let title_bar_y = window.region.y + 15; // Approximate title bar position
                let title_bar_x = window.region.x + (window.region.width / 2) as i32;

                let drag = crate::input::DragOperation::new(title_bar_x, title_bar_y, x, y);
                self.input
                    .mouse_drag(&drag)
                    .await
                    .map_err(|e| AppError::OperationFailed(e.to_string()))?;
            }
        }

        Ok(())
    }

    async fn resize_window(&self, app: &AppInfo, width: u32, height: u32) -> AppResult<()> {
        self.check_allowed(&app.process_name)?;
        self.focus(app).await?;

        // Alt+F8 on Linux to start resize
        #[cfg(target_os = "linux")]
        {
            self.input
                .shortcut(&KeyboardShortcut::alt(crate::input::Key::F8.into()))
                .await
                .map_err(|e| AppError::OperationFailed(e.to_string()))?;

            if let Some(window) = &app.window {
                let new_x = window.region.x + width as i32;
                let new_y = window.region.y + height as i32;
                self.input
                    .mouse_move(new_x, new_y)
                    .await
                    .map_err(|e| AppError::OperationFailed(e.to_string()))?;
            }

            self.input
                .press_enter()
                .await
                .map_err(|e| AppError::OperationFailed(e.to_string()))?;
        }

        #[cfg(not(target_os = "linux"))]
        {
            // On other platforms, drag the corner
            if let Some(window) = &app.window {
                let corner_x = window.region.x + window.region.width as i32;
                let corner_y = window.region.y + window.region.height as i32;
                let new_x = window.region.x + width as i32;
                let new_y = window.region.y + height as i32;

                let drag = crate::input::DragOperation::new(corner_x, corner_y, new_x, new_y);
                self.input
                    .mouse_drag(&drag)
                    .await
                    .map_err(|e| AppError::OperationFailed(e.to_string()))?;
            }
        }

        Ok(())
    }

    async fn wait_for_ready(&self, app: &AppInfo, timeout: Duration) -> AppResult<()> {
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            // Check if the window is visible and responsive
            if let Ok(Some(current)) = self.find_by_process(&app.process_name).await {
                if current.state == AppState::Visible || current.state == AppState::Focused {
                    return Ok(());
                }
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Err(AppError::Timeout)
    }

    async fn execute_action(&self, app: &AppInfo, action: &AppAction) -> AppResult<()> {
        self.check_allowed(&app.process_name)?;

        match action {
            AppAction::ClickButton { label } => {
                // Would need vision analysis to find the button
                // For now, log the intent
                tracing::info!("Would click button with label: {}", label);
                Ok(())
            }

            AppAction::ClickAt { x, y } => {
                self.input
                    .click(*x, *y)
                    .await
                    .map_err(|e| AppError::OperationFailed(e.to_string()))
            }

            AppAction::TypeText { text } => {
                self.input
                    .type_text(text)
                    .await
                    .map_err(|e| AppError::OperationFailed(e.to_string()))
            }

            AppAction::Shortcut { shortcut } => {
                let parsed = KeyboardShortcut::parse(shortcut)
                    .map_err(|e| AppError::OperationFailed(e.to_string()))?;
                self.input
                    .shortcut(&parsed)
                    .await
                    .map_err(|e| AppError::OperationFailed(e.to_string()))
            }

            AppAction::SelectMenu { path } => {
                // Would need to navigate menu hierarchy
                tracing::info!("Would select menu path: {:?}", path);
                Ok(())
            }

            AppAction::OpenFile { path } => {
                // Open file dialog shortcut, then type path
                self.input
                    .shortcut(&KeyboardShortcut::ctrl('o'.into()))
                    .await
                    .map_err(|e| AppError::OperationFailed(e.to_string()))?;

                tokio::time::sleep(Duration::from_millis(500)).await;

                self.input
                    .type_text(path)
                    .await
                    .map_err(|e| AppError::OperationFailed(e.to_string()))?;

                self.input
                    .press_enter()
                    .await
                    .map_err(|e| AppError::OperationFailed(e.to_string()))
            }

            AppAction::SaveFile { path } => {
                self.input
                    .shortcut(&KeyboardShortcut::ctrl_shift('s'.into()))
                    .await
                    .map_err(|e| AppError::OperationFailed(e.to_string()))?;

                tokio::time::sleep(Duration::from_millis(500)).await;

                self.input
                    .type_text(path)
                    .await
                    .map_err(|e| AppError::OperationFailed(e.to_string()))?;

                self.input
                    .press_enter()
                    .await
                    .map_err(|e| AppError::OperationFailed(e.to_string()))
            }

            AppAction::Wait { milliseconds } => {
                tokio::time::sleep(Duration::from_millis(*milliseconds)).await;
                Ok(())
            }

            AppAction::Sequence { actions } => {
                for action in actions {
                    self.execute_action(app, action).await?;
                }
                Ok(())
            }
        }
    }

    async fn get_focused(&self) -> AppResult<Option<AppInfo>> {
        // Get all windows and find the one that reports as focused
        // This is platform-specific and may not be fully reliable
        let apps = self.list_running().await?;

        // For now, return the first visible app (would need platform-specific API for true focus)
        Ok(apps.into_iter().find(|a| a.state == AppState::Visible))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_library_defaults() {
        let library = AppActionLibrary::with_defaults();

        assert!(library.get_patterns("Blender").is_some());
        assert!(library.get_patterns("OBS Studio").is_some());
        assert!(library.get_patterns("Unknown App").is_none());
    }

    #[test]
    fn test_find_action() {
        let library = AppActionLibrary::with_defaults();

        let action = library.find_action("Blender", "render");
        assert!(action.is_some());
        assert_eq!(action.unwrap().name, "render");
    }

    #[test]
    fn test_app_state() {
        assert_ne!(AppState::Focused, AppState::Minimized);
        assert_ne!(AppState::Visible, AppState::NotRunning);
    }
}
