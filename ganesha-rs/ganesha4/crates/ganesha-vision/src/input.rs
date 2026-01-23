//! Input simulation for the Vision/VLA system.
//!
//! This module provides:
//! - Mouse movement and clicks with smooth easing curves
//! - Keyboard input (typing and shortcuts)
//! - Drag and drop operations
//! - Speed modes for different use cases (demo, normal, fast, beast)
//! - Input recording for demonstrations
//! - Input playback for replaying learned actions
//! - Platform-specific implementations (X11, Wayland, Windows, macOS)

use crate::capture::Region;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::{mpsc, Mutex, RwLock};

/// Errors that can occur during input simulation.
#[derive(Error, Debug)]
pub enum InputError {
    #[error("Input simulation not available on this platform")]
    NotAvailable,

    #[error("Failed to simulate input: {0}")]
    SimulationFailed(String),

    #[error("Invalid key: {0}")]
    InvalidKey(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Coordinates out of bounds: ({0}, {1})")]
    OutOfBounds(i32, i32),

    #[error("Operation cancelled")]
    Cancelled,

    #[error("Recording error: {0}")]
    RecordingError(String),

    #[error("Playback error: {0}")]
    PlaybackError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Result type for input operations.
pub type InputResult<T> = Result<T, InputError>;

// ============================================================================
// Speed Modes
// ============================================================================

/// Speed mode for input simulation.
/// Controls timing between actions for different use cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SpeedMode {
    /// Slow mode for demonstrations - actions are deliberate and visible
    /// Mouse movement: ~2 seconds, action delay: 500ms
    Slow,

    /// Normal mode - balanced speed for regular use
    /// Mouse movement: ~500ms, action delay: 100ms
    #[default]
    Normal,

    /// Fast mode - quick execution with minimal delays
    /// Mouse movement: ~200ms, action delay: 50ms
    Fast,

    /// Beast mode - maximum speed, one action per second
    /// For automated batch processing
    Beast,
}

impl SpeedMode {
    /// Get the base delay between actions for this speed mode.
    pub fn action_delay(&self) -> Duration {
        match self {
            SpeedMode::Slow => Duration::from_millis(500),
            SpeedMode::Normal => Duration::from_millis(100),
            SpeedMode::Fast => Duration::from_millis(50),
            SpeedMode::Beast => Duration::from_millis(10),
        }
    }

    /// Get the mouse movement duration for this speed mode.
    pub fn mouse_move_duration(&self) -> Duration {
        match self {
            SpeedMode::Slow => Duration::from_millis(2000),
            SpeedMode::Normal => Duration::from_millis(500),
            SpeedMode::Fast => Duration::from_millis(200),
            SpeedMode::Beast => Duration::from_millis(50),
        }
    }

    /// Get the typing delay between characters for this speed mode.
    pub fn typing_delay(&self) -> Duration {
        match self {
            SpeedMode::Slow => Duration::from_millis(100),
            SpeedMode::Normal => Duration::from_millis(30),
            SpeedMode::Fast => Duration::from_millis(10),
            SpeedMode::Beast => Duration::from_millis(0),
        }
    }

    /// Get the number of interpolation steps for smooth mouse movement.
    pub fn mouse_steps(&self) -> u32 {
        match self {
            SpeedMode::Slow => 60,
            SpeedMode::Normal => 30,
            SpeedMode::Fast => 15,
            SpeedMode::Beast => 5,
        }
    }

    /// Get the double-click delay for this speed mode.
    pub fn double_click_delay(&self) -> Duration {
        match self {
            SpeedMode::Slow => Duration::from_millis(200),
            SpeedMode::Normal => Duration::from_millis(100),
            SpeedMode::Fast => Duration::from_millis(50),
            SpeedMode::Beast => Duration::from_millis(20),
        }
    }
}

/// Timing configuration for input simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingConfig {
    /// Current speed mode
    pub speed_mode: SpeedMode,
    /// Override for action delay (if set)
    pub action_delay_override: Option<Duration>,
    /// Override for mouse movement duration (if set)
    pub mouse_duration_override: Option<Duration>,
    /// Override for typing delay (if set)
    pub typing_delay_override: Option<Duration>,
    /// Minimum delay between any actions (safety)
    pub min_action_delay: Duration,
    /// Maximum actions per second (rate limiting)
    pub max_actions_per_second: u32,
}

impl Default for TimingConfig {
    fn default() -> Self {
        Self {
            speed_mode: SpeedMode::Normal,
            action_delay_override: None,
            mouse_duration_override: None,
            typing_delay_override: None,
            min_action_delay: Duration::from_millis(10),
            max_actions_per_second: 60,
        }
    }
}

impl TimingConfig {
    /// Create a new timing config with the specified speed mode.
    pub fn with_speed(speed_mode: SpeedMode) -> Self {
        Self {
            speed_mode,
            ..Default::default()
        }
    }

    /// Get the effective action delay.
    pub fn action_delay(&self) -> Duration {
        self.action_delay_override
            .unwrap_or_else(|| self.speed_mode.action_delay())
            .max(self.min_action_delay)
    }

    /// Get the effective mouse movement duration.
    pub fn mouse_duration(&self) -> Duration {
        self.mouse_duration_override
            .unwrap_or_else(|| self.speed_mode.mouse_move_duration())
    }

    /// Get the effective typing delay.
    pub fn typing_delay(&self) -> Duration {
        self.typing_delay_override
            .unwrap_or_else(|| self.speed_mode.typing_delay())
    }

    /// Set a custom action delay.
    pub fn with_action_delay(mut self, delay: Duration) -> Self {
        self.action_delay_override = Some(delay);
        self
    }

    /// Set a custom mouse movement duration.
    pub fn with_mouse_duration(mut self, duration: Duration) -> Self {
        self.mouse_duration_override = Some(duration);
        self
    }
}

// ============================================================================
// Easing Curves
// ============================================================================

/// Easing curve type for smooth animations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EasingCurve {
    /// Linear interpolation - constant speed
    Linear,

    /// Ease out - starts fast, slows down at the end
    EaseOut,

    /// Ease in - starts slow, speeds up
    EaseIn,

    /// Ease in-out - slow start, fast middle, slow end
    #[default]
    EaseInOut,

    /// Ease out cubic - more pronounced deceleration
    EaseOutCubic,

    /// Ease in cubic - more pronounced acceleration
    EaseInCubic,

    /// Ease in-out cubic - smoother s-curve
    EaseInOutCubic,

    /// Ease out elastic - slight overshoot with bounce back
    EaseOutElastic,

    /// Ease out back - slight overshoot
    EaseOutBack,
}

impl EasingCurve {
    /// Apply the easing function to a progress value [0.0, 1.0].
    /// Returns a value typically in [0.0, 1.0] but may exceed for elastic/back curves.
    pub fn apply(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            EasingCurve::Linear => t,

            EasingCurve::EaseOut => 1.0 - (1.0 - t).powi(2),

            EasingCurve::EaseIn => t * t,

            EasingCurve::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }

            EasingCurve::EaseOutCubic => 1.0 - (1.0 - t).powi(3),

            EasingCurve::EaseInCubic => t * t * t,

            EasingCurve::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }

            EasingCurve::EaseOutElastic => {
                if t == 0.0 || t == 1.0 {
                    t
                } else {
                    let c4 = (2.0 * std::f64::consts::PI) / 3.0;
                    2.0_f64.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
                }
            }

            EasingCurve::EaseOutBack => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
        }
    }

    /// Interpolate between two values using this easing curve.
    pub fn interpolate(&self, start: f64, end: f64, t: f64) -> f64 {
        let eased = self.apply(t);
        start + (end - start) * eased
    }

    /// Interpolate between two integer coordinates.
    pub fn interpolate_i32(&self, start: i32, end: i32, t: f64) -> i32 {
        self.interpolate(start as f64, end as f64, t) as i32
    }
}

// ============================================================================
// Mouse Types
// ============================================================================

/// Mouse button types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

impl Default for MouseButton {
    fn default() -> Self {
        Self::Left
    }
}

