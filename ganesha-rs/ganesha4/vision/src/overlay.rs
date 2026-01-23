//! Desktop overlay module for Ganesha Vision system.
//!
//! This module provides visual indicators when Ganesha is controlling the desktop:
//! - Red border around the screen to indicate active control
//! - Status window showing current action, stop/pause buttons, and prompt input
//! - Cross-platform support (Linux X11/Wayland, Windows, macOS)
//!
//! # Important: Self-Identification
//!
//! The overlay windows are specially identified so that Ganesha's vision system
//! can recognize and ignore them (avoid interacting with its own UI).
//!
//! # Example
//!
//! ```rust,no_run
//! use ganesha_vision::overlay::{ControlOverlay, OverlayConfig, ScreenBorder};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create the overlay system
//!     let config = OverlayConfig::default();
//!     let overlay = ControlOverlay::new(config).await?;
//!
//!     // Show the overlay when Ganesha takes control
//!     overlay.show().await?;
//!     overlay.set_status("Initializing...").await;
//!     overlay.set_action("Opening browser").await;
//!
//!     // Check for user input
//!     if overlay.is_stop_requested().await {
//!         overlay.hide().await?;
//!     }
//!
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, info, warn};

// ============================================================================
// Constants for Self-Identification
// ============================================================================

/// Window title prefix for all Ganesha overlay windows.
/// Vision system should ignore windows with titles starting with this prefix.
pub const GANESHA_WINDOW_PREFIX: &str = "[GANESHA-OVERLAY]";

/// Window title for the control panel overlay.
pub const CONTROL_PANEL_TITLE: &str = "[GANESHA-OVERLAY] Control Panel";

/// Window title for the screen border overlay.
pub const SCREEN_BORDER_TITLE: &str = "[GANESHA-OVERLAY] Screen Border";

/// Window class name for Ganesha overlays (X11/Windows).
pub const GANESHA_WINDOW_CLASS: &str = "GaneshaOverlay";

/// App identifier for macOS.
pub const GANESHA_APP_IDENTIFIER: &str = "com.ganesha.overlay";

/// List of all Ganesha window titles that should be ignored by vision.
pub const GANESHA_WINDOW_TITLES: &[&str] = &[
    CONTROL_PANEL_TITLE,
    SCREEN_BORDER_TITLE,
];

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during overlay operations.
#[derive(Error, Debug)]
pub enum OverlayError {
    /// Overlay system is not available on this platform.
    #[error("Overlay not available: {0}")]
    NotAvailable(String),

    /// Failed to create overlay window.
    #[error("Failed to create overlay: {0}")]
    CreationFailed(String),

    /// Failed to show/hide overlay.
    #[error("Visibility operation failed: {0}")]
    VisibilityFailed(String),

    /// Failed to update overlay content.
    #[error("Update failed: {0}")]
    UpdateFailed(String),

    /// Platform-specific error.
    #[error("Platform error: {0}")]
    PlatformError(String),

    /// Channel communication error.
    #[error("Communication error: {0}")]
    ChannelError(String),

    /// Overlay already running.
    #[error("Overlay already running")]
    AlreadyRunning,

    /// Overlay not running.
    #[error("Overlay not running")]
    NotRunning,
}

/// Result type for overlay operations.
pub type OverlayResult<T> = Result<T, OverlayError>;

// ============================================================================
// Configuration Types
// ============================================================================

/// RGBA color representation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Create a new color with full opacity.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Create a new color with specified alpha.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Default red color for Ganesha control indicator.
    pub const fn ganesha_red() -> Self {
        Self::rgba(220, 38, 38, 180) // Semi-transparent red
    }

    /// Convert to CSS rgba string.
    pub fn to_css(&self) -> String {
        format!("rgba({}, {}, {}, {:.2})", self.r, self.g, self.b, self.a as f32 / 255.0)
    }

    /// Convert to hex string (without alpha).
    pub fn to_hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Convert to u32 ARGB format.
    pub fn to_argb(&self) -> u32 {
        ((self.a as u32) << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::ganesha_red()
    }
}

/// Position for the control window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowPosition {
    /// Top-left corner of the screen.
    TopLeft,
    /// Top-right corner of the screen.
    TopRight,
    /// Bottom-left corner of the screen.
    BottomLeft,
    /// Bottom-right corner of the screen.
    BottomRight,
    /// Center of the screen.
    Center,
    /// Custom position (x, y from top-left).
    Custom(i32, i32),
}

impl Default for WindowPosition {
    fn default() -> Self {
        Self::TopRight
    }
}

impl WindowPosition {
    /// Calculate actual screen coordinates given screen dimensions and window size.
    pub fn to_coordinates(&self, screen_width: u32, screen_height: u32, window_width: u32, window_height: u32, margin: u32) -> (i32, i32) {
        match self {
            Self::TopLeft => (margin as i32, margin as i32),
            Self::TopRight => ((screen_width - window_width - margin) as i32, margin as i32),
            Self::BottomLeft => (margin as i32, (screen_height - window_height - margin) as i32),
            Self::BottomRight => (
                (screen_width - window_width - margin) as i32,
                (screen_height - window_height - margin) as i32,
            ),
            Self::Center => (
                ((screen_width - window_width) / 2) as i32,
                ((screen_height - window_height) / 2) as i32,
            ),
            Self::Custom(x, y) => (*x, *y),
        }
    }
}

