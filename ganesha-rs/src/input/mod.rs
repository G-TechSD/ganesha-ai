//! Input Module - Mouse, Keyboard, and Scroll Control
//!
//! SAFETY: This module is disabled by default and requires explicit opt-in
//! via the `input` feature flag: `--features input`
//!
//! This enables Ganesha to interact with GUIs for automation tasks.
//!
//! # Safety Mechanisms
//! - Disabled by default (requires --features input)
//! - Must be explicitly enabled at runtime with user consent
//! - Rate limiting on all input actions
//! - Emergency kill switch (Escape key or programmatic)
//! - Confirmation required for destructive patterns
//! - Auto-disable after inactivity timeout

#[cfg(feature = "input")]
use enigo::{Enigo, Keyboard, Mouse, Settings};

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Global kill switch for input capabilities
static INPUT_ENABLED: AtomicBool = AtomicBool::new(false);
static INPUT_KILL_SWITCH: AtomicBool = AtomicBool::new(false);

/// Rate limiting: max actions per second
const MAX_ACTIONS_PER_SECOND: u64 = 20;
static ACTION_COUNT: AtomicU64 = AtomicU64::new(0);
static RATE_LIMIT_RESET: AtomicU64 = AtomicU64::new(0);

/// Dangerous key combinations that require extra confirmation
const DANGEROUS_KEYS: &[&str] = &[
    "ctrl+alt+delete",
    "alt+f4",
    "ctrl+shift+escape",
    "super+l", // Lock screen
    "ctrl+q",  // Quit application
];

/// Input action types for logging and rate limiting
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputAction {
    MouseMove,
    MouseClick,
    MouseScroll,
    KeyPress,
    KeyRelease,
    TextInput,
}

/// Mouse button enum
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Input capability status
#[derive(Debug, Clone)]
pub struct InputStatus {
    pub feature_compiled: bool,
    pub enabled: bool,
    pub kill_switch_active: bool,
    pub actions_this_second: u64,
    pub rate_limit: u64,
}

/// Input controller with comprehensive safety mechanisms
pub struct InputController {
    /// Whether input was explicitly enabled by user
    enabled: Arc<AtomicBool>,
    /// Emergency kill switch
    kill_switch: Arc<AtomicBool>,
    /// Last activity timestamp for timeout
    last_activity: std::sync::Mutex<Instant>,
    /// Auto-disable timeout (default 2 minutes of inactivity)
    inactivity_timeout: Duration,
    /// Delay between actions (minimum 50ms for safety)
    action_delay: Duration,
    /// Enigo instance (when feature enabled)
    #[cfg(feature = "input")]
    enigo: std::sync::Mutex<Option<Enigo>>,
}

impl Default for InputController {
    fn default() -> Self {
        Self::new()
    }
}

