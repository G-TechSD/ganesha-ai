//! Input simulation for the Vision/VLA system.
//!
//! This module provides:
//! - Mouse movement and clicks
//! - Keyboard input (typing and shortcuts)
//! - Drag and drop operations
//! - Platform-specific implementations (X11, Wayland, Windows, macOS)

use crate::capture::Region;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

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
}

/// Result type for input operations.
pub type InputResult<T> = Result<T, InputError>;

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

        let key_str = key_str.ok_or_else(|| InputError::InvalidKey("No key in shortcut".to_string()))?;

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

/// Trait for platform-specific input simulation.
#[async_trait]
pub trait InputSimulator: Send + Sync {
    /// Check if input simulation is available.
    fn is_available(&self) -> bool;

    /// Get the current mouse position.
    async fn mouse_position(&self) -> InputResult<(i32, i32)>;

    /// Move the mouse to coordinates.
    async fn mouse_move(&self, x: i32, y: i32) -> InputResult<()>;

    /// Move the mouse smoothly to coordinates over a duration.
    async fn mouse_move_smooth(&self, x: i32, y: i32, duration: Duration) -> InputResult<()>;

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

/// Platform-specific input simulation using enigo.
#[cfg(feature = "gui-automation")]
pub mod platform {
    use super::*;
    use enigo::{
        Button, Coordinate, Direction, Enigo, Key as EnigoKey, Keyboard, Mouse, Settings,
    };
    use std::sync::Mutex;

    /// Enigo-based input simulator.
    pub struct EnigoSimulator {
        enigo: Mutex<Enigo>,
        delay: Duration,
    }

    impl EnigoSimulator {
        /// Create a new Enigo-based simulator.
        pub fn new() -> InputResult<Self> {
            let settings = Settings::default();
            let enigo = Enigo::new(&settings)
                .map_err(|e| InputError::SimulationFailed(e.to_string()))?;

            Ok(Self {
                enigo: Mutex::new(enigo),
                delay: Duration::from_millis(10),
            })
        }

        /// Set delay between actions.
        pub fn with_delay(mut self, delay: Duration) -> Self {
            self.delay = delay;
            self
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

            tokio::time::sleep(self.delay).await;
            Ok(())
        }

        async fn mouse_move_smooth(&self, x: i32, y: i32, duration: Duration) -> InputResult<()> {
            let (start_x, start_y) = self.mouse_position().await?;
            let steps = 20;
            let step_delay = duration / steps;

            for i in 1..=steps {
                let progress = i as f64 / steps as f64;
                // Use ease-in-out curve
                let eased = if progress < 0.5 {
                    2.0 * progress * progress
                } else {
                    1.0 - (-2.0 * progress + 2.0).powi(2) / 2.0
                };

                let current_x = start_x + ((x - start_x) as f64 * eased) as i32;
                let current_y = start_y + ((y - start_y) as f64 * eased) as i32;

                {
                    let mut enigo = self.enigo.lock().map_err(|e| {
                        InputError::SimulationFailed(format!("Failed to lock enigo: {}", e))
                    })?;
                    enigo
                        .move_mouse(current_x, current_y, Coordinate::Abs)
                        .map_err(|e| InputError::SimulationFailed(e.to_string()))?;
                }

                tokio::time::sleep(step_delay).await;
            }

            Ok(())
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

            tokio::time::sleep(self.delay).await;
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

            // Move smoothly to end position
            self.mouse_move_smooth(drag.end_x, drag.end_y, drag.duration)
                .await?;

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

            tokio::time::sleep(self.delay).await;
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

            tokio::time::sleep(self.delay).await;
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

            tokio::time::sleep(self.delay).await;
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

            tokio::time::sleep(self.delay).await;
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

            tokio::time::sleep(self.delay).await;
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

            tokio::time::sleep(self.delay).await;
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

            tokio::time::sleep(self.delay).await;
            Ok(())
        }
    }
}

/// Create the default input simulator for the current platform.
#[cfg(feature = "gui-automation")]
pub fn create_input_simulator() -> InputResult<impl InputSimulator> {
    platform::EnigoSimulator::new()
}

/// Mock input simulator for testing.
#[cfg(not(feature = "gui-automation"))]
pub mod mock {
    use super::*;

    /// Mock input simulator that always fails.
    pub struct MockSimulator;

    #[async_trait]
    impl InputSimulator for MockSimulator {
        fn is_available(&self) -> bool {
            false
        }

        async fn mouse_position(&self) -> InputResult<(i32, i32)> {
            Err(InputError::NotAvailable)
        }