/// Mouse action types (high-level enum for recording/playback).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MouseActionType {
    /// Move mouse to coordinates
    Move {
        x: i32,
        y: i32,
        smooth: bool,
        easing: Option<EasingCurve>,
    },

    /// Click at current or specified position
    Click {
        x: Option<i32>,
        y: Option<i32>,
        button: MouseButton,
    },

    /// Double-click at current or specified position
    DoubleClick {
        x: Option<i32>,
        y: Option<i32>,
        button: MouseButton,
    },

    /// Right-click at current or specified position
    RightClick { x: Option<i32>, y: Option<i32> },

    /// Drag from start to end position
    Drag {
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
        button: MouseButton,
    },

    /// Scroll at current or specified position
    Scroll {
        x: Option<i32>,
        y: Option<i32>,
        delta_x: i32,
        delta_y: i32,
    },

    /// Press and hold a button
    ButtonDown { button: MouseButton },

    /// Release a button
    ButtonUp { button: MouseButton },
}

// ============================================================================
// Keyboard Types
// ============================================================================

/// Keyboard modifier keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Modifier {
    Shift,
    Control,
    Alt,
    Meta, // Windows key or Command key
    Super,
}

/// Special keys that can be pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Key {
    // Function keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    // Navigation
    Escape,
    Tab,
    CapsLock,
    Backspace,
    Enter,
    Space,
    Insert,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,

    // Modifiers
    Shift,
    Control,
    Alt,
    Meta,
    Super,

    // Numpad
    NumLock,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    NumpadAdd,
    NumpadSubtract,
    NumpadMultiply,
    NumpadDivide,
    NumpadDecimal,
    NumpadEnter,

    // Media keys
    VolumeUp,
    VolumeDown,
    VolumeMute,
    PlayPause,
    Stop,
    NextTrack,
    PreviousTrack,

    // Other
    PrintScreen,
    ScrollLock,
    Pause,
}

/// Key input type (character or special key).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KeyInput {
    /// A regular character
    Char(char),
    /// A special key
    Special(Key),
}

impl From<char> for KeyInput {
    fn from(c: char) -> Self {
        Self::Char(c)
    }
}

impl From<Key> for KeyInput {
    fn from(key: Key) -> Self {
        Self::Special(key)
    }
}

/// Keyboard action types (high-level enum for recording/playback).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KeyActionType {
    /// Press and release a key
    Press { key: KeyInput },

    /// Hold down a key
    KeyDown { key: KeyInput },

    /// Release a key
    KeyUp { key: KeyInput },

    /// Type a string of text
    Type { text: String },

    /// Execute a keyboard shortcut
    Shortcut {
        modifiers: Vec<Modifier>,
        key: KeyInput,
    },
}

/// A keyboard shortcut (combination of modifiers and a key).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardShortcut {
    /// Modifier keys held during the shortcut
    pub modifiers: Vec<Modifier>,
    /// The main key of the shortcut
    pub key: KeyInput,
}

impl KeyboardShortcut {
    /// Create a new keyboard shortcut.
    pub fn new(key: KeyInput) -> Self {
        Self {
            modifiers: Vec::new(),
            key,
        }
    }

    /// Add a modifier to the shortcut.
    pub fn with_modifier(mut self, modifier: Modifier) -> Self {
        if !self.modifiers.contains(&modifier) {
            self.modifiers.push(modifier);
        }
        self
    }

    /// Create a Ctrl+key shortcut.
    pub fn ctrl(key: KeyInput) -> Self {
        Self::new(key).with_modifier(Modifier::Control)
    }

    /// Create an Alt+key shortcut.
    pub fn alt(key: KeyInput) -> Self {
        Self::new(key).with_modifier(Modifier::Alt)
    }

    /// Create a Shift+key shortcut.
    pub fn shift(key: KeyInput) -> Self {
        Self::new(key).with_modifier(Modifier::Shift)
    }

    /// Create a Meta/Super+key shortcut.
    pub fn meta(key: KeyInput) -> Self {
        Self::new(key).with_modifier(Modifier::Meta)
    }

    /// Create a Ctrl+Shift+key shortcut.
    pub fn ctrl_shift(key: KeyInput) -> Self {
        Self::new(key)
            .with_modifier(Modifier::Control)
            .with_modifier(Modifier::Shift)
    }

    /// Create a Ctrl+Alt+key shortcut.
    pub fn ctrl_alt(key: KeyInput) -> Self {
        Self::new(key)
            .with_modifier(Modifier::Control)
            .with_modifier(Modifier::Alt)
    }

    /// Parse a shortcut string like "Ctrl+Shift+S".
    pub fn parse(s: &str) -> Result<Self, InputError> {
        let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
        if parts.is_empty() {
            return Err(InputError::InvalidKey("Empty shortcut".to_string()));
        }

        let mut modifiers = Vec::new();
        let mut key_str = None;

        for part in parts {
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => modifiers.push(Modifier::Control),
                "shift" => modifiers.push(Modifier::Shift),
                "alt" => modifiers.push(Modifier::Alt),
                "meta" | "cmd" | "command" | "win" | "super" => modifiers.push(Modifier::Meta),
                _ => {
                    if key_str.is_some() {
                        return Err(InputError::InvalidKey(format!(
                            "Multiple keys in shortcut: {}",
                            s
                        )));
                    }
                    key_str = Some(part);
                }
            }
        }

        let key_str =
            key_str.ok_or_else(|| InputError::InvalidKey("No key in shortcut".to_string()))?;

        // Parse the key
        let key = if key_str.len() == 1 {
            KeyInput::Char(key_str.chars().next().unwrap())
        } else {
            match key_str.to_lowercase().as_str() {
                "f1" => KeyInput::Special(Key::F1),
                "f2" => KeyInput::Special(Key::F2),
                "f3" => KeyInput::Special(Key::F3),
                "f4" => KeyInput::Special(Key::F4),
                "f5" => KeyInput::Special(Key::F5),
                "f6" => KeyInput::Special(Key::F6),
                "f7" => KeyInput::Special(Key::F7),
                "f8" => KeyInput::Special(Key::F8),
                "f9" => KeyInput::Special(Key::F9),
                "f10" => KeyInput::Special(Key::F10),
                "f11" => KeyInput::Special(Key::F11),
                "f12" => KeyInput::Special(Key::F12),
                "escape" | "esc" => KeyInput::Special(Key::Escape),
                "tab" => KeyInput::Special(Key::Tab),
                "backspace" | "back" => KeyInput::Special(Key::Backspace),
                "enter" | "return" => KeyInput::Special(Key::Enter),
                "space" => KeyInput::Special(Key::Space),
                "delete" | "del" => KeyInput::Special(Key::Delete),
                "home" => KeyInput::Special(Key::Home),
                "end" => KeyInput::Special(Key::End),
                "pageup" | "pgup" => KeyInput::Special(Key::PageUp),
                "pagedown" | "pgdn" => KeyInput::Special(Key::PageDown),
                "up" => KeyInput::Special(Key::Up),
                "down" => KeyInput::Special(Key::Down),
                "left" => KeyInput::Special(Key::Left),
                "right" => KeyInput::Special(Key::Right),
                _ => return Err(InputError::InvalidKey(format!("Unknown key: {}", key_str))),
            }
        };

        Ok(Self { modifiers, key })
    }
}

// ============================================================================
// Action Structures (for legacy compatibility)
// ============================================================================

/// Mouse click type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClickType {
    /// Single click
    Single,
    /// Double click
    Double,
    /// Triple click (select line/paragraph)
    Triple,
    /// Press and hold (for drag operations)
    Press,
    /// Release (end drag operation)
    Release,
}

impl Default for ClickType {
    fn default() -> Self {
        Self::Single
    }
}

/// A mouse action to perform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseAction {
    /// Type of click (single, double, etc.)
    pub click_type: ClickType,
    /// Which button to use
    pub button: MouseButton,
    /// Target coordinates
    pub x: i32,
    pub y: i32,
    /// Modifiers held during click
    pub modifiers: Vec<Modifier>,
}

impl MouseAction {
    /// Create a single left-click at coordinates.
    pub fn click(x: i32, y: i32) -> Self {
        Self {
            click_type: ClickType::Single,
            button: MouseButton::Left,
            x,
            y,
            modifiers: Vec::new(),
        }
    }

    /// Create a double-click at coordinates.
    pub fn double_click(x: i32, y: i32) -> Self {
        Self {
            click_type: ClickType::Double,
            button: MouseButton::Left,
            x,
            y,
            modifiers: Vec::new(),
        }
    }