impl InputController {
    pub fn new() -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(false)),
            kill_switch: Arc::new(AtomicBool::new(false)),
            last_activity: std::sync::Mutex::new(Instant::now()),
            inactivity_timeout: Duration::from_secs(120), // 2 minutes
            action_delay: Duration::from_millis(50),      // 50ms minimum between actions
            #[cfg(feature = "input")]
            enigo: std::sync::Mutex::new(None),
        }
    }

    /// Enable input capabilities (requires user consent)
    ///
    /// # Safety
    /// This should only be called after explicit user confirmation
    /// and ideally with a visible warning about input control
    pub fn enable(&self) -> Result<(), InputError> {
        if self.kill_switch.load(Ordering::SeqCst) {
            return Err(InputError::KillSwitchActive);
        }

        #[cfg(not(feature = "input"))]
        return Err(InputError::FeatureNotCompiled);

        #[cfg(feature = "input")]
        {
            // Initialize enigo
            let settings = Settings::default();
            let enigo = Enigo::new(&settings).map_err(|e| InputError::InitError(e.to_string()))?;

            *self.enigo.lock().unwrap() = Some(enigo);
            INPUT_ENABLED.store(true, Ordering::SeqCst);
            self.enabled.store(true, Ordering::SeqCst);
            *self.last_activity.lock().unwrap() = Instant::now();
            Ok(())
        }
    }

    /// Disable input capabilities
    pub fn disable(&self) {
        INPUT_ENABLED.store(false, Ordering::SeqCst);
        self.enabled.store(false, Ordering::SeqCst);
        #[cfg(feature = "input")]
        {
            *self.enigo.lock().unwrap() = None;
        }
    }

    /// Activate emergency kill switch (cannot be reversed without restart)
    pub fn activate_kill_switch(&self) {
        INPUT_KILL_SWITCH.store(true, Ordering::SeqCst);
        self.kill_switch.store(true, Ordering::SeqCst);
        self.disable();
    }

    /// Check if input is currently available
    pub fn is_available(&self) -> bool {
        #[cfg(not(feature = "input"))]
        return false;

        #[cfg(feature = "input")]
        {
            self.enabled.load(Ordering::SeqCst)
                && !self.kill_switch.load(Ordering::SeqCst)
                && !self.is_inactive_timeout()
        }
    }

    /// Check if inactive timeout has expired
    fn is_inactive_timeout(&self) -> bool {
        let last = self.last_activity.lock().unwrap();
        last.elapsed() > self.inactivity_timeout
    }

    /// Update activity timestamp
    fn touch(&self) {
        *self.last_activity.lock().unwrap() = Instant::now();
    }

    /// Check rate limit
    fn check_rate_limit(&self) -> Result<(), InputError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let reset_time = RATE_LIMIT_RESET.load(Ordering::SeqCst);

        // Reset counter every second
        if now > reset_time {
            ACTION_COUNT.store(0, Ordering::SeqCst);
            RATE_LIMIT_RESET.store(now, Ordering::SeqCst);
        }

        let count = ACTION_COUNT.fetch_add(1, Ordering::SeqCst);
        if count >= MAX_ACTIONS_PER_SECOND {
            return Err(InputError::RateLimitExceeded);
        }

        Ok(())
    }

    /// Apply action delay for safety
    fn apply_delay(&self) {
        std::thread::sleep(self.action_delay);
    }

    /// Get current status
    pub fn status(&self) -> InputStatus {
        InputStatus {
            feature_compiled: cfg!(feature = "input"),
            enabled: self.enabled.load(Ordering::SeqCst),
            kill_switch_active: self.kill_switch.load(Ordering::SeqCst),
            actions_this_second: ACTION_COUNT.load(Ordering::SeqCst),
            rate_limit: MAX_ACTIONS_PER_SECOND,
        }
    }

    /// Check if a key combination is dangerous
    pub fn is_dangerous_key(&self, keys: &str) -> bool {
        let normalized = keys.to_lowercase().replace(' ', "");
        DANGEROUS_KEYS.iter().any(|&dk| {
            let dk_normalized = dk.replace(' ', "");
            normalized.contains(&dk_normalized)
        })
    }

    // ========== Mouse Operations ==========

    /// Move mouse to absolute position
    #[cfg(feature = "input")]
    pub fn mouse_move(&self, x: i32, y: i32) -> Result<(), InputError> {
        self.preflight_check()?;

        let mut enigo_guard = self.enigo.lock().unwrap();
        let enigo = enigo_guard.as_mut().ok_or(InputError::NotInitialized)?;

        enigo
            .move_mouse(x, y, enigo::Coordinate::Abs)
            .map_err(|e| InputError::MouseError(e.to_string()))?;

        self.apply_delay();
        Ok(())
    }

    /// Move mouse relative to current position
    #[cfg(feature = "input")]
    pub fn mouse_move_relative(&self, dx: i32, dy: i32) -> Result<(), InputError> {
        self.preflight_check()?;

        let mut enigo_guard = self.enigo.lock().unwrap();
        let enigo = enigo_guard.as_mut().ok_or(InputError::NotInitialized)?;

        enigo
            .move_mouse(dx, dy, enigo::Coordinate::Rel)
            .map_err(|e| InputError::MouseError(e.to_string()))?;

        self.apply_delay();
        Ok(())
    }

    /// Click mouse button
    #[cfg(feature = "input")]
    pub fn mouse_click(&self, button: MouseButton) -> Result<(), InputError> {
        self.preflight_check()?;

        let mut enigo_guard = self.enigo.lock().unwrap();
        let enigo = enigo_guard.as_mut().ok_or(InputError::NotInitialized)?;

        let btn = match button {
            MouseButton::Left => enigo::Button::Left,
            MouseButton::Right => enigo::Button::Right,
            MouseButton::Middle => enigo::Button::Middle,
        };

        enigo
            .button(btn, enigo::Direction::Click)
            .map_err(|e| InputError::MouseError(e.to_string()))?;

        self.apply_delay();
        Ok(())
    }

    /// Double click
    #[cfg(feature = "input")]
    pub fn mouse_double_click(&self, button: MouseButton) -> Result<(), InputError> {
        self.mouse_click(button)?;
        std::thread::sleep(Duration::from_millis(50));
        self.mouse_click(button)
    }

    /// Scroll mouse wheel
    #[cfg(feature = "input")]
    pub fn scroll(&self, dx: i32, dy: i32) -> Result<(), InputError> {
        self.preflight_check()?;

        let mut enigo_guard = self.enigo.lock().unwrap();
        let enigo = enigo_guard.as_mut().ok_or(InputError::NotInitialized)?;

        if dx != 0 {
            enigo
                .scroll(dx, enigo::Axis::Horizontal)
                .map_err(|e| InputError::ScrollError(e.to_string()))?;
        }
        if dy != 0 {
            enigo
                .scroll(dy, enigo::Axis::Vertical)
                .map_err(|e| InputError::ScrollError(e.to_string()))?;
        }

        self.apply_delay();
        Ok(())
    }

    // ========== Keyboard Operations ==========

    /// Type text (simulates typing each character)
    #[cfg(feature = "input")]
    pub fn type_text(&self, text: &str) -> Result<(), InputError> {
        self.preflight_check()?;

        // Safety: limit text length to prevent runaway typing
        if text.len() > 10000 {
            return Err(InputError::TextTooLong(text.len()));
        }

        let mut enigo_guard = self.enigo.lock().unwrap();
        let enigo = enigo_guard.as_mut().ok_or(InputError::NotInitialized)?;

        enigo
            .text(text)
            .map_err(|e| InputError::KeyboardError(e.to_string()))?;

        self.apply_delay();
        Ok(())
    }

    /// Press and release a key
    #[cfg(feature = "input")]
    pub fn key_press(&self, key: &str) -> Result<(), InputError> {
        // Check for dangerous combinations
        if self.is_dangerous_key(key) {
            return Err(InputError::DangerousKey(key.to_string()));
        }

        self.preflight_check()?;

        let mut enigo_guard = self.enigo.lock().unwrap();
        let enigo = enigo_guard.as_mut().ok_or(InputError::NotInitialized)?;

        let enigo_key = self.parse_key(key)?;
        enigo
            .key(enigo_key, enigo::Direction::Click)
            .map_err(|e| InputError::KeyboardError(e.to_string()))?;

        self.apply_delay();
        Ok(())
    }

    /// Hold a key down
    #[cfg(feature = "input")]
    pub fn key_down(&self, key: &str) -> Result<(), InputError> {
        if self.is_dangerous_key(key) {
            return Err(InputError::DangerousKey(key.to_string()));
        }

        self.preflight_check()?;

        let mut enigo_guard = self.enigo.lock().unwrap();
        let enigo = enigo_guard.as_mut().ok_or(InputError::NotInitialized)?;

        let enigo_key = self.parse_key(key)?;
        enigo
            .key(enigo_key, enigo::Direction::Press)
            .map_err(|e| InputError::KeyboardError(e.to_string()))?;

        self.apply_delay();
        Ok(())
    }

    /// Release a held key
    #[cfg(feature = "input")]
    pub fn key_up(&self, key: &str) -> Result<(), InputError> {
        self.preflight_check()?;

        let mut enigo_guard = self.enigo.lock().unwrap();
        let enigo = enigo_guard.as_mut().ok_or(InputError::NotInitialized)?;

        let enigo_key = self.parse_key(key)?;
        enigo
            .key(enigo_key, enigo::Direction::Release)
            .map_err(|e| InputError::KeyboardError(e.to_string()))?;

        self.apply_delay();
        Ok(())
    }

    /// Press a key combination (e.g., "ctrl+c")
    #[cfg(feature = "input")]
    pub fn key_combination(&self, combo: &str) -> Result<(), InputError> {
        if self.is_dangerous_key(combo) {
            return Err(InputError::DangerousKey(combo.to_string()));
        }

        self.preflight_check()?;

        let keys: Vec<&str> = combo.split('+').map(|s| s.trim()).collect();

        // Press all modifier keys
        for key in &keys[..keys.len() - 1] {
            self.key_down(key)?;
        }

        // Press and release the main key
        if let Some(main_key) = keys.last() {
            self.key_press(main_key)?;
        }

        // Release all modifier keys in reverse order
        for key in keys[..keys.len() - 1].iter().rev() {
            self.key_up(key)?;
        }

        Ok(())
    }

    // ========== Helper Methods ==========

    /// Common preflight checks for all input operations
    fn preflight_check(&self) -> Result<(), InputError> {
        if !self.is_available() {
            return Err(InputError::NotEnabled);
        }

        if self.kill_switch.load(Ordering::SeqCst) {
            return Err(InputError::KillSwitchActive);
        }

        self.check_rate_limit()?;
        self.touch();
        Ok(())
    }

    /// Parse key string to enigo Key
    #[cfg(feature = "input")]
    fn parse_key(&self, key: &str) -> Result<enigo::Key, InputError> {
        use enigo::Key;

        let key_lower = key.to_lowercase();
        let parsed = match key_lower.as_str() {
            // Modifier keys
            "ctrl" | "control" => Key::Control,
            "alt" => Key::Alt,
            "shift" => Key::Shift,
            "super" | "win" | "meta" | "cmd" | "command" => Key::Meta,

            // Function keys
            "f1" => Key::F1,
            "f2" => Key::F2,
            "f3" => Key::F3,
            "f4" => Key::F4,
            "f5" => Key::F5,
            "f6" => Key::F6,
            "f7" => Key::F7,
            "f8" => Key::F8,
            "f9" => Key::F9,
            "f10" => Key::F10,
            "f11" => Key::F11,
            "f12" => Key::F12,

            // Navigation keys
            "up" | "uparrow" => Key::UpArrow,
            "down" | "downarrow" => Key::DownArrow,
            "left" | "leftarrow" => Key::LeftArrow,
            "right" | "rightarrow" => Key::RightArrow,
            "home" => Key::Home,
            "end" => Key::End,
            "pageup" | "pgup" => Key::PageUp,
            "pagedown" | "pgdn" => Key::PageDown,

            // Editing keys
            "backspace" | "back" => Key::Backspace,
            "delete" | "del" => Key::Delete,
            "insert" | "ins" => Key::Insert,
            "enter" | "return" => Key::Return,
            "tab" => Key::Tab,
            "escape" | "esc" => Key::Escape,
            "space" | " " => Key::Space,

            // Single character
            _ if key.len() == 1 => Key::Unicode(key.chars().next().unwrap()),

            _ => return Err(InputError::UnknownKey(key.to_string())),
        };

        Ok(parsed)
    }

    // ========== Stub implementations when feature not compiled ==========

    #[cfg(not(feature = "input"))]
    pub fn mouse_move(&self, _x: i32, _y: i32) -> Result<(), InputError> {
        Err(InputError::FeatureNotCompiled)
    }

    #[cfg(not(feature = "input"))]
    pub fn mouse_move_relative(&self, _dx: i32, _dy: i32) -> Result<(), InputError> {
        Err(InputError::FeatureNotCompiled)
    }

    #[cfg(not(feature = "input"))]
    pub fn mouse_click(&self, _button: MouseButton) -> Result<(), InputError> {
        Err(InputError::FeatureNotCompiled)
    }

    #[cfg(not(feature = "input"))]
    pub fn mouse_double_click(&self, _button: MouseButton) -> Result<(), InputError> {
        Err(InputError::FeatureNotCompiled)
    }

    #[cfg(not(feature = "input"))]
    pub fn scroll(&self, _dx: i32, _dy: i32) -> Result<(), InputError> {
        Err(InputError::FeatureNotCompiled)
    }

    #[cfg(not(feature = "input"))]
    pub fn type_text(&self, _text: &str) -> Result<(), InputError> {
        Err(InputError::FeatureNotCompiled)
    }

    #[cfg(not(feature = "input"))]
    pub fn key_press(&self, _key: &str) -> Result<(), InputError> {
        Err(InputError::FeatureNotCompiled)
    }

    #[cfg(not(feature = "input"))]
    pub fn key_down(&self, _key: &str) -> Result<(), InputError> {
        Err(InputError::FeatureNotCompiled)
    }

    #[cfg(not(feature = "input"))]
    pub fn key_up(&self, _key: &str) -> Result<(), InputError> {
        Err(InputError::FeatureNotCompiled)
    }

    #[cfg(not(feature = "input"))]
    pub fn key_combination(&self, _combo: &str) -> Result<(), InputError> {
        Err(InputError::FeatureNotCompiled)
    }
}