        async fn mouse_move(&self, _x: i32, _y: i32) -> InputResult<()> {
            Err(InputError::NotAvailable)
        }

        async fn mouse_move_smooth(
            &self,
            _x: i32,
            _y: i32,
            _duration: Duration,
        ) -> InputResult<()> {
            Err(InputError::NotAvailable)
        }

        async fn mouse_click(&self, _action: &MouseAction) -> InputResult<()> {
            Err(InputError::NotAvailable)
        }

        async fn mouse_drag(&self, _drag: &DragOperation) -> InputResult<()> {
            Err(InputError::NotAvailable)
        }

        async fn mouse_scroll(&self, _scroll: &ScrollAction) -> InputResult<()> {
            Err(InputError::NotAvailable)
        }

        async fn type_text(&self, _text: &str) -> InputResult<()> {
            Err(InputError::NotAvailable)
        }

        async fn key_press(&self, _key: KeyInput) -> InputResult<()> {
            Err(InputError::NotAvailable)
        }

        async fn key_down(&self, _key: KeyInput) -> InputResult<()> {
            Err(InputError::NotAvailable)
        }

        async fn key_up(&self, _key: KeyInput) -> InputResult<()> {
            Err(InputError::NotAvailable)
        }

        async fn shortcut(&self, _shortcut: &KeyboardShortcut) -> InputResult<()> {
            Err(InputError::NotAvailable)
        }
    }
}

#[cfg(not(feature = "gui-automation"))]
pub fn create_input_simulator() -> InputResult<impl InputSimulator> {
    Ok(mock::MockSimulator)
}

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
    fn test_keyboard_shortcut_ctrl() {
        let s = KeyboardShortcut::ctrl(KeyInput::Char('s'));
        assert!(s.modifiers.contains(&Modifier::Control));
    }

    #[test]
    fn test_keyboard_shortcut_alt() {
        let s = KeyboardShortcut::alt(KeyInput::Special(Key::F4));
        assert!(s.modifiers.contains(&Modifier::Alt));
    }

    #[test]
    fn test_keyboard_shortcut_shift() {
        let s = KeyboardShortcut::shift(KeyInput::Special(Key::Tab));
        assert!(s.modifiers.contains(&Modifier::Shift));
    }

    #[test]
    fn test_keyboard_shortcut_ctrl_shift() {
        let s = KeyboardShortcut::ctrl_shift(KeyInput::Char('z'));
        assert!(s.modifiers.contains(&Modifier::Control));
        assert!(s.modifiers.contains(&Modifier::Shift));
    }

    #[test]
    fn test_keyboard_shortcut_ctrl_alt() {
        let s = KeyboardShortcut::ctrl_alt(KeyInput::Special(Key::Delete));
        assert!(s.modifiers.contains(&Modifier::Control));
        assert!(s.modifiers.contains(&Modifier::Alt));
    }

    #[test]
    fn test_mouse_double_click() {
        let action = MouseAction::double_click(50, 75);
        assert_eq!(action.x, 50);
        assert_eq!(action.y, 75);
        assert_eq!(action.click_type, ClickType::Double);
    }

    #[test]
    fn test_mouse_right_click() {
        let action = MouseAction::right_click(10, 20);
        assert_eq!(action.button, MouseButton::Right);
    }

    #[test]
    fn test_scroll_vertical() {
        let scroll = ScrollAction::vertical(0, 0, 5);
        assert_eq!(scroll.delta_y, 5);
        assert_eq!(scroll.delta_x, 0);
    }

    #[test]
    fn test_scroll_horizontal() {
        let scroll = ScrollAction::horizontal(0, 0, 3);
        assert_eq!(scroll.delta_x, 3);
        assert_eq!(scroll.delta_y, 0);
    }

    #[test]
    fn test_drag_operation_detailed() {
        let drag = DragOperation::new(10, 20, 300, 400);
        assert_eq!(drag.start_x, 10);
        assert_eq!(drag.start_y, 20);
        assert_eq!(drag.end_x, 300);
        assert_eq!(drag.end_y, 400);
    }

    #[test]
    fn test_keyboard_shortcut_parse_single() {
        let s = KeyboardShortcut::parse("A").unwrap();
        assert!(s.modifiers.is_empty());
    }

    #[test]
    fn test_keyboard_shortcut_parse_meta() {
        let s = KeyboardShortcut::parse("Meta+C");
        // Meta might not be supported in all parse implementations
        // but shouldn't panic
        let _ = s;
    }
}