    /// Create a right-click at coordinates.
    pub fn right_click(x: i32, y: i32) -> Self {
        Self {
            click_type: ClickType::Single,
            button: MouseButton::Right,
            x,
            y,
            modifiers: Vec::new(),
        }
    }

    /// Add a modifier to the click.
    pub fn with_modifier(mut self, modifier: Modifier) -> Self {
        if !self.modifiers.contains(&modifier) {
            self.modifiers.push(modifier);
        }
        self
    }
}

/// A drag operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DragOperation {
    /// Starting coordinates
    pub start_x: i32,
    pub start_y: i32,
    /// Ending coordinates
    pub end_x: i32,
    pub end_y: i32,
    /// Mouse button to use
    pub button: MouseButton,
    /// Duration of the drag
    pub duration: Duration,
    /// Modifiers held during drag
    pub modifiers: Vec<Modifier>,
    /// Easing curve for smooth movement
    pub easing: EasingCurve,
}

impl DragOperation {
    /// Create a new drag operation.
    pub fn new(start_x: i32, start_y: i32, end_x: i32, end_y: i32) -> Self {
        Self {
            start_x,
            start_y,
            end_x,
            end_y,
            button: MouseButton::Left,
            duration: Duration::from_millis(500),
            modifiers: Vec::new(),
            easing: EasingCurve::EaseInOut,
        }
    }

    /// Create from a start region to an end region.
    pub fn from_regions(start: &Region, end: &Region) -> Self {
        let (sx, sy) = start.center();
        let (ex, ey) = end.center();
        Self::new(sx, sy, ex, ey)
    }

    /// Set the drag duration.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Add a modifier to the drag.
    pub fn with_modifier(mut self, modifier: Modifier) -> Self {
        if !self.modifiers.contains(&modifier) {
            self.modifiers.push(modifier);
        }
        self
    }

    /// Set the easing curve.
    pub fn with_easing(mut self, easing: EasingCurve) -> Self {
        self.easing = easing;
        self
    }
}

/// A scroll action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrollAction {
    /// X coordinate of scroll
    pub x: i32,
    /// Y coordinate of scroll
    pub y: i32,
    /// Horizontal scroll amount (positive = right)
    pub delta_x: i32,
    /// Vertical scroll amount (positive = down)
    pub delta_y: i32,
}

impl ScrollAction {
    /// Create a vertical scroll at a position.
    pub fn vertical(x: i32, y: i32, amount: i32) -> Self {
        Self {
            x,
            y,
            delta_x: 0,
            delta_y: amount,
        }
    }

    /// Create a horizontal scroll at a position.
    pub fn horizontal(x: i32, y: i32, amount: i32) -> Self {
        Self {
            x,
            y,
            delta_x: amount,
            delta_y: 0,
        }
    }
}

// ============================================================================
// Recording and Playback
// ============================================================================

/// A recorded input event with timing information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedEvent {
    /// Timestamp relative to recording start (milliseconds)
    pub timestamp_ms: u64,
    /// The action that was performed
    pub action: RecordedAction,
    /// Optional description/annotation
    pub annotation: Option<String>,
}

/// A recorded action (unified mouse/keyboard).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "category", rename_all = "snake_case")]
pub enum RecordedAction {
    /// Mouse action
    Mouse(MouseActionType),
    /// Keyboard action
    Keyboard(KeyActionType),
    /// Wait/delay action
    Wait { duration_ms: u64 },
    /// Screenshot marker (for verification)
    Screenshot { description: String },
    /// Custom marker for segmenting recordings
    Marker { name: String },
}

/// A complete recording of input events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputRecording {
    /// Recording metadata
    pub metadata: RecordingMetadata,
    /// List of recorded events
    pub events: Vec<RecordedEvent>,
}

/// Metadata about a recording.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingMetadata {
    /// Name/title of the recording
    pub name: String,
    /// Description of what this recording does
    pub description: Option<String>,
    /// When the recording was created (Unix timestamp)
    pub created_at: i64,
    /// Total duration of the recording (milliseconds)
    pub duration_ms: u64,
    /// Target application (if known)
    pub target_app: Option<String>,
    /// Screen resolution at time of recording
    pub screen_resolution: Option<(u32, u32)>,
    /// Version of the recording format
    pub version: String,
    /// Tags for categorization
    pub tags: Vec<String>,
}

impl Default for RecordingMetadata {
    fn default() -> Self {
        Self {
            name: "Untitled Recording".to_string(),
            description: None,
            created_at: chrono::Utc::now().timestamp(),
            duration_ms: 0,
            target_app: None,
            screen_resolution: None,
            version: "1.0".to_string(),
            tags: Vec::new(),
        }
    }
}

impl InputRecording {
    /// Create a new empty recording.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            metadata: RecordingMetadata {
                name: name.into(),
                ..Default::default()
            },
            events: Vec::new(),
        }
    }

    /// Add an event to the recording.
    pub fn add_event(&mut self, event: RecordedEvent) {
        // Update duration
        if event.timestamp_ms > self.metadata.duration_ms {
            self.metadata.duration_ms = event.timestamp_ms;
        }
        self.events.push(event);
    }

    /// Get the total duration.
    pub fn duration(&self) -> Duration {
        Duration::from_millis(self.metadata.duration_ms)
    }

    /// Save the recording to a file.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> InputResult<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| InputError::SerializationError(e.to_string()))?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load a recording from a file.
    pub fn load<P: AsRef<Path>>(path: P) -> InputResult<Self> {
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json).map_err(|e| InputError::SerializationError(e.to_string()))
    }

    /// Get events within a time range.
    pub fn events_in_range(&self, start_ms: u64, end_ms: u64) -> Vec<&RecordedEvent> {
        self.events
            .iter()
            .filter(|e| e.timestamp_ms >= start_ms && e.timestamp_ms <= end_ms)
            .collect()
    }
}

/// State of an input recorder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecorderState {
    /// Not recording
    Idle,
    /// Currently recording
    Recording,
    /// Paused
    Paused,
}

/// Input recorder for capturing user demonstrations.
pub struct InputRecorder {
    /// Current state
    state: Arc<RwLock<RecorderState>>,
    /// Recording being built
    recording: Arc<Mutex<InputRecording>>,
    /// Start time of recording
    start_time: Arc<RwLock<Option<Instant>>>,
    /// Channel for receiving events
    event_tx: mpsc::Sender<RecordedEvent>,
    /// Channel receiver (moved to background task)
    event_rx: Arc<Mutex<Option<mpsc::Receiver<RecordedEvent>>>>,
}

impl InputRecorder {
    /// Create a new input recorder.
    pub fn new(name: impl Into<String>) -> Self {
        let (event_tx, event_rx) = mpsc::channel(1000);
        Self {
            state: Arc::new(RwLock::new(RecorderState::Idle)),
            recording: Arc::new(Mutex::new(InputRecording::new(name))),
            start_time: Arc::new(RwLock::new(None)),
            event_tx,
            event_rx: Arc::new(Mutex::new(Some(event_rx))),
        }
    }