/// Configuration for the overlay system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayConfig {
    /// Color of the screen border.
    pub border_color: Color,
    /// Width of the screen border in pixels.
    pub border_width: u32,
    /// Position of the control window.
    pub window_position: WindowPosition,
    /// Overall opacity of overlays (0.0 - 1.0).
    pub opacity: f32,
    /// Whether to show the screen border.
    pub show_border: bool,
    /// Whether to show the control panel.
    pub show_control_panel: bool,
    /// Width of the control panel in pixels.
    pub control_panel_width: u32,
    /// Height of the control panel in pixels.
    pub control_panel_height: u32,
    /// Margin from screen edges in pixels.
    pub margin: u32,
    /// Whether the border should be click-through (pass mouse events).
    pub border_click_through: bool,
    /// Monitor index to display overlay on (None = primary).
    pub monitor_index: Option<u32>,
    /// Font size for status text.
    pub font_size: u32,
    /// Background color for control panel.
    pub panel_background: Color,
    /// Text color for control panel.
    pub text_color: Color,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            border_color: Color::ganesha_red(),
            border_width: 3,
            window_position: WindowPosition::TopRight,
            opacity: 0.9,
            show_border: true,
            show_control_panel: true,
            control_panel_width: 320,
            control_panel_height: 180,
            margin: 20,
            border_click_through: true,
            monitor_index: None,
            font_size: 12,
            panel_background: Color::rgba(30, 30, 30, 230),
            text_color: Color::rgb(255, 255, 255),
        }
    }
}

impl OverlayConfig {
    /// Create a minimal configuration (border only, no control panel).
    pub fn minimal() -> Self {
        Self {
            show_control_panel: false,
            border_width: 2,
            ..Default::default()
        }
    }

    /// Create a configuration for debugging (more visible).
    pub fn debug() -> Self {
        Self {
            border_width: 5,
            border_color: Color::rgba(255, 0, 0, 255),
            opacity: 1.0,
            ..Default::default()
        }
    }

    /// Builder: set border color.
    pub fn with_border_color(mut self, color: Color) -> Self {
        self.border_color = color;
        self
    }

    /// Builder: set border width.
    pub fn with_border_width(mut self, width: u32) -> Self {
        self.border_width = width.max(1).min(20);
        self
    }

    /// Builder: set window position.
    pub fn with_position(mut self, position: WindowPosition) -> Self {
        self.window_position = position;
        self
    }

    /// Builder: set opacity.
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.max(0.0).min(1.0);
        self
    }
}

// ============================================================================
// State Types
// ============================================================================

/// Current state of the overlay.
#[derive(Debug, Clone, Default)]
pub struct OverlayState {
    /// Whether the overlay is currently visible.
    pub visible: bool,
    /// Current status message.
    pub status: String,
    /// Current action being performed.
    pub action: String,
    /// Whether stop has been requested.
    pub stop_requested: bool,
    /// Whether pause has been requested.
    pub pause_requested: bool,
    /// User prompt input (if any).
    pub prompt_input: Option<String>,
    /// Whether Ganesha is currently paused.
    pub is_paused: bool,
}

/// Commands that can be sent to the overlay.
#[derive(Debug, Clone)]
pub enum OverlayCommand {
    /// Show the overlay.
    Show,
    /// Hide the overlay.
    Hide,
    /// Update status message.
    SetStatus(String),
    /// Update current action.
    SetAction(String),
    /// Clear prompt input.
    ClearPrompt,
    /// Set pause state.
    SetPaused(bool),
    /// Shutdown the overlay.
    Shutdown,
}

/// Events from the overlay (user interactions).
#[derive(Debug, Clone)]
pub enum OverlayEvent {
    /// Stop button was clicked.
    StopRequested,
    /// Pause button was clicked.
    PauseRequested,
    /// Resume button was clicked.
    ResumeRequested,
    /// User submitted a prompt.
    PromptSubmitted(String),
    /// Overlay was closed.
    Closed,
}

// ============================================================================
// Overlay Trait
// ============================================================================

/// Trait for platform-specific overlay implementations.
#[async_trait]
pub trait OverlayBackend: Send + Sync {
    /// Check if this backend is available on the current platform.
    fn is_available(&self) -> bool;

    /// Get the backend name.
    fn name(&self) -> &'static str;

    /// Initialize the overlay backend.
    async fn initialize(&mut self, config: &OverlayConfig) -> OverlayResult<()>;

    /// Show the overlay.
    async fn show(&self) -> OverlayResult<()>;

    /// Hide the overlay.
    async fn hide(&self) -> OverlayResult<()>;

    /// Update the status message.
    async fn set_status(&self, message: &str) -> OverlayResult<()>;

    /// Update the current action.
    async fn set_action(&self, action: &str) -> OverlayResult<()>;

    /// Get user prompt input (non-blocking).
    async fn get_prompt_input(&self) -> Option<String>;

    /// Clear the prompt input field.
    async fn clear_prompt(&self) -> OverlayResult<()>;

    /// Check if stop was requested.
    fn is_stop_requested(&self) -> bool;

    /// Check if pause was requested.
    fn is_pause_requested(&self) -> bool;

    /// Reset stop/pause flags.
    fn reset_requests(&self);

    /// Set the paused state (changes button display).
    async fn set_paused(&self, paused: bool) -> OverlayResult<()>;

    /// Shutdown the overlay.
    async fn shutdown(&self) -> OverlayResult<()>;

    /// Poll for events (non-blocking).
    async fn poll_events(&self) -> Vec<OverlayEvent>;
}

// ============================================================================
// Screen Border
// ============================================================================

/// A screen border overlay that indicates Ganesha is in control.
///
/// The border is drawn around the entire screen and is designed to be:
/// - Visible but not intrusive (thin, semi-transparent)
/// - Click-through (doesn't capture mouse events)
/// - Always on top of other windows
pub struct ScreenBorder {
    config: OverlayConfig,
    visible: AtomicBool,
    // Platform-specific handle would go here
}

impl ScreenBorder {
    /// Create a new screen border.
    pub fn new(config: OverlayConfig) -> Self {
        Self {
            config,
            visible: AtomicBool::new(false),
        }
    }