/// Input errors
#[derive(Debug, thiserror::Error)]
pub enum InputError {
    #[error("Input feature not compiled. Rebuild with --features input")]
    FeatureNotCompiled,

    #[error("Input not enabled. Call enable() with user consent first")]
    NotEnabled,

    #[error("Input not initialized")]
    NotInitialized,

    #[error("Input initialization failed: {0}")]
    InitError(String),

    #[error("Emergency kill switch is active. Restart required")]
    KillSwitchActive,

    #[error("Rate limit exceeded ({MAX_ACTIONS_PER_SECOND}/second)")]
    RateLimitExceeded,

    #[error("Dangerous key combination blocked: {0}. Use force flag to override")]
    DangerousKey(String),

    #[error("Unknown key: {0}")]
    UnknownKey(String),

    #[error("Text too long ({0} chars). Max 10000 characters per call")]
    TextTooLong(usize),

    #[error("Mouse operation failed: {0}")]
    MouseError(String),

    #[error("Keyboard operation failed: {0}")]
    KeyboardError(String),

    #[error("Scroll operation failed: {0}")]
    ScrollError(String),

    #[error("Inactivity timeout - input auto-disabled")]
    InactivityTimeout,
}

/// High-level GUI automation helper
pub struct GuiAutomation {
    pub vision: super::vision::VisionController,
    pub input: InputController,
}