    /// Start recording.
    pub async fn start(&self) -> InputResult<()> {
        let mut state = self.state.write().await;
        if *state == RecorderState::Recording {
            return Err(InputError::RecordingError("Already recording".to_string()));
        }

        *state = RecorderState::Recording;
        *self.start_time.write().await = Some(Instant::now());

        // Start background event processing
        let recording = Arc::clone(&self.recording);
        let state_clone = Arc::clone(&self.state);
        let mut rx = self
            .event_rx
            .lock()
            .await
            .take()
            .ok_or_else(|| InputError::RecordingError("Recorder already started".to_string()))?;

        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                let state = *state_clone.read().await;
                if state == RecorderState::Recording {
                    recording.lock().await.add_event(event);
                }
            }
        });

        Ok(())
    }

    /// Pause recording.
    pub async fn pause(&self) -> InputResult<()> {
        let mut state = self.state.write().await;
        if *state != RecorderState::Recording {
            return Err(InputError::RecordingError("Not recording".to_string()));
        }
        *state = RecorderState::Paused;
        Ok(())
    }

    /// Resume recording.
    pub async fn resume(&self) -> InputResult<()> {
        let mut state = self.state.write().await;
        if *state != RecorderState::Paused {
            return Err(InputError::RecordingError("Not paused".to_string()));
        }
        *state = RecorderState::Recording;
        Ok(())
    }

    /// Stop recording and return the recording.
    pub async fn stop(&self) -> InputResult<InputRecording> {
        let mut state = self.state.write().await;
        *state = RecorderState::Idle;

        // Close the channel to stop the background task
        drop(self.event_tx.clone());

        let recording = self.recording.lock().await.clone();
        Ok(recording)
    }

    /// Record a mouse action.
    pub async fn record_mouse(&self, action: MouseActionType) -> InputResult<()> {
        self.record_action(RecordedAction::Mouse(action)).await
    }

    /// Record a keyboard action.
    pub async fn record_keyboard(&self, action: KeyActionType) -> InputResult<()> {
        self.record_action(RecordedAction::Keyboard(action)).await
    }

    /// Record a marker.
    pub async fn record_marker(&self, name: impl Into<String>) -> InputResult<()> {
        self.record_action(RecordedAction::Marker { name: name.into() })
            .await
    }

    /// Record a wait.
    pub async fn record_wait(&self, duration: Duration) -> InputResult<()> {
        self.record_action(RecordedAction::Wait {
            duration_ms: duration.as_millis() as u64,
        })
        .await
    }

    /// Record an arbitrary action.
    async fn record_action(&self, action: RecordedAction) -> InputResult<()> {
        let state = *self.state.read().await;
        if state != RecorderState::Recording {
            return Err(InputError::RecordingError("Not recording".to_string()));
        }

        let timestamp_ms = self
            .start_time
            .read()
            .await
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0);

        let event = RecordedEvent {
            timestamp_ms,
            action,
            annotation: None,
        };

        self.event_tx
            .send(event)
            .await
            .map_err(|e| InputError::RecordingError(e.to_string()))?;

        Ok(())
    }

    /// Get the current state.
    pub async fn state(&self) -> RecorderState {
        *self.state.read().await
    }
}

/// Playback options for replaying recordings.
#[derive(Clone)]
pub struct PlaybackOptions {
    /// Speed multiplier (1.0 = normal speed, 2.0 = 2x speed, 0.5 = half speed)
    pub speed_multiplier: f64,
    /// Whether to preserve original timing or use speed mode timing
    pub preserve_timing: bool,
    /// Speed mode to use if not preserving timing
    pub speed_mode: SpeedMode,
    /// Whether to loop the playback
    pub loop_playback: bool,
    /// Start position (event index)
    pub start_index: usize,
    /// End position (event index, None = end)
    pub end_index: Option<usize>,
    /// Callback for progress updates
    pub on_progress: Option<Arc<dyn Fn(usize, usize) + Send + Sync>>,
    /// Whether to pause on markers
    pub pause_on_markers: bool,
}

impl Default for PlaybackOptions {
    fn default() -> Self {
        Self {
            speed_multiplier: 1.0,
            preserve_timing: true,
            speed_mode: SpeedMode::Normal,
            loop_playback: false,
            start_index: 0,
            end_index: None,
            on_progress: None,
            pause_on_markers: false,
        }
    }
}

impl PlaybackOptions {
    /// Create options for demo playback (slow).
    pub fn demo() -> Self {
        Self {
            speed_multiplier: 0.5,
            preserve_timing: false,
            speed_mode: SpeedMode::Slow,
            ..Default::default()
        }
    }

    /// Create options for fast playback.
    pub fn fast() -> Self {
        Self {
            speed_multiplier: 2.0,
            preserve_timing: true,
            ..Default::default()
        }
    }

    /// Create options for beast mode (maximum speed).
    pub fn beast() -> Self {
        Self {
            preserve_timing: false,
            speed_mode: SpeedMode::Beast,
            ..Default::default()
        }
    }
}

/// State of input playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    /// Not playing
    Idle,
    /// Currently playing
    Playing,
    /// Paused
    Paused,
    /// Completed
    Completed,
}

/// Input player for replaying recorded actions.
pub struct InputPlayer<S: InputSimulator> {
    /// The simulator to use for playback
    simulator: Arc<S>,
    /// Current playback state
    state: Arc<RwLock<PlaybackState>>,
    /// Current event index
    current_index: Arc<RwLock<usize>>,
    /// Cancel signal
    cancel_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
}