    /// Check if the border is currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible.load(Ordering::Relaxed)
    }

    /// Get the border configuration.
    pub fn config(&self) -> &OverlayConfig {
        &self.config
    }

    /// Show the border.
    pub async fn show(&self) -> OverlayResult<()> {
        self.visible.store(true, Ordering::Relaxed);
        info!("Screen border shown ({}px, {})",
              self.config.border_width,
              self.config.border_color.to_css());
        Ok(())
    }

    /// Hide the border.
    pub async fn hide(&self) -> OverlayResult<()> {
        self.visible.store(false, Ordering::Relaxed);
        info!("Screen border hidden");
        Ok(())
    }

    /// Update the border color.
    pub async fn set_color(&mut self, color: Color) -> OverlayResult<()> {
        self.config.border_color = color;
        if self.is_visible() {
            // Would trigger a redraw on actual implementation
            debug!("Border color updated to {}", color.to_css());
        }
        Ok(())
    }

    /// Update the border width.
    pub async fn set_width(&mut self, width: u32) -> OverlayResult<()> {
        self.config.border_width = width.max(1).min(20);
        if self.is_visible() {
            // Would trigger a redraw on actual implementation
            debug!("Border width updated to {}px", self.config.border_width);
        }
        Ok(())
    }
}

// ============================================================================
// Control Overlay (Main Interface)
// ============================================================================

/// The main control overlay that manages the visual indicators for Ganesha control.
///
/// This provides:
/// - A red border around the screen
/// - A status window with current action and control buttons
/// - User prompt input capability
///
/// The overlay is thread-safe and async-compatible.
pub struct ControlOverlay {
    config: OverlayConfig,
    state: Arc<RwLock<OverlayState>>,
    border: Arc<RwLock<ScreenBorder>>,
    command_tx: mpsc::Sender<OverlayCommand>,
    event_rx: Arc<Mutex<mpsc::Receiver<OverlayEvent>>>,
    running: Arc<AtomicBool>,
    // Platform-specific backend
    backend: Arc<RwLock<Option<Box<dyn OverlayBackend>>>>,
}

impl ControlOverlay {
    /// Create a new control overlay with the given configuration.
    pub async fn new(config: OverlayConfig) -> OverlayResult<Self> {
        let (command_tx, mut command_rx) = mpsc::channel::<OverlayCommand>(100);
        let (event_tx, event_rx) = mpsc::channel::<OverlayEvent>(100);

        let state = Arc::new(RwLock::new(OverlayState::default()));
        let border = Arc::new(RwLock::new(ScreenBorder::new(config.clone())));
        let running = Arc::new(AtomicBool::new(false));

        // Try to initialize platform-specific backend
        let backend: Option<Box<dyn OverlayBackend>> = {
            #[cfg(target_os = "linux")]
            {
                let mut linux_backend = LinuxOverlayBackend::new();
                if linux_backend.is_available() {
                    if let Err(e) = linux_backend.initialize(&config).await {
                        warn!("Failed to initialize Linux backend: {}", e);
                        None
                    } else {
                        Some(Box::new(linux_backend))
                    }
                } else {
                    None
                }
            }
            #[cfg(target_os = "windows")]
            {
                let mut win_backend = WindowsOverlayBackend::new();
                if win_backend.is_available() {
                    if let Err(e) = win_backend.initialize(&config).await {
                        warn!("Failed to initialize Windows backend: {}", e);
                        None
                    } else {
                        Some(Box::new(win_backend))
                    }
                } else {
                    None
                }
            }
            #[cfg(target_os = "macos")]
            {
                let mut mac_backend = MacOSOverlayBackend::new();
                if mac_backend.is_available() {
                    if let Err(e) = mac_backend.initialize(&config).await {
                        warn!("Failed to initialize macOS backend: {}", e);
                        None
                    } else {
                        Some(Box::new(mac_backend))
                    }
                } else {
                    None
                }
            }
            #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
            {
                None
            }
        };

        if backend.is_none() {
            warn!("No native overlay backend available, using stub implementation");
        }

        let overlay = Self {
            config,
            state,
            border,
            command_tx,
            event_rx: Arc::new(Mutex::new(event_rx)),
            running,
            backend: Arc::new(RwLock::new(backend)),
        };

        // Start command processing task
        let state_clone = overlay.state.clone();
        let border_clone = overlay.border.clone();
        let backend_clone = overlay.backend.clone();
        let running_clone = overlay.running.clone();
        let event_tx_clone = event_tx.clone();

        tokio::spawn(async move {
            while let Some(cmd) = command_rx.recv().await {
                let backend_guard = backend_clone.read().await;
                match cmd {
                    OverlayCommand::Show => {
                        let mut state = state_clone.write().await;
                        state.visible = true;
                        if let Some(ref backend) = *backend_guard {
                            let _ = backend.show().await;
                        }
                        let _ = border_clone.read().await.show().await;
                    }
                    OverlayCommand::Hide => {
                        let mut state = state_clone.write().await;
                        state.visible = false;
                        if let Some(ref backend) = *backend_guard {
                            let _ = backend.hide().await;
                        }
                        let _ = border_clone.read().await.hide().await;
                    }
                    OverlayCommand::SetStatus(msg) => {
                        let mut state = state_clone.write().await;
                        state.status = msg.clone();
                        if let Some(ref backend) = *backend_guard {
                            let _ = backend.set_status(&msg).await;
                        }
                    }
                    OverlayCommand::SetAction(action) => {
                        let mut state = state_clone.write().await;
                        state.action = action.clone();
                        if let Some(ref backend) = *backend_guard {
                            let _ = backend.set_action(&action).await;
                        }
                    }
                    OverlayCommand::ClearPrompt => {
                        let mut state = state_clone.write().await;
                        state.prompt_input = None;
                        if let Some(ref backend) = *backend_guard {
                            let _ = backend.clear_prompt().await;
                        }
                    }
                    OverlayCommand::SetPaused(paused) => {
                        let mut state = state_clone.write().await;
                        state.is_paused = paused;
                        if let Some(ref backend) = *backend_guard {
                            let _ = backend.set_paused(paused).await;
                        }
                    }
                    OverlayCommand::Shutdown => {
                        running_clone.store(false, Ordering::Relaxed);
                        if let Some(ref backend) = *backend_guard {
                            let _ = backend.shutdown().await;
                        }
                        let _ = event_tx_clone.send(OverlayEvent::Closed).await;
                        break;
                    }
                }
            }
        });

        Ok(overlay)
    }

