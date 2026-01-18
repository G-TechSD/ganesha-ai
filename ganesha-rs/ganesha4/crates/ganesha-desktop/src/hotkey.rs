//! Global hotkey management

use crate::{config::DesktopConfig, Result};
use std::collections::HashMap;

/// Manages global hotkeys
pub struct HotkeyManager {
    hotkeys: HashMap<String, HotkeyBinding>,
    listening: bool,
}

impl HotkeyManager {
    /// Create a new hotkey manager
    pub fn new(config: &DesktopConfig) -> Result<Self> {
        let mut hotkeys = HashMap::new();

        // Register configured hotkeys
        hotkeys.insert(
            "push_to_talk".to_string(),
            HotkeyBinding::new(&config.hotkeys.push_to_talk, HotkeyAction::PushToTalk)?,
        );
        hotkeys.insert(
            "toggle_window".to_string(),
            HotkeyBinding::new(&config.hotkeys.toggle_window, HotkeyAction::ToggleWindow)?,
        );
        hotkeys.insert(
            "emergency_stop".to_string(),
            HotkeyBinding::new(&config.hotkeys.emergency_stop, HotkeyAction::EmergencyStop)?,
        );
        hotkeys.insert(
            "toggle_voice".to_string(),
            HotkeyBinding::new(&config.hotkeys.toggle_voice, HotkeyAction::ToggleVoice)?,
        );

        Ok(Self {
            hotkeys,
            listening: false,
        })
    }

    /// Start listening for hotkeys
    pub async fn start_listening(&mut self) -> Result<()> {
        self.listening = true;

        // In actual implementation:
        // - Use platform-specific hotkey registration
        // - Windows: RegisterHotKey / global-hotkey crate
        // - macOS: CGEventTapCreate
        // - Linux: X11 XGrabKey or libxkbcommon

        for (name, binding) in &self.hotkeys {
            tracing::info!("Registered hotkey: {} -> {:?}", binding.key_combo, binding.action);
        }

        tracing::info!("Hotkey listener started");
        Ok(())
    }

    /// Stop listening for hotkeys
    pub fn stop_listening(&mut self) -> Result<()> {
        self.listening = false;

        // Unregister all hotkeys

        tracing::info!("Hotkey listener stopped");
        Ok(())
    }

    /// Check if listening
    pub fn is_listening(&self) -> bool {
        self.listening
    }

    /// Update a hotkey binding
    pub fn rebind(&mut self, name: &str, key_combo: &str) -> Result<()> {
        if let Some(binding) = self.hotkeys.get_mut(name) {
            binding.key_combo = key_combo.to_string();
            binding.modifiers = parse_modifiers(key_combo);
            binding.key = parse_key(key_combo);

            tracing::info!("Hotkey {} rebound to {}", name, key_combo);
        }
        Ok(())
    }

    /// Get all registered hotkeys
    pub fn list_hotkeys(&self) -> Vec<(&str, &str)> {
        self.hotkeys
            .iter()
            .map(|(name, binding)| (name.as_str(), binding.key_combo.as_str()))
            .collect()
    }

    /// Handle a hotkey press event
    pub fn handle_keypress(&self, modifiers: Modifiers, key: Key) -> Option<HotkeyAction> {
        for binding in self.hotkeys.values() {
            if binding.modifiers == modifiers && binding.key == key {
                return Some(binding.action.clone());
            }
        }
        None
    }
}

/// A hotkey binding
#[derive(Debug, Clone)]
pub struct HotkeyBinding {
    /// The key combination string (e.g., "Ctrl+Shift+G")
    pub key_combo: String,
    /// Parsed modifiers
    pub modifiers: Modifiers,
    /// Parsed key
    pub key: Key,
    /// Action to perform
    pub action: HotkeyAction,
}

impl HotkeyBinding {
    /// Create a new hotkey binding
    pub fn new(key_combo: &str, action: HotkeyAction) -> Result<Self> {
        Ok(Self {
            key_combo: key_combo.to_string(),
            modifiers: parse_modifiers(key_combo),
            key: parse_key(key_combo),
            action,
        })
    }
}

/// Modifier keys
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool, // Windows key / Command key
}

impl Modifiers {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn ctrl() -> Self {
        Self {
            ctrl: true,
            ..Default::default()
        }
    }

    pub fn ctrl_shift() -> Self {
        Self {
            ctrl: true,
            shift: true,
            ..Default::default()
        }
    }
}

/// Key codes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    /// Letter key
    Letter(char),
    /// Function key
    Function(u8),
    /// Space bar
    Space,
    /// Escape
    Escape,
    /// Enter
    Enter,
    /// Tab
    Tab,
    /// Backspace
    Backspace,
    /// Arrow keys
    Up,
    Down,
    Left,
    Right,
    /// Unknown key
    Unknown(String),
}

impl Default for Key {
    fn default() -> Self {
        Key::Unknown("".to_string())
    }
}

/// Actions triggered by hotkeys
#[derive(Debug, Clone)]
pub enum HotkeyAction {
    /// Start/stop push-to-talk
    PushToTalk,
    /// Toggle window visibility
    ToggleWindow,
    /// Emergency stop all operations
    EmergencyStop,
    /// Toggle voice mode
    ToggleVoice,
    /// Custom action with name
    Custom(String),
}

/// Parse modifiers from key combo string
fn parse_modifiers(combo: &str) -> Modifiers {
    let lower = combo.to_lowercase();
    Modifiers {
        ctrl: lower.contains("ctrl"),
        alt: lower.contains("alt"),
        shift: lower.contains("shift"),
        meta: lower.contains("meta") || lower.contains("super") || lower.contains("win") || lower.contains("cmd"),
    }
}

/// Parse key from key combo string
fn parse_key(combo: &str) -> Key {
    // Get the last part after all modifiers
    let parts: Vec<&str> = combo.split('+').collect();
    let key_part = parts.last().unwrap_or(&"").trim().to_lowercase();

    match key_part.as_str() {
        "space" => Key::Space,
        "escape" | "esc" => Key::Escape,
        "enter" | "return" => Key::Enter,
        "tab" => Key::Tab,
        "backspace" => Key::Backspace,
        "up" => Key::Up,
        "down" => Key::Down,
        "left" => Key::Left,
        "right" => Key::Right,
        s if s.starts_with('f') && s.len() <= 3 => {
            if let Ok(n) = s[1..].parse::<u8>() {
                Key::Function(n)
            } else {
                Key::Unknown(key_part)
            }
        }
        s if s.len() == 1 => {
            Key::Letter(s.chars().next().unwrap())
        }
        _ => Key::Unknown(key_part),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_modifiers() {
        let mods = parse_modifiers("Ctrl+Shift+G");
        assert!(mods.ctrl);
        assert!(mods.shift);
        assert!(!mods.alt);
        assert!(!mods.meta);
    }

    #[test]
    fn test_parse_key() {
        assert_eq!(parse_key("Ctrl+Space"), Key::Space);
        assert_eq!(parse_key("Ctrl+Shift+G"), Key::Letter('g'));
        assert_eq!(parse_key("Escape"), Key::Escape);
        assert_eq!(parse_key("F1"), Key::Function(1));
    }
}