impl<S: InputSimulator + 'static> InputPlayer<S> {
    /// Create a new input player.
    pub fn new(simulator: S) -> Self {
        Self {
            simulator: Arc::new(simulator),
            state: Arc::new(RwLock::new(PlaybackState::Idle)),
            current_index: Arc::new(RwLock::new(0)),
            cancel_tx: Arc::new(Mutex::new(None)),
        }
    }

    /// Play a recording with the given options.
    pub async fn play(
        &self,
        recording: &InputRecording,
        options: PlaybackOptions,
    ) -> InputResult<()> {
        // Check if already playing
        {
            let state = *self.state.read().await;
            if state == PlaybackState::Playing {
                return Err(InputError::PlaybackError("Already playing".to_string()));
            }
        }

        // Set up cancellation
        let (cancel_tx, mut cancel_rx) = mpsc::channel::<()>(1);
        *self.cancel_tx.lock().await = Some(cancel_tx);

        // Set state to playing
        *self.state.write().await = PlaybackState::Playing;
        *self.current_index.write().await = options.start_index;

        let end_index = options.end_index.unwrap_or(recording.events.len());
        let events = &recording.events[options.start_index..end_index];

        let mut last_timestamp_ms = if options.start_index > 0 && !recording.events.is_empty() {
            recording.events[options.start_index.saturating_sub(1)].timestamp_ms
        } else {
            0
        };

        loop {
            for (i, event) in events.iter().enumerate() {
                // Check for cancellation
                if cancel_rx.try_recv().is_ok() {
                    *self.state.write().await = PlaybackState::Idle;
                    return Err(InputError::Cancelled);
                }

                // Check if paused
                while *self.state.read().await == PlaybackState::Paused {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    if cancel_rx.try_recv().is_ok() {
                        *self.state.write().await = PlaybackState::Idle;
                        return Err(InputError::Cancelled);
                    }
                }

                // Update current index
                *self.current_index.write().await = options.start_index + i;

                // Call progress callback
                if let Some(ref callback) = options.on_progress {
                    callback(i, events.len());
                }

                // Calculate delay
                let delay = if options.preserve_timing {
                    let delta_ms = event.timestamp_ms.saturating_sub(last_timestamp_ms);
                    Duration::from_millis((delta_ms as f64 / options.speed_multiplier) as u64)
                } else {
                    options.speed_mode.action_delay()
                };

                // Wait before action
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }

                // Execute the action
                self.execute_action(&event.action, &options).await?;

                last_timestamp_ms = event.timestamp_ms;

                // Handle markers
                if options.pause_on_markers {
                    if let RecordedAction::Marker { .. } = &event.action {
                        *self.state.write().await = PlaybackState::Paused;
                    }
                }
            }

            if !options.loop_playback {
                break;
            }

            // Reset for loop
            last_timestamp_ms = 0;
        }

        *self.state.write().await = PlaybackState::Completed;
        Ok(())
    }

    /// Execute a single recorded action.
    async fn execute_action(
        &self,
        action: &RecordedAction,
        options: &PlaybackOptions,
    ) -> InputResult<()> {
        match action {
            RecordedAction::Mouse(mouse_action) => {
                self.execute_mouse_action(mouse_action, options).await
            }
            RecordedAction::Keyboard(key_action) => {
                self.execute_keyboard_action(key_action, options).await
            }
            RecordedAction::Wait { duration_ms } => {
                let adjusted = (*duration_ms as f64 / options.speed_multiplier) as u64;
                tokio::time::sleep(Duration::from_millis(adjusted)).await;
                Ok(())
            }
            RecordedAction::Screenshot { .. } | RecordedAction::Marker { .. } => {
                // No-op for playback
                Ok(())
            }
        }
    }

    /// Execute a mouse action.
    async fn execute_mouse_action(
        &self,
        action: &MouseActionType,
        options: &PlaybackOptions,
    ) -> InputResult<()> {
        match action {
            MouseActionType::Move {
                x,
                y,
                smooth,
                easing,
            } => {
                if *smooth {
                    let duration = if options.preserve_timing {
                        Duration::from_millis(
                            (options.speed_mode.mouse_move_duration().as_millis() as f64
                                / options.speed_multiplier) as u64,
                        )
                    } else {
                        options.speed_mode.mouse_move_duration()
                    };
                    let easing = easing.unwrap_or(EasingCurve::EaseInOut);
                    smooth_move(&*self.simulator, *x, *y, duration, easing).await
                } else {
                    self.simulator.mouse_move(*x, *y).await
                }
            }

            MouseActionType::Click { x, y, button } => {
                if let (Some(x), Some(y)) = (x, y) {
                    self.simulator.mouse_move(*x, *y).await?;
                }
                let action = MouseAction {
                    click_type: ClickType::Single,
                    button: *button,
                    x: x.unwrap_or(0),
                    y: y.unwrap_or(0),
                    modifiers: Vec::new(),
                };
                self.simulator.mouse_click(&action).await
            }

            MouseActionType::DoubleClick { x, y, button } => {
                if let (Some(x), Some(y)) = (x, y) {
                    self.simulator.mouse_move(*x, *y).await?;
                }
                let action = MouseAction {
                    click_type: ClickType::Double,
                    button: *button,
                    x: x.unwrap_or(0),
                    y: y.unwrap_or(0),
                    modifiers: Vec::new(),
                };
                self.simulator.mouse_click(&action).await
            }

            MouseActionType::RightClick { x, y } => {
                if let (Some(x), Some(y)) = (x, y) {
                    self.simulator.mouse_move(*x, *y).await?;
                }
                self.simulator
                    .right_click(x.unwrap_or(0), y.unwrap_or(0))
                    .await
            }

            MouseActionType::Drag {
                start_x,
                start_y,
                end_x,
                end_y,
                button,
            } => {
                let drag = DragOperation {
                    start_x: *start_x,
                    start_y: *start_y,
                    end_x: *end_x,
                    end_y: *end_y,
                    button: *button,
                    duration: options.speed_mode.mouse_move_duration(),
                    modifiers: Vec::new(),
                    easing: EasingCurve::EaseInOut,
                };
                self.simulator.mouse_drag(&drag).await
            }

            MouseActionType::Scroll {
                x,
                y,
                delta_x,
                delta_y,
            } => {
                let scroll = ScrollAction {
                    x: x.unwrap_or(0),
                    y: y.unwrap_or(0),
                    delta_x: *delta_x,
                    delta_y: *delta_y,
                };
                self.simulator.mouse_scroll(&scroll).await
            }

            MouseActionType::ButtonDown { button: _ } | MouseActionType::ButtonUp { button: _ } => {
                // These require lower-level access not in the current trait
                Ok(())
            }
        }
    }

    /// Execute a keyboard action.
    async fn execute_keyboard_action(
        &self,
        action: &KeyActionType,
        _options: &PlaybackOptions,
    ) -> InputResult<()> {
        match action {
            KeyActionType::Press { key } => self.simulator.key_press(key.clone()).await,

            KeyActionType::KeyDown { key } => self.simulator.key_down(key.clone()).await,

            KeyActionType::KeyUp { key } => self.simulator.key_up(key.clone()).await,

            KeyActionType::Type { text } => self.simulator.type_text(text).await,

            KeyActionType::Shortcut { modifiers, key } => {
                let shortcut = KeyboardShortcut {
                    modifiers: modifiers.clone(),
                    key: key.clone(),
                };
                self.simulator.shortcut(&shortcut).await
            }
        }
    }

    /// Pause playback.
    pub async fn pause(&self) -> InputResult<()> {
        let mut state = self.state.write().await;
        if *state != PlaybackState::Playing {
            return Err(InputError::PlaybackError("Not playing".to_string()));
        }
        *state = PlaybackState::Paused;
        Ok(())
    }

    /// Resume playback.
    pub async fn resume(&self) -> InputResult<()> {
        let mut state = self.state.write().await;
        if *state != PlaybackState::Paused {
            return Err(InputError::PlaybackError("Not paused".to_string()));
        }
        *state = PlaybackState::Playing;
        Ok(())
    }

    /// Stop playback.
    pub async fn stop(&self) -> InputResult<()> {
        if let Some(tx) = self.cancel_tx.lock().await.take() {
            let _ = tx.send(()).await;
        }
        *self.state.write().await = PlaybackState::Idle;
        Ok(())
    }

    /// Get the current playback state.
    pub async fn state(&self) -> PlaybackState {
        *self.state.read().await
    }

    /// Get the current event index.
    pub async fn current_index(&self) -> usize {
        *self.current_index.read().await
    }
}

// ============================================================================
// Smooth Mouse Movement
// ============================================================================

/// Perform smooth mouse movement with configurable easing.
pub async fn smooth_move<S: InputSimulator + ?Sized>(
    simulator: &S,
    target_x: i32,
    target_y: i32,
    duration: Duration,
    easing: EasingCurve,
) -> InputResult<()> {
    let (start_x, start_y) = simulator.mouse_position().await?;

    // Calculate number of steps based on duration
    let total_ms = duration.as_millis() as u64;
    let step_ms = 16; // ~60 FPS
    let steps = (total_ms / step_ms).max(1) as u32;

    for i in 1..=steps {
        let t = i as f64 / steps as f64;
        let current_x = easing.interpolate_i32(start_x, target_x, t);
        let current_y = easing.interpolate_i32(start_y, target_y, t);

        simulator.mouse_move(current_x, current_y).await?;
        tokio::time::sleep(Duration::from_millis(step_ms)).await;
    }

    // Ensure we end exactly at the target
    simulator.mouse_move(target_x, target_y).await?;

    Ok(())
}

/// Perform smooth mouse movement with default easing (ease-in-out).
pub async fn smooth_move_default<S: InputSimulator + ?Sized>(
    simulator: &S,
    target_x: i32,
    target_y: i32,
    duration: Duration,
) -> InputResult<()> {
    smooth_move(simulator, target_x, target_y, duration, EasingCurve::EaseInOut).await
}

// ============================================================================
// InputSimulator Trait
// ============================================================================

/// Trait for platform-specific input simulation.
#[async_trait]
pub trait InputSimulator: Send + Sync {
    /// Check if input simulation is available.
    fn is_available(&self) -> bool;

    /// Get the current mouse position.
    async fn mouse_position(&self) -> InputResult<(i32, i32)>;

    /// Move the mouse to coordinates (instant).
    async fn mouse_move(&self, x: i32, y: i32) -> InputResult<()>;

    /// Move the mouse smoothly to coordinates over a duration.
    async fn mouse_move_smooth(&self, x: i32, y: i32, duration: Duration) -> InputResult<()>;

    /// Move the mouse smoothly with a specific easing curve.
    async fn mouse_move_eased(
        &self,
        x: i32,
        y: i32,
        duration: Duration,
        easing: EasingCurve,
    ) -> InputResult<()> {
        smooth_move(self, x, y, duration, easing).await
    }

    /// Perform a mouse click.
    async fn mouse_click(&self, action: &MouseAction) -> InputResult<()>;

    /// Perform a mouse drag.
    async fn mouse_drag(&self, drag: &DragOperation) -> InputResult<()>;

    /// Scroll the mouse wheel.
    async fn mouse_scroll(&self, scroll: &ScrollAction) -> InputResult<()>;

    /// Type a string of text.
    async fn type_text(&self, text: &str) -> InputResult<()>;

    /// Press a single key.
    async fn key_press(&self, key: KeyInput) -> InputResult<()>;

    /// Hold down a key.
    async fn key_down(&self, key: KeyInput) -> InputResult<()>;

    /// Release a key.
    async fn key_up(&self, key: KeyInput) -> InputResult<()>;

    /// Execute a keyboard shortcut.
    async fn shortcut(&self, shortcut: &KeyboardShortcut) -> InputResult<()>;

    // Convenience methods with default implementations

    /// Click at coordinates.
    async fn click(&self, x: i32, y: i32) -> InputResult<()> {
        self.mouse_click(&MouseAction::click(x, y)).await
    }

    /// Double-click at coordinates.
    async fn double_click(&self, x: i32, y: i32) -> InputResult<()> {
        self.mouse_click(&MouseAction::double_click(x, y)).await
    }