    /// Create with default configuration.
    pub async fn with_defaults() -> OverlayResult<Self> {
        Self::new(OverlayConfig::default()).await
    }

    /// Get the overlay configuration.
    pub fn config(&self) -> &OverlayConfig {
        &self.config
    }

    /// Get the current overlay state.
    pub async fn state(&self) -> OverlayState {
        self.state.read().await.clone()
    }

    /// Check if the overlay is currently visible.
    pub async fn is_visible(&self) -> bool {
        self.state.read().await.visible
    }

    /// Show the overlay (border and control panel).
    pub async fn show(&self) -> OverlayResult<()> {
        self.command_tx
            .send(OverlayCommand::Show)
            .await
            .map_err(|e| OverlayError::ChannelError(e.to_string()))?;
        self.running.store(true, Ordering::Relaxed);
        info!("Overlay shown");
        Ok(())
    }

    /// Hide the overlay.
    pub async fn hide(&self) -> OverlayResult<()> {
        self.command_tx
            .send(OverlayCommand::Hide)
            .await
            .map_err(|e| OverlayError::ChannelError(e.to_string()))?;
        info!("Overlay hidden");
        Ok(())
    }

    /// Set the status message displayed in the control panel.
    pub async fn set_status(&self, message: &str) {
        let _ = self.command_tx.send(OverlayCommand::SetStatus(message.to_string())).await;
        debug!("Status updated: {}", message);
    }

    /// Set the current action displayed in the control panel.
    pub async fn set_action(&self, action: &str) {
        let _ = self.command_tx.send(OverlayCommand::SetAction(action.to_string())).await;
        debug!("Action updated: {}", action);
    }

    /// Get user prompt input if available.
    ///
    /// Returns `Some(prompt)` if the user has submitted a prompt, `None` otherwise.
    /// This is non-blocking.
    pub async fn get_prompt_input(&self) -> Option<String> {
        let state = self.state.read().await;
        state.prompt_input.clone()
    }

    /// Clear the prompt input.
    pub async fn clear_prompt(&self) {
        let _ = self.command_tx.send(OverlayCommand::ClearPrompt).await;
    }

    /// Check if the user has requested to stop Ganesha.
    pub async fn is_stop_requested(&self) -> bool {
        let state = self.state.read().await;
        state.stop_requested
    }

    /// Check if the user has requested to pause Ganesha.
    pub async fn is_pause_requested(&self) -> bool {
        let state = self.state.read().await;
        state.pause_requested
    }

    /// Reset stop and pause request flags.
    pub async fn reset_requests(&self) {
        let mut state = self.state.write().await;
        state.stop_requested = false;
        state.pause_requested = false;

        let backend = self.backend.read().await;
        if let Some(ref b) = *backend {
            b.reset_requests();
        }
    }

    /// Set whether Ganesha is currently paused.
    pub async fn set_paused(&self, paused: bool) {
        let _ = self.command_tx.send(OverlayCommand::SetPaused(paused)).await;
    }

    /// Poll for events from the overlay (user interactions).
    pub async fn poll_events(&self) -> Vec<OverlayEvent> {
        let mut events = Vec::new();

        // Try to receive events without blocking
        let mut rx = self.event_rx.lock().await;
        while let Ok(event) = rx.try_recv() {
            // Update state based on events
            match &event {
                OverlayEvent::StopRequested => {
                    let mut state = self.state.write().await;
                    state.stop_requested = true;
                }
                OverlayEvent::PauseRequested => {
                    let mut state = self.state.write().await;
                    state.pause_requested = true;
                }
                OverlayEvent::PromptSubmitted(prompt) => {
                    let mut state = self.state.write().await;
                    state.prompt_input = Some(prompt.clone());
                }
                _ => {}
            }
            events.push(event);
        }

        // Also poll backend
        let backend = self.backend.read().await;
        if let Some(ref b) = *backend {
            events.extend(b.poll_events().await);
        }

        events
    }

    /// Shutdown the overlay system.
    pub async fn shutdown(&self) -> OverlayResult<()> {
        self.command_tx
            .send(OverlayCommand::Shutdown)
            .await
            .map_err(|e| OverlayError::ChannelError(e.to_string()))?;
        info!("Overlay shutdown requested");
        Ok(())
    }

    /// Get access to the screen border.
    pub async fn border(&self) -> tokio::sync::RwLockReadGuard<'_, ScreenBorder> {
        self.border.read().await
    }

    /// Check if the overlay system is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

// ============================================================================
// Window Identification Utilities
// ============================================================================

/// Check if a window title indicates it's a Ganesha overlay window.
///
/// Use this in the vision system to avoid interacting with Ganesha's own UI.
pub fn is_ganesha_window(title: &str) -> bool {
    title.starts_with(GANESHA_WINDOW_PREFIX)
}

/// Check if a window class/name indicates it's a Ganesha overlay.
pub fn is_ganesha_window_class(class_name: &str) -> bool {
    class_name == GANESHA_WINDOW_CLASS
}

/// Check if an app identifier indicates it's a Ganesha overlay (macOS).
pub fn is_ganesha_app(app_id: &str) -> bool {
    app_id == GANESHA_APP_IDENTIFIER
}

/// Filter a list of window titles to exclude Ganesha overlays.
pub fn filter_ganesha_windows(titles: &[String]) -> Vec<String> {
    titles
        .iter()
        .filter(|t| !is_ganesha_window(t))
        .cloned()
        .collect()
}

// ============================================================================
// Linux Backend (X11/Wayland)
// ============================================================================

#[cfg(target_os = "linux")]
mod linux {
    use super::*;