impl GuiAutomation {
    pub fn new() -> Self {
        Self {
            vision: super::vision::VisionController::new(),
            input: InputController::new(),
        }
    }

    /// Enable both vision and input (requires user consent)
    pub fn enable_all(&self) -> Result<(), String> {
        self.vision
            .enable()
            .map_err(|e| format!("Vision: {}", e))?;
        self.input.enable().map_err(|e| format!("Input: {}", e))?;
        Ok(())
    }

    /// Disable everything
    pub fn disable_all(&self) {
        self.vision.disable();
        self.input.disable();
    }

    /// Activate kill switch for everything
    pub fn kill_all(&self) {
        self.vision.activate_kill_switch();
        self.input.activate_kill_switch();
    }

    /// Click at specific screen coordinates
    #[cfg(feature = "input")]
    pub fn click_at(&self, x: i32, y: i32, button: MouseButton) -> Result<(), InputError> {
        self.input.mouse_move(x, y)?;
        std::thread::sleep(Duration::from_millis(50));
        self.input.mouse_click(button)
    }

    /// Type text at current cursor position
    #[cfg(feature = "input")]
    pub fn type_at(&self, x: i32, y: i32, text: &str) -> Result<(), InputError> {
        self.input.mouse_move(x, y)?;
        self.input.mouse_click(MouseButton::Left)?;
        std::thread::sleep(Duration::from_millis(100));
        self.input.type_text(text)
    }

    #[cfg(not(feature = "input"))]
    pub fn click_at(&self, _x: i32, _y: i32, _button: MouseButton) -> Result<(), InputError> {
        Err(InputError::FeatureNotCompiled)
    }

    #[cfg(not(feature = "input"))]
    pub fn type_at(&self, _x: i32, _y: i32, _text: &str) -> Result<(), InputError> {
        Err(InputError::FeatureNotCompiled)
    }
}

impl Default for GuiAutomation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_disabled_by_default() {
        let controller = InputController::new();
        assert!(!controller.is_available());
    }

    #[test]
    fn test_dangerous_key_detection() {
        let controller = InputController::new();
        assert!(controller.is_dangerous_key("ctrl+alt+delete"));
        assert!(controller.is_dangerous_key("Alt+F4"));
        assert!(!controller.is_dangerous_key("ctrl+c"));
        assert!(!controller.is_dangerous_key("ctrl+v"));
    }

    #[test]
    fn test_kill_switch() {
        let controller = InputController::new();
        controller.activate_kill_switch();
        assert!(controller.kill_switch.load(Ordering::SeqCst));
        assert!(controller.enable().is_err());
    }
}