    /// Right-click at coordinates.
    async fn right_click(&self, x: i32, y: i32) -> InputResult<()> {
        self.mouse_click(&MouseAction::right_click(x, y)).await
    }

    /// Press Enter key.
    async fn press_enter(&self) -> InputResult<()> {
        self.key_press(Key::Enter.into()).await
    }

    /// Press Tab key.
    async fn press_tab(&self) -> InputResult<()> {
        self.key_press(Key::Tab.into()).await
    }

    /// Press Escape key.
    async fn press_escape(&self) -> InputResult<()> {
        self.key_press(Key::Escape.into()).await
    }

    /// Copy (Ctrl+C / Cmd+C).
    async fn copy(&self) -> InputResult<()> {
        self.shortcut(&KeyboardShortcut::ctrl('c'.into())).await
    }

    /// Paste (Ctrl+V / Cmd+V).
    async fn paste(&self) -> InputResult<()> {
        self.shortcut(&KeyboardShortcut::ctrl('v'.into())).await
    }

    /// Cut (Ctrl+X / Cmd+X).
    async fn cut(&self) -> InputResult<()> {
        self.shortcut(&KeyboardShortcut::ctrl('x'.into())).await
    }

    /// Select all (Ctrl+A / Cmd+A).
    async fn select_all(&self) -> InputResult<()> {
        self.shortcut(&KeyboardShortcut::ctrl('a'.into())).await
    }

    /// Undo (Ctrl+Z / Cmd+Z).
    async fn undo(&self) -> InputResult<()> {
        self.shortcut(&KeyboardShortcut::ctrl('z'.into())).await
    }

    /// Redo (Ctrl+Shift+Z / Cmd+Shift+Z).
    async fn redo(&self) -> InputResult<()> {
        self.shortcut(&KeyboardShortcut::ctrl_shift('z'.into()))
            .await
    }

    /// Save (Ctrl+S / Cmd+S).
    async fn save(&self) -> InputResult<()> {
        self.shortcut(&KeyboardShortcut::ctrl('s'.into())).await
    }
}

// ============================================================================
// Platform Implementation (Enigo)
// ============================================================================

/// Platform-specific input simulation using enigo.
#[cfg(feature = "gui-automation")]
pub mod platform {
    use super::*;
    use enigo::{
        Button, Coordinate, Direction, Enigo, Key as EnigoKey, Keyboard, Mouse, Settings,
    };
    use std::sync::Mutex as StdMutex;

    /// Enigo-based input simulator.
    pub struct EnigoSimulator {
        enigo: StdMutex<Enigo>,
        timing: TimingConfig,
    }

    impl EnigoSimulator {
        /// Create a new Enigo-based simulator.
        pub fn new() -> InputResult<Self> {
            let settings = Settings::default();
            let enigo =
                Enigo::new(&settings).map_err(|e| InputError::SimulationFailed(e.to_string()))?;

            Ok(Self {
                enigo: StdMutex::new(enigo),
                timing: TimingConfig::default(),
            })
        }

        /// Create with a specific speed mode.
        pub fn with_speed(speed_mode: SpeedMode) -> InputResult<Self> {
            let mut sim = Self::new()?;
            sim.timing = TimingConfig::with_speed(speed_mode);
            Ok(sim)
        }

        /// Create with custom timing configuration.
        pub fn with_timing(timing: TimingConfig) -> InputResult<Self> {
            let mut sim = Self::new()?;
            sim.timing = timing;
            Ok(sim)
        }

        /// Set the timing configuration.
        pub fn set_timing(&mut self, timing: TimingConfig) {
            self.timing = timing;
        }

        /// Set the speed mode.
        pub fn set_speed_mode(&mut self, mode: SpeedMode) {
            self.timing.speed_mode = mode;
        }

        /// Get the current timing configuration.
        pub fn timing(&self) -> &TimingConfig {
            &self.timing
        }

        fn convert_key(key: &KeyInput) -> Result<EnigoKey, InputError> {
            match key {
                KeyInput::Char(c) => Ok(EnigoKey::Unicode(*c)),
                KeyInput::Special(k) => match k {
                    Key::F1 => Ok(EnigoKey::F1),
                    Key::F2 => Ok(EnigoKey::F2),
                    Key::F3 => Ok(EnigoKey::F3),
                    Key::F4 => Ok(EnigoKey::F4),
                    Key::F5 => Ok(EnigoKey::F5),
                    Key::F6 => Ok(EnigoKey::F6),
                    Key::F7 => Ok(EnigoKey::F7),
                    Key::F8 => Ok(EnigoKey::F8),
                    Key::F9 => Ok(EnigoKey::F9),
                    Key::F10 => Ok(EnigoKey::F10),
                    Key::F11 => Ok(EnigoKey::F11),
                    Key::F12 => Ok(EnigoKey::F12),
                    Key::Escape => Ok(EnigoKey::Escape),
                    Key::Tab => Ok(EnigoKey::Tab),
                    Key::CapsLock => Ok(EnigoKey::CapsLock),
                    Key::Backspace => Ok(EnigoKey::Backspace),
                    Key::Enter => Ok(EnigoKey::Return),
                    Key::Space => Ok(EnigoKey::Space),
                    Key::Delete => Ok(EnigoKey::Delete),
                    Key::Home => Ok(EnigoKey::Home),
                    Key::End => Ok(EnigoKey::End),
                    Key::PageUp => Ok(EnigoKey::PageUp),
                    Key::PageDown => Ok(EnigoKey::PageDown),
                    Key::Up => Ok(EnigoKey::UpArrow),
                    Key::Down => Ok(EnigoKey::DownArrow),
                    Key::Left => Ok(EnigoKey::LeftArrow),
                    Key::Right => Ok(EnigoKey::RightArrow),
                    Key::Shift => Ok(EnigoKey::Shift),
                    Key::Control => Ok(EnigoKey::Control),
                    Key::Alt => Ok(EnigoKey::Alt),
                    Key::Meta | Key::Super => Ok(EnigoKey::Meta),
                    _ => Err(InputError::InvalidKey(format!("Unsupported key: {:?}", k))),
                },
            }
        }

        fn convert_button(button: MouseButton) -> Button {
            match button {
                MouseButton::Left => Button::Left,
                MouseButton::Right => Button::Right,
                MouseButton::Middle => Button::Middle,
                MouseButton::Back => Button::Back,
                MouseButton::Forward => Button::Forward,
            }
        }

        fn convert_modifier(modifier: Modifier) -> EnigoKey {
            match modifier {
                Modifier::Shift => EnigoKey::Shift,
                Modifier::Control => EnigoKey::Control,
                Modifier::Alt => EnigoKey::Alt,
                Modifier::Meta | Modifier::Super => EnigoKey::Meta,
            }
        }
    }

    impl Default for EnigoSimulator {
        fn default() -> Self {
            Self::new().expect("Failed to create EnigoSimulator")
        }
    }

    #[async_trait]
    impl InputSimulator for EnigoSimulator {
        fn is_available(&self) -> bool {
            true
        }

        async fn mouse_position(&self) -> InputResult<(i32, i32)> {
            let enigo = self.enigo.lock().map_err(|e| {
                InputError::SimulationFailed(format!("Failed to lock enigo: {}", e))
            })?;

            let (x, y) = enigo
                .location()
                .map_err(|e| InputError::SimulationFailed(e.to_string()))?;

            Ok((x, y))
        }

        async fn mouse_move(&self, x: i32, y: i32) -> InputResult<()> {
            {
                let mut enigo = self.enigo.lock().map_err(|e| {
                    InputError::SimulationFailed(format!("Failed to lock enigo: {}", e))
                })?;

                enigo
                    .move_mouse(x, y, Coordinate::Abs)
                    .map_err(|e| InputError::SimulationFailed(e.to_string()))?;
            }

            tokio::time::sleep(self.timing.min_action_delay).await;
            Ok(())
        }

        async fn mouse_move_smooth(&self, x: i32, y: i32, duration: Duration) -> InputResult<()> {
            smooth_move(self, x, y, duration, EasingCurve::EaseInOut).await
        }