    /// Linux overlay backend supporting X11 and Wayland.
    pub struct LinuxOverlayBackend {
        display_server: DisplayServer,
        stop_requested: AtomicBool,
        pause_requested: AtomicBool,
        status: Arc<RwLock<String>>,
        action: Arc<RwLock<String>>,
        prompt_input: Arc<RwLock<Option<String>>>,
        paused: AtomicBool,
        initialized: AtomicBool,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum DisplayServer {
        X11,
        Wayland,
        Unknown,
    }

    impl LinuxOverlayBackend {
        pub fn new() -> Self {
            let display_server = Self::detect_display_server();
            info!("Linux display server detected: {:?}", display_server);

            Self {
                display_server,
                stop_requested: AtomicBool::new(false),
                pause_requested: AtomicBool::new(false),
                status: Arc::new(RwLock::new(String::new())),
                action: Arc::new(RwLock::new(String::new())),
                prompt_input: Arc::new(RwLock::new(None)),
                paused: AtomicBool::new(false),
                initialized: AtomicBool::new(false),
            }
        }

        fn detect_display_server() -> DisplayServer {
            // Check XDG_SESSION_TYPE first
            if let Ok(session_type) = std::env::var("XDG_SESSION_TYPE") {
                match session_type.as_str() {
                    "wayland" => return DisplayServer::Wayland,
                    "x11" => return DisplayServer::X11,
                    _ => {}
                }
            }

            // Check WAYLAND_DISPLAY
            if std::env::var("WAYLAND_DISPLAY").is_ok() {
                return DisplayServer::Wayland;
            }

            // Check DISPLAY for X11
            if std::env::var("DISPLAY").is_ok() {
                return DisplayServer::X11;
            }

            DisplayServer::Unknown
        }
    }

    impl Default for LinuxOverlayBackend {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl OverlayBackend for LinuxOverlayBackend {
        fn is_available(&self) -> bool {
            self.display_server != DisplayServer::Unknown
        }

        fn name(&self) -> &'static str {
            match self.display_server {
                DisplayServer::X11 => "Linux/X11",
                DisplayServer::Wayland => "Linux/Wayland",
                DisplayServer::Unknown => "Linux/Unknown",
            }
        }

        async fn initialize(&mut self, config: &OverlayConfig) -> OverlayResult<()> {
            if !self.is_available() {
                return Err(OverlayError::NotAvailable(
                    "No display server detected".to_string(),
                ));
            }

            info!(
                "Initializing Linux overlay backend ({}): border={}px, panel={}x{}",
                self.name(),
                config.border_width,
                config.control_panel_width,
                config.control_panel_height
            );

            // Note: Actual GTK/X11/Wayland initialization would go here
            // For now, we mark as initialized and log the configuration

            match self.display_server {
                DisplayServer::X11 => {
                    // X11-specific initialization using xcb or xlib
                    // Would create:
                    // 1. Border window (override_redirect, always-on-top, input-passthrough)
                    // 2. Control panel window (normal window, always-on-top)
                    debug!("X11: Would create overlay windows with WM hints");
                }
                DisplayServer::Wayland => {
                    // Wayland-specific initialization
                    // Would use layer-shell protocol for overlay
                    debug!("Wayland: Would create layer-shell surfaces");
                }
                DisplayServer::Unknown => {
                    return Err(OverlayError::NotAvailable(
                        "Unknown display server".to_string(),
                    ));
                }
            }

            self.initialized.store(true, Ordering::Relaxed);
            Ok(())
        }

        async fn show(&self) -> OverlayResult<()> {
            if !self.initialized.load(Ordering::Relaxed) {
                return Err(OverlayError::NotRunning);
            }
            info!("Linux overlay: show");
            // Would map/show the windows
            Ok(())
        }

        async fn hide(&self) -> OverlayResult<()> {
            if !self.initialized.load(Ordering::Relaxed) {
                return Err(OverlayError::NotRunning);
            }
            info!("Linux overlay: hide");
            // Would unmap/hide the windows
            Ok(())
        }

        async fn set_status(&self, message: &str) -> OverlayResult<()> {
            let mut status = self.status.write().await;
            *status = message.to_string();
            // Would update the status label widget
            Ok(())
        }

        async fn set_action(&self, action: &str) -> OverlayResult<()> {
            let mut action_state = self.action.write().await;
            *action_state = action.to_string();
            // Would update the action label widget
            Ok(())
        }

        async fn get_prompt_input(&self) -> Option<String> {
            self.prompt_input.read().await.clone()
        }

        async fn clear_prompt(&self) -> OverlayResult<()> {
            let mut prompt = self.prompt_input.write().await;
            *prompt = None;
            // Would clear the text entry widget
            Ok(())
        }

        fn is_stop_requested(&self) -> bool {
            self.stop_requested.load(Ordering::Relaxed)
        }

        fn is_pause_requested(&self) -> bool {
            self.pause_requested.load(Ordering::Relaxed)
        }

        fn reset_requests(&self) {
            self.stop_requested.store(false, Ordering::Relaxed);
            self.pause_requested.store(false, Ordering::Relaxed);
        }

        async fn set_paused(&self, paused: bool) -> OverlayResult<()> {
            self.paused.store(paused, Ordering::Relaxed);
            // Would update the pause/resume button appearance
            Ok(())
        }

        async fn shutdown(&self) -> OverlayResult<()> {
            self.initialized.store(false, Ordering::Relaxed);
            info!("Linux overlay: shutdown");
            // Would destroy windows and cleanup
            Ok(())
        }