        async fn mouse_click(&self, action: &MouseAction) -> InputResult<()> {
            // Move to position first
            self.mouse_move(action.x, action.y).await?;

            {
                let mut enigo = self.enigo.lock().map_err(|e| {
                    InputError::SimulationFailed(format!("Failed to lock enigo: {}", e))
                })?;

                let button = Self::convert_button(action.button);

                // Press modifiers
                for modifier in &action.modifiers {
                    let key = Self::convert_modifier(*modifier);
                    enigo
                        .key(key, Direction::Press)
                        .map_err(|e| InputError::SimulationFailed(e.to_string()))?;
                }

                // Perform click
                match action.click_type {
                    ClickType::Single => {
                        enigo
                            .button(button, Direction::Click)
                            .map_err(|e| InputError::SimulationFailed(e.to_string()))?;
                    }
                    ClickType::Double => {
                        enigo
                            .button(button, Direction::Click)
                            .map_err(|e| InputError::SimulationFailed(e.to_string()))?;
                        enigo
                            .button(button, Direction::Click)
                            .map_err(|e| InputError::SimulationFailed(e.to_string()))?;
                    }
                    ClickType::Triple => {
                        for _ in 0..3 {
                            enigo
                                .button(button, Direction::Click)
                                .map_err(|e| InputError::SimulationFailed(e.to_string()))?;
                        }
                    }
                    ClickType::Press => {
                        enigo
                            .button(button, Direction::Press)
                            .map_err(|e| InputError::SimulationFailed(e.to_string()))?;
                    }
                    ClickType::Release => {
                        enigo
                            .button(button, Direction::Release)
                            .map_err(|e| InputError::SimulationFailed(e.to_string()))?;
                    }
                }

                // Release modifiers
                for modifier in action.modifiers.iter().rev() {
                    let key = Self::convert_modifier(*modifier);
                    enigo
                        .key(key, Direction::Release)
                        .map_err(|e| InputError::SimulationFailed(e.to_string()))?;
                }
            }

            tokio::time::sleep(self.timing.action_delay()).await;
            Ok(())
        }

        async fn mouse_drag(&self, drag: &DragOperation) -> InputResult<()> {
            // Press modifiers
            {
                let mut enigo = self.enigo.lock().map_err(|e| {
                    InputError::SimulationFailed(format!("Failed to lock enigo: {}", e))
                })?;
                for modifier in &drag.modifiers {
                    let key = Self::convert_modifier(*modifier);
                    enigo
                        .key(key, Direction::Press)
                        .map_err(|e| InputError::SimulationFailed(e.to_string()))?;
                }
            }

            // Move to start position and press button
            self.mouse_move(drag.start_x, drag.start_y).await?;
            {
                let mut enigo = self.enigo.lock().map_err(|e| {
                    InputError::SimulationFailed(format!("Failed to lock enigo: {}", e))
                })?;
                let button = Self::convert_button(drag.button);
                enigo
                    .button(button, Direction::Press)
                    .map_err(|e| InputError::SimulationFailed(e.to_string()))?;
            }

            // Move smoothly to end position with easing
            smooth_move(self, drag.end_x, drag.end_y, drag.duration, drag.easing).await?;

            // Release button and modifiers
            {
                let mut enigo = self.enigo.lock().map_err(|e| {
                    InputError::SimulationFailed(format!("Failed to lock enigo: {}", e))
                })?;
                let button = Self::convert_button(drag.button);
                enigo
                    .button(button, Direction::Release)
                    .map_err(|e| InputError::SimulationFailed(e.to_string()))?;

                for modifier in drag.modifiers.iter().rev() {
                    let key = Self::convert_modifier(*modifier);
                    enigo
                        .key(key, Direction::Release)
                        .map_err(|e| InputError::SimulationFailed(e.to_string()))?;
                }
            }

            tokio::time::sleep(self.timing.action_delay()).await;
            Ok(())
        }

        async fn mouse_scroll(&self, scroll: &ScrollAction) -> InputResult<()> {
            self.mouse_move(scroll.x, scroll.y).await?;

            {
                let mut enigo = self.enigo.lock().map_err(|e| {
                    InputError::SimulationFailed(format!("Failed to lock enigo: {}", e))
                })?;

                if scroll.delta_y != 0 {
                    enigo
                        .scroll(scroll.delta_y, enigo::Axis::Vertical)
                        .map_err(|e| InputError::SimulationFailed(e.to_string()))?;
                }

                if scroll.delta_x != 0 {
                    enigo
                        .scroll(scroll.delta_x, enigo::Axis::Horizontal)
                        .map_err(|e| InputError::SimulationFailed(e.to_string()))?;
                }
            }

            tokio::time::sleep(self.timing.action_delay()).await;
            Ok(())
        }

        async fn type_text(&self, text: &str) -> InputResult<()> {
            {
                let mut enigo = self.enigo.lock().map_err(|e| {
                    InputError::SimulationFailed(format!("Failed to lock enigo: {}", e))
                })?;

                enigo
                    .text(text)
                    .map_err(|e| InputError::SimulationFailed(e.to_string()))?;
            }

            tokio::time::sleep(self.timing.action_delay()).await;
            Ok(())
        }

        async fn key_press(&self, key: KeyInput) -> InputResult<()> {
            {
                let mut enigo = self.enigo.lock().map_err(|e| {
                    InputError::SimulationFailed(format!("Failed to lock enigo: {}", e))
                })?;

                let enigo_key = Self::convert_key(&key)?;
                enigo
                    .key(enigo_key, Direction::Click)
                    .map_err(|e| InputError::SimulationFailed(e.to_string()))?;
            }

            tokio::time::sleep(self.timing.action_delay()).await;
            Ok(())
        }

        async fn key_down(&self, key: KeyInput) -> InputResult<()> {
            {
                let mut enigo = self.enigo.lock().map_err(|e| {
                    InputError::SimulationFailed(format!("Failed to lock enigo: {}", e))
                })?;

                let enigo_key = Self::convert_key(&key)?;
                enigo
                    .key(enigo_key, Direction::Press)
                    .map_err(|e| InputError::SimulationFailed(e.to_string()))?;
            }

            tokio::time::sleep(self.timing.min_action_delay).await;
            Ok(())
        }

        async fn key_up(&self, key: KeyInput) -> InputResult<()> {
            {
                let mut enigo = self.enigo.lock().map_err(|e| {
                    InputError::SimulationFailed(format!("Failed to lock enigo: {}", e))
                })?;

                let enigo_key = Self::convert_key(&key)?;
                enigo
                    .key(enigo_key, Direction::Release)
                    .map_err(|e| InputError::SimulationFailed(e.to_string()))?;
            }

            tokio::time::sleep(self.timing.min_action_delay).await;
            Ok(())
        }

        async fn shortcut(&self, shortcut: &KeyboardShortcut) -> InputResult<()> {
            {
                let mut enigo = self.enigo.lock().map_err(|e| {
                    InputError::SimulationFailed(format!("Failed to lock enigo: {}", e))
                })?;

                // Press modifiers
                for modifier in &shortcut.modifiers {
                    let key = Self::convert_modifier(*modifier);
                    enigo
                        .key(key, Direction::Press)
                        .map_err(|e| InputError::SimulationFailed(e.to_string()))?;
                }

                // Press the main key
                let main_key = Self::convert_key(&shortcut.key)?;
                enigo
                    .key(main_key, Direction::Click)
                    .map_err(|e| InputError::SimulationFailed(e.to_string()))?;

                // Release modifiers in reverse order
                for modifier in shortcut.modifiers.iter().rev() {
                    let key = Self::convert_modifier(*modifier);
                    enigo
                        .key(key, Direction::Release)
                        .map_err(|e| InputError::SimulationFailed(e.to_string()))?;
                }
            }

            tokio::time::sleep(self.timing.action_delay()).await;
            Ok(())
        }
    }
}

/// Create the default input simulator for the current platform.
#[cfg(feature = "gui-automation")]
pub fn create_input_simulator() -> InputResult<impl InputSimulator> {
    platform::EnigoSimulator::new()
}

/// Create an input simulator with a specific speed mode.
#[cfg(feature = "gui-automation")]
pub fn create_input_simulator_with_speed(speed_mode: SpeedMode) -> InputResult<impl InputSimulator> {
    platform::EnigoSimulator::with_speed(speed_mode)
}

// ============================================================================
// Mock Implementation
// ============================================================================

/// Mock input simulator for testing.
/// Always available for tests, even when gui-automation is enabled.
pub mod mock {
    use super::*;
    use std::collections::VecDeque;

    /// Mock input simulator that tracks calls but doesn't actually simulate input.
    pub struct MockSimulator {
        /// Simulated mouse position
        position: Arc<RwLock<(i32, i32)>>,
        /// Action log for verification
        actions: Arc<Mutex<VecDeque<String>>>,
        /// Maximum actions to keep in log
        max_log_size: usize,
    }

    impl MockSimulator {
        /// Create a new mock simulator.
        pub fn new() -> Self {
            Self {
                position: Arc::new(RwLock::new((0, 0))),
                actions: Arc::new(Mutex::new(VecDeque::new())),
                max_log_size: 1000,
            }
        }

        /// Get the action log.
        pub async fn actions(&self) -> Vec<String> {
            self.actions.lock().await.iter().cloned().collect()
        }

        /// Clear the action log.
        pub async fn clear_log(&self) {
            self.actions.lock().await.clear();
        }

        async fn log(&self, action: &str) {
            let mut actions = self.actions.lock().await;
            if actions.len() >= self.max_log_size {
                actions.pop_front();
            }
            actions.push_back(action.to_string());
        }
    }

    impl Default for MockSimulator {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl InputSimulator for MockSimulator {
        fn is_available(&self) -> bool {
            true // Mock is always available
        }

        async fn mouse_position(&self) -> InputResult<(i32, i32)> {
            Ok(*self.position.read().await)
        }

        async fn mouse_move(&self, x: i32, y: i32) -> InputResult<()> {
            *self.position.write().await = (x, y);
            self.log(&format!("mouse_move({}, {})", x, y)).await;
            Ok(())
        }

        async fn mouse_move_smooth(&self, x: i32, y: i32, duration: Duration) -> InputResult<()> {
            *self.position.write().await = (x, y);
            self.log(&format!(
                "mouse_move_smooth({}, {}, {:?})",
                x, y, duration
            ))
            .await;
            Ok(())
        }

        async fn mouse_click(&self, action: &MouseAction) -> InputResult<()> {
            *self.position.write().await = (action.x, action.y);
            self.log(&format!("mouse_click({:?})", action)).await;
            Ok(())
        }

        async fn mouse_drag(&self, drag: &DragOperation) -> InputResult<()> {
            *self.position.write().await = (drag.end_x, drag.end_y);
            self.log(&format!("mouse_drag({:?})", drag)).await;
            Ok(())
        }

        async fn mouse_scroll(&self, scroll: &ScrollAction) -> InputResult<()> {
            self.log(&format!("mouse_scroll({:?})", scroll)).await;
            Ok(())
        }

        async fn type_text(&self, text: &str) -> InputResult<()> {
            self.log(&format!("type_text(\"{}\")", text)).await;
            Ok(())
        }

        async fn key_press(&self, key: KeyInput) -> InputResult<()> {
            self.log(&format!("key_press({:?})", key)).await;
            Ok(())
        }

        async fn key_down(&self, key: KeyInput) -> InputResult<()> {
            self.log(&format!("key_down({:?})", key)).await;
            Ok(())
        }

        async fn key_up(&self, key: KeyInput) -> InputResult<()> {
            self.log(&format!("key_up({:?})", key)).await;
            Ok(())
        }

        async fn shortcut(&self, shortcut: &KeyboardShortcut) -> InputResult<()> {
            self.log(&format!("shortcut({:?})", shortcut)).await;
            Ok(())
        }
    }
}

#[cfg(not(feature = "gui-automation"))]
pub fn create_input_simulator() -> InputResult<impl InputSimulator> {
    Ok(mock::MockSimulator::new())
}

#[cfg(not(feature = "gui-automation"))]
pub fn create_input_simulator_with_speed(_speed_mode: SpeedMode) -> InputResult<impl InputSimulator> {
    Ok(mock::MockSimulator::new())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyboard_shortcut_parse() {
        let shortcut = KeyboardShortcut::parse("Ctrl+S").unwrap();
        assert_eq!(shortcut.modifiers, vec![Modifier::Control]);

        let shortcut = KeyboardShortcut::parse("Ctrl+Shift+Z").unwrap();
        assert!(shortcut.modifiers.contains(&Modifier::Control));
        assert!(shortcut.modifiers.contains(&Modifier::Shift));
    }

    #[test]
    fn test_mouse_action() {
        let action = MouseAction::click(100, 200);
        assert_eq!(action.x, 100);
        assert_eq!(action.y, 200);
        assert_eq!(action.button, MouseButton::Left);
        assert_eq!(action.click_type, ClickType::Single);
    }

    #[test]
    fn test_drag_operation() {
        let drag = DragOperation::new(0, 0, 100, 100);
        assert_eq!(drag.start_x, 0);
        assert_eq!(drag.end_x, 100);
    }

    #[test]
    fn test_easing_curves() {
        // Linear should be identity
        assert!((EasingCurve::Linear.apply(0.5) - 0.5).abs() < 0.001);

        // All curves should map 0 -> 0 and 1 -> 1
        for curve in [
            EasingCurve::Linear,
            EasingCurve::EaseIn,
            EasingCurve::EaseOut,
            EasingCurve::EaseInOut,
            EasingCurve::EaseInCubic,
            EasingCurve::EaseOutCubic,
            EasingCurve::EaseInOutCubic,
        ] {
            assert!((curve.apply(0.0) - 0.0).abs() < 0.001, "{:?} at 0", curve);
            assert!((curve.apply(1.0) - 1.0).abs() < 0.001, "{:?} at 1", curve);
        }

        // Ease out should be faster at the start
        assert!(EasingCurve::EaseOut.apply(0.5) > 0.5);

        // Ease in should be slower at the start
        assert!(EasingCurve::EaseIn.apply(0.5) < 0.5);
    }

    #[test]
    fn test_speed_modes() {
        assert!(SpeedMode::Slow.action_delay() > SpeedMode::Normal.action_delay());
        assert!(SpeedMode::Normal.action_delay() > SpeedMode::Fast.action_delay());
        assert!(SpeedMode::Fast.action_delay() > SpeedMode::Beast.action_delay());
    }

    #[test]
    fn test_timing_config() {
        let config = TimingConfig::with_speed(SpeedMode::Slow);
        assert_eq!(config.speed_mode, SpeedMode::Slow);
        assert_eq!(config.action_delay(), SpeedMode::Slow.action_delay());

        let config = config.with_action_delay(Duration::from_secs(1));
        assert_eq!(config.action_delay(), Duration::from_secs(1));
    }

    #[test]
    fn test_input_recording_serialization() {
        let mut recording = InputRecording::new("Test Recording");
        recording.add_event(RecordedEvent {
            timestamp_ms: 0,
            action: RecordedAction::Mouse(MouseActionType::Click {
                x: Some(100),
                y: Some(200),
                button: MouseButton::Left,
            }),
            annotation: None,
        });
        recording.add_event(RecordedEvent {
            timestamp_ms: 100,
            action: RecordedAction::Keyboard(KeyActionType::Type {
                text: "Hello".to_string(),
            }),
            annotation: Some("Type greeting".to_string()),
        });

        let json = serde_json::to_string(&recording).unwrap();
        let deserialized: InputRecording = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.events.len(), 2);
        assert_eq!(deserialized.metadata.duration_ms, 100);
    }

    #[test]
    fn test_easing_interpolation() {
        let curve = EasingCurve::Linear;
        assert_eq!(curve.interpolate_i32(0, 100, 0.0), 0);
        assert_eq!(curve.interpolate_i32(0, 100, 0.5), 50);
        assert_eq!(curve.interpolate_i32(0, 100, 1.0), 100);
    }

    #[tokio::test]
    async fn test_mock_simulator() {
        let sim = mock::MockSimulator::new();

        sim.mouse_move(100, 200).await.unwrap();
        assert_eq!(sim.mouse_position().await.unwrap(), (100, 200));

        sim.type_text("hello").await.unwrap();

        let actions = sim.actions().await;
        assert!(actions.iter().any(|a| a.contains("mouse_move")));
        assert!(actions.iter().any(|a| a.contains("type_text")));
    }
}