        async fn poll_events(&self) -> Vec<OverlayEvent> {
            // Would poll X11/Wayland events and return any button clicks, etc.
            Vec::new()
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux::LinuxOverlayBackend;

// ============================================================================
// Windows Backend
// ============================================================================

#[cfg(target_os = "windows")]
mod windows {
    use super::*;

    /// Windows overlay backend using Win32 API.
    pub struct WindowsOverlayBackend {
        stop_requested: AtomicBool,
        pause_requested: AtomicBool,
        status: Arc<RwLock<String>>,
        action: Arc<RwLock<String>>,
        prompt_input: Arc<RwLock<Option<String>>>,
        paused: AtomicBool,
        initialized: AtomicBool,
    }

    impl WindowsOverlayBackend {
        pub fn new() -> Self {
            Self {
                stop_requested: AtomicBool::new(false),
                pause_requested: AtomicBool::new(false),
                status: Arc::new(RwLock::new(String::new())),
                action: Arc::new(RwLock::new(String::new())),
                prompt_input: Arc::new(RwLock::new(None)),
                paused: AtomicBool::new(false),
                initialized: AtomicBool::new(false),
            }
        }
    }

    impl Default for WindowsOverlayBackend {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl OverlayBackend for WindowsOverlayBackend {
        fn is_available(&self) -> bool {
            true // Windows is always available on Windows
        }

        fn name(&self) -> &'static str {
            "Windows/Win32"
        }

        async fn initialize(&mut self, config: &OverlayConfig) -> OverlayResult<()> {
            info!(
                "Initializing Windows overlay backend: border={}px, panel={}x{}",
                config.border_width,
                config.control_panel_width,
                config.control_panel_height
            );

            // Note: Actual Win32 initialization would go here
            // Would create:
            // 1. Layered window for border (WS_EX_LAYERED, WS_EX_TRANSPARENT, WS_EX_TOPMOST)
            // 2. Tool window for control panel (WS_EX_TOOLWINDOW, WS_EX_TOPMOST)

            debug!("Windows: Would create overlay windows with extended styles");
            self.initialized.store(true, Ordering::Relaxed);
            Ok(())
        }

        async fn show(&self) -> OverlayResult<()> {
            if !self.initialized.load(Ordering::Relaxed) {
                return Err(OverlayError::NotRunning);
            }
            info!("Windows overlay: show");
            // Would call ShowWindow
            Ok(())
        }

        async fn hide(&self) -> OverlayResult<()> {
            if !self.initialized.load(Ordering::Relaxed) {
                return Err(OverlayError::NotRunning);
            }
            info!("Windows overlay: hide");
            // Would call ShowWindow(SW_HIDE)
            Ok(())
        }

        async fn set_status(&self, message: &str) -> OverlayResult<()> {
            let mut status = self.status.write().await;
            *status = message.to_string();
            Ok(())
        }

        async fn set_action(&self, action: &str) -> OverlayResult<()> {
            let mut action_state = self.action.write().await;
            *action_state = action.to_string();
            Ok(())
        }

        async fn get_prompt_input(&self) -> Option<String> {
            self.prompt_input.read().await.clone()
        }

        async fn clear_prompt(&self) -> OverlayResult<()> {
            let mut prompt = self.prompt_input.write().await;
            *prompt = None;
            Ok(())
        }

        fn is_stop_requested(&self) -> bool {
            self.stop_requested.load(Ordering::Relaxed)
        }

        fn is_pause_requested(&self) -> bool {
            self.pause_requested.load(Ordering::Relaxed)
        }

        fn reset_requests(&self) {
            self.stop_requested.store(false, Ordering::Relaxed);
            self.pause_requested.store(false, Ordering::Relaxed);
        }

        async fn set_paused(&self, paused: bool) -> OverlayResult<()> {
            self.paused.store(paused, Ordering::Relaxed);
            Ok(())
        }

        async fn shutdown(&self) -> OverlayResult<()> {
            self.initialized.store(false, Ordering::Relaxed);
            info!("Windows overlay: shutdown");
            // Would call DestroyWindow
            Ok(())
        }

        async fn poll_events(&self) -> Vec<OverlayEvent> {
            // Would poll Win32 message queue
            Vec::new()
        }
    }
}

#[cfg(target_os = "windows")]
pub use windows::WindowsOverlayBackend;

// ============================================================================
// macOS Backend
// ============================================================================

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    /// macOS overlay backend using Cocoa/AppKit.
    pub struct MacOSOverlayBackend {
        stop_requested: AtomicBool,
        pause_requested: AtomicBool,
        status: Arc<RwLock<String>>,
        action: Arc<RwLock<String>>,
        prompt_input: Arc<RwLock<Option<String>>>,
        paused: AtomicBool,
        initialized: AtomicBool,
    }

    impl MacOSOverlayBackend {
        pub fn new() -> Self {
            Self {
                stop_requested: AtomicBool::new(false),
                pause_requested: AtomicBool::new(false),
                status: Arc::new(RwLock::new(String::new())),
                action: Arc::new(RwLock::new(String::new())),
                prompt_input: Arc::new(RwLock::new(None)),
                paused: AtomicBool::new(false),
                initialized: AtomicBool::new(false),
            }
        }
    }

    impl Default for MacOSOverlayBackend {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl OverlayBackend for MacOSOverlayBackend {
        fn is_available(&self) -> bool {
            true // macOS is always available on macOS
        }

        fn name(&self) -> &'static str {
            "macOS/AppKit"
        }

        async fn initialize(&mut self, config: &OverlayConfig) -> OverlayResult<()> {
            info!(
                "Initializing macOS overlay backend: border={}px, panel={}x{}",
                config.border_width,
                config.control_panel_width,
                config.control_panel_height
            );

            // Note: Actual Cocoa/AppKit initialization would go here
            // Would create:
            // 1. NSWindow with NSWindowLevelScreenSaver for border
            // 2. NSPanel for control panel (floating, always-on-top)
            // Window properties: ignoresMouseEvents, hasShadow=NO, etc.

            debug!("macOS: Would create NSWindow/NSPanel overlays");
            self.initialized.store(true, Ordering::Relaxed);
            Ok(())
        }

        async fn show(&self) -> OverlayResult<()> {
            if !self.initialized.load(Ordering::Relaxed) {
                return Err(OverlayError::NotRunning);
            }
            info!("macOS overlay: show");
            // Would call orderFront/makeKeyAndOrderFront
            Ok(())
        }

        async fn hide(&self) -> OverlayResult<()> {
            if !self.initialized.load(Ordering::Relaxed) {
                return Err(OverlayError::NotRunning);
            }
            info!("macOS overlay: hide");
            // Would call orderOut
            Ok(())
        }

        async fn set_status(&self, message: &str) -> OverlayResult<()> {
            let mut status = self.status.write().await;
            *status = message.to_string();
            Ok(())
        }

        async fn set_action(&self, action: &str) -> OverlayResult<()> {
            let mut action_state = self.action.write().await;
            *action_state = action.to_string();
            Ok(())
        }

        async fn get_prompt_input(&self) -> Option<String> {
            self.prompt_input.read().await.clone()
        }

        async fn clear_prompt(&self) -> OverlayResult<()> {
            let mut prompt = self.prompt_input.write().await;
            *prompt = None;
            Ok(())
        }

        fn is_stop_requested(&self) -> bool {
            self.stop_requested.load(Ordering::Relaxed)
        }

        fn is_pause_requested(&self) -> bool {
            self.pause_requested.load(Ordering::Relaxed)
        }

        fn reset_requests(&self) {
            self.stop_requested.store(false, Ordering::Relaxed);
            self.pause_requested.store(false, Ordering::Relaxed);
        }

        async fn set_paused(&self, paused: bool) -> OverlayResult<()> {
            self.paused.store(paused, Ordering::Relaxed);
            Ok(())
        }

        async fn shutdown(&self) -> OverlayResult<()> {
            self.initialized.store(false, Ordering::Relaxed);
            info!("macOS overlay: shutdown");
            // Would call close on windows
            Ok(())
        }

        async fn poll_events(&self) -> Vec<OverlayEvent> {
            // Would poll NSEvent queue
            Vec::new()
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::MacOSOverlayBackend;

// ============================================================================
// Stub Backend (for unsupported platforms or testing)
// ============================================================================

/// Stub overlay backend for testing or unsupported platforms.
pub struct StubOverlayBackend {
    stop_requested: AtomicBool,
    pause_requested: AtomicBool,
    status: Arc<RwLock<String>>,
    action: Arc<RwLock<String>>,
    prompt_input: Arc<RwLock<Option<String>>>,
    paused: AtomicBool,
}

impl StubOverlayBackend {
    pub fn new() -> Self {
        Self {
            stop_requested: AtomicBool::new(false),
            pause_requested: AtomicBool::new(false),
            status: Arc::new(RwLock::new(String::new())),
            action: Arc::new(RwLock::new(String::new())),
            prompt_input: Arc::new(RwLock::new(None)),
            paused: AtomicBool::new(false),
        }
    }

    /// Simulate a stop request (for testing).
    pub fn simulate_stop(&self) {
        self.stop_requested.store(true, Ordering::Relaxed);
    }

    /// Simulate a pause request (for testing).
    pub fn simulate_pause(&self) {
        self.pause_requested.store(true, Ordering::Relaxed);
    }

    /// Simulate prompt submission (for testing).
    pub async fn simulate_prompt(&self, prompt: &str) {
        let mut input = self.prompt_input.write().await;
        *input = Some(prompt.to_string());
    }
}

impl Default for StubOverlayBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl OverlayBackend for StubOverlayBackend {
    fn is_available(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Stub"
    }

    async fn initialize(&mut self, _config: &OverlayConfig) -> OverlayResult<()> {
        debug!("Stub backend initialized");
        Ok(())
    }

    async fn show(&self) -> OverlayResult<()> {
        debug!("Stub: show");
        Ok(())
    }

    async fn hide(&self) -> OverlayResult<()> {
        debug!("Stub: hide");
        Ok(())
    }

    async fn set_status(&self, message: &str) -> OverlayResult<()> {
        let mut status = self.status.write().await;
        *status = message.to_string();
        debug!("Stub: status = {}", message);
        Ok(())
    }

    async fn set_action(&self, action: &str) -> OverlayResult<()> {
        let mut action_state = self.action.write().await;
        *action_state = action.to_string();
        debug!("Stub: action = {}", action);
        Ok(())
    }

    async fn get_prompt_input(&self) -> Option<String> {
        self.prompt_input.read().await.clone()
    }

    async fn clear_prompt(&self) -> OverlayResult<()> {
        let mut prompt = self.prompt_input.write().await;
        *prompt = None;
        Ok(())
    }

    fn is_stop_requested(&self) -> bool {
        self.stop_requested.load(Ordering::Relaxed)
    }

    fn is_pause_requested(&self) -> bool {
        self.pause_requested.load(Ordering::Relaxed)
    }

    fn reset_requests(&self) {
        self.stop_requested.store(false, Ordering::Relaxed);
        self.pause_requested.store(false, Ordering::Relaxed);
    }

    async fn set_paused(&self, paused: bool) -> OverlayResult<()> {
        self.paused.store(paused, Ordering::Relaxed);
        debug!("Stub: paused = {}", paused);
        Ok(())
    }

    async fn shutdown(&self) -> OverlayResult<()> {
        debug!("Stub: shutdown");
        Ok(())
    }

    async fn poll_events(&self) -> Vec<OverlayEvent> {
        Vec::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_creation() {
        let color = Color::rgb(255, 0, 0);
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 0);
        assert_eq!(color.b, 0);
        assert_eq!(color.a, 255);

        let color_alpha = Color::rgba(255, 0, 0, 128);
        assert_eq!(color_alpha.a, 128);
    }

    #[test]
    fn test_color_conversions() {
        let color = Color::rgba(255, 128, 64, 200);

        assert_eq!(color.to_hex(), "#ff8040");
        assert!(color.to_css().contains("255"));
        assert!(color.to_css().contains("128"));
        assert!(color.to_css().contains("64"));

        let argb = color.to_argb();
        assert_eq!(argb, 0xC8FF8040);
    }

    #[test]
    fn test_ganesha_red() {
        let red = Color::ganesha_red();
        assert_eq!(red.r, 220);
        assert_eq!(red.g, 38);
        assert_eq!(red.b, 38);
        assert_eq!(red.a, 180);
    }

    #[test]
    fn test_window_position_coordinates() {
        let screen_width = 1920;
        let screen_height = 1080;
        let window_width = 320;
        let window_height = 180;
        let margin = 20;

        let pos = WindowPosition::TopLeft;
        let (x, y) = pos.to_coordinates(screen_width, screen_height, window_width, window_height, margin);
        assert_eq!((x, y), (20, 20));

        let pos = WindowPosition::TopRight;
        let (x, y) = pos.to_coordinates(screen_width, screen_height, window_width, window_height, margin);
        assert_eq!((x, y), (1580, 20));

        let pos = WindowPosition::BottomRight;
        let (x, y) = pos.to_coordinates(screen_width, screen_height, window_width, window_height, margin);
        assert_eq!((x, y), (1580, 880));

        let pos = WindowPosition::Center;
        let (x, y) = pos.to_coordinates(screen_width, screen_height, window_width, window_height, margin);
        assert_eq!((x, y), (800, 450));

        let pos = WindowPosition::Custom(100, 200);
        let (x, y) = pos.to_coordinates(screen_width, screen_height, window_width, window_height, margin);
        assert_eq!((x, y), (100, 200));
    }

    #[test]
    fn test_overlay_config_defaults() {
        let config = OverlayConfig::default();
        assert_eq!(config.border_width, 3);
        assert!(config.show_border);
        assert!(config.show_control_panel);
        assert!(config.border_click_through);
        assert_eq!(config.window_position, WindowPosition::TopRight);
    }

    #[test]
    fn test_overlay_config_minimal() {
        let config = OverlayConfig::minimal();
        assert!(!config.show_control_panel);
        assert_eq!(config.border_width, 2);
    }

    #[test]
    fn test_overlay_config_builder() {
        let config = OverlayConfig::default()
            .with_border_color(Color::rgb(0, 255, 0))
            .with_border_width(5)
            .with_position(WindowPosition::BottomLeft)
            .with_opacity(0.5);

        assert_eq!(config.border_color, Color::rgb(0, 255, 0));
        assert_eq!(config.border_width, 5);
        assert_eq!(config.window_position, WindowPosition::BottomLeft);
        assert!((config.opacity - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_is_ganesha_window() {
        assert!(is_ganesha_window("[GANESHA-OVERLAY] Control Panel"));
        assert!(is_ganesha_window("[GANESHA-OVERLAY] Screen Border"));
        assert!(is_ganesha_window("[GANESHA-OVERLAY] Something Else"));
        assert!(!is_ganesha_window("Firefox"));
        assert!(!is_ganesha_window("Terminal"));
        assert!(!is_ganesha_window("GANESHA")); // Missing bracket prefix
    }

    #[test]
    fn test_is_ganesha_window_class() {
        assert!(is_ganesha_window_class("GaneshaOverlay"));
        assert!(!is_ganesha_window_class("firefox"));
        assert!(!is_ganesha_window_class("Ganesha")); // Case sensitive
    }

    #[test]
    fn test_filter_ganesha_windows() {
        let windows = vec![
            "[GANESHA-OVERLAY] Control Panel".to_string(),
            "Firefox".to_string(),
            "[GANESHA-OVERLAY] Screen Border".to_string(),
            "Terminal".to_string(),
        ];

        let filtered = filter_ganesha_windows(&windows);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0], "Firefox");
        assert_eq!(filtered[1], "Terminal");
    }

    #[tokio::test]
    async fn test_screen_border() {
        let config = OverlayConfig::default();
        let border = ScreenBorder::new(config);

        assert!(!border.is_visible());

        border.show().await.unwrap();
        assert!(border.is_visible());

        border.hide().await.unwrap();
        assert!(!border.is_visible());
    }

    #[tokio::test]
    async fn test_stub_backend() {
        let mut backend = StubOverlayBackend::new();
        let config = OverlayConfig::default();

        assert!(backend.is_available());
        assert_eq!(backend.name(), "Stub");

        backend.initialize(&config).await.unwrap();
        backend.show().await.unwrap();
        backend.set_status("Testing...").await.unwrap();
        backend.set_action("Running tests").await.unwrap();

        assert!(!backend.is_stop_requested());
        assert!(!backend.is_pause_requested());

        backend.simulate_stop();
        assert!(backend.is_stop_requested());

        backend.simulate_pause();
        assert!(backend.is_pause_requested());

        backend.reset_requests();
        assert!(!backend.is_stop_requested());
        assert!(!backend.is_pause_requested());

        backend.simulate_prompt("Hello Ganesha").await;
        let prompt = backend.get_prompt_input().await;
        assert_eq!(prompt, Some("Hello Ganesha".to_string()));

        backend.clear_prompt().await.unwrap();
        let prompt = backend.get_prompt_input().await;
        assert!(prompt.is_none());

        backend.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_control_overlay_creation() {
        let config = OverlayConfig::default();
        let overlay = ControlOverlay::new(config).await.unwrap();

        assert!(!overlay.is_running());

        let state = overlay.state().await;
        assert!(!state.visible);
        assert!(state.status.is_empty());
        assert!(state.action.is_empty());
    }

    #[tokio::test]
    async fn test_control_overlay_commands() {
        let config = OverlayConfig::default();
        let overlay = ControlOverlay::new(config).await.unwrap();

        overlay.show().await.unwrap();
        // Give the command processor time to handle the command
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        overlay.set_status("Initializing...").await;
        overlay.set_action("Opening browser").await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let state = overlay.state().await;
        assert!(state.visible);
        assert_eq!(state.status, "Initializing...");
        assert_eq!(state.action, "Opening browser");

        overlay.hide().await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let state = overlay.state().await;
        assert!(!state.visible);
    }

    #[tokio::test]
    async fn test_overlay_state() {
        let state = OverlayState::default();

        assert!(!state.visible);
        assert!(!state.stop_requested);
        assert!(!state.pause_requested);
        assert!(!state.is_paused);
        assert!(state.prompt_input.is_none());
    }
}
